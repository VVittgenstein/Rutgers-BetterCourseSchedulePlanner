CREATE TABLE personal_state_metadata_v1 (
    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
    state_revision INTEGER NOT NULL CHECK (state_revision > 0)
) STRICT;

INSERT INTO personal_state_metadata_v1(singleton_id, state_revision) VALUES (1, 1);

CREATE TABLE personal_current_filters_v1 (
    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
    revision INTEGER NOT NULL CHECK (revision > 0),
    has_value INTEGER NOT NULL CHECK (has_value IN (0, 1)),
    association_json TEXT CHECK (association_json IS NULL OR json_valid(association_json)),
    schema_version INTEGER CHECK (schema_version IS NULL OR schema_version > 0),
    snapshot_json TEXT CHECK (snapshot_json IS NULL OR json_valid(snapshot_json)),
    CHECK (
        (has_value = 1 AND association_json IS NOT NULL
            AND schema_version IS NOT NULL AND snapshot_json IS NOT NULL)
        OR
        (has_value = 0 AND association_json IS NULL
            AND schema_version IS NULL AND snapshot_json IS NULL)
    )
) STRICT;

CREATE TABLE personal_saved_views_v1 (
    view_id TEXT PRIMARY KEY CHECK (length(view_id) = 36),
    name TEXT NOT NULL CHECK (length(name) > 0 AND trim(name) = name),
    name_key TEXT NOT NULL UNIQUE CHECK (length(name_key) > 0),
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    revision INTEGER NOT NULL CHECK (revision > 0),
    snapshot_json TEXT NOT NULL CHECK (json_valid(snapshot_json)),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= created_at_ms)
) STRICT;

CREATE INDEX personal_saved_views_name
    ON personal_saved_views_v1(name_key, view_id);
