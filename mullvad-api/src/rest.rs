#[cfg(target_os = "android")]
pub use crate::https_client::SocketBypassRequest;
use crate::{
    DnsResolver,
    access::AccessTokenStore,
    availability::ApiAvailability,
    https_client::{HttpsConnector, HttpsConnectorHandle, InnerConnectionMode},
    proxy::ConnectionModeProvider,
    warren_auth::WarrenAuthSigner,
};
use futures::{
    channel::{mpsc, oneshot},
    stream::StreamExt,
};
use http_body_util::{BodyExt, Empty, Full, combinators::BoxBody};
use hyper::{
    Method, Uri,
    body::{Body, Buf, Bytes, Incoming},
    header::{self, HeaderValue},
};
use hyper_util::client::legacy::connect::Connect;
use mullvad_types::account::AccountNumber;
use std::{
    borrow::Cow,
    convert::Infallible,
    error::Error as StdError,
    str::FromStr,
    sync::{Arc, Weak},
    time::Duration,
};
use talpid_types::ErrorExt;

pub use hyper::StatusCode;

const USER_AGENT: &str = "mullvad-app";

pub type Result<T> = std::result::Result<T, Error>;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Describes all the ways a REST request can fail
#[derive(thiserror::Error, Debug, Clone)]
pub enum Error {
    #[error("REST client service is down")]
    RestServiceDown,

    #[error("Request cancelled")]
    Aborted,

    #[error("Legacy hyper error")]
    LegacyHyperError(#[from] Arc<hyper_util::client::legacy::Error>),

    #[error("Hyper error")]
    HyperError(#[from] Arc<hyper::Error>),

    #[error("Invalid header value")]
    InvalidHeaderError,

    #[error("HTTP error")]
    HttpError(#[from] Arc<http::Error>),

    #[error("Request timed out")]
    TimeoutError,

    #[error("Failed to deserialize data")]
    DeserializeError(#[from] Arc<serde_json::Error>),

    /// Unexpected response code
    #[error("Unexpected response status code {0} - {1}")]
    ApiError(StatusCode, String),

    /// The string given was not a valid URI.
    #[error("Not a valid URI {0}")]
    InvalidUri(#[from] Arc<http::uri::InvalidUri>),

    #[error("Set account number on factory with no access token store")]
    NoAccessTokenStore,

    /// Failed to obtain versions
    #[error("Failed to obtain versions")]
    FetchVersions(#[from] Arc<anyhow::Error>),

    /// Body exceeded size limit
    #[error("Body exceeded size limit")]
    BodyTooLarge,

    /// A `signed_*` helper was called on a [`RequestFactory`] that has
    /// no [`crate::warren_auth::WarrenAuthSigner`] configured (see
    /// [`RequestFactory::with_warren_signer`]). No silent fallback: we
    /// refuse to send an unsigned request to `warren-api` (which would
    /// reject it with 401).
    #[error("Warren signer not configured on request factory")]
    NoWarrenSigner,

    /// Failure to inject the `X-Warren-*` headers on a request. In
    /// practice impossible (values are hex or decimal ASCII), kept for
    /// strict propagation of the `Result` exposed by
    /// [`crate::warren_auth::WarrenAuthSigner::apply_to_request`].
    #[error("failed to inject Warren auth headers")]
    WarrenAuthInjection(Arc<std::io::Error>),
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl Error {
    pub fn is_network_error(&self) -> bool {
        matches!(
            self,
            Error::HyperError(_) | Error::LegacyHyperError(_) | Error::TimeoutError
        )
    }

    /// Return true if there was no route to the destination
    pub fn is_offline(&self) -> bool {
        match self {
            Error::LegacyHyperError(error)
                if error.is_connect()
                    && let Some(cause) = error.source()
                    && let Some(err) = cause.downcast_ref::<std::io::Error>() =>
            {
                err.raw_os_error() == Some(libc::ENETUNREACH)
            }
            // TODO: Currently, we use the legacy hyper client for all REST requests. If this
            // changes in the future, we likely need to match on `Error::HyperError` here and
            // determine how to achieve the equivalent behavior. See DES-1288.
            _ => false,
        }
    }

    pub fn is_aborted(&self) -> bool {
        matches!(self, Error::Aborted)
    }

    /// Returns a new instance for which `abortable_stream::Aborted` is mapped to `Self::Aborted`.
    fn map_aborted(self) -> Self {
        if let Error::HyperError(error) = &self {
            let mut source = error.source();
            while let Some(error) = source {
                let io_error: Option<&std::io::Error> = error.downcast_ref();
                if let Some(io_error) = io_error {
                    let abort_error: Option<&crate::abortable_stream::Aborted> =
                        io_error.get_ref().and_then(|inner| inner.downcast_ref());
                    if abort_error.is_some() {
                        return Self::Aborted;
                    }
                }
                source = error.source();
            }
        }
        self
    }
}

// TODO: Look into an alternative to using the legacy hyper client `DES-1288`
type RequestClient = hyper_util::client::legacy::Client<HttpsConnector, BoxBody<Bytes, Error>>;

/// A service that executes HTTP requests, allowing for on-demand termination of all in-flight
/// requests
pub(crate) struct RequestService<T: ConnectionModeProvider> {
    command_tx: Weak<mpsc::UnboundedSender<RequestCommand>>,
    command_rx: mpsc::UnboundedReceiver<RequestCommand>,
    connector_handle: HttpsConnectorHandle,
    client: RequestClient,
    connection_mode_provider: T,
    connection_mode_generation: usize,
    api_availability: ApiAvailability,
}

impl<T: ConnectionModeProvider + 'static> RequestService<T> {
    /// Constructs a new request service.
    pub fn spawn(
        api_availability: ApiAvailability,
        connection_mode_provider: T,
        dns_resolver: Arc<dyn DnsResolver>,
        #[cfg(target_os = "android")] socket_bypass_tx: Option<mpsc::Sender<SocketBypassRequest>>,
        #[cfg(any(feature = "api-override", test))] disable_tls: bool,
    ) -> RequestServiceHandle {
        let proxy_config = {
            let api_connection_mode = connection_mode_provider.initial();
            InnerConnectionMode::from(api_connection_mode)
        };
        let connector = HttpsConnector::new(
            dns_resolver,
            proxy_config,
            #[cfg(target_os = "android")]
            socket_bypass_tx.clone(),
            #[cfg(any(feature = "api-override", test))]
            disable_tls,
        );
        let client =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build(connector.clone());

        let (command_tx, command_rx) = mpsc::unbounded();
        let command_tx = Arc::new(command_tx);
        let service = Self {
            command_tx: Arc::downgrade(&command_tx),
            command_rx,
            connector_handle: connector.spawn(),
            client,
            connection_mode_provider,
            connection_mode_generation: 0,
            api_availability,
        };
        let handle = RequestServiceHandle { tx: command_tx };
        tokio::spawn(service.into_future());
        handle
    }

    async fn into_future(mut self) {
        loop {
            tokio::select! {
                new_mode = self.connection_mode_provider.receive() => {
                    let Some(new_mode) = new_mode else {
                        break;
                    };
                    self.connector_handle.set_connection_mode(new_mode);
                }
                command = self.command_rx.next() => {
                    let Some(command) = command else {
                        break;
                    };

                    self.process_command(command).await;
                }
            }
        }
        self.connector_handle.reset();
    }

    async fn process_command(&mut self, command: RequestCommand) {
        match command {
            RequestCommand::NewRequest(request, completion_tx) => {
                self.handle_new_request(request, completion_tx);
            }
            RequestCommand::Reset => {
                self.connector_handle.reset();
            }
            RequestCommand::NextApiConfig(generation) => {
                if generation == self.connection_mode_generation {
                    self.connection_mode_generation =
                        self.connection_mode_generation.wrapping_add(1);
                    self.connection_mode_provider.rotate().await;
                }
            }
        }
    }

    fn handle_new_request(
        &mut self,
        request: Request<BoxBody<Bytes, Error>>,
        completion_tx: oneshot::Sender<Result<Response<Incoming>>>,
    ) {
        let tx = self.command_tx.upgrade();

        let api_availability = self.api_availability.clone();
        let request_future = request
            .map(|r| http::Request::map(r, BodyExt::boxed))
            .into_future(self.client.clone(), api_availability.clone());

        let connection_mode_generation = self.connection_mode_generation;

        tokio::spawn(async move {
            let response = request_future.await.map_err(|error| error.map_aborted());

            // Switch API endpoint if the request failed due to a network error
            if let Err(err) = &response
                && err.is_network_error()
                && !api_availability.is_offline()
            {
                log::error!("{}", err.display_chain_with_msg("HTTP request failed"));
                if let Some(tx) = tx {
                    let _ = tx
                        .unbounded_send(RequestCommand::NextApiConfig(connection_mode_generation));
                }
            }

            let _ = completion_tx.send(response);
        });
    }
}

#[derive(Clone)]
/// A handle to interact with a spawned `RequestService`.
pub struct RequestServiceHandle {
    tx: Arc<mpsc::UnboundedSender<RequestCommand>>,
}

impl RequestServiceHandle {
    /// Resets the corresponding RequestService, dropping all in-flight requests.
    pub fn reset(&self) {
        let _ = self.tx.unbounded_send(RequestCommand::Reset);
    }

    /// Submits a `RestRequest` for execution to the request service.
    pub async fn request<B>(&self, request: Request<B>) -> Result<Response<Incoming>>
    where
        B: Body + Send + Sync + 'static,
        Error: From<B::Error>,
        Bytes: From<B::Data>,
    {
        let (completion_tx, completion_rx) = oneshot::channel();
        let request = request.map(|r| r.map(box_body));
        self.tx
            .unbounded_send(RequestCommand::NewRequest(request, completion_tx))
            .map_err(|_| Error::RestServiceDown)?;
        completion_rx.await.map_err(|_| Error::RestServiceDown)?
    }
}

#[derive(Debug)]
pub(crate) enum RequestCommand {
    NewRequest(
        Request<BoxBody<Bytes, Error>>,
        oneshot::Sender<std::result::Result<Response<Incoming>, Error>>,
    ),
    Reset,
    NextApiConfig(usize),
}

/// A REST request that is sent to the RequestService to be executed.
#[derive(Debug)]
pub struct Request<B> {
    request: hyper::Request<B>,
    timeout: Duration,
    access_token_store: Option<AccessTokenStore>,
    account: Option<AccountNumber>,
    expected_status: &'static [hyper::StatusCode],
}

// TODO: merge with `RequestFactory::get`
/// Constructs a GET request with the given URI. Returns an error if the URI is not valid.
pub fn get(uri: &str) -> Result<Request<Empty<Bytes>>> {
    let uri = hyper::Uri::from_str(uri)?;

    let mut builder = http::request::Builder::new()
        .method(Method::GET)
        .header(header::USER_AGENT, HeaderValue::from_static(USER_AGENT))
        .header(header::ACCEPT, HeaderValue::from_static("application/json"));
    if let Some(host) = uri.host() {
        builder = builder.header(
            header::HOST,
            HeaderValue::from_str(host).map_err(|_e| Error::InvalidHeaderError)?,
        );
    };

    let request = builder.uri(uri).body(Empty::<Bytes>::new())?;
    Ok(Request::new(request, None))
}

impl<B: Body> Request<B> {
    fn new(request: hyper::Request<B>, access_token_store: Option<AccessTokenStore>) -> Self {
        Self {
            request,
            timeout: DEFAULT_TIMEOUT,
            access_token_store,
            account: None,
            expected_status: &[],
        }
    }

    /// Set the account number to obtain authentication for.
    /// This fails if no store is set.
    pub fn account(mut self, account: AccountNumber) -> Result<Self> {
        if self.access_token_store.is_none() {
            return Err(Error::NoAccessTokenStore);
        }
        self.account = Some(account);
        Ok(self)
    }

    /// Sets timeout for the request.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn expected_status(mut self, expected_status: &'static [hyper::StatusCode]) -> Self {
        self.expected_status = expected_status;
        self
    }

    pub fn header<T: header::IntoHeaderName>(mut self, key: T, value: &str) -> Result<Self> {
        let header_value =
            http::HeaderValue::from_str(value).map_err(|_| Error::InvalidHeaderError)?;
        self.request.headers_mut().insert(key, header_value);
        Ok(self)
    }

    /// Returns the URI of the request
    pub fn uri(&self) -> &Uri {
        self.request.uri()
    }

    /// Returns the headers of the underlying [`hyper::Request`].
    ///
    /// Useful for inspection (tests, logs, debug) after the
    /// `X-Warren-*` headers have been injected by
    /// [`RequestFactory::signed_post_json_bytes`] and friends.
    pub fn headers(&self) -> &http::HeaderMap {
        self.request.headers()
    }

    /// Returns the HTTP method of the underlying [`hyper::Request`].
    ///
    /// Useful for tests that verify a dispatcher (`*_or_signed`)
    /// produces the correct HTTP method (critical cross-method
    /// anti-replay regression).
    pub fn method(&self) -> &http::Method {
        self.request.method()
    }
}
impl<B> Request<B> {
    /// Map the underlying [`hyper::Request`] type
    fn map<F, B2>(self, f: F) -> Request<B2>
    where
        F: FnOnce(hyper::Request<B>) -> hyper::Request<B2>,
    {
        Request {
            request: f(self.request),
            timeout: self.timeout,
            access_token_store: self.access_token_store,
            account: self.account,
            expected_status: self.expected_status,
        }
    }
}

fn box_body<B>(body: B) -> BoxBody<Bytes, Error>
where
    B: Body + Send + Sync + 'static,
    Error: From<B::Error>,
    Bytes: From<B::Data>,
{
    try_downcast(body).unwrap_or_else(|body| {
        body.map_frame(|frame| frame.map_data(Bytes::from))
            .map_err(Error::from)
            .boxed()
    })
}

pub(crate) fn try_downcast<T, K>(k: K) -> core::result::Result<T, K>
where
    T: 'static,
    K: Send + 'static,
{
    let mut k = Some(k);
    if let Some(k) = <dyn std::any::Any>::downcast_mut::<Option<T>>(&mut k) {
        Ok(k.take().unwrap())
    } else {
        Err(k.unwrap())
    }
}

impl<B> Request<B>
where
    B: Body + Send + 'static + Unpin,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    async fn into_future<C: Connect + Clone + Send + Sync + 'static>(
        self,
        hyper_client: hyper_util::client::legacy::Client<C, B>,
        api_availability: ApiAvailability,
    ) -> Result<Response<Incoming>> {
        let timeout = self.timeout;
        let inner_fut = self.into_future_without_timeout(hyper_client, api_availability);
        tokio::time::timeout(timeout, inner_fut)
            .await
            .map_err(|_| Error::TimeoutError)?
    }

    async fn into_future_without_timeout<C>(
        mut self,
        hyper_client: hyper_util::client::legacy::Client<C, B>,
        api_availability: ApiAvailability,
    ) -> Result<Response<Incoming>>
    where
        C: Connect + Clone + Send + Sync + 'static,
    {
        let _ = api_availability.wait_for_unsuspend().await;

        // Obtain access token first
        if let (Some(account), Some(store)) = (&self.account, &self.access_token_store) {
            let access_token = store.get_token(account).await?;
            let auth = HeaderValue::from_str(&format!("Bearer {access_token}"))
                .map_err(|_| Error::InvalidHeaderError)?;
            self.request
                .headers_mut()
                .insert(header::AUTHORIZATION, auth);
        }

        // Make request to hyper client
        let response = hyper_client
            .request(self.request)
            .await
            .map_err(Error::from);

        // Notify access token store of expired tokens
        if let (Some(account), Some(store)) = (&self.account, &self.access_token_store) {
            store.check_response(account, &response);
        }

        // Parse unexpected responses and errors
        let response = response?;

        if !self.expected_status.contains(&response.status()) {
            if !self.expected_status.is_empty() {
                log::error!(
                    "Unexpected HTTP status code {}, expected codes [{}]",
                    response.status(),
                    self.expected_status
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                );
            }
            if !response.status().is_success() {
                return handle_error_response(response).await;
            }
        }

        Ok(Response::new(response))
    }
}

/// Successful result of a REST request
#[derive(Debug)]
pub struct Response<B> {
    response: hyper::Response<B>,
}

impl<B: Body + Unpin> Response<B>
where
    Error: From<<B as Body>::Error>,
{
    fn new(response: hyper::Response<B>) -> Self {
        Self { response }
    }

    pub fn status(&self) -> StatusCode {
        self.response.status()
    }

    pub fn headers(&self) -> &hyper::HeaderMap<HeaderValue> {
        self.response.headers()
    }

    pub async fn deserialize<T: serde::de::DeserializeOwned>(self) -> Result<T> {
        deserialize_body_inner(self.response).await
    }

    pub async fn body(self) -> Result<Vec<u8>> {
        Ok(BodyExt::collect(self.response).await?.to_bytes().to_vec())
    }

    pub async fn body_with_max_size(self, size_limit: usize) -> Result<Vec<u8>> {
        let mut data: Vec<u8> = vec![];
        let mut stream = self.response.into_data_stream();

        while let Some(chunk) = stream.next().await {
            data.extend(chunk?.chunk());
            if data.len() > size_limit {
                return Err(Error::BodyTooLarge);
            }
        }

        Ok(data)
    }
}

#[derive(serde::Deserialize)]
struct OldErrorResponse {
    pub code: String,
}

/// If `NewErrorResponse::type` is not defined it should default to "about:blank"
const DEFAULT_ERROR_TYPE: &str = "about:blank";
#[derive(serde::Deserialize)]
struct NewErrorResponse {
    pub r#type: Option<String>,
}

#[derive(Clone)]
pub struct RequestFactory {
    hostname: Cow<'static, str>,
    token_store: Option<AccessTokenStore>,
    default_timeout: Duration,
    /// Optional signer injected via [`Self::with_warren_signer`].
    /// When present, the `signed_*` helpers inject the 4
    /// `X-Warren-*` headers; when absent, those helpers return
    /// [`Error::NoWarrenSigner`].
    warren_signer: Option<Arc<WarrenAuthSigner>>,
}

impl RequestFactory {
    pub fn new(
        hostname: impl Into<Cow<'static, str>>,
        token_store: Option<AccessTokenStore>,
    ) -> Self {
        Self {
            hostname: hostname.into(),
            token_store,
            default_timeout: DEFAULT_TIMEOUT,
            warren_signer: None,
        }
    }

    /// Configures a [`WarrenAuthSigner`] on the factory. All subsequent
    /// `signed_*` helpers use this signer to produce the `X-Warren-*`
    /// headers. The signer is shared via [`Arc`] because it is
    /// typically owned by `MullvadRestHandle` and cloned into several
    /// daemon services.
    #[must_use]
    pub fn with_warren_signer(mut self, signer: Arc<WarrenAuthSigner>) -> Self {
        self.warren_signer = Some(signer);
        self
    }

    /// `true` if the factory was configured with a Warren signer via
    /// [`Self::with_warren_signer`]. Used by daemon services to detect
    /// the auth mode (Warren signed vs legacy Mullvad Bearer).
    #[must_use]
    pub fn has_warren_signer(&self) -> bool {
        self.warren_signer.is_some()
    }

    /// `true` if the factory carries an [`AccessTokenStore`] (= a
    /// non-test factory built by
    /// [`crate::Runtime::mullvad_rest_handle_with_warren_signer`]).
    /// Production daemon paths require this invariant so that
    /// [`Request::account`] does not return [`Error::NoAccessTokenStore`]:
    /// the M4.H.A bench v1 caveat about that error against
    /// `api.warrenbrowse.com` was traced back to a dispatch issue
    /// (Phase G.4 routes Warren-Remote through `WarrenApiClient`, which
    /// bypasses this chain entirely). This getter exists so the
    /// invariant can be asserted in a regression test instead of being
    /// implicit in the call shape at lib.rs.
    #[must_use]
    pub fn has_access_token_store(&self) -> bool {
        self.token_store.is_some()
    }

    pub fn request<B: Body + Default>(&self, path: &str, method: Method) -> Result<Request<B>> {
        Ok(
            Request::new(self.hyper_request(path, method)?, self.token_store.clone())
                .timeout(self.default_timeout),
        )
    }

    pub fn get(&self, path: &str) -> Result<Request<Empty<Bytes>>> {
        self.request(path, Method::GET)
    }

    pub fn post(&self, path: &str) -> Result<Request<Empty<Bytes>>> {
        self.request(path, Method::POST)
    }

    pub fn put(&self, path: &str) -> Result<Request<Empty<Bytes>>> {
        self.request(path, Method::PUT)
    }

    pub fn delete(&self, path: &str) -> Result<Request<Empty<Bytes>>> {
        self.request(path, Method::DELETE)
    }

    pub fn head(&self, path: &str) -> Result<Request<Empty<Bytes>>> {
        self.request(path, Method::HEAD)
    }

    pub fn post_json<S: serde::Serialize>(
        &self,
        path: &str,
        body: &S,
    ) -> Result<Request<Full<Bytes>>> {
        self.json_request(Method::POST, path, body)
    }

    pub fn post_json_bytes(&self, path: &str, body: Vec<u8>) -> Result<Request<Full<Bytes>>> {
        self.json_request_with_bytes(Method::POST, path, body)
    }

    pub fn put_json<S: serde::Serialize>(
        &self,
        path: &str,
        body: &S,
    ) -> Result<Request<Full<Bytes>>> {
        self.json_request(Method::PUT, path, body)
    }

    pub fn default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }
    fn json_request_with_bytes(
        &self,
        method: Method,
        path: &str,
        body: Vec<u8>,
    ) -> Result<Request<Full<Bytes>>> {
        let mut request = self.hyper_request(path, method)?;

        let body_length = body.len();
        *request.body_mut() = Full::new(Bytes::from(body));

        let headers = request.headers_mut();
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from(body_length));
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );

        Ok(Request::new(request, self.token_store.clone()).timeout(self.default_timeout))
    }

    fn json_request<S: serde::Serialize>(
        &self,
        method: Method,
        path: &str,
        body: &S,
    ) -> Result<Request<Full<Bytes>>> {
        let json_body = serde_json::to_vec(&body)?;
        self.json_request_with_bytes(method, path, json_body)
    }

    /// GET variant that dispatches on [`Self::has_warren_signer`]. If a
    /// Warren signer is configured, the request is signed (with
    /// `X-Warren-*` headers); otherwise it is bare, like
    /// [`Self::get`].
    ///
    /// Lets legacy callers (e.g. `RelayListProxy`) migrate to Warren
    /// signing without wiring the "warren-mode-or-not" logic at every
    /// call site: a single helper centralizes the dispatch.
    ///
    /// # Errors
    ///
    /// Same errors as [`Self::get`] and [`Self::signed_get`] (invalid
    /// URI, header injection; the latter is impossible in practice).
    pub fn get_or_signed(&self, path: &str) -> Result<Request<Empty<Bytes>>> {
        if self.has_warren_signer() {
            self.signed_get(path)
        } else {
            self.get(path)
        }
    }

    /// Signed variant of [`Self::get`]. No body (= sha256(b"") in the
    /// canonical message); the 4 `X-Warren-*` headers are set on the
    /// `Empty<Bytes>` request.
    ///
    /// # Errors
    ///
    /// - [`Error::NoWarrenSigner`] if no signer is configured.
    pub fn signed_get(&self, path: &str) -> Result<Request<Empty<Bytes>>> {
        self.signed_empty_body_request(path, Method::GET)
    }

    /// Signed variant of [`Self::delete`]. Same as [`Self::signed_get`]
    /// but with `method = DELETE` in the canonical message
    /// (cross-method anti-replay).
    ///
    /// # Errors
    ///
    /// - [`Error::NoWarrenSigner`] if no signer is configured.
    pub fn signed_delete(&self, path: &str) -> Result<Request<Empty<Bytes>>> {
        self.signed_empty_body_request(path, Method::DELETE)
    }

    /// Signed variant of [`Self::post`] (POST **without body**, as
    /// opposed to [`Self::signed_post_json`]). The canonical message
    /// carries `method = POST` and an empty body.
    ///
    /// Used for endpoints like `POST /accounts` (create an account,
    /// where the body is empty and only the signed headers
    /// authenticate the caller).
    ///
    /// # Errors
    ///
    /// - [`Error::NoWarrenSigner`] if no signer is configured.
    pub fn signed_post(&self, path: &str) -> Result<Request<Empty<Bytes>>> {
        self.signed_empty_body_request(path, Method::POST)
    }

    /// Dispatcher between [`Self::delete`] and [`Self::signed_delete`]
    /// based on whether a Warren signer is configured. Symmetric of
    /// [`Self::get_or_signed`] for DELETE — used by `DELETE
    /// /accounts/me` and `DELETE /devices/{id}`.
    ///
    /// # Errors
    ///
    /// Same errors as [`Self::delete`] and [`Self::signed_delete`].
    pub fn delete_or_signed(&self, path: &str) -> Result<Request<Empty<Bytes>>> {
        if self.has_warren_signer() {
            self.signed_delete(path)
        } else {
            self.delete(path)
        }
    }

    /// Dispatcher between [`Self::post`] and [`Self::signed_post`]
    /// based on whether a Warren signer is configured.
    ///
    /// # Errors
    ///
    /// Same errors as [`Self::post`] and [`Self::signed_post`].
    pub fn post_or_signed(&self, path: &str) -> Result<Request<Empty<Bytes>>> {
        if self.has_warren_signer() {
            self.signed_post(path)
        } else {
            self.post(path)
        }
    }

    /// Dispatcher between [`Self::post_json`] and
    /// [`Self::signed_post_json`] based on whether a Warren signer is
    /// configured.
    ///
    /// # Errors
    ///
    /// Same errors as [`Self::post_json`] and
    /// [`Self::signed_post_json`].
    pub fn post_json_or_signed<S: serde::Serialize>(
        &self,
        path: &str,
        body: &S,
    ) -> Result<Request<Full<Bytes>>> {
        if self.has_warren_signer() {
            self.signed_post_json(path, body)
        } else {
            self.post_json(path, body)
        }
    }

    /// Dispatcher between [`Self::put_json`] and
    /// [`Self::signed_put_json`] based on whether a Warren signer is
    /// configured.
    ///
    /// # Errors
    ///
    /// Same errors as [`Self::put_json`] and
    /// [`Self::signed_put_json`].
    pub fn put_json_or_signed<S: serde::Serialize>(
        &self,
        path: &str,
        body: &S,
    ) -> Result<Request<Full<Bytes>>> {
        if self.has_warren_signer() {
            self.signed_put_json(path, body)
        } else {
            self.put_json(path, body)
        }
    }

    /// Signed variant of [`Self::put_json`]. The canonical message
    /// carries `method = PUT` + sha256(body); distinct from
    /// [`Self::signed_post_json`] to prevent cross-method replay
    /// (= a signed POST body cannot be replayed as a PUT and
    /// vice-versa).
    ///
    /// # Errors
    ///
    /// - [`Error::DeserializeError`] if `serde_json` serialization
    ///   fails (exotic cases: maps with non-string keys).
    /// - [`Error::NoWarrenSigner`] if no signer is configured.
    pub fn signed_put_json<S: serde::Serialize>(
        &self,
        path: &str,
        body: &S,
    ) -> Result<Request<Full<Bytes>>> {
        let json_body = serde_json::to_vec(body)?;
        self.signed_put_json_bytes(path, json_body)
    }

    /// Variant of [`Self::put_json_bytes`] that injects an Ed25519
    /// signature (4 `X-Warren-*` headers). Symmetric of
    /// [`Self::signed_post_json_bytes`] for PUT.
    ///
    /// # Errors
    ///
    /// - [`Error::NoWarrenSigner`] if no signer is configured.
    /// - Errors propagated from [`Self::hyper_request`] and from HTTP
    ///   header injection (impossible in practice).
    pub fn signed_put_json_bytes(&self, path: &str, body: Vec<u8>) -> Result<Request<Full<Bytes>>> {
        let mut request = self.hyper_request::<Full<Bytes>>(path, Method::PUT)?;
        let body_length = body.len();
        self.inject_warren_signature(&mut request, &body)?;
        *request.body_mut() = Full::new(Bytes::from(body));
        let headers = request.headers_mut();
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from(body_length));
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        Ok(Request::new(request, None).timeout(self.default_timeout))
    }

    /// Private helper: factorizes the `signed_get` / `signed_delete`
    /// variants, which share the same body (`Empty<Bytes>`) and the
    /// same signing mechanics (sha256 over an empty body).
    fn signed_empty_body_request(
        &self,
        path: &str,
        method: Method,
    ) -> Result<Request<Empty<Bytes>>> {
        let mut request = self.hyper_request::<Empty<Bytes>>(path, method)?;
        self.inject_warren_signature(&mut request, b"")?;
        Ok(Request::new(request, None).timeout(self.default_timeout))
    }

    /// Private helper shared by all `signed_*` helpers: checks that a
    /// Warren signer is present, then injects the 4 `X-Warren-*`
    /// headers on the `hyper::Request` via
    /// [`WarrenAuthSigner::apply_to_request`]. Maps injection errors
    /// to [`Error::WarrenAuthInjection`].
    fn inject_warren_signature<B>(
        &self,
        request: &mut http::Request<B>,
        body: &[u8],
    ) -> Result<()> {
        let signer = self.warren_signer.as_ref().ok_or(Error::NoWarrenSigner)?;
        signer
            .apply_to_request(request, body)
            .map_err(|e| Error::WarrenAuthInjection(Arc::new(e)))?;
        Ok(())
    }

    /// Signed variant of [`Self::post_json`]. Serializes `body` via
    /// `serde_json` and then delegates to
    /// [`Self::signed_post_json_bytes`] (which sets the 4
    /// `X-Warren-*` headers).
    ///
    /// # Errors
    ///
    /// - [`Error::DeserializeError`] if `serde_json` serialization
    ///   fails (exotic cases: maps with non-string keys).
    /// - [`Error::NoWarrenSigner`] if no signer is configured.
    pub fn signed_post_json<S: serde::Serialize>(
        &self,
        path: &str,
        body: &S,
    ) -> Result<Request<Full<Bytes>>> {
        let json_body = serde_json::to_vec(body)?;
        self.signed_post_json_bytes(path, json_body)
    }

    /// Variant of [`Self::post_json_bytes`] that injects an Ed25519
    /// signature (4 `X-Warren-*` headers) produced by the
    /// [`WarrenAuthSigner`] configured via
    /// [`Self::with_warren_signer`].
    ///
    /// The resulting request has the same standard HTTP headers
    /// (`Content-Type: application/json`, `Content-Length`) as
    /// [`Self::post_json_bytes`], **plus** the 4 `X-Warren-*` headers
    /// (see [`crate::warren_auth`] § canonical format).
    ///
    /// The `token_store` is intentionally not attached: Warren
    /// replaces the Bearer model with request signing, so the
    /// `Authorization` header must not be set for these endpoints
    /// (otherwise the server would attempt double-validation).
    ///
    /// # Errors
    ///
    /// - [`Error::NoWarrenSigner`] if the factory has no signer
    ///   configured.
    /// - Errors propagated from [`Self::hyper_request`] and from HTTP
    ///   header injection (impossible in practice: hex ASCII values).
    pub fn signed_post_json_bytes(
        &self,
        path: &str,
        body: Vec<u8>,
    ) -> Result<Request<Full<Bytes>>> {
        let mut request = self.hyper_request::<Full<Bytes>>(path, Method::POST)?;
        let body_length = body.len();

        // Sign while the body bytes are still held as `Vec<u8>` —
        // ordering avoids a clone (`inject_warren_signature` reads
        // the slice and sets the headers; the body is inserted right
        // after).
        self.inject_warren_signature(&mut request, &body)?;

        *request.body_mut() = Full::new(Bytes::from(body));
        let headers = request.headers_mut();
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from(body_length));
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );

        // No `token_store`: Warren does not combine Bearer + signature.
        Ok(Request::new(request, None).timeout(self.default_timeout))
    }

    fn hyper_request<B: Default>(&self, path: &str, method: Method) -> Result<http::Request<B>> {
        let uri = self.get_uri(path)?;
        let request = http::request::Builder::new()
            .method(method)
            .uri(uri)
            .header(header::USER_AGENT, HeaderValue::from_static(USER_AGENT))
            .header(header::ACCEPT, HeaderValue::from_static("application/json"))
            .header(
                header::HOST,
                HeaderValue::from_str(&self.hostname).map_err(|_| Error::InvalidHeaderError)?,
            );

        let result = request.body(B::default())?;
        Ok(result)
    }

    fn get_uri(&self, path: &str) -> Result<Uri> {
        let uri = format!("https://{}/{}", self.hostname, path);
        Ok(hyper::Uri::from_str(&uri)?)
    }
}

fn get_body_length<B>(response: &hyper::Response<B>) -> usize {
    response
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|header_value| header_value.to_str().ok())
        .and_then(|length| length.parse::<usize>().ok())
        .unwrap_or(0)
}

async fn handle_error_response<T, B: Body>(response: hyper::Response<B>) -> Result<T>
where
    Error: From<B::Error>,
{
    let status = response.status();
    let error_message = match status {
        hyper::StatusCode::METHOD_NOT_ALLOWED => "Method not allowed",
        status => match get_body_length(&response) {
            0 => status.canonical_reason().unwrap_or("Unexpected error"),
            _length => {
                return match response.headers().get("content-type") {
                    Some(content_type) if content_type == "application/problem+json" => {
                        // TODO: We should make sure we unify the new error format and the old
                        // error format so that they both produce the same Errors for the same
                        // problems after being processed.
                        let err: NewErrorResponse = deserialize_body_inner(response).await?;
                        // The new error type replaces the `code` field with the `type` field.
                        // This is what is used to programmatically check the error.
                        Err(Error::ApiError(
                            status,
                            err.r#type
                                .unwrap_or_else(|| String::from(DEFAULT_ERROR_TYPE)),
                        ))
                    }
                    _ => {
                        let err: OldErrorResponse = deserialize_body_inner(response).await?;
                        Err(Error::ApiError(status, err.code))
                    }
                };
            }
        },
    };
    Err(Error::ApiError(status, error_message.to_owned()))
}

async fn deserialize_body_inner<T, B>(response: hyper::Response<B>) -> Result<T>
where
    T: serde::de::DeserializeOwned,
    B: Body,
    Error: From<B::Error>,
{
    use http_body_util::BodyExt;

    let collected = BodyExt::collect(response).await?;
    let res = serde_json::from_slice(&collected.to_bytes())?;
    Ok(res)
}

#[derive(Clone)]
pub struct MullvadRestHandle {
    pub(crate) service: RequestServiceHandle,
    pub factory: RequestFactory,
    pub availability: ApiAvailability,
}

impl MullvadRestHandle {
    pub(crate) fn new(
        service: RequestServiceHandle,
        factory: RequestFactory,
        availability: ApiAvailability,
    ) -> Self {
        Self {
            service,
            factory,
            availability,
        }
    }

    pub fn service(&self) -> RequestServiceHandle {
        self.service.clone()
    }
}

macro_rules! impl_into_arc_err {
    ($ty:ty) => {
        impl From<$ty> for Error {
            fn from(error: $ty) -> Self {
                Error::from(Arc::from(error))
            }
        }
    };
}

impl_into_arc_err!(hyper::Error);
impl_into_arc_err!(hyper_util::client::legacy::Error);
impl_into_arc_err!(serde_json::Error);
impl_into_arc_err!(http::Error);
impl_into_arc_err!(http::uri::InvalidUri);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::warren_auth::{
        HEADER_NONCE, HEADER_PUBKEY, HEADER_SIGNATURE, HEADER_TIMESTAMP, WarrenAuthSigner,
    };
    use ed25519_dalek::SigningKey;

    fn fixed_signer() -> Arc<WarrenAuthSigner> {
        Arc::new(WarrenAuthSigner::new(SigningKey::from_bytes(&[7u8; 32])))
    }

    #[test]
    fn has_access_token_store_reports_constructor_value() {
        // M4.H.E.2 anti-regression for the bench v1 caveat
        // "Set account number on factory with no access token store"
        // observed against `api.warrenbrowse.com`. The production
        // `mullvad_rest_handle_with_warren_signer` always passes
        // `Some(token_store)` to `RequestFactory::new`, and the
        // dispatch in `mullvad-daemon/src/device/mod.rs` routes
        // Warren-Remote through `WarrenApiClient` which bypasses this
        // factory entirely. This test pins the boolean getter so the
        // higher-layer assertion at lib.rs has a stable surface to
        // call (instead of comparing factory shapes indirectly).
        let bare = RequestFactory::new("api.example.test", None);
        assert!(
            !bare.has_access_token_store(),
            "factory built with None must report has_access_token_store == false"
        );
    }

    #[test]
    fn account_returns_no_access_token_store_when_factory_lacks_store() {
        // Documents the exact error shape that the M4.H.A bench v1
        // observed. Any future regression that calls `.account()` on
        // a factory without a token store would still surface this
        // error verbatim — the test ensures the user-facing message
        // does not silently drift away from the documented caveat.
        let bare = RequestFactory::new("api.example.test", None);
        let req: Request<Empty<Bytes>> = bare.get("auth/v1/anything").expect("bare get builds");
        let err = req
            .account("test-account".to_owned())
            .expect_err("must fail: no token store");
        assert!(
            matches!(err, Error::NoAccessTokenStore),
            "must surface Error::NoAccessTokenStore verbatim, got {err}"
        );
    }

    #[test]
    fn get_or_signed_dispatches_on_has_warren_signer() {
        // Phase 2.A.4 V5 — `get_or_signed(path)` must return a signed
        // request (X-Warren-* headers) when a signer is configured,
        // and a bare request (no X-Warren-*) otherwise. The caller
        // (e.g. RelayListProxy) calls this helper without having to
        // wire the warren-vs-Bearer mode logic itself.
        let bare = RequestFactory::new("api.example.test", None);
        let bare_req = bare.get_or_signed("app/v1/relays").unwrap();
        assert!(
            !bare_req.headers().contains_key(HEADER_PUBKEY),
            "factory without signer must not set X-Warren-PubKey"
        );

        let warren =
            RequestFactory::new("api.example.test", None).with_warren_signer(fixed_signer());
        let warren_req = warren.get_or_signed("app/v1/relays").unwrap();
        assert!(
            warren_req.headers().contains_key(HEADER_PUBKEY),
            "factory with signer must set X-Warren-PubKey"
        );
        assert!(warren_req.headers().contains_key(HEADER_SIGNATURE));
        assert!(warren_req.headers().contains_key(HEADER_TIMESTAMP));
        assert!(warren_req.headers().contains_key(HEADER_NONCE));
    }

    #[test]
    fn has_warren_signer_reflects_factory_state() {
        // Phase 2.B Wave 3 — a caller (e.g. mullvad-daemon or an
        // integration test) must be able to query the factory to know
        // whether it is configured in Warren auth mode (to decide
        // between `signed_*` and legacy Bearer helpers).
        let bare = RequestFactory::new("api.example.test", None);
        assert!(!bare.has_warren_signer());
        let warren = bare.clone().with_warren_signer(fixed_signer());
        assert!(warren.has_warren_signer());
        // Builder does not mutate the original (= conventional immutability):
        assert!(
            !bare.has_warren_signer(),
            "builder must not mutate the original"
        );
    }

    #[test]
    fn signed_post_json_bytes_without_warren_signer_returns_error() {
        // Phase 2.A.3 — a caller that invokes a `signed_*` helper on
        // a factory with no Warren signer configured must receive an
        // explicit error (no panic, no silent header injection).
        let factory = RequestFactory::new("api.example.test", None);
        let res = factory.signed_post_json_bytes("v1/devices", b"{}".to_vec());
        assert!(
            matches!(res, Err(Error::NoWarrenSigner)),
            "expected Err(NoWarrenSigner), got {res:?}"
        );
    }

    #[test]
    fn signed_post_json_bytes_with_warren_signer_injects_four_warren_headers() {
        // Phase 2.A.3 — happy path: a factory configured with a
        // signer must produce a request with the 4 X-Warren-*
        // headers (pubkey 64ch, sig 128ch, timestamp u64, nonce
        // 32ch), **plus** the standard HTTP headers (content-length,
        // content-type=application/json).
        let factory =
            RequestFactory::new("api.example.test", None).with_warren_signer(fixed_signer());
        let body = br#"{"key":"value"}"#.to_vec();
        let req = factory
            .signed_post_json_bytes("v1/devices", body.clone())
            .expect("signed_post_json_bytes must succeed when signer is set");

        let h = req.headers();
        let pk = h.get(HEADER_PUBKEY).expect("X-Warren-PubKey present");
        let sig = h.get(HEADER_SIGNATURE).expect("X-Warren-Sig present");
        let ts = h.get(HEADER_TIMESTAMP).expect("X-Warren-Timestamp present");
        let nonce = h.get(HEADER_NONCE).expect("X-Warren-Nonce present");
        assert_eq!(pk.to_str().unwrap().len(), 64, "pubkey hex 64 chars");
        assert_eq!(sig.to_str().unwrap().len(), 128, "sig hex 128 chars");
        assert_eq!(nonce.to_str().unwrap().len(), 32, "nonce hex 32 chars");
        ts.to_str()
            .unwrap()
            .parse::<u64>()
            .expect("timestamp must parse as u64");

        // Standard HTTP headers from json_request_with_bytes:
        let cl = h
            .get(http::header::CONTENT_LENGTH)
            .expect("CONTENT_LENGTH set");
        assert_eq!(
            cl.to_str().unwrap().parse::<usize>().unwrap(),
            body.len(),
            "content-length must reflect the body size"
        );
        let ct = h.get(http::header::CONTENT_TYPE).expect("CONTENT_TYPE set");
        assert_eq!(ct, "application/json");
    }

    #[test]
    fn signed_get_signs_with_empty_body_hash() {
        // Phase 2.A.3 — for body-less requests (GET, DELETE, HEAD),
        // the `body_hash` in canonical_message is sha256(b"") =
        // `e3b0c44...` (frozen test vector in warren_auth.rs). This
        // test verifies that `signed_get` produces a request whose
        // signature covers that empty-body hash: we rebuild the
        // canonical with sha256("") and verify via the pubkey.
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};
        use sha2::{Digest, Sha256};

        let factory =
            RequestFactory::new("api.example.test", None).with_warren_signer(fixed_signer());
        let req = factory
            .signed_get("v1/exits")
            .expect("signed_get must succeed");
        let h = req.headers();

        // No Content-Length on GET (Empty<Bytes> body):
        assert!(
            h.get(http::header::CONTENT_LENGTH).is_none(),
            "GET must not set Content-Length"
        );
        // The 4 Warren headers must be present:
        for header in [
            HEADER_PUBKEY,
            HEADER_SIGNATURE,
            HEADER_TIMESTAMP,
            HEADER_NONCE,
        ] {
            assert!(h.contains_key(header), "{header} must be present");
        }

        // E2E verify with sha256(empty):
        let pk_hex = h.get(HEADER_PUBKEY).unwrap().to_str().unwrap();
        let sig_hex = h.get(HEADER_SIGNATURE).unwrap().to_str().unwrap();
        let ts: u64 = h
            .get(HEADER_TIMESTAMP)
            .unwrap()
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        let nonce_hex = h.get(HEADER_NONCE).unwrap().to_str().unwrap();
        let pk_bytes: [u8; 32] = hex::decode(pk_hex).unwrap().try_into().unwrap();
        let vk = VerifyingKey::from_bytes(&pk_bytes).unwrap();
        let sig_bytes: [u8; 64] = hex::decode(sig_hex).unwrap().try_into().unwrap();
        let sig = Signature::from_bytes(&sig_bytes);

        let body_hash_hex = hex::encode(Sha256::digest(b""));
        let path = req.uri().path();
        let canonical = format!("GET\n{path}\n{ts}\n{nonce_hex}\n{body_hash_hex}");
        vk.verify(canonical.as_bytes(), &sig)
            .expect("GET signature with empty body must verify");
    }

    #[test]
    fn signed_get_without_warren_signer_returns_error() {
        // Symmetry with `signed_post_json_bytes_without_warren_signer`:
        // the signed_get helper must also refuse without a signer.
        let factory = RequestFactory::new("api.example.test", None);
        assert!(matches!(
            factory.signed_get("v1/exits"),
            Err(Error::NoWarrenSigner)
        ));
    }

    #[test]
    fn signed_delete_signs_with_empty_body_and_correct_method() {
        // DELETE = same mechanics as GET (Empty body) but with method
        // "DELETE", which must appear in the canonical message. Also
        // verifies that `signed_get` and `signed_delete` produce
        // DIFFERENT signatures on the same path (cross-method
        // anti-replay is covered by warren_auth.rs; here we check
        // that it is properly plumbed up to the factory level).
        let factory =
            RequestFactory::new("api.example.test", None).with_warren_signer(fixed_signer());
        let req_get = factory.signed_get("v1/devices/abc").unwrap();
        let req_del = factory.signed_delete("v1/devices/abc").unwrap();
        let sig_get = req_get
            .headers()
            .get(HEADER_SIGNATURE)
            .unwrap()
            .to_str()
            .unwrap();
        let sig_del = req_del
            .headers()
            .get(HEADER_SIGNATURE)
            .unwrap()
            .to_str()
            .unwrap();
        // Same pubkey (same signer), different sig (different method
        // in the canonical):
        assert_eq!(
            req_get.headers().get(HEADER_PUBKEY).unwrap(),
            req_del.headers().get(HEADER_PUBKEY).unwrap()
        );
        assert_ne!(
            sig_get, sig_del,
            "GET and DELETE on the same path must produce different signatures"
        );
    }

    #[test]
    fn signed_post_json_serializes_serde_value_then_signs_canonical_bytes() {
        // Phase 2.A.3 — `signed_post_json<S: Serialize>` must produce
        // exactly the same request as `signed_post_json_bytes` once
        // the body has been serialized via `serde_json::to_vec`.
        // Test: sign the same payload through both helpers while
        // implicitly pinning pubkey/timestamp/nonce (= via the same
        // body sha256), then compare the deterministic canonical
        // inputs (path, method, body_hash).
        let factory =
            RequestFactory::new("api.example.test", None).with_warren_signer(fixed_signer());
        #[derive(serde::Serialize)]
        struct Payload<'a> {
            exit_pubkey: &'a str,
        }
        let payload = Payload { exit_pubkey: "abc" };
        let json_bytes = serde_json::to_vec(&payload).unwrap();

        let req_serde = factory
            .signed_post_json("v1/port-forward/request", &payload)
            .expect("signed_post_json must succeed");
        let req_bytes = factory
            .signed_post_json_bytes("v1/port-forward/request", json_bytes.clone())
            .expect("signed_post_json_bytes must succeed");

        // Deterministic inputs:
        assert_eq!(
            req_serde.uri().path(),
            req_bytes.uri().path(),
            "identical path"
        );
        let h_serde = req_serde.headers();
        let h_bytes = req_bytes.headers();
        assert_eq!(
            h_serde
                .get(http::header::CONTENT_LENGTH)
                .unwrap()
                .to_str()
                .unwrap(),
            h_bytes
                .get(http::header::CONTENT_LENGTH)
                .unwrap()
                .to_str()
                .unwrap(),
            "identical content-length (= serialized body size)"
        );
        // The 4 Warren headers must be present on both:
        for header in [
            HEADER_PUBKEY,
            HEADER_SIGNATURE,
            HEADER_TIMESTAMP,
            HEADER_NONCE,
        ] {
            assert!(h_serde.contains_key(header), "{header} on req_serde");
            assert!(h_bytes.contains_key(header), "{header} on req_bytes");
        }
        // Same pubkey (same signer):
        assert_eq!(
            h_serde.get(HEADER_PUBKEY).unwrap(),
            h_bytes.get(HEADER_PUBKEY).unwrap(),
            "same signer = same pubkey hex"
        );
    }

    #[test]
    fn signed_post_json_bytes_signature_verifies_e2e_via_pubkey() {
        // Phase 2.A.3 — E2E test that mimics what the axum middleware
        // in `warren-api` would do: extract pubkey/sig/nonce/timestamp
        // from the headers, rebuild the `canonical_message` from
        // (method, path-and-query, body sha256), and verify the
        // signature. If this test passes, then `warren-api` accepts
        // requests produced by this factory.
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};
        use sha2::{Digest, Sha256};

        let factory =
            RequestFactory::new("api.example.test", None).with_warren_signer(fixed_signer());
        let body = br#"{"exit_pubkey":"abc"}"#.to_vec();
        let req = factory
            .signed_post_json_bytes("v1/port-forward/request", body.clone())
            .expect("must succeed");
        let h = req.headers();

        // Server-side extraction:
        let pk_hex = h.get(HEADER_PUBKEY).unwrap().to_str().unwrap();
        let sig_hex = h.get(HEADER_SIGNATURE).unwrap().to_str().unwrap();
        let ts: u64 = h
            .get(HEADER_TIMESTAMP)
            .unwrap()
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        let nonce_hex = h.get(HEADER_NONCE).unwrap().to_str().unwrap();

        let pk_bytes: [u8; 32] = hex::decode(pk_hex).unwrap().try_into().unwrap();
        let vk = VerifyingKey::from_bytes(&pk_bytes).expect("valid pubkey");
        let sig_bytes: [u8; 64] = hex::decode(sig_hex).unwrap().try_into().unwrap();
        let sig = Signature::from_bytes(&sig_bytes);

        // Rebuild the canonical_message: method is POST, the path
        // includes the leading `/` (hyper::Uri::path), and the body
        // sha256 hex.
        let body_hash_hex = hex::encode(Sha256::digest(&body));
        let path = req.uri().path();
        let canonical = format!("POST\n{path}\n{ts}\n{nonce_hex}\n{body_hash_hex}");

        vk.verify(canonical.as_bytes(), &sig)
            .expect("signature must verify — otherwise the wire format has diverged");
    }

    /// Critical Phase D.1 regression: the 4 new `*_or_signed`
    /// dispatchers (`delete`, `post`, `post_json`, `put_json`) MUST
    /// sign when a Warren signer is configured, and MUST NOT sign
    /// otherwise. Without this property, the 10 critical REST
    /// endpoints migrated in D.1 would always fall back to legacy
    /// Bearer = an invisible bug identical to the one we are trying
    /// to fix.
    ///
    /// The test also covers the **correct HTTP method** per
    /// dispatcher: if someone accidentally swapped `delete_or_signed`
    /// to call `signed_get` instead of `signed_delete`, the canonical
    /// message would contain `GET` instead of `DELETE` → the server
    /// would reject with cross-method replay.
    #[test]
    fn all_or_signed_dispatchers_dispatch_and_use_correct_http_method() {
        let bare = RequestFactory::new("api.example.test", None);
        let warren =
            RequestFactory::new("api.example.test", None).with_warren_signer(fixed_signer());

        // List: (label, bare fn → req, warren fn → req, expected method).
        // We cannot trivially loop because the return types differ
        // (Empty vs Full body), so we inline.

        // delete_or_signed → DELETE
        let bare_req = bare.delete_or_signed("v1/devices/abc").unwrap();
        let warren_req = warren.delete_or_signed("v1/devices/abc").unwrap();
        assert!(
            !bare_req.headers().contains_key(HEADER_PUBKEY),
            "bare delete_or_signed must not sign"
        );
        assert!(
            warren_req.headers().contains_key(HEADER_PUBKEY),
            "warren delete_or_signed must sign"
        );
        assert_eq!(
            warren_req.method(),
            hyper::Method::DELETE,
            "delete_or_signed must produce a DELETE request"
        );

        // post_or_signed → POST without body
        let bare_req = bare.post_or_signed("v1/accounts").unwrap();
        let warren_req = warren.post_or_signed("v1/accounts").unwrap();
        assert!(!bare_req.headers().contains_key(HEADER_PUBKEY));
        assert!(warren_req.headers().contains_key(HEADER_PUBKEY));
        assert_eq!(warren_req.method(), hyper::Method::POST);

        // post_json_or_signed → POST with JSON body
        let payload = serde_json::json!({"voucher": "abc"});
        let bare_req = bare.post_json_or_signed("v1/voucher", &payload).unwrap();
        let warren_req = warren.post_json_or_signed("v1/voucher", &payload).unwrap();
        assert!(!bare_req.headers().contains_key(HEADER_PUBKEY));
        assert!(warren_req.headers().contains_key(HEADER_PUBKEY));
        assert_eq!(warren_req.method(), hyper::Method::POST);

        // put_json_or_signed → PUT with JSON body
        let bare_req = bare
            .put_json_or_signed("v1/devices/x/pubkey", &payload)
            .unwrap();
        let warren_req = warren
            .put_json_or_signed("v1/devices/x/pubkey", &payload)
            .unwrap();
        assert!(!bare_req.headers().contains_key(HEADER_PUBKEY));
        assert!(warren_req.headers().contains_key(HEADER_PUBKEY));
        assert_eq!(warren_req.method(), hyper::Method::PUT);
    }

    /// Phase D.1 cross-method anti-replay regression: the canonical
    /// message of a `signed_put_json` request must carry `PUT` (not
    /// `POST`). If someone copy-pastes the body of
    /// `signed_post_json_bytes` without changing the method, a signed
    /// `replace_wg_key` PUT payload would be *identical* to a signed
    /// POST `create_device` on the same path → server-side replay is
    /// possible.
    #[test]
    fn signed_put_json_signs_with_put_method_distinct_from_post() {
        let factory =
            RequestFactory::new("api.example.test", None).with_warren_signer(fixed_signer());
        let payload = serde_json::json!({"pubkey": "abc"});

        let req_post = factory.signed_post_json("v1/devices/x", &payload).unwrap();
        let req_put = factory.signed_put_json("v1/devices/x", &payload).unwrap();

        // Same pubkey (same signer), different methods:
        assert_eq!(
            req_post.headers().get(HEADER_PUBKEY).unwrap(),
            req_put.headers().get(HEADER_PUBKEY).unwrap()
        );
        assert_eq!(req_post.method(), hyper::Method::POST);
        assert_eq!(req_put.method(), hyper::Method::PUT);

        // DIFFERENT signatures (same path + body, but distinct method
        // in the canonical):
        let sig_post = req_post
            .headers()
            .get(HEADER_SIGNATURE)
            .unwrap()
            .to_str()
            .unwrap();
        let sig_put = req_put
            .headers()
            .get(HEADER_SIGNATURE)
            .unwrap()
            .to_str()
            .unwrap();
        assert_ne!(
            sig_post, sig_put,
            "signed_post_json and signed_put_json on the same path/body must produce different signatures (cross-method anti-replay)"
        );
    }
}
