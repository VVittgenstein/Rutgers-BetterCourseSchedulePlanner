//! Compile-only dependency contract for the P7 Rust workspace.
//!
//! Product modules replace this crate in P7.1-003. Keeping explicit crate
//! references here makes the P7.1-002 dependency graph mechanically testable
//! without introducing application behavior.

#![forbid(unsafe_code)]
#![deny(warnings)]

mod compile_contract {
    use axum as _;
    use include_dir as _;
    use jiff as _;
    use open as _;
    use reqwest as _;
    use rusqlite as _;
    use serde as _;
    use serde_json as _;
    use sha2 as _;
    use thiserror as _;
    use time as _;
    use tokio as _;
    use tower as _;
    use tower_http as _;
    use tracing as _;
    use tracing_subscriber as _;
    use uuid as _;
}

#[cfg(test)]
mod dev_compile_contract {
    use proptest as _;
    use tempfile as _;
}
