## Filter component playground

The `frontend` package hosts the React components requested in `ST-20251113-act-002-02-filter-components`. It is powered by Vite for a lightweight dev loop and bundles the following MVP widgets:

- `FilterPanel` renders the primary/secondary filter controls backed by the shared `CourseFilterState`.
- `TagChip` encapsulates the removable pill UI shared by the filter header and quick toggles.
- `SchedulePreview` visualizes a week view using mock section meetings so we can validate the calendar layout independently from the API.

### Run locally

```bash
cd frontend
npm install        # already executed in this patch
npm run dev        # launches http://localhost:5174 with mock data
npm run build      # type-check + production bundle
```

The dev page (`src/dev/ComponentPlayground.tsx`) wires the components together with the static dictionaries in `src/dev/mockData.ts`. Updating the mock data lets us demo multi-select/clear interactions, new tags, and time-slot scenarios before wiring the real API responses.
