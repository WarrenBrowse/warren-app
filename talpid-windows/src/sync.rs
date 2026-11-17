use std::{
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle},
    ptr,
};
use windows_sys::Win32::{
    Foundation::{DUPLICATE_SAME_ACCESS, DuplicateHandle},
    System::Threading::{CreateEventW, GetCurrentProcess, SetEvent},
};

/// Windows event object
pub struct Event(OwnedHandle);

// SAFETY: the only field is an `OwnedHandle`, which std already declares `Send`: an event is a
// kernel object whose handle may be used and closed from any thread of the process.
unsafe impl Send for Event {}
// SAFETY: `OwnedHandle` is also `Sync`, and every method takes `&self` and only hands the handle
// to thread-safe kernel calls (`SetEvent`, `DuplicateHandle`), so no Rust-side state can race.
unsafe impl Sync for Event {}

impl Event {
    /// Create a new event object using `CreateEventW`
    pub fn new(manual_reset: bool, initial_state: bool) -> io::Result<Self> {
        // SAFETY: both pointer arguments are null, which `CreateEventW` documents as "default
        // security descriptor, handle not inheritable" and "unnamed event". The other arguments
        // are plain integers.
        let event = unsafe {
            CreateEventW(
                ptr::null_mut(),
                i32::from(manual_reset),
                i32::from(initial_state),
                ptr::null(),
            )
        };
        if event.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `event` is a valid handle since `CreateEventW` succeeded
        Ok(Self(unsafe { OwnedHandle::from_raw_handle(event) }))
    }

    /// Signal the event object
    pub fn set(&self) -> io::Result<()> {
        // SAFETY: `self.0` is a valid handle
        if unsafe { SetEvent(self.0.as_raw_handle()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Duplicate the event object with `DuplicateHandle()`
    pub fn duplicate(&self) -> io::Result<Event> {
        let mut new_event = ptr::null_mut();
        // SAFETY: `GetCurrentProcess` returns a pseudo handle that needs no closing, the source
        // handle is owned by `self` and stays open for the duration of the call, and the target
        // pointer refers to a local that `DuplicateHandle` writes once on success.
        let status = unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                self.0.as_raw_handle(),
                GetCurrentProcess(),
                &raw mut new_event,
                0,
                0,
                DUPLICATE_SAME_ACCESS,
            )
        };
        if status == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `new_event` is a valid handle since `DuplicateHandle` succeeded
        Ok(Event(unsafe { OwnedHandle::from_raw_handle(new_event) }))
    }
}

impl AsRawHandle for Event {
    fn as_raw_handle(&self) -> RawHandle {
        self.0.as_raw_handle()
    }
}
