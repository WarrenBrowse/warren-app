//! One HTTP stack for every API request that leaves on an ordinary socket,
//! retired at each TUN transition.
//!
//! Sharing the stack is what keeps reqwest's connection pool: one TCP plus
//! TLS 1.3 handshake per host instead of one per call, and one root store
//! built per process instead of two per signed request. A pooled connection
//! is bound to the network it was opened on, though: once the VpnService
//! routes are installed, the same TCP flow leaves through the exit under
//! another source address, the server never answers it, and the first request
//! after the TUN came up died in the transport's 15 s timeout, twice in a row
//! on an exit switch (`android/docs/PERF-BASELINE.md`, S4 run 1). So every TUN
//! transition and every network handover retires the current stack, and the
//! next request opens a fresh connection on the network it will actually use.

use std::sync::Arc;

use parking_lot::Mutex;
use warren_api::transport::{HttpRequest, HttpResponse, HttpTransport, TransportError};

/// A lazily built transport that can be retired and rebuilt.
pub(crate) struct TransportSlot<T> {
    current: Mutex<Option<Arc<T>>>,
    build: fn() -> T,
}

impl<T> TransportSlot<T> {
    pub(crate) const fn new(build: fn() -> T) -> Self {
        Self {
            current: Mutex::new(None),
            build,
        }
    }

    /// The live stack, built on first use and after every [`Self::retire`].
    pub(crate) fn current(&self) -> Arc<T> {
        let mut slot = self.current.lock();
        match &*slot {
            Some(transport) => Arc::clone(transport),
            None => {
                let transport = Arc::new((self.build)());
                *slot = Some(Arc::clone(&transport));
                transport
            }
        }
    }

    /// Drops the live stack and its pooled connections; requests in flight
    /// keep their own handle and finish on it.
    pub(crate) fn retire(&self) {
        self.current.lock().take();
    }
}

/// An [`HttpTransport`] that resolves the slot's live stack per request, so a
/// client built once keeps working across retirements.
pub(crate) struct SharedTransport<T: 'static> {
    slot: &'static TransportSlot<T>,
}

impl<T: 'static> SharedTransport<T> {
    pub(crate) const fn new(slot: &'static TransportSlot<T>) -> Self {
        Self { slot }
    }
}

impl<T: HttpTransport + 'static> HttpTransport for SharedTransport<T> {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let transport = self.slot.current();
        transport.execute(request).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use warren_api::transport::{HttpRequest, HttpResponse, HttpTransport, Method, TransportError};

    use super::{SharedTransport, TransportSlot};

    /// A stand-in stack that answers with the number of stacks built before
    /// it, so a response names the instance that served it.
    struct Probe {
        id: usize,
    }

    impl HttpTransport for Probe {
        async fn execute(&self, _request: HttpRequest) -> Result<HttpResponse, TransportError> {
            Ok(HttpResponse {
                status: 200,
                body: self.id.to_string().into_bytes(),
            })
        }
    }

    fn request() -> HttpRequest {
        HttpRequest {
            method: Method::Get,
            url: "https://api.example.test/v1/network".to_owned(),
            headers: Vec::new(),
            body: Vec::new(),
            use_sni: true,
        }
    }

    static BUILT_A: AtomicUsize = AtomicUsize::new(0);
    fn build_a() -> Probe {
        Probe {
            id: BUILT_A.fetch_add(1, Ordering::SeqCst),
        }
    }
    static SLOT_A: TransportSlot<Probe> = TransportSlot::new(build_a);

    #[test]
    fn the_stack_is_built_once_and_shared() {
        let first = SLOT_A.current();
        let second = SLOT_A.current();

        assert!(
            Arc::ptr_eq(&first, &second),
            "every caller must get the same pool"
        );
        assert_eq!(BUILT_A.load(Ordering::SeqCst), 1);
    }

    static BUILT_B: AtomicUsize = AtomicUsize::new(0);
    fn build_b() -> Probe {
        Probe {
            id: BUILT_B.fetch_add(1, Ordering::SeqCst),
        }
    }
    static SLOT_B: TransportSlot<Probe> = TransportSlot::new(build_b);

    #[tokio::test]
    async fn retiring_the_stack_makes_the_next_request_open_a_new_one() {
        // A client built once, as the JNI singletons are.
        let shared = SharedTransport::new(&SLOT_B);
        let before = shared.execute(request()).await.unwrap().body;

        SLOT_B.retire();
        let after = shared.execute(request()).await.unwrap().body;

        assert_eq!(before, b"0", "the first request built the first stack");
        assert_eq!(after, b"1", "a TUN transition must not reuse the old pool");
        assert_eq!(BUILT_B.load(Ordering::SeqCst), 2);
    }
}
