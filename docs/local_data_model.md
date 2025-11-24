# Local Course Data Model

## Goals and scope
- Back the FR-01/FR-02 requirement set with a SQLite schema that stores every Rutgers SOC course/section/meeting field needed for browsing, filtering, and subscriptions.  
- Preserve enough provenance to re-run migrations idempotently, rehydrate JSON payloads for debugging, and power notification jobs that only react to delta updates.  
- Optimize for read-heavy local queries (term+campus/subject filtering, keyword search, meeting-time slicing) while allowing batch upserts from SOC payloads.
- The canonical SQL definition that reflects this model is checked into `data/schema.sql` and is applied through `scripts/migrate_db.ts`.

## Entity relationship overview
```mermaid
erDiagram
    TERMS ||--o{ COURSES : "1..* per term"
    CAMPUSES ||--o{ COURSES
    SUBJECTS ||--o{ COURSES
    COURSES ||--o{ COURSE_CORE_ATTRIBUTES : "core codes"
    COURSES ||--o{ SECTIONS
    SECTIONS ||--o{ SECTION_MEETINGS
    INSTRUCTORS ||--o{ SECTION_INSTRUCTORS
    SECTIONS ||--o{ SECTION_INSTRUCTORS
    SECTIONS ||--o{ SECTION_POPULATIONS : "majors/minors/honors"
    SECTIONS ||--o{ SECTION_CROSSLISTINGS
    SECTIONS ||--o{ SECTION_STATUS_EVENTS
    SECTIONS ||--o{ SUBSCRIPTIONS
    SUBSCRIPTIONS ||--o{ SUBSCRIPTION_EVENTS
```

## Table specifications
The tables below describe primary/foreign keys, SOC field mappings, and derived constraints.

### Reference tables
#### `terms`
Represents SOC `year` + `term` combinations (e.g. `2024` + `1` → `2024SP`).

| Column | Type | Source / Transform | Notes |
| --- | --- | --- | --- |
| `term_id` (PK) | TEXT | `${year}${term}` normalized during ingestion | e.g. `20241` or slug `2024SP`.
| `year` | INTEGER | API query parameter | Nullable for historical imports.
| `term_code` | TEXT | API query parameter | `0=Winter`, `1=Spring`, `7=Fall`, `9=Summer`.
| `display_name` | TEXT | Derived from config | Human-facing label.
| `start_on`, `end_on` | TEXT (DATE) | Manual config | Enables calendar filters.
| `snapshot_label` | TEXT | Derived | Helps tie to SOC pulls.

#### `campuses`
| Column | Type | Source / Transform | Notes |
| --- | --- | --- | --- |
| `campus_code` (PK) | TEXT | Request parameter | Values such as `NB`, `NK`, `CM`.
| `display_name` | TEXT | `course.campusLocations[].description` (majority) | Denormalized for UI.
| `location_code` | TEXT | `course.campusLocation.code` when present | Normalizes `1/3/O` codes.
| `region` | TEXT | Derived | Used for grouping (New Brunswick, Newark, Camden).

#### `subjects`
| Column | Type | Source / Transform | Notes |
| --- | --- | --- | --- |
| `subject_code` | TEXT | `course.subject` | Combined with `school.code` for uniqueness.
| `school_code` | TEXT | `course.school.code` | Links to Rutgers school.
| `school_description` | TEXT | `course.school.description` | Display label.
| `subject_description` | TEXT | `course.subjectDescription` fallback to curated dictionary | Covers FR-02 subject filter labels.
| `campus_code` | TEXT | Derived by frequency | Helps hide unavailable subjects per campus.
| `active` | INTEGER | Derived | Indicates presence in latest payload.

#### `instructors`
| Column | Type | Source / Transform | Notes |
| --- | --- | --- | --- |
| `instructor_id` (PK) | TEXT | SHA1 of upper-case name | Deterministic ID since SOC lacks numeric IDs.
| `full_name` | TEXT | `section.instructor.name` | Exactly as provided (e.g. `HABBAL, MANAR`).
| `normalized_name` | TEXT | Derived, uppercase last-first | Enables case-insensitive search.
| `source_payload` | TEXT | JSON blob of instructor entry | Future-proof for additional fields.

### Course and section tables
#### `courses`
Unique per `term+campus+subject+course_number`.

| Column | Type | Source / Transform | Notes |
| --- | --- | --- | --- |
| `course_id` (PK) | INTEGER | surrogate | Used by FKs.
| `term_id` (FK) | TEXT | Request context | References `terms.term_id`.
| `campus_code` (FK) | TEXT | Request context | References `campuses.campus_code`.
| `subject_code` (FK) | TEXT | `course.subject` | References `subjects.subject_code`.
| `course_number` | TEXT | `course.courseNumber` | Preserves leading zeroes.
| `course_string` | TEXT | `course.courseString` | Already `01:198:111` format.
| `title` | TEXT | `course.title` | Primary display name.
| `expanded_title` | TEXT | `course.expandedTitle` | Nullable.
| `level` | TEXT | `course.level` | `U/G` etc.
| `credits_min` | REAL | Parse `course.credits` or `creditsObject.description` | Handles variable credits.
| `credits_max` | REAL | Derived from `creditsObject.description` | e.g. `1-3` → `3`.
| `credits_display` | TEXT | `creditsObject.description` | Display-friendly string.
| `core_json` | TEXT | JSON dump of `course.coreCodes[]` | Duplicated into `course_core_attributes`.
| `has_core_attribute` | INTEGER | Derived from `coreCodes` length | Boolean for FR-02 filter.
| `prereq_html` | TEXT | `course.preReqNotes` | Raw HTML.
| `prereq_plain` | TEXT | Strip tags from `preReqNotes` | Text search and badges.
| `synopsis_url` | TEXT | `course.synopsisUrl` or curated fallback | Used for "syllabus" link.
| `course_notes` | TEXT | `course.courseNotes` | Optional.
| `unit_notes` | TEXT | `course.unitNotes` | Optional.
| `subject_notes` | TEXT | `course.subjectNotes` | Optional.
| `supplement_code` | TEXT | `course.supplementCode` | e.g. `LB`.
| `campus_locations_json` | TEXT | `course.campusLocations` | Allows multi-campus filters.
| `open_sections_count` | INTEGER | `course.openSections` | Input to progress bars.
| `has_open_sections` | INTEGER | Derived (`openSections > 0`) | Quick flag for FR-02.
| `tags` | TEXT | Derived (json list) | e.g. `{"honors":true}`.
| `search_vector` | TEXT | Derived (space-separated tokens) | Source for FTS shadow table.
| `source_hash` | TEXT | SHA1 of normalized JSON | Supports idempotent upserts.
| `source_payload` | TEXT | Raw `course` JSON | Troubleshooting/backfill.
| `created_at`, `updated_at` | TEXT (ISO8601) | ingestion time | For observability.

**Constraints**
- Unique index on `(term_id, campus_code, subject_code, course_number)`.
- `subject_code` must exist in `subjects`.

#### `course_campus_locations`
Denormalized tags to support filtering by College Ave/Busch/Livingston等地点。

| Column | Type | Source / Transform | Notes |
| --- | --- | --- | --- |
| `course_id` (FK) | INTEGER | references `courses.course_id` | Cascade delete with course. |
| `term_id` | TEXT | Derived from parent course | Denormalized for querying. |
| `campus_code` | TEXT | Derived from parent course | Denormalized for querying. |
| `location_code` | TEXT | `course.campusLocations[].code` uppercased | Primary filter value (`1/2/3/4/5/Z/S/O/NA…`). |
| `location_desc` | TEXT | `course.campusLocations[].description` | Display label fallback. |
| **PK** | (`course_id`, `location_code`) |  | Prevent duplicates. |
| Index | (`term_id`, `campus_code`, `location_code`) |  | Speeds `/api/courses?campusLocation=` |

#### `course_core_attributes`
Stores each element of `course.coreCodes` for filtering (e.g. `WCr`, `CCO`).

| Column | Type | Source / Transform | Notes |
| --- | --- | --- | --- |
| `course_id` (FK) | INTEGER | references `courses.course_id` | Cascade delete.
| `core_code` | TEXT | `coreCodes[].coreCode` | Filter value.
| `reference_id` | TEXT | `coreCodes[].coreCodeReferenceId` | Optional.
| `effective_term` | TEXT | `coreCodes[].effective` | Useful for auditing.
| `metadata` | TEXT | JSON of the entire element | Keep tags for later.
| **PK** | (`course_id`, `core_code`) |  | Prevent duplicates.

#### `sections`
One row per SOC section/index.

| Column | Type | Source / Transform | Notes |
| --- | --- | --- | --- |
| `section_id` (PK) | INTEGER | surrogate |  |
| `course_id` (FK) | INTEGER | references `courses` |  |
| `term_id` | TEXT | Derived from parent course | Denormalized for faster filtering.
| `campus_code` | TEXT | Derived | Denormalized.
| `subject_code` | TEXT | Derived | Denormalized.
| `section_number` | TEXT | `section.number` |  |
| `index_number` | TEXT UNIQUE | `section.index` | Drives FR-04 subscription lookup.
| `open_status` | TEXT | `section.openStatusText` | `OPEN/CLOSED/WAIT LIST`.
| `is_open` | INTEGER | `section.openStatus == True` | 0/1 for quick checks.
| `open_status_updated_at` | TEXT | Derived when change detected | Helps notifications.
| `instructors_text` | TEXT | `section.instructorsText` | Display fallback.
| `section_notes` | TEXT | `section.sectionNotes` | Multi-line allowed.
| `comments_json` | TEXT | `section.comments` array | Additional instructions.
| `eligibility_text` | TEXT | `section.sectionEligibility` | FR-02 filter.
| `open_to_text` | TEXT | `section.openToText` |  |
| `majors_json` | TEXT | `section.majors` | Normalized into `section_populations`.
| `minors_json` | TEXT | `section.minors` |  |
| `honor_programs_json` | TEXT | `section.honorPrograms` |  |
| `section_course_type` | TEXT | `section.sectionCourseType` | e.g. `H`.
| `exam_code` | TEXT | `section.examCode` |  |
| `exam_code_text` | TEXT | `section.examCodeText` |  |
| `special_permission_add_code` | TEXT | `section.specialPermissionAddCode` |  |
| `special_permission_add_desc` | TEXT | `section.specialPermissionAddCodeDescription` |  |
| `special_permission_drop_code` | TEXT | `section.specialPermissionDropCode` |  |
| `special_permission_drop_desc` | TEXT | `section.specialPermissionDropCodeDescription` |  |
| `printed` | TEXT | `section.printed` | `Y/N`.
| `session_print_indicator` | TEXT | `section.sessionDatePrintIndicator` |  |
| `subtitle` | TEXT | `section.subtitle` | Optional.
| `meeting_mode_summary` | TEXT | Derived from `meetingTimes` | Quick filter text.
| `delivery_method` | TEXT | Derived from `meetingModeDesc` values | Normalized `in_person/online/hybrid`.
| `has_meetings` | INTEGER | Derived from `meetingTimes` count | Flag for asynchronous courses.
| `source_hash` | TEXT | SHA1 normalized JSON | Upsert detection.
| `source_payload` | TEXT | Raw `section` JSON |  |
| `created_at`, `updated_at` | TEXT | ingestion timestamps |  |

Constraints: 
- Foreign keys to `courses` with cascade delete.
- Unique constraint on `(term_id, index_number)` to guard duplicates.

#### `section_instructors`
Join table between sections and instructors.

| Column | Type | Source / Transform | Notes |
| --- | --- | --- | --- |
| `section_id` | INTEGER | references `sections.section_id` |  |
| `instructor_id` | TEXT | references `instructors.instructor_id` |  |
| `display_order` | INTEGER | array index from SOC | Maintains ordering.
| `role` | TEXT | Derived from comments when available | `primary`, `secondary`, etc.
| **PK** | (`section_id`, `instructor_id`) |  | Deduplicates repeated names.

#### `section_meetings`
One row per meeting block inside `section.meetingTimes`.

| Column | Type | Source / Transform | Notes |
| --- | --- | --- | --- |
| `meeting_id` (PK) | INTEGER | surrogate |  |
| `section_id` (FK) | INTEGER | references `sections` |  |
| `meeting_day` | TEXT | `section.meeting.meetingDay` | `M/T/W/TH/F/SA/SU`.
| `week_mask` | INTEGER | Derived bitmask (Mon=1) | Enables multi-day filter.
| `start_time_label` | TEXT | `section.meeting.startTime` | HHMM.
| `end_time_label` | TEXT | `section.meeting.endTime` | HHMM.
| `start_minutes` | INTEGER | Convert `startTimeMilitary` → minutes since midnight | Allows numeric range filter.
| `end_minutes` | INTEGER | Convert `endTimeMilitary` |  |
| `meeting_mode_code` | TEXT | `section.meeting.meetingModeCode` |  |
| `meeting_mode_desc` | TEXT | `section.meeting.meetingModeDesc` | LEC/ONLINE/etc.
| `campus_abbrev` | TEXT | `section.meeting.campusAbbrev` | e.g. `LIV`.
| `campus_location_code` | TEXT | `section.meeting.campusLocation` |  |
| `campus_location_desc` | TEXT | `section.meeting.campusName` | Display label.
| `building_code` | TEXT | `section.meeting.buildingCode` |  |
| `room_number` | TEXT | `section.meeting.roomNumber` |  |
| `pm_code` | TEXT | `section.meeting.pmCode` | For 24h conversion double-check.
| `ba_class_hours` | TEXT | `section.meeting.baClassHours` | Rare but stored.
| `online_only` | INTEGER | Derived from `meetingModeCode in (90,HYB) and missing location` | Quick filter.
| `hash` | TEXT | SHA1 of normalized meeting entry | Identifies changes.

Indexes: `(meeting_day, start_minutes)` to accelerate FR-02 weekday/time filters.

#### `section_populations`
Normalizes `section.majors`, `minors`, `unitMajors`, `honorPrograms`.

| Column | Type | Source / Transform | Notes |
| --- | --- | --- | --- |
| `population_id` (PK) | INTEGER | surrogate |  |
| `section_id` | INTEGER | references `sections` |  |
| `population_type` | TEXT | Derived (`major`, `minor`, `unit_major`, `honor_program`) |  |
| `code` | TEXT | Raw code | e.g. `198`.
| `is_unit_code` | INTEGER | `section.unitMajors[].isUnitCode` |  |
| `is_major_code` | INTEGER | `section.majors[].isMajorCode` |  |
| `raw_payload` | TEXT | JSON entry |  |

#### `section_crosslistings`
Captures entries from `section.crossListedSections` if present.

| Column | Type | Source / Transform | Notes |
| --- | --- | --- | --- |
| `section_id` | INTEGER | references `sections` | Base section.
| `related_index` | TEXT | `crossListedSections[].index` | Index of sibling section.
| `related_subject_code` | TEXT | `crossListedSections[].subject` |  |
| `related_course_number` | TEXT | `crossListedSections[].courseNumber` |  |
| `raw_payload` | TEXT | JSON element |  |
| **PK** | (`section_id`, `related_index`) |  |

#### `section_status_events`
Tracks every detected status change for notifications.

| Column | Type | Source / Transform | Notes |
| --- | --- | --- | --- |
| `event_id` (PK) | INTEGER | surrogate |  |
| `section_id` | INTEGER | references `sections` |  |
| `previous_status` | TEXT | Derived | From prior snapshot.
| `current_status` | TEXT | Derived | From latest section row.
| `source` | TEXT | `courses.json` vs `openSections.json` | Explains trigger.
| `snapshot_term` | TEXT | Derived | Quick filtering.
| `snapshot_campus` | TEXT | Derived |  |
| `snapshot_received_at` | TEXT | ingestion timestamp |  |

### Subscription tables
#### `subscriptions`
Stores FR-04 subscription requests.

| Column | Type | Source / Transform | Notes |
| --- | --- | --- | --- |
| `subscription_id` (PK) | INTEGER | surrogate | |
| `section_id` (FK) | INTEGER | references `sections.section_id` | |
| `term_id`, `campus_code`, `index_number` | TEXT | Denormalized from section | Ensure lookups resilient even if FK missing temporarily.
| `contact_type` | TEXT | User input | Always `email` in local mode.
| `contact_value` | TEXT | User input | Raw string.
| `contact_hash` | TEXT | SHA1(contact_value) | Dedup + rate limit.
| `locale` | TEXT | User input/default | e.g. `en-US`, `zh-CN`.
| `status` | TEXT | Derived | `active`, `unsubscribed` (others reserved).
| `is_verified` | INTEGER | Derived from verification flow | Always 1 on insert in local mode.
| `created_at`, `updated_at` | TEXT | System timestamps | |
| `last_notified_at` | TEXT | Updated when alert sent | |
| `last_known_section_status` | TEXT | Derived from joined section at subscription time | Helps avoid duplicate triggers.
| `unsubscribe_token` | TEXT | Random string | Provided to UI.
| `metadata` | TEXT | JSON (channel-specific) | Stores preferences; no channel bindings.

Indexes:
- Unique partial index on `(section_id, contact_hash, contact_type)` for active rows → prevents duplicate subscriptions to the same contact.
- Index on `(status, section_id)` for fast polling.

#### `subscription_events`
Audit log for subscription lifecycle + notification sends.

| Column | Type | Source / Transform | Notes |
| --- | --- | --- | --- |
| `event_id` (PK) | INTEGER | surrogate | |
| `subscription_id` | INTEGER | references `subscriptions` | |
| `event_type` | TEXT | Derived (`created`, `unsubscribed`) | |
| `section_status_snapshot` | TEXT | Derived from joined section at time of event | |
| `payload` | TEXT | JSON (email id, error) | |
| `created_at` | TEXT | timestamp | |

### Utility tables and views
#### `open_section_snapshots`
Caches rows from `openSections.json` to cross-check with `sections.open_status`.

| Column | Type | Source / Transform | Notes |
| --- | --- | --- | --- |
| `snapshot_id` (PK) | INTEGER | surrogate | |
| `term_id` | TEXT | request context | |
| `campus_code` | TEXT | request context | |
| `index_number` | TEXT | `openSections[].index` | |
| `seen_open_at` | TEXT | Timestamp when index was last reported open | |
| `source_hash` | TEXT | Derived | Dedup per pull.

#### `course_search_fts`
SQLite FTS5 shadow table fed from `courses` + `sections` for keyword search (title, subject description, instructor names, index numbers).

Columns: `term_id`, `campus_code`, `course_id`, `section_id`, `document`. Document concatenates `course_string`, `title`, `expanded_title`, `subject_description`, `instructors_text`, `index_number`, `prereq_plain`.

## SOC → local field mapping highlights
The tables above list per-column mappings. The summary below focuses on the FR-critical filters.

| FR Filter | Local Column(s) | SOC Field(s) | Transform |
| --- | --- | --- | --- |
| Term + campus selection | `courses.term_id`, `courses.campus_code` | Query params | Stored verbatim; composite index `idx_courses_term_campus_subject` ensures O(log n) scans.
| Subject / department | `courses.subject_code`, `subjects.subject_description` | `course.subject`, `course.subjectDescription` | Text fallback to curated subject dictionary.
| Course title/code keywords | `course_search_fts.document` | `course.title`, `course.courseString`, `section.index` | Use FTS5 for prefix queries.
| Credits range | `courses.credits_min`, `courses.credits_max` | `course.credits`, `creditsObject.description` | Parse floats; if null, treat as variable credit and tag `credits_display`.
| Core requirement filter | `course_core_attributes.core_code` | `course.coreCodes[].coreCode` | Many-to-one table w/ index on `(core_code, term_id)`.
| Prerequisite present | `courses.has_prereq` | `course.preReqNotes` | Boolean derived from non-empty text.
| Instructor filter | `section_instructors.instructor_id` | `section.instructors[].name` | Normalized IDs.
| Section status (Open/Closed/Wait) | `sections.open_status`, `sections.is_open` | `section.openStatus`, `section.openStatusText` | `is_open` stored as 0/1.
| Meeting day/time filter | `section_meetings.week_mask`, `section_meetings.start_minutes`, `section_meetings.end_minutes` | `section.meeting.meetingDay`, `startTimeMilitary`, `endTimeMilitary` | Convert HHMM string to minutes.
| Campus / building filter | `section_meetings.campus_location_desc`, `building_code`, `room_number` | `section.meeting.campusName`, `buildingCode`, `roomNumber` | Null-safe comparisons.
| Instruction mode filter | `sections.delivery_method`, `section_meetings.meeting_mode_desc` | `meetingModeDesc` | Normalized to `in_person`, `hybrid`, `online`.
| Exam code filter | `sections.exam_code` | `section.examCode` | Already single char values.
| Cross-list awareness | `section_crosslistings.related_index` | `section.crossListedSections` | Track mapping for UI badges.
| Subscription join | `subscriptions.section_id`, `sections.index_number` | Local only (+ `openSections` for change detection) | Index on `sections.index_number` ensures cheap lookups.

## Index strategy and rationale
| Index | Definition | Purpose |
| --- | --- | --- |
| `idx_courses_term_subject` | `courses(term_id, campus_code, subject_code, title)` | Drives FR-01 listing per term/campus + subject filter.
| `idx_courses_search_vector` | `courses(term_id, campus_code, search_vector)` + FTS | Keyword search fallback when FTS is unavailable.
| `idx_sections_index_unique` | UNIQUE on `sections(index_number)` | Direct Section lookup (subscriptions, SOC diffing).
| `idx_sections_term_subject_status` | `sections(term_id, campus_code, subject_code, is_open)` | Enables combined subject + status filter.
| `idx_meetings_day_time` | `section_meetings(week_mask, start_minutes)` INCLUDE `section_id` | Efficient day/time range filtering for FR-02 + FR-03 (calendar view).
| `idx_core_code_course` | `course_core_attributes(core_code, term_id)` | Filter by core attribute quickly.
| `idx_instructors_name` | `section_instructors(instructor_id, section_id)` | Instructor filter.
| `idx_populations_code` | `section_populations(population_type, code)` | Support "open to majors" advanced filter.
| `idx_status_events_section` | `section_status_events(section_id, snapshot_received_at)` | Replay history for notifications.
| `idx_subscriptions_active` | `subscriptions(section_id, status)` INCLUDE `contact_type, contact_hash` | Background worker polls only relevant subscriptions.
| `idx_open_section_snapshot` | `open_section_snapshots(term_id, campus_code, index_number)` | Compare `openSections` payload vs stored section rows.

## Derived field notes
- **Credit parsing:** `creditsObject.description` strings like `3.0 Credits` or `BA` are parsed via regex; when parsing fails we set `credits_min`/`credits_max` to `NULL` and flag `tags -> variable_credit = true`.
- **Meeting minutes:** When SOC provides `startTimeMilitary`/`endTimeMilitary`, parse those 24h values directly (`int(hh) * 60 + int(mm)`) because many rows omit `pmCode`. Only fall back to the 12h clock when military fields are missing: compute `hour = int(start[:2]) % 12`, `minute = int(start[2:])`, then add `12 * 60` iff `pmCode == 'P'` so noon/midnight stay accurate.
- **Delivery method:** If all meetings have `meetingModeCode in ('02','03','04')` and campus/building present → `in_person`. If some meetings are `ONLINE` plus physical rooms → `hybrid`. Pure online rows or `meetingTimes` empty → `online`.
- **Has prerequisites/core:** boolean columns on `courses` allow quick toggles without string scanning during queries.
- **Search vector:** stored in lowercase ASCII by concatenating tokens and removing punctuation. `course_search_fts` is refreshed after each ingest transaction.
- **Status events:** ingestion diff compares `sections.source_hash`; when a change flips `is_open`, a row is inserted into `section_status_events` and `sections.open_status_updated_at` is refreshed. Subscription workers only notify if `section_status_events` shows a transition `Closed→Open` after the subscription `created_at`.

## Migration and idempotency hints
- Wrap each SOC payload ingest in a transaction. Upsert into reference tables first, then `courses`, then `sections`, followed by child tables.  
- Use `source_hash` comparisons to skip updates when payload rows match previous runs, keeping `updated_at` stable and minimizing churn for downstream caches.  
- `open_section_snapshots` should be truncated per run (per term/campus) and re-populated so the diff logic stays simple.
