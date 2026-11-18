use socket2::SockAddr;
use std::{
    ffi::{OsStr, OsString},
    fmt, io,
    mem::MaybeUninit,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    os::windows::ffi::{OsStrExt, OsStringExt},
    ptr,
    sync::Mutex,
    time::{Duration, Instant},
};
use talpid_types::win32_err;
use windows_sys::{
    Win32::{
        Foundation::{ERROR_NOT_FOUND, HANDLE},
        NetworkManagement::{
            IpHelper::{
                CancelMibChangeNotify2, ConvertInterfaceAliasToLuid, ConvertInterfaceLuidToAlias,
                ConvertInterfaceLuidToGuid, ConvertInterfaceLuidToIndex,
                CreateUnicastIpAddressEntry, FreeMibTable, GetIpInterfaceEntry,
                GetUnicastIpAddressEntry, GetUnicastIpAddressTable,
                InitializeUnicastIpAddressEntry, MIB_IPINTERFACE_ROW, MIB_UNICASTIPADDRESS_ROW,
                MIB_UNICASTIPADDRESS_TABLE, MibAddInstance, NotifyIpInterfaceChange,
                SetIpInterfaceEntry,
            },
            Ndis::{IF_MAX_STRING_SIZE, NET_LUID_LH},
        },
        Networking::WinSock::{
            AF_INET, AF_INET6, AF_UNSPEC, IN_ADDR, IN6_ADDR, IpDadStateDeprecated,
            IpDadStateDuplicate, IpDadStateInvalid, IpDadStatePreferred, IpDadStateTentative,
            NL_DAD_STATE, SOCKADDR_IN as sockaddr_in, SOCKADDR_IN6 as sockaddr_in6, SOCKADDR_INET,
        },
    },
    core::GUID,
};

/// Result type for this module.
pub type Result<T> = std::result::Result<T, Error>;

const DAD_CHECK_TIMEOUT: Duration = Duration::from_secs(5);
const DAD_CHECK_INTERVAL: Duration = Duration::from_millis(100);

/// Errors returned by some functions in this module.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Error returned from `ConvertInterfaceAliasToLuid`
    #[cfg(windows)]
    #[error("Cannot find LUID for virtual adapter")]
    NoDeviceLuid(#[source] io::Error),

    /// Error returned from `GetUnicastIpAddressTable`/`GetUnicastIpAddressEntry`
    #[cfg(windows)]
    #[error("Failed to obtain unicast IP address table")]
    ObtainUnicastAddress(#[source] io::Error),

    /// `GetUnicastIpAddressTable` contained no addresses for the interface
    #[cfg(windows)]
    #[error("Found no addresses for the given adapter")]
    NoUnicastAddress,

    /// Error returned from `CreateUnicastIpAddressEntry`
    #[cfg(windows)]
    #[error("Failed to create unicast IP address")]
    CreateUnicastEntry(#[source] io::Error),

    /// Unexpected DAD state returned for a unicast address
    #[cfg(windows)]
    #[error("Unexpected DAD state")]
    DadStateError(#[source] DadStateError),

    /// DAD check failed.
    #[cfg(windows)]
    #[error("Timed out waiting on tunnel device")]
    DeviceReadyTimeout,

    /// Unicast DAD check fail.
    #[cfg(windows)]
    #[error("Unicast channel sender was unexpectedly dropped")]
    UnicastSenderDropped,

    /// Unknown address family
    #[error("Unknown address family: {0}")]
    UnknownAddressFamily(u16),
}

/// Handles cases where there DAD state is neither tentative nor preferred.
#[derive(thiserror::Error, Debug)]
pub enum DadStateError {
    /// Invalid DAD state.
    #[error("Invalid DAD state")]
    Invalid,

    /// Duplicate unicast address.
    #[error("A duplicate IP address was detected")]
    Duplicate,

    /// Deprecated unicast address.
    #[error("The IP address has been deprecated")]
    Deprecated,

    /// Unknown DAD state constant.
    #[error("Unknown DAD state: {0}")]
    Unknown(i32),
}

#[expect(non_upper_case_globals)]
impl From<NL_DAD_STATE> for DadStateError {
    fn from(state: NL_DAD_STATE) -> DadStateError {
        match state {
            IpDadStateInvalid => DadStateError::Invalid,
            IpDadStateDuplicate => DadStateError::Duplicate,
            IpDadStateDeprecated => DadStateError::Deprecated,
            other => DadStateError::Unknown(other),
        }
    }
}

impl AddressFamily {
    /// Convert one of the `AF_*` constants to an [`AddressFamily`].
    pub fn try_from_af_family(family: u16) -> Result<AddressFamily> {
        match family {
            AF_INET => Ok(AddressFamily::Ipv4),
            AF_INET6 => Ok(AddressFamily::Ipv6),
            family => Err(Error::UnknownAddressFamily(family)),
        }
    }

    /// Convert an [`AddressFamily`] to one of the `AF_*` constants.
    pub fn to_af_family(&self) -> u16 {
        match self {
            Self::Ipv4 => AF_INET,
            Self::Ipv6 => AF_INET6,
        }
    }
}

/// Context for [`notify_ip_interface_change`]. When it is dropped,
/// the callback is unregistered.
pub struct IpNotifierHandle {
    #[expect(clippy::type_complexity)]
    callback: Mutex<Box<dyn FnMut(&MIB_IPINTERFACE_ROW, i32) + Send>>,
    handle: HANDLE,
}

// SAFETY: `callback` is `Send` (the bound on `notify_ip_interface_change`) and sits behind a
// `Mutex`. The only non-`Send` field is `handle`, a notification handle owned by this struct that
// any thread may pass to `CancelMibChangeNotify2`; `Drop` does so exactly once.
unsafe impl Send for IpNotifierHandle {}

impl Drop for IpNotifierHandle {
    fn drop(&mut self) {
        // SAFETY: `handle` is either the handle stored by a successful `NotifyIpInterfaceChange`,
        // cancelled only here, or null when the registration failed and `?` dropped the box, a
        // value `CancelMibChangeNotify2` rejects with `ERROR_INVALID_PARAMETER`, ignored here.
        // Cancelling waits for a callback in progress, so the box the callbacks borrow is freed
        // only after the last of them.
        unsafe { CancelMibChangeNotify2(self.handle) };
    }
}

unsafe extern "system" fn inner_callback(
    context: *const std::ffi::c_void,
    row: *const MIB_IPINTERFACE_ROW,
    notify_type: i32,
) {
    // SAFETY: `context` is the address of the boxed `IpNotifierHandle` registered by
    // `notify_ip_interface_change`, which stays allocated until `Drop` has cancelled the
    // notification; a box leaked instead of dropped keeps it allocated forever, and the callback
    // is `'static`, so it borrows nothing that could end first. `row` is non-null because the
    // registration asks for no initial notification (the only case where Windows passes null),
    // and points to a row that is valid for the duration of the callback. Windows may run
    // callbacks on several threads at once, so the context is only borrowed shared and the
    // callback is reached through the `Mutex`.
    unsafe {
        let context = &*(context as *const IpNotifierHandle);
        context
            .callback
            .lock()
            .expect("NotifyIpInterfaceChange mutex poisoned")(&*row, notify_type);
    }
}

/// Registers a callback function that is invoked when an interface is added, removed,
/// or changed.
pub fn notify_ip_interface_change<T: FnMut(&MIB_IPINTERFACE_ROW, i32) + Send + 'static>(
    callback: T,
    family: Option<AddressFamily>,
) -> io::Result<Box<IpNotifierHandle>> {
    let mut context = Box::new(IpNotifierHandle {
        callback: Mutex::new(Box::new(callback)),
        handle: ptr::null_mut(),
    });

    // SAFETY: the context pointer is the heap address of `context`, which does not move when the
    // box is returned, and `IpNotifierHandle::drop` cancels the registration before freeing it.
    // `inner_callback` casts the context back to exactly this type. The handle out-pointer refers
    // to a field of the same box.
    win32_err!(unsafe {
        NotifyIpInterfaceChange(
            af_family_from_family(family),
            Some(inner_callback),
            (&raw mut *context).cast(),
            false,
            &raw mut context.handle,
        )
    })?;
    Ok(context)
}

/// Returns information about a network IP interface.
pub fn get_ip_interface_entry(
    family: AddressFamily,
    luid: &NET_LUID_LH,
) -> io::Result<MIB_IPINTERFACE_ROW> {
    let mut row = MIB_IPINTERFACE_ROW {
        Family: family as u16,
        InterfaceLuid: *luid,
        ..Default::default()
    };

    // SAFETY: `row` is a live, initialised local with `Family` and `InterfaceLuid` set, which is
    // what `GetIpInterfaceEntry` reads before filling in the rest of the row.
    win32_err!(unsafe { GetIpInterfaceEntry(&raw mut row) })?;
    Ok(row)
}

/// Set the properties of an IP interface.
pub fn set_ip_interface_entry(row: &mut MIB_IPINTERFACE_ROW) -> io::Result<()> {
    // SAFETY: `row` is an exclusive reference to an initialised row, valid for the whole
    // synchronous call.
    win32_err!(unsafe { SetIpInterfaceEntry(row as *mut _) })
}

fn ip_interface_entry_exists(family: AddressFamily, luid: &NET_LUID_LH) -> io::Result<bool> {
    match get_ip_interface_entry(family, luid) {
        Ok(_) => Ok(true),
        Err(error) if error.raw_os_error() == Some(ERROR_NOT_FOUND as i32) => Ok(false),
        Err(error) => Err(error),
    }
}

/// Waits until the specified IP interfaces have attached to a given network interface.
pub async fn wait_for_interfaces(luid: NET_LUID_LH, ipv4: bool, ipv6: bool) -> io::Result<()> {
    let (tx, rx) = futures::channel::oneshot::channel();

    let mut found_ipv4 = !ipv4;
    let mut found_ipv6 = !ipv6;

    let mut tx = Some(tx);

    let _handle = notify_ip_interface_change(
        move |row, notification_type| {
            if found_ipv4 && found_ipv6 {
                return;
            }
            if notification_type != MibAddInstance {
                return;
            }
            // SAFETY: `NET_LUID_LH` is a union of a `u64` and a bitfield view of the same 8
            // bytes, so reading `Value` is valid for any initialised LUID.
            if unsafe { row.InterfaceLuid.Value != luid.Value } {
                return;
            }
            match row.Family {
                AF_INET => found_ipv4 = true,
                AF_INET6 => found_ipv6 = true,
                _ => (),
            }
            if found_ipv4
                && found_ipv6
                && let Some(tx) = tx.take()
            {
                let _ = tx.send(());
            }
        },
        None,
    )?;

    // Make sure they don't already exist
    if (!ipv4 || ip_interface_entry_exists(AddressFamily::Ipv4, &luid)?)
        && (!ipv6 || ip_interface_entry_exists(AddressFamily::Ipv6, &luid)?)
    {
        return Ok(());
    }

    let _ = rx.await;
    Ok(())
}

/// Wait for addresses to be usable on an network adapter.
pub async fn wait_for_addresses(luid: NET_LUID_LH) -> Result<()> {
    // Obtain unicast IP addresses
    let mut unicast_rows: Vec<MIB_UNICASTIPADDRESS_ROW> = get_unicast_table(None)
        .map_err(Error::ObtainUnicastAddress)?
        .into_iter()
        // SAFETY: `NET_LUID_LH` is a union of a `u64` and a bitfield view of the same 8 bytes, so
        // reading `Value` is valid for any initialised LUID.
        .filter(|row| unsafe { row.InterfaceLuid.Value == luid.Value })
        .collect();
    if unicast_rows.is_empty() {
        return Err(Error::NoUnicastAddress);
    }

    let (tx, rx) = futures::channel::oneshot::channel();
    let mut addr_check_thread = move || {
        // Poll DAD status using GetUnicastIpAddressEntry
        // https://docs.microsoft.com/en-us/windows/win32/api/netioapi/nf-netioapi-createunicastipaddressentry

        let deadline = Instant::now() + DAD_CHECK_TIMEOUT;
        while Instant::now() < deadline {
            let mut ready = true;

            for row in &mut unicast_rows {
                // SAFETY: `row` is an exclusive reference to a row copied out of the unicast
                // table, with `Address` and `InterfaceLuid` set as `GetUnicastIpAddressEntry`
                // requires, valid for the whole synchronous call.
                win32_err!(unsafe { GetUnicastIpAddressEntry(row) })
                    .map_err(Error::ObtainUnicastAddress)?;
                if row.DadState == IpDadStateTentative {
                    ready = false;
                    break;
                }
                if row.DadState != IpDadStatePreferred {
                    return Err(Error::DadStateError(DadStateError::from(row.DadState)));
                }
            }

            if ready {
                return Ok(());
            }
            std::thread::sleep(DAD_CHECK_INTERVAL);
        }

        Err(Error::DeviceReadyTimeout)
    };
    std::thread::spawn(move || {
        let _ = tx.send(addr_check_thread());
    });
    rx.await.map_err(|_| Error::UnicastSenderDropped)?
}

/// Returns the first unicast IP address for the given interface.
pub fn get_ip_address_for_interface(
    family: AddressFamily,
    luid: NET_LUID_LH,
) -> Result<Option<IpAddr>> {
    match get_unicast_table(Some(family))
        .map_err(Error::ObtainUnicastAddress)?
        .into_iter()
        // SAFETY: `NET_LUID_LH` is a union of a `u64` and a bitfield view of the same 8 bytes, so
        // reading `Value` is valid for any initialised LUID.
        .find(|row| unsafe { row.InterfaceLuid.Value == luid.Value })
    {
        Some(row) => Ok(Some(try_socketaddr_from_inet_sockaddr(row.Address)?.ip())),
        None => Ok(None),
    }
}

/// Adds a unicast IP address for the given interface.
pub fn add_ip_address_for_interface(luid: NET_LUID_LH, address: IpAddr) -> Result<()> {
    let mut row = MIB_UNICASTIPADDRESS_ROW::default();
    // SAFETY: `row` is a live local; the function only writes default values into it.
    unsafe { InitializeUnicastIpAddressEntry(&raw mut row) };

    row.InterfaceLuid = luid;
    row.Address = inet_sockaddr_from_socketaddr(SocketAddr::new(address, 0));
    row.DadState = IpDadStatePreferred;
    row.OnLinkPrefixLength = 255;

    // SAFETY: `row` is a fully initialised local, valid for the whole synchronous call, which
    // only reads it.
    win32_err!(unsafe { CreateUnicastIpAddressEntry(&raw const row) })
        .map_err(Error::CreateUnicastEntry)
}

/// Sets MTU on the specified network interface identified by `luid`.
pub fn set_mtu(mtu: u32, luid: NET_LUID_LH, ip_family: AddressFamily) -> io::Result<()> {
    let mut row = get_ip_interface_entry(ip_family, &luid)?;

    row.NlMtu = mtu;

    set_ip_interface_entry(&mut row)
}

/// Returns the unicast IP address table. If `family` is `None`, then addresses for all families are
/// returned.
pub fn get_unicast_table(
    family: Option<AddressFamily>,
) -> io::Result<Vec<MIB_UNICASTIPADDRESS_ROW>> {
    let mut unicast_rows = vec![];
    let mut unicast_table: *mut MIB_UNICASTIPADDRESS_TABLE = std::ptr::null_mut();

    // SAFETY: the out-pointer refers to a local; on success Windows stores in it a table that it
    // allocated and that is released with `FreeMibTable` below.
    win32_err!(unsafe {
        GetUnicastIpAddressTable(af_family_from_family(family), &raw mut unicast_table)
    })?;
    // SAFETY: the call succeeded, so `unicast_table` points to a valid table. Only the address
    // of the first row is taken here; rows are read below only for indices under `NumEntries`.
    let first_row = unsafe { &(*unicast_table).Table[0] } as *const MIB_UNICASTIPADDRESS_ROW;
    // SAFETY: `unicast_table` points to a valid table. The dereference copies the whole struct,
    // one-row `Table` included, so it assumes the allocation covers
    // `size_of::<MIB_UNICASTIPADDRESS_TABLE>()`: true whenever the table holds at least one row,
    // and the loopback addresses are always present.
    for i in 0..unsafe { *unicast_table }.NumEntries {
        // SAFETY: Windows lays out `NumEntries` contiguous rows starting at `Table[0]`, and
        // `i < NumEntries`, so the read stays inside the table allocation.
        unicast_rows.push(unsafe { *(first_row.offset(i as isize)) });
    }
    // SAFETY: `unicast_table` was allocated by `GetUnicastIpAddressTable`, is freed exactly once,
    // and every row was copied out before this point.
    unsafe { FreeMibTable(unicast_table as *const _) };

    Ok(unicast_rows)
}

/// Returns the index of a network interface given its LUID.
pub fn index_from_luid(luid: &NET_LUID_LH) -> io::Result<u32> {
    let mut index = 0u32;
    // SAFETY: `luid` is a reference, so it is valid for reads, and the out-pointer refers to a
    // local `u32`, the documented output type.
    win32_err!(unsafe { ConvertInterfaceLuidToIndex(luid, &raw mut index) })?;
    Ok(index)
}

/// Returns the GUID of a network interface given its LUID.
pub fn guid_from_luid(luid: &NET_LUID_LH) -> io::Result<GUID> {
    let mut guid = MaybeUninit::zeroed();
    // SAFETY: `luid` is a reference, so it is valid for reads, and the out-pointer refers to a
    // local `MaybeUninit<GUID>`, writable for one `GUID`.
    win32_err!(unsafe { ConvertInterfaceLuidToGuid(luid, guid.as_mut_ptr()) })?;
    // SAFETY: the buffer was zeroed, which is already a valid `GUID` (plain integers), and the
    // successful call above overwrote it.
    Ok(unsafe { guid.assume_init() })
}

/// Returns the LUID of an interface given its alias.
pub fn luid_from_alias<T: AsRef<OsStr>>(alias: T) -> io::Result<NET_LUID_LH> {
    let alias_wide: Vec<u16> = alias
        .as_ref()
        .encode_wide()
        .chain(std::iter::once(0u16))
        .collect();
    let mut luid = NET_LUID_LH::default();
    // SAFETY: `alias_wide` is NUL-terminated by the `chain` above and outlives the call, and the
    // out-pointer refers to a local `NET_LUID_LH`.
    win32_err!(unsafe { ConvertInterfaceAliasToLuid(alias_wide.as_ptr(), &raw mut luid) })?;
    Ok(luid)
}

/// Returns the alias of an interface given its LUID.
pub fn alias_from_luid(luid: &NET_LUID_LH) -> io::Result<OsString> {
    let mut buffer = [0u16; IF_MAX_STRING_SIZE as usize + 1];
    // SAFETY: `buffer` is a local array and its exact length in `u16` units is passed with it;
    // `IF_MAX_STRING_SIZE + 1` is the size the API documents as large enough for any alias.
    win32_err!(unsafe { ConvertInterfaceLuidToAlias(luid, buffer.as_mut_ptr(), buffer.len()) })?;
    let nul = buffer.iter().position(|&c| c == 0u16).unwrap();
    Ok(OsString::from_wide(&buffer[0..nul]))
}

fn af_family_from_family(family: Option<AddressFamily>) -> u16 {
    family.map(|family| family as u16).unwrap_or(AF_UNSPEC)
}

/// Converts an `Ipv4Addr` to `IN_ADDR`
pub fn inaddr_from_ipaddr(addr: Ipv4Addr) -> IN_ADDR {
    let sockaddr = SockAddr::from(SocketAddr::V4(SocketAddrV4::new(addr, 0)));
    // SAFETY: a `SockAddr` built from a V4 address holds an initialised `sockaddr_in` at the start
    // of its `sockaddr_storage`, which is aligned for any socket address type.
    unsafe { *(sockaddr.as_ptr() as *const sockaddr_in) }.sin_addr
}

/// Converts an `Ipv6Addr` to `IN6_ADDR`
pub fn in6addr_from_ipaddr(addr: Ipv6Addr) -> IN6_ADDR {
    let sockaddr = SockAddr::from(SocketAddr::V6(SocketAddrV6::new(addr, 0, 0, 0)));
    // SAFETY: a `SockAddr` built from a V6 address holds an initialised `sockaddr_in6` at the
    // start of its `sockaddr_storage`, which is aligned for any socket address type.
    unsafe { *(sockaddr.as_ptr() as *const sockaddr_in6) }.sin6_addr
}

/// Converts an `IN_ADDR` to `Ipv4Addr`
pub fn ipaddr_from_inaddr(addr: IN_ADDR) -> Ipv4Addr {
    // SAFETY: every variant of the `S_un` union is a plain integer view of the same 4 bytes, so
    // any initialised `IN_ADDR` is a valid `S_addr`.
    Ipv4Addr::from(unsafe { addr.S_un.S_addr }.to_ne_bytes())
}

/// Converts an `IN6_ADDR` to `Ipv6Addr`
pub fn ipaddr_from_in6addr(addr: IN6_ADDR) -> Ipv6Addr {
    // SAFETY: every variant of the `u` union is a plain integer view of the same 16 bytes, so
    // any initialised `IN6_ADDR` is a valid `Byte` array.
    Ipv6Addr::from(unsafe { addr.u.Byte })
}

/// Converts a `SocketAddr` to `SOCKADDR_INET`
pub fn inet_sockaddr_from_socketaddr(addr: SocketAddr) -> SOCKADDR_INET {
    // SAFETY: SOCKADDR_INET is a union of C structs, these can be safely zeroed.
    let mut sockaddr = SOCKADDR_INET::default();
    match addr {
        // SAFETY: `*const sockaddr` may be treated as `*const sockaddr_in` since we know it's a v4
        // address.
        SocketAddr::V4(_) => unsafe {
            sockaddr.Ipv4 = *(SockAddr::from(addr).as_ptr() as *const _)
        },
        // SAFETY: `*const sockaddr` may be treated as `*const sockaddr_in6` since we know it's a v6
        // address.
        SocketAddr::V6(_) => unsafe {
            sockaddr.Ipv6 = *(SockAddr::from(addr).as_ptr() as *const _)
        },
    }
    sockaddr
}

/// Converts a `SOCKADDR_INET` to `SocketAddr`. Returns an error if the address family is invalid.
pub fn try_socketaddr_from_inet_sockaddr(addr: SOCKADDR_INET) -> Result<SocketAddr> {
    // SAFETY: si_family is always valid
    let family = unsafe { addr.si_family };
    match family {
        AF_INET => {
            // SAFETY: We know this is an IPv4 address based on the family
            let ipv4_addr = unsafe { addr.Ipv4 };
            // SAFETY: The IPv4 address is initialized
            let ip = Ipv4Addr::from(u32::from_be(unsafe { ipv4_addr.sin_addr.S_un.S_addr }));
            let port = u16::from_be(ipv4_addr.sin_port);
            Ok(SocketAddr::V4(SocketAddrV4::new(ip, port)))
        }
        AF_INET6 => {
            // SAFETY: We know this is an IPv6 address based on the family
            let ipv6_addr = unsafe { addr.Ipv6 };
            // SAFETY: The IPv6 address is initialized
            let ip = Ipv6Addr::from(unsafe { ipv6_addr.sin6_addr.u.Byte });
            let port = u16::from_be(ipv6_addr.sin6_port);
            let flowinfo = ipv6_addr.sin6_flowinfo;
            // SAFETY: The scope ID is initialized
            let scope_id = unsafe { ipv6_addr.Anonymous.sin6_scope_id };
            Ok(SocketAddr::V6(SocketAddrV6::new(
                ip, port, flowinfo, scope_id,
            )))
        }
        _ => Err(Error::UnknownAddressFamily(family)),
    }
}

/// Address family. These correspond to the `AF_*` constants.
#[derive(Debug, Clone, Copy)]
pub enum AddressFamily {
    /// IPv4 address family
    Ipv4 = AF_INET as isize,
    /// IPv6 address family
    Ipv6 = AF_INET6 as isize,
}

impl fmt::Display for AddressFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            AddressFamily::Ipv4 => write!(f, "IPv4 (AF_INET)"),
            AddressFamily::Ipv6 => write!(f, "IPv6 (AF_INET6)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sockaddr_v4() {
        let addr_v4 = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(1, 2, 3, 4), 1234));
        assert_eq!(
            addr_v4,
            try_socketaddr_from_inet_sockaddr(inet_sockaddr_from_socketaddr(addr_v4)).unwrap()
        );
    }

    #[test]
    fn test_sockaddr_v6() {
        let addr_v6 = SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8),
            1234,
            0xa,
            0xb,
        ));
        assert_eq!(
            addr_v6,
            try_socketaddr_from_inet_sockaddr(inet_sockaddr_from_socketaddr(addr_v6)).unwrap()
        );
    }
}
