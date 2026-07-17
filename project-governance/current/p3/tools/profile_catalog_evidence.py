#!/usr/bin/env python3
"""Profile the cached P3 Rutgers Catalog evidence without network access."""

from __future__ import annotations

import csv
import hashlib
import json
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable


REPOSITORY_ROOT = Path(__file__).resolve().parents[4]
P3_ROOT = Path(__file__).resolve().parents[1]
LEDGER_PATH = P3_ROOT / "02-catalog-request-ledger.tsv"
PROFILE_PATH = P3_ROOT / "04a-catalog-profile.json"
SCOPE_TSV_PATH = P3_ROOT / "04b-catalog-scope-summary.tsv"
FIELD_TSV_PATH = P3_ROOT / "04c-catalog-field-profile.tsv"
DELIVERY_TSV_PATH = P3_ROOT / "05a-delivery-raw-values.tsv"
MEETING_TSV_PATH = P3_ROOT / "05b-meeting-raw-values.tsv"
SECTION_TYPE_MODE_TSV_PATH = P3_ROOT / "05c-section-course-type-meeting-cross-tab.tsv"
SECTION_TYPE_TIME_TSV_PATH = P3_ROOT / "05d-section-course-type-time-evidence.tsv"
SECTION_MODALITY_CONSISTENCY_TSV_PATH = P3_ROOT / "05e-section-modality-consistency.tsv"
IDENTITY_TSV_PATH = P3_ROOT / "06a-section-identity-summary.tsv"
FILTER_TSV_PATH = P3_ROOT / "06b-filter-source-profile.tsv"
EVIDENCE_REGISTER_PATH = P3_ROOT / "03-catalog-evidence-register.tsv"


def is_nonempty(value: Any) -> bool:
    if value is None:
        return False
    if isinstance(value, str):
        return bool(value.strip())
    if isinstance(value, (list, dict, tuple, set)):
        return bool(value)
    return True


def type_name(value: Any) -> str:
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, int):
        return "integer"
    if isinstance(value, float):
        return "number"
    if isinstance(value, str):
        return "string"
    if isinstance(value, list):
        return "array"
    if isinstance(value, dict):
        return "object"
    return type(value).__name__


def norm_text(value: Any) -> str:
    return str(value).strip() if value is not None else ""


def canonical_hash(value: Any) -> str:
    encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest().upper()


ORDER_DERIVED_SECTION_FIELDS = {"commentsText", "instructorsText", "crossListedSectionsText"}


def normalize_semantic_value(value: Any, *, drop_section_derived: bool = False) -> Any:
    """Normalize order-insensitive raw collections for duplicate conflict checks.

    Rutgers occasionally repeats the same section and only changes array order plus
    the corresponding derived display text. This function is not a general ingest
    merge; it is a conservative evidence comparison used to decide whether duplicate
    `(term,campus,index)` rows contradict one another.
    """

    if isinstance(value, dict):
        return {
            key: normalize_semantic_value(child)
            for key, child in sorted(value.items())
            if not (drop_section_derived and key in ORDER_DERIVED_SECTION_FIELDS)
        }
    if isinstance(value, list):
        normalized = [normalize_semantic_value(child) for child in value]
        return sorted(normalized, key=lambda child: json.dumps(child, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
    if isinstance(value, str):
        return value.strip()
    return value


def semantic_section_hash(section: dict[str, Any]) -> str:
    normalized = {
        key: normalize_semantic_value(value)
        for key, value in sorted(section.items())
        if key not in ORDER_DERIVED_SECTION_FIELDS
    }
    return canonical_hash(normalized)


@dataclass
class FieldStat:
    parent_rows: int = 0
    key_present: int = 0
    non_null: int = 0
    non_empty: int = 0
    types: Counter[str] = field(default_factory=Counter)
    scopes: set[str] = field(default_factory=set)

    def observe_value(self, value: Any, scope: str) -> None:
        self.key_present += 1
        self.types[type_name(value)] += 1
        self.scopes.add(scope)
        if value is not None:
            self.non_null += 1
        if is_nonempty(value):
            self.non_empty += 1


def write_tsv(path: Path, fieldnames: list[str], rows: Iterable[dict[str, Any]]) -> None:
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        for row in rows:
            writer.writerow({name: row.get(name, "") for name in fieldnames})


def load_success_rows() -> list[dict[str, str]]:
    with LEDGER_PATH.open("r", encoding="utf-8", newline="") as handle:
        rows = [row for row in csv.DictReader(handle, delimiter="\t") if row.get("outcome") == "SUCCESS"]
    if len(rows) != 21:
        raise RuntimeError(f"expected 21 successful Catalog payloads, found {len(rows)}")
    return rows


def track_fields(stats: dict[str, FieldStat], parent_counts: Counter[str], prefix: str, record: dict[str, Any], scope: str) -> None:
    parent_counts[prefix] += 1
    for key in record:
        path = f"{prefix}.{key}"
        stat = stats.setdefault(path, FieldStat())
        stat.observe_value(record[key], scope)


def main() -> None:
    ledger_rows = load_success_rows()
    field_stats: dict[str, FieldStat] = {}
    parent_counts: Counter[str] = Counter()
    scope_rows: list[dict[str, Any]] = []
    delivery_counts: Counter[tuple[str, str]] = Counter()
    delivery_scopes: defaultdict[tuple[str, str], set[str]] = defaultdict(set)
    day_counts: Counter[str] = Counter()
    day_scopes: defaultdict[str, set[str]] = defaultdict(set)
    time_shape_counts: Counter[str] = Counter()
    section_type_mode_section_counts: Counter[tuple[str, str, str]] = Counter()
    section_type_mode_scopes: defaultdict[tuple[str, str, str], set[str]] = defaultdict(set)
    section_type_time_evidence_counts: Counter[tuple[str, str]] = Counter()
    section_modality_consistency_counts: Counter[tuple[str, str, str]] = Counter()
    section_tuple_records: defaultdict[tuple[str, str, str], list[dict[str, Any]]] = defaultdict(list)
    term_index_scopes: defaultdict[tuple[str, str], set[str]] = defaultdict(set)
    bare_index_scopes: defaultdict[str, set[tuple[str, str]]] = defaultdict(set)
    course_identity_counts: Counter[tuple[str, str, str]] = Counter()
    course_group_records: defaultdict[tuple[str, str, str], list[dict[str, Any]]] = defaultdict(list)
    raw_top_level_hashes: dict[str, str] = {}

    global_counts = Counter()
    open_status_counts: Counter[str] = Counter()
    section_course_type_counts: Counter[str] = Counter()
    session_indicator_counts: Counter[str] = Counter()
    permission_add_counts: Counter[str] = Counter()
    final_exam_type_counts: Counter[str] = Counter()
    instructor_object_type_counts: Counter[str] = Counter()

    for ledger in ledger_rows:
        request_id = ledger["request_id"]
        term_id = ledger["term_id"]
        campus = ledger["campus"]
        scope = f"{term_id}/{campus}"
        raw_path = REPOSITORY_ROOT / ledger["raw_relative_path"]
        raw_bytes = raw_path.read_bytes()
        actual_sha = hashlib.sha256(raw_bytes).hexdigest().upper()
        if actual_sha != ledger["body_sha256"]:
            raise RuntimeError(f"body hash mismatch for {request_id}")
        payload = json.loads(raw_bytes)
        if not isinstance(payload, list):
            raise RuntimeError(f"root is not array for {request_id}")
        raw_top_level_hashes[request_id] = actual_sha

        scope_counts = Counter()
        scope_section_keys: Counter[tuple[str, str, str]] = Counter()
        for course in payload:
            if not isinstance(course, dict):
                scope_counts["non_object_courses"] += 1
                continue
            scope_counts["courses"] += 1
            global_counts["courses"] += 1
            track_fields(field_stats, parent_counts, "course", course, scope)
            course_key = (term_id, campus, norm_text(course.get("courseString")))
            course_identity_counts[course_key] += 1
            course_group_records[course_key].append(
                {
                    "variantSignature": canonical_hash(
                        {
                            "supplementCode": norm_text(course.get("supplementCode")),
                            "creditsObject": normalize_semantic_value(course.get("creditsObject")),
                            "title": norm_text(course.get("title")),
                            "expandedTitle": norm_text(course.get("expandedTitle")),
                            "level": norm_text(course.get("level")),
                            "offeringUnitCode": norm_text(course.get("offeringUnitCode")),
                        }
                    ),
                    "supplementCode": norm_text(course.get("supplementCode")),
                    "creditsCode": norm_text((course.get("creditsObject") or {}).get("code")) if isinstance(course.get("creditsObject"), dict) else "",
                    "title": norm_text(course.get("title")),
                    "sectionIndexes": sorted(
                        norm_text(section.get("index"))
                        for section in (course.get("sections") or [])
                        if isinstance(section, dict) and norm_text(section.get("index"))
                    ),
                }
            )

            sections = course.get("sections")
            if not isinstance(sections, list):
                scope_counts["courses_sections_not_array"] += 1
                continue
            for section in sections:
                if not isinstance(section, dict):
                    scope_counts["non_object_sections"] += 1
                    continue
                scope_counts["sections"] += 1
                global_counts["sections"] += 1
                track_fields(field_stats, parent_counts, "section", section, scope)
                index = norm_text(section.get("index"))
                if not index:
                    scope_counts["sections_missing_index"] += 1
                else:
                    section_key = (term_id, campus, index)
                    scope_section_keys[section_key] += 1
                    term_index_scopes[(term_id, index)].add(campus)
                    bare_index_scopes[index].add((term_id, campus))
                    section_tuple_records[section_key].append(
                        {
                            "courseString": norm_text(course.get("courseString")),
                            "subject": norm_text(course.get("subject")),
                            "courseNumber": norm_text(course.get("courseNumber")),
                            "sectionNumber": norm_text(section.get("number")),
                            "sectionHash": canonical_hash(section),
                            "semanticSectionHash": semantic_section_hash(section),
                            "crossListedSectionType": norm_text(section.get("crossListedSectionType")),
                            "crossListedSectionsCount": len(section.get("crossListedSections") or [])
                            if isinstance(section.get("crossListedSections"), list)
                            else 0,
                        }
                    )

                open_status_counts[norm_text(section.get("openStatus")) or "<EMPTY>"] += 1
                section_course_type_counts[norm_text(section.get("sectionCourseType")) or "<EMPTY>"] += 1
                session_indicator_counts[norm_text(section.get("sessionDatePrintIndicator")) or "<EMPTY>"] += 1
                permission_add_counts[norm_text(section.get("specialPermissionAddCode")) or "<EMPTY>"] += 1
                final_exam_type_counts[type_name(section.get("finalExam"))] += 1

                instructors = section.get("instructors")
                if isinstance(instructors, list):
                    for instructor in instructors:
                        scope_counts["instructor_assignments"] += 1
                        global_counts["instructor_assignments"] += 1
                        instructor_object_type_counts[type_name(instructor)] += 1
                        if isinstance(instructor, dict):
                            track_fields(field_stats, parent_counts, "instructor", instructor, scope)
                    if any(isinstance(item, dict) and is_nonempty(item.get("name")) for item in instructors):
                        global_counts["sections_with_instructor"] += 1

                meetings = section.get("meetingTimes")
                if not isinstance(meetings, list):
                    scope_counts["sections_meetings_not_array"] += 1
                    continue
                if not meetings:
                    scope_counts["sections_without_meetings"] += 1
                section_type = norm_text(section.get("sectionCourseType")) or "<EMPTY>"
                section_mode_tuples: set[tuple[str, str]] = set()
                has_scheduled = False
                has_explicit_sync = False
                has_explicit_async = False
                has_generic_online = False
                has_by_arrangement_online_remote = False
                has_explicit_remote = False
                has_explicit_physical = False
                for meeting in meetings:
                    if not isinstance(meeting, dict):
                        scope_counts["non_object_meetings"] += 1
                        continue
                    scope_counts["meetings"] += 1
                    global_counts["meetings"] += 1
                    track_fields(field_stats, parent_counts, "meeting", meeting, scope)
                    code = norm_text(meeting.get("meetingModeCode")) or "<EMPTY>"
                    desc = norm_text(meeting.get("meetingModeDesc")) or "<EMPTY>"
                    delivery_counts[(code, desc)] += 1
                    delivery_scopes[(code, desc)].add(scope)
                    section_mode_tuples.add((code, desc))
                    has_explicit_sync = has_explicit_sync or code == "92"
                    has_explicit_async = has_explicit_async or code == "93"
                    has_generic_online = has_generic_online or code == "90"
                    campus_location = norm_text(meeting.get("campusLocation"))
                    if code in {"90", "92", "93"} or campus_location in {"O", "T"}:
                        has_explicit_remote = True
                    if campus_location and campus_location not in {"O", "T"} and code not in {"90", "92", "93"}:
                        has_explicit_physical = True
                    day = norm_text(meeting.get("meetingDay")) or "<EMPTY>"
                    day_counts[day] += 1
                    day_scopes[day].add(scope)
                    start = norm_text(meeting.get("startTimeMilitary"))
                    end = norm_text(meeting.get("endTimeMilitary"))
                    if start and end:
                        time_shape_counts["BOTH_PRESENT"] += 1
                        has_scheduled = True
                    elif not start and not end:
                        time_shape_counts["BOTH_EMPTY"] += 1
                        if norm_text(meeting.get("baClassHours")) == "B" and campus_location in {"O", "T"}:
                            has_by_arrangement_online_remote = True
                    else:
                        time_shape_counts["PARTIAL"] += 1

                for code, desc in section_mode_tuples:
                    section_type_mode_section_counts[(section_type, code, desc)] += 1
                    section_type_mode_scopes[(section_type, code, desc)].add(scope)
                evidence_flags = {
                    "HAS_SCHEDULED_OCCURRENCE": has_scheduled,
                    "HAS_EXPLICIT_REMOTE_SYNC_92": has_explicit_sync,
                    "HAS_EXPLICIT_REMOTE_ASYNC_93": has_explicit_async,
                    "HAS_GENERIC_ONLINE_90": has_generic_online,
                    "HAS_BY_ARRANGEMENT_ONLINE_REMOTE_LOCATION": has_by_arrangement_online_remote,
                    "NO_SCHEDULED_OCCURRENCE": not has_scheduled,
                }
                for evidence_name, present in evidence_flags.items():
                    if present:
                        section_type_time_evidence_counts[(section_type, evidence_name)] += 1
                occurrence_kind = (
                    "REMOTE_AND_PHYSICAL"
                    if has_explicit_remote and has_explicit_physical
                    else "REMOTE_ONLY"
                    if has_explicit_remote
                    else "PHYSICAL_ONLY"
                    if has_explicit_physical
                    else "NO_EXPLICIT_KIND"
                )
                if section_type == "T":
                    oracle_outcome = "UNKNOWN_CONFLICT" if has_explicit_remote else "ON_CAMPUS_OR_IN_PERSON"
                elif section_type == "O":
                    oracle_outcome = "UNKNOWN_CONFLICT" if has_explicit_physical else "ONLINE"
                elif section_type == "H":
                    oracle_outcome = "HYBRID"
                else:
                    oracle_outcome = "UNKNOWN_UNRECOGNIZED_SECTION_TYPE"
                section_modality_consistency_counts[(section_type, occurrence_kind, oracle_outcome)] += 1

        duplicate_keys = sum(1 for count in scope_section_keys.values() if count > 1)
        duplicate_rows = sum(count - 1 for count in scope_section_keys.values() if count > 1)
        scope_rows.append(
            {
                "request_id": request_id,
                "term_id": term_id,
                "campus": campus,
                "courses": scope_counts["courses"],
                "sections": scope_counts["sections"],
                "meetings": scope_counts["meetings"],
                "instructor_assignments": scope_counts["instructor_assignments"],
                "sections_without_meetings": scope_counts["sections_without_meetings"],
                "sections_missing_index": scope_counts["sections_missing_index"],
                "duplicate_composite_keys": duplicate_keys,
                "duplicate_composite_rows": duplicate_rows,
                "decoded_bytes": int(ledger["decoded_bytes"]),
                "duration_ms": float(ledger["duration_ms"]),
                "body_sha256": actual_sha,
            }
        )

    duplicate_composite = []
    for key, records in section_tuple_records.items():
        if len(records) > 1:
            duplicate_composite.append(
                {
                    "term_id": key[0],
                    "campus": key[1],
                    "index": key[2],
                    "records": records,
                    "record_count": len(records),
                    "distinct_section_hashes": len({record["sectionHash"] for record in records}),
                    "distinct_semantic_section_hashes": len({record["semanticSectionHash"] for record in records}),
                    "distinct_courses": len({record["courseString"] for record in records}),
                }
            )

    term_index_collisions = [
        {"term_id": term, "index": index, "campuses": sorted(campuses)}
        for (term, index), campuses in term_index_scopes.items()
        if len(campuses) > 1
    ]
    bare_index_collisions = [
        {"index": index, "scopes": [f"{term}/{campus}" for term, campus in sorted(scopes)]}
        for index, scopes in bare_index_scopes.items()
        if len(scopes) > 1
    ]
    duplicate_course_keys = [
        {
            "term_id": key[0],
            "campus": key[1],
            "course_string": key[2],
            "count": count,
            "distinct_variant_signatures": len({record["variantSignature"] for record in course_group_records[key]}),
            "section_overlap_count": sum(
                1
                for index, index_count in Counter(
                    index for record in course_group_records[key] for index in record["sectionIndexes"]
                ).items()
                if index_count > 1
            ),
            "variants": [
                {
                    "supplementCode": record["supplementCode"],
                    "creditsCode": record["creditsCode"],
                    "title": record["title"],
                    "sectionCount": len(record["sectionIndexes"]),
                }
                for record in course_group_records[key]
            ],
        }
        for key, count in course_identity_counts.items()
        if count > 1
    ]

    field_rows = []
    for path, stat in sorted(field_stats.items()):
        prefix = path.split(".", 1)[0]
        stat.parent_rows = parent_counts[prefix]
        field_rows.append(
            {
                "path": path,
                "parent_rows": stat.parent_rows,
                "key_present": stat.key_present,
                "key_present_rate": round(stat.key_present / stat.parent_rows, 8) if stat.parent_rows else 0,
                "non_null": stat.non_null,
                "non_null_rate": round(stat.non_null / stat.parent_rows, 8) if stat.parent_rows else 0,
                "non_empty": stat.non_empty,
                "non_empty_rate": round(stat.non_empty / stat.parent_rows, 8) if stat.parent_rows else 0,
                "types": json.dumps(dict(sorted(stat.types.items())), separators=(",", ":")),
                "scope_count": len(stat.scopes),
            }
        )

    delivery_rows = [
        {
            "meeting_mode_code": code,
            "meeting_mode_desc": desc,
            "meeting_count": count,
            "scope_count": len(delivery_scopes[(code, desc)]),
            "scopes": ",".join(sorted(delivery_scopes[(code, desc)])),
        }
        for (code, desc), count in sorted(delivery_counts.items(), key=lambda item: (-item[1], item[0]))
    ]
    meeting_rows = [
        {
            "meeting_day": day,
            "meeting_count": count,
            "scope_count": len(day_scopes[day]),
            "scopes": ",".join(sorted(day_scopes[day])),
        }
        for day, count in sorted(day_counts.items(), key=lambda item: (-item[1], item[0]))
    ]
    section_type_mode_rows = [
        {
            "section_course_type": section_type,
            "meeting_mode_code": code,
            "meeting_mode_desc": desc,
            "raw_section_rows": count,
            "scope_count": len(section_type_mode_scopes[(section_type, code, desc)]),
            "scopes": ",".join(sorted(section_type_mode_scopes[(section_type, code, desc)])),
        }
        for (section_type, code, desc), count in sorted(
            section_type_mode_section_counts.items(),
            key=lambda item: (item[0][0], -item[1], item[0][1], item[0][2]),
        )
    ]
    section_type_time_rows = [
        {
            "section_course_type": section_type,
            "evidence_flag": evidence_name,
            "raw_section_rows": count,
            "section_type_total": section_course_type_counts[section_type],
            "rate_within_section_type": round(count / section_course_type_counts[section_type], 8)
            if section_course_type_counts[section_type]
            else 0,
        }
        for (section_type, evidence_name), count in sorted(section_type_time_evidence_counts.items())
    ]
    section_modality_consistency_rows = [
        {
            "section_course_type": section_type,
            "occurrence_kind": occurrence_kind,
            "oracle_outcome": oracle_outcome,
            "raw_section_rows": count,
            "section_type_total": section_course_type_counts[section_type],
            "rate_within_section_type": round(count / section_course_type_counts[section_type], 8)
            if section_course_type_counts[section_type]
            else 0,
        }
        for (section_type, occurrence_kind, oracle_outcome), count in sorted(section_modality_consistency_counts.items())
    ]

    path_map = {row["path"]: row for row in field_rows}
    filter_specs = [
        ("FLT-C01", "Term", "request.term_id", len(scope_rows), len(scope_rows), "request scope"),
        ("FLT-C02", "Campus", "request.campus", len(scope_rows), len(scope_rows), "official selector + request scope"),
        ("FLT-C03", "Subject", "course.subject", global_counts["courses"], path_map["course.subject"]["non_empty"], "structured"),
        ("FLT-C04", "Text search", "course.title|expandedTitle|description|notes", global_counts["courses"], max(path_map["course.title"]["non_empty"], path_map["course.expandedTitle"]["non_empty"]), "raw text exists; FTS ingest separately audited"),
        ("FLT-C05", "Course number", "course.courseNumber", global_counts["courses"], path_map["course.courseNumber"]["non_empty"], "structured"),
        ("FLT-C06", "Level", "course.level", global_counts["courses"], path_map["course.level"]["non_empty"], "structured raw value"),
        ("FLT-C07", "Credits", "course.creditsObject|credits", global_counts["courses"], max(path_map["course.creditsObject"]["non_empty"], path_map["course.credits"]["non_empty"]), "structured object preferred"),
        ("FLT-C08", "Core code", "course.coreCodes", global_counts["courses"], path_map["course.coreCodes"]["non_empty"], "legitimate sparse array"),
        ("FLT-C09", "Prerequisite presence", "course.preReqNotes", global_counts["courses"], path_map["course.preReqNotes"]["non_empty"], "empty vs unknown requires source rule"),
        ("FLT-C10", "Course campus location", "course.campusLocations", global_counts["courses"], path_map["course.campusLocations"]["non_empty"], "structured array"),
        ("FLT-S01", "Index", "section.index", global_counts["sections"], path_map["section.index"]["non_empty"], "structured; composite scope required"),
        ("FLT-S02", "Section number", "section.number", global_counts["sections"], path_map["section.number"]["non_empty"], "structured"),
        ("FLT-S03", "Open status", "section.openStatus", global_counts["sections"], path_map["section.openStatus"]["non_empty"], "catalog status is not live Open evidence; see P3 Open documents 14-17"),
        ("FLT-S04a", "Delivery modality", "section.sectionCourseType", global_counts["sections"], path_map["section.sectionCourseType"]["non_empty"], "official SOC selector defines T/H/O; preserve raw"),
        ("FLT-S04b", "Delivery synchronicity", "meeting.meetingModeCode|Desc|day|start|end|campusLocation", global_counts["meetings"], sum(count for (code, desc), count in delivery_counts.items() if code != "<EMPTY>" or desc != "<EMPTY>"), "explicit 92/93 and by-arrangement O/T are evidence; generic 90 remains UNSPECIFIED"),
        ("FLT-S05", "Instructor", "section.instructors[].name", global_counts["sections"], global_counts["sections_with_instructor"], "name only; no stable ID observed"),
        ("FLT-S06", "Availability", "meeting.day/start/end", global_counts["meetings"], time_shape_counts["BOTH_PRESENT"], "requiredness and TBA rules apply"),
        ("FLT-S07", "Meeting campus/location", "meeting.campusLocation|campusName", global_counts["meetings"], max(path_map["meeting.campusLocation"]["non_empty"], path_map["meeting.campusName"]["non_empty"]), "structured raw"),
        ("FLT-S08", "Building/room", "meeting.buildingCode|roomNumber", global_counts["meetings"], max(path_map["meeting.buildingCode"]["non_empty"], path_map["meeting.roomNumber"]["non_empty"]), "legitimate sparse"),
        ("FLT-S09", "Exam", "section.examCode|finalExam", global_counts["sections"], max(path_map["section.examCode"]["non_empty"], path_map["section.finalExam"]["non_empty"]), "mixed structured/text shape"),
        ("FLT-S10", "Special permission", "section.specialPermissionAddCode", global_counts["sections"], path_map["section.specialPermissionAddCode"]["non_empty"], "structured code"),
        ("FLT-S11", "Eligibility", "section.majors|minors|honorPrograms|eligibility|openToText", global_counts["sections"], max(path_map["section.majors"]["non_empty"], path_map["section.minors"]["non_empty"], path_map["section.honorPrograms"]["non_empty"], path_map["section.sectionEligibility"]["non_empty"], path_map["section.openToText"]["non_empty"]), "multiple structured/text sources"),
    ]
    filter_rows = [
        {
            "filter_id": fid,
            "label": label,
            "source_paths": source_paths,
            "denominator": denominator,
            "non_empty": non_empty,
            "non_empty_rate": round(non_empty / denominator, 8) if denominator else 0,
            "evidence_note": note,
        }
        for fid, label, source_paths, denominator, non_empty, note in filter_specs
    ]

    evidence_rows = [
        {
            "evidence_id": f"CAT-E{position:03d}",
            "request_id": row["request_id"],
            "term_id": row["term_id"],
            "campus": row["campus"],
            "raw_relative_path": row["raw_relative_path"],
            "body_sha256": row["body_sha256"],
            "observed_at_utc": row["completed_at_utc"],
            "observation_strength": "OBSERVED_ONCE",
            "fixture_path": "<NONE_FULL_RAW_HASH_ONLY>",
            "claims_supported": "CATALOG_SCOPE_COVERAGE|FIELD_PROFILE|IDENTITY_PROFILE|FILTER_SOURCE_PROFILE",
            "limitations": "single_observation_per_scope;not_an_SLA;Open_semantics_not_tested",
        }
        for position, row in enumerate(ledger_rows, start=1)
    ]

    identity_rows = [
        {"metric": "section_rows", "count": global_counts["sections"], "severity": "INFO"},
        {"metric": "unique_term_campus_index", "count": len(section_tuple_records), "severity": "INFO"},
        {"metric": "duplicate_term_campus_index_keys", "count": len(duplicate_composite), "severity": "RAW_DUPLICATE_REQUIRES_NORMALIZATION"},
        {"metric": "duplicate_term_campus_index_rows", "count": sum(item["record_count"] - 1 for item in duplicate_composite), "severity": "RAW_DUPLICATE_REQUIRES_NORMALIZATION"},
        {"metric": "semantically_equivalent_duplicate_keys", "count": sum(1 for item in duplicate_composite if item["distinct_semantic_section_hashes"] == 1), "severity": "SAFE_TO_COLLAPSE_WITH_AUDIT"},
        {"metric": "semantically_conflicting_duplicate_keys", "count": sum(1 for item in duplicate_composite if item["distinct_semantic_section_hashes"] > 1), "severity": "CRITICAL_IF_NONZERO"},
        {"metric": "term_index_cross_campus_collisions", "count": len(term_index_collisions), "severity": "EXPECTED_PROOF_COMPOSITE"},
        {"metric": "bare_index_cross_scope_collisions", "count": len(bare_index_collisions), "severity": "EXPECTED_PROOF_COMPOSITE"},
        {"metric": "multi_object_course_string_groups", "count": len(duplicate_course_keys), "severity": "COURSE_GROUP_NORMALIZATION_REQUIRED"},
        {"metric": "course_groups_with_multiple_variants", "count": sum(1 for item in duplicate_course_keys if item["distinct_variant_signatures"] > 1), "severity": "COURSE_VARIANT_MODEL_REQUIRED"},
        {"metric": "course_groups_with_section_overlap", "count": sum(1 for item in duplicate_course_keys if item["section_overlap_count"] > 0), "severity": "CONFLICT_CHECK_REQUIRED"},
    ]

    profile = {
        "schemaVersion": 1,
        "source": "P3 frozen Catalog raw evidence",
        "successPayloads": len(ledger_rows),
        "counts": dict(global_counts),
        "scopeSummary": scope_rows,
        "fieldProfile": field_rows,
        "deliveryRawValues": delivery_rows,
        "meetingDayValues": meeting_rows,
        "timeShapeCounts": dict(time_shape_counts),
        "openStatusCounts": dict(open_status_counts),
        "sectionCourseTypeCounts": dict(section_course_type_counts),
        "sectionCourseTypeMeetingModeCrossTab": section_type_mode_rows,
        "sectionCourseTypeTimeEvidence": section_type_time_rows,
        "sectionModalityConsistency": section_modality_consistency_rows,
        "sessionIndicatorCounts": dict(session_indicator_counts),
        "permissionAddCounts": dict(permission_add_counts),
        "finalExamTypeCounts": dict(final_exam_type_counts),
        "instructorObjectTypeCounts": dict(instructor_object_type_counts),
        "identity": {
            "summary": identity_rows,
            "duplicateCompositeKeys": duplicate_composite,
            "termIndexCrossCampusCollisions": term_index_collisions,
            "bareIndexCrossScopeCollisions": bare_index_collisions,
            "duplicateCourseIdentityKeys": duplicate_course_keys,
        },
        "filterSourceProfile": filter_rows,
        "rawBodyHashes": raw_top_level_hashes,
    }
    PROFILE_PATH.write_text(json.dumps(profile, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    write_tsv(
        SCOPE_TSV_PATH,
        ["request_id", "term_id", "campus", "courses", "sections", "meetings", "instructor_assignments", "sections_without_meetings", "sections_missing_index", "duplicate_composite_keys", "duplicate_composite_rows", "decoded_bytes", "duration_ms", "body_sha256"],
        scope_rows,
    )
    write_tsv(FIELD_TSV_PATH, ["path", "parent_rows", "key_present", "key_present_rate", "non_null", "non_null_rate", "non_empty", "non_empty_rate", "types", "scope_count"], field_rows)
    write_tsv(DELIVERY_TSV_PATH, ["meeting_mode_code", "meeting_mode_desc", "meeting_count", "scope_count", "scopes"], delivery_rows)
    write_tsv(MEETING_TSV_PATH, ["meeting_day", "meeting_count", "scope_count", "scopes"], meeting_rows)
    write_tsv(
        SECTION_TYPE_MODE_TSV_PATH,
        ["section_course_type", "meeting_mode_code", "meeting_mode_desc", "raw_section_rows", "scope_count", "scopes"],
        section_type_mode_rows,
    )
    write_tsv(
        SECTION_TYPE_TIME_TSV_PATH,
        ["section_course_type", "evidence_flag", "raw_section_rows", "section_type_total", "rate_within_section_type"],
        section_type_time_rows,
    )
    write_tsv(
        SECTION_MODALITY_CONSISTENCY_TSV_PATH,
        ["section_course_type", "occurrence_kind", "oracle_outcome", "raw_section_rows", "section_type_total", "rate_within_section_type"],
        section_modality_consistency_rows,
    )
    write_tsv(IDENTITY_TSV_PATH, ["metric", "count", "severity"], identity_rows)
    write_tsv(FILTER_TSV_PATH, ["filter_id", "label", "source_paths", "denominator", "non_empty", "non_empty_rate", "evidence_note"], filter_rows)
    write_tsv(
        EVIDENCE_REGISTER_PATH,
        [
            "evidence_id",
            "request_id",
            "term_id",
            "campus",
            "raw_relative_path",
            "body_sha256",
            "observed_at_utc",
            "observation_strength",
            "fixture_path",
            "claims_supported",
            "limitations",
        ],
        evidence_rows,
    )

    print(json.dumps({"successPayloads": len(ledger_rows), "counts": dict(global_counts), "identity": identity_rows, "deliveryValues": len(delivery_rows), "meetingDayValues": len(meeting_rows)}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
