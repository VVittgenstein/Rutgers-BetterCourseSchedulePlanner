# Local Query API Contract

This document defines the HTTP contract for the lightweight API that powers the Rutgers course browser and vacancy alert UI. The goal is to expose the SQLite-backed data set through a small REST surface that is easy to mirror in other deployments while keeping the payloads predictable.

## General principles
- **Transport**: JSON over HTTPS/HTTP. UTF-8. Clients MUST set `Accept: application/json` and the server always answers JSON.
- **Versioning**: A single `v1` namespace. We express this by prefixing every route with `/api` and embedding `version` in the response body.
- **Pagination envelope**: Every list endpoint returns `{ meta, data }`. `meta` contains `page`, `pageSize`, `total`, `hasNext`, `generatedAt`, and `version`.
- **Filtering semantics**: Unless otherwise noted, filters are ANDed together. Multi-valued parameters (arrays) default to OR semantics within the same field.
- **Meeting-day semantics**: When meeting-day filters are provided, sections with meetings outside the requested day set are excluded (subset, not intersection).
- **Time units**: Meeting start/end filters use minutes since midnight (local campus time) to stay consistent with `section_meetings.start_minutes`.
- **Error shape**: Errors use `{ error: { code, message, traceId, details? } }`, the same `traceId` is echoed on the `X-Trace-Id` response header, and `details` stays an array of strings for validation errors so UI code can enumerate issues.

## Endpoints overview
| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/api/health` | Liveness checks with dependency status.
| `GET` | `/api/ready` | Strict readiness check (DB reachability + schema guardrails).
| `GET` | `/api/courses` | Primary course browser. Supports multi-dimensional filters, pagination, and summary statistics about sections.
| `GET` | `/api/sections` | Detail-level section query, frequently used by the “subscribe to open seats” panel.
| `GET` | `/api/filters` | Returns curated filter dictionaries (terms, campuses, subjects, delivery methods, core codes, etc.).

## `GET /api/health`
| Field | Type | Notes |
| --- | --- | --- |
| `status` | `"ok" \| "degraded"` | `ok` only when SQLite is reachable and required tables are present; `degraded` otherwise.
| `dependencies` | `Record<string, "up" \| "down">` | Includes `sqlite` (connection) and `schema` (ensures `courses`, `sections`, `course_core_attributes`, `course_search_fts`, `section_meetings`, `subjects` exist).
| `version` | string | Semantic app version/commit.
| `generatedAt` | ISO timestamp. |

`/api/health` is safe for k8s/docker liveness probes. A degraded response still uses HTTP `200` so orchestrators can surface the dependency failure without restarting the pod/container immediately.

## `GET /api/ready`
| Field | Type | Notes |
| --- | --- | --- |
| `status` | `"ready" \| "not_ready"` | `ready` when DB connectivity and schema checks pass; otherwise `not_ready`.
| `checks.sqlite.status` | `"up" \| "down"` | `down` when SQLite cannot be opened or `PRAGMA`/`SELECT 1` fails. `checks.sqlite.message` includes the raw error text to help operators debug file permissions and locks.
| `checks.tables.status` | `"up" \| "down"` | `down` when any required table is missing. `checks.tables.missing` lists the missing tables.
| `version` | string | Semantic app version/commit.
| `generatedAt` | ISO timestamp. |

`/api/ready` returns HTTP `200` for `"ready"` and `503` when not ready. Point readiness probes or load balancer health checks here so broken replicas stop receiving traffic.

## `GET /api/courses`
The central list endpoint that powers the search page.

### Query parameters
| Parameter | Type | Example | Behavior |
| --- | --- | --- | --- |
| `term` (required) | `string` | `20241` | Matches `courses.term_id`. |
| `campus` | `string` | `NB` | Matches `courses.campus_code`. Multiple values allowed via comma or repeated params. |
| `campusLocation` | `string` | `1` | Filters by campus location tags (e.g., College Avenue `1`, Busch `2`, Livingston `3`). Multiple values allowed. |
| `subject` | `string` | `01:198` | Restricts to one or many subject codes. Accepts `school:subject` as `01:198` and normalizes to the subject code. |
| `q` | `string` | `"data structures"` | Full-text search against `title`, `expanded_title`, `prereq_plain`. Requires 2+ trimmed characters. |
| `level` | `string` | `UG` | Accepts `UG`, `GR`, `N/A`. Multiple values OR-ed. |
| `coreCode` | `string` | `WC` | OR filter on `course_core_attributes.core_code` (case-insensitive). |
| `examCode` | `string` | `B` | Filters sections by `sections.exam_code`. Accepts SOC exam codes `A,B,C,D,F,G,I,J,M,O,Q,S,T,U`. |
| `creditsMin` | integer | `3` | Numeric comparison on `credits_min`. |
| `creditsMax` | integer | `4` | Numeric comparison on `credits_max`. |
| `delivery` | enum | `online` | Derived from `sections.delivery_method`. Accepts `in_person`, `online`, `hybrid`. Multiple values allowed. |
| `hasOpenSection` | boolean | `true` | True if any section for the course currently has `is_open = 1`. |
| `hasPrerequisite` | boolean | `false` | When `true`, keep courses with prereq text; when `false`, keep courses without prereq text. |
| `meetingDays` | string | `MWF` | Subset filter on `section_meetings.week_mask`; sections with meetings on other days are excluded. Accepts concatenated or comma-separated `M,T,W,TH,F,SA,SU`. |
| `meetingStart` | integer | `600` | Minutes after midnight. Sections qualify when at least one meeting starts at/after this value. |
| `meetingEnd` | integer | `900` | Minutes after midnight. Sections qualify when at least one meeting ends at/before this value. |
| `meetingCampus` | string | `LIV` | Optional filter against meeting campus abbreviations or location codes. |
| `sortBy` | enum | `sectionsOpen` | Allowed: `subject`, `courseNumber`, `title`, `credits`, `sectionsOpen`, `updatedAt`. |
| `sortDir` | enum | `desc` | `asc`/`desc`. Defaults depend on field (see below). |
| `page` | integer | `1` | 1-indexed page number. Default `1`. |
| `pageSize` | integer | `25` | Max `100`. Default `20`. |
| `include` | string | `sectionsSummary` | Comma list of optional expansions: `sectionsSummary`, `subjects`, `sections`. |
| `sectionsLimit` | integer | `200` | Caps preview rows per course when `include=sections` (1–300). |

Validation rules:
- `term` and at least one of `campus` or `subject` must be present to cap result sets.
- `creditsMin` must not exceed `creditsMax` when both are provided.
- `meetingStart`/`meetingEnd` must be between `0` and `1440` and `meetingStart <= meetingEnd`.
- Meeting-day filters use subset logic: any section with meetings outside the provided day set is excluded.
- `pageSize` is capped at `100` for server safety.
- Default ordering uses `(subject asc, courseNumber asc, title asc)`. Aggregation-centric sorts such as `sectionsOpen` or `updatedAt` default to `desc` when explicitly requested.

### Response shape
```json5
{
  "meta": {
    "page": 1,
    "pageSize": 25,
    "total": 158,
    "hasNext": true,
    "generatedAt": "2025-11-13T12:00:00Z",
    "version": "v1"
  },
  "data": [
    {
      "courseId": 12345,
      "termId": "20241",
      "campusCode": "NB",
      "subjectCode": "198",
      "courseNumber": "111",
      "courseString": "01:198:111",
      "title": "INTRODUCTION TO COMPUTER SCIENCE",
      "expandedTitle": null,
      "level": "UG",
      "creditsMin": 4,
      "creditsMax": 4,
      "creditsDisplay": "4",
      "coreAttributes": ["QQ", "WC"],
      "hasOpenSections": true,
      "sectionsOpen": 3,
      "prerequisites": "None",
      "updatedAt": "2025-11-11T21:33:02Z",
      "subject": {
        "code": "198",
        "description": "COMPUTER SCIENCE",
        "schoolCode": "01",
        "schoolDescription": "SAS"
      },
      "sectionsSummary": {
        "total": 8,
        "open": 3,
        "deliveryMethods": ["in_person", "online"]
      },
      "sections": [
        {
          "sectionId": 56789,
          "indexNumber": "12345",
          "sectionNumber": "01",
          "openStatus": "OPEN",
          "isOpen": true,
          "deliveryMethod": "in_person",
          "campusCode": "NB",
          "meetingCampus": "LIV",
          "instructorsText": "DOE",
          "meetingModeSummary": "LEC",
          "meetings": [
            {
              "meetingDay": "M",
              "startMinutes": 600,
              "endMinutes": 690,
              "campus": "LIV",
              "building": "HLL",
              "room": "005"
            }
          ]
        }
      ]
    }
  ]
}
```

## `GET /api/sections`
Provides detailed rows for subscription + advanced filtering experiences.

### Query parameters
| Parameter | Type | Example | Notes |
| --- | --- | --- | --- |
| `term` (required) | string | `20241` | Must match `sections.term_id`.
| `campus` | string | `NB` | Optional but recommended.
| `subject` | string | `198` | Optional. Supports multiples.
| `courseId` | integer | `12345` | Either `courseId` or `courseString` narrows search.
| `courseString` | string | `01:198:111` | Convenience for UI.
| `index` | string | `12345` | Exact match and terminates results quickly.
| `sectionNumber` | string | `02` | Works in combination with course filters.
| `openStatus` | enum | `OPEN` | Accepts `OPEN`, `CLOSED`, `WAITLIST`.
| `isOpen` | boolean | `true` | Derived from `openStatus` when provided.
| `delivery` | enum | `online` | Derived from meeting modes.
| `meetingDay` | enum[] | `M,TH` | Accepts comma list. Matches any of the meetings.
| `meetingStart` | integer | `540` | Minutes. Combined with `meetingEnd`.
| `meetingEnd` | integer | `750` | Minutes.
| `meetingCampus` | string | `LIV` | Checks `section_meetings.campus_abbrev` or location code.
| `instructor` | string | `DOE` | Substring on instructor join.
| `majors` | string[] | `014=198` | Uses `section_populations` records.
| `permissionOnly` | boolean | `false` | Whether section has `special_permission_add_code` or drop.
| `hasWaitlist` | boolean | `true` | Derived from `open_status` text or comments.
| `updatedSince` | ISO string | `2024-12-01T00:00:00Z` | Compare with `sections.updated_at`.
| `sortBy` | enum | `index` | Allowed: `index`, `openStatusUpdatedAt`, `meetingStart`, `meetingEnd`, `instructor`, `campus`.
| `sortDir` | enum | `asc`/`desc` |
| `page` | integer | default 1 |
| `pageSize` | integer | default 50; max 200 |

### Response shape
`data` is an array of `SectionDetail` objects:

| Field | Type | Source |
| --- | --- | --- |
| `sectionId` | integer | `sections.section_id` |
| `courseId` | integer | FK |
| `term` | string | `term_id` |
| `campus` | string | `campus_code` |
| `subject` | object | Denormalized subject info |
| `courseString` | string | `course_string` |
| `sectionNumber` | string | `section_number` |
| `index` | string | `index_number` |
| `openStatus` | string | `open_status` |
| `isOpen` | boolean | `is_open` |
| `openStatusUpdatedAt` | string | Latest change timestamp |
| `deliveryMethod` | string | Derived summary |
| `credits` | object | from parent course |
| `meetingSummary` | array | Flattened records from `section_meetings` (
`day`, `startLabel`, `endLabel`, `location`, `mode`)
| `instructors` | array | `section_instructors` join |
| `notes` | object | `section_notes`, `eligibility_text`, `comments_json` |
| `populations` | object | majors/minors/honors arrays |
| `specialPermission` | object | `addCode`, `dropCode`, descriptions |
| `lastSyncedAt` | string | `sections.updated_at` |
| `sourceHash` | string | `source_hash` |

## `GET /api/filters`
Returns curated dictionaries so the UI can avoid hard-coding. Core codes are sourced from `course_core_attributes` (fallback set includes AHO/AHP/AHQ/AHR/CCD/CCO/HST/SCL/NS/QQ/QR/WCD/WCR/WC/W/ITR/CE/ECN/GVT/SOEHS) and exam codes mirror the SOC exam letters (`A,B,C,D,F,G,I,J,M,O,Q,S,T,U`). When a table is empty, the API omits that array and the UI falls back to the baked-in defaults.
```json5
{
  "meta": { "generatedAt": "2025-11-13T12:00:00Z", "version": "v1" },
  "data": {
    "terms": [ { "id": "20241", "display": "Spring 2024" } ],
    "campuses": [ { "code": "NB", "display": "New Brunswick" } ],
    "subjects": [ { "code": "01:198", "description": "COMPUTER SCIENCE", "campus": "NB" } ],
    "coreCodes": [ { "code": "WC", "description": "Writing and Communication" } ],
    "examCodes": [ { "code": "A", "description": "Common Exam A" } ],
    "levels": [ "UG", "GR" ],
    "deliveryMethods": [ "in_person", "online", "hybrid" ]
  }
}
```

## Errors
| HTTP | Code | Message | When |
| --- | --- | --- | --- |
| `400` | `BAD_REQUEST` | `term is required` | Missing/invalid query parameter.
| `400` | `VALIDATION_FAILED` | `meetingStart must be <= meetingEnd` | Validation combos fail.
| `404` | `NOT_FOUND` | `Course 123 not found` | For entity GETs (future detail endpoints).
| `500` | `INTERNAL_ERROR` | `Unexpected error` | Server fault.

The error payload example:
```json5
{
  "error": {
    "code": "VALIDATION_FAILED",
    "message": "meetingStart must be <= meetingEnd",
    "details": ["meetingStart=900", "meetingEnd=840"],
    "traceId": "req-123"
  }
}
```
The `X-Trace-Id` response header repeats the same identifier.

## Operations & Observability
### Environment & ports
- `APP_HOST`/`APP_PORT` control the Fastify listener (defaults `127.0.0.1:3333`).
- `SQLITE_FILE` points to the SQLite snapshot (defaults to `data/local.db`). The service refuses to start if the file is missing.
- `LOG_LEVEL` defaults to `info`. Set `warn`/`error` for quiet cron deployments or `debug` when tracing filters locally.
- `NODE_ENV` toggles general runtime behavior; keep `production` for deployments so Fastify disables extra logging noise.

### Health probes
- `/api/health` is the shallow probe (liveness). It always replies `200` but sets `status: "degraded"` and `dependencies.schema = "down"` when tables are missing.
- `/api/ready` is strict readiness. It returns `503` when SQLite cannot be opened or when any of `courses`, `sections`, `course_core_attributes`, `course_search_fts`, `section_meetings`, or `subjects` is absent.
- Both responses include `version` and `generatedAt` so dashboards can ensure all instances picked up the latest deploy.

### Logging, tracing & rate limits
- Every request is wrapped by the `requestLogging` plugin (request/response duration) plus a `query.metrics` log message that records filters, pagination, and record counts for `/api/courses`/`/api/sections`.
- Error payloads always include `traceId` and mirror it via the `X-Trace-Id` header, enabling correlation with Fastify/Pino logs or upstream observability tools.
- Rate limiting is not built-in; place the API behind a reverse proxy (nginx, Envoy, API gateway) or add Fastify's `@fastify/rate-limit` plugin to keep abusive scrapes from exhausting the local DB.

## Non-goals in this iteration
- Authentication/authorization. Local deployments assume trusted usage; the design leaves room for future API keys.
- Mutation endpoints. This service is read-only; subscription creation runs in a separate worker/task.
- Real-time push. Consumers are expected to poll `/api/sections` or subscribe via dedicated notification pipeline (outside scope).
