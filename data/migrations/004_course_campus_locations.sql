CREATE TABLE IF NOT EXISTS course_campus_locations (
  course_id INTEGER NOT NULL,
  term_id TEXT NOT NULL,
  campus_code TEXT NOT NULL,
  location_code TEXT NOT NULL,
  location_desc TEXT,
  PRIMARY KEY (course_id, location_code),
  FOREIGN KEY (course_id) REFERENCES courses(course_id) ON DELETE CASCADE,
  FOREIGN KEY (term_id) REFERENCES terms(term_id),
  FOREIGN KEY (campus_code) REFERENCES campuses(campus_code)
);

CREATE INDEX IF NOT EXISTS idx_course_campus_locations_lookup
  ON course_campus_locations(term_id, campus_code, location_code);

CREATE INDEX IF NOT EXISTS idx_course_campus_locations_code
  ON course_campus_locations(location_code);

INSERT OR IGNORE INTO course_campus_locations (course_id, term_id, campus_code, location_code, location_desc)
SELECT
  c.course_id,
  c.term_id,
  c.campus_code,
  UPPER(TRIM(COALESCE(CAST(json_extract(loc.value, '$.code') AS TEXT), CAST(json_extract(loc.value, '$') AS TEXT)))) AS location_code,
  COALESCE(CAST(json_extract(loc.value, '$.description') AS TEXT), CAST(json_extract(loc.value, '$') AS TEXT)) AS location_desc
FROM courses c
JOIN json_each(c.source_payload, '$.campusLocations') AS loc
WHERE UPPER(TRIM(COALESCE(CAST(json_extract(loc.value, '$.code') AS TEXT), CAST(json_extract(loc.value, '$') AS TEXT)))) IS NOT NULL
  AND UPPER(TRIM(COALESCE(CAST(json_extract(loc.value, '$.code') AS TEXT), CAST(json_extract(loc.value, '$') AS TEXT)))) <> '';
