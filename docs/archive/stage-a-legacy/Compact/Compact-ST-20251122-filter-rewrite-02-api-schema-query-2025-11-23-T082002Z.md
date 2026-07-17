Subtask: ST-20251122-filter-rewrite-02-api-schema-query  
Status: Implemented, waiting on test run (Node missing locally)

Confirmed changes
- API schema `/api/courses` (api/src/routes/courses.ts): removed deprecated query params (courseNumber/index/sectionNumber/sectionStatus/instructor/building/room/requiresPermission); added `examCode` array; summarizer logging pruned accordingly.
- Query logic (api/src/queries/course_search.ts): added examCode normalization and filtering against `sections.exam_code`; meetingDays filter now enforces subset semantics (week_mask > 0 and NOT EXISTS meetings with days outside selection) while still honoring time window/campus filters; section preview uses same filters and only returns matching sections. Removed logic tied to the deleted params.
- Tests (api/tests/course_search.test.ts): fixtures extended with exam_code/open_status/meeting metadata; new coverage for meetingDays subset + delivery/time window, and for examCode filtering with sections include; removed requiresPermission/instructor-focused test.

Interface/behavior shifts
- GET /courses query params: `examCode` added; courseNumber/index/sectionNumber/sectionStatus/instructor/building/room/requiresPermission no longer accepted or logged.
- meetingDays semantics: now require all meetings to fall within selected days (subsets), not just intersect; results exclude sections with any meeting outside the chosen set.
- Sections payload when included honors examCode filter (only matching sections returned).

Risks / TODO
- Tests not executed: `node` executable absent; rerun `npx tsx api/tests/course_search.test.ts` after installing Node.
- Recommend manual verification with real data for meetingDays subset and examCode filtering once runtime is available.

## Code Review - ST-20251122-filter-rewrite-02-api-schema-query - 2025-11-23T08:42:26Z
Codex Review: Didn't find any major issues. Swish!
