use std::net::SocketAddr;
use std::time::Duration;

use axum::http::Uri;
use bcsp_application::{RefreshPolicy, RefreshPolicyError};
use bcsp_open::{
    ACTIVE_WATCH_OPEN_INTERVAL_SECONDS, GeneralOpenInterval,
    PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS as SHARED_PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS,
};
use thiserror::Error;

pub const PUBLIC_BIND_ADDRESS: &str = "127.0.0.1:8080";
pub const PUBLIC_CATALOG_INTERVAL_SECONDS: u64 = 600;
pub const PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS: u32 = SHARED_PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS;
pub const PUBLIC_WATCHED_OPEN_INTERVAL_SECONDS: u32 = ACTIVE_WATCH_OPEN_INTERVAL_SECONDS;
pub const PUBLIC_ORIGIN_ENVIRONMENT: &str = "BCSP_PUBLIC_ORIGIN";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicHostConfig {
    bind: SocketAddr,
    external_origin: String,
    external_authority: String,
    release: String,
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
        if uri.scheme_str() != Some("https")
            || uri.authority().is_none()
            || !matches!(uri.path(), "" | "/")
            || uri.query().is_some()
        {
            return Err(PublicHostConfigError::InvalidExternalOrigin);
        }
        let external_authority = uri
            .authority()
            .expect("authority checked above")
            .as_str()
            .to_owned();
        let external_origin = external_origin.trim_end_matches('/').to_owned();
        let release = release.into();
        if release.is_empty() || release.chars().any(char::is_control) {
            return Err(PublicHostConfigError::InvalidRelease);
        }
        Ok(Self {
            bind,
            external_origin,
            external_authority,
            release,
        })
    }

    pub fn from_production_environment() -> Result<Self, PublicHostConfigError> {
        let external_origin = std::env::var(PUBLIC_ORIGIN_ENVIRONMENT)
            .map_err(|_| PublicHostConfigError::MissingExternalOrigin)?;
        let bind = PUBLIC_BIND_ADDRESS
            .parse()
            .map_err(|_| PublicHostConfigError::InvalidBind)?;
        Self::try_new(bind, external_origin, env!("CARGO_PKG_VERSION"))
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
    RefreshPolicy::try_new(
        Duration::from_secs(PUBLIC_CATALOG_INTERVAL_SECONDS),
        GeneralOpenInterval::public(),
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
}

impl PublicHostConfigError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::MissingExternalOrigin => "PUBLIC_EXTERNAL_ORIGIN_REQUIRED",
            Self::InvalidExternalOrigin => "PUBLIC_EXTERNAL_ORIGIN_INVALID",
            Self::InvalidBind => "PUBLIC_BIND_INVALID",
            Self::NonLoopbackBind => "PUBLIC_BIND_NOT_LOOPBACK",
            Self::InvalidRelease => "PUBLIC_RELEASE_INVALID",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
