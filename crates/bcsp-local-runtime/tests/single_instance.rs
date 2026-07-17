use std::fs;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use bcsp_local_runtime::{
    LocalInstanceClaim, LocalRuntimePaths, LoopbackBrowserUrl, PrimaryInstanceLease,
};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "rbcsp-local-instance-{label}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn package(temp: &TestDirectory) -> LocalRuntimePaths {
    let root = temp.path().join("RBCSP package");
    fs::create_dir_all(&root).unwrap();
    let executable = root.join("RBCSP.exe");
    fs::write(&executable, b"test executable").unwrap();
    LocalRuntimePaths::from_executable(executable).unwrap()
}

fn primary(claim: LocalInstanceClaim) -> PrimaryInstanceLease {
    match claim {
        LocalInstanceClaim::Primary(lease) => lease,
        LocalInstanceClaim::Secondary(_) => panic!("expected the first process slot"),
    }
}

#[test]
fn first_instance_publishes_a_healthy_url_for_the_second_instance() {
    let temp = TestDirectory::new("first-second");
    let paths = package(&temp);
    let mut first = primary(LocalInstanceClaim::acquire(&paths).unwrap());
    assert!(!paths.database().exists());

    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let responder = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut request = [0_u8; 512];
        let count = stream.read(&mut request).unwrap();
        assert!(String::from_utf8_lossy(&request[..count]).starts_with("GET / HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
        stream.flush().unwrap();
    });

    let expected = format!("http://127.0.0.1:{port}/");
    first.publish_browser_url(&expected).unwrap();
    let second = match LocalInstanceClaim::acquire(&paths).unwrap() {
        LocalInstanceClaim::Secondary(instance) => instance,
        LocalInstanceClaim::Primary(_) => panic!("a second process must not own the slot"),
    };
    assert!(!paths.database().exists());
    let actual = second
        .wait_for_healthy_browser_url(Duration::from_secs(2))
        .unwrap();
    assert_eq!(actual.as_str(), expected);
    responder.join().unwrap();
}

#[test]
fn process_slot_can_be_reacquired_after_the_primary_handle_is_released() {
    let temp = TestDirectory::new("reacquire");
    let paths = package(&temp);
    let first = primary(LocalInstanceClaim::acquire(&paths).unwrap());
    let lock_path = first.lock_path().to_path_buf();
    assert!(lock_path.is_file());
    drop(first);

    let replacement = primary(LocalInstanceClaim::acquire(&paths).unwrap());
    assert_eq!(replacement.lock_path(), lock_path);
    drop(replacement);
    assert!(!lock_path.exists());
}

#[test]
fn browser_url_parser_only_accepts_the_canonical_ipv4_loopback_form() {
    assert!(LoopbackBrowserUrl::parse("http://127.0.0.1:49152/").is_ok());
    for invalid in [
        "http://localhost:49152/",
        "http://127.0.0.1:0/",
        "http://127.0.0.1:049152/",
        "http://127.0.0.1:49152",
        "http://127.0.0.1:49152/?session=x",
        "https://127.0.0.1:49152/",
        " http://127.0.0.1:49152/",
    ] {
        assert!(
            LoopbackBrowserUrl::parse(invalid).is_err(),
            "accepted {invalid}"
        );
    }
}
