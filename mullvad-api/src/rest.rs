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

    /// Warren fork — Phase 2.A.3 : un helper `signed_*` a été appelé
    /// sur une [`RequestFactory`] qui n'a pas de
    /// [`crate::warren_auth::WarrenAuthSigner`] configuré (cf.
    /// [`RequestFactory::with_warren_signer`]). Pas de fallback
    /// silencieux : on refuse de poster une requête non-signée vers
    /// `warren-api` (qui rejetterait avec 401).
    #[error("Warren signer not configured on request factory")]
    NoWarrenSigner,

    /// Warren fork — Phase 2.A.3 : échec d'injection des headers
    /// `X-Warren-*` sur une requête. En pratique impossible (les
    /// valeurs sont hex ou décimal ASCII), gardé pour propagation
    /// stricte du `Result` exposé par
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
    /// Warren fork — Phase 2.A.3 : utile pour l'inspection (tests,
    /// logs, debug) après injection des headers `X-Warren-*` par
    /// [`RequestFactory::signed_post_json_bytes`] et consorts.
    pub fn headers(&self) -> &http::HeaderMap {
        self.request.headers()
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
    /// Warren fork — Phase 2.A.3 : signer optionnel injecté via
    /// [`Self::with_warren_signer`]. Quand présent, les helpers
    /// `signed_*` injectent les 4 headers `X-Warren-*` ; quand absent,
    /// ces helpers retournent [`Error::NoWarrenSigner`].
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

    /// Warren fork — configure un [`WarrenAuthSigner`] sur la factory.
    /// Tous les helpers `signed_*` ultérieurs utiliseront ce signer
    /// pour produire les headers `X-Warren-*`. Le signer est partagé
    /// via [`Arc`] parce qu'il sera typiquement détenu par
    /// `MullvadRestHandle` et cloné dans plusieurs services daemon.
    #[must_use]
    pub fn with_warren_signer(mut self, signer: Arc<WarrenAuthSigner>) -> Self {
        self.warren_signer = Some(signer);
        self
    }

    /// Warren fork — `true` si la factory a été configurée avec un
    /// signer Warren via [`Self::with_warren_signer`]. Utilisé par
    /// les services daemon pour détecter le mode auth (Warren signed
    /// vs Mullvad Bearer historique).
    #[must_use]
    pub fn has_warren_signer(&self) -> bool {
        self.warren_signer.is_some()
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

    /// Warren fork — Phase 2.A.3 : variante signée de [`Self::get`].
    /// Aucun body (= sha256(b"") dans le canonical message) ; les 4
    /// headers `X-Warren-*` sont posés sur la requête `Empty<Bytes>`.
    ///
    /// # Errors
    ///
    /// - [`Error::NoWarrenSigner`] si pas de signer configuré.
    pub fn signed_get(&self, path: &str) -> Result<Request<Empty<Bytes>>> {
        self.signed_empty_body_request(path, Method::GET)
    }

    /// Warren fork — Phase 2.A.3 : variante signée de
    /// [`Self::delete`]. Idem [`Self::signed_get`] mais avec
    /// `method = DELETE` dans le canonical (anti-replay cross-method).
    ///
    /// # Errors
    ///
    /// - [`Error::NoWarrenSigner`] si pas de signer configuré.
    pub fn signed_delete(&self, path: &str) -> Result<Request<Empty<Bytes>>> {
        self.signed_empty_body_request(path, Method::DELETE)
    }

    /// Helper privé : factorise les variantes `signed_get` /
    /// `signed_delete` qui partagent le même body (`Empty<Bytes>`) et
    /// la même mécanique de signature (sha256 sur body vide).
    fn signed_empty_body_request(
        &self,
        path: &str,
        method: Method,
    ) -> Result<Request<Empty<Bytes>>> {
        let mut request = self.hyper_request::<Empty<Bytes>>(path, method)?;
        self.inject_warren_signature(&mut request, b"")?;
        Ok(Request::new(request, None).timeout(self.default_timeout))
    }

    /// Helper privé partagé par tous les helpers `signed_*` : checke
    /// la présence d'un signer Warren puis injecte les 4 headers
    /// `X-Warren-*` sur la `hyper::Request` via
    /// [`WarrenAuthSigner::apply_to_request`]. Mappe les erreurs
    /// d'injection vers [`Error::WarrenAuthInjection`].
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

    /// Warren fork — Phase 2.A.3 : variante signée de
    /// [`Self::post_json`]. Sérialise `body` via `serde_json` puis
    /// délègue à [`Self::signed_post_json_bytes`] (qui pose les 4
    /// headers `X-Warren-*`).
    ///
    /// # Errors
    ///
    /// - [`Error::DeserializeError`] si la sérialisation `serde_json`
    ///   échoue (cas exotiques : map à clé non-string).
    /// - [`Error::NoWarrenSigner`] si pas de signer configuré.
    pub fn signed_post_json<S: serde::Serialize>(
        &self,
        path: &str,
        body: &S,
    ) -> Result<Request<Full<Bytes>>> {
        let json_body = serde_json::to_vec(body)?;
        self.signed_post_json_bytes(path, json_body)
    }

    /// Warren fork — Phase 2.A.3 : variante de [`Self::post_json_bytes`]
    /// qui injecte une signature Ed25519 (4 headers `X-Warren-*`)
    /// produite par le [`WarrenAuthSigner`] configuré via
    /// [`Self::with_warren_signer`].
    ///
    /// La requête résultante a les mêmes headers HTTP standard
    /// (`Content-Type: application/json`, `Content-Length`) que
    /// [`Self::post_json_bytes`], **plus** les 4 headers `X-Warren-*`
    /// (cf. [`crate::warren_auth`] § format canonique).
    ///
    /// Le `token_store` n'est volontairement pas associé : Warren
    /// remplace le modèle Bearer par signature de requête, donc
    /// l'`Authorization` header ne doit pas être posé pour ces
    /// endpoints (sinon le serveur tente une double-validation).
    ///
    /// # Errors
    ///
    /// - [`Error::NoWarrenSigner`] si la factory n'a pas de signer
    ///   configuré.
    /// - Erreurs propagées de [`Self::hyper_request`] et de
    ///   l'injection HTTP (impossibles en pratique : valeurs hex
    ///   ASCII).
    pub fn signed_post_json_bytes(
        &self,
        path: &str,
        body: Vec<u8>,
    ) -> Result<Request<Full<Bytes>>> {
        let mut request = self.hyper_request::<Full<Bytes>>(path, Method::POST)?;
        let body_length = body.len();

        // Sign avec body bytes encore détenu en `Vec<u8>` — l'ordre
        // évite un clone (`inject_warren_signature` lit le slice, pose
        // les headers ; le body est inséré juste après).
        self.inject_warren_signature(&mut request, &body)?;

        *request.body_mut() = Full::new(Bytes::from(body));
        let headers = request.headers_mut();
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from(body_length));
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );

        // Pas de `token_store` : Warren ne combine pas Bearer + signature.
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
    fn has_warren_signer_reflects_factory_state() {
        // Phase 2.B Vague 3 — un caller (e.g. mullvad-daemon ou un
        // test d'intégration) doit pouvoir interroger la factory pour
        // savoir si elle est configurée en mode Warren auth (pour
        // décider entre `signed_*` et helpers Bearer historiques).
        let bare = RequestFactory::new("api.example.test", None);
        assert!(!bare.has_warren_signer());
        let warren = bare.clone().with_warren_signer(fixed_signer());
        assert!(warren.has_warren_signer());
        // Builder ne mute pas l'original (= immutability conventionnelle) :
        assert!(
            !bare.has_warren_signer(),
            "builder ne doit pas muter l'original"
        );
    }

    #[test]
    fn signed_post_json_bytes_without_warren_signer_returns_error() {
        // Phase 2.A.3 — un caller qui appelle un helper `signed_*` sur
        // une factory qui n'a pas configuré de signer Warren doit
        // recevoir une erreur explicite (pas de panic, pas de header
        // injection silencieuse).
        let factory = RequestFactory::new("api.example.test", None);
        let res = factory.signed_post_json_bytes("v1/devices", b"{}".to_vec());
        assert!(
            matches!(res, Err(Error::NoWarrenSigner)),
            "expected Err(NoWarrenSigner), got {res:?}"
        );
    }

    #[test]
    fn signed_post_json_bytes_with_warren_signer_injects_four_warren_headers() {
        // Phase 2.A.3 — vrai chemin : factory configurée avec un
        // signer doit produire une requête avec les 4 X-Warren-*
        // headers (pubkey 64ch, sig 128ch, timestamp u64, nonce 32ch),
        // **plus** les headers HTTP standard (content-length,
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

        // Headers HTTP standard de json_request_with_bytes :
        let cl = h
            .get(http::header::CONTENT_LENGTH)
            .expect("CONTENT_LENGTH set");
        assert_eq!(
            cl.to_str().unwrap().parse::<usize>().unwrap(),
            body.len(),
            "content-length doit refléter la taille du body"
        );
        let ct = h.get(http::header::CONTENT_TYPE).expect("CONTENT_TYPE set");
        assert_eq!(ct, "application/json");
    }

    #[test]
    fn signed_get_signs_with_empty_body_hash() {
        // Phase 2.A.3 — pour les requêtes sans body (GET, DELETE,
        // HEAD), le `body_hash` côté canonical_message est sha256(b"")
        // = `e3b0c44...` (vector test figé dans warren_auth.rs). Ce
        // test vérifie que `signed_get` produit bien une requête où
        // la signature couvre cet empty-body-hash : on reconstitue
        // canonical avec sha256("") et on vérifie via la pubkey.
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};
        use sha2::{Digest, Sha256};

        let factory =
            RequestFactory::new("api.example.test", None).with_warren_signer(fixed_signer());
        let req = factory
            .signed_get("v1/exits")
            .expect("signed_get must succeed");
        let h = req.headers();

        // Pas de Content-Length sur GET (Empty<Bytes> body) :
        assert!(
            h.get(http::header::CONTENT_LENGTH).is_none(),
            "GET ne doit pas poser Content-Length"
        );
        // 4 headers Warren présents :
        for header in [
            HEADER_PUBKEY,
            HEADER_SIGNATURE,
            HEADER_TIMESTAMP,
            HEADER_NONCE,
        ] {
            assert!(h.contains_key(header), "{header} doit être présent");
        }

        // E2E verify avec sha256(empty) :
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
            .expect("signature GET avec empty body doit vérifier");
    }

    #[test]
    fn signed_get_without_warren_signer_returns_error() {
        // Symétrie avec `signed_post_json_bytes_without_warren_signer`
        // : le helper signed_get doit aussi refuser sans signer.
        let factory = RequestFactory::new("api.example.test", None);
        assert!(matches!(
            factory.signed_get("v1/exits"),
            Err(Error::NoWarrenSigner)
        ));
    }

    #[test]
    fn signed_delete_signs_with_empty_body_and_correct_method() {
        // DELETE = même mécanique que GET (Empty body) mais avec method
        // "DELETE" qui doit apparaître dans le canonical message.
        // Vérifie aussi que `signed_get` et `signed_delete` produisent
        // des signatures DIFFÉRENTES sur le même path (anti-replay
        // cross-method couvert par warren_auth.rs, ici on vérifie que
        // c'est plumbed jusqu'au niveau factory).
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
        // Pubkey identique (même signer), sig différente (method
        // différent dans le canonical) :
        assert_eq!(
            req_get.headers().get(HEADER_PUBKEY).unwrap(),
            req_del.headers().get(HEADER_PUBKEY).unwrap()
        );
        assert_ne!(
            sig_get, sig_del,
            "GET et DELETE sur même path doivent produire signatures différentes"
        );
    }

    #[test]
    fn signed_post_json_serializes_serde_value_then_signs_canonical_bytes() {
        // Phase 2.A.3 — `signed_post_json<S: Serialize>` doit produire
        // exactement la même requête que `signed_post_json_bytes` une
        // fois le body sérialisé via `serde_json::to_vec`. Test :
        // signer un même payload via les deux helpers en figeant
        // pubkey/timestamp/nonce indirectement (= via le même body
        // sha256), puis comparer les fields de canonical_message
        // (path, method, body_hash) qui sont les inputs déterministes.
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

        // Inputs déterministes :
        assert_eq!(
            req_serde.uri().path(),
            req_bytes.uri().path(),
            "path identique"
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
            "content-length identique (= taille body sérialisé)"
        );
        // 4 headers Warren présents dans les deux :
        for header in [
            HEADER_PUBKEY,
            HEADER_SIGNATURE,
            HEADER_TIMESTAMP,
            HEADER_NONCE,
        ] {
            assert!(h_serde.contains_key(header), "{header} sur req_serde");
            assert!(h_bytes.contains_key(header), "{header} sur req_bytes");
        }
        // Pubkey identique (même signer) :
        assert_eq!(
            h_serde.get(HEADER_PUBKEY).unwrap(),
            h_bytes.get(HEADER_PUBKEY).unwrap(),
            "même signer = même pubkey hex"
        );
    }

    #[test]
    fn signed_post_json_bytes_signature_verifies_e2e_via_pubkey() {
        // Phase 2.A.3 — test E2E qui mime ce que ferait le middleware
        // axum côté `warren-api` : extraire pubkey/sig/nonce/timestamp
        // depuis les headers, reconstituer le `canonical_message` à
        // partir de (méthode, path-and-query, body sha256), et
        // vérifier la signature. Si ce test passe, alors `warren-api`
        // accepte les requêtes produites par cette factory.
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};
        use sha2::{Digest, Sha256};

        let factory =
            RequestFactory::new("api.example.test", None).with_warren_signer(fixed_signer());
        let body = br#"{"exit_pubkey":"abc"}"#.to_vec();
        let req = factory
            .signed_post_json_bytes("v1/port-forward/request", body.clone())
            .expect("must succeed");
        let h = req.headers();

        // Extraction comme côté serveur :
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

        // Reconstitution du canonical_message : la méthode est POST,
        // le path inclut le préfixe `/` (hyper::Uri::path), et le body
        // sha256 hex.
        let body_hash_hex = hex::encode(Sha256::digest(&body));
        let path = req.uri().path();
        let canonical = format!("POST\n{path}\n{ts}\n{nonce_hex}\n{body_hash_hex}");

        vk.verify(canonical.as_bytes(), &sig)
            .expect("signature doit vérifier — sinon le wire format a divergé");
    }
}
