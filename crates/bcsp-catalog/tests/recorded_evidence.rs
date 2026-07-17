use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use bcsp_catalog::{
    NormalizationError, normalize_target, to_catalog_refresh_command, to_normalized_catalog_v1,
};
use bcsp_contracts::{CatalogFieldKnowledge, CatalogUnknownReason, TermCampusKey, TraceId};
use bcsp_operational_storage::{EmptySnapshotDecision, OperationalStorage};
use bcsp_rutgers_client::{SourceProvenance, decode_catalog_payload, sha256_hex, sha256_v1};

const REGISTER_RELATIVE_PATH: &str =
    "project-governance/current/p3/03-catalog-evidence-register.tsv";
const RAW_ROOT_RELATIVE_PATH: &str = "data/staging/p3-rutgers-evidence-20260713/catalog";
const RAW_PATH_PREFIX: &str = "data/staging/p3-rutgers-evidence-20260713/catalog/";

#[derive(Default)]
struct Aggregate {
    scopes: usize,
    nonempty_scopes: usize,
    empty_scopes: usize,
    courses: usize,
    raw_sections: usize,
    unique_sections: usize,
    equivalent_duplicate_keys: usize,
    conflicting_duplicate_keys: usize,
    cross_variant_section_overlap: usize,
    meetings: usize,
    instructor_assignments: usize,
    multi_object_groups: usize,
    multi_variant_groups: usize,
    delivery_conflicts: usize,
}

#[test]
#[ignore = "opt-in controlled replay; requires P3_CATALOG_EVIDENCE_ROOT and P3_CATALOG_EVIDENCE_REGISTER"]
fn replays_registered_p3_catalog_evidence_as_aggregate_counts_only() {
    let root = controlled_repository_root();
    let register = controlled_register_path(&root);
    let raw_root = controlled_raw_root(&root);
    let register_text = std::fs::read_to_string(&register)
        .unwrap_or_else(|_| panic!("evidence register is unavailable"));
    let mut lines = register_text.lines();
    let header = lines
        .next()
        .unwrap_or_else(|| panic!("evidence register is empty"))
        .split('\t')
        .collect::<Vec<_>>();
    const REQUIRED_COLUMNS: [&str; 7] = [
        "request_id",
        "term_id",
        "campus",
        "raw_relative_path",
        "body_sha256",
        "observed_at_utc",
        "evidence_id",
    ];
    for required in REQUIRED_COLUMNS {
        assert_eq!(
            header.iter().filter(|column| **column == required).count(),
            1,
            "evidence register required column must occur exactly once"
        );
    }
    let mut aggregate = Aggregate::default();
    let mut verified_body_hashes = Vec::new();
    let mut normalized_content_hashes = Vec::new();
    let mut request_ids = BTreeSet::new();
    let mut target_keys = BTreeSet::new();
    let mut canonical_paths = BTreeSet::new();

    for line in lines.filter(|line| !line.trim().is_empty()) {
        let values = line.split('\t').collect::<Vec<_>>();
        assert_eq!(
            values.len(),
            header.len(),
            "evidence register row width differs from its header"
        );
        let row = header
            .iter()
            .copied()
            .zip(values)
            .collect::<BTreeMap<_, _>>();
        let relative_path = required_column(&row, "raw_relative_path");
        let relative_path = validate_raw_relative_path(relative_path)
            .unwrap_or_else(|_| panic!("registered evidence path violates the fixed policy"));
        let candidate = root.join(relative_path);
        require_regular_file(&candidate, "registered evidence body is not a regular file");
        let candidate = candidate
            .canonicalize()
            .unwrap_or_else(|_| panic!("registered evidence body is unavailable"));
        assert!(
            candidate.starts_with(&raw_root),
            "registered evidence path escaped the controlled raw root"
        );
        assert!(
            canonical_paths.insert(candidate.clone()),
            "registered evidence canonical path is duplicated"
        );
        assert!(
            request_ids.insert(required_column(&row, "request_id").to_owned()),
            "registered evidence request ID is duplicated"
        );
        assert!(
            target_keys.insert((
                required_column(&row, "term_id").to_owned(),
                required_column(&row, "campus").to_owned(),
            )),
            "registered evidence target is duplicated"
        );
        let body = std::fs::read(candidate)
            .unwrap_or_else(|_| panic!("registered evidence body could not be read"));
        let registered_hash = required_column(&row, "body_sha256");
        assert!(
            registered_hash.len() == 64
                && registered_hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "registered evidence SHA-256 is not 64 hex characters"
        );
        let registered_hash = registered_hash.to_ascii_lowercase();
        assert_eq!(
            sha256_hex(&body),
            registered_hash,
            "registered evidence SHA-256 mismatch"
        );
        verified_body_hashes.push(registered_hash);
        let courses = decode_catalog_payload(&body)
            .unwrap_or_else(|_| panic!("registered evidence did not decode as typed Catalog JSON"));
        let target = TermCampusKey::try_new(
            required_column(&row, "term_id"),
            required_column(&row, "campus"),
        )
        .unwrap_or_else(|_| panic!("registered evidence target identity is invalid"));
        let normalized = normalize_target(
            target,
            courses,
            SourceProvenance::from_body(
                required_column(&row, "request_id"),
                required_column(&row, "observed_at_utc"),
                &body,
            ),
        )
        .unwrap_or_else(|error| {
            panic!(
                "registered evidence normalization failed closed: {}",
                safe_normalization_error(&error)
            )
        });
        normalized_content_hashes.push(normalized.content_hash.clone());

        if !normalized.is_empty() {
            let observation_id: TraceId =
                format!("00000000-0000-4000-8000-{:012x}", aggregate.scopes + 1)
                    .parse()
                    .expect("synthetic replay trace ID");
            let observed_at = required_column(&row, "observed_at_utc");
            let command =
                to_catalog_refresh_command(&normalized, observation_id, observed_at, observed_at)
                    .expect("recorded normalized target should map to storage");
            let mut storage = OperationalStorage::open_in_memory()
                .expect("recorded projection storage should initialize");
            storage
                .apply_catalog_refresh(
                    command,
                    EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
                )
                .expect("recorded target should publish transactionally");
            let published = storage
                .published_catalog_snapshot(&normalized.target)
                .expect("recorded publication should load")
                .expect("recorded publication should exist");
            let projected = to_normalized_catalog_v1(&published)
                .expect("recorded publication should project to the public contract");
            for variant in &projected.course_variants {
                for field in [
                    &variant.notes,
                    &variant.subject_group_notes,
                    &variant.subject_notes,
                    &variant.unit_notes,
                    &variant.synopsis_url,
                    &variant.offering_unit_title,
                ] {
                    assert_not_malformed(field);
                }
            }
            for section in &projected.sections {
                for field in [
                    &section.subtitle,
                    &section.subtopic,
                    &section.section_notes,
                    &section.session_dates,
                    &section.session_date_print_indicator,
                    &section.comments_text,
                    &section.cross_listed_sections_text,
                    &section.cross_listed_section_type,
                    &section.exam_code,
                    &section.exam_code_text,
                    &section.special_permission_add_code,
                    &section.special_permission_add_description,
                    &section.special_permission_drop_code,
                    &section.special_permission_drop_description,
                    &section.eligibility_text,
                    &section.open_to_text,
                ] {
                    assert_not_malformed(field);
                }
                assert_not_malformed(&section.comments);
                assert_not_malformed(&section.major_codes);
                assert_not_malformed(&section.unit_codes);
                assert_not_malformed(&section.minor_codes);
                assert_not_malformed(&section.honor_program_codes);
                assert_not_malformed(&section.unit_majors);
            }
            for occurrence in &projected.occurrences {
                assert_not_malformed(&occurrence.campus);
                assert_not_malformed(&occurrence.campus_name);
                assert_not_malformed(&occurrence.building);
                assert_not_malformed(&occurrence.room);
            }
        }

        aggregate.scopes += 1;
        aggregate.courses += normalized.metrics.raw_courses;
        if normalized.metrics.raw_courses == 0 {
            aggregate.empty_scopes += 1;
        } else {
            aggregate.nonempty_scopes += 1;
        }
        aggregate.raw_sections += normalized.metrics.raw_sections;
        aggregate.unique_sections += normalized.metrics.normalized_sections;
        aggregate.equivalent_duplicate_keys += normalized.metrics.equivalent_duplicate_keys;
        aggregate.meetings += normalized.metrics.raw_meetings;
        aggregate.instructor_assignments += normalized.metrics.raw_instructor_assignments;
        aggregate.delivery_conflicts += normalized.metrics.delivery_conflict_raw_sections;
        aggregate.multi_object_groups += normalized
            .groups
            .iter()
            .filter(|group| group.raw_course_count > 1)
            .count();
        aggregate.multi_variant_groups += normalized
            .groups
            .iter()
            .filter(|group| group.variants.len() > 1)
            .count();
    }

    assert_eq!(aggregate.scopes, 21);
    assert_eq!(aggregate.nonempty_scopes, 17);
    assert_eq!(aggregate.empty_scopes, 4);
    assert_eq!(aggregate.courses, 10_629);
    assert_eq!(aggregate.raw_sections, 22_069);
    assert_eq!(aggregate.unique_sections, 22_051);
    assert_eq!(aggregate.equivalent_duplicate_keys, 18);
    assert_eq!(aggregate.conflicting_duplicate_keys, 0);
    assert_eq!(aggregate.cross_variant_section_overlap, 0);
    assert_eq!(aggregate.meetings, 30_804);
    assert_eq!(aggregate.instructor_assignments, 19_202);
    assert_eq!(aggregate.multi_object_groups, 41);
    assert_eq!(aggregate.multi_variant_groups, 31);
    assert_eq!(aggregate.delivery_conflicts, 184);

    verified_body_hashes.sort();
    normalized_content_hashes.sort();
    let body_set_hash = sha256_v1(verified_body_hashes.join("\n").as_bytes());
    let content_set_hash = sha256_v1(
        normalized_content_hashes
            .iter()
            .map(bcsp_catalog::Sha256Hex::as_str)
            .collect::<Vec<_>>()
            .join("\n")
            .as_bytes(),
    );
    let compact = serde_json::json!({
        "scopes": aggregate.scopes,
        "nonemptyScopes": aggregate.nonempty_scopes,
        "emptyScopes": aggregate.empty_scopes,
        "courses": aggregate.courses,
        "rawSections": aggregate.raw_sections,
        "uniqueSectionKeys": aggregate.unique_sections,
        "equivalentDuplicateKeys": aggregate.equivalent_duplicate_keys,
        "conflictingDuplicateKeys": aggregate.conflicting_duplicate_keys,
        "crossVariantSectionOverlap": aggregate.cross_variant_section_overlap,
        "meetings": aggregate.meetings,
        "instructorAssignments": aggregate.instructor_assignments,
        "multiObjectGroups": aggregate.multi_object_groups,
        "multiVariantGroups": aggregate.multi_variant_groups,
        "deliveryConflicts": aggregate.delivery_conflicts,
        "verifiedBodyHashSet": body_set_hash,
        "normalizedContentHashSet": content_set_hash,
    });
    println!(
        "P7_RECORDED_AGGREGATE={}",
        serde_json::to_string(&compact).expect("aggregate JSON should serialize")
    );
}

fn assert_not_malformed<T>(value: &CatalogFieldKnowledge<T>) {
    assert!(
        !matches!(
            value,
            CatalogFieldKnowledge::Unknown {
                reason: CatalogUnknownReason::Malformed
            }
        ),
        "recorded typed projection unexpectedly classified a frozen field as malformed"
    );
}

fn safe_normalization_error(error: &NormalizationError) -> String {
    match error {
        NormalizationError::MissingRequiredField(field) => {
            format!("MISSING_REQUIRED_FIELD:{field}")
        }
        NormalizationError::MalformedRequiredField(field) => {
            format!("MALFORMED_REQUIRED_FIELD:{field}")
        }
        NormalizationError::InvalidIdentity(_) => "INVALID_IDENTITY".to_owned(),
        NormalizationError::ConflictingVariantFacts { .. } => {
            "CONFLICTING_VARIANT_FACTS".to_owned()
        }
        NormalizationError::ConflictingDuplicate { .. } => "CONFLICTING_DUPLICATE".to_owned(),
        NormalizationError::SectionAcrossVariants { .. } => "SECTION_ACROSS_VARIANTS".to_owned(),
        NormalizationError::Canonicalization(_) => "CANONICALIZATION_FAILED".to_owned(),
    }
}

#[test]
fn recorded_relative_path_policy_is_fixed_and_fail_closed() {
    assert!(
        validate_raw_relative_path(
            "data/staging/p3-rutgers-evidence-20260713/catalog/SYN_T2030_NB_courses.json"
        )
        .is_ok()
    );
    let windows_absolute = ["C", ":/synthetic/catalog/SYN_courses.json"].concat();
    assert!(validate_raw_relative_path(&windows_absolute).is_err());
    for invalid in [
        "/data/staging/p3-rutgers-evidence-20260713/catalog/SYN_courses.json",
        "data/staging/p3-rutgers-evidence-20260713/catalog/../SYN_courses.json",
        "data/staging/p3-rutgers-evidence-20260713/catalog/./SYN_courses.json",
        "data/staging/p3-rutgers-evidence-20260713/catalog/nested/SYN_courses.json",
        "data\\staging\\p3-rutgers-evidence-20260713\\catalog\\SYN_courses.json",
        "data/staging/other/catalog/SYN_courses.json",
        ".secrets/SYN_courses.json",
        "chat-log-codex-2026-07-13.md",
        "project-governance/current/p1/SYN_courses.json",
        "data/staging/p3-rutgers-evidence-20260713/catalog/SYN.metadata.json",
    ] {
        assert!(
            validate_raw_relative_path(invalid).is_err(),
            "unsafe synthetic path unexpectedly passed the fixed policy"
        );
    }
}

fn controlled_repository_root() -> PathBuf {
    let requested = required_env_path("P3_CATALOG_EVIDENCE_ROOT");
    require_regular_directory(&requested, "evidence root is not a regular directory");
    let requested = requested
        .canonicalize()
        .unwrap_or_else(|_| panic!("evidence root is unavailable"));
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .unwrap_or_else(|_| panic!("package root is unavailable"));
    let expected = manifest_dir
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| panic!("repository root cannot be derived"));
    assert_eq!(
        requested, expected,
        "evidence root is not the current repository"
    );
    requested
}

fn controlled_register_path(root: &Path) -> PathBuf {
    let requested = required_env_path("P3_CATALOG_EVIDENCE_REGISTER");
    require_regular_file(&requested, "evidence register is not a regular file");
    let requested = requested
        .canonicalize()
        .unwrap_or_else(|_| panic!("evidence register is unavailable"));
    let expected_path = root.join(REGISTER_RELATIVE_PATH);
    require_regular_file(
        &expected_path,
        "expected evidence register is not a regular file",
    );
    let expected = expected_path
        .canonicalize()
        .unwrap_or_else(|_| panic!("expected evidence register is unavailable"));
    assert!(
        expected.starts_with(root),
        "expected evidence register escaped the repository"
    );
    assert_eq!(
        requested, expected,
        "evidence register is not the fixed P3 register"
    );
    requested
}

fn controlled_raw_root(root: &Path) -> PathBuf {
    let path = root.join(RAW_ROOT_RELATIVE_PATH);
    require_regular_directory(&path, "controlled raw root is not a regular directory");
    let path = path
        .canonicalize()
        .unwrap_or_else(|_| panic!("controlled raw root is unavailable"));
    assert!(
        path.starts_with(root),
        "controlled raw root escaped the repository"
    );
    path
}

fn validate_raw_relative_path(value: &str) -> Result<PathBuf, &'static str> {
    if value.contains('\\') || !value.starts_with(RAW_PATH_PREFIX) {
        return Err("wrong raw path prefix");
    }
    let file_name = value
        .strip_prefix(RAW_PATH_PREFIX)
        .ok_or("wrong raw path prefix")?;
    if file_name.is_empty() || file_name.contains('/') || !file_name.ends_with("_courses.json") {
        return Err("wrong raw file name");
    }
    let path = Path::new(value);
    if path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err("raw path is not a normal relative path");
    }
    Ok(path.to_path_buf())
}

fn require_regular_file(path: &Path, message: &'static str) {
    let metadata = std::fs::symlink_metadata(path).unwrap_or_else(|_| panic!("{message}"));
    assert!(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "{message}"
    );
}

fn require_regular_directory(path: &Path, message: &'static str) {
    let metadata = std::fs::symlink_metadata(path).unwrap_or_else(|_| panic!("{message}"));
    assert!(
        metadata.file_type().is_dir() && !metadata.file_type().is_symlink(),
        "{message}"
    );
}

fn required_env_path(name: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("required controlled replay environment variable is absent"))
}

fn required_column<'a>(row: &'a BTreeMap<&str, &str>, name: &str) -> &'a str {
    row.get(name)
        .copied()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| panic!("required evidence register column is absent"))
}
