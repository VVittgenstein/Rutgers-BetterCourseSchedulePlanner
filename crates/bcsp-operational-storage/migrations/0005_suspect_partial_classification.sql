-- bcsp:requires_foreign_keys_off
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
