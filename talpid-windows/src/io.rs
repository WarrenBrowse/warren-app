use std::{
    io,
    marker::PhantomData,
    mem::{self, ManuallyDrop},
    os::windows::io::{AsRawHandle, RawHandle},
    ptr,
};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};

use crate::sync::Event;

/// Abstraction over `OVERLAPPED`.
///
/// - <https://learn.microsoft.com/en-us/windows/win32/api/minwinbase/ns-minwinbase-overlapped>
/// - <https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-createeventw>
pub struct Overlapped {
    overlapped: OVERLAPPED,
    event: Option<Event>,
}

// SAFETY: Both OVERLAPPED and Event is used for async I/O, so this *should* be safe.
unsafe impl Send for Overlapped {}

impl Overlapped {
    /// Creates an `OVERLAPPED` object with `hEvent` set.
    pub fn new(event: Option<Event>) -> io::Result<Self> {
        let mut overlapped = Overlapped {
            // SAFETY: OVERLAPPED is a C struct and can safely be zeroed.
            overlapped: unsafe { mem::zeroed() },
            event: None,
        };
        overlapped.set_event(event);
        Ok(overlapped)
    }

    /// Borrows the underlying `OVERLAPPED` object.
    pub fn as_mut_ptr(&mut self) -> *mut OVERLAPPED {
        &raw mut self.overlapped
    }

    /// Returns a reference to the associated event.
    pub fn get_event(&self) -> Option<&Event> {
        self.event.as_ref()
    }

    /// Sets the event object for the underlying `OVERLAPPED` object (i.e., `hEvent`)
    fn set_event(&mut self, event: Option<Event>) {
        match event {
            Some(event) => {
                self.overlapped.hEvent = event.as_raw_handle();
                self.event = Some(event);
            }
            None => {
                self.overlapped.hEvent = ptr::null_mut();
                self.event = None;
            }
        }
    }
}

/// An overlapped operation that Windows may still be writing into.
///
/// Until the operation completes, the kernel owns the `OVERLAPPED` and the buffer it was started
/// with. Dropping this guard cancels the operation and blocks until the kernel has let go of both,
/// so neither can be freed or reused under a pending request on any return path.
#[must_use = "dropping the guard cancels the operation"]
pub struct PendingIo<'a> {
    handle: RawHandle,
    overlapped: &'a mut Overlapped,
    _buffer: PhantomData<&'a mut [u8]>,
}

impl<'a> PendingIo<'a> {
    /// Takes charge of an overlapped operation that has been started and not yet completed.
    ///
    /// # Panics
    ///
    /// Panics if `overlapped` has no event: completion is waited for on that event.
    ///
    /// # Safety
    ///
    /// The operation must have been started on `handle` with `overlapped`, writing into
    /// `buffer` or into no buffer at all, and the call that started it must have reported
    /// `ERROR_IO_PENDING`.
    pub unsafe fn new<H: AsRawHandle>(
        handle: &'a H,
        overlapped: &'a mut Overlapped,
        _buffer: &'a mut [u8],
    ) -> Self {
        assert!(
            overlapped.get_event().is_some(),
            "a pending operation needs an event to be waited on"
        );
        Self {
            handle: handle.as_raw_handle(),
            overlapped,
            _buffer: PhantomData,
        }
    }

    /// The event that is signaled when the operation completes.
    pub fn event(&self) -> &Event {
        self.overlapped
            .get_event()
            .expect("checked when the guard was created")
    }

    /// Waits for the operation to complete and returns the number of bytes it transferred.
    pub fn finish(self) -> io::Result<u32> {
        let mut this = ManuallyDrop::new(self);
        this.wait()
    }

    fn wait(&mut self) -> io::Result<u32> {
        let mut transferred = 0u32;
        // SAFETY: `handle` and `overlapped` are the ones the operation was started with (the
        // contract of `new`), the handle is borrowed for the guard's lifetime so it is still
        // open, and `bWait` is set, so the call returns only once the operation has completed.
        let result = unsafe {
            GetOverlappedResult(
                self.handle,
                self.overlapped.as_mut_ptr(),
                &raw mut transferred,
                1,
            )
        };
        if result == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(transferred)
    }
}

impl Drop for PendingIo<'_> {
    fn drop(&mut self) {
        // SAFETY: same handle and `OVERLAPPED` as the operation. Cancelling an operation that has
        // already completed fails with `ERROR_NOT_FOUND`, which is harmless here.
        unsafe { CancelIoEx(self.handle, self.overlapped.as_mut_ptr()) };
        // A cancelled operation completes with `ERROR_OPERATION_ABORTED`: that error is the
        // expected outcome, and the wait has returned either way.
        let _ = self.wait();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::{
        ffi::OsStr,
        iter,
        os::windows::{
            ffi::OsStrExt,
            io::{FromRawHandle, OwnedHandle},
        },
    };
    use windows_sys::Win32::{
        Foundation::{ERROR_IO_PENDING, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{
            CreateFileW, FILE_FLAG_OVERLAPPED, OPEN_EXISTING, PIPE_ACCESS_DUPLEX, ReadFile,
            WriteFile,
        },
        System::Pipes::{CreateNamedPipeW, PIPE_TYPE_BYTE},
    };

    /// `STATUS_PENDING`, which `OVERLAPPED.Internal` holds while the kernel owns the request.
    const STATUS_PENDING: usize = 0x103;

    struct Pipe {
        server: OwnedHandle,
        client: OwnedHandle,
    }

    fn pipe(name: &str) -> Pipe {
        let name: Vec<u16> = OsStr::new(&format!(
            r"\\.\pipe\talpid-windows-io-test-{name}-{}",
            std::process::id()
        ))
        .encode_wide()
        .chain(iter::once(0))
        .collect();
        // SAFETY: `name` is a NUL-terminated wide string that outlives the call, and the
        // security attributes pointer is null (default descriptor).
        let server = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED,
                PIPE_TYPE_BYTE,
                1,
                4096,
                4096,
                0,
                ptr::null(),
            )
        };
        assert_ne!(
            server,
            INVALID_HANDLE_VALUE,
            "{}",
            io::Error::last_os_error()
        );
        // SAFETY: same name, null security attributes and no template file.
        let client = unsafe {
            CreateFileW(
                name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                ptr::null(),
                OPEN_EXISTING,
                0,
                ptr::null_mut(),
            )
        };
        assert_ne!(
            client,
            INVALID_HANDLE_VALUE,
            "{}",
            io::Error::last_os_error()
        );
        // SAFETY: both handles were just created and nothing else owns them.
        unsafe {
            Pipe {
                server: OwnedHandle::from_raw_handle(server),
                client: OwnedHandle::from_raw_handle(client),
            }
        }
    }

    fn start_read(handle: HANDLE, overlapped: &mut Overlapped, buffer: &mut [u8]) {
        // SAFETY: the buffer pointer and length come from `buffer`, which the caller hands to a
        // `PendingIo` together with `overlapped` right after, so both outlive the request.
        let result = unsafe {
            ReadFile(
                handle,
                buffer.as_mut_ptr(),
                u32::try_from(buffer.len()).unwrap(),
                ptr::null_mut(),
                overlapped.as_mut_ptr(),
            )
        };
        assert_eq!(result, 0, "the read must not complete synchronously");
        assert_eq!(
            io::Error::last_os_error().raw_os_error(),
            Some(ERROR_IO_PENDING as i32)
        );
    }

    fn write(handle: &OwnedHandle, data: &[u8]) {
        let mut written = 0u32;
        // SAFETY: the data pointer and length come from the same slice, and the client end was
        // opened without FILE_FLAG_OVERLAPPED, so a null OVERLAPPED is a synchronous write.
        let ok = unsafe {
            WriteFile(
                handle.as_raw_handle(),
                data.as_ptr(),
                u32::try_from(data.len()).unwrap(),
                &raw mut written,
                ptr::null_mut(),
            )
        };
        assert_ne!(ok, 0, "{}", io::Error::last_os_error());
    }

    #[test]
    fn dropping_a_pending_operation_cancels_it_before_returning() {
        let pipe = pipe("drop");
        let mut overlapped = Overlapped::new(Some(Event::new(true, false).unwrap())).unwrap();
        let mut buffer = vec![0u8; 16];

        start_read(pipe.server.as_raw_handle(), &mut overlapped, &mut buffer);
        // SAFETY: the read was started on the server end with this overlapped and buffer.
        let pending = unsafe { PendingIo::new(&pipe.server, &mut overlapped, &mut buffer) };
        drop(pending);

        assert_ne!(
            overlapped.overlapped.Internal, STATUS_PENDING,
            "the kernel still owns the request after the guard was dropped"
        );
        write(&pipe.client, b"late");
        assert_eq!(
            buffer,
            vec![0u8; 16],
            "a cancelled read wrote into the buffer"
        );
    }

    #[test]
    fn finishing_a_pending_operation_returns_the_bytes_it_transferred() {
        let pipe = pipe("finish");
        let mut overlapped = Overlapped::new(Some(Event::new(true, false).unwrap())).unwrap();
        let mut buffer = vec![0u8; 16];

        start_read(pipe.server.as_raw_handle(), &mut overlapped, &mut buffer);
        // SAFETY: the read was started on the server end with this overlapped and buffer.
        let pending = unsafe { PendingIo::new(&pipe.server, &mut overlapped, &mut buffer) };
        write(&pipe.client, b"done");

        assert_eq!(pending.finish().unwrap(), 4);
        assert_eq!(&buffer[..4], b"done");
    }
}
