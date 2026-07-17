# Compact – ST-20251122-filter-rewrite-01-frontend-state-ui

## Scope
Front-end state/UI slimming for course filters per T-20251122-filter-rewrite: remove quick tags/section/location/permission/waitlist knobs; keep credits/open-only; add core/exam codes; meeting days must be subset-only.

## Implemented facts (code + self-test)
- Filter state trimmed (`frontend/src/state/courseFilters.ts`): removed courseNumber/sectionIndex/sectionNumber/instructors/sectionStatuses/meetingCampuses/location/tags/keywords/permission; `openStatus` now `all|openOnly`; added `examCodes`; meeting filter unchanged except subset semantics used in UI logic.
- Query serialization/parsing updated: only emits/reads term, campus, subject, q, level, coreCode, examCode, credits min/max, delivery, meetingDays/start/end, prerequisite, pagination, sort; drops removed params and waitlist flag. Throws if `term` missing when building query.
- FilterPanel UI rewritten (`frontend/src/components/FilterPanel.tsx`): sections now include term/campus, keyword, open-only toggle, subject picker, meeting days/time with subset hint, credits, core/exam multi-select, prerequisite pills, delivery/level; removed course number, section/index, instructor, status, permission, location, tags, waitlist button. Chips mirror the new set (keyword/subject/level/delivery/core/exam/meeting/credits/prerequisite/open-only).
- Dictionary shape changed (`frontend/src/api/filters.ts`, `frontend/src/data/fallbackDictionary.ts`, `frontend/src/api/types.ts`): dropped tags/instructors; added `examCodes` mapping from `/filters` or fallback list.
- Local filtering semantics adjusted (`frontend/src/hooks/useCourseQuery.ts`, `frontend/src/dev/ComponentPlayground.tsx`): meeting days now require **all** section meetings to stay within selected days; time window uses `every` check and fails if meetings missing when filters set.
- i18n refreshed (`frontend/i18n/messages.json`): removed strings for deleted controls; added meeting subset hint, exam/core labels; open status only “All/Open seats”.
- Self-test: `npm run -C frontend build` passes.

## Interface/behavior changes
- `CourseFilterState` and URL/query params now exclude section/status/location/instructor/tag/permission fields; consumers expecting these will break. `openStatus` no longer supports waitlist; `examCodes` added.
- Filters dictionary contract now requires `examCodes` array and no longer provides `tags`/`instructors`.
- Meeting filter behavior is stricter: selected days must cover all meetings (subset-only), and time windows must be satisfied by every meeting.
- Core/exam codes surfaced as checkbox blocks; prerequisite filter remains three-state but moved to course attributes block.

## Risks / TODO / external deps
- Backend `/api/courses` and `/api/filters` must align with new params (`examCode`, removed fields, meeting subset logic); until then UI/query mismatch possible.
- Fallback exam code list is static; ensure data pipeline populates real codes (depends on other subtasks).
- Legacy URLs with removed params are ignored (silently dropped); confirm acceptable or add redirects/messaging.
