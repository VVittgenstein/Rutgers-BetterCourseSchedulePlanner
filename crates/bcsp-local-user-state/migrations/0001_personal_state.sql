CREATE TABLE personal_settings_v1 (
    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
    revision INTEGER NOT NULL CHECK (revision > 0),
    settings_json TEXT NOT NULL CHECK (json_valid(settings_json))
) STRICT;

CREATE TABLE personal_selected_sections_v1 (
    position INTEGER PRIMARY KEY CHECK (position BETWEEN 0 AND 8),
    term_id TEXT NOT NULL,
    campus_code TEXT NOT NULL,
    section_index TEXT NOT NULL,
    UNIQUE (term_id, campus_code, section_index)
) STRICT;

CREATE TABLE personal_episode_summaries_v1 (
    term_id TEXT NOT NULL,
    campus_code TEXT NOT NULL,
    section_index TEXT NOT NULL,
    run_id TEXT NOT NULL CHECK (length(run_id) = 36),
    episode_id TEXT NOT NULL CHECK (length(episode_id) = 36),
    state TEXT NOT NULL CHECK (state IN (
        'UNACKNOWLEDGED', 'ACKNOWLEDGED', 'TIMED_OUT', 'CLOSED'
    )),
    mode TEXT NOT NULL CHECK (mode IN ('ONE_SHOT', 'CONTINUOUS')),
    first_observed_at_ms INTEGER NOT NULL CHECK (first_observed_at_ms >= 0),
    last_observed_at_ms INTEGER NOT NULL CHECK (last_observed_at_ms >= first_observed_at_ms),
    acknowledged_at_ms INTEGER CHECK (
        acknowledged_at_ms IS NULL OR acknowledged_at_ms >= first_observed_at_ms
    ),
    timed_out_at_ms INTEGER CHECK (
        timed_out_at_ms IS NULL OR timed_out_at_ms >= first_observed_at_ms
    ),
    closed_at_ms INTEGER CHECK (
        closed_at_ms IS NULL OR closed_at_ms >= last_observed_at_ms
    ),
    disposition_json TEXT CHECK (
        disposition_json IS NULL OR json_valid(disposition_json)
    ),
    audible_count INTEGER NOT NULL CHECK (audible_count >= 0),
    observation_count INTEGER NOT NULL CHECK (observation_count > 0),
    last_observation_id TEXT NOT NULL CHECK (length(last_observation_id) = 36),
    PRIMARY KEY (term_id, campus_code, section_index, run_id, episode_id),
    CHECK (NOT (acknowledged_at_ms IS NOT NULL AND timed_out_at_ms IS NOT NULL)),
    CHECK ((state = 'CLOSED') = (closed_at_ms IS NOT NULL)),
    CHECK ((state = 'UNACKNOWLEDGED') = (disposition_json IS NULL))
) STRICT;

CREATE INDEX personal_episode_summaries_recent
    ON personal_episode_summaries_v1(last_observed_at_ms DESC);

CREATE TABLE personal_episode_actions_v1 (
    action_id TEXT PRIMARY KEY CHECK (length(action_id) = 36),
    term_id TEXT NOT NULL,
    campus_code TEXT NOT NULL,
    section_index TEXT NOT NULL,
    run_id TEXT NOT NULL CHECK (length(run_id) = 36),
    episode_id TEXT NOT NULL CHECK (length(episode_id) = 36),
    action_kind TEXT NOT NULL CHECK (action_kind IN (
        'OPENED', 'UPDATED', 'ACKNOWLEDGED', 'TIMED_OUT', 'RESUMED', 'CLOSED',
        'CUE_PLAYED', 'CUE_MUTED', 'CUE_AUTOPLAY_BLOCKED', 'CUE_FAILED',
        'ALERT_DISMISSED', 'POLICY_UPDATED'
    )),
    occurred_at_ms INTEGER NOT NULL CHECK (occurred_at_ms >= 0),
    FOREIGN KEY (term_id, campus_code, section_index, run_id, episode_id)
        REFERENCES personal_episode_summaries_v1(
            term_id, campus_code, section_index, run_id, episode_id
        ) ON DELETE CASCADE
) STRICT;

CREATE INDEX personal_episode_actions_episode_time
    ON personal_episode_actions_v1(
        term_id, campus_code, section_index, run_id, episode_id, occurred_at_ms DESC
    );
