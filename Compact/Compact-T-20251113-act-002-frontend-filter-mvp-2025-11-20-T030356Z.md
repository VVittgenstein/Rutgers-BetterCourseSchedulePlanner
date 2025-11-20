## Fact Snapshot
- Frontend course filter builder now forwards all selected instructors to the API (array) instead of only the first selection (`frontend/src/state/courseFilters.ts`).
- `/api/courses` now accepts multi-value `instructor` parameters (`stringOrArrayParam`) and the search query matches any selected instructor via OR over instructor text (`api/src/routes/courses.ts`, `api/src/queries/course_search.ts`); request metrics include instructorCount.

## Interface / Behavior Changes
- API change: `instructor` query param supports multiple values; backend uses OR match across provided instructors.
- Frontend change: instructor filter chips and URL serialization already multi-select; outbound API params now mirror that selection.

## Risks / TODO
- Tests not rerun; prior frontend build failed due to missing deps/tsc. Need `npm install` and rebuild to validate.***

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
