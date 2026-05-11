Subtask: ST-20251122-filter-rewrite-03-data-pipeline-dicts

Facts (implemented)
- Fetch pipeline now normalizes core attributes, sets has_core_attribute, and deletes/reinserts course_core_attributes per course with metadata/reference/effective term (scripts/fetch_soc_data.ts).
- Core filter SQL uses case-insensitive match (upper(cca.core_code)) so mixed/lower SOC codes are matched (api/src/queries/course_search.ts).
- /api/filters coreCodes now include fallback descriptions when metadata is missing or table empty; front-end fallback list expanded to observed core codes (api/src/queries/filters.ts, frontend/src/data/fallbackDictionary.ts).
- Added backfill utility and npm script to rebuild course_core_attributes from courses.core_json (scripts/backfill_core_attributes.ts, package.json: data:backfill-core).
- Ran `npm run data:backfill-core` on data/courses.sqlite → course_core_attributes rows=1,646; sample counts: AHO 60, AHP 90, AHQ 37, AHR 7, CCD 70, CCO 63, CE 165, ECN 4, GVT 7, HST 78. Courses processed=4,581.

Risks / TODO
- API must point to the populated DB (`SQLITE_FILE`); a different sqlite (e.g., data/local.db) may lack the table/data.
- Future fresh fetches should include the new pipeline or rerun backfill to repopulate core table.

Self-test
- npm run data:backfill-core (success; row counts above).

## Code Review - ST-20251122-filter-rewrite-03-data-pipeline-dicts - 2025-11-23T11:20:44Z
Codex Review: Didn't find any major issues. You're on a roll.
