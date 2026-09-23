//! Who may drive this machine's VPN and reach its wallet.
//!
//! The daemon runs as root (SYSTEM on Windows) and serves one management
//! endpoint that every local account can connect to: a Unix socket with mode
//! 0766, a named pipe that authenticated users may open. Connecting grants
//! nothing. Every connection carries the identity the OS reported for it (the
//! socket peer's uid, the pipe client's token SID), and every RPC is admitted
//! by [`decide`] from that identity, the RPC's [`RpcClass`] and the wallet's
//! [`Ownership`], before its request body is read. The table that gives each
//! RPC its class is `crate::rpc_access`.
//!
//! # The owner
//!
//! The account that installs a wallet (creates one, imports one, or logs in)
//! becomes its owner, and the daemon records it in [`OWNER_FILENAME`] in its
//! settings directory, which only root or SYSTEM can write. The record
//! survives a daemon restart, so a restart never reopens a race to claim the
//! wallet. A true sign-out, which erases the mnemonic, releases it.
//!
//! Once there is an owner, only the owner and administrators (root, the
//! daemon's own account, SYSTEM, an elevated member of Administrators) may
//! change the tunnel, the firewall and the settings, or reach the wallet, the
//! account and the forum signatures. Every other account keeps read access to
//! state that carries no identity, and what it reads has the owner's identity
//! material and secrets removed ([`may_see_identity`]).
//!
//! # A wallet with no recorded owner
//!
//! A wallet installed before owners were recorded, or installed by an
//! administrator, has no owner. It is not first come, first served: only the
//! account at the computer's own screen may claim it (the console user on
//! macOS, the active console session on Windows, the active user of a logind
//! seat on Linux), and its first control or wallet call does. Any other
//! account is refused until then. Administrators never claim, so an
//! administrator's command never takes the wallet from the desktop user. A
//! mnemonic that is stored but cannot be loaded, or that the storage cannot be
//! read about, counts as installed.
//!
//! # No wallet
//!
//! With no wallet installed, any local account may set Warren up, and the one
//! that installs the wallet becomes its owner. The claim is written before the
//! install runs, under a lock, so no other account can act between the
//! mnemonic reaching the disk and the owner being known. Until then only the
//! console account and administrators may change the tunnel and the settings:
//! whatever is set before the wallet exists is what its owner inherits.
//!
//! # When the decision is made
//!
//! A call is admitted twice. The gate decides on the request head, before the
//! body is read, so a refused account never gets a request buffer. The client
//! chooses when to send the body, so the service decides again when it acts,
//! in [`WalletAccessControl::admit_then`], holding the ownership while the
//! daemon command is queued. Commands run in the order they are queued, and
//! an install records its owner before queueing its own command, so no call
//! admitted under an earlier ownership can run against a later one.
//!
//! A record that cannot be read leaves the wallet to administrators alone,
//! until one of them removes the record.

use std::{
    io,
    path::{Path, PathBuf},
    sync::{
        Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
};

pub use mullvad_management_interface::{PeerCredentials, Principal};

/// Name of the owner record in the daemon's settings directory.
pub const OWNER_FILENAME: &str = mullvad_paths::WALLET_OWNER_FILENAME;

/// What an RPC can do, which is what decides who may call it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcClass {
    /// Reads state that carries no identity: tunnel state, versions, relays,
    /// settings. Open to every local account; identity material and secrets
    /// are removed from what a non-owner receives.
    ReadPublic,
    /// Changes the tunnel, the firewall, DNS, the settings, the relays, split
    /// tunneling, updates, or the daemon itself.
    ControlMachine,
    /// Reaches the wallet, the account, the device identity, a secret, or a
    /// forum signature made with the wallet key.
    Identity,
    /// Puts a wallet in place: create, import, login.
    InstallWallet,
}

/// Who owns the wallet, as far as authorization is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership<'a> {
    /// No wallet is installed.
    Vacant,
    /// A wallet is installed and has no recorded owner.
    Unclaimed,
    /// The wallet belongs to this account.
    Owned(&'a Principal),
    /// The owner record exists and cannot be read.
    Unknown,
}

/// The answer to one call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allow,
    /// Allowed, and the caller becomes the wallet's owner.
    AllowAsNewOwner,
    Deny(Refusal),
}

/// Why a call was refused. The message is what the caller is shown, and
/// [`Refusal::code`] is what a client decides on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Refusal {
    #[error("Warren could not tell which account is calling; only read-only requests are allowed")]
    NoCredentials,
    #[error("Warren is set up by another account on this computer")]
    OwnedByAnotherAccount,
    #[error(
        "Warren is set up on this computer and has no owner yet: open the Warren app from the \
         account signed in at the computer's screen, or run this as an administrator"
    )]
    ClaimNeedsConsoleUser,
    #[error(
        "Warren is not set up on this computer yet: create or import an account first, or run \
         this as an administrator"
    )]
    SetUpFirst,
    #[error(
        "Warren cannot read which account set it up on this computer; ask an administrator to \
         reset it"
    )]
    OwnerRecordUnreadable,
}

impl Refusal {
    /// The stable, machine-readable name of the reason, sent as the details of
    /// the refused call's status.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::NoCredentials => "no_credentials",
            Self::OwnedByAnotherAccount => "owned_by_another_account",
            Self::ClaimNeedsConsoleUser => "claim_needs_console_user",
            Self::SetUpFirst => "set_up_first",
            Self::OwnerRecordUnreadable => "owner_record_unreadable",
        }
    }
}

/// The authorization of one call, and the whole of it.
///
/// `is_console_user` is only asked when a wallet with no owner is at stake.
#[must_use]
pub fn decide(
    peer: Option<&PeerCredentials>,
    class: RpcClass,
    ownership: Ownership<'_>,
    is_console_user: impl Fn(&PeerCredentials) -> bool,
) -> Decision {
    if class == RpcClass::ReadPublic {
        return Decision::Allow;
    }
    let Some(peer) = peer else {
        return Decision::Deny(Refusal::NoCredentials);
    };
    if peer.privileged {
        return Decision::Allow;
    }
    match ownership {
        Ownership::Owned(owner) if *owner == peer.principal => Decision::Allow,
        Ownership::Owned(_) => Decision::Deny(Refusal::OwnedByAnotherAccount),
        Ownership::Unknown => Decision::Deny(Refusal::OwnerRecordUnreadable),
        Ownership::Vacant if class == RpcClass::InstallWallet => Decision::AllowAsNewOwner,
        // Nothing to reach yet, and the reads the app makes before it offers
        // to create an account must work for whoever opens it.
        Ownership::Vacant if class == RpcClass::Identity => Decision::Allow,
        Ownership::Vacant if is_console_user(peer) => Decision::Allow,
        Ownership::Vacant => Decision::Deny(Refusal::SetUpFirst),
        Ownership::Unclaimed if is_console_user(peer) => Decision::AllowAsNewOwner,
        Ownership::Unclaimed => Decision::Deny(Refusal::ClaimNeedsConsoleUser),
    }
}

/// Whether what `peer` reads may carry the owner's identity material (account
/// number, device, voucher codes) and secrets (proxy credentials).
#[must_use]
pub fn may_see_identity(peer: Option<&PeerCredentials>, ownership: Ownership<'_>) -> bool {
    match (peer, ownership) {
        (Some(peer), _) if peer.privileged => true,
        (Some(peer), Ownership::Owned(owner)) => peer.principal == *owner,
        _ => false,
    }
}

/// The on-disk form of the owner record.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct OwnerRecord {
    version: u32,
    owner: StoredPrincipal,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredPrincipal {
    Uid(u32),
    Sid(String),
}

const OWNER_RECORD_VERSION: u32 = 1;

impl From<&Principal> for StoredPrincipal {
    fn from(principal: &Principal) -> Self {
        match principal {
            Principal::Uid(uid) => Self::Uid(*uid),
            Principal::Sid(sid) => Self::Sid(sid.clone()),
        }
    }
}

impl From<StoredPrincipal> for Principal {
    fn from(stored: StoredPrincipal) -> Self {
        match stored {
            StoredPrincipal::Uid(uid) => Self::Uid(uid),
            StoredPrincipal::Sid(sid) => Self::Sid(sid),
        }
    }
}

/// Failure to read or write the owner record.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum OwnerStoreError {
    #[error("Failed to read the wallet owner record")]
    Read(#[source] io::Error),
    #[error("The wallet owner record is not valid")]
    Parse(#[source] serde_json::Error),
    #[error("The wallet owner record has an unknown version")]
    UnknownVersion,
    #[error("Failed to write the wallet owner record")]
    Write(#[source] io::Error),
}

/// The persisted wallet owner, in the daemon's settings directory.
pub struct OwnerStore {
    path: PathBuf,
}

impl OwnerStore {
    #[must_use]
    pub fn in_settings_dir(settings_dir: &Path) -> Self {
        Self {
            path: settings_dir.join(OWNER_FILENAME),
        }
    }

    /// The recorded owner, `None` when there is no record.
    ///
    /// # Errors
    /// [`OwnerStoreError::Read`] when the record exists but cannot be read,
    /// [`OwnerStoreError::Parse`] when it is not a record at all,
    /// [`OwnerStoreError::UnknownVersion`] when a newer daemon wrote it.
    pub fn load(&self) -> Result<Option<Principal>, OwnerStoreError> {
        let raw = match std::fs::read(&self.path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(OwnerStoreError::Read(error)),
        };
        let record: OwnerRecord = serde_json::from_slice(&raw).map_err(OwnerStoreError::Parse)?;
        if record.version != OWNER_RECORD_VERSION {
            return Err(OwnerStoreError::UnknownVersion);
        }
        Ok(Some(record.owner.into()))
    }

    /// Record `owner`, replacing any previous record atomically.
    ///
    /// # Errors
    /// [`OwnerStoreError::Write`] when the record cannot be written.
    pub fn save(&self, owner: &Principal) -> Result<(), OwnerStoreError> {
        let record = OwnerRecord {
            version: OWNER_RECORD_VERSION,
            owner: owner.into(),
        };
        let json = serde_json::to_vec(&record).expect("an owner record always serializes");
        let temp = self.path.with_extension("json.tmp");
        write_private(&temp, &json)
            .and_then(|()| std::fs::rename(&temp, &self.path))
            .map_err(|error| {
                let _ = std::fs::remove_file(&temp);
                OwnerStoreError::Write(error)
            })
    }

    /// Remove the record. Removing an absent record succeeds.
    ///
    /// # Errors
    /// [`OwnerStoreError::Write`] when the record exists and cannot be removed.
    pub fn clear(&self) -> Result<(), OwnerStoreError> {
        match std::fs::remove_file(&self.path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                Err(OwnerStoreError::Write(error))
            }
            _ => Ok(()),
        }
    }
}

/// Create `path` readable and writable by its owner only (root), and write
/// `contents` to disk. On Windows the settings directory's ACL already limits
/// it to SYSTEM and Administrators.
fn write_private(path: &Path, contents: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(path)?;
    file.write_all(contents)?;
    file.sync_all()
}

/// Whether a peer is the account at the computer's own screen. A system
/// boundary, so it is a trait: tests answer it without a console.
pub trait ConsoleProbe: Send + Sync {
    fn is_console_user(&self, peer: &PeerCredentials) -> bool;
}

/// The operating system's own answer.
pub struct SystemConsole;

impl ConsoleProbe for SystemConsole {
    fn is_console_user(&self, peer: &PeerCredentials) -> bool {
        console::is_console_user(peer)
    }
}

#[cfg(target_os = "macos")]
mod console {
    use super::{PeerCredentials, Principal};

    /// macOS gives `/dev/console` to the user logged in at the login window,
    /// and back to root when nobody is.
    pub(super) fn is_console_user(peer: &PeerCredentials) -> bool {
        use std::os::unix::fs::MetadataExt;
        let Principal::Uid(uid) = peer.principal else {
            return false;
        };
        std::fs::metadata("/dev/console").is_ok_and(|console| uid != 0 && console.uid() == uid)
    }
}

#[cfg(target_os = "linux")]
mod console {
    use super::{PeerCredentials, Principal};

    pub(super) fn is_console_user(peer: &PeerCredentials) -> bool {
        let Principal::Uid(uid) = peer.principal else {
            return false;
        };
        super::logind::uid_is_active_on_a_seat(std::path::Path::new(SEATS_DIR), uid)
    }

    /// Where logind keeps one record per seat, which is what `loginctl` and
    /// `sd-login` read.
    const SEATS_DIR: &str = "/run/systemd/seats";
}

#[cfg(windows)]
mod console {
    use super::PeerCredentials;
    use windows_sys::Win32::System::RemoteDesktop::WTSGetActiveConsoleSessionId;

    /// No session is attached to the console while it switches users.
    const NO_CONSOLE_SESSION: u32 = u32::MAX;

    pub(super) fn is_console_user(peer: &PeerCredentials) -> bool {
        // SAFETY: takes no arguments and only reads kernel state.
        let console = unsafe { WTSGetActiveConsoleSessionId() };
        console != NO_CONSOLE_SESSION && peer.session_id == Some(console)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
mod console {
    use super::PeerCredentials;

    pub(super) fn is_console_user(_peer: &PeerCredentials) -> bool {
        false
    }
}

/// logind's seat records.
#[cfg(any(target_os = "linux", test))]
mod logind {
    use std::path::Path;

    /// Whether `uid` is the active user of a seat: the account whose session
    /// has the screen and keyboard. A session without a seat (ssh, including
    /// `ssh localhost`, a remote desktop, a cron job) is active by logind's
    /// definition and never the user of a seat. No logind at all answers
    /// `false`.
    pub(super) fn uid_is_active_on_a_seat(seats_dir: &Path, uid: u32) -> bool {
        let Ok(entries) = std::fs::read_dir(seats_dir) else {
            return false;
        };
        entries.flatten().any(|entry| {
            // Only plain files are seat records: never open anything that
            // could block, like a FIFO.
            entry.file_type().is_ok_and(|kind| kind.is_file())
                && std::fs::read_to_string(entry.path())
                    .is_ok_and(|record| active_uid(&record) == Some(uid))
        })
    }

    fn active_uid(record: &str) -> Option<u32> {
        record
            .lines()
            .find_map(|line| line.strip_prefix("ACTIVE_UID="))?
            .parse()
            .ok()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn seat(active_uid: Option<u32>) -> String {
            let active = active_uid
                .map(|uid| format!("ACTIVE=2\nACTIVE_UID={uid}\n"))
                .unwrap_or_default();
            format!(
                "# This is private data. Do not parse.\nIS_SEAT0=1\nCAN_MULTI_SESSION=1\n\
                 CAN_TTY=1\nCAN_GRAPHICAL=1\n{active}SESSIONS=2 5\nUIDS=1000 1001\n"
            )
        }

        #[test]
        fn the_active_user_of_a_seat_is_read_from_its_record() {
            assert_eq!(active_uid(&seat(Some(1000))), Some(1000));
            assert_eq!(
                active_uid(&seat(None)),
                None,
                "a seat nobody is logged in at"
            );
            assert_eq!(active_uid("ACTIVE_UID=root\n"), None);
        }

        /// Being logged in, even on the seat, is not having it: 1001 has a
        /// session on seat0 (it is in UIDS) while 1000 holds the screen.
        #[test]
        fn only_the_account_holding_a_seat_is_at_the_screen() {
            let dir = std::env::temp_dir().join(format!("wlogind-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("seat0"), seat(Some(1000))).unwrap();
            std::fs::write(dir.join("seat1"), seat(None)).unwrap();

            assert!(uid_is_active_on_a_seat(&dir, 1000));
            assert!(!uid_is_active_on_a_seat(&dir, 1001));
            assert!(!uid_is_active_on_a_seat(&dir.join("absent"), 1000));
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

/// The owner as the daemon last read or wrote it.
enum OwnerState {
    None,
    Recorded(Principal),
    /// The record exists and cannot be read: the wallet is left to
    /// administrators until one of them removes the record.
    Unreadable,
}

/// The owner and the install in progress, as the daemon holds them at run time.
pub struct WalletAccessControl {
    store: OwnerStore,
    owner: Mutex<OwnerState>,
    wallet_installed: Box<dyn Fn() -> bool + Send + Sync>,
    /// A mnemonic was in storage at boot, or the storage could not tell. It
    /// may not have loaded, and it still is somebody's wallet.
    stored_at_boot: AtomicBool,
    installing: AtomicBool,
    install_lock: tokio::sync::Mutex<()>,
    console: Box<dyn ConsoleProbe>,
}

/// What an admitted call did to the ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    Allowed,
    /// This call made its caller the owner: things withheld from it until now
    /// may be shown to it.
    Claimed,
}

/// Why a wallet install could not start.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InstallError {
    #[error(transparent)]
    Refused(#[from] Refusal),
    #[error("Failed to record the wallet owner")]
    Record(#[source] OwnerStoreError),
}

impl WalletAccessControl {
    /// Load the recorded owner from `store`. `wallet_installed` answers, at
    /// any time, whether the daemon holds a wallet; `stored_at_boot` says
    /// whether the storage held one, or could not tell, when the daemon
    /// started.
    pub fn new(
        store: OwnerStore,
        wallet_installed: impl Fn() -> bool + Send + Sync + 'static,
        stored_at_boot: bool,
        console: impl ConsoleProbe + 'static,
    ) -> Self {
        let owner = match store.load() {
            Ok(Some(owner)) => OwnerState::Recorded(owner),
            Ok(None) => OwnerState::None,
            Err(error) => {
                log::error!("{error}; only administrators may use Warren until it is removed");
                OwnerState::Unreadable
            }
        };
        Self {
            store,
            owner: Mutex::new(owner),
            wallet_installed: Box::new(wallet_installed),
            stored_at_boot: AtomicBool::new(stored_at_boot),
            installing: AtomicBool::new(false),
            install_lock: tokio::sync::Mutex::new(()),
            console: Box::new(console),
        }
    }

    fn lock_owner(&self) -> std::sync::MutexGuard<'_, OwnerState> {
        // Every write to the state is a whole-value assignment, so a panic
        // elsewhere cannot leave it half-written.
        self.owner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn wallet_present(&self) -> bool {
        // An install in progress counts as installed: its mnemonic may already
        // be on disk before the daemon reports it.
        self.installing.load(Ordering::SeqCst)
            || self.stored_at_boot.load(Ordering::SeqCst)
            || (self.wallet_installed)()
    }

    fn ownership<'a>(&self, owner: &'a OwnerState) -> Ownership<'a> {
        match owner {
            OwnerState::Recorded(owner) => Ownership::Owned(owner),
            OwnerState::Unreadable => Ownership::Unknown,
            OwnerState::None if self.wallet_present() => Ownership::Unclaimed,
            OwnerState::None => Ownership::Vacant,
        }
    }

    /// Decide a call of `class` from `peer`, under the caller's lock on the
    /// ownership. A decision that makes `peer` the owner records it, except
    /// for a wallet install, whose claim [`Self::begin_install`] makes.
    fn admit_locked(
        &self,
        owner: &mut OwnerState,
        peer: Option<&PeerCredentials>,
        class: RpcClass,
    ) -> Result<Admission, Refusal> {
        let decision = decide(peer, class, self.ownership(owner), |peer| {
            self.console.is_console_user(peer)
        });
        match (decision, peer) {
            (Decision::Allow, _) => Ok(Admission::Allowed),
            (Decision::AllowAsNewOwner, _) if class == RpcClass::InstallWallet => {
                Ok(Admission::Allowed)
            }
            (Decision::AllowAsNewOwner, Some(peer)) => match self.store.save(&peer.principal) {
                Ok(()) => {
                    log::info!("The wallet has an owner now: the console account claimed it");
                    *owner = OwnerState::Recorded(peer.principal.clone());
                    Ok(Admission::Claimed)
                }
                Err(error) => {
                    // Serve the call anyway: the console account is entitled
                    // to it, and its next call claims again.
                    log::error!("{error}");
                    Ok(Admission::Allowed)
                }
            },
            (Decision::AllowAsNewOwner, None) => Err(Refusal::NoCredentials),
            (Decision::Deny(refusal), _) => Err(refusal),
        }
    }

    /// Admit one call of `class` from `peer`.
    ///
    /// # Errors
    /// The [`Refusal`] to send back, when the call is not allowed.
    pub fn admit(
        &self,
        peer: Option<&PeerCredentials>,
        class: RpcClass,
    ) -> Result<Admission, Refusal> {
        if class == RpcClass::ReadPublic {
            return Ok(Admission::Allowed);
        }
        self.admit_locked(&mut self.lock_owner(), peer, class)
    }

    /// Admit one call of `class` from `peer` and run `act` before the
    /// ownership can change: `act` is where the call queues its daemon
    /// command.
    ///
    /// # Errors
    /// The [`Refusal`] to send back, in which case `act` never runs.
    pub fn admit_then<R>(
        &self,
        peer: Option<&PeerCredentials>,
        class: RpcClass,
        act: impl FnOnce() -> R,
    ) -> Result<(Admission, R), Refusal> {
        let mut owner = self.lock_owner();
        let admission = self.admit_locked(&mut owner, peer, class)?;
        Ok((admission, act()))
    }

    /// Whether `peer` may make a call of `class` now, without claiming
    /// anything: for a stream that has to stop once its caller no longer may.
    #[must_use]
    pub fn may_call(&self, peer: Option<&PeerCredentials>, class: RpcClass) -> bool {
        let owner = self.lock_owner();
        !matches!(
            decide(peer, class, self.ownership(&owner), |peer| self
                .console
                .is_console_user(peer)),
            Decision::Deny(_)
        )
    }

    /// Start installing a wallet for `peer`: create, import, login.
    ///
    /// Holds the install lock until the returned guard is gone, and records
    /// the new owner before the install runs, so no other account can act on
    /// the wallet between its mnemonic reaching the disk and its owner being
    /// known.
    ///
    /// # Errors
    /// [`InstallError::Refused`] when `peer` may not install a wallet now,
    /// [`InstallError::Record`] when its ownership cannot be recorded, in which
    /// case nothing is installed.
    pub async fn begin_install(
        &self,
        peer: Option<&PeerCredentials>,
    ) -> Result<InstallGuard<'_>, InstallError> {
        let lock = self.install_lock.lock().await;
        let mut owner = self.lock_owner();
        let decision = decide(
            peer,
            RpcClass::InstallWallet,
            self.ownership(&owner),
            |peer| self.console.is_console_user(peer),
        );
        let claimed = match (decision, peer) {
            (Decision::Allow, _) => false,
            (Decision::AllowAsNewOwner, Some(peer)) => {
                self.store
                    .save(&peer.principal)
                    .map_err(InstallError::Record)?;
                *owner = OwnerState::Recorded(peer.principal.clone());
                true
            }
            (Decision::AllowAsNewOwner, None) => return Err(Refusal::NoCredentials.into()),
            (Decision::Deny(refusal), _) => return Err(refusal.into()),
        };
        self.installing.store(true, Ordering::SeqCst);
        Ok(InstallGuard {
            access: self,
            claimed,
            _lock: lock,
        })
    }

    /// Release the ownership once the wallet has been erased.
    pub async fn release(&self) {
        let _lock = self.install_lock.lock().await;
        let mut owner = self.lock_owner();
        if let Err(error) = self.store.clear() {
            log::error!("{error}");
        }
        self.stored_at_boot.store(false, Ordering::SeqCst);
        *owner = OwnerState::None;
    }

    /// Whether what `peer` reads may carry identity material and secrets.
    #[must_use]
    pub fn may_see_identity(&self, peer: Option<&PeerCredentials>) -> bool {
        let owner = self.lock_owner();
        may_see_identity(peer, self.ownership(&owner))
    }
}

/// A wallet install in progress, from [`WalletAccessControl::begin_install`].
///
/// Dropped without [`InstallGuard::finish`], for instance because the client
/// went away while the daemon was installing, it keeps the ownership it
/// claimed: the install may still complete, and a wallet must never be left
/// without its owner by a hang-up.
#[must_use]
pub struct InstallGuard<'a> {
    access: &'a WalletAccessControl,
    claimed: bool,
    _lock: tokio::sync::MutexGuard<'a, ()>,
}

impl InstallGuard<'_> {
    /// Whether this install made its caller the owner.
    #[must_use]
    pub fn claimed(&self) -> bool {
        self.claimed
    }

    /// End the install. An install that reported failure gives back the
    /// ownership it claimed, unless a wallet is there anyway: an import that
    /// stored its mnemonic and then failed to log in has still installed it.
    pub fn finish(self, reported_success: bool) {
        if self.claimed && !reported_success && !(self.access.wallet_installed)() {
            let mut owner = self.access.lock_owner();
            if let Err(error) = self.access.store.clear() {
                log::error!("{error}");
            }
            *owner = OwnerState::None;
        }
    }
}

impl Drop for InstallGuard<'_> {
    fn drop(&mut self) {
        self.access.installing.store(false, Ordering::SeqCst);
    }
}

/// Fixtures shared with the gate's tests.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    /// A private settings directory, removed on drop.
    pub(crate) struct Scratch(pub(crate) PathBuf);

    impl Scratch {
        pub(crate) fn new(tag: &str) -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos();
            let dir =
                std::env::temp_dir().join(format!("wacl-{tag}-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }

        pub(crate) fn store(&self) -> OwnerStore {
            OwnerStore::in_settings_dir(&self.0)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A machine where nobody is at the console.
    pub(crate) struct NoConsole;

    impl ConsoleProbe for NoConsole {
        fn is_console_user(&self, _: &PeerCredentials) -> bool {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicBool};

    use super::test_support::Scratch;
    use super::*;

    const OWNER: u32 = 1000;
    const OTHER: u32 = 1001;

    fn user(uid: u32) -> PeerCredentials {
        PeerCredentials {
            principal: Principal::Uid(uid),
            privileged: false,
            session_id: None,
        }
    }

    fn admin() -> PeerCredentials {
        PeerCredentials {
            principal: Principal::Sid("S-1-5-18".to_owned()),
            privileged: true,
            session_id: Some(0),
        }
    }

    const CLASSES: [RpcClass; 4] = [
        RpcClass::ReadPublic,
        RpcClass::ControlMachine,
        RpcClass::Identity,
        RpcClass::InstallWallet,
    ];

    /// A caller, its peer, the ownership, whether it is at the console, and
    /// the decision for each class in [`CLASSES`] order.
    type Row<'a> = (
        &'static str,
        Option<PeerCredentials>,
        Ownership<'a>,
        bool,
        [Decision; 4],
    );

    /// The whole policy, one row per (caller, ownership, console) situation
    /// and one column per class: nothing is left to a default.
    #[test]
    fn the_policy_table() {
        use Decision::{Allow as A, AllowAsNewOwner as C, Deny};
        const OTHER_OWNS: Decision = Deny(Refusal::OwnedByAnotherAccount);
        const NOT_CONSOLE: Decision = Deny(Refusal::ClaimNeedsConsoleUser);
        const SET_UP: Decision = Deny(Refusal::SetUpFirst);
        const UNREADABLE: Decision = Deny(Refusal::OwnerRecordUnreadable);
        const NO_ID: Decision = Deny(Refusal::NoCredentials);

        let owner = Principal::Uid(OWNER);
        let owned = Ownership::Owned(&owner);
        #[rustfmt::skip]
        let rows: [Row<'_>; 15] = [
            // caller,                    peer,              ownership,            console, [read, control, identity, install]
            ("owner",                     Some(user(OWNER)), owned,                false, [A, A, A, A]),
            ("another account",           Some(user(OTHER)), owned,                true,  [A, OTHER_OWNS, OTHER_OWNS, OTHER_OWNS]),
            ("administrator, owned",      Some(admin()),     owned,                false, [A, A, A, A]),
            ("unidentified, owned",       None,              owned,                true,  [A, NO_ID, NO_ID, NO_ID]),
            ("elsewhere, no wallet",      Some(user(OTHER)), Ownership::Vacant,    false, [A, SET_UP, A, C]),
            ("console user, no wallet",   Some(user(OTHER)), Ownership::Vacant,    true,  [A, A, A, C]),
            ("administrator, no wallet",  Some(admin()),     Ownership::Vacant,    true,  [A, A, A, A]),
            ("unidentified, no wallet",   None,              Ownership::Vacant,    true,  [A, NO_ID, NO_ID, NO_ID]),
            ("console user, unclaimed",   Some(user(OTHER)), Ownership::Unclaimed, true,  [A, C, C, C]),
            ("elsewhere, unclaimed",      Some(user(OTHER)), Ownership::Unclaimed, false, [A, NOT_CONSOLE, NOT_CONSOLE, NOT_CONSOLE]),
            ("administrator, unclaimed",  Some(admin()),     Ownership::Unclaimed, false, [A, A, A, A]),
            ("unidentified, unclaimed",   None,              Ownership::Unclaimed, true,  [A, NO_ID, NO_ID, NO_ID]),
            ("console user, unreadable",  Some(user(OWNER)), Ownership::Unknown,   true,  [A, UNREADABLE, UNREADABLE, UNREADABLE]),
            ("administrator, unreadable", Some(admin()),     Ownership::Unknown,   false, [A, A, A, A]),
            ("unidentified, unreadable",  None,              Ownership::Unknown,   true,  [A, NO_ID, NO_ID, NO_ID]),
        ];

        for (caller, peer, ownership, console, expected) in rows {
            for (class, expected) in CLASSES.into_iter().zip(expected) {
                assert_eq!(
                    decide(peer.as_ref(), class, ownership, |_| console),
                    expected,
                    "{caller}, {class:?}"
                );
            }
        }
    }

    #[test]
    fn only_the_owner_and_administrators_see_identity_material() {
        let owner = Principal::Uid(OWNER);
        let owned = Ownership::Owned(&owner);

        assert!(may_see_identity(Some(&user(OWNER)), owned));
        assert!(may_see_identity(Some(&admin()), owned));
        assert!(may_see_identity(Some(&admin()), Ownership::Unclaimed));
        assert!(!may_see_identity(Some(&user(OTHER)), owned));
        assert!(!may_see_identity(None, owned));
        assert!(!may_see_identity(Some(&user(OTHER)), Ownership::Unclaimed));
        assert!(!may_see_identity(Some(&user(OTHER)), Ownership::Vacant));
        assert!(!may_see_identity(Some(&user(OWNER)), Ownership::Unknown));
    }

    /// The reason codes are the contract clients choose their words from.
    #[test]
    fn every_refusal_has_its_own_code() {
        let codes = [
            Refusal::NoCredentials,
            Refusal::OwnedByAnotherAccount,
            Refusal::ClaimNeedsConsoleUser,
            Refusal::SetUpFirst,
            Refusal::OwnerRecordUnreadable,
        ]
        .map(Refusal::code);
        assert_eq!(
            codes,
            [
                "no_credentials",
                "owned_by_another_account",
                "claim_needs_console_user",
                "set_up_first",
                "owner_record_unreadable",
            ]
        );
    }

    struct FixedConsole(Option<u32>);

    impl ConsoleProbe for FixedConsole {
        fn is_console_user(&self, peer: &PeerCredentials) -> bool {
            self.0
                .is_some_and(|uid| peer.principal == Principal::Uid(uid))
        }
    }

    /// A wallet presence the test can flip, standing in for the identity
    /// manager.
    fn wallet(present: bool) -> (Arc<AtomicBool>, impl Fn() -> bool + Send + Sync + 'static) {
        let flag = Arc::new(AtomicBool::new(present));
        let reader = flag.clone();
        (flag, move || reader.load(Ordering::SeqCst))
    }

    fn control(scratch: &Scratch, present: bool, console: Option<u32>) -> WalletAccessControl {
        let (_, installed) = wallet(present);
        WalletAccessControl::new(scratch.store(), installed, false, FixedConsole(console))
    }

    #[test]
    fn the_owner_record_round_trips_both_kinds_of_account() {
        let scratch = Scratch::new("rt");
        let store = scratch.store();
        assert_eq!(store.load().unwrap(), None);

        store.save(&Principal::Uid(OWNER)).unwrap();
        assert_eq!(store.load().unwrap(), Some(Principal::Uid(OWNER)));

        let sid = Principal::Sid("S-1-5-21-1-2-3-1001".to_owned());
        store.save(&sid).unwrap();
        assert_eq!(store.load().unwrap(), Some(sid));

        store.clear().unwrap();
        assert_eq!(store.load().unwrap(), None);
        store.clear().expect("clearing an absent record succeeds");
    }

    #[cfg(unix)]
    #[test]
    fn the_owner_record_is_readable_by_root_only() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new("mode");
        scratch.store().save(&Principal::Uid(OWNER)).unwrap();

        let mode = std::fs::metadata(scratch.0.join(OWNER_FILENAME))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn a_record_that_cannot_be_read_is_reported() {
        let scratch = Scratch::new("bad");
        let path = scratch.0.join(OWNER_FILENAME);

        std::fs::write(&path, b"{not json").unwrap();
        assert!(matches!(
            scratch.store().load(),
            Err(OwnerStoreError::Parse(_))
        ));

        std::fs::write(&path, br#"{"version":2,"owner":{"uid":1000}}"#).unwrap();
        assert!(matches!(
            scratch.store().load(),
            Err(OwnerStoreError::UnknownVersion)
        ));

        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(matches!(
            scratch.store().load(),
            Err(OwnerStoreError::Read(_))
        ));
    }

    /// A record nobody can read is not an absent record: handing the wallet
    /// to whoever sits at the console would give it away.
    #[test]
    fn an_unreadable_record_leaves_the_wallet_to_administrators() {
        let scratch = Scratch::new("unread");
        std::fs::write(scratch.0.join(OWNER_FILENAME), b"{not json").unwrap();
        let access = control(&scratch, true, Some(OWNER));

        assert_eq!(
            access.admit(Some(&user(OWNER)), RpcClass::Identity),
            Err(Refusal::OwnerRecordUnreadable)
        );
        assert_eq!(
            access.admit(Some(&admin()), RpcClass::Identity),
            Ok(Admission::Allowed)
        );
    }

    #[test]
    fn a_record_that_cannot_be_written_fails_the_install_before_it_runs() {
        let scratch = Scratch::new("ro");
        let access = WalletAccessControl::new(
            OwnerStore::in_settings_dir(&scratch.0.join("missing-dir")),
            || false,
            false,
            FixedConsole(None),
        );

        let result = futures::executor::block_on(access.begin_install(Some(&user(OWNER))));

        assert!(matches!(
            result,
            Err(InstallError::Record(OwnerStoreError::Write(_)))
        ));
        assert!(!access.may_see_identity(Some(&user(OWNER))));
    }

    #[tokio::test]
    async fn an_install_the_policy_refuses_never_starts() {
        let scratch = Scratch::new("refused");
        scratch.store().save(&Principal::Uid(OWNER)).unwrap();
        let access = control(&scratch, true, None);

        let result = access.begin_install(Some(&user(OTHER))).await;

        assert!(matches!(
            result,
            Err(InstallError::Refused(Refusal::OwnedByAnotherAccount))
        ));
    }

    /// The install that claims ownership survives a daemon restart: the
    /// restarted daemon refuses everyone else from its first call.
    #[tokio::test]
    async fn the_owner_survives_a_restart() {
        let scratch = Scratch::new("restart");
        {
            let access = control(&scratch, false, None);
            let install = access.begin_install(Some(&user(OWNER))).await.unwrap();
            assert!(install.claimed());
            install.finish(true);
        }

        let restarted = control(&scratch, true, Some(OTHER));

        assert_eq!(
            restarted.admit(Some(&user(OTHER)), RpcClass::ControlMachine),
            Err(Refusal::OwnedByAnotherAccount),
            "not even the console account may take an owned wallet"
        );
        assert_eq!(
            restarted.admit(Some(&user(OWNER)), RpcClass::Identity),
            Ok(Admission::Allowed)
        );
    }

    /// Between the claim and the end of the install, the wallet is already
    /// the installer's: nobody else can read the mnemonic the install is
    /// writing.
    #[tokio::test]
    async fn nobody_else_acts_while_an_install_is_running() {
        let scratch = Scratch::new("race");
        let access = control(&scratch, false, None);

        let install = access.begin_install(Some(&user(OWNER))).await.unwrap();

        assert_eq!(
            access.admit(Some(&user(OTHER)), RpcClass::Identity),
            Err(Refusal::OwnedByAnotherAccount)
        );
        install.finish(true);
    }

    /// A call the gate let through while there was no wallet is decided again
    /// when it acts: an install that claimed in between wins.
    #[tokio::test]
    async fn a_call_admitted_before_an_install_is_refused_when_it_acts() {
        let scratch = Scratch::new("dispatch");
        let access = control(&scratch, false, None);
        assert_eq!(
            access.admit(Some(&user(OTHER)), RpcClass::Identity),
            Ok(Admission::Allowed),
            "admitted at the gate, with no wallet yet"
        );

        let install = access.begin_install(Some(&user(OWNER))).await.unwrap();
        let mut acted = false;
        let dispatched = access.admit_then(Some(&user(OTHER)), RpcClass::Identity, || {
            acted = true;
        });

        assert_eq!(dispatched, Err(Refusal::OwnedByAnotherAccount));
        assert!(!acted, "the refused call never reaches the daemon");
        install.finish(true);
    }

    #[tokio::test]
    async fn a_failed_install_leaves_no_owner_behind() {
        let scratch = Scratch::new("fail");
        let access = control(&scratch, false, Some(OTHER));

        let install = access.begin_install(Some(&user(OWNER))).await.unwrap();
        install.finish(false);

        assert_eq!(scratch.store().load().unwrap(), None);
        assert_eq!(
            access.admit(Some(&user(OTHER)), RpcClass::ControlMachine),
            Ok(Admission::Allowed)
        );
    }

    /// An import that stored its mnemonic and then failed to log in has still
    /// installed a wallet, and it stays its installer's.
    #[tokio::test]
    async fn an_install_that_failed_after_storing_the_wallet_keeps_its_owner() {
        let scratch = Scratch::new("half");
        let (present, installed) = wallet(false);
        let access =
            WalletAccessControl::new(scratch.store(), installed, false, FixedConsole(None));

        let install = access.begin_install(Some(&user(OWNER))).await.unwrap();
        present.store(true, Ordering::SeqCst);
        install.finish(false);

        assert_eq!(scratch.store().load().unwrap(), Some(Principal::Uid(OWNER)));
    }

    /// The client may hang up while the daemon is still installing: the
    /// ownership stays with the installer rather than leaving a fresh wallet
    /// without an owner.
    #[tokio::test]
    async fn an_install_abandoned_by_its_client_keeps_its_owner() {
        let scratch = Scratch::new("hangup");
        let access = control(&scratch, false, None);

        drop(access.begin_install(Some(&user(OWNER))).await.unwrap());

        assert_eq!(
            access.admit(Some(&user(OTHER)), RpcClass::Identity),
            Err(Refusal::OwnedByAnotherAccount)
        );
    }

    /// An administrator's install leaves the wallet unowned, and an install
    /// in progress already counts as installed, so its mnemonic is never
    /// served to an account that is not at the console.
    #[tokio::test]
    async fn an_administrator_install_claims_nothing_and_is_guarded_while_it_runs() {
        let scratch = Scratch::new("admin");
        let access = control(&scratch, false, Some(OWNER));

        let install = access.begin_install(Some(&admin())).await.unwrap();
        assert!(!install.claimed());
        assert_eq!(
            access.admit(Some(&user(OTHER)), RpcClass::Identity),
            Err(Refusal::ClaimNeedsConsoleUser)
        );
        install.finish(true);

        assert_eq!(scratch.store().load().unwrap(), None);
    }

    /// A mnemonic found in storage at boot is a wallet even when it did not
    /// load, so it is claimable by the console account only.
    #[test]
    fn a_wallet_stored_at_boot_counts_as_installed() {
        let scratch = Scratch::new("boot");
        let access = WalletAccessControl::new(scratch.store(), || false, true, FixedConsole(None));

        assert_eq!(
            access.admit(Some(&user(OTHER)), RpcClass::Identity),
            Err(Refusal::ClaimNeedsConsoleUser)
        );
    }

    /// An existing wallet with no recorded owner goes to the console account,
    /// on its first control or wallet call, and to nobody else.
    #[test]
    fn an_unowned_wallet_is_claimed_by_the_console_account_only() {
        let scratch = Scratch::new("migrate");
        let access = control(&scratch, true, Some(OWNER));

        assert_eq!(
            access.admit(Some(&user(OTHER)), RpcClass::Identity),
            Err(Refusal::ClaimNeedsConsoleUser)
        );
        assert_eq!(
            access.admit(Some(&user(OWNER)), RpcClass::ReadPublic),
            Ok(Admission::Allowed),
            "reading state never claims"
        );
        assert_eq!(scratch.store().load().unwrap(), None);

        assert_eq!(
            access.admit(Some(&user(OWNER)), RpcClass::ControlMachine),
            Ok(Admission::Claimed)
        );
        assert_eq!(scratch.store().load().unwrap(), Some(Principal::Uid(OWNER)));
        assert_eq!(
            access.admit(Some(&user(OWNER)), RpcClass::Identity),
            Ok(Admission::Allowed)
        );
    }

    /// The console account is entitled to its call even when its claim cannot
    /// be written; it claims again on its next call.
    #[test]
    fn a_claim_that_cannot_be_written_still_serves_the_console_account() {
        let scratch = Scratch::new("noclaim");
        let access = WalletAccessControl::new(
            OwnerStore::in_settings_dir(&scratch.0.join("missing-dir")),
            || true,
            false,
            FixedConsole(Some(OWNER)),
        );

        assert_eq!(
            access.admit(Some(&user(OWNER)), RpcClass::ControlMachine),
            Ok(Admission::Allowed)
        );
        assert!(
            !access.may_see_identity(Some(&user(OWNER))),
            "nothing was claimed"
        );
    }

    /// A true sign-out erases the wallet, and with it the ownership and the
    /// memory of a wallet found at boot.
    #[tokio::test]
    async fn signing_out_releases_the_wallet() {
        let scratch = Scratch::new("release");
        scratch.store().save(&Principal::Uid(OWNER)).unwrap();
        let access = WalletAccessControl::new(scratch.store(), || false, true, FixedConsole(None));

        access.release().await;

        assert_eq!(scratch.store().load().unwrap(), None);
        let install = access.begin_install(Some(&user(OTHER))).await.unwrap();
        assert!(
            install.claimed(),
            "a released wallet is anyone's to set up again"
        );
        install.finish(true);
    }

    #[test]
    fn identity_is_shown_to_the_owner_once_known() {
        let scratch = Scratch::new("see");
        let access = control(&scratch, true, Some(OWNER));
        assert!(!access.may_see_identity(Some(&user(OWNER))));

        access
            .admit(Some(&user(OWNER)), RpcClass::Identity)
            .unwrap();

        assert!(access.may_see_identity(Some(&user(OWNER))));
        assert!(!access.may_see_identity(Some(&user(OTHER))));
    }

    /// A stream checks, without claiming, whether its caller still may.
    #[test]
    fn may_call_follows_the_ownership_without_claiming() {
        let scratch = Scratch::new("maycall");
        let access = control(&scratch, true, Some(OWNER));

        assert!(access.may_call(Some(&user(OWNER)), RpcClass::Identity));
        assert!(!access.may_call(Some(&user(OTHER)), RpcClass::Identity));
        assert_eq!(
            scratch.store().load().unwrap(),
            None,
            "asking claimed nothing"
        );
    }
}
