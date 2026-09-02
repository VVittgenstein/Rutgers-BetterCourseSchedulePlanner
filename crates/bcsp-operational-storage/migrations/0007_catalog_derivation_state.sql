-- Record which catalog derivation rule produced the derived delivery columns
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
