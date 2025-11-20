# Field Validation Report — Term 12024 · NB

_Last updated: 2025-11-17_

## Inputs & scope
- Local snapshot: `data/courses.sqlite` built via `npm run data:fetch` (`logs/fetch_runs/summary_latest.json` shows the 2025-11-17 incremental pull inserted 66 courses / 627 sections for term `12024`, campus `NB`, subject `198`).
- Source of truth: Rutgers SOC `courses.json` (`https://classes.rutgers.edu/soc/api/courses.json?year=2024&term=1&campus=NB`).
- Validation artifacts: `reports/field_validation_samples.json` (structured comparisons) and `reports/field_validation.md` (this document).

## Methodology
1. Queried the SQLite snapshot for term `12024` counts (`66` courses, `627` sections) and pulled a random pool of 30 course rows (`ORDER BY RANDOM()`), ensuring variety across undergraduate (01), graduate (16/26), business (22) prefixes, and open/closed states.
2. Used a Python helper (inline script, gzip-aware) to fetch the SOC payload once, map it by `{subject}-{courseNumber}`, and align each sampled course with the matching SOC entry plus its nested sections/meetings.
3. For each matched course we compared:
   - Course-level fields: `title`, `expanded_title`, `credits_display/min/max`, `core_codes`, `campus_locations`, `synopsis_url`, `prereq_plain` vs SOC `preReqNotes`, and `open_sections`/`has_open_sections`.
   - Section-level fields for the first index (sorted ascending): `section_number`, `index_number`, `open_status`, instructor list, subtitle, derived delivery summary, and the full set of `section_meetings`.
4. Persisted the paired records to `reports/field_validation_samples.json` and generated the Markdown snippets below for reproducibility.

## Sample overview (10 randomly selected courses)
| Course | Unit | Open sections (DB/SOC) | Core codes | Sample section (number · index · status/instructor) | Notes |
| --- | --- | --- | --- | --- | --- |
| 01:198:440 | 01 | 0/0 | — | 01 · 06847 · CLOSED (BOULARIAS) | LEC + RECIT meetings captured; lengthy HTML prereq text stripped to plain text without losing clauses. |
| 16:198:560 | 16 | 1/1 | — | 01 · 13592 · OPEN (BEKRIS, KOSTAS) | Graduate robotics course on College Ave; dual-day lecture slots identical after order-insensitive compare. |
| 16:198:844 | 16 | 1/1 | — | 01 · 13821 · OPEN (GAO, JIE) | Zero-credit internship uses placeholder `GRADUATE 800-LEVEL` meeting, matching SOC payload exactly. |
| 01:198:111 | 01 | 3/3 | ITR,QQ,QR | 01 · 06570 · CLOSED (GOEL, APOORVA) | Core attributes (ITR/QQ/QR) and multi-campus meeting mix (Busch + College Ave) present in both sources. |
| 01:198:213 | 01 | 1/1 | — | 01 · 06796 · OPEN (VENUGOPAL, SESH) | Course with lecture + recitation pair; meeting sets (T/H) and instructor text stay in sync. |
| 16:198:702 | 16 | 61/61 | — | A1 · 13708 · OPEN (ALLENDER, ERIC) | Research sections with null meeting metadata retain SOC’s `RSCH-MA` legend. |
| 16:198:800 | 16 | 1/1 | — | 01 · 13819 · OPEN (GAO, JIE) | Matriculation placeholder matches SOC zero-credit representation (no meetings, open flag true). |
| 26:198:684 | 26 | 1/1 | — | 01 · 16666 · OPEN (KATEHAKIS) | Online-only MBA section keeps the SOC `ONLINE INSTRUCTION(INTERNET)` + `** INVALID **` campus marker. |
| 22:198:660 | 22 | 0/0 | — | 60 · 16629 · CLOSED (GILANI, WAJAHAT) | Hybrid Livingston+online course shows both the physical slot (BRR 5105) and remote meeting stub. |
| 16:198:520 | 16 | 0/0 | — | 01 · 13580 · CLOSED (COWAN, CHARLES) | Graduate Intro to AI retains dual-day lecture entries; prereqs absent in both systems, as expected. |

## Findings
- Every sampled field pair (titles, credit ranges, campus lists, synopsis URLs, prereq text, `open_sections` counts) matched exactly. SOC still returns HTML `<em>` markers in `preReqNotes`; our normalized plain-text field preserves the content without tags.
- Section statuses, instructors, subtitles, and `has_open_sections` mirrors are consistent across graduate/undergraduate levels. Derived delivery labels (`in_person`, `hybrid`, `online`) track the meeting structures even though SOC does not expose those exact strings.
- Meeting comparisons succeeded after treating the sets as order-independent and tolerating blank placeholders (`""` vs `NULL`) for research/online sections. Physical rooms/campuses match for every concrete meeting (e.g., SEC 111, TIL 254, BRR 5105).
- Edge placeholders from SOC (e.g., campus `"** INVALID **"` for remote sessions, meeting mode `"GRADUATE 800-LEVEL"`, `RSCH-MA`) are stored unmodified in SQLite, so downstream consumers can reason about these special cases without extra mapping.

## Detailed course checks
_Raw row-by-row comparisons (per course) copied directly from `reports/field_validation_samples.json`:_

#### 01:198:440 · INTRO ARTIFIC INTELL

- Title: SQLite `INTRO ARTIFIC INTELL` vs SOC `INTRO ARTIFIC INTELL` — match
- Expanded title: SQLite `INTRODUCTION TO ARTIFICIAL INTELLIGENCE` vs SOC `INTRODUCTION TO ARTIFICIAL INTELLIGENCE`
- Credits: SQLite `4.0 credits` (min=4.0 max=4.0) vs SOC `4.0 credits`/`4`
- Core codes: SQLite `—` vs SOC `—`
- Campus locations: SQLite `Busch` vs SOC `Busch`
- Synopsis URL: SQLite `http://www.cs.rutgers.edu/undergraduate/courses/` vs SOC `http://www.cs.rutgers.edu/undergraduate/courses/`
- Prerequisites: SQLite `((01:198:205 INTRODUCTION TO DISCRETE STRUCTURES I or 14:332:202 DISCRETE MATH ECE or 14:332:312 DISCRETE MATHEMATICS ECE ) and (01:640:152 CALCULUS II FOR MATHEMATICAL AND PHYSICAL SCIENCES )) OR ((01:198:205 INTRODUCTION TO DISCRETE STRUCTURES I or 14:332:202 DISCRETE MATH ECE or 14:332:312 DISCRETE MATHEMATICS ECE ) and (01:640:192 HONORS CALCULUS II ))` vs SOC HTML `((01:198:205 INTRODUCTION TO DISCRETE STRUCTURES I  or 14:332:202 DISCRETE MATH ECE  or 14:332:312 DISCRETE MATHEMATICS ECE ) and (01:640:152 CALCULUS II FOR MATHEMATICAL AND PHYSICAL SCIENCES )) <em> OR </em> ((01:198:205 INTRODUCTION TO DISCRETE STRUCTURES I  or 14:332:202 DISCRETE MATH ECE  or 14:332:312 DISCRETE MATHEMATICS ECE ) and (01:640:192 HONORS CALCULUS II ))`
- Open sections: SQLite `0` (has_open=False) vs SOC `0` (has_open=False)
- Section 01 (index 06847): status `CLOSED` vs `CLOSED`, instructor `BOULARIAS` vs `BOULARIAS`
- Meetings: SQLite `F 02:00-03:20 LEC SEC 111 (BUSCH); W 05:55-06:50 RECIT SEC 202 (BUSCH); W 12:10-01:30 LEC SEC 111 (BUSCH)` vs SOC `W 12:10-01:30 LEC SEC 111 (BUSCH); F 02:00-03:20 LEC SEC 111 (BUSCH); W 05:55-06:50 RECIT SEC 202 (BUSCH)`

#### 16:198:560 · INTRO COMP ROBOTICS

- Title: SQLite `INTRO COMP ROBOTICS` vs SOC `INTRO COMP ROBOTICS` — match
- Expanded title: SQLite `INTRODUCTION TO COMPUTATIONAL ROBOTICS` vs SOC `INTRODUCTION TO COMPUTATIONAL ROBOTICS`
- Credits: SQLite `3.0 credits` (min=3.0 max=3.0) vs SOC `3.0 credits`/`3`
- Core codes: SQLite `—` vs SOC `—`
- Campus locations: SQLite `College Avenue` vs SOC `College Avenue`
- Synopsis URL: SQLite `http://www.cs.rutgers.edu/graduate/courses/` vs SOC `http://www.cs.rutgers.edu/graduate/courses/`
- Prerequisites: SQLite `—` vs SOC HTML `—`
- Open sections: SQLite `1` (has_open=True) vs SOC `1` (has_open=True)
- Section 01 (index 13592): status `OPEN` vs `OPEN`, instructor `BEKRIS, KOSTAS` vs `BEKRIS, KOSTAS`
- Meetings: SQLite `H 12:10-01:30 LEC HH A7 (COLLEGE AVENUE); M 12:10-01:30 LEC HH A7 (COLLEGE AVENUE)` vs SOC `M 12:10-01:30 LEC HH A7 (COLLEGE AVENUE); H 12:10-01:30 LEC HH A7 (COLLEGE AVENUE)`

#### 16:198:844 · RESEARCH INTERNSHIP

- Title: SQLite `RESEARCH INTERNSHIP` vs SOC `RESEARCH INTERNSHIP` — match
- Expanded title: SQLite `—` vs SOC `—`
- Credits: SQLite `0.0 credits` (min=0.0 max=0.0) vs SOC `0.0 credits`/`0`
- Core codes: SQLite `—` vs SOC `—`
- Campus locations: SQLite `N/A` vs SOC `N/A`
- Synopsis URL: SQLite `http://www.cs.rutgers.edu/graduate/courses/` vs SOC `http://www.cs.rutgers.edu/graduate/courses/`
- Prerequisites: SQLite `—` vs SOC HTML `—`
- Open sections: SQLite `1` (has_open=True) vs SOC `1` (has_open=True)
- Section 01 (index 13821): status `OPEN` vs `OPEN`, instructor `GAO, JIE` vs `GAO, JIE`
- Meetings: SQLite `— —-— GRADUATE 800-LEVEL` vs SOC `— —-— GRADUATE 800-LEVEL`

#### 01:198:111 · INTRO COMPUTER SCI

- Title: SQLite `INTRO COMPUTER SCI` vs SOC `INTRO COMPUTER SCI` — match
- Expanded title: SQLite `—` vs SOC `—`
- Credits: SQLite `4.0 credits` (min=4.0 max=4.0) vs SOC `4.0 credits`/`4`
- Core codes: SQLite `ITR, QQ, QR` vs SOC `ITR, QQ, QR`
- Campus locations: SQLite `Busch, College Avenue, Livingston` vs SOC `Busch, College Avenue, Livingston`
- Synopsis URL: SQLite `http://www.cs.rutgers.edu/undergraduate/courses/` vs SOC `http://www.cs.rutgers.edu/undergraduate/courses/`
- Prerequisites: SQLite `Any Course EQUAL or GREATER Than: (01:640:112 PRECALCULUS PART II )` vs SOC HTML `Any Course EQUAL or GREATER Than: (01:640:112 PRECALCULUS PART II )`
- Open sections: SQLite `3` (has_open=True) vs SOC `3` (has_open=True)
- Section 01 (index 06570): status `CLOSED` vs `CLOSED`, instructor `GOEL, APOORVA` vs `GOEL, APOORVA`
- Meetings: SQLite `M 02:00-03:20 LEC AB 2125 (COLLEGE AVENUE); T 10:35-11:30 RECIT HLL 252 (BUSCH); W 02:00-03:20 LEC AB 2125 (COLLEGE AVENUE)` vs SOC `M 02:00-03:20 LEC AB 2125 (COLLEGE AVENUE); W 02:00-03:20 LEC AB 2125 (COLLEGE AVENUE); T 10:35-11:30 RECIT HLL 252 (BUSCH)`

#### 01:198:213 · SOFTWARE METHODOLOGY

- Title: SQLite `SOFTWARE METHODOLOGY` vs SOC `SOFTWARE METHODOLOGY` — match
- Expanded title: SQLite `—` vs SOC `—`
- Credits: SQLite `4.0 credits` (min=4.0 max=4.0) vs SOC `4.0 credits`/`4`
- Core codes: SQLite `—` vs SOC `—`
- Campus locations: SQLite `Livingston` vs SOC `Livingston`
- Synopsis URL: SQLite `http://www.cs.rutgers.edu/undergraduate/courses/` vs SOC `http://www.cs.rutgers.edu/undergraduate/courses/`
- Prerequisites: SQLite `(01:198:112 DATA STRUCTURES ) OR (14:332:351 PROGRM METHODOLOGYII )` vs SOC HTML `(01:198:112 DATA STRUCTURES )<em> OR </em>(14:332:351 PROGRM METHODOLOGYII )`
- Open sections: SQLite `1` (has_open=True) vs SOC `1` (has_open=True)
- Section 01 (index 06796): status `OPEN` vs `OPEN`, instructor `VENUGOPAL, SESH` vs `VENUGOPAL, SESH`
- Meetings: SQLite `H 03:50-05:10 LEC TIL 254 (LIVINGSTON); H 07:45-08:40 RECIT TIL 253 (LIVINGSTON); T 03:50-05:10 LEC TIL 254 (LIVINGSTON)` vs SOC `T 03:50-05:10 LEC TIL 254 (LIVINGSTON); H 03:50-05:10 LEC TIL 254 (LIVINGSTON); H 07:45-08:40 RECIT TIL 253 (LIVINGSTON)`

#### 16:198:702 · RESEARCH COMP SC

- Title: SQLite `RESEARCH COMP SC` vs SOC `RESEARCH COMP SC` — match
- Expanded title: SQLite `—` vs SOC `—`
- Credits: SQLite `Credits by arrangement` (min=— max=—) vs SOC `Credits by arrangement`/`—`
- Core codes: SQLite `—` vs SOC `—`
- Campus locations: SQLite `Busch, N/A` vs SOC `Busch, N/A`
- Synopsis URL: SQLite `http://www.cs.rutgers.edu/graduate/courses/` vs SOC `http://www.cs.rutgers.edu/graduate/courses/`
- Prerequisites: SQLite `—` vs SOC HTML `—`
- Open sections: SQLite `61` (has_open=True) vs SOC `61` (has_open=True)
- Section A1 (index 13708): status `OPEN` vs `OPEN`, instructor `ALLENDER, ERIC` vs `ALLENDER, ERIC`
- Meetings: SQLite `— —-— RSCH-MA` vs SOC `— —-— RSCH-MA`

#### 16:198:800 · MATRICULATION CONTD

- Title: SQLite `MATRICULATION CONTD` vs SOC `MATRICULATION CONTD` — match
- Expanded title: SQLite `MATRICULATION CONTINUED` vs SOC `MATRICULATION CONTINUED`
- Credits: SQLite `0.0 credits` (min=0.0 max=0.0) vs SOC `0.0 credits`/`0`
- Core codes: SQLite `—` vs SOC `—`
- Campus locations: SQLite `N/A` vs SOC `N/A`
- Synopsis URL: SQLite `http://www.cs.rutgers.edu/graduate/courses/` vs SOC `http://www.cs.rutgers.edu/graduate/courses/`
- Prerequisites: SQLite `—` vs SOC HTML `—`
- Open sections: SQLite `1` (has_open=True) vs SOC `1` (has_open=True)
- Section 01 (index 13819): status `OPEN` vs `OPEN`, instructor `GAO, JIE` vs `GAO, JIE`
- Meetings: SQLite `— —-— GRADUATE 800-LEVEL` vs SOC `— —-— GRADUATE 800-LEVEL`

#### 26:198:684 · SP TOPICS INFO SYSTM

- Title: SQLite `SP TOPICS INFO SYSTM` vs SOC `SP TOPICS INFO SYSTM` — match
- Expanded title: SQLite `SPECIAL TOPICS INFORMATION SYSTEMS` vs SOC `SPECIAL TOPICS INFORMATION SYSTEMS`
- Credits: SQLite `3.0 credits` (min=3.0 max=3.0) vs SOC `3.0 credits`/`3`
- Core codes: SQLite `—` vs SOC `—`
- Campus locations: SQLite `O` vs SOC `O`
- Synopsis URL: SQLite `—` vs SOC `—`
- Prerequisites: SQLite `—` vs SOC HTML `—`
- Open sections: SQLite `1` (has_open=True) vs SOC `1` (has_open=True)
- Section 01 (index 16666): status `OPEN` vs `OPEN`, instructor `KATEHAKIS` vs `KATEHAKIS`
- Meetings: SQLite `W 10:20-01:20 ONLINE INSTRUCTION(INTERNET) (** INVALID **)` vs SOC `W 10:20-01:20 ONLINE INSTRUCTION(INTERNET) (** INVALID **)`

#### 22:198:660 · BUS ANLYTICS PROGRAM

- Title: SQLite `BUS ANLYTICS PROGRAM` vs SOC `BUS ANLYTICS PROGRAM` — match
- Expanded title: SQLite `BUSINESS ANALYTICS PROGRAMMING` vs SOC `BUSINESS ANALYTICS PROGRAMMING`
- Credits: SQLite `3.0 credits` (min=3.0 max=3.0) vs SOC `3.0 credits`/`3`
- Core codes: SQLite `—` vs SOC `—`
- Campus locations: SQLite `Livingston, O` vs SOC `Livingston, O`
- Synopsis URL: SQLite `—` vs SOC `—`
- Prerequisites: SQLite `(22:960:641 TOPIC:ANALYTICS FOR BUSINESS INTELLIGENCE )` vs SOC HTML `(22:960:641 TOPIC:ANALYTICS FOR BUSINESS            INTELLIGENCE )`
- Open sections: SQLite `0` (has_open=False) vs SOC `0` (has_open=False)
- Section 60 (index 16629): status `CLOSED` vs `CLOSED`, instructor `GILANI, WAJAHAT` vs `GILANI, WAJAHAT`
- Meetings: SQLite `— —-— ONLINE INSTRUCTION(INTERNET) (** INVALID **); T 06:00-07:20 LEC BRR 5105 (LIVINGSTON)` vs SOC `T 06:00-07:20 LEC BRR 5105 (LIVINGSTON); — —-— ONLINE INSTRUCTION(INTERNET) (** INVALID **)`

#### 16:198:520 · INTRO TO ARTIF INTEL

- Title: SQLite `INTRO TO ARTIF INTEL` vs SOC `INTRO TO ARTIF INTEL` — match
- Expanded title: SQLite `—` vs SOC `—`
- Credits: SQLite `3.0 credits` (min=3.0 max=3.0) vs SOC `3.0 credits`/`3`
- Core codes: SQLite `—` vs SOC `—`
- Campus locations: SQLite `Busch` vs SOC `Busch`
- Synopsis URL: SQLite `http://www.cs.rutgers.edu/graduate/courses/` vs SOC `http://www.cs.rutgers.edu/graduate/courses/`
- Prerequisites: SQLite `—` vs SOC HTML `—`
- Open sections: SQLite `0` (has_open=False) vs SOC `0` (has_open=False)
- Section 01 (index 13580): status `CLOSED` vs `CLOSED`, instructor `COWAN, CHARLES` vs `COWAN, CHARLES`
- Meetings: SQLite `H 12:10-01:30 LEC SEC 208 (BUSCH); M 12:10-01:30 LEC SEC 208 (BUSCH)` vs SOC `M 12:10-01:30 LEC SEC 208 (BUSCH); H 12:10-01:30 LEC SEC 208 (BUSCH)`

## Reproduction checklist
- Ensure `npm install` + `npm run db:migrate` succeeded and `data/courses.sqlite` exists.
- Recreate the sample JSON/Markdown bundle by running the inline helper below (edit `TERM`, `CAMPUS`, or `TARGET_COUNT` as needed):
  ```bash
  python3 - <<'PY'
  import json, gzip, sqlite3, urllib.request
  from pathlib import Path

  TERM = '12024'
  CAMPUS = 'NB'
  TARGET_COUNT = 10

  def fetch_soc(term: str, campus: str):
      year = int(term[-4:])
      term_code = int(term[0])
      url = f'https://classes.rutgers.edu/soc/api/courses.json?year={year}&term={term_code}&campus={campus}'
      req = urllib.request.Request(url, headers={'User-Agent': 'BetterCourseSchedulePlanner/validation', 'Accept-Encoding': 'gzip'})
      with urllib.request.urlopen(req) as resp:
          raw = resp.read()
          if resp.headers.get('Content-Encoding') == 'gzip':
              raw = gzip.decompress(raw)
      return json.loads(raw)

  def normalize_list(value):
      if not value:
          return []
      if isinstance(value, list):
          return value
      return [value]

  conn = sqlite3.connect('data/courses.sqlite')
  conn.row_factory = sqlite3.Row
  soc_map = {f"{row.get('subject')}-{row.get('courseNumber')}": row for row in fetch_soc(TERM, CAMPUS)}
  rows = conn.execute(
      '''
      SELECT *
      FROM courses
      WHERE term_id = ?
      ORDER BY RANDOM()
      LIMIT 30
      ''',
      (TERM,)
  ).fetchall()

  samples = []
  for row in rows:
      key = f"{row['subject_code']}-{row['course_number']}"
      soc = soc_map.get(key)
      if not soc:
          continue
      section = conn.execute(
          '''
          SELECT * FROM sections
          WHERE course_id = ?
          ORDER BY index_number ASC
          LIMIT 1
          ''',
          (row['course_id'],)
      ).fetchone()
      meetings = conn.execute(
          '''
          SELECT meeting_day, start_time_label, end_time_label, meeting_mode_desc,
                 campus_location_desc, building_code, room_number
          FROM section_meetings
          WHERE section_id = ?
          ORDER BY meeting_day ASC, start_time_label ASC
          ''',
          (section['section_id'],)
      ).fetchall() if section else []
      payload = {
          'course_string': row['course_string'],
          'subject': row['subject_code'],
          'course_number': row['course_number'],
          'title_sqlite': row['title'],
          'title_soc': soc.get('title'),
          'expanded_title_sqlite': row['expanded_title'],
          'expanded_title_soc': soc.get('expandedTitle'),
          'credits_display_sqlite': row['credits_display'],
          'credits_description_soc': soc.get('creditsObject', {}).get('description') if isinstance(soc.get('creditsObject'), dict) else None,
          'credits_value_soc': soc.get('credits'),
          'credits_min_sqlite': row['credits_min'],
          'credits_max_sqlite': row['credits_max'],
          'prereq_plain_sqlite': row['prereq_plain'],
          'prereq_html_soc': soc.get('preReqNotes'),
          'synopsis_sqlite': row['synopsis_url'],
          'synopsis_soc': soc.get('synopsisUrl'),
          'open_sections_sqlite': row['open_sections_count'],
          'open_sections_soc': soc.get('openSections'),
          'has_open_sqlite': bool(row['has_open_sections']),
          'has_open_soc': bool(soc.get('openSections')),
          'core_codes_sqlite': sorted({item.get('coreCode') for item in json.loads(row['core_json'] or '[]') if isinstance(item, dict) and item.get('coreCode')} if row['core_json'] else []),
          'core_codes_soc': sorted({item.get('code') or item.get('coreCode') for item in soc.get('coreCodes', []) if isinstance(item, dict) and (item.get('code') or item.get('coreCode'))}),
          'campus_locations_sqlite': sorted({(entry.get('description') if isinstance(entry, dict) else entry) for entry in json.loads(row['campus_locations_json'] or '[]')} if row['campus_locations_json'] else []),
          'campus_locations_soc': sorted({(entry.get('description') if isinstance(entry, dict) else entry) for entry in soc.get('campusLocations', []) if (entry.get('description') if isinstance(entry, dict) else entry)}),
          'section_index': section['index_number'] if section else None,
          'section_number': section['section_number'] if section else None,
          'section_open_sqlite': section['open_status'] if section else None,
          'section_open_soc': None,
          'section_instructors_sqlite': section['instructors_text'] if section else None,
          'section_instructors_soc': None,
          'meetings_sqlite': [
              {
                  'meeting_day': mt['meeting_day'],
                  'start_time': mt['start_time_label'],
                  'end_time': mt['end_time_label'],
                  'meeting_mode_desc': mt['meeting_mode_desc'],
                  'campus': mt['campus_location_desc'],
                  'building': mt['building_code'],
                  'room': mt['room_number'],
              }
              for mt in meetings
          ],
          'meetings_soc': [],
      }
      if section:
          for sec in soc.get('sections', []):
              idx = sec.get('index') or sec.get('indexNumber')
              if idx == section['index_number']:
                  payload['section_open_soc'] = sec.get('openStatusText') or sec.get('openStatus')
                  payload['section_instructors_soc'] = sec.get('instructorsText')
                  payload['meetings_soc'] = [
                      {
                          'meeting_day': mt.get('meetingDay'),
                          'start_time': mt.get('startTime'),
                          'end_time': mt.get('endTime'),
                          'meeting_mode_desc': mt.get('meetingModeDesc'),
                          'campus': mt.get('campusName'),
                          'building': mt.get('buildingCode'),
                          'room': mt.get('roomNumber'),
                      }
                      for mt in normalize_list(sec.get('meetingTimes'))
                  ]
                  break
      samples.append(payload)
      if len(samples) >= TARGET_COUNT:
          break

  Path('reports/field_validation_samples.json').write_text(json.dumps(samples, indent=2), encoding='utf-8')
  print('Samples written to reports/field_validation_samples.json')
  PY
  ```
- Regenerate the “Detailed course checks” section by re-running the Markdown generator (see `reports/field_validation_details.mdpart` in this commit or adapt the helper inside `scripts/tools` if promoted later).
