//! Windows-local composition root. Product startup is implemented later.

#![forbid(unsafe_code)]
#![deny(warnings)]

fn main() {
    let _ = bcsp_local_runtime::boundary_marker();
}
