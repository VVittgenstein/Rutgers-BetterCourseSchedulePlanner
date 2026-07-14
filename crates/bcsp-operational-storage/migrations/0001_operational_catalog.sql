CREATE TABLE bcsp_operational_migrations (
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
