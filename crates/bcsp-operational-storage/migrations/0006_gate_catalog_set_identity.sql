-- Bind gated Open attempts to the exact catalog section-set identity the
-- integrity-gate decision was computed against (snapshot integrity gate,
-- design docs/design/2026-08-20-open-snapshot-integrity-gate.md v5, restart
-- rebuild rules).
--
-- The restart rebuild resumes a persisted suspect run only when every member
-- carries the identity of the current serving catalog; content version
-- comparison alone cannot pin the section set (unpublished candidates may
-- reuse serving + 1). NULL marks ungated commits, hard rejections, failures,
-- and all pre-gate history -- the rebuild treats NULL as a run breaker.
--
-- Simple additive column: no table rebuild, no foreign-key work.
ALTER TABLE open_pull_attempts
    ADD COLUMN gate_catalog_set_identity TEXT
        CHECK (gate_catalog_set_identity IS NULL OR length(gate_catalog_set_identity) = 64);
