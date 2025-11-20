## Fact Snapshot
- Frontend course filter builder forwards all selected instructors to the API (array) instead of only the first selection (`frontend/src/state/courseFilters.ts`).
- `/api/courses` accepts multi-value `instructor` parameters (`stringOrArrayParam`) and the search query matches any selected instructor via OR over instructor text (`api/src/routes/courses.ts`, `api/src/queries/course_search.ts`); request metrics include instructorCount.
- Instructor normalization now tolerates scalar strings (or comma-delimited strings) without splitting into characters when the query bypasses the schema (e.g., direct executeCourseSearch calls); strings are normalized to full tokens before filtering.

## Interface / Behavior Changes
- API change: `instructor` query param supports multiple values; backend uses OR match across provided instructors and handles both scalar and array inputs safely.
- Frontend change: instructor filter chips and URL serialization already multi-select; outbound API params mirror that selection.

## Risks / TODO
- Tests not rerun; prior frontend build failed due to missing deps/tsc. Need `npm install` and rebuild to validate.

## Code Review - T-20251113-act-002-frontend-filter-mvp - 2025-11-20T03:04:52Z
---review-start---
frontend/src/state/courseFilters.ts
Comment on lines +124 to +126


  if (state.coreCodes.length) params.coreCode = [...state.coreCodes];
  if (state.delivery.length) params.delivery = [...state.delivery];
  if (state.instructors.length) params.instructor = state.instructors[0];|

 
@chatgpt-codex-connector
chatgpt-codex-connector bot

P2 Badge Preserve multiple instructor filters in API query

The course query builder only forwards the first selected instructor (state.instructors[0]) to the API, even though the UI exposes instructor filters as a multi-select checkbox list and serializes all of them into the URL. As a result, whenever a user selects more than one instructor, the backend request still filters by only the earliest selection and silently drops the others, producing broader results than the UI indicates.
---review-end---

## Code Review - T-20251113-act-002-frontend-filter-mvp - 2025-11-20T03:28:39Z
---review-start---
api/src/queries/course_search.ts
Comment on lines +234 to +238



  const instructors = normalizeStringList(query.instructor, (value) => value.toLowerCase());
  if (instructors.length) {
    const instructorClauses = instructors.map(
      (needle) =>
        `(s_filter.instructors_text IS NOT NULL AND s_filter.instructors_text <> '' AND instr(lower(s_filter.instructors_text), ${binder.bind(needle)}) > 0)`,



@chatgpt-codex-connector
chatgpt-codex-connector bot


P1 Badge Prevent string instructor filters from degrading to char ORs

If executeCourseSearch receives a string instructor filter (as in api/tests/course_search.test.ts where instructor: 'doe' is passed), the new normalizeStringList(query.instructor, …) treats the string as an iterable and splits it into characters. The generated SQL then ORs over single-letter needles ('d' OR 'o' OR 'e'), so any non-empty instructor text matches and the filter effectively disappears, breaking the instructor-focused test and any callers that still supply a scalar. Either normalize scalar input back to an array of full strings or keep supporting a string path to avoid this regression.
---review-end---

## Code Review - T-20251113-act-002-frontend-filter-mvp - 2025-11-20T03:57:22Z
---review-start---
Codex Review: Didn't find any major issues. More of your lovely PRs please.
---review-end---

## Code Review - T-20251113-act-002-frontend-filter-mvp - 2025-11-20T03:29:06Z
---review-start---
api/src/queries/course_search.ts
Comment on lines +234 to +238



  const instructors = normalizeStringList(query.instructor, (value) => value.toLowerCase());
  if (instructors.length) {
    const instructorClauses = instructors.map(
      (needle) =>
        `(s_filter.instructors_text IS NOT NULL AND s_filter.instructors_text <> '' AND instr(lower(s_filter.instructors_text), ${binder.bind(needle)}) > 0)`,



@chatgpt-codex-connector
chatgpt-codex-connector bot


P1 Badge Prevent string instructor filters from degrading to char ORs

If executeCourseSearch receives a string instructor filter (as in api/tests/course_search.test.ts where instructor: 'doe' is passed), the new normalizeStringList(query.instructor, …) treats the string as an iterable and splits it into characters. The generated SQL then ORs over single-letter needles ('d' OR 'o' OR 'e'), so any non-empty instructor text matches and the filter effectively disappears, breaking the instructor-focused test and any callers that still supply a scalar. Either normalize scalar input back to an array of full strings or keep supporting a string path to avoid this regression.
---review-end---
