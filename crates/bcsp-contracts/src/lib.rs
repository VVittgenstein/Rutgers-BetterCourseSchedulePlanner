//! Shared wire-contract boundary.
//!
//! P7.1-003 establishes only the package boundary. Typed contracts arrive in
//! the task that owns domain and API schemas.

#![forbid(unsafe_code)]
#![deny(warnings)]

pub const PACKAGE_BOUNDARY: &str = "bcsp-contracts";

mod dependency_contract {
    use serde as _;
    use serde_json as _;
    use thiserror as _;
    use time as _;
    use uuid as _;
}
