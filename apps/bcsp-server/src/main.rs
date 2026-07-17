//! Linux-public composition root.

#![forbid(unsafe_code)]
#![deny(warnings)]

#[tokio::main]
async fn main() {
    if let Err(error) = bcsp_public_runtime::run_production().await {
        tracing::error!(code = error.code(), message = %error);
        std::process::exit(1);
    }
}
