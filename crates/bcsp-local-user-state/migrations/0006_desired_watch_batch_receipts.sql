-- One durable idempotency receipt per local desired-watch batch.
--
-- A 255-Section gesture must not consume 255 rows from the single-mutation
-- ledger. The batch fingerprint covers every ordered item; the outcome keeps
-- the complete committed stamp set (or the terminal refusal) so a retry after
-- an uncertain HTTP result replays one indivisible decision.
CREATE TABLE personal_desired_watch_batch_receipts_v1 (
    authority_generation INTEGER NOT NULL
        CHECK (authority_generation BETWEEN 1 AND 9007199254740991),
    mutation_id TEXT NOT NULL CHECK (length(mutation_id) = 36),
    fingerprint TEXT NOT NULL CHECK (length(fingerprint) = 64),
    outcome_json TEXT NOT NULL CHECK (json_valid(outcome_json)),
    PRIMARY KEY (authority_generation, mutation_id)
) STRICT;
