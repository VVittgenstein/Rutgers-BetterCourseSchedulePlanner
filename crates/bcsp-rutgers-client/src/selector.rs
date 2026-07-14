use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

#[cfg(test)]
use std::net::IpAddr;

use bcsp_contracts::{CampusCode, TermId};
use reqwest::header::{
    ACCEPT, CACHE_CONTROL, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, DATE, ETAG, HeaderMap,
    LAST_MODIFIED, LOCATION,
};
use reqwest::{Client, Method, Request, Response, StatusCode, Url, redirect::Policy};
use serde_json::Value;
use thiserror::Error;

use crate::open_sections::RedirectScope;
use crate::raw_catalog::decode_strict_json;
use crate::{
    DiscoverySnapshot, Presence, RawDiscoveryCampus, RawDiscoveryDocument, RawDiscoveryTarget,
    RawDiscoveryTerm, SourceProvenance, sha256_hex,
};

pub const RUTGERS_DISCOVERY_ROOT: &str = "https://classes.rutgers.edu/soc/";
pub const DISCOVERY_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const DISCOVERY_TOTAL_TIMEOUT: Duration = Duration::from_secs(20);
pub const DISCOVERY_MAX_BOOTSTRAP_BYTES: usize = 1024 * 1024;
pub const DISCOVERY_MAX_SELECTOR_BYTES: usize = 512 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveryResourceKind {
    Bootstrap,
    SelectorScript,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryResourceMetadata {
    pub resource: DiscoveryResourceKind,
    pub source_url: String,
    pub http_status: u16,
    pub decoded_bytes: usize,
    pub decoded_body_sha256: String,
    pub content_type: String,
    pub content_encoding: Option<String>,
    pub content_length: Option<u64>,
    pub etag: Option<String>,
    pub cache_control: Option<String>,
    pub date: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryResponseMetadata {
    pub source_version: String,
    pub bootstrap: DiscoveryResourceMetadata,
    pub selector_script: DiscoveryResourceMetadata,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryResponse {
    raw: RawDiscoveryDocument,
    snapshot: DiscoverySnapshot,
    source_provenance: SourceProvenance,
    metadata: DiscoveryResponseMetadata,
}

impl DiscoveryResponse {
    pub const fn raw(&self) -> &RawDiscoveryDocument {
        &self.raw
    }

    pub const fn snapshot(&self) -> &DiscoverySnapshot {
        &self.snapshot
    }

    pub const fn source_provenance(&self) -> &SourceProvenance {
        &self.source_provenance
    }

    pub const fn metadata(&self) -> &DiscoveryResponseMetadata {
        &self.metadata
    }

    pub fn into_parts(
        self,
    ) -> (
        RawDiscoveryDocument,
        DiscoverySnapshot,
        SourceProvenance,
        DiscoveryResponseMetadata,
    ) {
        (
            self.raw,
            self.snapshot,
            self.source_provenance,
            self.metadata,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryFailureResponseMetadata {
    pub resource: DiscoveryResourceKind,
    pub http_status: u16,
    pub decoded_bytes: Option<usize>,
    pub decoded_body_sha256: Option<String>,
    pub content_type: Option<String>,
    pub content_encoding: Option<String>,
    pub content_length: Option<u64>,
    pub etag: Option<String>,
    pub cache_control: Option<String>,
    pub date: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DiscoveryTransportError {
    #[error("Rutgers discovery request timed out")]
    Timeout,
    #[error("Rutgers discovery request encountered a network failure")]
    Network,
    #[error("Rutgers discovery request could not be constructed")]
    RequestConstruction,
    #[error("Rutgers discovery response URL differed from the requested allowlisted origin")]
    ResponseUrlMismatch,
    #[error("Rutgers discovery upstream returned a redirect that was not followed")]
    Redirect {
        resource: DiscoveryResourceKind,
        status: u16,
        scope: RedirectScope,
    },
    #[error("Rutgers discovery upstream returned a non-success status")]
    NonSuccessHttp {
        resource: DiscoveryResourceKind,
        status: u16,
    },
    #[error("Rutgers discovery response Content-Type was not allowlisted")]
    InvalidContentType { resource: DiscoveryResourceKind },
    #[error("Rutgers discovery decoded response exceeded its product limit")]
    ResponseTooLarge { resource: DiscoveryResourceKind },
    #[error("Rutgers discovery response content encoding could not be decoded")]
    ContentDecoding { resource: DiscoveryResourceKind },
    #[error("Rutgers bootstrap format was invalid")]
    InvalidBootstrap,
    #[error("Rutgers selector script was not a versioned same-origin soc_utils.js resource")]
    InvalidSelectorScriptUrl,
    #[error("Rutgers selector script format was invalid")]
    InvalidSelectorScript,
    #[error("Rutgers discovery document failed identity validation")]
    InvalidDiscoveryDocument,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{kind}")]
pub struct DiscoveryFailure {
    #[source]
    kind: DiscoveryTransportError,
    response_metadata: Option<Box<DiscoveryFailureResponseMetadata>>,
}

impl DiscoveryFailure {
    fn before_response(kind: DiscoveryTransportError) -> Self {
        Self {
            kind,
            response_metadata: None,
        }
    }

    fn from_response(
        kind: DiscoveryTransportError,
        response_metadata: DiscoveryFailureResponseMetadata,
    ) -> Self {
        Self {
            kind,
            response_metadata: Some(Box::new(response_metadata)),
        }
    }

    pub const fn kind(&self) -> &DiscoveryTransportError {
        &self.kind
    }

    pub fn response_metadata(&self) -> Option<&DiscoveryFailureResponseMetadata> {
        self.response_metadata.as_deref()
    }

    pub fn into_parts(
        self,
    ) -> (
        DiscoveryTransportError,
        Option<DiscoveryFailureResponseMetadata>,
    ) {
        (self.kind, self.response_metadata.map(|metadata| *metadata))
    }
}

#[derive(Clone, Copy)]
struct TransportLimits {
    connect_timeout: Duration,
    total_timeout: Duration,
    max_bootstrap_bytes: usize,
    max_selector_bytes: usize,
}

impl TransportLimits {
    const OFFICIAL: Self = Self {
        connect_timeout: DISCOVERY_CONNECT_TIMEOUT,
        total_timeout: DISCOVERY_TOTAL_TIMEOUT,
        max_bootstrap_bytes: DISCOVERY_MAX_BOOTSTRAP_BYTES,
        max_selector_bytes: DISCOVERY_MAX_SELECTOR_BYTES,
    };
}

#[derive(Clone, Copy)]
enum EndpointMode {
    Official,
    #[cfg(test)]
    TestLoopback,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("Rutgers discovery client configuration is invalid")]
pub struct DiscoveryClientBuildError;

pub struct RutgersDiscoveryClient {
    client: Client,
    root: Url,
    max_bootstrap_bytes: usize,
    max_selector_bytes: usize,
}

impl RutgersDiscoveryClient {
    pub fn new_official() -> Result<Self, DiscoveryClientBuildError> {
        let root = Url::parse(RUTGERS_DISCOVERY_ROOT).map_err(|_| DiscoveryClientBuildError)?;
        Self::build(root, TransportLimits::OFFICIAL, EndpointMode::Official)
    }

    fn build(
        root: Url,
        limits: TransportLimits,
        mode: EndpointMode,
    ) -> Result<Self, DiscoveryClientBuildError> {
        if root.query().is_some()
            || root.fragment().is_some()
            || root.cannot_be_a_base()
            || !root.username().is_empty()
            || root.password().is_some()
            || root.path() != "/soc/"
            || !valid_endpoint(&root, mode)
            || limits.max_bootstrap_bytes == 0
            || limits.max_selector_bytes == 0
        {
            return Err(DiscoveryClientBuildError);
        }
        let client = Client::builder()
            .redirect(Policy::none())
            .retry(reqwest::retry::never())
            .referer(false)
            .https_only(matches!(mode, EndpointMode::Official))
            .connect_timeout(limits.connect_timeout)
            .timeout(limits.total_timeout)
            .gzip(true)
            .user_agent(format!("RBCSP/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| DiscoveryClientBuildError)?;
        Ok(Self {
            client,
            root,
            max_bootstrap_bytes: limits.max_bootstrap_bytes,
            max_selector_bytes: limits.max_selector_bytes,
        })
    }

    fn build_request(
        &self,
        url: Url,
        resource: DiscoveryResourceKind,
    ) -> Result<Request, DiscoveryFailure> {
        let accept = match resource {
            DiscoveryResourceKind::Bootstrap => "text/html, application/json;q=0.9",
            DiscoveryResourceKind::SelectorScript => {
                "application/javascript, text/javascript;q=0.9"
            }
        };
        self.client
            .get(url)
            .header(ACCEPT, accept)
            .build()
            .map_err(|_| {
                DiscoveryFailure::before_response(DiscoveryTransportError::RequestConstruction)
            })
    }

    pub async fn fetch(
        &self,
        request_id: &str,
        observed_at_utc: &str,
    ) -> Result<DiscoveryResponse, DiscoveryFailure> {
        let bootstrap = self
            .fetch_resource(
                self.root.clone(),
                DiscoveryResourceKind::Bootstrap,
                self.max_bootstrap_bytes,
            )
            .await?;
        let parsed_bootstrap = parse_bootstrap(&bootstrap.body, &self.root)
            .map_err(|kind| DiscoveryFailure::from_response(kind, bootstrap.failure_metadata()))?;
        let selector_script = self
            .fetch_resource(
                parsed_bootstrap.script_url.clone(),
                DiscoveryResourceKind::SelectorScript,
                self.max_selector_bytes,
            )
            .await?;
        let raw = parse_selector_script(
            &selector_script.body,
            &parsed_bootstrap.current,
            &parsed_bootstrap.source_version,
        )
        .map_err(|kind| {
            DiscoveryFailure::from_response(kind, selector_script.failure_metadata())
        })?;

        let mut source_preimage = b"rbcsp-rutgers-selector-source-v1\0".to_vec();
        source_preimage.extend_from_slice(bootstrap.metadata.decoded_body_sha256.as_bytes());
        source_preimage.push(0);
        source_preimage.extend_from_slice(selector_script.metadata.decoded_body_sha256.as_bytes());
        let source_provenance = SourceProvenance {
            request_id: request_id.to_owned(),
            observed_at_utc: observed_at_utc.to_owned(),
            body_sha256: sha256_hex(&source_preimage),
            decoded_bytes: bootstrap.body.len() + selector_script.body.len(),
        };
        let snapshot = DiscoverySnapshot::try_from_raw(raw.clone(), source_provenance.clone())
            .map_err(|_| {
                DiscoveryFailure::from_response(
                    DiscoveryTransportError::InvalidDiscoveryDocument,
                    selector_script.failure_metadata(),
                )
            })?;
        Ok(DiscoveryResponse {
            raw,
            snapshot,
            source_provenance,
            metadata: DiscoveryResponseMetadata {
                source_version: parsed_bootstrap.source_version,
                bootstrap: bootstrap.metadata,
                selector_script: selector_script.metadata,
            },
        })
    }

    async fn fetch_resource(
        &self,
        url: Url,
        resource: DiscoveryResourceKind,
        max_decoded_bytes: usize,
    ) -> Result<FetchedResource, DiscoveryFailure> {
        let outbound = self.build_request(url, resource)?;
        debug_assert_eq!(outbound.method(), Method::GET);
        let requested_url = outbound.url().clone();
        let response = self.client.execute(outbound).await.map_err(|error| {
            DiscoveryFailure::before_response(classify_reqwest_error(error, resource))
        })?;
        if !same_origin(response.url(), &requested_url) {
            let metadata = DiscoveryFailureResponseMetadata::capture(resource, &response);
            return Err(DiscoveryFailure::from_response(
                DiscoveryTransportError::ResponseUrlMismatch,
                metadata,
            ));
        }
        decode_resource(response, resource, max_decoded_bytes).await
    }
}

struct FetchedResource {
    body: Vec<u8>,
    metadata: DiscoveryResourceMetadata,
}

impl FetchedResource {
    fn failure_metadata(&self) -> DiscoveryFailureResponseMetadata {
        DiscoveryFailureResponseMetadata {
            resource: self.metadata.resource,
            http_status: self.metadata.http_status,
            decoded_bytes: Some(self.metadata.decoded_bytes),
            decoded_body_sha256: Some(self.metadata.decoded_body_sha256.clone()),
            content_type: Some(self.metadata.content_type.clone()),
            content_encoding: self.metadata.content_encoding.clone(),
            content_length: self.metadata.content_length,
            etag: self.metadata.etag.clone(),
            cache_control: self.metadata.cache_control.clone(),
            date: self.metadata.date.clone(),
            last_modified: self.metadata.last_modified.clone(),
        }
    }
}

async fn decode_resource(
    mut response: Response,
    resource: DiscoveryResourceKind,
    max_decoded_bytes: usize,
) -> Result<FetchedResource, DiscoveryFailure> {
    let status = response.status();
    let mut failure_metadata = DiscoveryFailureResponseMetadata::capture(resource, &response);
    if !status.is_success() {
        return Err(DiscoveryFailure::from_response(
            classify_http_failure(&response, resource),
            failure_metadata,
        ));
    }
    let Some(content_type) = accepted_content_type(response.headers(), resource) else {
        return Err(DiscoveryFailure::from_response(
            DiscoveryTransportError::InvalidContentType { resource },
            failure_metadata,
        ));
    };
    if failure_metadata
        .content_length
        .is_some_and(|length| length > max_decoded_bytes as u64)
    {
        return Err(DiscoveryFailure::from_response(
            DiscoveryTransportError::ResponseTooLarge { resource },
            failure_metadata,
        ));
    }
    let headers = ResponseHeaders::capture(response.headers());
    let source_url = response.url().as_str().to_owned();
    let mut body = Vec::new();
    loop {
        let chunk = match response.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(error) => {
                failure_metadata.decoded_bytes = Some(body.len());
                return Err(DiscoveryFailure::from_response(
                    classify_reqwest_error(error, resource),
                    failure_metadata,
                ));
            }
        };
        let Some(next_len) = body.len().checked_add(chunk.len()) else {
            failure_metadata.decoded_bytes = Some(body.len());
            return Err(DiscoveryFailure::from_response(
                DiscoveryTransportError::ResponseTooLarge { resource },
                failure_metadata,
            ));
        };
        if next_len > max_decoded_bytes {
            failure_metadata.decoded_bytes = Some(next_len);
            return Err(DiscoveryFailure::from_response(
                DiscoveryTransportError::ResponseTooLarge { resource },
                failure_metadata,
            ));
        }
        body.extend_from_slice(&chunk);
    }
    let body_sha256 = sha256_hex(&body);
    Ok(FetchedResource {
        metadata: DiscoveryResourceMetadata {
            resource,
            source_url,
            http_status: status.as_u16(),
            decoded_bytes: body.len(),
            decoded_body_sha256: body_sha256,
            content_type,
            content_encoding: headers.content_encoding,
            content_length: headers.content_length,
            etag: headers.etag,
            cache_control: headers.cache_control,
            date: headers.date,
            last_modified: headers.last_modified,
        },
        body,
    })
}

#[derive(Clone, Debug)]
struct CurrentTerm {
    year: i64,
    term_code: i64,
    campus: String,
}

struct ParsedBootstrap {
    current: CurrentTerm,
    script_url: Url,
    source_version: String,
}

fn parse_bootstrap(body: &[u8], root: &Url) -> Result<ParsedBootstrap, DiscoveryTransportError> {
    let init_json = extract_init_json(body).ok_or(DiscoveryTransportError::InvalidBootstrap)?;
    let current_value = extract_json_object_member(init_json, b"currentTermDate")
        .ok_or(DiscoveryTransportError::InvalidBootstrap)?;
    let current_object = decode_strict_json(current_value)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .ok_or(DiscoveryTransportError::InvalidBootstrap)?;
    let year = json_integer(&current_object, "year")
        .filter(|value| (1_000..=9_999).contains(value))
        .ok_or(DiscoveryTransportError::InvalidBootstrap)?;
    let term_code = json_integer(&current_object, "term")
        .filter(|value| matches!(value, 0 | 1 | 7 | 9))
        .ok_or(DiscoveryTransportError::InvalidBootstrap)?;
    let campus = current_object
        .get("campus")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(DiscoveryTransportError::InvalidBootstrap)?
        .to_owned();
    CampusCode::try_from(campus.clone()).map_err(|_| DiscoveryTransportError::InvalidBootstrap)?;
    let date = current_object
        .get("date")
        .and_then(Value::as_str)
        .ok_or(DiscoveryTransportError::InvalidBootstrap)?;
    if !valid_iso_date(date) {
        return Err(DiscoveryTransportError::InvalidBootstrap);
    }

    let script_reference =
        extract_soc_utils_script(body).ok_or(DiscoveryTransportError::InvalidSelectorScriptUrl)?;
    let script_url = root
        .join(&script_reference)
        .map_err(|_| DiscoveryTransportError::InvalidSelectorScriptUrl)?;
    let source_version = validate_script_url(root, &script_url)?;
    Ok(ParsedBootstrap {
        current: CurrentTerm {
            year,
            term_code,
            campus,
        },
        script_url,
        source_version,
    })
}

fn parse_selector_script(
    body: &[u8],
    current: &CurrentTerm,
    source_version: &str,
) -> Result<RawDiscoveryDocument, DiscoveryTransportError> {
    let mut parser = JsParser::new(body);
    let campus_value = parser
        .find_definition("CAMPUSES", b':')
        .and_then(|position| parser.parse_array_of_objects(position).ok())
        .ok_or(DiscoveryTransportError::InvalidSelectorScript)?;
    if campus_value.is_empty() || campus_value.len() > 256 {
        return Err(DiscoveryTransportError::InvalidSelectorScript);
    }
    let mut campus_codes = BTreeSet::new();
    let mut campuses = Vec::with_capacity(campus_value.len());
    for object in campus_value {
        let code = object
            .get("code")
            .and_then(JsScalar::as_text)
            .filter(|value| !value.is_empty())
            .ok_or(DiscoveryTransportError::InvalidSelectorScript)?;
        let name = object
            .get("name")
            .and_then(JsScalar::as_text)
            .filter(|value| !value.is_empty())
            .ok_or(DiscoveryTransportError::InvalidSelectorScript)?;
        let category = object
            .get("type")
            .and_then(JsScalar::as_integer)
            .filter(|value| *value >= 0)
            .ok_or(DiscoveryTransportError::InvalidSelectorScript)?;
        CampusCode::try_from(code.to_owned())
            .map_err(|_| DiscoveryTransportError::InvalidSelectorScript)?;
        if !campus_codes.insert(code.to_owned()) {
            return Err(DiscoveryTransportError::InvalidSelectorScript);
        }
        campuses.push(RawDiscoveryCampus {
            campus_code: Presence::Value(code.to_owned()),
            display: Presence::Value(name.to_owned()),
            category: Presence::Value(category.to_string()),
            enabled: Presence::Value(true),
            extra: BTreeMap::new(),
        });
    }
    if !campus_codes.contains(&current.campus) {
        return Err(DiscoveryTransportError::InvalidSelectorScript);
    }

    let semester_names = parse_semester_names(body)?;
    let previous_count = parse_previous_count(body)?;
    let transitions = parse_previous_transitions(body, &semester_names)?;
    let mut year = current.year;
    let mut term_code = current.term_code;
    let mut terms = Vec::with_capacity(previous_count + 1);
    let mut seen_terms = BTreeSet::new();
    for ordinal in 0..=previous_count {
        let display_name = semester_names
            .get(&term_code)
            .ok_or(DiscoveryTransportError::InvalidSelectorScript)?;
        let term_id_text = format!("{term_code}{year}");
        TermId::try_from(term_id_text.clone())
            .map_err(|_| DiscoveryTransportError::InvalidSelectorScript)?;
        if !seen_terms.insert(term_id_text.clone()) {
            return Err(DiscoveryTransportError::InvalidSelectorScript);
        }
        terms.push(RawDiscoveryTerm {
            term_id: Presence::Value(term_id_text),
            year: Presence::Value(year),
            term_code: Presence::Value(term_code.to_string()),
            display: Presence::Value(format!("{display_name} {year}")),
            published: Presence::Value(true),
            extra: BTreeMap::new(),
        });
        if ordinal < previous_count {
            let transition = transitions
                .get(&term_code)
                .ok_or(DiscoveryTransportError::InvalidSelectorScript)?;
            term_code = transition.next_term;
            if transition.decrement_year {
                year = year
                    .checked_sub(1)
                    .ok_or(DiscoveryTransportError::InvalidSelectorScript)?;
            }
        }
    }

    let mut targets = Vec::with_capacity(terms.len() * campuses.len());
    for term in &terms {
        let Presence::Value(term_id) = &term.term_id else {
            return Err(DiscoveryTransportError::InvalidSelectorScript);
        };
        for campus in &campuses {
            let Presence::Value(campus_code) = &campus.campus_code else {
                return Err(DiscoveryTransportError::InvalidSelectorScript);
            };
            targets.push(RawDiscoveryTarget {
                term_id: Presence::Value(term_id.clone()),
                campus_code: Presence::Value(campus_code.clone()),
                enabled: Presence::Value(true),
                extra: BTreeMap::new(),
            });
        }
    }
    Ok(RawDiscoveryDocument {
        source_version: Presence::Value(source_version.to_owned()),
        terms: Presence::Value(terms),
        campuses: Presence::Value(campuses),
        targets: Presence::Value(targets),
        subjects: Presence::Value(Vec::new()),
        extra: BTreeMap::new(),
    })
}

#[derive(Clone, Copy)]
struct PreviousTransition {
    next_term: i64,
    decrement_year: bool,
}

fn parse_semester_names(body: &[u8]) -> Result<BTreeMap<i64, String>, DiscoveryTransportError> {
    let mut parser = JsParser::new(body);
    let position = parser
        .find_definition("_SEMESTER_NAMES", b'=')
        .ok_or(DiscoveryTransportError::InvalidSelectorScript)?;
    let object = parser
        .parse_object(position)
        .map_err(|_| DiscoveryTransportError::InvalidSelectorScript)?;
    let mut names = BTreeMap::new();
    for (key, value) in object {
        let code = key
            .parse::<i64>()
            .ok()
            .filter(|code| matches!(code, 0 | 1 | 7 | 9))
            .ok_or(DiscoveryTransportError::InvalidSelectorScript)?;
        let name = value
            .as_text()
            .filter(|value| !value.is_empty())
            .ok_or(DiscoveryTransportError::InvalidSelectorScript)?;
        if names.insert(code, name.to_owned()).is_some() {
            return Err(DiscoveryTransportError::InvalidSelectorScript);
        }
    }
    if names.len() != 4 {
        return Err(DiscoveryTransportError::InvalidSelectorScript);
    }
    Ok(names)
}

fn parse_previous_count(body: &[u8]) -> Result<usize, DiscoveryTransportError> {
    let function = function_body(body, "getSemesters")?;
    let tokens = tokenize_js(function)?;
    let mut counts = Vec::new();
    for window in tokens.windows(6) {
        if window[0] == "i"
            && window[1] == "<"
            && window[3] == ";"
            && window[4] == "i"
            && window[5] == "++"
            && let Ok(count) = window[2].parse::<usize>()
        {
            counts.push(count);
        }
    }
    if counts.len() != 1 || !(1..=32).contains(&counts[0]) {
        return Err(DiscoveryTransportError::InvalidSelectorScript);
    }
    for expected in [
        ["semesters", ".", "push", "(", "current", ")"].as_slice(),
        ["SemesterUtils", ".", "getPreviousSemester", "(", "tmp", ")"].as_slice(),
        ["semesters", ".", "push", "(", "tmp", ")"].as_slice(),
    ] {
        if !contains_tokens(&tokens, expected) {
            return Err(DiscoveryTransportError::InvalidSelectorScript);
        }
    }
    Ok(counts[0])
}

fn parse_previous_transitions(
    body: &[u8],
    names: &BTreeMap<i64, String>,
) -> Result<BTreeMap<i64, PreviousTransition>, DiscoveryTransportError> {
    let function = function_body(body, "getPreviousSemester")?;
    let tokens = tokenize_js(function)?;
    if !contains_tokens(
        &tokens,
        &[
            "return",
            "SemesterUtils",
            ".",
            "buildSemesterObject",
            "(",
            "year",
            ",",
            "term",
            ")",
        ],
    ) {
        return Err(DiscoveryTransportError::InvalidSelectorScript);
    }
    let mut transitions = BTreeMap::new();
    for index in 0..tokens.len().saturating_sub(9) {
        let slice = &tokens[index..];
        if slice[0] != "if"
            || slice[1] != "("
            || slice[2] != "term"
            || slice[3] != "=="
            || slice[5] != ")"
            || slice[6] != "term"
            || slice[7] != "="
        {
            continue;
        }
        let (Ok(from), Ok(to)) = (slice[4].parse::<i64>(), slice[8].parse::<i64>()) else {
            continue;
        };
        if transitions
            .insert(
                from,
                PreviousTransition {
                    next_term: to,
                    decrement_year: false,
                },
            )
            .is_some()
        {
            return Err(DiscoveryTransportError::InvalidSelectorScript);
        }
    }
    let missing: Vec<_> = names
        .keys()
        .filter(|code| !transitions.contains_key(code))
        .copied()
        .collect();
    if missing.len() != 1 {
        return Err(DiscoveryTransportError::InvalidSelectorScript);
    }
    let mut wrap_targets = Vec::new();
    for index in 0..tokens.len().saturating_sub(7) {
        let slice = &tokens[index..];
        if slice[0] == "else"
            && slice[1] == "{"
            && slice[2] == "term"
            && slice[3] == "="
            && slice[5] == ";"
            && slice[6] == "year"
            && slice[7] == "--"
            && let Ok(target) = slice[4].parse::<i64>()
        {
            wrap_targets.push(target);
        }
    }
    if wrap_targets.len() != 1 {
        return Err(DiscoveryTransportError::InvalidSelectorScript);
    }
    transitions.insert(
        missing[0],
        PreviousTransition {
            next_term: wrap_targets[0],
            decrement_year: true,
        },
    );
    if transitions.len() != names.len()
        || transitions
            .values()
            .any(|transition| !names.contains_key(&transition.next_term))
    {
        return Err(DiscoveryTransportError::InvalidSelectorScript);
    }
    Ok(transitions)
}

#[derive(Clone, Debug)]
enum JsScalar {
    Text(String),
    Integer(i64),
    Boolean,
    Null,
}

impl JsScalar {
    fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            Self::Integer(_) | Self::Boolean | Self::Null => None,
        }
    }

    const fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            Self::Text(_) | Self::Boolean | Self::Null => None,
        }
    }
}

struct JsParser<'a> {
    input: &'a [u8],
    position: usize,
}

impl<'a> JsParser<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self { input, position: 0 }
    }

    fn find_definition(&mut self, marker: &str, delimiter: u8) -> Option<usize> {
        let mut found = None;
        self.position = 0;
        while self.position < self.input.len() {
            self.skip_ws_comments().ok()?;
            if self.position >= self.input.len() {
                break;
            }
            if matches!(self.input[self.position], b'\'' | b'"') {
                self.parse_string().ok()?;
                continue;
            }
            if is_identifier_start(self.input[self.position]) {
                let identifier = self.parse_identifier().ok()?;
                let after_identifier = self.position;
                self.skip_ws_comments().ok()?;
                if identifier == marker && self.peek() == Some(delimiter) {
                    self.position += 1;
                    self.skip_ws_comments().ok()?;
                    if found.replace(self.position).is_some() {
                        return None;
                    }
                } else {
                    self.position = after_identifier;
                }
            } else {
                self.position += 1;
            }
        }
        found
    }

    fn parse_array_of_objects(
        &mut self,
        position: usize,
    ) -> Result<Vec<BTreeMap<String, JsScalar>>, ()> {
        self.position = position;
        self.expect(b'[')?;
        let mut objects = Vec::new();
        loop {
            self.skip_ws_comments()?;
            if self.consume(b']') {
                break;
            }
            objects.push(self.parse_object_at_current()?);
            self.skip_ws_comments()?;
            if self.consume(b',') {
                continue;
            }
            self.expect(b']')?;
            break;
        }
        Ok(objects)
    }

    fn parse_object(&mut self, position: usize) -> Result<BTreeMap<String, JsScalar>, ()> {
        self.position = position;
        self.parse_object_at_current()
    }

    fn parse_object_at_current(&mut self) -> Result<BTreeMap<String, JsScalar>, ()> {
        self.expect(b'{')?;
        let mut object = BTreeMap::new();
        loop {
            self.skip_ws_comments()?;
            if self.consume(b'}') {
                break;
            }
            let key = if matches!(self.peek(), Some(b'\'' | b'"')) {
                self.parse_string()?
            } else if self.peek().is_some_and(is_identifier_start) {
                self.parse_identifier()?.to_owned()
            } else {
                self.parse_integer()?.to_string()
            };
            self.skip_ws_comments()?;
            self.expect(b':')?;
            self.skip_ws_comments()?;
            let value = self.parse_scalar()?;
            if object.insert(key, value).is_some() {
                return Err(());
            }
            self.skip_ws_comments()?;
            if self.consume(b',') {
                continue;
            }
            self.expect(b'}')?;
            break;
        }
        Ok(object)
    }

    fn parse_scalar(&mut self) -> Result<JsScalar, ()> {
        match self.peek() {
            Some(b'\'' | b'"') => self.parse_string().map(JsScalar::Text),
            Some(byte) if byte.is_ascii_digit() || byte == b'-' => {
                self.parse_integer().map(JsScalar::Integer)
            }
            Some(byte) if is_identifier_start(byte) => match self.parse_identifier()? {
                "true" | "false" => Ok(JsScalar::Boolean),
                "null" => Ok(JsScalar::Null),
                _ => Err(()),
            },
            _ => Err(()),
        }
    }

    fn parse_string(&mut self) -> Result<String, ()> {
        let quote = self.peek().ok_or(())?;
        if !matches!(quote, b'\'' | b'"') {
            return Err(());
        }
        self.position += 1;
        let mut output = String::new();
        while let Some(byte) = self.peek() {
            self.position += 1;
            if byte == quote {
                return Ok(output);
            }
            if byte == b'\\' {
                let escaped = self.peek().ok_or(())?;
                self.position += 1;
                output.push(match escaped {
                    b'\'' => '\'',
                    b'"' => '"',
                    b'\\' => '\\',
                    b'n' => '\n',
                    b'r' => '\r',
                    b't' => '\t',
                    _ => return Err(()),
                });
            } else if byte.is_ascii() {
                output.push(char::from(byte));
            } else {
                return Err(());
            }
        }
        Err(())
    }

    fn parse_identifier(&mut self) -> Result<&'a str, ()> {
        let start = self.position;
        if !self.peek().is_some_and(is_identifier_start) {
            return Err(());
        }
        self.position += 1;
        while self.peek().is_some_and(is_identifier_continue) {
            self.position += 1;
        }
        std::str::from_utf8(&self.input[start..self.position]).map_err(|_| ())
    }

    fn parse_integer(&mut self) -> Result<i64, ()> {
        let start = self.position;
        if self.peek() == Some(b'-') {
            self.position += 1;
        }
        let digit_start = self.position;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.position += 1;
        }
        if digit_start == self.position {
            return Err(());
        }
        std::str::from_utf8(&self.input[start..self.position])
            .ok()
            .and_then(|value| value.parse().ok())
            .ok_or(())
    }

    fn skip_ws_comments(&mut self) -> Result<(), ()> {
        loop {
            while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
                self.position += 1;
            }
            if self.input.get(self.position..self.position + 2) == Some(b"//") {
                self.position += 2;
                while self
                    .peek()
                    .is_some_and(|byte| !matches!(byte, b'\r' | b'\n'))
                {
                    self.position += 1;
                }
                continue;
            }
            if self.input.get(self.position..self.position + 2) == Some(b"/*") {
                self.position += 2;
                let end = self.input[self.position..]
                    .windows(2)
                    .position(|window| window == b"*/")
                    .ok_or(())?;
                self.position += end + 2;
                continue;
            }
            return Ok(());
        }
    }

    fn expect(&mut self, expected: u8) -> Result<(), ()> {
        self.skip_ws_comments()?;
        if self.consume(expected) {
            Ok(())
        } else {
            Err(())
        }
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.position).copied()
    }
}

fn function_body<'a>(body: &'a [u8], name: &str) -> Result<&'a [u8], DiscoveryTransportError> {
    let mut parser = JsParser::new(body);
    let mut position = parser
        .find_definition(name, b':')
        .ok_or(DiscoveryTransportError::InvalidSelectorScript)?;
    let function = b"function";
    while body.get(position).is_some_and(u8::is_ascii_whitespace) {
        position += 1;
    }
    if body.get(position..position + function.len()) != Some(function) {
        return Err(DiscoveryTransportError::InvalidSelectorScript);
    }
    position += function.len();
    let open = body[position..]
        .iter()
        .position(|byte| *byte == b'{')
        .map(|offset| position + offset)
        .ok_or(DiscoveryTransportError::InvalidSelectorScript)?;
    let close = balanced_end(body, open, b'{', b'}')
        .ok_or(DiscoveryTransportError::InvalidSelectorScript)?;
    Ok(&body[open + 1..close - 1])
}

fn tokenize_js(body: &[u8]) -> Result<Vec<String>, DiscoveryTransportError> {
    let mut parser = JsParser::new(body);
    let mut tokens = Vec::new();
    while parser.position < body.len() {
        parser
            .skip_ws_comments()
            .map_err(|_| DiscoveryTransportError::InvalidSelectorScript)?;
        let Some(byte) = parser.peek() else {
            break;
        };
        if matches!(byte, b'\'' | b'"') {
            parser
                .parse_string()
                .map_err(|_| DiscoveryTransportError::InvalidSelectorScript)?;
        } else if is_identifier_start(byte) {
            tokens.push(
                parser
                    .parse_identifier()
                    .map_err(|_| DiscoveryTransportError::InvalidSelectorScript)?
                    .to_owned(),
            );
        } else if byte.is_ascii_digit()
            || byte == b'-'
                && body
                    .get(parser.position + 1)
                    .is_some_and(u8::is_ascii_digit)
        {
            tokens.push(
                parser
                    .parse_integer()
                    .map_err(|_| DiscoveryTransportError::InvalidSelectorScript)?
                    .to_string(),
            );
        } else {
            let two = body.get(parser.position..parser.position + 2);
            if matches!(two, Some(b"==" | b"++" | b"--")) {
                tokens.push(std::str::from_utf8(two.unwrap()).unwrap().to_owned());
                parser.position += 2;
            } else {
                tokens.push(char::from(byte).to_string());
                parser.position += 1;
            }
        }
    }
    Ok(tokens)
}

fn contains_tokens(tokens: &[String], expected: &[&str]) -> bool {
    tokens.windows(expected.len()).any(|window| {
        window
            .iter()
            .map(String::as_str)
            .eq(expected.iter().copied())
    })
}

fn extract_init_json(body: &[u8]) -> Option<&[u8]> {
    let marker = find_once(body, b"id=\"initJsonData\"")
        .or_else(|| find_once(body, b"id='initJsonData'"))?;
    let content_start = body[marker..].iter().position(|byte| *byte == b'>')? + marker + 1;
    let content_end =
        find_ascii_case_insensitive(&body[content_start..], b"</div>")? + content_start;
    Some(&body[content_start..content_end])
}

fn extract_json_object_member<'a>(body: &'a [u8], name: &[u8]) -> Option<&'a [u8]> {
    let mut marker = Vec::with_capacity(name.len() + 2);
    marker.push(b'"');
    marker.extend_from_slice(name);
    marker.push(b'"');
    let member = find_once(body, &marker)?;
    let mut position = member + marker.len();
    while body.get(position).is_some_and(u8::is_ascii_whitespace) {
        position += 1;
    }
    if body.get(position) != Some(&b':') {
        return None;
    }
    position += 1;
    while body.get(position).is_some_and(u8::is_ascii_whitespace) {
        position += 1;
    }
    if body.get(position) != Some(&b'{') {
        return None;
    }
    let end = balanced_json_object_end(body, position)?;
    Some(&body[position..end])
}

fn extract_soc_utils_script(body: &[u8]) -> Option<String> {
    let mut position = 0;
    let mut matches = Vec::new();
    while let Some(offset) = find_ascii_case_insensitive(&body[position..], b"<script") {
        let start = position + offset;
        let end = body[start..].iter().position(|byte| *byte == b'>')? + start;
        let tag = &body[start..=end];
        if let Some(src) = html_attribute(tag, b"src")
            && src.contains("soc_utils.js")
        {
            matches.push(src);
        }
        position = end + 1;
    }
    (matches.len() == 1).then(|| matches.remove(0))
}

fn html_attribute(tag: &[u8], name: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(tag).ok()?;
    for quote in ['"', '\''] {
        let marker = format!("{}={quote}", std::str::from_utf8(name).ok()?);
        if let Some(start) = text.to_ascii_lowercase().find(&marker) {
            let value_start = start + marker.len();
            let value_end = text[value_start..].find(quote)? + value_start;
            return Some(text[value_start..value_end].to_owned());
        }
    }
    None
}

fn validate_script_url(root: &Url, script: &Url) -> Result<String, DiscoveryTransportError> {
    if !same_origin(root, script)
        || script.path() != "/soc/js/soc_utils.js"
        || script.fragment().is_some()
        || !script.username().is_empty()
        || script.password().is_some()
    {
        return Err(DiscoveryTransportError::InvalidSelectorScriptUrl);
    }
    let pairs: Vec<_> = script.query_pairs().collect();
    if pairs.len() != 1 || pairs[0].0 != "v" {
        return Err(DiscoveryTransportError::InvalidSelectorScriptUrl);
    }
    let version = pairs[0].1.as_ref();
    if version.is_empty()
        || version.len() > 64
        || !version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(DiscoveryTransportError::InvalidSelectorScriptUrl);
    }
    Ok(version.to_owned())
}

fn json_integer(object: &serde_json::Map<String, Value>, key: &str) -> Option<i64> {
    object.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
    })
}

fn valid_iso_date(value: &str) -> bool {
    value.len() == 10
        && value.bytes().enumerate().all(|(index, byte)| {
            matches!(index, 4 | 7) && byte == b'-'
                || !matches!(index, 4 | 7) && byte.is_ascii_digit()
        })
}

fn balanced_json_object_end(body: &[u8], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut position = start;
    let mut in_string = false;
    let mut escaped = false;
    while position < body.len() {
        let byte = body[position];
        position += 1;
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(position);
                }
            }
            _ => {}
        }
    }
    None
}

fn balanced_end(body: &[u8], start: usize, open: u8, close: u8) -> Option<usize> {
    let mut parser = JsParser::new(body);
    parser.position = start;
    let mut depth = 0usize;
    while parser.position < body.len() {
        parser.skip_ws_comments().ok()?;
        let byte = parser.peek()?;
        if matches!(byte, b'\'' | b'"') {
            parser.parse_string().ok()?;
            continue;
        }
        parser.position += 1;
        if byte == open {
            depth += 1;
        } else if byte == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(parser.position);
            }
        }
    }
    None
}

fn find_once(body: &[u8], needle: &[u8]) -> Option<usize> {
    let mut matches = body
        .windows(needle.len())
        .enumerate()
        .filter_map(|(index, window)| (window == needle).then_some(index));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn find_ascii_case_insensitive(body: &[u8], needle: &[u8]) -> Option<usize> {
    body.windows(needle.len()).position(|window| {
        window
            .iter()
            .zip(needle)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
    })
}

const fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$')
}

const fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}

fn valid_endpoint(endpoint: &Url, mode: EndpointMode) -> bool {
    match mode {
        EndpointMode::Official => endpoint.as_str() == RUTGERS_DISCOVERY_ROOT,
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
            last_modified: header_text(headers, LAST_MODIFIED),
        }
    }
}

impl DiscoveryFailureResponseMetadata {
    fn capture(resource: DiscoveryResourceKind, response: &Response) -> Self {
        let headers = response.headers();
        Self {
            resource,
            http_status: response.status().as_u16(),
            decoded_bytes: None,
            decoded_body_sha256: None,
            content_type: header_text(headers, CONTENT_TYPE),
            content_encoding: header_text(headers, CONTENT_ENCODING),
            content_length: content_length(headers),
            etag: header_text(headers, ETAG),
            cache_control: header_text(headers, CACHE_CONTROL),
            date: header_text(headers, DATE),
            last_modified: header_text(headers, LAST_MODIFIED),
        }
    }
}

fn classify_reqwest_error(
    error: reqwest::Error,
    resource: DiscoveryResourceKind,
) -> DiscoveryTransportError {
    if error.is_decode() {
        DiscoveryTransportError::ContentDecoding { resource }
    } else if error.is_timeout() {
        DiscoveryTransportError::Timeout
    } else {
        DiscoveryTransportError::Network
    }
}

fn classify_http_failure(
    response: &Response,
    resource: DiscoveryResourceKind,
) -> DiscoveryTransportError {
    let status = response.status();
    if status.is_redirection() && status != StatusCode::NOT_MODIFIED {
        return DiscoveryTransportError::Redirect {
            resource,
            status: status.as_u16(),
            scope: redirect_scope(response),
        };
    }
    DiscoveryTransportError::NonSuccessHttp {
        resource,
        status: status.as_u16(),
    }
}

fn accepted_content_type(headers: &HeaderMap, resource: DiscoveryResourceKind) -> Option<String> {
    let value = headers.get(CONTENT_TYPE)?.to_str().ok()?;
    let media_type = value.split(';').next()?.trim();
    let accepted = match resource {
        DiscoveryResourceKind::Bootstrap => {
            media_type.eq_ignore_ascii_case("text/html")
                || media_type.eq_ignore_ascii_case("application/json")
        }
        DiscoveryResourceKind::SelectorScript => {
            media_type.eq_ignore_ascii_case("application/javascript")
                || media_type.eq_ignore_ascii_case("text/javascript")
                || media_type.eq_ignore_ascii_case("application/x-javascript")
        }
    };
    accepted.then(|| value.to_owned())
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

    struct FakeResponse {
        status: &'static str,
        content_type: &'static str,
        body: Vec<u8>,
        extra_headers: Vec<(&'static str, String)>,
    }

    impl FakeResponse {
        fn html(body: impl Into<Vec<u8>>) -> Self {
            Self {
                status: "200 OK",
                content_type: "text/html;charset=ISO-8859-1",
                body: body.into(),
                extra_headers: Vec::new(),
            }
        }

        fn javascript(body: impl Into<Vec<u8>>) -> Self {
            Self {
                status: "200 OK",
                content_type: "application/javascript",
                body: body.into(),
                extra_headers: Vec::new(),
            }
        }

        fn status(status: &'static str, content_type: &'static str) -> Self {
            Self {
                status,
                content_type,
                body: Vec::new(),
                extra_headers: Vec::new(),
            }
        }
    }

    fn fake_upstream(responses: Vec<FakeResponse>) -> (Url, Receiver<String>, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback listener");
        let address = listener.local_addr().expect("listener address");
        let (sender, receiver) = channel();
        let handle = thread::spawn(move || {
            for response in responses {
                let (mut stream, _) = listener.accept().expect("fake request");
                let mut request = Vec::new();
                let mut chunk = [0u8; 1024];
                loop {
                    let read = stream.read(&mut chunk).expect("read fake request");
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&chunk[..read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                let _ = sender.send(String::from_utf8(request).expect("ASCII fake request"));
                let mut head = format!(
                    "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                    response.status,
                    response.content_type,
                    response.body.len()
                );
                for (name, value) in response.extra_headers {
                    head.push_str(name);
                    head.push_str(": ");
                    head.push_str(&value);
                    head.push_str("\r\n");
                }
                head.push_str("\r\n");
                stream
                    .write_all(head.as_bytes())
                    .expect("fake response head");
                stream
                    .write_all(&response.body)
                    .expect("fake response body");
            }
        });
        let root = Url::parse(&format!("http://{address}/soc/")).expect("fake root");
        (root, receiver, handle)
    }

    fn test_client(root: Url) -> RutgersDiscoveryClient {
        RutgersDiscoveryClient::build(
            root,
            TransportLimits {
                connect_timeout: Duration::from_secs(1),
                total_timeout: Duration::from_secs(2),
                max_bootstrap_bytes: DISCOVERY_MAX_BOOTSTRAP_BYTES,
                max_selector_bytes: DISCOVERY_MAX_SELECTOR_BYTES,
            },
            EndpointMode::TestLoopback,
        )
        .expect("loopback client")
    }

    fn bootstrap(script_src: &str) -> Vec<u8> {
        format!(
            r#"<!doctype html><html><head><script type="text/javascript" src="{script_src}"></script></head>
            <body><div id="initJsonData" style="display:none;">{{"currentTermDate":{{"date":"2026-03-30","year":2026,"campus":"AC","term":9}},"units":[{{"code":"SYN"}}]}}</div></body></html>"#
        )
        .into_bytes()
    }

    fn selector_script() -> Vec<u8> {
        br#"
            var AppConstants = (function() {
                return {
                    CAMPUSES: [
                        { code:"AC", name:"Mays Landing - RU-ACCC", type:3 },
                        { code:"D", name:"Mercer County Community College", type:3 },
                        // { code:"DISABLED", name:"Commented Out", type:3 },
                        { code:"SYN_FUTURE", name:"Future Dynamic Campus", type:4, parentcode:"AC" }
                    ]
                };
            })();
            var SemesterUtils = (function() {
                var _SEMESTER_NAMES = { 0:"Winter", 1:"Spring", 7:"Summer", 9:"Fall" };
                return {
                    getSemesters: function() {
                        var semesters = [];
                        var item = AppData.initData.currentTermDate;
                        var current = SemesterUtils.buildSemesterObject(item.year, item.term);
                        semesters.push(current);
                        var tmp = current;
                        for (var i = 0; i < 8; i++) {
                            tmp = SemesterUtils.getPreviousSemester(tmp);
                            semesters.push(tmp);
                        }
                        AppData.semesters = semesters;
                        return semesters;
                    },
                    getPreviousSemester: function(semester) {
                        var year = semester.year;
                        var term = semester.term;
                        if (term == 9) term = 7;
                        else if (term == 7) term = 1;
                        else if (term == 1) term = 0;
                        else {
                            term = 9;
                            year--;
                        }
                        return SemesterUtils.buildSemesterObject(year, term);
                    }
                };
            })();
        "#
        .to_vec()
    }

    #[test]
    fn official_client_is_fixed_and_does_not_fetch_during_construction() {
        let client = RutgersDiscoveryClient::new_official().expect("fixed official client");
        assert_eq!(client.root.as_str(), RUTGERS_DISCOVERY_ROOT);
        assert_eq!(DISCOVERY_CONNECT_TIMEOUT, Duration::from_secs(5));
        assert_eq!(DISCOVERY_TOTAL_TIMEOUT, Duration::from_secs(20));
        assert_eq!(DISCOVERY_MAX_BOOTSTRAP_BYTES, 1024 * 1024);
        assert_eq!(DISCOVERY_MAX_SELECTOR_BYTES, 512 * 1024);

        let alternate = Url::parse("https://example.invalid/soc/").unwrap();
        assert!(
            RutgersDiscoveryClient::build(
                alternate,
                TransportLimits::OFFICIAL,
                EndpointMode::Official,
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn fetch_discovers_every_enabled_campus_and_published_target_without_credentials() {
        let script_path = "/soc/js/soc_utils.js?v=future-2026.04.07";
        let (root, captures, handle) = fake_upstream(vec![
            FakeResponse::html(bootstrap(script_path)),
            FakeResponse::javascript(selector_script()),
        ]);
        let response = test_client(root)
            .fetch("SYN_DISCOVERY", "2030-01-01T00:00:00Z")
            .await
            .expect("dynamic selector response");
        let first_request = captures.recv().expect("bootstrap request");
        let second_request = captures.recv().expect("selector request");
        handle.join().expect("fake upstream thread");

        assert!(first_request.starts_with("GET /soc/ HTTP/1.1\r\n"));
        assert!(second_request.starts_with(&format!("GET {script_path} HTTP/1.1\r\n")));
        for request in [&first_request, &second_request] {
            let lower = request.to_ascii_lowercase();
            assert!(!lower.contains("authorization:"));
            assert!(!lower.contains("cookie:"));
            assert!(!lower.contains("referer:"));
        }

        let snapshot = response.snapshot();
        assert_eq!(snapshot.terms.len(), 9);
        assert_eq!(snapshot.campuses.len(), 3);
        assert_eq!(snapshot.targets.len(), 27);
        assert!(snapshot.subjects.is_empty());
        assert!(
            snapshot
                .campuses
                .iter()
                .any(|campus| campus.campus_code.as_str() == "D")
        );
        assert!(
            snapshot
                .campuses
                .iter()
                .any(|campus| campus.campus_code.as_str() == "SYN_FUTURE")
        );
        assert!(
            snapshot
                .campuses
                .iter()
                .all(|campus| campus.campus_code.as_str() != "DISABLED")
        );
        assert!(
            snapshot
                .terms
                .iter()
                .any(|term| term.term_id.as_str() == "92024")
        );
        assert!(
            snapshot
                .targets
                .iter()
                .any(|target| target.key.term().as_str() == "92026"
                    && target.key.campus().as_str() == "D")
        );
        assert_eq!(response.metadata().source_version, "future-2026.04.07");
        assert_eq!(
            response.source_provenance().decoded_bytes,
            bootstrap(script_path).len() + selector_script().len()
        );
        assert_eq!(response.source_provenance().body_sha256.len(), 64);
        assert_eq!(
            response.metadata().selector_script.decoded_body_sha256,
            sha256_hex(&selector_script())
        );
    }

    #[tokio::test]
    async fn off_origin_selector_script_is_rejected_before_a_second_request() {
        let (root, captures, handle) = fake_upstream(vec![FakeResponse::html(bootstrap(
            "https://evil.invalid/soc/js/soc_utils.js?v=bad",
        ))]);
        let failure = test_client(root)
            .fetch("SYN_OFF_ORIGIN", "2030-01-01T00:00:00Z")
            .await
            .expect_err("off-origin selector must fail");
        captures.recv().expect("only bootstrap request");
        handle.join().expect("fake upstream thread");
        assert_eq!(
            failure.kind(),
            &DiscoveryTransportError::InvalidSelectorScriptUrl
        );
        assert_eq!(
            failure
                .response_metadata()
                .map(|metadata| metadata.resource),
            Some(DiscoveryResourceKind::Bootstrap)
        );
    }

    #[tokio::test]
    async fn malformed_selector_entry_fails_instead_of_silently_omitting_a_campus() {
        let malformed = String::from_utf8(selector_script()).unwrap().replace(
            "{ code:\"D\", name:\"Mercer County Community College\", type:3 }",
            "{ name:\"Mercer County Community College\", type:3 }",
        );
        let (root, _, handle) = fake_upstream(vec![
            FakeResponse::html(bootstrap("/soc/js/soc_utils.js?v=bad-format")),
            FakeResponse::javascript(malformed.into_bytes()),
        ]);
        let failure = test_client(root)
            .fetch("SYN_BAD_FORMAT", "2030-01-01T00:00:00Z")
            .await
            .expect_err("malformed campus must fail closed");
        handle.join().expect("fake upstream thread");
        assert_eq!(
            failure.kind(),
            &DiscoveryTransportError::InvalidSelectorScript
        );
    }

    #[tokio::test]
    async fn response_limits_apply_to_bootstrap_and_selector_decoded_bodies() {
        let large_bootstrap = vec![b'x'; 65];
        let (root, _, handle) = fake_upstream(vec![FakeResponse::html(large_bootstrap)]);
        let client = RutgersDiscoveryClient::build(
            root,
            TransportLimits {
                max_bootstrap_bytes: 64,
                ..TransportLimits::OFFICIAL
            },
            EndpointMode::TestLoopback,
        )
        .unwrap();
        let bootstrap_failure = client
            .fetch("SYN_BIG_BOOTSTRAP", "2030-01-01T00:00:00Z")
            .await
            .expect_err("oversize bootstrap");
        handle.join().expect("fake upstream thread");
        assert_eq!(
            bootstrap_failure.kind(),
            &DiscoveryTransportError::ResponseTooLarge {
                resource: DiscoveryResourceKind::Bootstrap
            }
        );

        let script = selector_script();
        let (root, _, handle) = fake_upstream(vec![
            FakeResponse::html(bootstrap("/soc/js/soc_utils.js?v=large")),
            FakeResponse::javascript(script.clone()),
        ]);
        let client = RutgersDiscoveryClient::build(
            root,
            TransportLimits {
                max_selector_bytes: script.len() - 1,
                ..TransportLimits::OFFICIAL
            },
            EndpointMode::TestLoopback,
        )
        .unwrap();
        let selector_failure = client
            .fetch("SYN_BIG_SELECTOR", "2030-01-01T00:00:00Z")
            .await
            .expect_err("oversize selector");
        handle.join().expect("fake upstream thread");
        assert_eq!(
            selector_failure.kind(),
            &DiscoveryTransportError::ResponseTooLarge {
                resource: DiscoveryResourceKind::SelectorScript
            }
        );
    }

    #[tokio::test]
    async fn non_success_status_is_redacted_for_either_resource() {
        let (root, _, handle) = fake_upstream(vec![FakeResponse::status(
            "503 Service Unavailable",
            "text/html",
        )]);
        let bootstrap_failure = test_client(root)
            .fetch("SYN_STATUS_ROOT", "2030-01-01T00:00:00Z")
            .await
            .expect_err("bootstrap status");
        handle.join().expect("fake upstream thread");
        assert_eq!(
            bootstrap_failure.kind(),
            &DiscoveryTransportError::NonSuccessHttp {
                resource: DiscoveryResourceKind::Bootstrap,
                status: 503,
            }
        );

        let (root, _, handle) = fake_upstream(vec![
            FakeResponse::html(bootstrap("/soc/js/soc_utils.js?v=status")),
            FakeResponse::status("500 Internal Server Error", "application/javascript"),
        ]);
        let selector_failure = test_client(root)
            .fetch("SYN_STATUS_SCRIPT", "2030-01-01T00:00:00Z")
            .await
            .expect_err("selector status");
        handle.join().expect("fake upstream thread");
        assert_eq!(
            selector_failure.kind(),
            &DiscoveryTransportError::NonSuccessHttp {
                resource: DiscoveryResourceKind::SelectorScript,
                status: 500,
            }
        );
        let metadata = selector_failure
            .response_metadata()
            .expect("redacted metadata");
        assert_eq!(metadata.http_status, 500);
        assert_eq!(metadata.decoded_body_sha256, None);
    }
}
