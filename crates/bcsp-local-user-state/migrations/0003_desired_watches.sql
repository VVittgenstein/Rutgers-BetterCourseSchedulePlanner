-- Desired-watch intent table (alert-delivery design v3.1, L1): one row per
-- section the user asked to monitor, with the watch policy captured at
-- START. Written on user START, removed on explicit STOP. Deliberately
-- carries NO watch identifier, ring consumption, or transport state:
-- restoring this table after a refresh or restart re-arms INTENT via a
-- fresh START, never a live watch.
CREATE TABLE personal_desired_watches_v1 (
    term_id TEXT NOT NULL CHECK (length(term_id) > 0),
    campus_code TEXT NOT NULL CHECK (length(campus_code) > 0),
    section_index TEXT NOT NULL CHECK (length(section_index) = 5),
    policy_json TEXT NOT NULL CHECK (json_valid(policy_json)),
    PRIMARY KEY (term_id, campus_code, section_index)
) STRICT;
