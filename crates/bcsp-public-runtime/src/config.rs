use std::net::SocketAddr;
use std::time::Duration;

use axum::http::Uri;
use bcsp_application::{RefreshPolicy, RefreshPolicyError};
use bcsp_open::{
    GeneralOpenInterval, OpenRefreshIntervals,
    PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS as SHARED_PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS,
    PUBLIC_WATCH_OPEN_INTERVAL_SECONDS, WatchOpenInterval,
};
use thiserror::Error;

pub const PUBLIC_BIND_ADDRESS: &str = "127.0.0.1:8080";
pub const PUBLIC_CATALOG_INTERVAL_SECONDS: u64 = 600;
pub const PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS: u32 = SHARED_PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS;
pub const PUBLIC_WATCHED_OPEN_INTERVAL_SECONDS: u32 = PUBLIC_WATCH_OPEN_INTERVAL_SECONDS;
pub const PUBLIC_ORIGIN_ENVIRONMENT: &str = "BCSP_PUBLIC_ORIGIN";
/// Optional override for the per-client active WebSocket cap (H4). The only
/// configurable H4 number: campus NAT or a shared IPv6 /64 can legitimately
/// need more than the launch default of 64, so this one value is tunable in
/// `1..=1024`. Every other H4 constant stays frozen in `crate::capacity`.
pub const PUBLIC_WS_PER_CLIENT_LIMIT_ENVIRONMENT: &str = "BCSP_PUBLIC_WS_PER_CLIENT_LIMIT";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicHostConfig {
    bind: SocketAddr,
    external_origin: String,
    external_authority: String,
    release: String,
    per_client_ws_limit: u32,
}

impl PublicHostConfig {
    pub fn try_new(
        bind: SocketAddr,
        external_origin: impl Into<String>,
        release: impl Into<String>,
    ) -> Result<Self, PublicHostConfigError> {
        if !bind.ip().is_loopback() {
            return Err(PublicHostConfigError::NonLoopbackBind);
        }
        let external_origin = external_origin.into();
        let uri = external_origin
            .parse::<Uri>()
            .map_err(|_| PublicHostConfigError::InvalidExternalOrigin)?;
        if !uri
            .scheme_str()
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https"))
            || uri.authority().is_none()
            || !matches!(uri.path(), "" | "/")
            || uri.query().is_some()
        {
            return Err(PublicHostConfigError::InvalidExternalOrigin);
        }
        // H7: DNS hosts are case-insensitive, so the authority is stored in
        // its canonical lowercase form and the origin is REBUILT from that
        // canonical authority rather than trimmed from the input. An
        // operator's `https://Planner.Example` and the browser's
        // `https://planner.example` therefore meet at one stored value;
        // nothing else about the origin (scheme identity, port, empty
        // path, no query) is relaxed.
        let external_authority = uri
            .authority()
            .expect("authority checked above")
            .as_str()
            .to_ascii_lowercase();
        let external_origin = format!("https://{external_authority}");
        let release = release.into();
        if release.is_empty() || release.chars().any(char::is_control) {
            return Err(PublicHostConfigError::InvalidRelease);
        }
        Ok(Self {
            bind,
            external_origin,
            external_authority,
            release,
            per_client_ws_limit: crate::capacity::DEFAULT_PER_CLIENT_WS_CONNECTIONS,
        })
    }

    pub fn from_production_environment() -> Result<Self, PublicHostConfigError> {
        let external_origin = std::env::var(PUBLIC_ORIGIN_ENVIRONMENT)
            .map_err(|_| PublicHostConfigError::MissingExternalOrigin)?;
        let bind = PUBLIC_BIND_ADDRESS
            .parse()
            .map_err(|_| PublicHostConfigError::InvalidBind)?;
        let mut config = Self::try_new(bind, external_origin, env!("CARGO_PKG_VERSION"))?;
        // Absent means the default; present means it must parse and land in
        // range. A malformed override fails startup rather than silently
        // running with a limit nobody chose.
        if let Ok(raw) = std::env::var(PUBLIC_WS_PER_CLIENT_LIMIT_ENVIRONMENT) {
            config = config.try_with_per_client_ws_limit(&raw)?;
        }
        Ok(config)
    }

    /// Applies the one tunable H4 value, refusing anything outside the
    /// frozen `1..=1024` range.
    pub fn try_with_per_client_ws_limit(
        mut self,
        raw: &str,
    ) -> Result<Self, PublicHostConfigError> {
        let value = raw
            .trim()
            .parse::<u32>()
            .map_err(|_| PublicHostConfigError::InvalidPerClientWsLimit)?;
        if !crate::capacity::PER_CLIENT_WS_LIMIT_RANGE.contains(&value) {
            return Err(PublicHostConfigError::InvalidPerClientWsLimit);
        }
        self.per_client_ws_limit = value;
        Ok(self)
    }

    pub const fn per_client_ws_limit(&self) -> u32 {
        self.per_client_ws_limit
    }

    pub const fn bind(&self) -> SocketAddr {
        self.bind
    }

    pub fn external_origin(&self) -> &str {
        &self.external_origin
    }

    pub fn external_authority(&self) -> &str {
        &self.external_authority
    }

    pub fn release(&self) -> &str {
        &self.release
    }
}

pub fn fixed_public_refresh_policy() -> Result<RefreshPolicy, RefreshPolicyError> {
    RefreshPolicy::try_new_with_intervals(
        Duration::from_secs(PUBLIC_CATALOG_INTERVAL_SECONDS),
        OpenRefreshIntervals::new(GeneralOpenInterval::public(), WatchOpenInterval::public()),
    )
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum PublicHostConfigError {
    #[error("public external origin is required")]
    MissingExternalOrigin,
    #[error("public external origin is invalid")]
    InvalidExternalOrigin,
    #[error("public server bind address is invalid")]
    InvalidBind,
    #[error("public server must bind to a loopback address")]
    NonLoopbackBind,
    #[error("public release identifier is invalid")]
    InvalidRelease,
    #[error("public per-client WebSocket limit is invalid")]
    InvalidPerClientWsLimit,
}

impl PublicHostConfigError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::MissingExternalOrigin => "PUBLIC_EXTERNAL_ORIGIN_REQUIRED",
            Self::InvalidExternalOrigin => "PUBLIC_EXTERNAL_ORIGIN_INVALID",
            Self::InvalidBind => "PUBLIC_BIND_INVALID",
            Self::NonLoopbackBind => "PUBLIC_BIND_NOT_LOOPBACK",
            Self::InvalidRelease => "PUBLIC_RELEASE_INVALID",
            Self::InvalidPerClientWsLimit => "PUBLIC_WS_PER_CLIENT_LIMIT_INVALID",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_uppercase_origin_is_stored_in_its_canonical_lowercase_form() {
        let shouted = PublicHostConfig::try_new(
            "127.0.0.1:0".parse().expect("loopback address"),
            "HTTPS://Planner.EXAMPLE.Test",
            "test-release",
        )
        .expect("uppercase origin must configure");
        assert_eq!(shouted.external_authority(), "planner.example.test");
        assert_eq!(shouted.external_origin(), "https://planner.example.test");
        let with_port = PublicHostConfig::try_new(
            "127.0.0.1:0".parse().expect("loopback address"),
            "https://Planner.Example.Test:8443",
            "test-release",
        )
        .expect("origin with a port must configure");
        assert_eq!(with_port.external_authority(), "planner.example.test:8443");
        assert_eq!(
            with_port.external_origin(),
            "https://planner.example.test:8443",
            "the port survives canonicalization untouched",
        );
    }

    #[test]
    fn configuration_keeps_the_public_listener_loopback_only() {
        let valid = PublicHostConfig::try_new(
            "127.0.0.1:0".parse().expect("loopback address"),
            "https://planner.example.test/",
            "test-release",
        )
        .expect("valid public host");
        assert_eq!(valid.external_origin(), "https://planner.example.test");
        assert_eq!(valid.external_authority(), "planner.example.test");
        assert!(matches!(
            PublicHostConfig::try_new(
                "0.0.0.0:8080".parse().expect("wildcard address"),
                "https://planner.example.test",
                "test-release"
            ),
            Err(PublicHostConfigError::NonLoopbackBind)
        ));
        assert!(matches!(
            PublicHostConfig::try_new(
                "127.0.0.1:8080".parse().expect("loopback address"),
                "https://planner.example.test/path",
                "test-release"
            ),
            Err(PublicHostConfigError::InvalidExternalOrigin)
        ));
        assert!(matches!(
            PublicHostConfig::try_new(
                "127.0.0.1:8080".parse().expect("loopback address"),
                "http://planner.example.test",
                "test-release"
            ),
            Err(PublicHostConfigError::InvalidExternalOrigin)
        ));
    }

    #[test]
    fn the_per_client_ws_limit_is_tunable_only_inside_its_frozen_range() {
        let base = PublicHostConfig::try_new(
            "127.0.0.1:0".parse().expect("loopback address"),
            "https://planner.example.test",
            "test-release",
        )
        .expect("valid public host");
        assert_eq!(base.per_client_ws_limit(), 64, "the launch default");
        assert_eq!(
            base.clone()
                .try_with_per_client_ws_limit("1")
                .expect("floor of the range")
                .per_client_ws_limit(),
            1
        );
        assert_eq!(
            base.clone()
                .try_with_per_client_ws_limit(" 1024 ")
                .expect("ceiling of the range")
                .per_client_ws_limit(),
            1024
        );
        for invalid in ["0", "1025", "", "sixty-four", "-1", "64.5"] {
            assert!(
                matches!(
                    base.clone().try_with_per_client_ws_limit(invalid),
                    Err(PublicHostConfigError::InvalidPerClientWsLimit)
                ),
                "{invalid:?} must be refused",
            );
        }
    }

    #[test]
    fn public_refresh_policy_has_no_input_and_is_exactly_fixed() {
        let policy = fixed_public_refresh_policy().expect("fixed public policy");
        assert_eq!(policy.catalog_interval(), Duration::from_secs(600));
        assert_eq!(policy.open_general_interval().seconds(), 30);
        assert_eq!(
            policy.effective_open_interval(false),
            Duration::from_secs(30)
        );
        assert_eq!(
            policy.effective_open_interval(true),
            Duration::from_secs(10)
        );
        assert_eq!(PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS, 30);
        assert_eq!(PUBLIC_WATCHED_OPEN_INTERVAL_SECONDS, 10);
    }
}
