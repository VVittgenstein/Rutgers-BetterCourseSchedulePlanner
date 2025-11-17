# Rutgers SOC API Probe Notes

_Last updated: 2025-11-16 (field matrix refresh)_

## Probe CLI quickstart
- Install dependencies once with `npm install` (TypeScript + tsx runner are already configured).
- Run probes via `npm run soc:probe -- [flags]`. Required flags are `--term <semester>` and `--campus <code or comma list>`. Optional flags include `--subject <code>` (local filter + request context), `--endpoint courses|openSections`, `--samples <n>` and `--timeout <ms>`.
- Semesters accept multiple formats (`12024`, `20241`, `FA2024`). The script normalizes to SOC's `year` + `term` (0/1/7/9) parameters before calling `https://classes.rutgers.edu/soc/api/*`.
- Errors bubble up as JSON logs that include `requestId`, endpoint, HTTP status (when available) and a retry hint so failed experiments can be reproduced.

## Scenario snapshots (2024 data)
### 1. Spring 2024 • New Brunswick • Subject 198 (courses endpoint)
Command:
```bash
npm run soc:probe -- --term 12024 --campus NB --subject 198 --samples 2
```
Summary:
- Request ID `2ce8c3a5-b93b-4d10-a293-7a596419d6c6`, URL automatically expanded to `year=2024&term=1&campus=NB&subject=198`.
- Status `200 OK`, 195 ms end-to-end, decoded payload size ≈ 20.4 MB.
- Records returned: total campus payload 4,530 course objects, 66 match subject `198` locally.
- Sample excerpt:
  - `198-107 COMPUT MATH & SCIENC` (Busch campus, 2 sections, 2 open, 4 credits)
  - `198-110 PRINCIPLES OF CS` (Busch campus, 4 sections, 2 open, 3 credits)

### 2. Fall 2024 • Newark • Subject 640 (courses endpoint)
Command:
```bash
npm run soc:probe -- --term 92024 --campus NK --subject 640 --samples 2
```
Summary:
- Request ID `326816d2-727b-494a-803e-dde0892200cc`.
- Status `200 OK`, 274 ms, decoded payload ≈ 4.8 MB.
- Records: 1,286 Newark courses in total, 30 locally filtered rows for subject `640`.
- Sample excerpt:
  - `640-033 MATH LIB ARTS INTENS` (Newark campus, 1 section, 0 open, 3 credits)
  - `640-038 INTER ALGEBRA INT` (Newark campus, 18 sections, 4 open, 3 credits)

### 3. Summer 2024 • Camden • Open sections heartbeat
Command:
```bash
npm run soc:probe -- --term 72024 --campus CM --endpoint openSections --samples 5
```
Summary:
- Request ID `54ca2220-0c8e-45c2-b147-faa30a69735c`.
- Status `200 OK`, 140 ms, decoded payload ≈ 1.9 KB.
- Open section indexes returned: 238 total. Sample indexes `00622, 04667, 06151, 06152, 06146` confirm non-empty coverage for Camden summer data.

## Observations & follow-ups
- The public SOC endpoints ignore `subject` in the query string; filtering happens client-side (the probe mirrors this behavior and reports both total + filtered counts so downstream tasks know how much needs to be cached locally).
- `level` also has no effect (U/G return the same payload); we still record the requested level per combo so downstream jobs understand the intended audience.
- Responses are served with `Content-Encoding: gzip`; `fetch` transparently decodes. Any custom HTTP client must send `Accept-Encoding` or handle gzip manually.
- Typical headers include `Cache-Control: max-age=900` and a persistent `ETag`, which hints that caching probes for at least 15 minutes will avoid redundant downloads.
- When the server returns non-2xx status codes, the probe emits structured JSON errors (`errorType`, `requestId`, `retryHint`). These logs can be copied into future tickets for reproducibility.
- Next experiments (field matrix & rate-limit tasks) can reuse the CLI or import the `performProbe` helper to fan out term/campus batches with controlled concurrency.

## Field matrix batch (2025-11-16)

### Runner scope
- Command: `npm run soc:field-matrix` (Python script at `scripts/soc_field_matrix.py`) downloads each `term+campus` payload once, reuses it for 2×3×5 subject permutations with level hints (U/G).
- Output: `docs/soc_field_matrix.csv` enumerates 78 fields across course, section, meeting, instructor, and openSections scopes with presence ratios + FR mapping tags.
- Combos executed: 42 (`term {12024, 92024}` × `campus {NB, NK, CM}` × `subject {198, 640, 750, 960, 014}` × subject-defined `level` lists). Subject 014 in FA24 Camden is the smallest slice (3 courses / 5 sections); 640 in SP24 NB is the largest filtered subset (82 courses / 429 sections).

### Dataset coverage by campus

| Term | Campus | Courses | Sections | Meeting rows | Open indexes |
| --- | --- | ---:| ---:| ---:| ---:|
| 12024 | NB | 4,530 | 11,515 | 16,454 | 8,494 |
| 12024 | NK | 1,327 | 2,519 | 3,642 | 1,825 |
| 12024 | CM | 905 | 1,837 | 2,276 | 1,452 |
| 92024 | NB | 4,367 | 11,882 | 17,349 | 8,614 |
| 92024 | NK | 1,286 | 2,582 | 3,771 | 1,936 |
| 92024 | CM | 907 | 1,914 | 2,392 | 1,542 |

Totals across the six payloads: 13,322 courses, 32,249 sections, 45,884 meeting-time rows, and 23,863 open section indexes. These numbers give the upper bound for caching/storage sizing per refresh cycle.

### FR-01 / FR-02 coverage snapshot

| Field group | API source | Status | Notes |
| --- | --- | --- | --- |
| Course title & code | `course.title`, `course.courseString` | Direct (100%) | Both fields exist for every record; either can drive the “01:198:111 - Computer Science” display. |
| Credits | `course.credits` (89%), `course.creditsObject` (100%) | Direct | `creditsObject.description` is reliable when `credits` is missing for arranged/variable-credit courses. |
| School/department | `school.code/description` | Direct | Always populated; `offeringUnitTitle` is null, so full department names must come from `school` or subject dictionaries. |
| Core codes | `course.coreCodes[]` | Direct but sparse (15.8%) | Only courses with approved core attributes populate this array; filtering for “CCO/QQ” remains possible. |
| Prerequisites | `course.preReqNotes` | Direct (28.1%) | Present where SOC lists prereqs; HTML tags need sanitizing before UI display. |
| Synopsis / description | `synopsisUrl`, `courseDescription`, `courseNotes` | Direct / derived | `synopsisUrl` exists for 56.6% of courses; when missing we fall back to `courseDescription` or curated docs. |
| Index & section number | `section.index`, `section.number` | Direct (100%) | Guaranteed for every section; suitable for FR-01 listings and subscription flows. |
| Section status | `section.openStatus`, `section.openStatusText` | Direct (100%) | Text matches SOC labels (“Open”, “Closed”, “Wait List”). |
| Instructor name | `section.instructorsText`, `section.instructors[].name` | Direct (92.1%) | 7.9% of sections omit names (staff TBD); fall back to `instructors[].name` array when present. |
| Meeting time / location | `section.meeting.*` | Direct (100%) | Meeting rows contain weekdays, start/end military time, campus name, building, room, and instruction mode (LEC/ONLINE/HYBRID). |
| Campus tags | `course.campusLocations[]`, `section.sectionCampusLocations[]` | Direct (100%) | Useful for filtering FR-02 by Busch/Livingston/Newark, etc. |
| Cross-listed, comments, exam code | `section.crossListedSections`, `commentsText`, `examCode*` | Direct | Cross-listed data appears on 6.9% of sections; exam codes cover 100%. |
| Open-section counts | `course.openSections` | Direct | Provides number of currently open sections per course; still need section-level status for alerts. |
| Capacity / waitlist size | _Not in API_ | Missing | Neither `courses` nor `openSections` exposes seat capacity, waitlist count, or enrollment numbers; notifications must rely on state transitions only. |

See `docs/soc_field_matrix.csv` for the full list (78 rows) with percentage values per field.

### OpenSections endpoint detail
- Payload is a flat list of Index strings (`openSections.json`), with no capacity, campus, or note metadata. Each campus-term pair reported 1.4k–8.6k indexes (table above).
- Because only indexes are returned, the notification service must join against cached `courses.json` to determine subject/title/time before messaging.
- Capacity and note fields are absent; acceptable workaround is to treat any index appearing in openSections as “has ≥1 open seat” and display the latest section-level comments from `courses.json`.

### Subject-level sampling (per 42 combos)
- Spring 2024 NB `640` produced the largest filtered slice (82 courses / 429 sections), while Fall 2024 CM `014` produced the smallest (3 courses / 5 sections). CS (198) remains dense at NB but sparser in NK/CM.
- Statistics for every combo are printed at the end of the runner (e.g., `12024 NB subj=198 lvl=U → 66 courses / 627 sections`) for traceability and to justify cache sizing per department-driven user queries.
