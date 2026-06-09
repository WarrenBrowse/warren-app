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
/// [ManagementServiceClient]) and the management interface gRPC service.
#[cfg(not(target_os = "android"))]
pub(crate) async fn grpc_transport_channel() -> Result<Channel, Error> {
    use futures::TryFutureExt;

    let ipc_path = mullvad_paths::get_rpc_socket_path();

    // The URI will be ignored
    Endpoint::from_static("lttp://[::]:50051")
        .connect_with_connector(service_fn(move |_: Uri| {
            IpcEndpoint::connect(ipc_path.clone()).map_ok(hyper_util::rt::tokio::TokioIo::new)
        }))
        .await
        .map_err(Error::GrpcTransportError)
}

#[cfg(not(target_os = "android"))]
pub use client::MullvadProxyClient;

pub type ServerJoinHandle = tokio::task::JoinHandle<()>;

/// Default Unix group granted access to the management socket when no
/// explicit override is configured. Members of this group (plus root)
/// can drive the daemon and read wallet secrets, so the installer
/// creates it and adds the desktop user. See `apply_socket_permissions`.
#[cfg(all(unix, not(target_os = "android")))]
const DEFAULT_SOCKET_GROUP: &str = "warren";

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

pub fn spawn_rpc_server(
    management_service: impl ManagementService,
    relay_selector_service: impl RelaySelectorService,
    abort_rx: impl Future<Output = ()> + Send + 'static,
    rpc_socket_path: PathBuf,
) -> std::result::Result<(ServerJoinHandle, SocketSecurity), Error> {
    let (incoming, security) = build_incoming(rpc_socket_path)?;

    let grpc_server = Server::builder()
        .add_service(ManagementServiceServer::new(management_service))
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
                Ok(StreamBox { inner: stream, creds })
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

/// Apply access control to the freshly-bound Unix socket. Restricts it to
/// root + a Unix group when one is available; fails closed if an operator
/// explicitly named a group that does not exist; otherwise falls back to
/// world-accessible with a loud warning (so the desktop GUI still works on
/// boxes where the group has not been provisioned).
#[cfg(all(unix, not(target_os = "android")))]
fn apply_socket_permissions(path: &std::path::Path) -> Result<SocketSecurity, Error> {
    let env_group = env::var("WARREN_MANAGEMENT_SOCKET_GROUP")
        .or_else(|_| env::var("MULLVAD_MANAGEMENT_SOCKET_GROUP"))
        .ok();
    let explicit = env_group.is_some();
    let group_name = env_group.as_deref().unwrap_or(DEFAULT_SOCKET_GROUP);

    match nix::unistd::Group::from_name(group_name).map_err(Error::ObtainGidError)? {
        Some(group) => {
            nix::unistd::chown(path, None, Some(group.gid)).map_err(Error::SetGidError)?;
            fs::set_permissions(path, PermissionsExt::from_mode(0o760))
                .map_err(Error::PermissionsError)?;
            Ok(SocketSecurity::GroupRestricted)
        }
        None if explicit => {
            // Operator asked for a specific group that does not exist: fail
            // closed rather than silently exposing the socket to all users.
            log::error!(
                "Configured management socket group '{group_name}' does not exist; refusing to expose the management socket"
            );
            Err(Error::NoGidError)
        }
        None => {
            fs::set_permissions(path, PermissionsExt::from_mode(0o766))
                .map_err(Error::PermissionsError)?;
            log::warn!(
                "Management socket group '{group_name}' not found: the socket at {} is reachable by every local user. \
                 Wallet/secret RPCs are restricted to the first local user that connects (trust-on-first-use). \
                 For multi-user safety, create the '{DEFAULT_SOCKET_GROUP}' group and add your desktop user to it.",
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
