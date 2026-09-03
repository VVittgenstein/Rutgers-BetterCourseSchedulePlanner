-- Expand the persisted selection order to the product's 255-section limit.
-- This is a forward-only rebuild: 10001 is already checksummed in released
-- databases and must remain byte-for-byte unchanged.
ALTER TABLE personal_selected_sections_v1
    RENAME TO personal_selected_sections_pre10005;

CREATE TABLE personal_selected_sections_v1 (
    position INTEGER PRIMARY KEY CHECK (position BETWEEN 0 AND 254),
    term_id TEXT NOT NULL,
    campus_code TEXT NOT NULL,
    section_index TEXT NOT NULL,
    UNIQUE (term_id, campus_code, section_index)
) STRICT;

INSERT INTO personal_selected_sections_v1 (
    position,
    term_id,
    campus_code,
    section_index
)
SELECT
    position,
    term_id,
    campus_code,
    section_index
FROM personal_selected_sections_pre10005
ORDER BY position;

DROP TABLE personal_selected_sections_pre10005;
