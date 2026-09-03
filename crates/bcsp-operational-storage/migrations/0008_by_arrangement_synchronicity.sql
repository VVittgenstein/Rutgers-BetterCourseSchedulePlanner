-- bcsp:requires_foreign_keys_off
-- Rebuild the four catalog tables whose synchronicity CHECK predates
-- BY_ARRANGEMENT, the value Rutgers itself shows as "Hours by arrangement" for
-- a by-arrangement meeting that is not online or remote (derivation v3, design
-- docs/design/2026-09-02-filter-semantics-and-by-arrangement.md).
--
-- Standard twelve-step table rebuild, as in
-- 0005_suspect_partial_classification.sql. The migration runner disables
-- foreign key enforcement OUTSIDE the surrounding transaction (the marker
-- comment on line 1 requests this), so dropping a parent does not cascade into
-- its children; the children re-attach to the renamed parent by name. The
-- runner then walks PRAGMA foreign_key_check inside the transaction and fails
-- the migration on any violation before re-enabling enforcement.
--
-- Only the synchronicity CHECK changes. Every other column, constraint, key
-- and index is reproduced exactly, and no stored value is rewritten here:
-- re-derivation to v3 happens afterwards through catalog_derivation_state.

CREATE TABLE catalog_staging_sections_v3 (
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
        'SYNC', 'ASYNC', 'MIXED', 'BY_ARRANGEMENT', 'UNSPECIFIED', 'UNKNOWN'
    )),
    canonical_facts_json TEXT NOT NULL CHECK (json_valid(canonical_facts_json)),
    canonical_sha256 TEXT NOT NULL CHECK (length(canonical_sha256) = 64),
    PRIMARY KEY (observation_id, section_index),
    FOREIGN KEY (observation_id, course_string, fingerprint)
        REFERENCES catalog_staging_course_variants(observation_id, course_string, fingerprint) ON DELETE CASCADE
) STRICT;

INSERT INTO catalog_staging_sections_v3
SELECT
    observation_id, target_id, section_index, course_string, fingerprint,
    section_number, catalog_status, section_course_type, delivery_modality,
    synchronicity, canonical_facts_json, canonical_sha256
FROM catalog_staging_sections;

DROP TABLE catalog_staging_sections;

ALTER TABLE catalog_staging_sections_v3 RENAME TO catalog_staging_sections;

CREATE INDEX catalog_staging_sections_variant_fk
    ON catalog_staging_sections(observation_id, course_string, fingerprint);

CREATE TABLE catalog_staging_occurrences_v3 (
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
        'SYNC', 'ASYNC', 'MIXED', 'BY_ARRANGEMENT', 'UNSPECIFIED', 'UNKNOWN'
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

INSERT INTO catalog_staging_occurrences_v3
SELECT
    observation_id, target_id, section_index, occurrence_key, ordinal, weekday,
    start_minute, end_minute, time_knowledge, requiredness, occurrence_kind,
    modality, synchronicity, evidence, normalization_reason, location, building,
    room, raw_sha256, canonical_facts_json
FROM catalog_staging_occurrences;

DROP TABLE catalog_staging_occurrences;

ALTER TABLE catalog_staging_occurrences_v3 RENAME TO catalog_staging_occurrences;

CREATE TABLE catalog_sections_v3 (
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
        'SYNC', 'ASYNC', 'MIXED', 'BY_ARRANGEMENT', 'UNSPECIFIED', 'UNKNOWN'
    )),
    canonical_facts_json TEXT NOT NULL CHECK (json_valid(canonical_facts_json)),
    canonical_sha256 TEXT NOT NULL CHECK (length(canonical_sha256) = 64),
    content_version INTEGER NOT NULL CHECK (content_version >= 1),
    PRIMARY KEY (target_id, section_index),
    FOREIGN KEY (target_id, course_string, fingerprint)
        REFERENCES catalog_course_variants(target_id, course_string, fingerprint) ON DELETE CASCADE
) STRICT;

INSERT INTO catalog_sections_v3
SELECT
    target_id, section_index, course_string, fingerprint, section_number,
    catalog_status, section_course_type, delivery_modality, synchronicity,
    canonical_facts_json, canonical_sha256, content_version
FROM catalog_sections;

DROP TABLE catalog_sections;

ALTER TABLE catalog_sections_v3 RENAME TO catalog_sections;

CREATE INDEX catalog_sections_variant_fk
    ON catalog_sections(target_id, course_string, fingerprint);

CREATE TABLE catalog_occurrences_v3 (
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
        'SYNC', 'ASYNC', 'MIXED', 'BY_ARRANGEMENT', 'UNSPECIFIED', 'UNKNOWN'
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

INSERT INTO catalog_occurrences_v3
SELECT
    target_id, section_index, occurrence_key, ordinal, weekday, start_minute,
    end_minute, time_knowledge, requiredness, occurrence_kind, modality,
    synchronicity, evidence, normalization_reason, location, building, room,
    raw_sha256, canonical_facts_json, content_version
FROM catalog_occurrences;

DROP TABLE catalog_occurrences;

ALTER TABLE catalog_occurrences_v3 RENAME TO catalog_occurrences;
