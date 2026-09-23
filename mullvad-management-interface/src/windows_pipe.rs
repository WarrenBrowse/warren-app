//! The management pipe on Windows: who may open it, and who each client is.
//!
//! Every local account that is logged on may open the pipe, because the GUI
//! and the CLI run as ordinary users. What a connection may then do is decided
//! per RPC by the daemon, against the identity read here from the client's
//! token.

use std::{
    ffi::c_void,
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    path::{Path, PathBuf},
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures::Stream;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf},
    net::windows::named_pipe::{NamedPipeServer, ServerOptions},
    sync::mpsc,
};
use windows_sys::Win32::{
    Foundation::{HANDLE, LocalFree},
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            SDDL_REVISION_1,
        },
        CheckTokenMembership, CreateWellKnownSid, GetTokenInformation, IsWellKnownSid,
        PSECURITY_DESCRIPTOR, RevertToSelf, SECURITY_ATTRIBUTES, SECURITY_MAX_SID_SIZE,
        TOKEN_QUERY, TOKEN_USER, TokenSessionId, TokenUser, WinBuiltinAdministratorsSid,
        WinLocalSystemSid,
    },
    System::{
        Pipes::ImpersonateNamedPipeClient,
        Threading::{GetCurrentThread, OpenThreadToken},
    },
};

use crate::{Error, PeerCredentials, Principal, StreamBox};

/// Security descriptor of the management pipe, in SDDL.
///
/// - `D:P`: a protected DACL, so nothing is inherited into it.
/// - `(A;;GA;;;SY)(A;;GA;;;BA)`: SYSTEM and Administrators get full access,
///   which includes `FILE_CREATE_PIPE_INSTANCE`. They are the only accounts
///   that can serve an instance of this pipe name, so no other account can put
///   its own server behind the name and read what a client sends it.
/// - `(A;;0x12019b;;;AU)`: authenticated users may open the pipe as clients:
///   read and write data, attributes and extended attributes, read the
///   descriptor, synchronize. The mask leaves out `FILE_APPEND_DATA` (0x4),
///   which on a pipe is `FILE_CREATE_PIPE_INSTANCE`. A client open that asks
///   for `GENERIC_WRITE`, as Node's and tokio's clients do, still succeeds:
///   the named pipe file system drops that bit from a client's requested
///   access before the check.
///
/// Everyone, Anonymous and network logons get no entry. Remote clients are
/// refused at the pipe level as well (`PIPE_REJECT_REMOTE_CLIENTS`), so the
/// pipe is unreachable over SMB whatever the DACL says.
const PIPE_SDDL: &str = "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;0x12019b;;;AU)";

/// How long a client has, once connected, to send its first bytes. The
/// identity is read then, and a connection that never speaks holds a pipe
/// instance for nothing.
const FIRST_BYTES_TIMEOUT: Duration = Duration::from_secs(10);

/// Buffer for the first read. Any size works: the bytes are replayed in front
/// of the stream. An HTTP/2 client preface is 24 bytes.
const FIRST_READ_BYTES: usize = 64;

/// How long to wait before retrying to create the next pipe instance after
/// the OS refused one.
const INSTANCE_RETRY_DELAY: Duration = Duration::from_millis(500);

/// Connections that are identified and waiting for tonic to pick them up.
const ACCEPT_QUEUE: usize = 16;

/// The security descriptor built from [`PIPE_SDDL`], owned for as long as the
/// server creates instances with it.
struct PipeSecurity {
    descriptor: PSECURITY_DESCRIPTOR,
}

// SAFETY: the descriptor is a LocalAlloc'd block this value owns alone. It is
// never written after construction, and CreateNamedPipeW only reads it.
unsafe impl Send for PipeSecurity {}

impl PipeSecurity {
    fn new() -> io::Result<Self> {
        let sddl: Vec<u16> = PIPE_SDDL.encode_utf16().chain(Some(0)).collect();
        let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        // SAFETY: `sddl` is a NUL-terminated UTF-16 string that outlives the
        // call, and `descriptor` is a valid out-pointer. On success the OS
        // hands us a LocalAlloc'd descriptor, released in `Drop`.
        let converted = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &raw mut descriptor,
                std::ptr::null_mut(),
            )
        };
        if converted == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { descriptor })
    }

    fn create_instance(&self, path: &Path, first: bool) -> io::Result<NamedPipeServer> {
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.descriptor,
            bInheritHandle: 0,
        };
        // SAFETY: `attributes` is a valid SECURITY_ATTRIBUTES whose descriptor
        // `self` keeps alive across the call. CreateNamedPipeW copies what it
        // needs and keeps no reference to either.
        unsafe {
            ServerOptions::new()
                .first_pipe_instance(first)
                .reject_remote_clients(true)
                .access_inbound(true)
                .access_outbound(true)
                .in_buffer_size(65536)
                .out_buffer_size(65536)
                .create_with_security_attributes_raw(path, (&raw mut attributes).cast::<c_void>())
        }
    }
}

impl Drop for PipeSecurity {
    fn drop(&mut self) {
        // SAFETY: `descriptor` came from
        // ConvertStringSecurityDescriptorToSecurityDescriptorW, which documents
        // LocalFree as its release, and nothing else frees it.
        unsafe { LocalFree(self.descriptor) };
    }
}

/// The stream of identified connections to the management pipe at `path`.
///
/// Creating the first instance with `FILE_FLAG_FIRST_PIPE_INSTANCE` makes the
/// start fail if anything already serves that name, instead of serving next to
/// it.
pub(crate) fn incoming(
    path: PathBuf,
) -> Result<impl Stream<Item = io::Result<StreamBox<IdentifiedPipe>>>, Error> {
    let security = PipeSecurity::new().map_err(Error::SecurityAttributes)?;
    let first = security
        .create_instance(&path, true)
        .map_err(Error::StartServerError)?;
    let (tx, rx) = mpsc::channel(ACCEPT_QUEUE);
    tokio::spawn(accept_loop(path, security, first, tx));
    Ok(futures::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|connection| (Ok(connection), rx))
    }))
}

async fn accept_loop(
    path: PathBuf,
    security: PipeSecurity,
    mut listening: NamedPipeServer,
    tx: mpsc::Sender<StreamBox<IdentifiedPipe>>,
) {
    loop {
        let connected = tokio::select! {
            () = tx.closed() => return,
            connected = listening.connect() => connected,
        };
        let next = loop {
            match security.create_instance(&path, false) {
                Ok(next) => break next,
                Err(error) => {
                    log::error!("Failed to create the next management pipe instance: {error}");
                    tokio::select! {
                        () = tx.closed() => return,
                        () = tokio::time::sleep(INSTANCE_RETRY_DELAY) => {}
                    }
                }
            }
        };
        let pipe = std::mem::replace(&mut listening, next);
        match connected {
            Ok(()) => {
                let tx = tx.clone();
                tokio::spawn(async move {
                    if let Some(connection) = identify(pipe).await {
                        let _ = tx.send(connection).await;
                    }
                });
            }
            Err(error) => {
                log::debug!("A management pipe client left before it was accepted: {error}")
            }
        }
    }
}

/// Wait for the client's first bytes, then read its identity.
///
/// Windows only lets a pipe server impersonate its client once it has read
/// from the pipe (`ERROR_CANNOT_IMPERSONATE` before that), so the identity is
/// read after the first read and the bytes are replayed to tonic.
async fn identify(mut pipe: NamedPipeServer) -> Option<StreamBox<IdentifiedPipe>> {
    let mut first = vec![0u8; FIRST_READ_BYTES];
    let read = tokio::time::timeout(FIRST_BYTES_TIMEOUT, pipe.read(&mut first)).await;
    let read = match read {
        Ok(Ok(read)) if read > 0 => read,
        _ => return None,
    };
    first.truncate(read);
    let creds = client_credentials(&pipe);
    if creds.is_none() {
        log::warn!("Could not read the identity of a management pipe client");
    }
    Some(StreamBox {
        inner: IdentifiedPipe {
            pending: first,
            offset: 0,
            pipe,
        },
        creds,
    })
}

/// The identity of the client connected to `pipe`, from the token Windows
/// captured when that client opened the pipe.
///
/// Read through impersonation rather than by looking up the client's process
/// id: a process id can be recycled between the lookup and the open, and it
/// names a process rather than the security context of the open itself. A
/// client that opened the pipe at the Anonymous impersonation level has no
/// token to read, and gets `None`.
fn client_credentials(pipe: &NamedPipeServer) -> Option<PeerCredentials> {
    let token = impersonated_client_token(pipe)?;
    let (sid, is_local_system) = token_user(&token)?;
    Some(PeerCredentials {
        principal: Principal::Sid(sid),
        privileged: is_local_system || is_administrator(&token),
        session_id: token_session_id(&token),
    })
}

fn impersonated_client_token(pipe: &NamedPipeServer) -> Option<OwnedHandle> {
    // SAFETY: the handle is a connected server pipe handle, alive for the
    // whole call because `pipe` is borrowed.
    if unsafe { ImpersonateNamedPipeClient(pipe.as_raw_handle()) } == 0 {
        return None;
    }
    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: `token` is a valid out-pointer. `OpenAsSelf` makes the open use
    // the daemon's own context, which is required for a client that allowed
    // only identification.
    let opened = unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &raw mut token) };
    // SAFETY: always paired with the successful impersonation above.
    if unsafe { RevertToSelf() } == 0 {
        // The thread would go on running daemon code as the client.
        log::error!("Could not revert a management pipe impersonation; aborting");
        std::process::abort();
    }
    if opened == 0 {
        return None;
    }
    // SAFETY: OpenThreadToken succeeded, so `token` is an open handle that
    // nothing else owns.
    Some(unsafe { OwnedHandle::from_raw_handle(token) })
}

/// The token user's SID in string form, and whether it is LocalSystem.
fn token_user(token: &OwnedHandle) -> Option<(String, bool)> {
    let buffer = token_information(token, TokenUser)?;
    // SAFETY: GetTokenInformation(TokenUser) fills the buffer with a TOKEN_USER
    // followed by the SID it points into, and the buffer is u64-aligned.
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    let sid = user.User.Sid;
    // SAFETY: `sid` points into `buffer`, which outlives both calls.
    let is_local_system = unsafe { IsWellKnownSid(sid, WinLocalSystemSid) } != 0;
    let mut string_sid: *mut u16 = std::ptr::null_mut();
    // SAFETY: `sid` is valid as above and `string_sid` is a valid out-pointer.
    if unsafe { ConvertSidToStringSidW(sid, &raw mut string_sid) } == 0 {
        return None;
    }
    // SAFETY: on success the OS returns a NUL-terminated UTF-16 string.
    let length = (0..)
        .take_while(|&i| unsafe { *string_sid.add(i) } != 0)
        .count();
    // SAFETY: `length` UTF-16 units precede the terminator.
    let text = String::from_utf16(unsafe { std::slice::from_raw_parts(string_sid, length) });
    // SAFETY: ConvertSidToStringSidW documents LocalFree as its release.
    unsafe { LocalFree(string_sid.cast()) };
    text.ok().map(|sid| (sid, is_local_system))
}

fn token_session_id(token: &OwnedHandle) -> Option<u32> {
    let mut session_id = 0u32;
    let mut returned = 0u32;
    // SAFETY: the out-buffer is a u32, the size TokenSessionId writes.
    let ok = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenSessionId,
            (&raw mut session_id).cast(),
            size_of::<u32>() as u32,
            &raw mut returned,
        )
    };
    (ok != 0).then_some(session_id)
}

/// Whether the token carries the Administrators group as an enabled group.
/// A non-elevated administrator's token carries it as deny-only, and does
/// not count.
fn is_administrator(token: &OwnedHandle) -> bool {
    let mut sid = [0u32; SECURITY_MAX_SID_SIZE as usize / 4];
    let mut sid_size = SECURITY_MAX_SID_SIZE;
    // SAFETY: `sid` is a writable, suitably aligned buffer of `sid_size` bytes.
    let created = unsafe {
        CreateWellKnownSid(
            WinBuiltinAdministratorsSid,
            std::ptr::null_mut(),
            sid.as_mut_ptr().cast(),
            &raw mut sid_size,
        )
    };
    if created == 0 {
        return false;
    }
    let mut is_member = 0;
    // SAFETY: the token is an impersonation token opened with TOKEN_QUERY, and
    // `sid` holds the SID just created.
    let checked = unsafe {
        CheckTokenMembership(
            token.as_raw_handle(),
            sid.as_mut_ptr().cast(),
            &raw mut is_member,
        )
    };
    checked != 0 && is_member != 0
}

/// A token information class read into a u64-aligned buffer, which is what the
/// structures it holds need.
fn token_information(token: &OwnedHandle, class: i32) -> Option<Vec<u64>> {
    let mut needed = 0u32;
    // SAFETY: a size query: a null buffer of length 0, and a valid out-pointer.
    unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            class,
            std::ptr::null_mut(),
            0,
            &raw mut needed,
        )
    };
    if needed == 0 {
        return None;
    }
    let mut buffer = vec![0u64; (needed as usize).div_ceil(size_of::<u64>())];
    // SAFETY: `buffer` holds at least `needed` bytes.
    let ok = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            class,
            buffer.as_mut_ptr().cast(),
            needed,
            &raw mut needed,
        )
    };
    (ok != 0).then_some(buffer)
}

/// A server pipe that replays the bytes read while identifying the client
/// before reading on.
#[derive(Debug)]
pub(crate) struct IdentifiedPipe {
    pending: Vec<u8>,
    offset: usize,
    pipe: NamedPipeServer,
}

impl AsyncRead for IdentifiedPipe {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = &mut *self;
        if this.offset < this.pending.len() {
            let rest = &this.pending[this.offset..];
            let take = rest.len().min(buf.remaining());
            buf.put_slice(&rest[..take]);
            this.offset += take;
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut this.pipe).poll_read(cx, buf)
    }
}

impl AsyncWrite for IdentifiedPipe {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.pipe).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.pipe).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.pipe).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use std::pin::pin;

    use futures::StreamExt;
    use tokio::{io::AsyncWriteExt, net::windows::named_pipe::ClientOptions};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    use super::*;

    /// `SECURITY_ANONYMOUS` from `winbase.h`; tokio adds `SECURITY_SQOS_PRESENT`.
    const SECURITY_ANONYMOUS: u32 = 0;

    fn unique_pipe(tag: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        PathBuf::from(format!(
            r"\\.\pipe\warren-{tag}-{}-{unique}",
            std::process::id()
        ))
    }

    fn own_sid() -> String {
        let mut token: HANDLE = std::ptr::null_mut();
        // SAFETY: a valid out-pointer for this process' own token.
        let opened = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) };
        assert_ne!(opened, 0);
        // SAFETY: OpenProcessToken succeeded and nothing else owns the handle.
        let token = unsafe { OwnedHandle::from_raw_handle(token) };
        token_user(&token).expect("own token user").0
    }

    /// The identity is read from the client's own token, and the bytes read
    /// to get there reach tonic intact.
    #[tokio::test]
    async fn a_client_is_identified_by_its_token_and_its_first_bytes_are_replayed() {
        let path = unique_pipe("id");
        let mut incoming = pin!(incoming(path.clone()).unwrap());
        let mut client = ClientOptions::new().open(&path).unwrap();
        client.write_all(b"PRI * HTTP/2.0").await.unwrap();

        let mut connection = incoming.next().await.unwrap().unwrap();

        let creds = connection.creds.clone().expect("an identified client");
        assert_eq!(creds.principal, Principal::Sid(own_sid()));
        assert!(creds.session_id.is_some());
        let mut replayed = [0u8; 14];
        connection.inner.read_exact(&mut replayed).await.unwrap();
        assert_eq!(&replayed, b"PRI * HTTP/2.0");
    }

    /// A client that opened the pipe at the Anonymous level carries no token
    /// to read, and must come through as unidentified rather than as anybody.
    #[tokio::test]
    async fn an_anonymous_client_is_unidentified() {
        let path = unique_pipe("anon");
        let mut incoming = pin!(incoming(path.clone()).unwrap());
        let mut client = ClientOptions::new()
            .security_qos_flags(SECURITY_ANONYMOUS)
            .open(&path)
            .unwrap();
        client.write_all(b"x").await.unwrap();

        let connection = incoming.next().await.unwrap().unwrap();

        assert_eq!(connection.creds, None);
    }

    /// Nothing may serve the daemon's pipe name next to the daemon.
    #[tokio::test]
    async fn a_second_server_cannot_serve_a_name_already_served() {
        let path = unique_pipe("first");
        let _served = incoming(path.clone()).unwrap();

        assert!(incoming(path).is_err());
    }
}
