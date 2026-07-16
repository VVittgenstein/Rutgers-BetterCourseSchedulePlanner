use std::collections::BTreeSet;
use std::time::Duration;

use bcsp_contracts::{OpenBatchKey, OpenCanonicalSetHash, SectionIndex, TermCampusKey, TermId};
use reqwest::header::{
    ACCEPT, AGE, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, DATE, ETAG, HeaderMap, LAST_MODIFIED,
    LOCATION, RETRY_AFTER,
};
use reqwest::{Client, Method, Request, Response, StatusCode, Url, redirect::Policy};
use serde_json::Value;
use thiserror::Error;

use crate::hashing::sha256_hex;
use crate::{DiscoveredTarget, DiscoveredTerm, DiscoverySourceId};

pub const RUTGERS_OPEN_SECTIONS_ENDPOINT: &str =
    "https://classes.rutgers.edu/soc/api/openSections.json";
pub const OPEN_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const OPEN_TOTAL_TIMEOUT: Duration = Duration::from_secs(15);
pub const OPEN_MAX_DECODED_BYTES: usize = 10 * 1024 * 1024;
pub const OPEN_SET_HASH_DOMAIN: &[u8] = b"bcsp-open-set-v1\n";

/// Discovery-bound source coordinates for one `(term, campus)` Open batch.
///
/// The only public constructor requires a discovered term and target from the
/// same immutable discovery source and verifies their term identities match.
/// This prevents a caller from pairing one target with another term's Rutgers
/// `year`/`termCode`, while still treating [`OpenBatchKey::term`] as opaque.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenSectionsRequest {
    discovery_source_id: DiscoverySourceId,
    target: OpenBatchKey,
    year: u16,
    term_code: u8,
}

impl OpenSectionsRequest {
    pub fn try_from_discovery(
        term: &DiscoveredTerm,
        target: &DiscoveredTarget,
    ) -> Result<Self, OpenSectionsRequestError> {
        if term.source_id != target.source_id {
            return Err(OpenSectionsRequestError::SourceMismatch);
        }
        if &term.term_id != target.key.term() {
            return Err(OpenSectionsRequestError::TermMismatch);
        }
        let Some(year) = term.year.value().copied() else {
            return Err(OpenSectionsRequestError::MissingYear);
        };
        let year = u16::try_from(year).map_err(|_| OpenSectionsRequestError::InvalidYear)?;
        if !(1_000..=9_999).contains(&year) {
            return Err(OpenSectionsRequestError::InvalidYear);
        }
        let Some(term_code) = term.term_code.value() else {
            return Err(OpenSectionsRequestError::MissingTermCode);
        };
        if term_code.is_empty()
            || term_code.len() > 2
            || !term_code.bytes().all(|byte| byte.is_ascii_digit())
            || (term_code.len() > 1 && term_code.starts_with('0'))
        {
            return Err(OpenSectionsRequestError::InvalidTermCode);
        }
        let term_code = term_code
            .parse::<u8>()
            .map_err(|_| OpenSectionsRequestError::InvalidTermCode)?;
        Ok(Self {
            discovery_source_id: term.source_id.clone(),
            target: OpenBatchKey::new(term.term_id.clone(), target.key.campus().clone()),
            year,
            term_code,
        })
    }

    /// Reconstructs one Open request from a validated, published discovery snapshot.
    ///
    /// Callers must first prove that the persisted term and target are owned by the same source
    /// version. This constructor independently revalidates the immutable source identity and the
    /// complete Rutgers term coordinates before allowing them back onto the wire.
    pub fn try_from_persisted_discovery(
        discovery_source_id: &str,
        term_id: &TermId,
        target: &TermCampusKey,
        year: Option<u16>,
        term_code: Option<&str>,
    ) -> Result<Self, OpenSectionsRequestError> {
        if term_id != target.term() {
            return Err(OpenSectionsRequestError::TermMismatch);
        }
        let discovery_source_id = DiscoverySourceId::from_persisted(discovery_source_id)
            .ok_or(OpenSectionsRequestError::InvalidDiscoverySourceId)?;
        let year = year.ok_or(OpenSectionsRequestError::MissingYear)?;
        if !(1_000..=9_999).contains(&year) {
            return Err(OpenSectionsRequestError::InvalidYear);
        }
        let term_code = term_code.ok_or(OpenSectionsRequestError::MissingTermCode)?;
        if term_code.is_empty()
            || term_code.len() > 2
            || !term_code.bytes().all(|byte| byte.is_ascii_digit())
            || (term_code.len() > 1 && term_code.starts_with('0'))
        {
            return Err(OpenSectionsRequestError::InvalidTermCode);
        }
        let term_code = term_code
            .parse::<u8>()
            .map_err(|_| OpenSectionsRequestError::InvalidTermCode)?;
        Ok(Self {
            discovery_source_id,
            target: OpenBatchKey::new(term_id.clone(), target.campus().clone()),
            year,
            term_code,
        })
    }

    pub const fn discovery_source_id(&self) -> &DiscoverySourceId {
        &self.discovery_source_id
    }

    pub const fn target(&self) -> &OpenBatchKey {
        &self.target
    }

    pub const fn year(&self) -> u16 {
        self.year
    }

    pub const fn term_code(&self) -> u8 {
        self.term_code
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum OpenSectionsRequestError {
    #[error("Open term and target must belong to the same immutable discovery source")]
    SourceMismatch,
    #[error("Open target must reference the exact discovered term")]
    TermMismatch,
    #[error("Open source year is unavailable in discovery")]
    MissingYear,
    #[error("Open source year must be a four-digit discovery value")]
    InvalidYear,
    #[error("Open source term code is unavailable in discovery")]
    MissingTermCode,
    #[error("Open source term code must be a canonical one- or two-digit value")]
    InvalidTermCode,
    #[error("Open source identity is not a canonical persisted discovery source version")]
    InvalidDiscoverySourceId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenPayloadClassification {
    Nonempty,
    Empty,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenResponseMetadata {
    pub http_status: u16,
    pub decoded_bytes: usize,
    pub decoded_body_sha256: String,
    pub canonical_set_sha256: OpenCanonicalSetHash,
    pub raw_value_count: usize,
    pub unique_value_count: usize,
    pub duplicate_value_count: usize,
    pub content_type: String,
    pub etag: Option<String>,
    pub cache_control: Option<String>,
    pub date: Option<String>,
    pub age_seconds: Option<u64>,
    pub last_modified: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenSectionsResponse {
    pub target: OpenBatchKey,
    pub classification: OpenPayloadClassification,
    pub open_indexes: Vec<SectionIndex>,
    pub metadata: OpenResponseMetadata,
}

/// Redacted response facts retained when a response cannot become an Open
/// observation. Header text is preserved only when it is valid HTTP text; no
/// response-body bytes or parser diagnostics are retained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenFailureResponseMetadata {
    pub http_status: u16,
    /// Decoded bytes observed before failure. This is the complete body length
    /// when [`decoded_body_sha256`](Self::decoded_body_sha256) is present.
    pub decoded_bytes: Option<usize>,
    /// SHA-256 of the complete decoded body, available only after EOF.
    pub decoded_body_sha256: Option<String>,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub cache_control: Option<String>,
    pub date: Option<String>,
    pub age: Option<String>,
    pub age_seconds: Option<u64>,
    pub last_modified: Option<String>,
    /// Exact text of a single valid-text `Retry-After` header. Duplicate or
    /// non-text values remain unavailable while [`retry_after`](Self::retry_after)
    /// records their fail-closed parsed classification.
    pub retry_after_raw: Option<String>,
    pub retry_after: RetryAfterHeader,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryAfterValue {
    DelaySeconds(u64),
    HttpDateUnixSeconds(i64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryAfterHeader {
    Missing,
    Invalid,
    Valid(RetryAfterValue),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedirectScope {
    SameOrigin,
    OffOrigin,
    InvalidLocation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenFailureDisposition {
    TransientBackoff,
    RateLimitedOriginGate,
    FatalProtocolOriginCircuit,
    LocalConfiguration,
}

/// Redacted failure categories. No variant contains response-body bytes or
/// upstream parser text, so routine logging cannot disclose a raw body.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum OpenSectionsError {
    #[error("Open request timed out")]
    Timeout,
    #[error("Open request encountered a network failure")]
    Network,
    #[error("Open request could not be constructed")]
    RequestConstruction,
    #[error("Open response URL differed from the requested allowlisted origin")]
    ResponseUrlMismatch,
    #[error("Open upstream returned transient HTTP status {status}")]
    TransientHttp { status: u16 },
    #[error("Open upstream rate limited the origin")]
    RateLimited { retry_after: RetryAfterHeader },
    #[error("Open upstream denied the request with HTTP 403")]
    Forbidden,
    #[error("Open upstream returned a redirect that was not followed")]
    Redirect { status: u16, scope: RedirectScope },
    #[error("Open upstream returned fail-closed client HTTP status {status}")]
    FatalClientHttp { status: u16 },
    #[error("Open upstream returned unsupported HTTP status {status}")]
    UnsupportedHttp { status: u16 },
    #[error("Open response Content-Type was not application/json")]
    InvalidContentType,
    #[error("Open decoded response exceeded the 10 MiB product limit")]
    ResponseTooLarge,
    #[error("Open response content encoding could not be decoded")]
    ContentDecoding,
    #[error("Open response was not valid JSON")]
    InvalidJson,
    #[error("Open response JSON root was not an array")]
    RootNotArray,
    #[error("Open response array value at position {position} was not a string")]
    NonStringValue { position: usize },
    #[error("Open response array value at position {position} was not a five-digit SectionIndex")]
    InvalidSectionIndex { position: usize },
}

/// One redacted Open failure plus any response metadata known at the failure
/// boundary. Pre-response failures have no response metadata.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{kind}")]
pub struct OpenSectionsFailure {
    #[source]
    kind: OpenSectionsError,
    response_metadata: Option<Box<OpenFailureResponseMetadata>>,
}

impl OpenSectionsFailure {
    fn before_response(kind: OpenSectionsError) -> Self {
        Self {
            kind,
            response_metadata: None,
        }
    }

    fn from_response(
        kind: OpenSectionsError,
        response_metadata: OpenFailureResponseMetadata,
    ) -> Self {
        Self {
            kind,
            response_metadata: Some(Box::new(response_metadata)),
        }
    }

    pub const fn kind(&self) -> &OpenSectionsError {
        &self.kind
    }

    pub fn response_metadata(&self) -> Option<&OpenFailureResponseMetadata> {
        match &self.response_metadata {
            Some(metadata) => Some(metadata.as_ref()),
            None => None,
        }
    }

    pub fn into_parts(self) -> (OpenSectionsError, Option<OpenFailureResponseMetadata>) {
        (self.kind, self.response_metadata.map(|metadata| *metadata))
    }

    pub const fn disposition(&self) -> OpenFailureDisposition {
        self.kind.disposition()
    }
}

/// Creates a redacted failure without response metadata. This conversion is
/// intended for pre-response transport failures and deterministic test fakes;
/// [`RutgersOpenSectionsClient::fetch`] attaches response metadata itself as
/// soon as an HTTP response exists.
impl From<OpenSectionsError> for OpenSectionsFailure {
    fn from(kind: OpenSectionsError) -> Self {
        Self::before_response(kind)
    }
}

impl OpenSectionsError {
    pub const fn disposition(&self) -> OpenFailureDisposition {
        match self {
            Self::Timeout | Self::Network | Self::TransientHttp { .. } => {
                OpenFailureDisposition::TransientBackoff
            }
            Self::RateLimited { .. } => OpenFailureDisposition::RateLimitedOriginGate,
            Self::RequestConstruction => OpenFailureDisposition::LocalConfiguration,
            Self::ResponseUrlMismatch
            | Self::Forbidden
            | Self::Redirect { .. }
            | Self::FatalClientHttp { .. }
            | Self::UnsupportedHttp { .. }
            | Self::InvalidContentType
            | Self::ResponseTooLarge
            | Self::ContentDecoding
            | Self::InvalidJson
            | Self::RootNotArray
            | Self::NonStringValue { .. }
            | Self::InvalidSectionIndex { .. } => {
                OpenFailureDisposition::FatalProtocolOriginCircuit
            }
        }
    }
}

#[derive(Clone, Copy)]
struct TransportLimits {
    connect_timeout: Duration,
    total_timeout: Duration,
    max_decoded_bytes: usize,
    https_only: bool,
}

impl TransportLimits {
    const OFFICIAL: Self = Self {
        connect_timeout: OPEN_CONNECT_TIMEOUT,
        total_timeout: OPEN_TOTAL_TIMEOUT,
        max_decoded_bytes: OPEN_MAX_DECODED_BYTES,
        https_only: true,
    };
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("Rutgers Open client configuration is invalid")]
pub struct OpenClientBuildError;

pub struct RutgersOpenSectionsClient {
    client: Client,
    endpoint: Url,
    max_decoded_bytes: usize,
}

impl RutgersOpenSectionsClient {
    pub fn new_official() -> Result<Self, OpenClientBuildError> {
        let endpoint =
            Url::parse(RUTGERS_OPEN_SECTIONS_ENDPOINT).map_err(|_| OpenClientBuildError)?;
        Self::build(endpoint, TransportLimits::OFFICIAL)
    }

    fn build(endpoint: Url, limits: TransportLimits) -> Result<Self, OpenClientBuildError> {
        if endpoint.query().is_some()
            || endpoint.fragment().is_some()
            || endpoint.cannot_be_a_base()
            || (limits.https_only && endpoint.scheme() != "https")
        {
            return Err(OpenClientBuildError);
        }
        let client = Client::builder()
            .redirect(Policy::none())
            .referer(false)
            .https_only(limits.https_only)
            .connect_timeout(limits.connect_timeout)
            .timeout(limits.total_timeout)
            .gzip(true)
            .user_agent(format!("RBCSP/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| OpenClientBuildError)?;
        Ok(Self {
            client,
            endpoint,
            max_decoded_bytes: limits.max_decoded_bytes,
        })
    }

    pub fn request_url(&self, request: &OpenSectionsRequest) -> Url {
        let mut url = self.endpoint.clone();
        url.query_pairs_mut()
            .append_pair("year", &request.year.to_string())
            .append_pair("term", &request.term_code.to_string())
            .append_pair("campus", request.target.campus().as_str());
        url
    }

    fn build_request(&self, request: &OpenSectionsRequest) -> Result<Request, OpenSectionsFailure> {
        self.client
            .get(self.request_url(request))
            .header(ACCEPT, "application/json")
            .build()
            .map_err(|_| {
                OpenSectionsFailure::before_response(OpenSectionsError::RequestConstruction)
            })
    }

    pub async fn fetch(
        &self,
        request: &OpenSectionsRequest,
    ) -> Result<OpenSectionsResponse, OpenSectionsFailure> {
        let outbound = self.build_request(request)?;
        debug_assert_eq!(outbound.method(), Method::GET);
        let requested_url = outbound.url().clone();
        let response =
            self.client.execute(outbound).await.map_err(|error| {
                OpenSectionsFailure::before_response(classify_reqwest_error(error))
            })?;
        if !same_origin(response.url(), &requested_url) {
            let metadata = OpenFailureResponseMetadata::capture(&response);
            return Err(OpenSectionsFailure::from_response(
                OpenSectionsError::ResponseUrlMismatch,
                metadata,
            ));
        }
        self.decode_response(request.target.clone(), response).await
    }

    async fn decode_response(
        &self,
        target: OpenBatchKey,
        mut response: Response,
    ) -> Result<OpenSectionsResponse, OpenSectionsFailure> {
        let status = response.status();
        let mut failure_metadata = OpenFailureResponseMetadata::capture(&response);
        if !status.is_success() {
            return Err(OpenSectionsFailure::from_response(
                classify_http_failure(&response),
                failure_metadata,
            ));
        }
        let Some(content_type) = json_content_type(response.headers()) else {
            return Err(OpenSectionsFailure::from_response(
                OpenSectionsError::InvalidContentType,
                failure_metadata,
            ));
        };
        if response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|length| length > self.max_decoded_bytes as u64)
        {
            return Err(OpenSectionsFailure::from_response(
                OpenSectionsError::ResponseTooLarge,
                failure_metadata,
            ));
        }

        let headers = ResponseHeaders::capture(response.headers());
        let mut body = Vec::new();
        loop {
            let chunk = match response.chunk().await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(error) => {
                    failure_metadata.decoded_bytes = Some(body.len());
                    return Err(OpenSectionsFailure::from_response(
                        classify_reqwest_error(error),
                        failure_metadata,
                    ));
                }
            };
            let Some(next_len) = body.len().checked_add(chunk.len()) else {
                failure_metadata.decoded_bytes = Some(body.len());
                return Err(OpenSectionsFailure::from_response(
                    OpenSectionsError::ResponseTooLarge,
                    failure_metadata,
                ));
            };
            if next_len > self.max_decoded_bytes {
                failure_metadata.decoded_bytes = Some(next_len);
                return Err(OpenSectionsFailure::from_response(
                    OpenSectionsError::ResponseTooLarge,
                    failure_metadata,
                ));
            }
            body.extend_from_slice(&chunk);
        }

        let decoded_body_sha256 = sha256_hex(&body);
        failure_metadata.decoded_bytes = Some(body.len());
        failure_metadata.decoded_body_sha256 = Some(decoded_body_sha256.clone());
        let (raw_value_count, duplicate_value_count, open_indexes) = match decode_open_array(&body)
        {
            Ok(decoded) => decoded,
            Err(error) => {
                return Err(OpenSectionsFailure::from_response(error, failure_metadata));
            }
        };
        let classification = if open_indexes.is_empty() {
            OpenPayloadClassification::Empty
        } else {
            OpenPayloadClassification::Nonempty
        };
        let metadata = OpenResponseMetadata {
            http_status: status.as_u16(),
            decoded_bytes: body.len(),
            decoded_body_sha256,
            canonical_set_sha256: canonical_open_set_sha256(&open_indexes),
            raw_value_count,
            unique_value_count: open_indexes.len(),
            duplicate_value_count,
            content_type,
            etag: headers.etag,
            cache_control: headers.cache_control,
            date: headers.date,
            age_seconds: headers.age_seconds,
            last_modified: headers.last_modified,
        };
        Ok(OpenSectionsResponse {
            target,
            classification,
            open_indexes,
            metadata,
        })
    }
}

struct ResponseHeaders {
    etag: Option<String>,
    cache_control: Option<String>,
    date: Option<String>,
    age_seconds: Option<u64>,
    last_modified: Option<String>,
}

impl OpenFailureResponseMetadata {
    fn capture(response: &Response) -> Self {
        let headers = response.headers();
        let age = header_text(headers, AGE);
        Self {
            http_status: response.status().as_u16(),
            decoded_bytes: None,
            decoded_body_sha256: None,
            content_type: header_text(headers, CONTENT_TYPE),
            etag: header_text(headers, ETAG),
            cache_control: header_text(headers, CACHE_CONTROL),
            date: header_text(headers, DATE),
            age_seconds: age.as_deref().and_then(|value| value.parse().ok()),
            age,
            last_modified: header_text(headers, LAST_MODIFIED),
            retry_after_raw: single_header_text(headers, RETRY_AFTER),
            retry_after: retry_after(headers),
        }
    }
}

impl ResponseHeaders {
    fn capture(headers: &HeaderMap) -> Self {
        Self {
            etag: header_text(headers, ETAG),
            cache_control: header_text(headers, CACHE_CONTROL),
            date: header_text(headers, DATE),
            age_seconds: headers
                .get(AGE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse().ok()),
            last_modified: header_text(headers, LAST_MODIFIED),
        }
    }
}

fn decode_open_array(body: &[u8]) -> Result<(usize, usize, Vec<SectionIndex>), OpenSectionsError> {
    let root: Value = serde_json::from_slice(body).map_err(|_| OpenSectionsError::InvalidJson)?;
    let values = root.as_array().ok_or(OpenSectionsError::RootNotArray)?;
    let raw_value_count = values.len();
    let mut unique = BTreeSet::new();
    for (position, value) in values.iter().enumerate() {
        let value = value
            .as_str()
            .ok_or(OpenSectionsError::NonStringValue { position })?;
        let index = SectionIndex::try_from(value)
            .map_err(|_| OpenSectionsError::InvalidSectionIndex { position })?;
        unique.insert(index);
    }
    let duplicate_value_count = raw_value_count - unique.len();
    Ok((
        raw_value_count,
        duplicate_value_count,
        unique.into_iter().collect(),
    ))
}

/// Hash a canonical Open set independently of target identity.
///
/// The exact UTF-8 preimage is `bcsp-open-set-v1\n`, followed by every sorted
/// unique five-digit index and a trailing `\n` for each index.
pub fn canonical_open_set_sha256(indexes: &[SectionIndex]) -> OpenCanonicalSetHash {
    let unique = indexes.iter().collect::<BTreeSet<_>>();
    let mut preimage = Vec::with_capacity(OPEN_SET_HASH_DOMAIN.len() + unique.len() * 6);
    preimage.extend_from_slice(OPEN_SET_HASH_DOMAIN);
    for index in unique {
        preimage.extend_from_slice(index.as_str().as_bytes());
        preimage.push(b'\n');
    }
    OpenCanonicalSetHash::try_from(sha256_hex(&preimage))
        .expect("SHA-256 always produces a canonical lowercase 64-digit hexadecimal hash")
}

fn classify_reqwest_error(error: reqwest::Error) -> OpenSectionsError {
    if error.is_timeout() {
        OpenSectionsError::Timeout
    } else if error.is_decode() {
        OpenSectionsError::ContentDecoding
    } else {
        OpenSectionsError::Network
    }
}

fn classify_http_failure(response: &Response) -> OpenSectionsError {
    let status = response.status();
    if status == StatusCode::REQUEST_TIMEOUT || status.is_server_error() {
        return OpenSectionsError::TransientHttp {
            status: status.as_u16(),
        };
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        return OpenSectionsError::RateLimited {
            retry_after: retry_after(response.headers()),
        };
    }
    if status == StatusCode::FORBIDDEN {
        return OpenSectionsError::Forbidden;
    }
    if status.is_redirection() && status != StatusCode::NOT_MODIFIED {
        return OpenSectionsError::Redirect {
            status: status.as_u16(),
            scope: redirect_scope(response),
        };
    }
    if status.is_client_error() {
        return OpenSectionsError::FatalClientHttp {
            status: status.as_u16(),
        };
    }
    OpenSectionsError::UnsupportedHttp {
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

fn header_text(headers: &HeaderMap, name: reqwest::header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn single_header_text(headers: &HeaderMap, name: reqwest::header::HeaderName) -> Option<String> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?;
    if values.next().is_some() {
        return None;
    }
    value.to_str().ok().map(str::to_owned)
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

pub(crate) fn retry_after(headers: &HeaderMap) -> RetryAfterHeader {
    let mut values = headers.get_all(RETRY_AFTER).iter();
    let Some(value) = values.next() else {
        return RetryAfterHeader::Missing;
    };
    if values.next().is_some() {
        return RetryAfterHeader::Invalid;
    }
    let Ok(value) = value.to_str() else {
        return RetryAfterHeader::Invalid;
    };
    if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) {
        return value
            .parse::<u64>()
            .map(RetryAfterValue::DelaySeconds)
            .map(RetryAfterHeader::Valid)
            .unwrap_or(RetryAfterHeader::Invalid);
    }
    parse_imf_fixdate(value)
        .map(RetryAfterValue::HttpDateUnixSeconds)
        .map(RetryAfterHeader::Valid)
        .unwrap_or(RetryAfterHeader::Invalid)
}

fn parse_imf_fixdate(value: &str) -> Option<i64> {
    let bytes = value.as_bytes();
    if bytes.len() != 29
        || bytes[3] != b','
        || bytes[4] != b' '
        || bytes[7] != b' '
        || bytes[11] != b' '
        || bytes[16] != b' '
        || bytes[19] != b':'
        || bytes[22] != b':'
        || bytes[25] != b' '
        || &bytes[26..] != b"GMT"
    {
        return None;
    }
    let weekday = match &bytes[..3] {
        b"Sun" => 0,
        b"Mon" => 1,
        b"Tue" => 2,
        b"Wed" => 3,
        b"Thu" => 4,
        b"Fri" => 5,
        b"Sat" => 6,
        _ => return None,
    };
    let day = parse_decimal(&bytes[5..7])?;
    let month = match &bytes[8..11] {
        b"Jan" => 1,
        b"Feb" => 2,
        b"Mar" => 3,
        b"Apr" => 4,
        b"May" => 5,
        b"Jun" => 6,
        b"Jul" => 7,
        b"Aug" => 8,
        b"Sep" => 9,
        b"Oct" => 10,
        b"Nov" => 11,
        b"Dec" => 12,
        _ => return None,
    };
    let year = parse_decimal(&bytes[12..16])?;
    let hour = parse_decimal(&bytes[17..19])?;
    let minute = parse_decimal(&bytes[20..22])?;
    let second = parse_decimal(&bytes[23..25])?;
    if year < 1_601
        || day == 0
        || day > days_in_month(year, month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    let days = days_from_civil(year as i64, month as i64, day as i64);
    if (days + 4).rem_euclid(7) != weekday {
        return None;
    }
    days.checked_mul(86_400)?
        .checked_add((hour * 3_600 + minute * 60 + second) as i64)
}

fn parse_decimal(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    bytes.iter().try_fold(0_u32, |value, byte| {
        value.checked_mul(10)?.checked_add(u32::from(byte - b'0'))
    })
}

const fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100)) => {
            29
        }
        2 => 28,
        _ => 0,
    }
}

const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let adjusted_year = year - if month <= 2 { 1 } else { 0 };
    let era = if adjusted_year >= 0 {
        adjusted_year
    } else {
        adjusted_year - 399
    } / 400;
    let year_of_era = adjusted_year - era * 400;
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc::{Receiver, channel};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    use bcsp_contracts::SectionIndex;

    use crate::{DiscoverySnapshot, SourceProvenance, decode_discovery_payload};

    use super::*;

    struct FakeResponse {
        status: &'static str,
        headers: Vec<(&'static str, String)>,
        body: Vec<u8>,
        delay: Duration,
    }

    impl FakeResponse {
        fn json(body: impl Into<Vec<u8>>) -> Self {
            Self {
                status: "200 OK",
                headers: vec![("Content-Type", "application/json".to_owned())],
                body: body.into(),
                delay: Duration::ZERO,
            }
        }

        fn status(status: &'static str) -> Self {
            Self {
                status,
                headers: Vec::new(),
                body: Vec::new(),
                delay: Duration::ZERO,
            }
        }
    }

    fn fake_upstream(response: FakeResponse) -> (Url, Receiver<String>, JoinHandle<()>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind fake upstream");
        let address = listener.local_addr().expect("fake address");
        let (sender, receiver) = channel();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept fake request");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("set fake read timeout");
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
            thread::sleep(response.delay);
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
        let endpoint = Url::parse(&format!("http://{address}/soc/api/openSections.json"))
            .expect("fake endpoint");
        (endpoint, receiver, handle)
    }

    fn discovery_snapshot(body_suffix: &str) -> DiscoverySnapshot {
        let body = format!(
            r#"{{
              "sourceVersion":"synthetic-{body_suffix}",
              "terms":[
                {{"termId":"92026","year":2026,"termCode":"9"}},
                {{"termId":"12027","year":2027,"termCode":"1"}}
              ],
              "campuses":[{{"campusCode":"ONLINE_NB"}}],
              "targets":[
                {{"termId":"92026","campusCode":"ONLINE_NB"}},
                {{"termId":"12027","campusCode":"ONLINE_NB"}}
              ],
              "subjects":[]
            }}"#
        );
        DiscoverySnapshot::try_from_raw(
            decode_discovery_payload(body.as_bytes()).expect("synthetic discovery JSON"),
            SourceProvenance::from_body(
                "SYNTHETIC_DISCOVERY",
                "2026-07-14T00:00:00Z",
                body.as_bytes(),
            ),
        )
        .expect("bound synthetic discovery")
    }

    fn request() -> OpenSectionsRequest {
        let snapshot = discovery_snapshot("request");
        let term = snapshot
            .terms
            .iter()
            .find(|term| term.term_id.as_str() == "92026")
            .expect("discovered term");
        let target = snapshot
            .targets
            .iter()
            .find(|target| target.key.term().as_str() == "92026")
            .expect("discovered target");
        OpenSectionsRequest::try_from_discovery(term, target).expect("bound source request")
    }

    fn test_client(endpoint: Url) -> RutgersOpenSectionsClient {
        RutgersOpenSectionsClient::build(
            endpoint,
            TransportLimits {
                https_only: false,
                ..TransportLimits::OFFICIAL
            },
        )
        .expect("test client")
    }

    async fn fetch_fake(response: FakeResponse) -> OpenSectionsFailure {
        let (endpoint, capture, handle) = fake_upstream(response);
        let error = test_client(endpoint)
            .fetch(&request())
            .await
            .expect_err("fake response must fail");
        capture.recv().expect("captured request");
        handle.join().expect("fake upstream thread");
        error
    }

    async fn fetch_fake_kind(response: FakeResponse) -> OpenSectionsError {
        fetch_fake(response).await.into_parts().0
    }

    #[test]
    fn official_request_url_is_exact_and_coordinates_are_bound_to_discovery_identity() {
        let client = RutgersOpenSectionsClient::new_official().unwrap();
        let snapshot = discovery_snapshot("binding");
        let term = snapshot
            .terms
            .iter()
            .find(|term| term.term_id.as_str() == "92026")
            .unwrap();
        let other_term = snapshot
            .terms
            .iter()
            .find(|term| term.term_id.as_str() == "12027")
            .unwrap();
        let target = snapshot
            .targets
            .iter()
            .find(|target| target.key.term().as_str() == "92026")
            .unwrap();
        let request = OpenSectionsRequest::try_from_discovery(term, target).unwrap();
        assert_eq!(
            client.request_url(&request).as_str(),
            "https://classes.rutgers.edu/soc/api/openSections.json?year=2026&term=9&campus=ONLINE_NB"
        );
        assert_eq!(request.discovery_source_id(), &term.source_id);
        assert_eq!(OPEN_CONNECT_TIMEOUT, Duration::from_secs(5));
        assert_eq!(OPEN_TOTAL_TIMEOUT, Duration::from_secs(15));
        assert_eq!(OPEN_MAX_DECODED_BYTES, 10_485_760);

        assert_eq!(
            OpenSectionsRequest::try_from_discovery(other_term, target),
            Err(OpenSectionsRequestError::TermMismatch)
        );
        let other_snapshot = discovery_snapshot("different-immutable-source");
        let other_target = other_snapshot
            .targets
            .iter()
            .find(|target| target.key.term().as_str() == "92026")
            .unwrap();
        assert_eq!(
            OpenSectionsRequest::try_from_discovery(term, other_target),
            Err(OpenSectionsRequestError::SourceMismatch)
        );

        let mut invalid_year = term.clone();
        invalid_year.year = crate::Presence::Value(999);
        assert_eq!(
            OpenSectionsRequest::try_from_discovery(&invalid_year, target),
            Err(OpenSectionsRequestError::InvalidYear)
        );
        let mut invalid_term_code = term.clone();
        invalid_term_code.term_code = crate::Presence::Value("09".to_owned());
        assert_eq!(
            OpenSectionsRequest::try_from_discovery(&invalid_term_code, target),
            Err(OpenSectionsRequestError::InvalidTermCode)
        );

        let persisted = OpenSectionsRequest::try_from_persisted_discovery(
            term.source_id.as_str(),
            &term.term_id,
            &target.key,
            Some(2026),
            Some("9"),
        )
        .expect("persisted coordinates");
        assert_eq!(persisted, request);
        assert_eq!(
            OpenSectionsRequest::try_from_persisted_discovery(
                "selector:not-a-digest",
                &term.term_id,
                &target.key,
                Some(2026),
                Some("9"),
            ),
            Err(OpenSectionsRequestError::InvalidDiscoverySourceId)
        );
        assert_eq!(
            OpenSectionsRequest::try_from_persisted_discovery(
                term.source_id.as_str(),
                &term.term_id,
                &target.key,
                Some(999),
                Some("9"),
            ),
            Err(OpenSectionsRequestError::InvalidYear)
        );
        assert_eq!(
            OpenSectionsRequest::try_from_persisted_discovery(
                term.source_id.as_str(),
                &term.term_id,
                &target.key,
                Some(2026),
                Some("09"),
            ),
            Err(OpenSectionsRequestError::InvalidTermCode)
        );
        assert_eq!(
            OpenSectionsRequest::try_from_persisted_discovery(
                term.source_id.as_str(),
                &term.term_id,
                &snapshot
                    .targets
                    .iter()
                    .find(|target| target.key.term().as_str() == "12027")
                    .expect("other persisted target")
                    .key,
                Some(2027),
                Some("1"),
            ),
            Err(OpenSectionsRequestError::TermMismatch)
        );
    }

    #[test]
    fn canonical_set_hash_has_a_locked_target_independent_preimage() {
        let first = SectionIndex::try_from("00002").unwrap();
        let second = SectionIndex::try_from("00001").unwrap();
        assert_eq!(
            canonical_open_set_sha256(&[]).as_str(),
            "f362d41cca731d76356389382967f447802b8493f110c63aca66baf7235d4ff6"
        );
        assert_eq!(
            canonical_open_set_sha256(&[first, second.clone(), second]).as_str(),
            "817ceb456303f598c8631b1d31dcfd471c8facd6bef7b93c4d4f44433784f369"
        );
    }

    #[tokio::test]
    async fn fake_upstream_proves_exact_get_no_credentials_and_strict_set_metadata() {
        let mut response = FakeResponse::json(br#"["00002","00001","00002"]"#.to_vec());
        response.headers.extend([
            ("ETag", "\"synthetic-etag\"".to_owned()),
            ("Cache-Control", "max-age=30".to_owned()),
            ("Date", "Sun, 06 Nov 1994 08:49:37 GMT".to_owned()),
            ("Age", "7".to_owned()),
            ("Last-Modified", "Sun, 06 Nov 1994 08:49:37 GMT".to_owned()),
        ]);
        let (endpoint, capture, handle) = fake_upstream(response);
        let result = test_client(endpoint).fetch(&request()).await.unwrap();
        let captured = capture.recv().expect("captured request");
        handle.join().expect("fake upstream thread");

        let lower = captured.to_ascii_lowercase();
        assert!(captured.starts_with(
            "GET /soc/api/openSections.json?year=2026&term=9&campus=ONLINE_NB HTTP/1.1\r\n"
        ));
        assert!(lower.contains("accept: application/json\r\n"));
        assert!(!lower.contains("authorization:"));
        assert!(!lower.contains("cookie:"));
        assert!(!lower.contains("cache-bust"));
        assert!(!lower.contains("timestamp="));

        assert_eq!(result.classification, OpenPayloadClassification::Nonempty);
        assert_eq!(
            result
                .open_indexes
                .iter()
                .map(SectionIndex::as_str)
                .collect::<Vec<_>>(),
            ["00001", "00002"]
        );
        assert_eq!(result.metadata.raw_value_count, 3);
        assert_eq!(result.metadata.unique_value_count, 2);
        assert_eq!(result.metadata.duplicate_value_count, 1);
        assert_eq!(
            result.metadata.canonical_set_sha256.as_str(),
            "817ceb456303f598c8631b1d31dcfd471c8facd6bef7b93c4d4f44433784f369"
        );
        assert_eq!(result.metadata.etag.as_deref(), Some("\"synthetic-etag\""));
        assert_eq!(result.metadata.cache_control.as_deref(), Some("max-age=30"));
        assert_eq!(result.metadata.age_seconds, Some(7));
    }

    #[tokio::test]
    async fn empty_array_is_typed_success_for_later_catalog_aware_reconcile() {
        let (endpoint, capture, handle) = fake_upstream(FakeResponse::json(b"[]".to_vec()));
        let result = test_client(endpoint).fetch(&request()).await.unwrap();
        capture.recv().expect("captured request");
        handle.join().expect("fake upstream thread");
        assert_eq!(result.classification, OpenPayloadClassification::Empty);
        assert!(result.open_indexes.is_empty());
        assert_eq!(result.metadata.raw_value_count, 0);
        assert_eq!(
            result.metadata.decoded_body_sha256,
            "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
        );
        assert_eq!(
            result.metadata.canonical_set_sha256.as_str(),
            "f362d41cca731d76356389382967f447802b8493f110c63aca66baf7235d4ff6"
        );
    }

    #[tokio::test]
    async fn schema_and_content_type_fail_closed_without_echoing_raw_values() {
        for (body, expected) in [
            (b"not-json".as_slice(), OpenSectionsError::InvalidJson),
            (br#"{"value":[]}"#, OpenSectionsError::RootNotArray),
            (
                br#"["00001",2]"#,
                OpenSectionsError::NonStringValue { position: 1 },
            ),
            (
                br#"[" 00001"]"#,
                OpenSectionsError::InvalidSectionIndex { position: 0 },
            ),
            (
                br#"["0001"]"#,
                OpenSectionsError::InvalidSectionIndex { position: 0 },
            ),
        ] {
            assert_eq!(
                fetch_fake_kind(FakeResponse::json(body.to_vec())).await,
                expected
            );
        }

        let mut html = FakeResponse::json(b"[]".to_vec());
        html.headers[0].1 = "text/html".to_owned();
        assert_eq!(
            fetch_fake_kind(html).await,
            OpenSectionsError::InvalidContentType
        );
    }

    #[tokio::test]
    async fn post_response_failures_retain_redacted_headers_and_complete_body_facts() {
        let body = b"not-json".to_vec();
        let mut invalid_json = FakeResponse::json(body.clone());
        invalid_json.headers.extend([
            ("ETag", "\"failure-etag\"".to_owned()),
            ("Cache-Control", "max-age=15".to_owned()),
            ("Date", "Sun, 06 Nov 1994 08:49:37 GMT".to_owned()),
            ("Age", "7".to_owned()),
            ("Last-Modified", "Sun, 06 Nov 1994 08:49:37 GMT".to_owned()),
            ("Retry-After", "120".to_owned()),
        ]);
        let failure = fetch_fake(invalid_json).await;
        assert_eq!(failure.kind(), &OpenSectionsError::InvalidJson);
        let metadata = failure
            .response_metadata()
            .expect("a response failure must retain metadata");
        assert_eq!(metadata.http_status, 200);
        assert_eq!(metadata.decoded_bytes, Some(body.len()));
        assert_eq!(
            metadata.decoded_body_sha256.as_deref(),
            Some(sha256_hex(&body).as_str())
        );
        assert_eq!(metadata.content_type.as_deref(), Some("application/json"));
        assert_eq!(metadata.etag.as_deref(), Some("\"failure-etag\""));
        assert_eq!(metadata.cache_control.as_deref(), Some("max-age=15"));
        assert_eq!(metadata.age.as_deref(), Some("7"));
        assert_eq!(metadata.age_seconds, Some(7));
        assert_eq!(metadata.retry_after_raw.as_deref(), Some("120"));
        assert_eq!(
            metadata.retry_after,
            RetryAfterHeader::Valid(RetryAfterValue::DelaySeconds(120))
        );

        let mut unavailable = FakeResponse::status("503 Service Unavailable");
        unavailable
            .headers
            .push(("ETag", "\"unavailable\"".to_owned()));
        let failure = fetch_fake(unavailable).await;
        assert_eq!(
            failure.kind(),
            &OpenSectionsError::TransientHttp { status: 503 }
        );
        let metadata = failure.response_metadata().expect("HTTP response metadata");
        assert_eq!(metadata.http_status, 503);
        assert_eq!(metadata.etag.as_deref(), Some("\"unavailable\""));
        assert_eq!(metadata.decoded_bytes, None);
        assert_eq!(metadata.decoded_body_sha256, None);
    }

    #[tokio::test]
    async fn corrupt_gzip_is_a_fatal_content_violation_with_response_metadata() {
        let mut corrupt = FakeResponse::json(b"not-a-gzip-stream".to_vec());
        corrupt
            .headers
            .push(("Content-Encoding", "gzip".to_owned()));
        corrupt.headers.push(("ETag", "\"corrupt\"".to_owned()));
        let failure = fetch_fake(corrupt).await;
        assert_eq!(failure.kind(), &OpenSectionsError::ContentDecoding);
        assert_eq!(
            failure.disposition(),
            OpenFailureDisposition::FatalProtocolOriginCircuit
        );
        let metadata = failure.response_metadata().expect("response metadata");
        assert_eq!(metadata.http_status, 200);
        assert_eq!(metadata.etag.as_deref(), Some("\"corrupt\""));
        assert!(metadata.decoded_bytes.is_some());
        assert_eq!(metadata.decoded_body_sha256, None);
    }

    #[tokio::test]
    async fn decoded_limit_is_enforced_by_length_and_streamed_byte_count() {
        let mut declared = FakeResponse::json(Vec::new());
        declared.headers.push((
            "Content-Length",
            (OPEN_MAX_DECODED_BYTES as u64 + 1).to_string(),
        ));
        assert_eq!(
            fetch_fake_kind(declared).await,
            OpenSectionsError::ResponseTooLarge
        );

        let (endpoint, capture, handle) =
            fake_upstream(FakeResponse::json(b"[\"00001\"]".to_vec()));
        let client = RutgersOpenSectionsClient::build(
            endpoint,
            TransportLimits {
                max_decoded_bytes: 8,
                https_only: false,
                ..TransportLimits::OFFICIAL
            },
        )
        .unwrap();
        assert_eq!(
            client.fetch(&request()).await.unwrap_err().kind(),
            &OpenSectionsError::ResponseTooLarge
        );
        capture.recv().expect("captured request");
        handle.join().expect("fake upstream thread");
    }

    #[tokio::test]
    async fn redirects_http_failures_and_retry_after_are_classified_without_retry() {
        assert_eq!(
            fetch_fake_kind(FakeResponse::status("408 Request Timeout")).await,
            OpenSectionsError::TransientHttp { status: 408 }
        );
        assert_eq!(
            fetch_fake_kind(FakeResponse::status("503 Service Unavailable")).await,
            OpenSectionsError::TransientHttp { status: 503 }
        );
        assert_eq!(
            fetch_fake_kind(FakeResponse::status("403 Forbidden")).await,
            OpenSectionsError::Forbidden
        );
        assert_eq!(
            fetch_fake_kind(FakeResponse::status("404 Not Found")).await,
            OpenSectionsError::FatalClientHttp { status: 404 }
        );

        let mut rate_limited = FakeResponse::status("429 Too Many Requests");
        rate_limited.headers.push(("Retry-After", "120".to_owned()));
        let failure = fetch_fake(rate_limited).await;
        assert_eq!(
            failure.kind(),
            &OpenSectionsError::RateLimited {
                retry_after: RetryAfterHeader::Valid(RetryAfterValue::DelaySeconds(120))
            }
        );
        assert_eq!(
            failure
                .response_metadata()
                .expect("rate-limit metadata")
                .retry_after_raw
                .as_deref(),
            Some("120")
        );
        let mut dated = FakeResponse::status("429 Too Many Requests");
        dated
            .headers
            .push(("Retry-After", "Sun, 06 Nov 1994 08:49:37 GMT".to_owned()));
        assert_eq!(
            fetch_fake_kind(dated).await,
            OpenSectionsError::RateLimited {
                retry_after: RetryAfterHeader::Valid(RetryAfterValue::HttpDateUnixSeconds(
                    784_111_777
                ))
            }
        );
        let mut invalid = FakeResponse::status("429 Too Many Requests");
        invalid.headers.push(("Retry-After", "tomorrow".to_owned()));
        let failure = fetch_fake(invalid).await;
        assert_eq!(
            failure.kind(),
            &OpenSectionsError::RateLimited {
                retry_after: RetryAfterHeader::Invalid
            }
        );
        assert_eq!(
            failure
                .response_metadata()
                .expect("invalid Retry-After metadata")
                .retry_after_raw
                .as_deref(),
            Some("tomorrow")
        );

        let mut duplicate = FakeResponse::status("429 Too Many Requests");
        duplicate.headers.extend([
            ("Retry-After", "120".to_owned()),
            ("Retry-After", "240".to_owned()),
        ]);
        let failure = fetch_fake(duplicate).await;
        assert_eq!(
            failure.kind(),
            &OpenSectionsError::RateLimited {
                retry_after: RetryAfterHeader::Invalid
            }
        );
        assert_eq!(
            failure
                .response_metadata()
                .expect("duplicate Retry-After metadata")
                .retry_after_raw,
            None
        );

        let mut redirect = FakeResponse::status("302 Found");
        redirect
            .headers
            .push(("Location", "https://example.invalid/open".to_owned()));
        assert_eq!(
            fetch_fake_kind(redirect).await,
            OpenSectionsError::Redirect {
                status: 302,
                scope: RedirectScope::OffOrigin
            }
        );
    }

    #[tokio::test]
    async fn total_timeout_is_redacted_and_transient() {
        let mut delayed = FakeResponse::json(b"[]".to_vec());
        delayed.delay = Duration::from_millis(150);
        let (endpoint, capture, handle) = fake_upstream(delayed);
        let client = RutgersOpenSectionsClient::build(
            endpoint,
            TransportLimits {
                total_timeout: Duration::from_millis(25),
                https_only: false,
                ..TransportLimits::OFFICIAL
            },
        )
        .unwrap();
        let error = client.fetch(&request()).await.unwrap_err();
        assert_eq!(error.kind(), &OpenSectionsError::Timeout);
        assert_eq!(
            error.disposition(),
            OpenFailureDisposition::TransientBackoff
        );
        capture.recv().expect("captured request");
        handle.join().expect("fake upstream thread");
    }

    #[test]
    fn retry_after_date_parser_rejects_invalid_calendar_and_weekday_values() {
        assert_eq!(
            parse_imf_fixdate("Sun, 06 Nov 1994 08:49:37 GMT"),
            Some(784_111_777)
        );
        assert_eq!(parse_imf_fixdate("Mon, 06 Nov 1994 08:49:37 GMT"), None);
        assert_eq!(parse_imf_fixdate("Sun, 31 Feb 1994 08:49:37 GMT"), None);
        assert_eq!(parse_imf_fixdate("Sun, 06 Nov 1994 25:49:37 GMT"), None);
    }
}
