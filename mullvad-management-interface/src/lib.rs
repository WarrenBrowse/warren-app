pub mod client;
pub mod types;

#[cfg(all(unix, not(target_os = "android")))]
use std::{env, fs, os::unix::fs::PermissionsExt};
use std::{
    future::Future,
    io,
    path::PathBuf,
    pin::Pin,
    task::{Context, Poll},
};
use tipsy::Endpoint as IpcEndpoint;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
#[cfg(not(target_os = "android"))]
use tonic::transport::{Endpoint, Uri};
use tonic::transport::{Server, server::Connected};
#[cfg(not(target_os = "android"))]
use tower::service_fn;

pub use tonic::{Code, Request, Response, Status, async_trait, transport::Channel};

pub type ManagementServiceClient =
    types::management_service_client::ManagementServiceClient<Channel>;
pub use types::management_service_server::{ManagementService, ManagementServiceServer};

pub use types::{RelaySelectorService, RelaySelectorServiceClient, RelaySelectorServiceServer};

pub const API_ACCESS_METHOD_EXISTS_DETAILS: &[u8] = b"api_access_method_exists";
pub const CUSTOM_LIST_LIST_NOT_FOUND_DETAILS: &[u8] = b"custom_list_list_not_found";
pub const CUSTOM_LIST_LIST_EXISTS_DETAILS: &[u8] = b"custom_list_list_exists";
pub const CUSTOM_LIST_LIST_NAME_TOO_LONG_DETAILS: &[u8] = b"custom_list_list_name_too_long";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Management RPC server or client error")]
    GrpcTransportError(#[source] tonic::transport::Error),

    #[error("Failed to start IPC pipe/socket")]
    StartServerError(#[source] io::Error),

    #[error("Failed to initialize pipe/socket security attributes")]
    SecurityAttributes(#[source] io::Error),

    #[error("Unable to set permissions for IPC endpoint")]
    PermissionsError(#[source] io::Error),

    #[cfg(all(unix, not(target_os = "android")))]
    #[error("Group not found")]
    NoGidError,

    #[cfg(all(unix, not(target_os = "android")))]
    #[error("Failed to obtain group ID")]
    ObtainGidError(#[source] nix::Error),

    #[cfg(all(unix, not(target_os = "android")))]
    #[error("Failed to set group ID")]
    SetGidError(#[source] nix::Error),

    // TODO: Remove box when upgrading tonic to a version with
    // https://github.com/hyperium/tonic/pull/2282
    #[error("gRPC call returned error")]
    Rpc(#[source] Box<tonic::Status>),

    #[error("Failed to parse gRPC response")]
    InvalidResponse(#[source] types::FromProtobufTypeError),

    #[error("Duration is too large")]
    DurationTooLarge,

    #[error("Unexpected non-UTF8 string")]
    PathMustBeUtf8,

    #[error("Missing daemon event")]
    MissingDaemonEvent,

    #[error("This voucher code is invalid")]
    InvalidVoucher,

    #[error("This voucher code has already been used")]
    UsedVoucher,

    #[error("There are too many devices on the account. One must be revoked to log in")]
    TooManyDevices,

    #[error("You are already logged in. Log out to create a new account")]
    AlreadyLoggedIn,

    #[error("The account does not exist")]
    InvalidAccount,

    #[error("There is no such device")]
    DeviceNotFound,

    #[error("Location data is unavailable")]
    NoLocationData,

    #[error("A custom list with that name already exists")]
    CustomListExists,

    #[error("A custom list with that name does not exist")]
    CustomListListNotFound,

    #[error("Location already exists in the custom list")]
    LocationExistsInCustomList,

    #[error("Location was not found in the custom list")]
    LocationNotFoundInCustomlist,

    #[error("Could not retrieve API access methods from settings")]
    ApiAccessMethodSettingsNotFound,

    #[error("An access method with that id does not exist")]
    ApiAccessMethodNotFound,

    #[error("An access method with that name already exists")]
    ApiAccessMethodExists,

    #[error("Failed to parse IP Address")]
    IpAddr(#[from] std::net::AddrParseError),
}

impl From<tonic::Status> for Error {
    fn from(value: tonic::Status) -> Self {
        Error::Rpc(Box::new(value))
    }
}

#[cfg(not(target_os = "android"))]
#[deprecated(note = "Prefer MullvadProxyClient")]
pub async fn new_management_service_client() -> Result<ManagementServiceClient, Error> {
    grpc_transport_channel()
        .await
        .map(ManagementServiceClient::new)
}

/// Create a [Channel] for communication between any of the available gRPC clients (e.g.
/// [ManagementServiceClient]) and this environment's management interface gRPC service.
#[cfg(not(target_os = "android"))]
pub(crate) async fn grpc_transport_channel() -> Result<Channel, Error> {
    grpc_transport_channel_at(mullvad_paths::get_rpc_socket_path()).await
}

/// Create a [Channel] to this environment's own management interface at
/// `ipc_path`.
///
/// Crate-private on purpose: the only paths outside this crate that make sense
/// belong to ANOTHER product environment, and those go through
/// [`grpc_transport_channel_to`], which cannot be handed an unvouched one.
#[cfg(not(target_os = "android"))]
pub(crate) async fn grpc_transport_channel_at(ipc_path: PathBuf) -> Result<Channel, Error> {
    use futures::TryFutureExt;

    // The URI will be ignored
    Endpoint::from_static("lttp://[::]:50051")
        .connect_with_connector(service_fn(move |_: Uri| {
            IpcEndpoint::connect(ipc_path.clone()).map_ok(hyper_util::rt::tokio::TokioIo::new)
        }))
        .await
        .map_err(Error::GrpcTransportError)
}

/// Create a [Channel] to another product environment's management interface.
///
/// Takes a [`PrivilegedSocketPath`] rather than a path, so the ownership check
/// cannot be forgotten: see that type for why it is load-bearing.
#[cfg(all(unix, not(target_os = "android")))]
pub async fn grpc_transport_channel_to(path: &PrivilegedSocketPath) -> Result<Channel, Error> {
    // The unix ownership question is settled by the path itself (see
    // [`PrivilegedSocketPath`]), so the ordinary connector is enough here.
    grpc_transport_channel_at(path.as_path().to_path_buf()).await
}

/// Create a [Channel] to another product environment's management interface.
///
/// Deliberately NOT [`grpc_transport_channel_at`]: on Windows the ownership
/// question has to be asked of the pipe instance this channel carries its
/// bytes over, and it is asked again for every connection the channel opens,
/// reconnects included. See [`PrivilegedSocketPath`] for why a check made on
/// a handle that is then dropped gates nothing here.
#[cfg(windows)]
pub async fn grpc_transport_channel_to(path: &PrivilegedSocketPath) -> Result<Channel, Error> {
    let ipc_path = path.as_path().to_path_buf();
    Endpoint::from_static("lttp://[::]:50051")
        .connect_with_connector(service_fn(move |_: Uri| {
            let ipc_path = ipc_path.clone();
            async move {
                connect_admin_owned_pipe(&ipc_path)
                    .await
                    .map(hyper_util::rt::tokio::TokioIo::new)
            }
        }))
        .await
        .map_err(Error::GrpcTransportError)
}

/// Open the named pipe at `path` and hand it back only if the OS says a
/// privileged account owns THAT instance.
///
/// The verification and the connection are one operation on purpose: two
/// separate opens of the same pipe name can land on two different instances,
/// and an unprivileged process is allowed to own one of them (see
/// [`PrivilegedSocketPath`]).
#[cfg(windows)]
async fn connect_admin_owned_pipe(
    path: &std::path::Path,
) -> io::Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    use std::os::windows::io::AsHandle;
    use std::time::{Duration, Instant};

    /// `ERROR_PIPE_BUSY`, spelled out rather than imported so this crate does
    /// not grow a `windows-sys` edge for one integer.
    const ERROR_PIPE_BUSY: i32 = 231;
    /// How long to wait for a free instance, matching what `tipsy` does for
    /// this crate's other dials: the daemon creates the next free instance
    /// only after accepting, so a dial landing in that window is busy rather
    /// than refused.
    const PIPE_AVAILABILITY_TIMEOUT: Duration = Duration::from_secs(5);

    let deadline = Instant::now() + PIPE_AVAILABILITY_TIMEOUT;
    let pipe = loop {
        match tokio::net::windows::named_pipe::ClientOptions::new()
            .read(true)
            .write(true)
            .open(path)
        {
            Ok(pipe) => break pipe,
            Err(error)
                if error.raw_os_error() == Some(ERROR_PIPE_BUSY) && Instant::now() < deadline =>
            {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(error) => return Err(error),
        }
    };

    if endpoint_admits(talpid_windows::fs::is_admin_owned(pipe.as_handle()).ok()) {
        Ok(pipe)
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "the management pipe instance is not owned by a privileged account",
        ))
    }
}

/// The admission decision every cross-environment dial turns on, given what
/// the OS said about the endpoint: `Some(true)` a privileged owner (uid 0 on
/// unix, SYSTEM or the built-in administrators on Windows), `Some(false)`
/// anybody else, `None` nothing observable at all.
///
/// One function for both platforms so the ACCEPTING direction is drivable by
/// a test that owns no root-owned endpoint: an implementation that stopped
/// admitting a genuine one would leave every refusal gate green while
/// silently deciding that no environment is ever asserting the machine, and
/// the whole stand-down would quietly do nothing.
///
/// `None` is refused like an unprivileged owner. An environment that is not
/// installed has no endpoint, and a probe that cannot see one must read as
/// "nothing to yield to" rather than as evidence.
#[cfg(not(target_os = "android"))]
#[must_use]
fn endpoint_admits(privileged_owner: Option<bool>) -> bool {
    privileged_owner == Some(true)
}

/// What the OS says about the owner of the management endpoint at `path`, or
/// `None` when there is nothing there to ask about.
#[cfg(all(unix, not(target_os = "android")))]
#[must_use]
fn unix_endpoint_owner_is_privileged(path: &std::path::Path) -> Option<bool> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};

    // symlink_metadata, not metadata: a symlink an unprivileged user planted
    // at the derived path must never inherit the target's ownership.
    let meta = std::fs::symlink_metadata(path).ok()?;
    // A path that is not a socket is not an endpoint, whoever owns it.
    meta.file_type().is_socket().then(|| meta.uid() == 0)
}

/// Whether the OS vouches that the management endpoint at `path` belongs to a
/// privileged process: owned by uid 0.
///
/// This is the admission gate for every cross-environment dial, and it is the
/// same check the desktop GUI runs on its own daemon in `verifyOwnership()`.
/// Without it a foreign environment's state is worthless as evidence: the
/// management socket is world-accessible and `DisconnectTunnel`,
/// `SetLockdownMode` and `SetAutoConnect` are unauthenticated, so an
/// unprivileged process that binds prod's path could hold a kill switch open
/// forever and any environment that believed it would stand down on command.
#[cfg(all(unix, not(target_os = "android")))]
#[must_use]
fn foreign_socket_is_privileged(path: &std::path::Path) -> bool {
    endpoint_admits(unix_endpoint_owner_is_privileged(path))
}

/// Windows arm of the gate above: open the named pipe and ask the security
/// descriptor who owns it, mirroring `pipeIsAdminOwned` in the desktop main
/// process.
///
/// An early rejection, not the gate. The pipe name can carry another
/// process's instance (see [`PrivilegedSocketPath`]), so the answer that
/// binds is the one [`connect_admin_owned_pipe`] gets about the instance the
/// channel actually holds.
#[cfg(windows)]
#[must_use]
fn foreign_socket_is_privileged(path: &std::path::Path) -> bool {
    let Ok(pipe) = std::fs::File::options().read(true).open(path) else {
        return false;
    };
    endpoint_admits(talpid_windows::fs::is_admin_owned(pipe).ok())
}

/// A management endpoint of ANOTHER product environment that a
/// cross-environment dial is allowed to attempt.
///
/// The rule exists as a type rather than as a line in a doc comment because
/// it is load-bearing and the wrong call would otherwise compile: the
/// management socket is world-accessible and its disconnect, lockdown and
/// auto-connect RPCs are unauthenticated, so believing an unvouched endpoint
/// is a documented way for any local process to talk an environment into
/// standing down.
///
/// WHERE the ownership question is settled differs by platform, and the
/// difference is not cosmetic:
///
/// - unix: the PATH is the evidence. The socket lives in a root-owned
///   directory an unprivileged user cannot create an entry in, so a
///   root-owned socket at the derived path can only be that environment's
///   daemon, and [`Self::vouched_for`] settles it once at construction.
/// - Windows: the path carries no evidence at all. The daemon's pipe is
///   created with `allow_everyone_create`, whose Everyone ACE grants
///   `FILE_GENERIC_WRITE` and therefore `FILE_CREATE_PIPE_INSTANCE`, so any
///   local process may keep its own instance of `\\.\pipe\Warren VPN`
///   listening and serve a connection. Asking a handle and then dropping it
///   settles nothing, because the next connect can land on a different
///   instance. So [`Self::vouched_for`] is only a cheap early rejection
///   there, and the binding check is made on the pipe instance the channel
///   actually holds, inside [`grpc_transport_channel_to`].
#[cfg(not(target_os = "android"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivilegedSocketPath(PathBuf);

#[cfg(not(target_os = "android"))]
impl PrivilegedSocketPath {
    /// The only constructor, so there is no spelling of a foreign dial that
    /// skips the gate: `Some` when the OS says the endpoint at `path` belongs
    /// to a privileged process, `None` for everything else, an absent path
    /// included.
    ///
    /// On unix this settles the question. On Windows it is an early
    /// rejection and the binding check is made again, on the pipe instance
    /// the dial lands on, in [`grpc_transport_channel_to`]. See the type.
    #[must_use]
    pub fn vouched_for(path: PathBuf) -> Option<Self> {
        foreign_socket_is_privileged(&path).then_some(Self(path))
    }

    #[must_use]
    pub fn as_path(&self) -> &std::path::Path {
        &self.0
    }
}

#[cfg(not(target_os = "android"))]
pub use client::MullvadProxyClient;

pub type ServerJoinHandle = tokio::task::JoinHandle<()>;

/// How the freshly bound management socket will be exposed, decided from
/// explicit operator configuration only (see `plan_socket_access`).
#[cfg(all(unix, not(target_os = "android")))]
#[derive(Debug)]
enum SocketAccessPlan {
    RestrictToGroup(nix::unistd::Gid),
    WorldAccessible,
}

/// Peer credentials of a connected management client, captured from the
/// Unix domain socket via `SO_PEERCRED`. Used to authorize wallet/secret
/// RPCs against the calling process. A `None` connect-info means the
/// platform could not supply credentials (Windows named pipe), where
/// access is gated by the pipe DACL + admin-ownership check instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerCredentials {
    pub uid: u32,
    pub gid: u32,
    pub pid: Option<i32>,
}

/// Connect-info attached by tonic to every request's extensions. Handlers
/// read it via `request.extensions().get::<ManagementConnectInfo>()`.
pub type ManagementConnectInfo = Option<PeerCredentials>;

/// Outcome of applying access control to the management socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketSecurity {
    /// Socket is restricted to root + a Unix group; the kernel enforces
    /// that only authorized users can connect at all.
    GroupRestricted,
    /// Socket is reachable by any local user (no group configured, or a
    /// platform without Unix socket permissions). Wallet RPCs must be
    /// gated per-uid by the caller.
    WorldAccessible,
}

/// Largest gzipped log a signed forum request carries: the broker's own cap on
/// the base64 field translated back to bytes. Spelled here rather than
/// imported, because this crate is the IPC layer and does not depend on the
/// forum crate; `mullvad-daemon`'s `forum_rpc_size` test pins it equal to
/// `warren_forum::MAX_LOG_GZ_BYTES`.
const MAX_FORUM_LOG_GZ_BYTES: usize = 12_000_000;

/// Headroom over that log for the rest of the request: the report fields, the
/// sid, the topic id and the protobuf framing.
pub const MAX_RPC_MESSAGE_OVERHEAD_BYTES: usize = 1024 * 1024;

/// Largest request the management service decodes. tonic's default is 4 MiB,
/// under the gzipped log a forum report or attach-logs request carries
/// (`SignForumReport`, `SignForumAttachLogs`), so an at-cap report was refused
/// here before the daemon ever saw it.
///
/// Derived from that log rather than rounded up to a power of two: this is the
/// buffer EVERY method of the service gets, tonic fills it while decoding and
/// therefore before a handler can run `authorize_wallet_access`, and the
/// socket is world-accessible by default on Linux and macOS. So a round number
/// picked by hand is a round amount of memory any local process can make the
/// daemon hold, and the daemon owns the kill switch.
pub const MAX_RPC_MESSAGE_BYTES: usize = MAX_FORUM_LOG_GZ_BYTES + MAX_RPC_MESSAGE_OVERHEAD_BYTES;

pub fn spawn_rpc_server(
    management_service: impl ManagementService,
    relay_selector_service: impl RelaySelectorService,
    abort_rx: impl Future<Output = ()> + Send + 'static,
    rpc_socket_path: PathBuf,
) -> std::result::Result<(ServerJoinHandle, SocketSecurity), Error> {
    let (incoming, security) = build_incoming(rpc_socket_path)?;

    let grpc_server = Server::builder()
        .add_service(
            ManagementServiceServer::new(management_service)
                .max_decoding_message_size(MAX_RPC_MESSAGE_BYTES),
        )
        .add_service(RelaySelectorServiceServer::new(relay_selector_service))
        .serve_with_incoming_shutdown(incoming, abort_rx);

    let server_task = tokio::spawn(async move {
        if let Err(execution_error) = grpc_server.await.map_err(Error::GrpcTransportError) {
            log::error!("Management server panic: {execution_error}");
        }
        log::trace!("gRPC server is shutting down");
    });

    Ok((server_task, security))
}

/// Build the stream of incoming connections, capturing `SO_PEERCRED` per
/// connection on Unix so wallet RPCs can authorize the calling process.
#[cfg(all(unix, not(target_os = "android")))]
fn build_incoming(
    rpc_socket_path: PathBuf,
) -> Result<
    (
        impl futures::Stream<Item = io::Result<StreamBox<tokio::net::UnixStream>>>,
        SocketSecurity,
    ),
    Error,
> {
    use std::os::unix::fs::FileTypeExt;
    use std::os::unix::net::UnixListener as StdUnixListener;

    // The daemon removes a stale socket before spawning us, but guard
    // against a leftover socket file from an unclean exit so bind()
    // succeeds. Only remove an actual socket, never a regular file.
    if let Ok(meta) = std::fs::symlink_metadata(&rpc_socket_path)
        && meta.file_type().is_socket()
    {
        let _ = std::fs::remove_file(&rpc_socket_path);
    }

    let std_listener = StdUnixListener::bind(&rpc_socket_path).map_err(Error::StartServerError)?;
    std_listener
        .set_nonblocking(true)
        .map_err(Error::StartServerError)?;
    let security = apply_socket_permissions(&rpc_socket_path)?;
    let listener =
        tokio::net::UnixListener::from_std(std_listener).map_err(Error::StartServerError)?;

    let incoming = futures::stream::unfold(listener, |listener| async move {
        let item = match listener.accept().await {
            Ok((stream, _addr)) => {
                let creds = stream.peer_cred().ok().map(|c| PeerCredentials {
                    uid: c.uid(),
                    gid: c.gid(),
                    pid: c.pid(),
                });
                Ok(StreamBox {
                    inner: stream,
                    creds,
                })
            }
            Err(e) => Err(e),
        };
        Some((item, listener))
    });

    Ok((incoming, security))
}

/// Windows (named pipe) and Android keep the original `tipsy` transport.
/// Peer credentials are not captured here; the named-pipe DACL plus the
/// desktop's admin-ownership check are the access boundary on Windows.
#[cfg(any(windows, target_os = "android"))]
fn build_incoming(
    rpc_socket_path: PathBuf,
) -> Result<
    (
        impl futures::Stream<Item = io::Result<StreamBox<tipsy::Connection>>>,
        SocketSecurity,
    ),
    Error,
> {
    use futures::TryStreamExt;

    let endpoint = create_endpoint(rpc_socket_path)?;
    let incoming = endpoint
        .incoming()
        .map_err(Error::StartServerError)?
        .map_ok(|conn| StreamBox {
            inner: conn,
            creds: None,
        });
    Ok((incoming, SocketSecurity::WorldAccessible))
}

/// Decide how to expose the management socket. Only an explicitly
/// configured group (env override) restricts it; the default is
/// world-accessible with wallet/secret RPCs gated per-uid, matching
/// upstream Mullvad's threat model (local users are trusted) and the
/// wider industry practice. A group is deliberately never auto-detected:
/// group membership only takes effect at the next login, so flipping on a
/// pre-existing group would lock the freshly-installed GUI out of the
/// socket until the user logs out and back in.
#[cfg(all(unix, not(target_os = "android")))]
fn plan_socket_access(
    configured_group: Option<&str>,
    resolve_group: impl FnOnce(&str) -> Result<Option<nix::unistd::Gid>, Error>,
) -> Result<SocketAccessPlan, Error> {
    match configured_group {
        None => Ok(SocketAccessPlan::WorldAccessible),
        Some(name) => match resolve_group(name)? {
            Some(gid) => Ok(SocketAccessPlan::RestrictToGroup(gid)),
            None => {
                // Operator asked for a specific group that does not exist: fail
                // closed rather than silently exposing the socket to all users.
                log::error!(
                    "Configured management socket group '{name}' does not exist; refusing to expose the management socket"
                );
                Err(Error::NoGidError)
            }
        },
    }
}

/// Apply access control to the freshly-bound Unix socket per
/// `plan_socket_access`.
#[cfg(all(unix, not(target_os = "android")))]
fn apply_socket_permissions(path: &std::path::Path) -> Result<SocketSecurity, Error> {
    let env_group = env::var("WARREN_MANAGEMENT_SOCKET_GROUP")
        .or_else(|_| env::var("MULLVAD_MANAGEMENT_SOCKET_GROUP"))
        .ok();

    let plan = plan_socket_access(env_group.as_deref(), |name| {
        Ok(nix::unistd::Group::from_name(name)
            .map_err(Error::ObtainGidError)?
            .map(|group| group.gid))
    })?;

    match plan {
        SocketAccessPlan::RestrictToGroup(gid) => {
            nix::unistd::chown(path, None, Some(gid)).map_err(Error::SetGidError)?;
            fs::set_permissions(path, PermissionsExt::from_mode(0o760))
                .map_err(Error::PermissionsError)?;
            Ok(SocketSecurity::GroupRestricted)
        }
        SocketAccessPlan::WorldAccessible => {
            fs::set_permissions(path, PermissionsExt::from_mode(0o766))
                .map_err(Error::PermissionsError)?;
            log::info!(
                "Management socket at {} is reachable by all local users; wallet/secret RPCs are \
                 gated to the owning uid. Set WARREN_MANAGEMENT_SOCKET_GROUP to restrict the \
                 socket to a dedicated Unix group instead.",
                path.display()
            );
            Ok(SocketSecurity::WorldAccessible)
        }
    }
}

#[cfg(any(windows, target_os = "android"))]
fn create_endpoint(rpc_socket_path: PathBuf) -> Result<IpcEndpoint, Error> {
    let endpoint = IpcEndpoint::new(rpc_socket_path, tipsy::OnConflict::Error)
        .map_err(Error::StartServerError)?;
    let endpoint = endpoint.security_attributes(
        tipsy::SecurityAttributes::allow_everyone_create()
            .map_err(Error::SecurityAttributes)?
            .mode(0o766)
            .map_err(Error::SecurityAttributes)?,
    );
    Ok(endpoint)
}

#[derive(Debug)]
struct StreamBox<T: AsyncRead + AsyncWrite> {
    inner: T,
    creds: Option<PeerCredentials>,
}
impl<T: AsyncRead + AsyncWrite> Connected for StreamBox<T> {
    type ConnectInfo = ManagementConnectInfo;

    fn connect_info(&self) -> Self::ConnectInfo {
        self.creds
    }
}
impl<T: AsyncRead + AsyncWrite + Unpin> AsyncRead for StreamBox<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}
impl<T: AsyncRead + AsyncWrite + Unpin> AsyncWrite for StreamBox<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[cfg(all(test, not(target_os = "android")))]
mod foreign_socket_tests {
    use super::PrivilegedSocketPath;

    /// The gate as its only public spelling: a path is admitted exactly when a
    /// `PrivilegedSocketPath` can be built from it.
    fn vouched(path: &std::path::Path) -> bool {
        PrivilegedSocketPath::vouched_for(path.to_path_buf()).is_some()
    }

    /// A private directory under the system temp dir, removed on drop.
    ///
    /// The name is kept short on purpose: a unix socket path must fit in
    /// `sun_path` (104 bytes on macOS), and the system temp dir already eats
    /// most of that on a Mac.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos();
            let dir =
                std::env::temp_dir().join(format!("wfs-{tag}-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("create scratch dir");
            Scratch(dir)
        }

        fn join(&self, name: &str) -> std::path::PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_path_that_does_not_exist_is_not_privileged() {
        // The probe must answer "no" rather than error out: an environment
        // that is not installed simply has no socket, and the caller reads a
        // false here as "nothing to yield to".
        let scratch = Scratch::new("a");
        assert!(!vouched(&scratch.join("no-such-socket")));
    }

    #[test]
    fn a_regular_file_is_not_privileged() {
        // Only a live management endpoint counts. A regular file at the
        // derived path is leftover garbage or bait, never a daemon.
        let scratch = Scratch::new("r");
        let path = scratch.join("not-a-socket");
        std::fs::write(&path, b"not a socket").expect("write file");
        assert!(!vouched(&path));
    }

    #[cfg(unix)]
    #[test]
    fn a_socket_owned_by_an_unprivileged_user_is_not_privileged() {
        // The whole point of the gate. The management socket is
        // world-accessible and its RPCs are unauthenticated, so any local
        // process can bind a lookalike path and answer "connected" forever.
        // Believing it would disarm this build's kill switch on demand.
        let scratch = Scratch::new("u");
        let path = scratch.join("s");
        let _listener = std::os::unix::net::UnixListener::bind(&path).expect("bind test socket");
        assert!(
            path.exists(),
            "the test socket must exist for the assertion below to mean anything"
        );
        // Phrased against the running uid so a root shell exercises the
        // accepting branch instead of producing a spurious red. Every gated
        // runner (CI and dev) is unprivileged, so this is the rejecting case.
        let owner_is_root = nix::unistd::geteuid().is_root();
        assert_eq!(vouched(&path), owner_is_root);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_does_not_borrow_the_ownership_of_what_it_points_at() {
        // `symlink_metadata`, not `metadata`. Following the link would let an
        // unprivileged user plant a link at the derived path, aim it at
        // something root owns, and borrow its vouch.
        let scratch = Scratch::new("l");
        let link = scratch.join("s");
        std::os::unix::fs::symlink(std::path::Path::new("/dev/null"), &link)
            .expect("plant a symlink");
        assert!(!vouched(&link));
    }

    #[cfg(unix)]
    #[test]
    fn a_vouched_path_carries_the_path_it_was_admitted_with() {
        // The newtype is the only way to name a foreign endpoint, so it has to
        // hand back exactly what was checked. A constructor that normalised or
        // re-derived the path would dial something the gate never saw.
        if !nix::unistd::geteuid().is_root() {
            return;
        }
        let scratch = Scratch::new("v");
        let path = scratch.join("s");
        let _listener = std::os::unix::net::UnixListener::bind(&path).expect("bind test socket");
        let vouched = PrivilegedSocketPath::vouched_for(path.clone()).expect("root-owned socket");
        assert_eq!(vouched.as_path(), path);
    }
}

#[cfg(all(test, unix, not(target_os = "android")))]
mod socket_access_tests {
    use super::*;

    #[test]
    fn no_configured_group_is_world_accessible_without_consulting_groups() {
        // A stray `warren` group on the system must never flip the socket to
        // group-restricted (locking the GUI out until the next login), so the
        // group database must not even be consulted without explicit config.
        let plan = plan_socket_access(None, |_| -> Result<Option<nix::unistd::Gid>, Error> {
            panic!("group database consulted although no group was configured")
        })
        .unwrap();
        assert!(matches!(plan, SocketAccessPlan::WorldAccessible));
    }

    #[test]
    fn configured_group_restricts_to_that_gid() {
        let plan = plan_socket_access(Some("vpnadmins"), |_| {
            Ok(Some(nix::unistd::Gid::from_raw(4242)))
        })
        .unwrap();
        match plan {
            SocketAccessPlan::RestrictToGroup(gid) => assert_eq!(gid.as_raw(), 4242),
            SocketAccessPlan::WorldAccessible => panic!("explicit group was ignored"),
        }
    }

    #[test]
    fn configured_but_missing_group_fails_closed() {
        let result = plan_socket_access(Some("vpnadmins"), |_| Ok(None));
        assert!(matches!(result, Err(Error::NoGidError)));
    }
}

/// The decoding cap against the forum request contract. That one number is at
/// once a floor (the largest legal signed report has to fit, or the daemon
/// refuses an at-cap report before it ever sees it) and a ceiling (it is the
/// buffer every other method gets, filled while tonic decodes and therefore
/// before a handler can authorize the caller), so both edges are pinned.
#[cfg(test)]
mod rpc_size_tests {
    use super::{MAX_FORUM_LOG_GZ_BYTES, MAX_RPC_MESSAGE_BYTES, MAX_RPC_MESSAGE_OVERHEAD_BYTES};
    use crate::types::{ForumAttachLogsRequest, ForumReportRequest};
    use prost::Message;

    /// tonic 0.13's own `DEFAULT_MAX_RECV_MESSAGE_SIZE`, the value the cap had
    /// to be raised above.
    const TONIC_DEFAULT_DECODING_BYTES: usize = 4 * 1024 * 1024;

    /// The broker caps the description and the steps at 4,000 characters each;
    /// the rest of the report is a handful of short enumerated fields.
    fn largest_report_json() -> String {
        format!(
            r#"{{"area":"connectivity","frequency":"always","description":"{}","steps":"{}","platform":"linux","version":"9999.99.99"}}"#,
            "d".repeat(4_000),
            "s".repeat(4_000),
        )
    }

    #[test]
    fn the_largest_legal_signed_report_fits_the_cap_and_needs_it() {
        let request = ForumReportRequest {
            report_json: largest_report_json(),
            log_gz: vec![0u8; MAX_FORUM_LOG_GZ_BYTES],
        };
        let encoded = request.encoded_len();
        assert!(
            encoded <= MAX_RPC_MESSAGE_BYTES,
            "an at-cap report encodes to {encoded} bytes, over the {MAX_RPC_MESSAGE_BYTES} byte cap"
        );
        assert!(
            encoded > TONIC_DEFAULT_DECODING_BYTES,
            "the cap no longer needs raising over tonic's default, so drop the override"
        );
    }

    #[test]
    fn the_largest_legal_attach_logs_request_fits_the_cap() {
        let request = ForumAttachLogsRequest {
            sid: "a".repeat(32),
            topic_id: u64::MAX,
            log_gz: vec![0u8; MAX_FORUM_LOG_GZ_BYTES],
        };
        let encoded = request.encoded_len();
        assert!(
            encoded <= MAX_RPC_MESSAGE_BYTES,
            "an at-cap attach-logs request encodes to {encoded} bytes, over the {MAX_RPC_MESSAGE_BYTES} byte cap"
        );
    }

    #[test]
    fn the_cap_is_the_forum_contract_plus_the_declared_headroom() {
        // Widening it widens the buffer every method of a socket that is
        // world-accessible by default hands to any local process.
        assert_eq!(
            MAX_RPC_MESSAGE_BYTES,
            MAX_FORUM_LOG_GZ_BYTES + MAX_RPC_MESSAGE_OVERHEAD_BYTES
        );
    }
}

/// The admission gate every cross-environment dial turns on.
///
/// CI runs unprivileged and owns no root-owned endpoint, so the accepting
/// direction is driven through [`endpoint_admits`] rather than through a real
/// one. That direction needs a gate as much as the refusing one does: a build
/// that stopped admitting a genuine root-owned endpoint would keep every
/// refusal test green while quietly deciding that no other environment ever
/// asserts the machine, and the whole stand-down would do nothing at all.
#[cfg(all(test, not(target_os = "android")))]
mod foreign_endpoint_tests {
    use super::endpoint_admits;

    #[test]
    fn an_endpoint_a_privileged_account_owns_is_admitted() {
        assert!(
            endpoint_admits(Some(true)),
            "refusing a genuine root-owned endpoint disables the whole \
             cross-environment stand-down, silently and with every other gate green"
        );
    }

    #[test]
    fn an_endpoint_anybody_else_owns_is_refused() {
        // The socket is world-accessible and its disconnect, lockdown and
        // auto-connect RPCs are unauthenticated, so an unprivileged process
        // holding a look-alike endpoint could otherwise talk this build into
        // standing down on demand.
        assert!(!endpoint_admits(Some(false)));
    }

    #[test]
    fn an_endpoint_nothing_could_be_asked_about_is_refused() {
        // An environment that is not installed has no endpoint, and a probe
        // that cannot see one must read as "nothing to yield to", never as
        // evidence.
        assert!(!endpoint_admits(None));
    }
}
