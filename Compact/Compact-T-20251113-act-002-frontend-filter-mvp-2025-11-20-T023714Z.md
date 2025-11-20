## Fact Snapshot
- Backend `/api/filters` now reads SQLite dictionaries (terms/campuses/subjects/coreCodes/levels/deliveryMethods/instructors) via `fetchFiltersDictionary`, with safe fallbacks.
- `/api/courses` query schema expanded: index, sectionNumber, sectionStatus, hasPrerequisite, meetingCampus/building/room, sectionsLimit, meeting location filters, permission/instructor, and include `sections`. Course search now returns optional `sections` previews with meetings; new normalization for meeting/location/index/section status.
- Course search attaches section summaries and optional previews; respects new filters (prereq, permission, meeting location, instructor, status, index/section, delivery).
- Frontend filter state/UI extended: course/index/section inputs, credits min/max, core code checkboxes, section status and instructor filters, prerequisite/permission toggles, meeting campus/building/room inputs; chips and URL/param serialization updated to map to new API parameters (`sectionStatus`, `meetingCampus`, `building`, `room`, `hasPrerequisite`, `requiresPermission`, etc.).
- Filter dictionary fetcher consumes new instructors array and falls back to offline data.
- Course query hook maps core codes and section previews; subject/delivery/credits mapping unchanged.
- Course list renders instructor/core badges from course data.
- App adds view toggle (list/calendar) and Calendar uses section previews to build schedule blocks.

## Interface / Behavior Changes
- API: `/api/filters` now returns populated dictionaries from DB; `/api/courses` accepts new filters (index, sectionNumber, sectionStatus, hasPrerequisite, meetingCampus/building/room, sectionsLimit) and can include `sections` previews in response.
- Frontend: query params built with new keys; `include` now requests `sections`. UI exposes new filters and chips; view toggle adds calendar rendering.

## Risks / TODO
- Build/test not executed: `npm run -C frontend build` failed because `tsc` binary not installed (dependencies missing). Need `npm install` then rerun.
- Backend relies on SQLite data presence; empty tables fall back only partly (core codes/levels/delivery fall back, meeting location filters may yield empty).
- Calendar depends on section meeting data; if API omits `sections` include, calendar will be empty though list still works.
