# Local Query API Contract

This document defines the HTTP contract for the lightweight API that powers the Rutgers course browser and vacancy alert UI. The goal is to expose the SQLite-backed data set through a small REST surface that is easy to mirror in other deployments while keeping the payloads predictable.

## General principles
- **Transport**: JSON over HTTPS/HTTP. UTF-8. Clients MUST set `Accept: application/json` and the server always answers JSON.
- **Versioning**: A single `v1` namespace. We express this by prefixing every route with `/api` and embedding `version` in the response body.
- **Pagination envelope**: Every list endpoint returns `{ meta, data }`. `meta` contains `page`, `pageSize`, `total`, `hasNext`, `generatedAt`, and `version`.
- **Filtering semantics**: Unless otherwise noted, filters are ANDed together. Multi-valued parameters (arrays) default to OR semantics within the same field.
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
| `term` (required) | `string` | `20241` | Matches `courses.term_id`.
| `campus` | `string` | `NB` | Matches `courses.campus_code`. Multiple values allowed: `campus=NB&campus=NW`.
| `subject` | `string` | `198` | Restricts to one or many subject codes. Accepts `school:subject` as `01:198` and normalizes.
| `q` | `string` | `"data structures"` | Full-text search against `title`, `expanded_title`, `prereq_plain`.
| `level` | `string` | `UG` | Accepts `UG`, `GR`, `N/A`. Multiple values OR-ed.
| `courseNumber` | `string` | `111` | Exact match.
| `coreCode` | `string` | `WC` | OR filter on `course_core_attributes.core_code`.
| `creditsMin` | integer | `3` | Numeric comparison on `credits_min`.
| `creditsMax` | integer | `4` | Numeric comparison on `credits_max`.
| `delivery` | enum | `online` | Derived from `sections.delivery_method`. Accepts `in_person`, `online`, `hybrid`.
| `hasOpenSection` | boolean | `true` | True if any section for the course currently has `is_open = 1`.
| `meetingDays` | string | `MWF` | Intersects with aggregated meeting `week_mask`. Accepts comma-separated `M,T,W,TH,F,SA,SU`.
| `meetingStart` | integer | `600` | Minutes after midnight. Courses qualify when any meeting starts at/after this value.
| `meetingEnd` | integer | `900` | Minutes after midnight. Courses qualify when any meeting ends before this value.
| `instructor` | string | `HABBAL` | Case-insensitive substring match over concatenated instructors for all sections under the course.
| `requiresPermission` | boolean | `false` | Filters out courses where every section requires special permission.
| `sortBy` | enum | `subject` | Allowed: `subject`, `courseNumber`, `title`, `credits`, `sectionsOpen`, `updatedAt`.
| `sortDir` | enum | `asc` | `asc`/`desc`. Defaults depend on field (see below).
| `page` | integer | `1` | 1-indexed page number. Default `1`.
| `pageSize` | integer | `20` | Max `50`. Default `20`.
| `include` | string | `sectionsSummary` | Comma list of optional expansions. Currently supports `sectionsSummary` (joins `sections` table for aggregated info) and `subjects` (subject metadata).

Validation rules:
- `term` and at least one of `campus` or `subject` must be present to cap result sets.
- `meetingStart`/`meetingEnd` must be between `0` and `1440` and `meetingStart <= meetingEnd`.
- `pageSize` is capped at `100` for server safety.
- Default ordering uses `(subject asc, courseNumber asc, title asc)`. Aggregation-centric sorts such as `sectionsOpen` or `updatedAt` default to `desc` when explicitly requested.

### Response shape
```json5
{
  "meta": {
    "page": 1,
    "pageSize": 20,
    "total": 158,
    "hasNext": true,
    "generatedAt": "2025-11-13T12:00:00Z",
    "version": "v1"
  },
  "data": [
    {
      "courseId": 12345,
      "term": "20241",
      "campus": "NB",
      "subject": {
        "code": "198",
        "school": "01",
        "description": "COMPUTER SCIENCE"
      },
      "courseNumber": "111",
      "courseString": "01:198:111",
      "title": "INTRODUCTION TO COMPUTER SCIENCE",
      "expandedTitle": null,
      "level": "UG",
      "credits": {
        "min": 4,
        "max": 4,
        "display": "4"
      },
      "coreAttributes": ["QQ", "WC"],
      "hasCoreAttribute": true,
      "synopsisUrl": "https://synopsis.example",
      "prerequisites": {
        "html": "<p>None</p>",
        "plain": "None"
      },
      "sections": {
        "total": 8,
        "open": 3,
        "closed": 5,
        "waitlist": 0,
        "earliestStart": "08:10",
        "latestEnd": "21:55"
      },
      "instructors": ["DOE, JANE", "SMITH, JOHN"],
      "updatedAt": "2025-11-11T21:33:02Z"
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
Returns curated dictionaries so the UI can avoid hard-coding. Example payload:
```json5
{
  "meta": { "generatedAt": "2025-11-13T12:00:00Z", "version": "v1" },
  "data": {
    "terms": [ { "id": "20241", "display": "Spring 2024" } ],
    "campuses": [ { "code": "NB", "display": "New Brunswick" } ],
    "subjects": [ { "code": "01:198", "description": "COMPUTER SCIENCE", "campus": "NB" } ],
    "coreCodes": [ { "code": "WC", "description": "Writing and Communication" } ],
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
- `APP_HOST`/`APP_PORT` control the Fastify listener (defaults `0.0.0.0:3333`).
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
