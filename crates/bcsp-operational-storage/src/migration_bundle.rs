pub(crate) struct BundledMigration {
    pub(crate) filename: &'static str,
    pub(crate) sql: &'static str,
}

// This committed, self-contained mirror keeps production binaries independent
// of source-tree paths. A unit test below the loader requires byte-for-byte
// equality with every version-controlled migration SQL file.
pub(crate) const BUNDLED_MIGRATIONS: &[BundledMigration] = &[
    BundledMigration {
        filename: "0001_operational_catalog.sql",
        sql: r#"CREATE TABLE bcsp_operational_migrations (
    migration_id INTEGER PRIMARY KEY CHECK (migration_id > 0),
    name TEXT NOT NULL UNIQUE CHECK (length(name) > 0),
    sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
    applied_at TEXT NOT NULL
) STRICT;

CREATE TABLE catalog_discovery_observations (
    observation_id TEXT PRIMARY KEY,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    status TEXT NOT NULL CHECK (status IN ('STARTED', 'APPLIED_CHANGED', 'APPLIED_UNCHANGED', 'FAILED')),
    semantic_sha256 TEXT CHECK (semantic_sha256 IS NULL OR length(semantic_sha256) = 64),
    changed INTEGER CHECK (changed IN (0, 1)),
    resulting_content_version INTEGER CHECK (resulting_content_version >= 0),
    term_count INTEGER NOT NULL DEFAULT 0 CHECK (term_count >= 0),
    campus_count INTEGER NOT NULL DEFAULT 0 CHECK (campus_count >= 0),
    subject_count INTEGER NOT NULL DEFAULT 0 CHECK (subject_count >= 0),
    error_stage TEXT,
    error_code TEXT,
    diagnostic_token TEXT CHECK (diagnostic_token IS NULL OR length(diagnostic_token) <= 64)
) STRICT;

CREATE TABLE catalog_discovery_source_versions (
    source_version_id TEXT PRIMARY KEY,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('SELECTOR', 'BOOTSTRAP')),
    source_identity TEXT NOT NULL,
    content_sha256 TEXT NOT NULL CHECK (length(content_sha256) = 64),
    canonical_facts_json TEXT NOT NULL CHECK (json_valid(canonical_facts_json)),
    observed_at TEXT NOT NULL,
    UNIQUE (source_kind, source_identity, content_sha256)
) STRICT;

CREATE TABLE catalog_discovery_observation_sources (
    observation_id TEXT NOT NULL REFERENCES catalog_discovery_observations(observation_id) ON DELETE CASCADE,
    source_version_id TEXT NOT NULL REFERENCES catalog_discovery_source_versions(source_version_id),
    PRIMARY KEY (observation_id, source_version_id)
) STRICT;

CREATE TABLE catalog_discovery_state (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    content_version INTEGER NOT NULL DEFAULT 0 CHECK (content_version >= 0),
    semantic_sha256 TEXT CHECK (semantic_sha256 IS NULL OR length(semantic_sha256) = 64),
    last_attempt_observation_id TEXT,
    last_success_observation_id TEXT,
    last_published_observation_id TEXT,
    last_nonempty_observation_id TEXT
) STRICT;

INSERT INTO catalog_discovery_state(singleton) VALUES (1);

CREATE TABLE catalog_terms (
    term_id TEXT PRIMARY KEY,
    year INTEGER CHECK (year IS NULL OR year >= 1900),
    term_code TEXT,
    display_name TEXT,
    published INTEGER CHECK (published IS NULL OR published IN (0, 1)),
    canonical_facts_json TEXT NOT NULL CHECK (json_valid(canonical_facts_json)),
    source_version_id TEXT NOT NULL REFERENCES catalog_discovery_source_versions(source_version_id),
    content_version INTEGER NOT NULL CHECK (content_version >= 1)
) STRICT;

CREATE TABLE catalog_campuses (
    term_id TEXT NOT NULL REFERENCES catalog_terms(term_id) ON DELETE CASCADE,
    campus_code TEXT NOT NULL,
    display_name TEXT,
    category TEXT,
    enabled INTEGER CHECK (enabled IS NULL OR enabled IN (0, 1)),
    canonical_facts_json TEXT NOT NULL CHECK (json_valid(canonical_facts_json)),
    source_version_id TEXT NOT NULL REFERENCES catalog_discovery_source_versions(source_version_id),
    content_version INTEGER NOT NULL CHECK (content_version >= 1),
    PRIMARY KEY (term_id, campus_code)
) STRICT;

CREATE TABLE catalog_subjects (
    term_id TEXT NOT NULL,
    campus_code TEXT NOT NULL,
    subject_code TEXT NOT NULL,
    display_name TEXT,
    enabled INTEGER CHECK (enabled IS NULL OR enabled IN (0, 1)),
    canonical_facts_json TEXT NOT NULL CHECK (json_valid(canonical_facts_json)),
    source_version_id TEXT NOT NULL REFERENCES catalog_discovery_source_versions(source_version_id),
    content_version INTEGER NOT NULL CHECK (content_version >= 1),
    PRIMARY KEY (term_id, campus_code, subject_code),
    FOREIGN KEY (term_id, campus_code)
        REFERENCES catalog_campuses(term_id, campus_code) ON DELETE CASCADE
) STRICT;

CREATE TABLE catalog_targets (
    target_id TEXT PRIMARY KEY,
    term_id TEXT NOT NULL,
    campus_code TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_attempt_sequence INTEGER NOT NULL DEFAULT 0 CHECK (last_attempt_sequence >= 0),
    current_content_version INTEGER NOT NULL DEFAULT 0 CHECK (current_content_version >= 0),
    last_attempt_observation_id TEXT,
    last_success_observation_id TEXT,
    last_published_observation_id TEXT,
    last_nonempty_observation_id TEXT,
    pending_empty_observation_id TEXT,
    accepted_semantic_hash TEXT CHECK (accepted_semantic_hash IS NULL OR length(accepted_semantic_hash) = 64),
    group_count INTEGER NOT NULL DEFAULT 0 CHECK (group_count >= 0),
    variant_count INTEGER NOT NULL DEFAULT 0 CHECK (variant_count >= 0),
    section_count INTEGER NOT NULL DEFAULT 0 CHECK (section_count >= 0),
    occurrence_count INTEGER NOT NULL DEFAULT 0 CHECK (occurrence_count >= 0),
    UNIQUE (term_id, campus_code)
) STRICT;

CREATE TABLE catalog_refresh_observations (
    observation_id TEXT PRIMARY KEY,
    target_id TEXT NOT NULL REFERENCES catalog_targets(target_id) ON DELETE CASCADE,
    attempt_sequence INTEGER NOT NULL CHECK (attempt_sequence > 0),
    started_at TEXT NOT NULL,
    completed_at TEXT,
    status TEXT NOT NULL CHECK (status IN (
        'STARTED',
        'STAGED',
        'APPLIED_CHANGED',
        'APPLIED_UNCHANGED',
        'EMPTY_VALID_INITIAL',
        'EMPTY_SUSPECT_RETAINED',
        'FAILED',
        'INTERRUPTED'
    )),
    source_content_sha256 TEXT CHECK (source_content_sha256 IS NULL OR length(source_content_sha256) = 64),
    semantic_sha256 TEXT CHECK (semantic_sha256 IS NULL OR length(semantic_sha256) = 64),
    source_bytes INTEGER CHECK (source_bytes IS NULL OR source_bytes >= 0),
    group_count INTEGER NOT NULL DEFAULT 0 CHECK (group_count >= 0),
    variant_count INTEGER NOT NULL DEFAULT 0 CHECK (variant_count >= 0),
    section_count INTEGER NOT NULL DEFAULT 0 CHECK (section_count >= 0),
    occurrence_count INTEGER NOT NULL DEFAULT 0 CHECK (occurrence_count >= 0),
    changed INTEGER CHECK (changed IN (0, 1)),
    resulting_content_version INTEGER CHECK (resulting_content_version >= 0),
    retained_observation_id TEXT,
    error_stage TEXT,
    error_code TEXT,
    diagnostic_token TEXT CHECK (diagnostic_token IS NULL OR length(diagnostic_token) <= 64),
    UNIQUE (target_id, attempt_sequence)
) STRICT;

CREATE INDEX catalog_refresh_observations_target_started
    ON catalog_refresh_observations(target_id, started_at DESC);

CREATE TABLE catalog_refresh_checkpoints (
    target_id TEXT PRIMARY KEY REFERENCES catalog_targets(target_id) ON DELETE CASCADE,
    content_version INTEGER NOT NULL CHECK (content_version >= 1),
    semantic_sha256 TEXT NOT NULL CHECK (length(semantic_sha256) = 64),
    observation_id TEXT NOT NULL REFERENCES catalog_refresh_observations(observation_id),
    accepted_at TEXT NOT NULL,
    group_count INTEGER NOT NULL CHECK (group_count >= 0),
    variant_count INTEGER NOT NULL CHECK (variant_count >= 0),
    section_count INTEGER NOT NULL CHECK (section_count >= 0),
    occurrence_count INTEGER NOT NULL CHECK (occurrence_count >= 0)
) STRICT;

CREATE TABLE catalog_staging_payloads (
    observation_id TEXT PRIMARY KEY REFERENCES catalog_refresh_observations(observation_id) ON DELETE CASCADE,
    target_id TEXT NOT NULL,
    sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
    byte_count INTEGER NOT NULL CHECK (byte_count >= 0),
    encoded_body BLOB NOT NULL
) STRICT;

CREATE TABLE catalog_staging_course_groups (
    observation_id TEXT NOT NULL REFERENCES catalog_refresh_observations(observation_id) ON DELETE CASCADE,
    target_id TEXT NOT NULL,
    course_string TEXT NOT NULL,
    canonical_facts_json TEXT NOT NULL CHECK (json_valid(canonical_facts_json)),
    PRIMARY KEY (observation_id, course_string)
) STRICT;

CREATE TABLE catalog_staging_course_variants (
    observation_id TEXT NOT NULL,
    target_id TEXT NOT NULL,
    course_string TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    subject_code TEXT,
    course_number TEXT,
    title TEXT,
    description TEXT,
    credits_summary TEXT,
    supplement TEXT,
    search_document TEXT NOT NULL,
    canonical_sha256 TEXT NOT NULL CHECK (length(canonical_sha256) = 64),
    raw_multiplicity INTEGER NOT NULL CHECK (raw_multiplicity >= 1),
    canonical_facts_json TEXT NOT NULL CHECK (json_valid(canonical_facts_json)),
    PRIMARY KEY (observation_id, course_string, fingerprint),
    FOREIGN KEY (observation_id, course_string)
        REFERENCES catalog_staging_course_groups(observation_id, course_string) ON DELETE CASCADE
) STRICT;

CREATE TABLE catalog_staging_sections (
    observation_id TEXT NOT NULL,
    target_id TEXT NOT NULL,
    section_index TEXT NOT NULL,
    course_string TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    section_number TEXT,
    catalog_status TEXT,
    section_course_type TEXT,
    delivery_modality TEXT NOT NULL CHECK (delivery_modality IN (
        'ON_CAMPUS_OR_IN_PERSON', 'HYBRID', 'ONLINE', 'OTHER', 'UNKNOWN', 'UNKNOWN_CONFLICT'
    )),
    synchronicity TEXT NOT NULL CHECK (synchronicity IN (
        'SYNC', 'ASYNC', 'MIXED', 'UNSPECIFIED', 'UNKNOWN'
    )),
    canonical_facts_json TEXT NOT NULL CHECK (json_valid(canonical_facts_json)),
    canonical_sha256 TEXT NOT NULL CHECK (length(canonical_sha256) = 64),
    PRIMARY KEY (observation_id, section_index),
    FOREIGN KEY (observation_id, course_string, fingerprint)
        REFERENCES catalog_staging_course_variants(observation_id, course_string, fingerprint) ON DELETE CASCADE
) STRICT;

CREATE TABLE catalog_staging_occurrences (
    observation_id TEXT NOT NULL,
    target_id TEXT NOT NULL,
    section_index TEXT NOT NULL,
    occurrence_key TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    weekday TEXT,
    start_minute INTEGER CHECK (start_minute IS NULL OR start_minute BETWEEN 0 AND 1439),
    end_minute INTEGER CHECK (end_minute IS NULL OR end_minute BETWEEN 1 AND 1440),
    time_knowledge TEXT NOT NULL CHECK (time_knowledge IN (
        'MISSING', 'EXPLICIT_NULL', 'EMPTY', 'PARTIAL', 'INVALID', 'KNOWN'
    )),
    requiredness TEXT NOT NULL CHECK (requiredness IN (
        'REQUIRED', 'OPTIONAL', 'UNKNOWN_REQUIREDNESS'
    )),
    occurrence_kind TEXT NOT NULL CHECK (occurrence_kind IN (
        'SCHEDULED', 'BY_ARRANGEMENT', 'UNSPECIFIED'
    )),
    modality TEXT NOT NULL CHECK (modality IN (
        'ON_CAMPUS_OR_IN_PERSON', 'HYBRID', 'ONLINE', 'OTHER', 'UNKNOWN', 'UNKNOWN_CONFLICT'
    )),
    synchronicity TEXT NOT NULL CHECK (synchronicity IN (
        'SYNC', 'ASYNC', 'MIXED', 'UNSPECIFIED', 'UNKNOWN'
    )),
    evidence TEXT NOT NULL CHECK (evidence IN ('NONE', 'PHYSICAL', 'REMOTE')),
    normalization_reason TEXT NOT NULL CHECK (
        length(normalization_reason) BETWEEN 1 AND 64
        AND normalization_reason NOT GLOB '*[^A-Z0-9_]*'
    ),
    location TEXT,
    building TEXT,
    room TEXT,
    raw_sha256 TEXT NOT NULL CHECK (length(raw_sha256) = 64),
    canonical_facts_json TEXT NOT NULL CHECK (json_valid(canonical_facts_json)),
    CHECK (
        (time_knowledge = 'KNOWN'
            AND start_minute IS NOT NULL AND end_minute IS NOT NULL
            AND end_minute > start_minute)
        OR
        (time_knowledge != 'KNOWN' AND start_minute IS NULL AND end_minute IS NULL)
    ),
    PRIMARY KEY (observation_id, section_index, occurrence_key),
    FOREIGN KEY (observation_id, section_index)
        REFERENCES catalog_staging_sections(observation_id, section_index) ON DELETE CASCADE
) STRICT;

CREATE TABLE catalog_staging_provenance (
    observation_id TEXT NOT NULL REFERENCES catalog_refresh_observations(observation_id) ON DELETE CASCADE,
    target_id TEXT NOT NULL,
    entity_kind TEXT NOT NULL,
    entity_key TEXT NOT NULL,
    field_name TEXT NOT NULL,
    source_sha256 TEXT NOT NULL CHECK (length(source_sha256) = 64),
    source_ordinal INTEGER NOT NULL CHECK (source_ordinal >= 0),
    detail_json TEXT NOT NULL CHECK (json_valid(detail_json)),
    PRIMARY KEY (observation_id, entity_kind, entity_key, field_name, source_ordinal)
) STRICT;

CREATE TABLE catalog_course_groups (
    target_id TEXT NOT NULL REFERENCES catalog_targets(target_id) ON DELETE CASCADE,
    course_string TEXT NOT NULL,
    canonical_facts_json TEXT NOT NULL CHECK (json_valid(canonical_facts_json)),
    content_version INTEGER NOT NULL CHECK (content_version >= 1),
    PRIMARY KEY (target_id, course_string)
) STRICT;

CREATE TABLE catalog_course_variants (
    target_id TEXT NOT NULL,
    course_string TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    subject_code TEXT,
    course_number TEXT,
    title TEXT,
    description TEXT,
    credits_summary TEXT,
    supplement TEXT,
    search_document TEXT NOT NULL,
    canonical_sha256 TEXT NOT NULL CHECK (length(canonical_sha256) = 64),
    raw_multiplicity INTEGER NOT NULL CHECK (raw_multiplicity >= 1),
    canonical_facts_json TEXT NOT NULL CHECK (json_valid(canonical_facts_json)),
    content_version INTEGER NOT NULL CHECK (content_version >= 1),
    PRIMARY KEY (target_id, course_string, fingerprint),
    FOREIGN KEY (target_id, course_string)
        REFERENCES catalog_course_groups(target_id, course_string) ON DELETE CASCADE
) STRICT;

CREATE TABLE catalog_sections (
    target_id TEXT NOT NULL,
    section_index TEXT NOT NULL,
    course_string TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    section_number TEXT,
    catalog_status TEXT,
    section_course_type TEXT,
    delivery_modality TEXT NOT NULL CHECK (delivery_modality IN (
        'ON_CAMPUS_OR_IN_PERSON', 'HYBRID', 'ONLINE', 'OTHER', 'UNKNOWN', 'UNKNOWN_CONFLICT'
    )),
    synchronicity TEXT NOT NULL CHECK (synchronicity IN (
        'SYNC', 'ASYNC', 'MIXED', 'UNSPECIFIED', 'UNKNOWN'
    )),
    canonical_facts_json TEXT NOT NULL CHECK (json_valid(canonical_facts_json)),
    canonical_sha256 TEXT NOT NULL CHECK (length(canonical_sha256) = 64),
    content_version INTEGER NOT NULL CHECK (content_version >= 1),
    PRIMARY KEY (target_id, section_index),
    FOREIGN KEY (target_id, course_string, fingerprint)
        REFERENCES catalog_course_variants(target_id, course_string, fingerprint) ON DELETE CASCADE
) STRICT;

CREATE TABLE catalog_occurrences (
    target_id TEXT NOT NULL,
    section_index TEXT NOT NULL,
    occurrence_key TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    weekday TEXT,
    start_minute INTEGER CHECK (start_minute IS NULL OR start_minute BETWEEN 0 AND 1439),
    end_minute INTEGER CHECK (end_minute IS NULL OR end_minute BETWEEN 1 AND 1440),
    time_knowledge TEXT NOT NULL CHECK (time_knowledge IN (
        'MISSING', 'EXPLICIT_NULL', 'EMPTY', 'PARTIAL', 'INVALID', 'KNOWN'
    )),
    requiredness TEXT NOT NULL CHECK (requiredness IN (
        'REQUIRED', 'OPTIONAL', 'UNKNOWN_REQUIREDNESS'
    )),
    occurrence_kind TEXT NOT NULL CHECK (occurrence_kind IN (
        'SCHEDULED', 'BY_ARRANGEMENT', 'UNSPECIFIED'
    )),
    modality TEXT NOT NULL CHECK (modality IN (
        'ON_CAMPUS_OR_IN_PERSON', 'HYBRID', 'ONLINE', 'OTHER', 'UNKNOWN', 'UNKNOWN_CONFLICT'
    )),
    synchronicity TEXT NOT NULL CHECK (synchronicity IN (
        'SYNC', 'ASYNC', 'MIXED', 'UNSPECIFIED', 'UNKNOWN'
    )),
    evidence TEXT NOT NULL CHECK (evidence IN ('NONE', 'PHYSICAL', 'REMOTE')),
    normalization_reason TEXT NOT NULL CHECK (
        length(normalization_reason) BETWEEN 1 AND 64
        AND normalization_reason NOT GLOB '*[^A-Z0-9_]*'
    ),
    location TEXT,
    building TEXT,
    room TEXT,
    raw_sha256 TEXT NOT NULL CHECK (length(raw_sha256) = 64),
    canonical_facts_json TEXT NOT NULL CHECK (json_valid(canonical_facts_json)),
    content_version INTEGER NOT NULL CHECK (content_version >= 1),
    CHECK (
        (time_knowledge = 'KNOWN'
            AND start_minute IS NOT NULL AND end_minute IS NOT NULL
            AND end_minute > start_minute)
        OR
        (time_knowledge != 'KNOWN' AND start_minute IS NULL AND end_minute IS NULL)
    ),
    PRIMARY KEY (target_id, section_index, occurrence_key),
    FOREIGN KEY (target_id, section_index)
        REFERENCES catalog_sections(target_id, section_index) ON DELETE CASCADE
) STRICT;

CREATE TABLE catalog_provenance (
    target_id TEXT NOT NULL REFERENCES catalog_targets(target_id) ON DELETE CASCADE,
    entity_kind TEXT NOT NULL,
    entity_key TEXT NOT NULL,
    field_name TEXT NOT NULL,
    source_sha256 TEXT NOT NULL CHECK (length(source_sha256) = 64),
    source_ordinal INTEGER NOT NULL CHECK (source_ordinal >= 0),
    detail_json TEXT NOT NULL CHECK (json_valid(detail_json)),
    content_version INTEGER NOT NULL CHECK (content_version >= 1),
    PRIMARY KEY (target_id, entity_kind, entity_key, field_name, source_ordinal)
) STRICT;

CREATE VIRTUAL TABLE catalog_course_fts USING fts5(
    target_id UNINDEXED,
    course_string UNINDEXED,
    fingerprint UNINDEXED,
    content_version UNINDEXED,
    document,
    tokenize = 'unicode61 remove_diacritics 2'
);
"#,
    },
    BundledMigration {
        filename: "0002_operational_open.sql",
        sql: r#"CREATE TABLE open_batch_state (
    target_id TEXT PRIMARY KEY REFERENCES catalog_targets(target_id) ON DELETE CASCADE,
    last_attempt_sequence INTEGER NOT NULL DEFAULT 0 CHECK (last_attempt_sequence >= 0),
    last_observation_sequence INTEGER NOT NULL DEFAULT 0 CHECK (last_observation_sequence >= 0),
    current_catalog_content_version INTEGER NOT NULL DEFAULT 0 CHECK (current_catalog_content_version >= 0),
    lkg_attempt_id TEXT,
    lkg_observation_sequence INTEGER CHECK (lkg_observation_sequence IS NULL OR lkg_observation_sequence > 0),
    lkg_observed_at TEXT,
    lkg_canonical_set_sha256 TEXT CHECK (lkg_canonical_set_sha256 IS NULL OR length(lkg_canonical_set_sha256) = 64),
    lkg_state_sha256 TEXT CHECK (lkg_state_sha256 IS NULL OR length(lkg_state_sha256) = 64),
    last_attempt_id TEXT,
    last_attempt_at TEXT,
    last_success_at TEXT,
    last_failure_attempt_sequence INTEGER CHECK (last_failure_attempt_sequence IS NULL OR last_failure_attempt_sequence > 0),
    last_failure_at TEXT,
    last_failure_error_code TEXT CHECK (last_failure_error_code IS NULL OR length(last_failure_error_code) BETWEEN 1 AND 64),
    last_failure_http_status INTEGER CHECK (last_failure_http_status IS NULL OR last_failure_http_status BETWEEN 100 AND 599),
    last_body_change_at TEXT,
    last_state_change_at TEXT,
    requested_interval_seconds INTEGER CHECK (requested_interval_seconds IS NULL OR requested_interval_seconds > 0),
    effective_interval_seconds INTEGER CHECK (effective_interval_seconds IS NULL OR effective_interval_seconds > 0),
    last_schedule_lag_ms INTEGER CHECK (last_schedule_lag_ms IS NULL OR last_schedule_lag_ms >= 0),
    CHECK (
        (
            last_failure_attempt_sequence IS NULL
            AND last_failure_at IS NULL
            AND last_failure_error_code IS NULL
            AND last_failure_http_status IS NULL
        )
        OR (
            last_failure_attempt_sequence IS NOT NULL
            AND last_failure_at IS NOT NULL
            AND last_failure_error_code IS NOT NULL
        )
    )
) STRICT;

CREATE TABLE open_pull_attempts (
    attempt_id TEXT PRIMARY KEY,
    target_id TEXT NOT NULL REFERENCES catalog_targets(target_id) ON DELETE CASCADE,
    run_id TEXT NOT NULL,
    attempt_sequence INTEGER NOT NULL CHECK (attempt_sequence > 0),
    rutgers_day TEXT NOT NULL CHECK (
        length(rutgers_day) = 10
        AND substr(rutgers_day, 5, 1) = '-'
        AND substr(rutgers_day, 8, 1) = '-'
    ),
    captured_catalog_content_version INTEGER NOT NULL CHECK (captured_catalog_content_version > 0),
    started_at TEXT NOT NULL,
    completed_at TEXT,
    classification TEXT NOT NULL CHECK (classification IN (
        'STARTED',
        'VALID_APPLIED',
        'VALID_EMPTY_NO_ROWS',
        'FAILED',
        'UNSAFE_EMPTY',
        'UNSAFE_ZERO_INTERSECTION',
        'STALE_CATALOG_RACE',
        'INTERRUPTED'
    )),
    lane TEXT NOT NULL CHECK (lane IN (
        'GENERAL', 'ACTIVE_WATCH', 'FIRST_LOAD', 'MANUAL_REFRESH', 'CATALOG_RACE_RECHECK'
    )),
    http_status INTEGER CHECK (http_status IS NULL OR http_status BETWEEN 100 AND 599),
    cache_status TEXT CHECK (cache_status IS NULL OR length(cache_status) BETWEEN 1 AND 32),
    decoded_bytes INTEGER CHECK (decoded_bytes IS NULL OR decoded_bytes >= 0),
    decoded_body_sha256 TEXT CHECK (
        decoded_body_sha256 IS NULL OR (
            length(decoded_body_sha256) = 64
            AND decoded_body_sha256 NOT GLOB '*[^0-9a-f]*'
        )
    ),
    content_type TEXT CHECK (content_type IS NULL OR (length(content_type) BETWEEN 1 AND 4096 AND instr(content_type, char(13)) = 0 AND instr(content_type, char(10)) = 0)),
    etag TEXT CHECK (etag IS NULL OR (length(etag) BETWEEN 1 AND 4096 AND instr(etag, char(13)) = 0 AND instr(etag, char(10)) = 0)),
    cache_control TEXT CHECK (cache_control IS NULL OR (length(cache_control) BETWEEN 1 AND 4096 AND instr(cache_control, char(13)) = 0 AND instr(cache_control, char(10)) = 0)),
    response_date TEXT CHECK (response_date IS NULL OR (length(response_date) BETWEEN 1 AND 4096 AND instr(response_date, char(13)) = 0 AND instr(response_date, char(10)) = 0)),
    age_seconds INTEGER CHECK (age_seconds IS NULL OR age_seconds >= 0),
    last_modified TEXT CHECK (last_modified IS NULL OR (length(last_modified) BETWEEN 1 AND 4096 AND instr(last_modified, char(13)) = 0 AND instr(last_modified, char(10)) = 0)),
    retry_after TEXT CHECK (retry_after IS NULL OR (length(retry_after) BETWEEN 1 AND 4096 AND instr(retry_after, char(13)) = 0 AND instr(retry_after, char(10)) = 0)),
    retry_after_seconds INTEGER CHECK (retry_after_seconds IS NULL OR retry_after_seconds >= 0),
    canonical_set_sha256 TEXT CHECK (canonical_set_sha256 IS NULL OR length(canonical_set_sha256) = 64),
    state_sha256 TEXT CHECK (state_sha256 IS NULL OR length(state_sha256) = 64),
    source_value_count INTEGER NOT NULL DEFAULT 0 CHECK (source_value_count >= 0),
    catalog_section_count INTEGER NOT NULL DEFAULT 0 CHECK (catalog_section_count >= 0),
    intersection_count INTEGER NOT NULL DEFAULT 0 CHECK (intersection_count >= 0),
    orphan_count INTEGER NOT NULL DEFAULT 0 CHECK (orphan_count >= 0),
    duplicate_count INTEGER NOT NULL DEFAULT 0 CHECK (duplicate_count >= 0),
    requested_interval_seconds INTEGER CHECK (requested_interval_seconds IS NULL OR requested_interval_seconds > 0),
    effective_interval_seconds INTEGER CHECK (effective_interval_seconds IS NULL OR effective_interval_seconds > 0),
    schedule_lag_ms INTEGER CHECK (schedule_lag_ms IS NULL OR schedule_lag_ms >= 0),
    lkg_age_ms INTEGER CHECK (lkg_age_ms IS NULL OR lkg_age_ms >= 0),
    error_code TEXT CHECK (error_code IS NULL OR length(error_code) BETWEEN 1 AND 64),
    diagnostic_token TEXT CHECK (diagnostic_token IS NULL OR length(diagnostic_token) BETWEEN 1 AND 64),
    UNIQUE (target_id, attempt_sequence)
) STRICT;

CREATE INDEX open_pull_attempts_target_sequence
    ON open_pull_attempts(target_id, attempt_sequence DESC);

CREATE INDEX open_pull_attempts_target_day
    ON open_pull_attempts(target_id, rutgers_day, attempt_sequence DESC);

CREATE UNIQUE INDEX open_pull_attempts_one_started_per_target
    ON open_pull_attempts(target_id) WHERE classification = 'STARTED';

CREATE TABLE open_attempt_catalog_sections (
    attempt_id TEXT NOT NULL REFERENCES open_pull_attempts(attempt_id) ON DELETE CASCADE,
    section_index TEXT NOT NULL,
    PRIMARY KEY (attempt_id, section_index)
) STRICT;

CREATE TABLE open_batch_observations (
    attempt_id TEXT PRIMARY KEY REFERENCES open_pull_attempts(attempt_id) ON DELETE CASCADE,
    observation_id TEXT NOT NULL UNIQUE,
    target_id TEXT NOT NULL REFERENCES catalog_targets(target_id) ON DELETE CASCADE,
    observation_sequence INTEGER NOT NULL CHECK (observation_sequence > 0),
    catalog_content_version INTEGER NOT NULL CHECK (catalog_content_version > 0),
    observed_at TEXT NOT NULL,
    classification TEXT NOT NULL CHECK (classification IN ('VALID_APPLIED', 'VALID_EMPTY_NO_ROWS')),
    http_status INTEGER NOT NULL CHECK (http_status BETWEEN 200 AND 299),
    cache_status TEXT CHECK (cache_status IS NULL OR length(cache_status) BETWEEN 1 AND 32),
    decoded_bytes INTEGER NOT NULL CHECK (decoded_bytes >= 0),
    decoded_body_sha256 TEXT NOT NULL CHECK (length(decoded_body_sha256) = 64 AND decoded_body_sha256 NOT GLOB '*[^0-9a-f]*'),
    content_type TEXT NOT NULL CHECK (length(content_type) BETWEEN 1 AND 4096 AND instr(content_type, char(13)) = 0 AND instr(content_type, char(10)) = 0),
    etag TEXT CHECK (etag IS NULL OR (length(etag) BETWEEN 1 AND 4096 AND instr(etag, char(13)) = 0 AND instr(etag, char(10)) = 0)),
    cache_control TEXT CHECK (cache_control IS NULL OR (length(cache_control) BETWEEN 1 AND 4096 AND instr(cache_control, char(13)) = 0 AND instr(cache_control, char(10)) = 0)),
    response_date TEXT CHECK (response_date IS NULL OR (length(response_date) BETWEEN 1 AND 4096 AND instr(response_date, char(13)) = 0 AND instr(response_date, char(10)) = 0)),
    age_seconds INTEGER CHECK (age_seconds IS NULL OR age_seconds >= 0),
    last_modified TEXT CHECK (last_modified IS NULL OR (length(last_modified) BETWEEN 1 AND 4096 AND instr(last_modified, char(13)) = 0 AND instr(last_modified, char(10)) = 0)),
    retry_after TEXT CHECK (retry_after IS NULL OR (length(retry_after) BETWEEN 1 AND 4096 AND instr(retry_after, char(13)) = 0 AND instr(retry_after, char(10)) = 0)),
    retry_after_seconds INTEGER CHECK (retry_after_seconds IS NULL OR retry_after_seconds >= 0),
    canonical_set_sha256 TEXT NOT NULL CHECK (length(canonical_set_sha256) = 64),
    state_sha256 TEXT NOT NULL CHECK (length(state_sha256) = 64),
    source_value_count INTEGER NOT NULL CHECK (source_value_count >= 0),
    catalog_section_count INTEGER NOT NULL CHECK (catalog_section_count >= 0),
    intersection_count INTEGER NOT NULL CHECK (intersection_count >= 0),
    orphan_count INTEGER NOT NULL CHECK (orphan_count >= 0),
    duplicate_count INTEGER NOT NULL CHECK (duplicate_count >= 0),
    changed_section_count INTEGER NOT NULL CHECK (changed_section_count >= 0),
    body_changed INTEGER NOT NULL CHECK (body_changed IN (0, 1)),
    state_changed INTEGER NOT NULL CHECK (state_changed IN (0, 1)),
    UNIQUE (target_id, observation_sequence)
) STRICT;

CREATE INDEX open_batch_observations_target_sequence
    ON open_batch_observations(target_id, observation_sequence DESC);

CREATE TABLE open_section_current (
    target_id TEXT NOT NULL REFERENCES catalog_targets(target_id) ON DELETE CASCADE,
    section_index TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('OPEN', 'CLOSED')),
    catalog_content_version INTEGER NOT NULL CHECK (catalog_content_version > 0),
    attempt_id TEXT NOT NULL REFERENCES open_batch_observations(attempt_id) ON DELETE CASCADE,
    observation_sequence INTEGER NOT NULL CHECK (observation_sequence > 0),
    observed_at TEXT NOT NULL,
    PRIMARY KEY (target_id, section_index)
) STRICT;

CREATE TABLE open_section_events (
    event_id INTEGER PRIMARY KEY AUTOINCREMENT,
    observation_id TEXT NOT NULL UNIQUE,
    refresh_observation_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL REFERENCES open_batch_observations(attempt_id) ON DELETE CASCADE,
    target_id TEXT NOT NULL REFERENCES catalog_targets(target_id) ON DELETE CASCADE,
    section_index TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('OPEN', 'CLOSED')),
    catalog_content_version INTEGER NOT NULL CHECK (catalog_content_version > 0),
    observation_sequence INTEGER NOT NULL CHECK (observation_sequence > 0),
    observed_at TEXT NOT NULL,
    UNIQUE (attempt_id, section_index)
) STRICT;

CREATE INDEX open_section_events_after
    ON open_section_events(event_id, target_id);

CREATE TABLE open_daily_counters (
    target_id TEXT NOT NULL REFERENCES catalog_targets(target_id) ON DELETE CASCADE,
    rutgers_day TEXT NOT NULL,
    attempted INTEGER NOT NULL DEFAULT 0 CHECK (attempted >= 0),
    succeeded INTEGER NOT NULL DEFAULT 0 CHECK (succeeded >= 0),
    failed INTEGER NOT NULL DEFAULT 0 CHECK (failed >= 0),
    empty INTEGER NOT NULL DEFAULT 0 CHECK (empty >= 0),
    PRIMARY KEY (target_id, rutgers_day)
) STRICT;

CREATE TABLE open_run_counters (
    run_id TEXT NOT NULL,
    target_id TEXT NOT NULL REFERENCES catalog_targets(target_id) ON DELETE CASCADE,
    attempted INTEGER NOT NULL DEFAULT 0 CHECK (attempted >= 0),
    succeeded INTEGER NOT NULL DEFAULT 0 CHECK (succeeded >= 0),
    failed INTEGER NOT NULL DEFAULT 0 CHECK (failed >= 0),
    empty INTEGER NOT NULL DEFAULT 0 CHECK (empty >= 0),
    PRIMARY KEY (run_id, target_id)
) STRICT;

CREATE TABLE open_origin_state (
    origin_id TEXT PRIMARY KEY CHECK (length(origin_id) BETWEEN 1 AND 64),
    circuit_state TEXT NOT NULL CHECK (circuit_state IN ('CLOSED', 'RETRY_AFTER', 'FATAL_DIAGNOSTIC')),
    reason_code TEXT CHECK (reason_code IS NULL OR length(reason_code) BETWEEN 1 AND 64),
    opened_at TEXT,
    retry_at TEXT,
    diagnostic_recheck_required INTEGER NOT NULL CHECK (diagnostic_recheck_required IN (0, 1)),
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE open_schedule_state (
    target_id TEXT PRIMARY KEY REFERENCES catalog_targets(target_id) ON DELETE CASCADE,
    requested_interval_seconds INTEGER NOT NULL CHECK (requested_interval_seconds > 0),
    effective_interval_seconds INTEGER NOT NULL CHECK (effective_interval_seconds > 0),
    next_due_at TEXT NOT NULL,
    last_scheduled_at TEXT,
    last_actual_start_at TEXT,
    schedule_lag_ms INTEGER NOT NULL DEFAULT 0 CHECK (schedule_lag_ms >= 0),
    failure_streak INTEGER NOT NULL DEFAULT 0 CHECK (failure_streak >= 0),
    updated_at TEXT NOT NULL
) STRICT;
"#,
    },
    BundledMigration {
        filename: "0003_catalog_variant_fk_indexes.sql",
        sql: r#"CREATE INDEX catalog_staging_sections_variant_fk
    ON catalog_staging_sections(observation_id, course_string, fingerprint);

CREATE INDEX catalog_sections_variant_fk
    ON catalog_sections(target_id, course_string, fingerprint);
"#,
    },
    BundledMigration {
        filename: "0004_complete_target_snapshots.sql",
        sql: r#"ALTER TABLE open_pull_attempts
    ADD COLUMN candidate_catalog_observation_id TEXT
        REFERENCES catalog_refresh_observations(observation_id);

ALTER TABLE open_pull_attempts
    ADD COLUMN candidate_base_content_version INTEGER
        CHECK (candidate_base_content_version IS NULL OR candidate_base_content_version >= 0);

ALTER TABLE catalog_refresh_observations
    ADD COLUMN http_status INTEGER
        CHECK (http_status IS NULL OR http_status BETWEEN 100 AND 599);

ALTER TABLE catalog_refresh_observations
    ADD COLUMN content_type TEXT
        CHECK (content_type IS NULL OR (
            length(content_type) BETWEEN 1 AND 4096
            AND instr(content_type, char(13)) = 0
            AND instr(content_type, char(10)) = 0
        ));

ALTER TABLE catalog_refresh_observations
    ADD COLUMN content_encoding TEXT
        CHECK (content_encoding IS NULL OR (
            length(content_encoding) BETWEEN 1 AND 4096
            AND instr(content_encoding, char(13)) = 0
            AND instr(content_encoding, char(10)) = 0
        ));

ALTER TABLE catalog_refresh_observations
    ADD COLUMN decoded_bytes INTEGER
        CHECK (decoded_bytes IS NULL OR decoded_bytes >= 0);

ALTER TABLE catalog_refresh_observations
    ADD COLUMN error_detail TEXT
        CHECK (error_detail IS NULL OR (
            length(error_detail) BETWEEN 1 AND 4096
            AND instr(error_detail, char(13)) = 0
            AND instr(error_detail, char(10)) = 0
        ));

UPDATE open_origin_state
SET circuit_state = 'CLOSED',
    reason_code = NULL,
    opened_at = NULL,
    retry_at = NULL,
    diagnostic_recheck_required = 0,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE circuit_state = 'FATAL_DIAGNOSTIC';
"#,
    },
    BundledMigration {
        filename: "0005_suspect_partial_classification.sql",
        sql: r#"-- bcsp:requires_foreign_keys_off
-- Rebuild open_pull_attempts so the classification CHECK admits
-- 'SUSPECT_PARTIAL_SNAPSHOT' (snapshot integrity gate, design
-- docs/design/2026-08-20-open-snapshot-integrity-gate.md v5).
--
-- Standard twelve-step table rebuild. The migration runner disables foreign
-- key enforcement OUTSIDE the surrounding transaction (the marker comment on
-- line 1 requests this), so dropping the old parent does not cascade into
-- open_batch_observations / open_attempt_catalog_sections; both child tables
-- re-attach to the renamed parent by name. The runner then walks
-- PRAGMA foreign_key_check inside the transaction and fails the migration on
-- any violation before re-enabling enforcement.

CREATE TABLE open_pull_attempts_v5 (
    attempt_id TEXT PRIMARY KEY,
    target_id TEXT NOT NULL REFERENCES catalog_targets(target_id) ON DELETE CASCADE,
    run_id TEXT NOT NULL,
    attempt_sequence INTEGER NOT NULL CHECK (attempt_sequence > 0),
    rutgers_day TEXT NOT NULL CHECK (
        length(rutgers_day) = 10
        AND substr(rutgers_day, 5, 1) = '-'
        AND substr(rutgers_day, 8, 1) = '-'
    ),
    captured_catalog_content_version INTEGER NOT NULL CHECK (captured_catalog_content_version > 0),
    started_at TEXT NOT NULL,
    completed_at TEXT,
    classification TEXT NOT NULL CHECK (classification IN (
        'STARTED',
        'VALID_APPLIED',
        'VALID_EMPTY_NO_ROWS',
        'FAILED',
        'UNSAFE_EMPTY',
        'UNSAFE_ZERO_INTERSECTION',
        'SUSPECT_PARTIAL_SNAPSHOT',
        'STALE_CATALOG_RACE',
        'INTERRUPTED'
    )),
    lane TEXT NOT NULL CHECK (lane IN (
        'GENERAL', 'ACTIVE_WATCH', 'FIRST_LOAD', 'MANUAL_REFRESH', 'CATALOG_RACE_RECHECK'
    )),
    http_status INTEGER CHECK (http_status IS NULL OR http_status BETWEEN 100 AND 599),
    cache_status TEXT CHECK (cache_status IS NULL OR length(cache_status) BETWEEN 1 AND 32),
    decoded_bytes INTEGER CHECK (decoded_bytes IS NULL OR decoded_bytes >= 0),
    decoded_body_sha256 TEXT CHECK (
        decoded_body_sha256 IS NULL OR (
            length(decoded_body_sha256) = 64
            AND decoded_body_sha256 NOT GLOB '*[^0-9a-f]*'
        )
    ),
    content_type TEXT CHECK (content_type IS NULL OR (length(content_type) BETWEEN 1 AND 4096 AND instr(content_type, char(13)) = 0 AND instr(content_type, char(10)) = 0)),
    etag TEXT CHECK (etag IS NULL OR (length(etag) BETWEEN 1 AND 4096 AND instr(etag, char(13)) = 0 AND instr(etag, char(10)) = 0)),
    cache_control TEXT CHECK (cache_control IS NULL OR (length(cache_control) BETWEEN 1 AND 4096 AND instr(cache_control, char(13)) = 0 AND instr(cache_control, char(10)) = 0)),
    response_date TEXT CHECK (response_date IS NULL OR (length(response_date) BETWEEN 1 AND 4096 AND instr(response_date, char(13)) = 0 AND instr(response_date, char(10)) = 0)),
    age_seconds INTEGER CHECK (age_seconds IS NULL OR age_seconds >= 0),
    last_modified TEXT CHECK (last_modified IS NULL OR (length(last_modified) BETWEEN 1 AND 4096 AND instr(last_modified, char(13)) = 0 AND instr(last_modified, char(10)) = 0)),
    retry_after TEXT CHECK (retry_after IS NULL OR (length(retry_after) BETWEEN 1 AND 4096 AND instr(retry_after, char(13)) = 0 AND instr(retry_after, char(10)) = 0)),
    retry_after_seconds INTEGER CHECK (retry_after_seconds IS NULL OR retry_after_seconds >= 0),
    canonical_set_sha256 TEXT CHECK (canonical_set_sha256 IS NULL OR length(canonical_set_sha256) = 64),
    state_sha256 TEXT CHECK (state_sha256 IS NULL OR length(state_sha256) = 64),
    source_value_count INTEGER NOT NULL DEFAULT 0 CHECK (source_value_count >= 0),
    catalog_section_count INTEGER NOT NULL DEFAULT 0 CHECK (catalog_section_count >= 0),
    intersection_count INTEGER NOT NULL DEFAULT 0 CHECK (intersection_count >= 0),
    orphan_count INTEGER NOT NULL DEFAULT 0 CHECK (orphan_count >= 0),
    duplicate_count INTEGER NOT NULL DEFAULT 0 CHECK (duplicate_count >= 0),
    requested_interval_seconds INTEGER CHECK (requested_interval_seconds IS NULL OR requested_interval_seconds > 0),
    effective_interval_seconds INTEGER CHECK (effective_interval_seconds IS NULL OR effective_interval_seconds > 0),
    schedule_lag_ms INTEGER CHECK (schedule_lag_ms IS NULL OR schedule_lag_ms >= 0),
    lkg_age_ms INTEGER CHECK (lkg_age_ms IS NULL OR lkg_age_ms >= 0),
    error_code TEXT CHECK (error_code IS NULL OR length(error_code) BETWEEN 1 AND 64),
    diagnostic_token TEXT CHECK (diagnostic_token IS NULL OR length(diagnostic_token) BETWEEN 1 AND 64),
    candidate_catalog_observation_id TEXT
        REFERENCES catalog_refresh_observations(observation_id),
    candidate_base_content_version INTEGER
        CHECK (candidate_base_content_version IS NULL OR candidate_base_content_version >= 0),
    UNIQUE (target_id, attempt_sequence)
) STRICT;

INSERT INTO open_pull_attempts_v5 (
    attempt_id, target_id, run_id, attempt_sequence, rutgers_day,
    captured_catalog_content_version, started_at, completed_at,
    classification, lane, http_status, cache_status, decoded_bytes,
    decoded_body_sha256, content_type, etag, cache_control, response_date,
    age_seconds, last_modified, retry_after, retry_after_seconds,
    canonical_set_sha256, state_sha256, source_value_count,
    catalog_section_count, intersection_count, orphan_count, duplicate_count,
    requested_interval_seconds, effective_interval_seconds, schedule_lag_ms,
    lkg_age_ms, error_code, diagnostic_token,
    candidate_catalog_observation_id, candidate_base_content_version
)
SELECT
    attempt_id, target_id, run_id, attempt_sequence, rutgers_day,
    captured_catalog_content_version, started_at, completed_at,
    classification, lane, http_status, cache_status, decoded_bytes,
    decoded_body_sha256, content_type, etag, cache_control, response_date,
    age_seconds, last_modified, retry_after, retry_after_seconds,
    canonical_set_sha256, state_sha256, source_value_count,
    catalog_section_count, intersection_count, orphan_count, duplicate_count,
    requested_interval_seconds, effective_interval_seconds, schedule_lag_ms,
    lkg_age_ms, error_code, diagnostic_token,
    candidate_catalog_observation_id, candidate_base_content_version
FROM open_pull_attempts;

DROP TABLE open_pull_attempts;

ALTER TABLE open_pull_attempts_v5 RENAME TO open_pull_attempts;

CREATE INDEX open_pull_attempts_target_sequence
    ON open_pull_attempts(target_id, attempt_sequence DESC);

CREATE INDEX open_pull_attempts_target_day
    ON open_pull_attempts(target_id, rutgers_day, attempt_sequence DESC);

CREATE UNIQUE INDEX open_pull_attempts_one_started_per_target
    ON open_pull_attempts(target_id) WHERE classification = 'STARTED';
"#,
    },
    BundledMigration {
        filename: "0006_gate_catalog_set_identity.sql",
        sql: r#"-- Bind gated Open attempts to the exact catalog section-set identity the
-- integrity-gate decision was computed against (snapshot integrity gate,
-- design docs/design/2026-08-20-open-snapshot-integrity-gate.md v5, restart
-- rebuild rules).
--
-- The restart rebuild resumes a persisted suspect run only when every member
-- carries the identity of the current serving catalog; content version
-- comparison alone cannot pin the section set (unpublished candidates may
-- reuse serving + 1). NULL marks ungated commits, hard rejections, failures,
-- and all pre-gate history -- the rebuild treats NULL as a run breaker.
--
-- Simple additive column: no table rebuild, no foreign-key work.
ALTER TABLE open_pull_attempts
    ADD COLUMN gate_catalog_set_identity TEXT
        CHECK (gate_catalog_set_identity IS NULL OR length(gate_catalog_set_identity) = 64);
"#,
    },
    BundledMigration {
        filename: "0007_catalog_derivation_state.sql",
        sql: r#"-- Record which catalog derivation rule produced the derived delivery columns
-- currently held in a target's serving tables (catalog_sections
-- delivery_modality/synchronicity; catalog_occurrences occurrence_kind,
-- modality, synchronicity, evidence, normalization_reason; the matching
-- catalog_provenance occurrence detail), design
-- docs/design/2026-09-01-synchronicity-derivation-v2.md.
--
-- The read-side projection recomputes the derivation from stored canonical
-- facts and rejects any serving row that disagrees, and publication skips
-- targets whose raw semantic hash is unchanged. A binary carrying a newer
-- derivation rule therefore re-derives the stored rows in place at startup,
-- keyed by this per-target stamp. An absent row means the legacy v1 rule
-- (v0.1.1 and earlier). Publication stamps the row only on the branches that
-- actually rewrote serving rows, so the stamp always describes the rows that
-- are really in the serving tables.
--
-- Simple additive table: no table rebuild, no foreign-key work.
CREATE TABLE catalog_derivation_state (
    target_id TEXT PRIMARY KEY REFERENCES catalog_targets(target_id) ON DELETE CASCADE,
    derivation_version INTEGER NOT NULL CHECK (derivation_version >= 1),
    stamped_at TEXT NOT NULL,
    stamped_observation_id TEXT
) STRICT;
"#,
    },
];
