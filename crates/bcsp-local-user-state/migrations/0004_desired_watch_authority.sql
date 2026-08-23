-- Desired-watch authority state (revision/CAS co-editing design, v7.2).
--
-- The 10003 table stored intent as "a row exists". That cannot answer the
-- question a late command asks -- "was this decided before or after the one
-- already applied?" -- so it is rebuilt around a per-section revision, and a
-- removal now leaves a TOMBSTONE (desired = 0) rather than vanishing: a row
-- that disappears leaves a stale basedOnRevision nothing to compare against,
-- which is how a delayed START would resurrect intent the user cancelled.
--
-- Still deliberately absent, as in 10003: any watch identifier, ring
-- consumption, or transport state. Restoring this table re-arms INTENT.
ALTER TABLE personal_desired_watches_v1 RENAME TO personal_desired_watches_pre10004;

CREATE TABLE personal_desired_watches_v1 (
    term_id TEXT NOT NULL CHECK (length(term_id) > 0),
    campus_code TEXT NOT NULL CHECK (length(campus_code) > 0),
    section_index TEXT NOT NULL CHECK (length(section_index) = 5),
    desired INTEGER NOT NULL CHECK (desired IN (0, 1)),
    policy_json TEXT CHECK (policy_json IS NULL OR json_valid(policy_json)),
    revision INTEGER NOT NULL CHECK (revision BETWEEN 1 AND 9007199254740991),
    materialization_epoch INTEGER NOT NULL
        CHECK (materialization_epoch BETWEEN 1 AND 9007199254740991),
    PRIMARY KEY (term_id, campus_code, section_index),
    CHECK ((desired = 1) = (policy_json IS NOT NULL))
) STRICT;

INSERT INTO personal_desired_watches_v1 (
    term_id,
    campus_code,
    section_index,
    desired,
    policy_json,
    revision,
    materialization_epoch
)
SELECT
    term_id,
    campus_code,
    section_index,
    1,
    policy_json,
    ROW_NUMBER() OVER (ORDER BY term_id, campus_code, section_index),
    ROW_NUMBER() OVER (ORDER BY term_id, campus_code, section_index)
FROM personal_desired_watches_pre10004;

DROP TABLE personal_desired_watches_pre10004;

-- Receipts make a repeated mutation replay its original outcome instead of
-- being evaluated again. Keyed by (generation, mutationId) only: putting the
-- section in the key would let the same id land twice under two sections,
-- which is the collision the ledger exists to report. Section and fingerprint
-- are compared, not keyed.
CREATE TABLE personal_desired_watch_receipts_v1 (
    authority_generation INTEGER NOT NULL
        CHECK (authority_generation BETWEEN 1 AND 9007199254740991),
    mutation_id TEXT NOT NULL CHECK (length(mutation_id) = 36),
    term_id TEXT NOT NULL CHECK (length(term_id) > 0),
    campus_code TEXT NOT NULL CHECK (length(campus_code) > 0),
    section_index TEXT NOT NULL CHECK (length(section_index) = 5),
    fingerprint TEXT NOT NULL CHECK (length(fingerprint) = 64),
    outcome_json TEXT NOT NULL CHECK (json_valid(outcome_json)),
    PRIMARY KEY (authority_generation, mutation_id)
) STRICT;

-- The four authority counters live on the singleton metadata row. The table is
-- rebuilt rather than ALTERed so every counter carries its own bound: all four
-- values cross the wire to a JavaScript client, so none may exceed
-- Number.MAX_SAFE_INTEGER.
ALTER TABLE personal_state_metadata_v1 RENAME TO personal_state_metadata_pre10004;

CREATE TABLE personal_state_metadata_v1 (
    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
    state_revision INTEGER NOT NULL CHECK (state_revision > 0),
    desired_watch_authority_generation INTEGER NOT NULL
        CHECK (desired_watch_authority_generation BETWEEN 1 AND 9007199254740991),
    desired_watch_revision_counter INTEGER NOT NULL
        CHECK (desired_watch_revision_counter BETWEEN 0 AND 9007199254740991),
    desired_watch_materialization_counter INTEGER NOT NULL
        CHECK (desired_watch_materialization_counter BETWEEN 0 AND 9007199254740991),
    desired_watch_actor_incarnation INTEGER NOT NULL
        CHECK (desired_watch_actor_incarnation BETWEEN 1 AND 9007199254740991)
) STRICT;

INSERT INTO personal_state_metadata_v1 (
    singleton_id,
    state_revision,
    desired_watch_authority_generation,
    desired_watch_revision_counter,
    desired_watch_materialization_counter,
    desired_watch_actor_incarnation
)
SELECT
    singleton_id,
    state_revision,
    1,
    (SELECT COALESCE(MAX(revision), 0) FROM personal_desired_watches_v1),
    (SELECT COALESCE(MAX(materialization_epoch), 0) FROM personal_desired_watches_v1),
    1
FROM personal_state_metadata_pre10004;

DROP TABLE personal_state_metadata_pre10004;
