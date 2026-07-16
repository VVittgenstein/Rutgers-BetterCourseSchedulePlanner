use std::time::Duration;

#[cfg(test)]
use std::net::IpAddr;

use bcsp_contracts::TermCampusKey;
use reqwest::header::{
    ACCEPT, AGE, CACHE_CONTROL, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, DATE, ETAG,
    HeaderMap, LAST_MODIFIED, LOCATION, RETRY_AFTER,
};
use reqwest::{Client, Method, Request, Response, StatusCode, Url, redirect::Policy};
use thiserror::Error;

#[cfg(test)]
use crate::open_sections::RetryAfterValue;
use crate::open_sections::{RedirectScope, RetryAfterHeader, retry_after};
use crate::{RawCatalogCourse, SourceProvenance, decode_catalog_payload};

pub const RUTGERS_CATALOG_ENDPOINT: &str = "https://classes.rutgers.edu/soc/api/courses.json";
pub const CATALOG_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const CATALOG_RESPONSE_HEADER_TIMEOUT: Duration = Duration::from_secs(30);
pub const CATALOG_BODY_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
pub const CATALOG_TOTAL_TIMEOUT: Duration = Duration::from_secs(300);
pub const CATALOG_MAX_DECODED_BYTES: usize = 64 * 1024 * 1024;

/// One canonical Rutgers Catalog target and its strictly decoded URL fields.
///
/// Rutgers term identities used by the transport are exactly five ASCII
/// digits: one Rutgers term code (`0`, `1`, `7`, or `9`) followed by a
/// four-digit year. The
/// request retains the complete target so a decoded payload cannot silently be
/// reassigned to a different `(term, campus)` pair.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogRequest {
    target: TermCampusKey,
    year: u16,
    term_code: u8,
}

impl CatalogRequest {
    pub fn try_from_target(target: TermCampusKey) -> Result<Self, CatalogRequestError> {
        let term = target.term().as_str().as_bytes();
        if term.len() != 5 || !term.iter().all(u8::is_ascii_digit) {
            return Err(CatalogRequestError::InvalidTermIdentity);
        }
        let term_code = term[0] - b'0';
        if !matches!(term_code, 0 | 1 | 7 | 9) {
            return Err(CatalogRequestError::InvalidTermIdentity);
        }
        let year = std::str::from_utf8(&term[1..])
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .filter(|year| (1_000..=9_999).contains(year))
            .ok_or(CatalogRequestError::InvalidTermIdentity)?;
        Ok(Self {
            target,
            year,
            term_code,
        })
    }

    pub const fn target(&self) -> &TermCampusKey {
        &self.target
    }

    pub const fn year(&self) -> u16 {
        self.year
    }

    pub const fn term_code(&self) -> u8 {
        self.term_code
    }
}

impl TryFrom<TermCampusKey> for CatalogRequest {
    type Error = CatalogRequestError;

    fn try_from(target: TermCampusKey) -> Result<Self, Self::Error> {
        Self::try_from_target(target)
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CatalogRequestError {
    #[error(
        "Catalog target term must use Rutgers term code 0, 1, 7, or 9 followed by a four-digit year"
    )]
    InvalidTermIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogResponseMetadata {
    pub http_status: u16,
    pub decoded_bytes: usize,
    pub decoded_body_sha256: String,
    pub course_count: usize,
    pub content_type: String,
    pub content_encoding: Option<String>,
    pub content_length: Option<u64>,
    pub etag: Option<String>,
    pub cache_control: Option<String>,
    pub date: Option<String>,
    pub age_seconds: Option<u64>,
    pub last_modified: Option<String>,
}

/// A decoded Catalog payload whose target and provenance travel together.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogResponse {
    target: TermCampusKey,
    payload: Vec<RawCatalogCourse>,
    provenance: SourceProvenance,
    metadata: CatalogResponseMetadata,
}

impl CatalogResponse {
    pub const fn target(&self) -> &TermCampusKey {
        &self.target
    }

    pub fn payload(&self) -> &[RawCatalogCourse] {
        &self.payload
    }

    pub const fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }

    pub const fn metadata(&self) -> &CatalogResponseMetadata {
        &self.metadata
    }

    /// Return the exact argument tuple accepted by
    /// `bcsp_catalog::normalize_target` without adding a dependency edge from
    /// the transport crate back to the catalog crate.
    pub fn into_normalization_parts(
        self,
    ) -> (TermCampusKey, Vec<RawCatalogCourse>, SourceProvenance) {
        (self.target, self.payload, self.provenance)
    }
}

/// Redacted response facts retained when a response cannot become a Catalog
/// snapshot. The response body is never retained; callers may separately
/// persist a bounded, sanitized error-chain summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogFailureResponseMetadata {
    pub http_status: u16,
    pub decoded_bytes: Option<usize>,
    pub decoded_body_sha256: Option<String>,
    pub content_type: Option<String>,
    pub content_encoding: Option<String>,
    pub content_length: Option<u64>,
    pub etag: Option<String>,
    pub cache_control: Option<String>,
    pub date: Option<String>,
    pub age_seconds: Option<u64>,
    pub last_modified: Option<String>,
    pub retry_after_raw: Option<String>,
    pub retry_after: RetryAfterHeader,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CatalogTransportError {
    #[error("Catalog request timed out")]
    Timeout,
    #[error("Catalog request encountered a network failure")]
    Network,
    #[error("Catalog request could not be constructed")]
    RequestConstruction,
    #[error("Catalog response URL differed from the requested allowlisted origin")]
    ResponseUrlMismatch,
    #[error("Catalog upstream returned a redirect that was not followed")]
    Redirect { status: u16, scope: RedirectScope },
    #[error("Catalog upstream returned non-success HTTP status {status}")]
    NonSuccessHttp { status: u16 },
    #[error("Catalog response Content-Type was not application/json")]
    InvalidContentType,
    #[error("Catalog decoded response exceeded the 64 MiB product limit")]
    ResponseTooLarge,
    #[error("Catalog response content encoding could not be decoded")]
    ContentDecoding,
    #[error("Catalog response was not valid typed JSON")]
    InvalidJson,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{kind}")]
pub struct CatalogFailure {
    #[source]
    kind: CatalogTransportError,
    response_metadata: Option<Box<CatalogFailureResponseMetadata>>,
    error_chain: Option<String>,
}

impl CatalogFailure {
    fn before_response(kind: CatalogTransportError) -> Self {
        Self {
            kind,
            response_metadata: None,
            error_chain: None,
        }
    }

    fn from_response(
        kind: CatalogTransportError,
        response_metadata: CatalogFailureResponseMetadata,
    ) -> Self {
        Self {
            kind,
            response_metadata: Some(Box::new(response_metadata)),
            error_chain: None,
        }
    }

    fn before_response_with_chain(kind: CatalogTransportError, error_chain: String) -> Self {
        Self {
            kind,
            response_metadata: None,
            error_chain: Some(error_chain),
        }
    }

    fn from_response_with_chain(
        kind: CatalogTransportError,
        response_metadata: CatalogFailureResponseMetadata,
        error_chain: String,
    ) -> Self {
        Self {
            kind,
            response_metadata: Some(Box::new(response_metadata)),
            error_chain: Some(error_chain),
        }
    }

    pub const fn kind(&self) -> &CatalogTransportError {
        &self.kind
    }

    pub fn response_metadata(&self) -> Option<&CatalogFailureResponseMetadata> {
        self.response_metadata.as_deref()
    }

    pub fn error_chain(&self) -> Option<&str> {
        self.error_chain.as_deref()
    }

    pub fn into_parts(
        self,
    ) -> (
        CatalogTransportError,
        Option<CatalogFailureResponseMetadata>,
    ) {
        (self.kind, self.response_metadata.map(|metadata| *metadata))
    }
}

#[derive(Clone, Copy)]
struct TransportLimits {
    connect_timeout: Duration,
    response_header_timeout: Duration,
    body_idle_timeout: Duration,
    total_timeout: Duration,
    max_decoded_bytes: usize,
}

impl TransportLimits {
    const OFFICIAL: Self = Self {
        connect_timeout: CATALOG_CONNECT_TIMEOUT,
        response_header_timeout: CATALOG_RESPONSE_HEADER_TIMEOUT,
        body_idle_timeout: CATALOG_BODY_IDLE_TIMEOUT,
        total_timeout: CATALOG_TOTAL_TIMEOUT,
        max_decoded_bytes: CATALOG_MAX_DECODED_BYTES,
    };
}

#[derive(Clone, Copy)]
enum EndpointMode {
    Official,
    #[cfg(test)]
    TestLoopback,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("Rutgers Catalog client configuration is invalid")]
pub struct CatalogClientBuildError;

pub struct RutgersCatalogClient {
    client: Client,
    endpoint: Url,
    response_header_timeout: Duration,
    body_idle_timeout: Duration,
    max_decoded_bytes: usize,
}

impl RutgersCatalogClient {
    /// Build the production client. No production API accepts an alternate
    /// endpoint; loopback injection exists only inside this module's tests.
    pub fn new_official() -> Result<Self, CatalogClientBuildError> {
        let endpoint = Url::parse(RUTGERS_CATALOG_ENDPOINT).map_err(|_| CatalogClientBuildError)?;
        Self::build(endpoint, TransportLimits::OFFICIAL, EndpointMode::Official)
    }

    fn build(
        endpoint: Url,
        limits: TransportLimits,
        mode: EndpointMode,
    ) -> Result<Self, CatalogClientBuildError> {
        if endpoint.query().is_some()
            || endpoint.fragment().is_some()
            || endpoint.cannot_be_a_base()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.path() != "/soc/api/courses.json"
            || !valid_endpoint(&endpoint, mode)
            || limits.connect_timeout.is_zero()
            || limits.response_header_timeout.is_zero()
            || limits.body_idle_timeout.is_zero()
            || limits.total_timeout.is_zero()
            || limits.max_decoded_bytes == 0
        {
            return Err(CatalogClientBuildError);
        }
        let https_only = matches!(mode, EndpointMode::Official);
        let client = Client::builder()
            .redirect(Policy::none())
            .retry(reqwest::retry::never())
            .referer(false)
            .https_only(https_only)
            .connect_timeout(limits.connect_timeout)
            .timeout(limits.total_timeout)
            .gzip(true)
            .user_agent(format!("RBCSP/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| CatalogClientBuildError)?;
        Ok(Self {
            client,
            endpoint,
            response_header_timeout: limits.response_header_timeout,
            body_idle_timeout: limits.body_idle_timeout,
            max_decoded_bytes: limits.max_decoded_bytes,
        })
    }

    pub fn request_url(&self, request: &CatalogRequest) -> Url {
        let mut url = self.endpoint.clone();
        url.query_pairs_mut()
            .append_pair("year", &request.year.to_string())
            .append_pair("term", &request.term_code.to_string())
            .append_pair("campus", request.target.campus().as_str());
        url
    }

    fn build_request(&self, request: &CatalogRequest) -> Result<Request, CatalogFailure> {
        self.client
            .get(self.request_url(request))
            .header(ACCEPT, "application/json")
            .build()
            .map_err(|error| {
                CatalogFailure::before_response_with_chain(
                    CatalogTransportError::RequestConstruction,
                    reqwest_error_chain(error),
                )
            })
    }

    pub async fn fetch(
        &self,
        request: &CatalogRequest,
        request_id: &str,
        observed_at_utc: &str,
    ) -> Result<CatalogResponse, CatalogFailure> {
        let outbound = self.build_request(request)?;
        debug_assert_eq!(outbound.method(), Method::GET);
        let requested_url = outbound.url().clone();
        let response =
            tokio::time::timeout(self.response_header_timeout, self.client.execute(outbound))
                .await
                .map_err(|_| CatalogFailure::before_response(CatalogTransportError::Timeout))?
                .map_err(|error| {
                    let kind = classify_reqwest_error(&error);
                    CatalogFailure::before_response_with_chain(kind, reqwest_error_chain(error))
                })?;
        if !same_origin(response.url(), &requested_url) {
            let metadata = CatalogFailureResponseMetadata::capture(&response);
            return Err(CatalogFailure::from_response(
                CatalogTransportError::ResponseUrlMismatch,
                metadata,
            ));
        }
        self.decode_response(
            request.target.clone(),
            request_id,
            observed_at_utc,
            response,
        )
        .await
    }

    async fn decode_response(
        &self,
        target: TermCampusKey,
        request_id: &str,
        observed_at_utc: &str,
        mut response: Response,
    ) -> Result<CatalogResponse, CatalogFailure> {
        let status = response.status();
        let mut failure_metadata = CatalogFailureResponseMetadata::capture(&response);
        if !status.is_success() {
            return Err(CatalogFailure::from_response(
                classify_http_failure(&response),
                failure_metadata,
            ));
        }
        let Some(content_type) = json_content_type(response.headers()) else {
            return Err(CatalogFailure::from_response(
                CatalogTransportError::InvalidContentType,
                failure_metadata,
            ));
        };
        if failure_metadata
            .content_length
            .is_some_and(|length| length > self.max_decoded_bytes as u64)
        {
            return Err(CatalogFailure::from_response(
                CatalogTransportError::ResponseTooLarge,
                failure_metadata,
            ));
        }

        let headers = ResponseHeaders::capture(response.headers());
        let mut body = Vec::new();
        loop {
            let chunk = match tokio::time::timeout(self.body_idle_timeout, response.chunk()).await {
                Ok(Ok(Some(chunk))) => chunk,
                Ok(Ok(None)) => break,
                Ok(Err(error)) => {
                    failure_metadata.decoded_bytes = Some(body.len());
                    let kind = classify_reqwest_error(&error);
                    return Err(CatalogFailure::from_response_with_chain(
                        kind,
                        failure_metadata,
                        reqwest_error_chain(error),
                    ));
                }
                Err(_) => {
                    failure_metadata.decoded_bytes = Some(body.len());
                    return Err(CatalogFailure::from_response(
                        CatalogTransportError::Timeout,
                        failure_metadata,
                    ));
                }
            };
            let Some(next_len) = body.len().checked_add(chunk.len()) else {
                failure_metadata.decoded_bytes = Some(body.len());
                return Err(CatalogFailure::from_response(
                    CatalogTransportError::ResponseTooLarge,
                    failure_metadata,
                ));
            };
            if next_len > self.max_decoded_bytes {
                failure_metadata.decoded_bytes = Some(next_len);
                return Err(CatalogFailure::from_response(
                    CatalogTransportError::ResponseTooLarge,
                    failure_metadata,
                ));
            }
            body.extend_from_slice(&chunk);
        }

        let provenance = SourceProvenance::from_body(request_id, observed_at_utc, &body);
        failure_metadata.decoded_bytes = Some(body.len());
        failure_metadata.decoded_body_sha256 = Some(provenance.body_sha256.clone());
        let payload = decode_catalog_payload(&body).map_err(|error| {
            CatalogFailure::from_response_with_chain(
                CatalogTransportError::InvalidJson,
                failure_metadata.clone(),
                format!("Catalog JSON decode: {error}"),
            )
        })?;
        let metadata = CatalogResponseMetadata {
            http_status: status.as_u16(),
            decoded_bytes: body.len(),
            decoded_body_sha256: provenance.body_sha256.clone(),
            course_count: payload.len(),
            content_type,
            content_encoding: headers.content_encoding,
            content_length: headers.content_length,
            etag: headers.etag,
            cache_control: headers.cache_control,
            date: headers.date,
            age_seconds: headers.age_seconds,
            last_modified: headers.last_modified,
        };
        Ok(CatalogResponse {
            target,
            payload,
            provenance,
            metadata,
        })
    }
}

fn valid_endpoint(endpoint: &Url, mode: EndpointMode) -> bool {
    match mode {
        EndpointMode::Official => endpoint.as_str() == RUTGERS_CATALOG_ENDPOINT,
        #[cfg(test)]
        EndpointMode::TestLoopback => {
            endpoint.scheme() == "http"
                && endpoint.port().is_some()
                && endpoint
                    .host_str()
                    .and_then(|host| host.parse::<IpAddr>().ok())
                    .is_some_and(|address| address.is_loopback())
        }
    }
}

struct ResponseHeaders {
    content_encoding: Option<String>,
    content_length: Option<u64>,
    etag: Option<String>,
    cache_control: Option<String>,
    date: Option<String>,
    age_seconds: Option<u64>,
    last_modified: Option<String>,
}

impl ResponseHeaders {
    fn capture(headers: &HeaderMap) -> Self {
        Self {
            content_encoding: header_text(headers, CONTENT_ENCODING),
            content_length: content_length(headers),
            etag: header_text(headers, ETAG),
            cache_control: header_text(headers, CACHE_CONTROL),
            date: header_text(headers, DATE),
            age_seconds: header_text(headers, AGE).and_then(|value| value.parse().ok()),
            last_modified: header_text(headers, LAST_MODIFIED),
        }
    }
}

impl CatalogFailureResponseMetadata {
    fn capture(response: &Response) -> Self {
        let headers = response.headers();
        Self {
            http_status: response.status().as_u16(),
            decoded_bytes: None,
            decoded_body_sha256: None,
            content_type: header_text(headers, CONTENT_TYPE),
            content_encoding: header_text(headers, CONTENT_ENCODING),
            content_length: content_length(headers),
            etag: header_text(headers, ETAG),
            cache_control: header_text(headers, CACHE_CONTROL),
            date: header_text(headers, DATE),
            age_seconds: header_text(headers, AGE).and_then(|value| value.parse().ok()),
            last_modified: header_text(headers, LAST_MODIFIED),
            retry_after_raw: header_text(headers, RETRY_AFTER),
            retry_after: retry_after(headers),
        }
    }
}

fn classify_reqwest_error(error: &reqwest::Error) -> CatalogTransportError {
    if error.is_timeout() {
        CatalogTransportError::Timeout
    } else if error.is_decode() {
        CatalogTransportError::ContentDecoding
    } else {
        CatalogTransportError::Network
    }
}

fn reqwest_error_chain(error: reqwest::Error) -> String {
    use std::error::Error as _;

    let error = error.without_url();
    let mut chain = vec![error.to_string()];
    let mut source = error.source();
    while let Some(current) = source {
        chain.push(current.to_string());
        source = current.source();
    }
    chain.join(": ")
}

fn classify_http_failure(response: &Response) -> CatalogTransportError {
    let status = response.status();
    if status.is_redirection() && status != StatusCode::NOT_MODIFIED {
        return CatalogTransportError::Redirect {
            status: status.as_u16(),
            scope: redirect_scope(response),
        };
    }
    CatalogTransportError::NonSuccessHttp {
        status: status.as_u16(),
    }
}

fn json_content_type(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(CONTENT_TYPE)?.to_str().ok()?;
    let media_type = value.split(';').next()?.trim();
    media_type
        .eq_ignore_ascii_case("application/json")
        .then(|| value.to_owned())
}

fn content_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
}

fn header_text(headers: &HeaderMap, name: reqwest::header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn redirect_scope(response: &Response) -> RedirectScope {
    let Some(location) = response
        .headers()
        .get(LOCATION)
        .and_then(|value| value.to_str().ok())
    else {
        return RedirectScope::InvalidLocation;
    };
    let Ok(destination) = response.url().join(location) else {
        return RedirectScope::InvalidLocation;
    };
    if same_origin(response.url(), &destination) {
        RedirectScope::SameOrigin
    } else {
        RedirectScope::OffOrigin
    }
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc::{Receiver, channel};
    use std::thread::{self, JoinHandle};

    use super::*;
    use crate::sha256_hex;

    const SYNTHETIC_BODY: &[u8] = br#"[{"courseString":"SYN:CAT:001","sections":[]}]"#;
    const SYNTHETIC_GZIP: &[u8] = &[
        31, 139, 8, 0, 0, 0, 0, 0, 2, 255, 139, 174, 86, 74, 206, 47, 45, 42, 78, 13, 46, 41, 202,
        204, 75, 87, 178, 82, 10, 142, 244, 179, 114, 118, 12, 177, 50, 48, 48, 84, 210, 81, 42,
        78, 77, 46, 201, 204, 207, 43, 86, 178, 138, 142, 173, 141, 5, 0, 6, 128, 42, 63, 46, 0, 0,
        0,
    ];
    const EXPANDED_GZIP: &[u8] = &[
        31, 139, 8, 0, 0, 0, 0, 0, 2, 255, 139, 174, 86, 74, 206, 47, 45, 42, 78, 13, 46, 41, 202,
        204, 75, 87, 178, 82, 10, 142, 244, 179, 114, 118, 12, 177, 50, 48, 48, 84, 210, 81, 42,
        201, 44, 201, 73, 5, 138, 58, 142, 130, 17, 13, 128, 73, 161, 56, 53, 185, 36, 51, 63, 175,
        88, 201, 42, 58, 182, 54, 22, 0, 243, 110, 9, 189, 57, 2, 0, 0,
    ];

    struct FakeResponse {
        status: &'static str,
        headers: Vec<(&'static str, String)>,
        body: Vec<u8>,
    }

    impl FakeResponse {
        fn json(body: Vec<u8>) -> Self {
            Self {
                status: "200 OK",
                headers: vec![("Content-Type", "application/json".to_owned())],
                body,
            }
        }

        fn status(status: &'static str) -> Self {
            Self {
                status,
                headers: Vec::new(),
                body: Vec::new(),
            }
        }
    }

    fn fake_upstream(response: FakeResponse) -> (Url, Receiver<String>, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake Catalog upstream");
        let address = listener.local_addr().expect("fake upstream address");
        let (sender, receiver) = channel();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept one Catalog request");
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1_024];
            while request.windows(4).all(|window| window != b"\r\n\r\n") {
                let read = stream.read(&mut chunk).expect("read fake request");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                assert!(request.len() <= 64 * 1024, "request headers too large");
            }
            sender
                .send(String::from_utf8(request).expect("ASCII request"))
                .expect("capture fake request");
            let mut head = format!("HTTP/1.1 {}\r\n", response.status);
            for (name, value) in &response.headers {
                head.push_str(name);
                head.push_str(": ");
                head.push_str(value);
                head.push_str("\r\n");
            }
            if !response
                .headers
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case("Content-Length"))
            {
                head.push_str(&format!("Content-Length: {}\r\n", response.body.len()));
            }
            head.push_str("Connection: close\r\n\r\n");
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&response.body);
        });
        let endpoint =
            Url::parse(&format!("http://{address}/soc/api/courses.json")).expect("fake endpoint");
        (endpoint, receiver, handle)
    }

    fn delayed_fake_upstream(
        response: FakeResponse,
        header_delay: Duration,
        body_delay: Duration,
    ) -> (Url, Receiver<String>, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind delayed Catalog upstream");
        let address = listener.local_addr().expect("delayed upstream address");
        let (sender, receiver) = channel();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept delayed request");
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1_024];
            while request.windows(4).all(|window| window != b"\r\n\r\n") {
                let read = stream.read(&mut chunk).expect("read delayed request");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
            }
            sender
                .send(String::from_utf8(request).expect("ASCII request"))
                .expect("capture delayed request");
            thread::sleep(header_delay);
            let mut head = format!("HTTP/1.1 {}\r\n", response.status);
            for (name, value) in &response.headers {
                head.push_str(name);
                head.push_str(": ");
                head.push_str(value);
                head.push_str("\r\n");
            }
            if !response
                .headers
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case("Content-Length"))
            {
                head.push_str(&format!("Content-Length: {}\r\n", response.body.len()));
            }
            head.push_str("Connection: close\r\n\r\n");
            if stream.write_all(head.as_bytes()).is_err() {
                return;
            }
            let _ = stream.flush();
            thread::sleep(body_delay);
            let _ = stream.write_all(&response.body);
        });
        let endpoint =
            Url::parse(&format!("http://{address}/soc/api/courses.json")).expect("fake endpoint");
        (endpoint, receiver, handle)
    }

    fn request(term: &str, campus: &str) -> CatalogRequest {
        CatalogRequest::try_from_target(
            TermCampusKey::try_new(term, campus).expect("synthetic target"),
        )
        .expect("transport-shaped target")
    }

    fn test_client(endpoint: Url, max_decoded_bytes: usize) -> RutgersCatalogClient {
        RutgersCatalogClient::build(
            endpoint,
            TransportLimits {
                max_decoded_bytes,
                ..TransportLimits::OFFICIAL
            },
            EndpointMode::TestLoopback,
        )
        .expect("loopback test client")
    }

    async fn fetch_fake(response: FakeResponse) -> CatalogFailure {
        let (endpoint, capture, handle) = fake_upstream(response);
        let error = test_client(endpoint, CATALOG_MAX_DECODED_BYTES)
            .fetch(
                &request("92026", "NB"),
                "SYNTHETIC_REQUEST",
                "2030-01-01T00:00:00Z",
            )
            .await
            .expect_err("fake response must fail");
        capture.recv().expect("captured request");
        handle.join().expect("fake upstream thread");
        error
    }

    fn timeout_test_client(endpoint: Url) -> RutgersCatalogClient {
        RutgersCatalogClient::build(
            endpoint,
            TransportLimits {
                connect_timeout: Duration::from_secs(1),
                response_header_timeout: Duration::from_millis(40),
                body_idle_timeout: Duration::from_millis(40),
                total_timeout: Duration::from_secs(2),
                max_decoded_bytes: CATALOG_MAX_DECODED_BYTES,
            },
            EndpointMode::TestLoopback,
        )
        .expect("timeout test client")
    }

    #[test]
    fn target_identity_is_split_strictly_and_official_url_is_fixed() {
        let fall_request = request("92026", "ONLINE_NB");
        assert_eq!(fall_request.year(), 2026);
        assert_eq!(fall_request.term_code(), 9);
        assert_eq!(fall_request.target().term().as_str(), "92026");
        let client = RutgersCatalogClient::new_official().expect("fixed official client");
        assert_eq!(
            client.request_url(&fall_request).as_str(),
            "https://classes.rutgers.edu/soc/api/courses.json?year=2026&term=9&campus=ONLINE_NB"
        );
        assert_eq!(CATALOG_CONNECT_TIMEOUT, Duration::from_secs(10));
        assert_eq!(CATALOG_RESPONSE_HEADER_TIMEOUT, Duration::from_secs(30));
        assert_eq!(CATALOG_BODY_IDLE_TIMEOUT, Duration::from_secs(30));
        assert_eq!(CATALOG_TOTAL_TIMEOUT, Duration::from_secs(300));
        assert_eq!(CATALOG_MAX_DECODED_BYTES, 67_108_864);

        let winter = request("02027", "NB");
        assert_eq!(winter.year(), 2027);
        assert_eq!(winter.term_code(), 0);

        for invalid in ["T2026", "22026", "9202", "992026", "9_026"] {
            let target = TermCampusKey::try_new(invalid, "NB").expect("contract-valid term");
            assert_eq!(
                CatalogRequest::try_from_target(target),
                Err(CatalogRequestError::InvalidTermIdentity)
            );
        }
    }

    #[test]
    fn endpoint_injection_is_limited_to_loopback_tests() {
        let remote = Url::parse("http://192.0.2.1:8080/soc/api/courses.json").unwrap();
        assert!(
            RutgersCatalogClient::build(
                remote,
                TransportLimits::OFFICIAL,
                EndpointMode::TestLoopback
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn gzip_fetch_uses_exact_get_without_credentials_and_returns_normalization_parts() {
        let mut response = FakeResponse::json(SYNTHETIC_GZIP.to_vec());
        response
            .headers
            .push(("Content-Encoding", "gzip".to_owned()));
        response.headers.extend([
            ("ETag", "\"synthetic-etag\"".to_owned()),
            ("Cache-Control", "max-age=900".to_owned()),
            ("Date", "Sun, 06 Nov 1994 08:49:37 GMT".to_owned()),
            ("Age", "7".to_owned()),
        ]);
        let (endpoint, capture, handle) = fake_upstream(response);
        let request = request("92026", "NB");
        let result = test_client(endpoint, CATALOG_MAX_DECODED_BYTES)
            .fetch(&request, "SYNTHETIC_REQUEST", "2030-01-01T00:00:00Z")
            .await
            .expect("gzip Catalog response");
        let captured = capture.recv().expect("captured request");
        handle.join().expect("fake upstream thread");

        let lower = captured.to_ascii_lowercase();
        assert!(
            captured
                .starts_with("GET /soc/api/courses.json?year=2026&term=9&campus=NB HTTP/1.1\r\n")
        );
        assert!(lower.contains("accept: application/json\r\n"));
        assert!(lower.contains("accept-encoding: gzip\r\n"));
        assert!(!lower.contains("authorization:"));
        assert!(!lower.contains("proxy-authorization:"));
        assert!(!lower.contains("cookie:"));

        assert_eq!(result.target(), request.target());
        assert_eq!(result.payload().len(), 1);
        assert_eq!(result.metadata().http_status, 200);
        assert_eq!(result.metadata().decoded_bytes, SYNTHETIC_BODY.len());
        assert_eq!(result.metadata().course_count, 1);
        assert_eq!(result.metadata().age_seconds, Some(7));
        assert_eq!(
            result.metadata().decoded_body_sha256,
            sha256_hex(SYNTHETIC_BODY)
        );
        assert_eq!(result.provenance().request_id, "SYNTHETIC_REQUEST");
        assert_eq!(result.provenance().observed_at_utc, "2030-01-01T00:00:00Z");
        assert_eq!(
            result.provenance().body_sha256,
            result.metadata().decoded_body_sha256
        );

        let (target, payload, provenance) = result.into_normalization_parts();
        assert_eq!(target, request.target().clone());
        assert_eq!(payload.len(), 1);
        assert_eq!(provenance.decoded_bytes, SYNTHETIC_BODY.len());
    }

    #[tokio::test]
    async fn header_and_body_idle_timeouts_are_transport_timeouts() {
        let (endpoint, capture, handle) = delayed_fake_upstream(
            FakeResponse::json(SYNTHETIC_BODY.to_vec()),
            Duration::from_millis(120),
            Duration::ZERO,
        );
        let header_failure = timeout_test_client(endpoint)
            .fetch(
                &request("92026", "NB"),
                "SYNTHETIC_HEADER_TIMEOUT",
                "2030-01-01T00:00:00Z",
            )
            .await
            .expect_err("delayed headers must time out");
        capture.recv().expect("captured header-timeout request");
        handle.join().expect("header-timeout upstream thread");
        assert_eq!(header_failure.kind(), &CatalogTransportError::Timeout);
        assert!(header_failure.response_metadata().is_none());

        let (endpoint, capture, handle) = delayed_fake_upstream(
            FakeResponse::json(SYNTHETIC_BODY.to_vec()),
            Duration::ZERO,
            Duration::from_millis(120),
        );
        let body_failure = timeout_test_client(endpoint)
            .fetch(
                &request("92026", "NB"),
                "SYNTHETIC_BODY_TIMEOUT",
                "2030-01-01T00:00:00Z",
            )
            .await
            .expect_err("idle body must time out");
        capture.recv().expect("captured body-timeout request");
        handle.join().expect("body-timeout upstream thread");
        assert_eq!(body_failure.kind(), &CatalogTransportError::Timeout);
        assert_eq!(
            body_failure
                .response_metadata()
                .and_then(|metadata| metadata.decoded_bytes),
            Some(0)
        );
    }

    #[tokio::test]
    async fn decoded_limit_applies_after_gzip_expansion() {
        let mut response = FakeResponse::json(EXPANDED_GZIP.to_vec());
        response
            .headers
            .push(("Content-Encoding", "gzip".to_owned()));
        assert!(EXPANDED_GZIP.len() < 256);
        let (endpoint, capture, handle) = fake_upstream(response);
        let error = test_client(endpoint, 256)
            .fetch(
                &request("92026", "NB"),
                "SYNTHETIC_REQUEST",
                "2030-01-01T00:00:00Z",
            )
            .await
            .expect_err("decoded body exceeds the limit");
        capture.recv().expect("captured request");
        handle.join().expect("fake upstream thread");
        assert_eq!(error.kind(), &CatalogTransportError::ResponseTooLarge);
        assert!(
            error
                .response_metadata()
                .and_then(|metadata| metadata.decoded_bytes)
                .is_some_and(|decoded| decoded > 256)
        );
    }

    #[tokio::test]
    async fn status_redirect_content_type_and_json_fail_closed() {
        let unavailable = fetch_fake(FakeResponse::status("503 Service Unavailable")).await;
        assert_eq!(
            unavailable.kind(),
            &CatalogTransportError::NonSuccessHttp { status: 503 }
        );
        assert_eq!(
            unavailable
                .response_metadata()
                .expect("HTTP metadata")
                .decoded_bytes,
            None
        );

        let mut rate_limited = FakeResponse::status("429 Too Many Requests");
        rate_limited.headers.push(("Retry-After", "120".to_owned()));
        let rate_limited = fetch_fake(rate_limited).await;
        let metadata = rate_limited.response_metadata().expect("429 metadata");
        assert_eq!(metadata.retry_after_raw.as_deref(), Some("120"));
        assert_eq!(
            metadata.retry_after,
            RetryAfterHeader::Valid(RetryAfterValue::DelaySeconds(120))
        );

        let mut redirect = FakeResponse::status("302 Found");
        redirect
            .headers
            .push(("Location", "https://example.invalid/courses".to_owned()));
        assert_eq!(
            fetch_fake(redirect).await.kind(),
            &CatalogTransportError::Redirect {
                status: 302,
                scope: RedirectScope::OffOrigin
            }
        );

        let mut html = FakeResponse::json(b"[]".to_vec());
        html.headers[0].1 = "text/html".to_owned();
        assert_eq!(
            fetch_fake(html).await.kind(),
            &CatalogTransportError::InvalidContentType
        );

        let invalid = fetch_fake(FakeResponse::json(b"not-json".to_vec())).await;
        assert_eq!(invalid.kind(), &CatalogTransportError::InvalidJson);
        let metadata = invalid.response_metadata().expect("decode metadata");
        assert_eq!(metadata.decoded_bytes, Some(8));
        assert_eq!(
            metadata.decoded_body_sha256.as_deref(),
            Some(sha256_hex(b"not-json").as_str())
        );
        assert!(!invalid.to_string().contains("not-json"));
        let chain = invalid.error_chain().expect("decode chain");
        assert!(chain.contains("Catalog JSON decode"));
        assert!(!chain.contains("not-json"));
        assert!(!chain.contains("courses.json"));
    }

    #[tokio::test]
    async fn payload_is_owned_by_the_requested_target_not_upstream_fields() {
        let body = br#"[{"courseString":"SYN:CAT:002","campusCode":"NWK","sections":[]}]"#;
        let (endpoint, capture, handle) = fake_upstream(FakeResponse::json(body.to_vec()));
        let request = request("72026", "NB");
        let result = test_client(endpoint, CATALOG_MAX_DECODED_BYTES)
            .fetch(&request, "SYNTHETIC_OWNERSHIP", "2030-01-01T00:00:00Z")
            .await
            .expect("target-bound response");
        let captured = capture.recv().expect("captured request");
        handle.join().expect("fake upstream thread");
        assert!(
            captured
                .starts_with("GET /soc/api/courses.json?year=2026&term=7&campus=NB HTTP/1.1\r\n")
        );
        assert_eq!(result.target(), request.target());
        assert_eq!(
            result.payload()[0].campus_code.value().map(String::as_str),
            Some("NWK")
        );
    }
}
