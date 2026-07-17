ALTER TABLE open_pull_attempts
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
