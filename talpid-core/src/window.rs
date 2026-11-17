//! Utilities for working with windows on Windows.

use std::{os::windows::io::AsRawHandle, ptr, sync::Arc, thread};
use tokio::sync::broadcast;
use windows_sys::{
    Win32::{
        Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM},
        System::{LibraryLoader::GetModuleHandleW, Threading::GetThreadId},
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GWLP_USERDATA,
            GWLP_WNDPROC, GetMessageW, GetWindowLongPtrW, PBT_APMRESUMEAUTOMATIC,
            PBT_APMRESUMESUSPEND, PBT_APMSUSPEND, PostQuitMessage, PostThreadMessageW,
            SetWindowLongPtrW, TranslateMessage, WM_DESTROY, WM_POWERBROADCAST, WM_USER,
        },
    },
    w,
};

const CLASS_NAME: *const u16 = w!("STATIC");
const REQUEST_THREAD_SHUTDOWN: u32 = WM_USER + 1;

/// Handle for closing an associated window.
/// The window is not destroyed when this is dropped.
pub struct WindowCloseHandle {
    thread: Option<std::thread::JoinHandle<()>>,
}

impl WindowCloseHandle {
    /// Close the window and wait for the thread.
    pub fn close(&mut self) {
        if let Some(thread) = self.thread.take() {
            // SAFETY: the thread handle is owned by `thread`, which is alive until the `join`
            // below, so it is open during the call.
            let thread_id = unsafe { GetThreadId(thread.as_raw_handle() as HANDLE) };
            // SAFETY: only integers are passed. A thread id that no longer has a message queue
            // makes the call fail, which reads no memory and is ignored here.
            unsafe { PostThreadMessageW(thread_id, REQUEST_THREAD_SHUTDOWN, 0, 0) };
            let _ = thread.join();
        }
    }
}

/// Creates a dummy window whose messages are handled by `wnd_proc`.
pub fn create_hidden_window<F: (Fn(HWND, u32, WPARAM, LPARAM) -> LRESULT) + Send + 'static>(
    wnd_proc: F,
) -> WindowCloseHandle {
    let join_handle = thread::spawn(move || {
        // SAFETY: `CLASS_NAME` is a static NUL-terminated UTF-16 string naming the system
        // "STATIC" class. The window name, parent, menu and creation parameter are null, which
        // the API accepts, and `GetModuleHandleW(null)` returns the handle of the running
        // executable without taking a reference. A failure returns a null handle, which the
        // calls below reject without touching memory.
        let dummy_window = unsafe {
            CreateWindowExW(
                0,
                CLASS_NAME,
                ptr::null_mut(),
                0,
                0,
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                GetModuleHandleW(ptr::null_mut()),
                ptr::null_mut(),
            )
        };

        // Move callback information to the heap.
        // This enables us to reach the callback through a "thin pointer".
        let raw_callback = Box::into_raw(Box::new(wnd_proc));

        // SAFETY: `dummy_window` was created by this thread, which is the only one allowed to
        // change it. The user data is set before the window procedure, so `window_procedure::<F>`
        // never sees a pointer of another type, and `raw_callback` stays allocated until after
        // the message loop below has ended.
        unsafe {
            SetWindowLongPtrW(dummy_window, GWLP_USERDATA, raw_callback as isize);
            SetWindowLongPtrW(
                dummy_window,
                GWLP_WNDPROC,
                // Clippy does not like casting function pointers to anything but usize.
                // But this is correct, since the Windows API expects a signed int for pointer.
                window_procedure::<F> as *const () as usize as isize,
            );
        }

        // SAFETY: `MSG` is a plain C struct of integers and pointers, for which all-zero bytes are
        // a valid value.
        let mut msg = unsafe { std::mem::zeroed() };

        loop {
            // SAFETY: `msg` is a live local `MSG` that `GetMessageW` fills in; a null window
            // filter means "any message of this thread".
            let status = unsafe { GetMessageW(&raw mut msg, ptr::null_mut(), 0, 0) };

            if status < 0 {
                continue;
            }
            if status == 0 {
                break;
            }

            if msg.hwnd.is_null() {
                if msg.message == REQUEST_THREAD_SHUTDOWN {
                    // SAFETY: `DestroyWindow` must run on the thread that created the window,
                    // which is this one; a null or already destroyed handle only makes it fail.
                    unsafe { DestroyWindow(dummy_window) };
                }
            } else {
                // SAFETY: `msg` was filled in by the successful `GetMessageW` above and is only
                // read by both calls.
                unsafe {
                    TranslateMessage(&raw const msg);
                    DispatchMessageW(&raw const msg);
                }
            }
        }

        // Free callback.
        // SAFETY: `raw_callback` came from `Box::into_raw` above and is freed only here. The
        // message loop has ended and this thread retrieves no more messages, so the window
        // procedure, the only other user of the pointer, can no longer run.
        let _ = unsafe { Box::from_raw(raw_callback) };
    });

    WindowCloseHandle {
        thread: Some(join_handle),
    }
}

unsafe extern "system" fn window_procedure<F>(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT
where
    F: Fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
{
    if message == WM_DESTROY {
        // SAFETY: only posts `WM_QUIT` to the queue of the calling thread, the window's own.
        unsafe { PostQuitMessage(0) };
        return 0;
    }
    // SAFETY: `window` is the live handle Windows passes to its own window procedure.
    let raw_callback = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) };
    if raw_callback != 0 {
        // SAFETY: this procedure is only installed by `create_hidden_window::<F>`, which stores
        // a `Box<F>` pointer in the user data first and frees it only after its message loop has
        // ended. The callback runs on the window's own thread. A re-entrant message sent while
        // the callback runs would create a second `&mut F`; `F: Fn`, so a shared reference would
        // be enough and would remove that aliasing.
        let typed_callback = unsafe { &mut *(raw_callback as *mut F) };
        return typed_callback(window, message, wparam, lparam);
    }
    // SAFETY: forwards unchanged the arguments Windows passed to this window procedure.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

/// Power management events
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub enum PowerManagementEvent {
    /// The system is resuming from sleep or hibernation
    /// irrespective of user activity.
    ResumeAutomatic,
    /// The system is resuming from sleep or hibernation
    /// due to user activity.
    ResumeSuspend,
    /// The computer is about to enter a suspended state.
    Suspend,
}

impl PowerManagementEvent {
    fn try_from_winevent(wparam: usize) -> Option<Self> {
        use PowerManagementEvent::*;
        match wparam as u32 {
            PBT_APMRESUMEAUTOMATIC => Some(ResumeAutomatic),
            PBT_APMRESUMESUSPEND => Some(ResumeSuspend),
            PBT_APMSUSPEND => Some(Suspend),
            _ => None,
        }
    }
}

/// Provides power management events to listeners
pub struct PowerManagementListener {
    _window: Arc<WindowScopedHandle>,
    rx: broadcast::Receiver<PowerManagementEvent>,
}

impl PowerManagementListener {
    /// Creates a new listener. This is expensive compared to cloning an existing instance.
    pub fn new() -> Self {
        let (tx, rx) = tokio::sync::broadcast::channel(16);

        let power_broadcast_callback = move |window, message, wparam, lparam| {
            if message == WM_POWERBROADCAST
                && let Some(event) = PowerManagementEvent::try_from_winevent(wparam)
                && tx.send(event).is_err()
            {
                log::error!("Stopping power management event monitor");
                // SAFETY: only posts `WM_QUIT` to the queue of the calling thread, the window's
                // own, since this closure only runs inside its window procedure.
                unsafe { PostQuitMessage(0) };
                return 0;
            }
            // SAFETY: forwards unchanged the arguments Windows passed to the window procedure
            // that invoked this closure.
            unsafe { DefWindowProcW(window, message, wparam, lparam) }
        };

        let window = create_hidden_window(power_broadcast_callback);

        Self {
            _window: Arc::new(WindowScopedHandle(window)),
            rx,
        }
    }

    /// Returns the next power event.
    pub async fn next(&mut self) -> Option<PowerManagementEvent> {
        loop {
            match self.rx.recv().await {
                Ok(event) => break Some(event),
                Err(broadcast::error::RecvError::Closed) => {
                    log::error!("Sender was unexpectedly dropped");
                    break None;
                }
                Err(broadcast::error::RecvError::Lagged(num_skipped)) => {
                    log::warn!("Skipped {num_skipped} power broadcast events");
                }
            }
        }
    }
}

impl Default for PowerManagementListener {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for PowerManagementListener {
    fn clone(&self) -> Self {
        Self {
            _window: self._window.clone(),
            rx: self.rx.resubscribe(),
        }
    }
}

struct WindowScopedHandle(WindowCloseHandle);

impl Drop for WindowScopedHandle {
    fn drop(&mut self) {
        self.0.close();
    }
}
