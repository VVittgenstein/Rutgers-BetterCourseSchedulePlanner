//! Windows-local composition root.

#![forbid(unsafe_code)]
#![deny(warnings)]

fn main() -> std::process::ExitCode {
    match bcsp_local_runtime::run_blocking() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!(
                "{}",
                bcsp_local_runtime::StartupFailureReport::from_error(&error)
            );
            std::process::ExitCode::FAILURE
        }
    }
}
