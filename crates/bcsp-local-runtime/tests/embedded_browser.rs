use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use bcsp_application::{RouteExtension, WebSocketExtension, spawn_loopback_server_with_socket};
use bcsp_local_runtime::PreparedLocalRuntime;

fn required_environment(name: &str) -> String {
    std::env::var(name)
        .unwrap_or_else(|_| panic!("{name} is required for the embedded browser test"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires the locked Node runtime and a real Chromium-family browser"]
async fn real_browser_loads_embedded_ui_api_and_websocket_from_rust() {
    let temp = tempfile::tempdir().unwrap();
    let package_root = temp.path().join("package");
    std::fs::create_dir_all(&package_root).unwrap();
    let executable = package_root.join("RBCSP.exe");
    std::fs::write(&executable, b"test executable anchor").unwrap();

    let prepared = PreparedLocalRuntime::from_executable(executable).unwrap();
    let extension: Arc<dyn RouteExtension> = prepared.route_extension();
    let socket: Arc<dyn WebSocketExtension> = prepared.watch_socket();
    let server = spawn_loopback_server_with_socket(extension, socket, prepared.nonce().clone())
        .await
        .unwrap();

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let frontend = manifest.join("../../frontend");
    let script = frontend.join("tools/verify-embedded-local-runtime.mjs");
    let output = Command::new(required_environment("BCSP_NODE_EXECUTABLE"))
        .arg(script)
        .current_dir(frontend)
        .env("BCSP_EMBEDDED_RUNTIME_URL", server.origin())
        .env(
            "BCSP_BROWSER_EXECUTABLE",
            required_environment("BCSP_BROWSER_EXECUTABLE"),
        )
        .output()
        .unwrap();

    server.shutdown().await.unwrap();

    assert!(
        output.status.success(),
        "browser smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
