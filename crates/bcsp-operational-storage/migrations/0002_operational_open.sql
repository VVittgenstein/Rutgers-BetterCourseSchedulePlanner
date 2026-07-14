CREATE TABLE open_batch_state (
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
