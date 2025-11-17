#!/usr/bin/env python3
"""
Generate a Rutgers SOC field matrix by sampling multiple term/campus combinations.

The runner fetches courses.json + openSections.json payloads, aggregates field coverage
across courses/sections/meetings, and emits docs/soc_field_matrix.csv so downstream
tasks can reason about FR-01/FR-02 requirements.
"""

from __future__ import annotations

import csv
import gzip
import json
import time
import urllib.error
import urllib.parse
import urllib.request
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

BASE_URL = "https://classes.rutgers.edu/soc/api"
USER_AGENT = "BetterCourseSchedulePlanner/field-matrix"
OUTPUT_CSV = Path("docs/soc_field_matrix.csv")


@dataclass(frozen=True)
class TermConfig:
    label: str
    code: str
    year: int
    term: int


@dataclass(frozen=True)
class CampusConfig:
    code: str
    label: str


@dataclass(frozen=True)
class SubjectConfig:
    code: str
    label: str
    levels: tuple[str, ...]


TERMS: tuple[TermConfig, ...] = (
    TermConfig(label="Spring 2024", code="12024", year=2024, term=1),
    TermConfig(label="Fall 2024", code="92024", year=2024, term=9),
)

CAMPUSES: tuple[CampusConfig, ...] = (
    CampusConfig(code="NB", label="New Brunswick"),
    CampusConfig(code="NK", label="Newark"),
    CampusConfig(code="CM", label="Camden"),
)

SUBJECTS: tuple[SubjectConfig, ...] = (
    SubjectConfig(code="198", label="Computer Science", levels=("U",)),
    SubjectConfig(code="640", label="Mathematics", levels=("U", "G")),
    SubjectConfig(code="750", label="Physics", levels=("G",)),
    SubjectConfig(code="960", label="Statistics", levels=("U", "G")),
    SubjectConfig(code="014", label="AMESALL / Area Studies", levels=("U",)),
)


def _has_value(value: Any) -> bool:
    if value is None:
        return False
    if isinstance(value, str):
        return bool(value.strip())
    if isinstance(value, (list, dict, set, tuple)):
        return len(value) > 0
    return True


class FieldStats:
    """Track field presence/sample values for a given scope."""

    def __init__(self, scope: str, skip_keys: Iterable[str] | None = None) -> None:
        self.scope = scope
        self.skip_keys = set(skip_keys or ())
        self.total = 0
        self.counts: dict[str, int] = defaultdict(int)
        self.samples: dict[str, list[str]] = defaultdict(list)

    def observe(self, obj: dict[str, Any]) -> None:
        self.total += 1
        for key, value in obj.items():
            if key in self.skip_keys:
                continue
            self._ingest(key, value)

    def _ingest(self, key: str, value: Any) -> None:
        if value is None:
            return
        if isinstance(value, dict):
            for sub_key, sub_value in value.items():
                self._ingest(f"{key}.{sub_key}", sub_value)
            return
        if isinstance(value, list):
            if not value:
                return
            self.counts[key] += 1
            if len(self.samples[key]) < 3:
                snippet = value if isinstance(value[0], (str, int, float, bool)) else value[:1]
                self.samples[key].append(_truncate(json.dumps(snippet, ensure_ascii=False)))
            return
        if isinstance(value, str):
            value = value.strip()
            if not value:
                return
            display = value
        else:
            display = str(value)
        self.counts[key] += 1
        if len(self.samples[key]) < 3:
            self.samples[key].append(_truncate(display))

    def to_rows(self) -> list[dict[str, Any]]:
        rows: list[dict[str, Any]] = []
        for field in sorted(self.counts.keys()):
            count = self.counts[field]
            presence = count / self.total if self.total else 0
            rows.append(
                {
                    "scope": self.scope,
                    "field": field,
                    "non_null": count,
                    "total": self.total,
                    "presence_pct": f"{presence:.4f}",
                    "sample_values": " | ".join(self.samples[field]),
                }
            )
        return rows


def _truncate(value: str, limit: int = 120) -> str:
    return value if len(value) <= limit else f"{value[:limit]}…"


def fetch_json(endpoint: str, *, year: int, term: int, campus: str, level: str | None = None) -> tuple[Any, dict[str, Any]]:
    params: dict[str, Any] = {
        "year": year,
        "term": term,
        "campus": campus,
    }
    if level:
        params["level"] = level
    query = urllib.parse.urlencode(params)
    url = f"{BASE_URL}/{endpoint}.json?{query}"
    req = urllib.request.Request(
        url,
        headers={
            "Accept": "application/json",
            "Accept-Encoding": "gzip",
            "User-Agent": USER_AGENT,
        },
    )
    started = time.time()
    try:
        with urllib.request.urlopen(req, timeout=30) as response:
            payload = response.read()
            if response.headers.get("Content-Encoding") == "gzip" or payload[:2] == b"\x1f\x8b":
                payload = gzip.decompress(payload)
    except urllib.error.HTTPError as error:
        raise RuntimeError(f"HTTP {error.code} for {url}") from error
    decoded = json.loads(payload)
    metadata = {
        "url": url,
        "bytes": len(payload),
        "duration_ms": int((time.time() - started) * 1000),
    }
    return decoded, metadata


def main() -> None:
    combos = []
    for term in TERMS:
        for campus in CAMPUSES:
            for subject in SUBJECTS:
                for level in subject.levels:
                    combos.append(
                        {
                            "term": term,
                            "campus": campus,
                            "subject": subject,
                            "level": level,
                        }
                    )

    print(f"Planned combinations: {len(combos)} (terms={len(TERMS)}, campus={len(CAMPUSES)}, subjects={len(SUBJECTS)})")

    course_stats = FieldStats("course", skip_keys={"sections", "campusLocations"})
    course_campus_stats = FieldStats("course.campusLocation")
    section_stats = FieldStats("section", skip_keys={"meetingTimes", "instructors", "sectionCampusLocations"})
    section_campus_stats = FieldStats("section.campusLocation")
    meeting_stats = FieldStats("section.meeting")
    instructor_stats = FieldStats("section.instructor")

    dataset_cache: dict[tuple[str, str], list[dict[str, Any]]] = {}
    open_sections_cache: dict[tuple[str, str], list[str]] = {}
    combo_results: list[dict[str, Any]] = []

    courses_with_locations = 0
    sections_with_meetings = 0

    for term in TERMS:
        for campus in CAMPUSES:
            data, meta = fetch_json("courses", year=term.year, term=term.term, campus=campus.code)
            print(f"[courses] {term.code} {campus.code}: {len(data)} rows • {meta['bytes']:,} bytes • {meta['duration_ms']} ms")
            dataset_cache[(term.code, campus.code)] = data

            open_sections, o_meta = fetch_json("openSections", year=term.year, term=term.term, campus=campus.code)
            print(f"[openSections] {term.code} {campus.code}: {len(open_sections)} indexes • {o_meta['bytes']:,} bytes")
            open_sections_cache[(term.code, campus.code)] = open_sections

            for course in data:
                course_stats.observe(course)
                if course.get("campusLocations"):
                    courses_with_locations += 1
                for campus_loc in course.get("campusLocations") or []:
                    if isinstance(campus_loc, dict):
                        course_campus_stats.observe(campus_loc)
                for section in course.get("sections") or []:
                    if isinstance(section, dict):
                        section_stats.observe(section)
                        if section.get("meetingTimes"):
                            sections_with_meetings += 1
                        for loc in section.get("sectionCampusLocations") or []:
                            if isinstance(loc, dict):
                                section_campus_stats.observe(loc)
                        for meeting in section.get("meetingTimes") or []:
                            if isinstance(meeting, dict):
                                meeting_stats.observe(meeting)
                        for instructor in section.get("instructors") or []:
                            if isinstance(instructor, dict):
                                instructor_stats.observe(instructor)

    # Subject-level filtering uses cached datasets to avoid redundant downloads.
    for combo in combos:
        term = combo["term"]
        campus = combo["campus"]
        subject = combo["subject"]
        level = combo["level"]
        payload = dataset_cache[(term.code, campus.code)]
        filtered = [course for course in payload if course.get("subject") == subject.code]
        section_count = sum(len(course.get("sections") or []) for course in filtered)
        combo_results.append(
            {
                "term": term.code,
                "campus": campus.code,
                "level": level,
                "subject": subject.code,
                "courses": len(filtered),
                "sections": section_count,
            }
        )

    OUTPUT_CSV.parent.mkdir(parents=True, exist_ok=True)
    all_rows = []
    for stats in (
        course_stats,
        course_campus_stats,
        section_stats,
        section_campus_stats,
        meeting_stats,
        instructor_stats,
    ):
        all_rows.extend(stats.to_rows())

    # FR status overrides for specific fields (direct/derived/missing context).
    fr_mapping: dict[tuple[str, str], dict[str, str]] = {
        ("course", "subject"): {"fr_mapping": "FR-01/FR-02", "fr_status": "direct"},
        ("course", "courseNumber"): {"fr_mapping": "FR-01/FR-02", "fr_status": "direct"},
        ("course", "courseString"): {"fr_mapping": "FR-01/FR-02", "fr_status": "direct"},
        ("course", "title"): {"fr_mapping": "FR-01/FR-02", "fr_status": "direct"},
        ("course", "credits"): {"fr_mapping": "FR-01/FR-02", "fr_status": "direct"},
        ("course", "offeringUnitCode"): {"fr_mapping": "FR-01/FR-02", "fr_status": "direct"},
        ("course", "offeringUnitTitle"): {
            "fr_mapping": "FR-01/FR-02",
            "fr_status": "derived",
            "notes": "API returns null; derive from school/subject metadata.",
        },
        ("course", "school.code"): {"fr_mapping": "FR-01/FR-02", "fr_status": "direct"},
        ("course", "school.description"): {"fr_mapping": "FR-01/FR-02", "fr_status": "direct"},
        ("course", "coreCodes"): {"fr_mapping": "FR-01/FR-02", "fr_status": "direct"},
        ("course", "preReqNotes"): {"fr_mapping": "FR-01/FR-02", "fr_status": "direct"},
        ("course", "courseDescription"): {"fr_mapping": "FR-02", "fr_status": "direct"},
        ("course", "subjectDescription"): {"fr_mapping": "FR-02", "fr_status": "derived"},
        ("course", "synopsisUrl"): {"fr_mapping": "FR-02", "fr_status": "derived"},
        ("course", "openSections"): {"fr_mapping": "FR-02", "fr_status": "direct"},
        ("course", "campusLocations"): {"fr_mapping": "FR-02", "fr_status": "direct"},
        ("course.campusLocation", "description"): {"fr_mapping": "FR-02", "fr_status": "direct"},
        ("section", "index"): {"fr_mapping": "FR-01/FR-02", "fr_status": "direct"},
        ("section", "number"): {"fr_mapping": "FR-01/FR-02", "fr_status": "direct"},
        ("section", "openStatus"): {"fr_mapping": "FR-01/FR-02", "fr_status": "direct"},
        ("section", "openStatusText"): {"fr_mapping": "FR-01/FR-02", "fr_status": "direct"},
        ("section", "instructorsText"): {"fr_mapping": "FR-01/FR-02", "fr_status": "direct"},
        ("section", "crossListedSections"): {"fr_mapping": "FR-02", "fr_status": "direct"},
        ("section", "commentsText"): {"fr_mapping": "FR-02", "fr_status": "direct"},
        ("section", "examCode"): {"fr_mapping": "FR-02", "fr_status": "direct"},
        ("section", "examCodeText"): {"fr_mapping": "FR-02", "fr_status": "direct"},
        ("section", "meetingTimes"): {"fr_mapping": "FR-02", "fr_status": "direct"},
        ("section.campusLocation", "description"): {"fr_mapping": "FR-02", "fr_status": "direct"},
        ("section.meeting", "meetingDay"): {"fr_mapping": "FR-02", "fr_status": "direct"},
        ("section.meeting", "startTimeMilitary"): {"fr_mapping": "FR-02", "fr_status": "direct"},
        ("section.meeting", "endTimeMilitary"): {"fr_mapping": "FR-02", "fr_status": "direct"},
        ("section.meeting", "campusName"): {"fr_mapping": "FR-02", "fr_status": "direct"},
        ("section.meeting", "buildingCode"): {"fr_mapping": "FR-02", "fr_status": "direct"},
        ("section.meeting", "roomNumber"): {"fr_mapping": "FR-02", "fr_status": "direct"},
        ("section.meeting", "meetingModeDesc"): {"fr_mapping": "FR-02", "fr_status": "direct"},
        ("section.meeting", "meetingModeCode"): {"fr_mapping": "FR-02", "fr_status": "direct"},
        ("section.instructor", "name"): {"fr_mapping": "FR-01/FR-02", "fr_status": "direct"},
    }

    # Add explicit missing entries for openSections metadata.
    open_sections_rows = []
    total_indexes = sum(len(items) for items in open_sections_cache.values())
    sample_indexes: list[str] = []
    for items in open_sections_cache.values():
        for idx in items:
            sample_indexes.append(idx)
            if len(sample_indexes) >= 5:
                break
        if len(sample_indexes) >= 5:
            break
    if total_indexes:
        open_sections_rows.append(
            {
                "scope": "openSections",
                "field": "index",
                "non_null": total_indexes,
                "total": total_indexes,
                "presence_pct": "1.0000",
                "sample_values": " | ".join(sample_indexes),
                "fr_mapping": "FR-04",
                "fr_status": "direct",
                "notes": "Only exposes Index strings; need courses payload for metadata.",
            }
        )
    open_sections_rows.append(
        {
            "scope": "openSections",
            "field": "capacity",
            "non_null": 0,
            "total": total_indexes,
            "presence_pct": "0.0000",
            "sample_values": "",
            "fr_mapping": "FR-02/FR-04",
            "fr_status": "missing",
            "notes": "Capacity is not provided; must cross-reference sections for limits.",
        }
    )
    open_sections_rows.append(
        {
            "scope": "openSections",
            "field": "notes",
            "non_null": 0,
            "total": total_indexes,
            "presence_pct": "0.0000",
            "sample_values": "",
            "fr_mapping": "FR-02",
            "fr_status": "missing",
            "notes": "No notes/status metadata returned with openSections indexes.",
        }
    )

    field_rows: list[dict[str, Any]] = []
    for row in all_rows:
        key = (row["scope"], row["field"])
        fr_data = fr_mapping.get(key, {})
        row.update(
            {
                "fr_mapping": fr_data.get("fr_mapping", ""),
                "fr_status": fr_data.get("fr_status", ""),
                "notes": fr_data.get("notes", ""),
            }
        )
        field_rows.append(row)

    scope_totals = {
        "course": course_stats.total,
        "section": section_stats.total,
        "section.meeting": meeting_stats.total,
        "section.instructor": instructor_stats.total,
        "section.campusLocation": section_campus_stats.total,
        "course.campusLocation": course_campus_stats.total,
    }

    # Inject manual coverage rows for fields skipped in automatic flattening.
    for extra_field, counts in (
        (("course", "campusLocations"), {"non_null": courses_with_locations}),
        (("section", "meetingTimes"), {"non_null": sections_with_meetings}),
        (("course", "offeringUnitTitle"), {"non_null": 0}),
    ):
        scope, field_name = extra_field
        if any(row["scope"] == scope and row["field"] == field_name for row in field_rows):
            continue
        total = scope_totals.get(scope, 0)
        presence = counts["non_null"] / total if total else 0
        fr_data = fr_mapping.get(extra_field, {})
        field_rows.append(
            {
                "scope": scope,
                "field": field_name,
                "non_null": counts["non_null"],
                "total": total,
                "presence_pct": f"{presence:.4f}",
                "fr_mapping": fr_data.get("fr_mapping", ""),
                "fr_status": fr_data.get("fr_status", ""),
                "sample_values": "",
                "notes": fr_data.get("notes", ""),
            }
        )

    field_rows.extend(open_sections_rows)

    with OUTPUT_CSV.open("w", newline="", encoding="utf-8") as fh:
        writer = csv.DictWriter(
            fh,
            fieldnames=[
                "scope",
                "field",
                "non_null",
                "total",
                "presence_pct",
                "fr_mapping",
                "fr_status",
                "sample_values",
                "notes",
            ],
        )
        writer.writeheader()
        for row in sorted(field_rows, key=lambda item: (item["scope"], item["field"])):
            writer.writerow(row)

    print(f"Field matrix written to {OUTPUT_CSV} ({len(field_rows)} rows)")
    print("\nSubject coverage snapshot (courses • sections):")
    for result in combo_results:
        print(
            f"- {result['term']} {result['campus']} subj={result['subject']} lvl={result['level']} → "
            f"{result['courses']} courses / {result['sections']} sections"
        )


if __name__ == "__main__":
    main()
