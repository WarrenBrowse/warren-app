//! Server-side admission of every management RPC.
//!
//! The endpoint accepts connections from every local account, so what a
//! connection may do is decided per call, from the method it names and the
//! identity the transport recorded for the connection. This first decision
//! runs on the request head, before tonic reads and decodes the body, so a
//! refused caller never gets a buffer. It is not the last one: a client
//! chooses when to send the body, so the service decides again when it acts.

use std::{
    convert::Infallible,
    task::{Context, Poll},
};

use futures::future::{Either, Ready, ready};
use tonic::{
    Status,
    body::Body,
    codegen::{Service, http},
    server::NamedService,
};

/// What the gate hands on to the method with an admitted call.
pub type CallExtensions = http::Extensions;

use crate::{ManagementConnectInfo, PeerCredentials};

/// Decides whether the calling peer may invoke a method.
pub trait RpcGate: Clone + Send + Sync + 'static {
    /// `method` is the full gRPC path, `/<package>.<Service>/<Method>`, and
    /// `peer` is `None` when the transport could not establish who the caller
    /// is. An `Err` is sent to the caller as the call's status, and the method
    /// never runs. What an admitted call puts in `extensions` reaches the
    /// method in its `Request`.
    #[expect(
        clippy::result_large_err,
        reason = "the status is the refused call's whole response, built once per refusal"
    )]
    fn admit(
        &self,
        method: &str,
        peer: Option<&PeerCredentials>,
        extensions: &mut CallExtensions,
    ) -> Result<(), Status>;
}

/// A tonic service whose every call is first put to an [`RpcGate`].
#[derive(Clone)]
pub(crate) struct Gated<S, G> {
    inner: S,
    gate: G,
}

impl<S, G> Gated<S, G> {
    pub(crate) fn new(inner: S, gate: G) -> Self {
        Self { inner, gate }
    }
}

impl<S: NamedService, G> NamedService for Gated<S, G> {
    const NAME: &'static str = S::NAME;
}

impl<S, G> Service<http::Request<Body>> for Gated<S, G>
where
    S: Service<http::Request<Body>, Response = http::Response<Body>, Error = Infallible>,
    G: RpcGate,
{
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = Either<Ready<Result<Self::Response, Infallible>>, S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        let (mut head, body) = request.into_parts();
        let peer = head
            .extensions
            .get::<ManagementConnectInfo>()
            .cloned()
            .flatten();
        let mut admitted = CallExtensions::new();
        match self
            .gate
            .admit(head.uri.path(), peer.as_ref(), &mut admitted)
        {
            Ok(()) => {
                head.extensions.extend(admitted);
                Either::Right(self.inner.call(http::Request::from_parts(head, body)))
            }
            Err(status) => Either::Left(ready(Ok(status.into_http()))),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::Principal;

    type Seen = Arc<Mutex<Vec<(String, Option<PeerCredentials>)>>>;

    /// Admits exactly the methods it is told to, and records what it was asked.
    #[derive(Clone)]
    struct ListGate {
        admitted: &'static [&'static str],
        seen: Seen,
    }

    /// What the gate attaches to a call it admits.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct AdmittedAs(&'static str);

    impl RpcGate for ListGate {
        fn admit(
            &self,
            method: &str,
            peer: Option<&PeerCredentials>,
            extensions: &mut CallExtensions,
        ) -> Result<(), Status> {
            self.seen
                .lock()
                .unwrap()
                .push((method.to_owned(), peer.cloned()));
            if self.admitted.contains(&method) {
                extensions.insert(AdmittedAs("listed"));
                Ok(())
            } else {
                Err(Status::permission_denied("not yours"))
            }
        }
    }

    /// Stands in for a generated tonic server: counts the calls that reach it,
    /// and keeps what the gate attached to the last one.
    #[derive(Clone, Default)]
    struct CountingMethod {
        calls: Arc<Mutex<usize>>,
        admitted_as: Arc<Mutex<Option<AdmittedAs>>>,
    }

    impl Service<http::Request<Body>> for CountingMethod {
        type Response = http::Response<Body>;
        type Error = Infallible;
        type Future = Ready<Result<Self::Response, Infallible>>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, request: http::Request<Body>) -> Self::Future {
            *self.calls.lock().unwrap() += 1;
            *self.admitted_as.lock().unwrap() = request.extensions().get::<AdmittedAs>().cloned();
            ready(Ok(http::Response::new(Body::empty())))
        }
    }

    fn request(path: &str, peer: ManagementConnectInfo) -> http::Request<Body> {
        let mut request = http::Request::builder()
            .uri(path)
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(peer);
        request
    }

    fn user(uid: u32) -> PeerCredentials {
        PeerCredentials {
            principal: Principal::Uid(uid),
            privileged: false,
            session_id: None,
        }
    }

    fn gated(admitted: &'static [&'static str]) -> (Gated<CountingMethod, ListGate>, Seen) {
        let seen = Seen::default();
        let gate = ListGate {
            admitted,
            seen: seen.clone(),
        };
        (Gated::new(CountingMethod::default(), gate), seen)
    }

    #[tokio::test]
    async fn a_refused_call_never_reaches_the_method_and_carries_the_gate_status() {
        let (mut service, _) = gated(&[]);

        let response = service
            .call(request("/pkg.Service/Connect", Some(user(1000))))
            .await
            .unwrap();

        assert_eq!(*service.inner.calls.lock().unwrap(), 0);
        let status = Status::from_header_map(response.headers()).expect("a grpc status");
        assert_eq!(status.code(), tonic::Code::PermissionDenied);
        assert_eq!(status.message(), "not yours");
    }

    /// The method is reached, and with it what the gate attached: that is how
    /// the method learns the class it must decide again when it acts.
    #[tokio::test]
    async fn an_admitted_call_reaches_the_method_with_what_the_gate_attached() {
        let (mut service, _) = gated(&["/pkg.Service/Connect"]);

        service
            .call(request("/pkg.Service/Connect", Some(user(1000))))
            .await
            .unwrap();

        assert_eq!(*service.inner.calls.lock().unwrap(), 1);
        assert_eq!(
            *service.inner.admitted_as.lock().unwrap(),
            Some(AdmittedAs("listed"))
        );
    }

    /// The gate decides from the method path and from the identity the
    /// transport recorded for the connection, including its absence.
    #[tokio::test]
    async fn the_gate_is_asked_about_the_method_path_and_the_connection_identity() {
        let (mut service, seen) = gated(&[]);

        let _ = service
            .call(request("/pkg.Service/GetMnemonic", Some(user(1001))))
            .await;
        let _ = service.call(request("/pkg.Service/GetState", None)).await;

        assert_eq!(
            *seen.lock().unwrap(),
            vec![
                ("/pkg.Service/GetMnemonic".to_owned(), Some(user(1001))),
                ("/pkg.Service/GetState".to_owned(), None),
            ]
        );
    }
}
