//! The packaging fixture seeder, and its own negative control.
//!
//! The Windows release rehearsal has to prove that a real candidate restores
//! stored intent and puts a REAL watch behind it. That needs published
//! catalog data on a product campus in the current term, released by the
//! integrity gate -- and it must not come from Rutgers, must be identical on
//! every run, and must not ship inside the twelve-file archive.
//!
//! So it is seeded from outside, between two lifetimes of the candidate,
//! into the package-local database the candidate already owns. This is a
//! harness-free integration test target: run with no arguments it seeds a
//! throwaway database and checks its own work, which is what `cargo test`
//! does with it; run with `--executable` it seeds the candidate the release
//! script is about to restart. Either way it is a test artifact and never
//! reaches the package.

use std::path::PathBuf;

use bcsp_contracts::SectionKey;
use bcsp_domain::{RutgersTermWindow, RutgersTermWindowScope};
use bcsp_local_runtime::PreparedLocalRuntime;
use bcsp_open::project_current_open_observation;
use bcsp_watch::WatchStartAdmission;
use time::OffsetDateTime;

mod support;

struct Arguments {
    executable: PathBuf,
    term: String,
}

fn main() {
    match parse(std::env::args().skip(1).collect::<Vec<_>>()) {
        Some(arguments) => seed(&arguments),
        // `cargo test` runs this target with no arguments. Seeding a
        // throwaway database and then asserting the section really became
        // admissible is what keeps the release rehearsal's fixture honest:
        // a fixture that silently stopped publishing would otherwise be
        // discovered only by a failing packaging run, on a candidate.
        None => self_check(),
    }
}

fn parse(arguments: Vec<String>) -> Option<Arguments> {
    let mut executable = None;
    let mut term = None;
    let mut remaining = arguments.into_iter();
    while let Some(argument) = remaining.next() {
        match argument.as_str() {
            "--executable" => executable = remaining.next(),
            "--term" => term = remaining.next(),
            other => panic!("unknown fixture argument: {other}"),
        }
    }
    match (executable, term) {
        (Some(executable), Some(term)) => Some(Arguments {
            executable: PathBuf::from(executable),
            term,
        }),
        (None, None) => None,
        _ => panic!("--executable and --term must be supplied together"),
    }
}

fn seed(arguments: &Arguments) {
    let prepared = PreparedLocalRuntime::from_executable(&arguments.executable)
        .expect("open the package-local database beside the candidate");
    support::seed_ready_query_scope(&prepared, &[arguments.term.as_str()]);
    let section = section(&arguments.term);
    assert_admissible(&prepared, &section);
    println!(
        "seeded {}/{}/{} into {}",
        section.term().as_str(),
        section.campus().as_str(),
        section.index().as_str(),
        prepared.paths().database().display(),
    );
}

fn self_check() {
    let directory = tempfile::TempDir::new().expect("fixture scratch directory");
    let root = directory.path().join("candidate");
    std::fs::create_dir_all(&root).expect("fixture package root");
    let executable = root.join("RBCSP.exe");
    std::fs::write(&executable, b"fixture").expect("fixture executable placeholder");
    let window = RutgersTermWindow::at(OffsetDateTime::now_utc(), RutgersTermWindowScope::Public)
        .expect("the bundled calendar covers today");
    let term = window.current_term().as_str().to_owned();

    let prepared = PreparedLocalRuntime::from_executable(&executable).expect("prepared runtime");
    let section = section(&term);
    let control = admission(&prepared, &section);
    assert!(
        !matches!(control, WatchStartAdmission::Admitted { .. }),
        "the control: an empty database publishes nothing, got {control:?}",
    );

    support::seed_ready_query_scope(&prepared, &[term.as_str()]);
    assert_admissible(&prepared, &section);
}

fn section(term: &str) -> SectionKey {
    SectionKey::try_new(
        term,
        support::FIXTURE_CAMPUS,
        support::FIXTURE_SECTION_INDEX,
    )
    .expect("fixture Section identity")
}

/// The whole point of the fixture, asserted rather than assumed: this exact
/// Section can be armed right now, by the same projection the runtime's watch
/// admission consults.
fn assert_admissible(prepared: &PreparedLocalRuntime, section: &SectionKey) {
    let verdict = admission(prepared, section);
    assert!(
        matches!(verdict, WatchStartAdmission::Admitted { .. }),
        "the fixture must leave {}/{}/{} armable, got {verdict:?}",
        section.term().as_str(),
        section.campus().as_str(),
        section.index().as_str(),
    );
}

fn admission(prepared: &PreparedLocalRuntime, section: &SectionKey) -> WatchStartAdmission {
    let snapshot = prepared
        .open_runtime()
        .snapshot(&section.target())
        .expect("Open runtime snapshot");
    let runtime = prepared
        .core()
        .projection_runtime(&snapshot)
        .expect("projection runtime");
    let database = prepared.operational().database();
    let mut database = database.lock().expect("package-local database");
    match project_current_open_observation(database.operational_mut(), section, &runtime) {
        Ok(observation) => WatchStartAdmission::admitted(observation),
        Err(bcsp_open::OpenProjectionError::SectionNotPublished) => {
            WatchStartAdmission::SectionNotFound
        }
        Err(_) => WatchStartAdmission::TargetUnavailable,
    }
}
