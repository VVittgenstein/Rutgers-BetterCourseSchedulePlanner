#!/usr/bin/env python3
"""Analyze Rutgers SOC sample JSON files to extract field coverage stats."""
from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Sequence


COURSE_FIELDS: Sequence[str] = (
    "title",
    "subject",
    "subjectDescription",
    "courseNumber",
    "courseString",
    "credits",
    "creditsObject",
    "synopsisUrl",
    "courseDescription",
    "preReqNotes",
    "courseNotes",
    "coreCodes",
    "campusLocations",
    "openSections",
    "mainCampus",
    "school",
    "offeringUnitCode",
    "offeringUnitTitle",
    "level",
    "sections",
)

SECTION_FIELDS: Sequence[str] = (
    "index",
    "number",
    "openStatus",
    "openStatusText",
    "campusCode",
    "sectionCampusLocations",
    "sectionNotes",
    "meetingTimes",
    "honorPrograms",
    "sessionDates",
    "subtopic",
    "subtitle",
    "commentsText",
    "comments",
    "instructors",
    "instructorsText",
    "openToText",
    "legendKey",
    "sectionEligibility",
    "finalExam",
    "sessionDatePrintIndicator",
    "examCode",
    "examCodeText",
)

MEETING_FIELDS: Sequence[str] = (
    "meetingDay",
    "startTimeMilitary",
    "endTimeMilitary",
    "pmCode",
    "meetingModeCode",
    "meetingModeDesc",
    "buildingCode",
    "campusLocation",
    "campusName",
    "roomNumber",
)


@dataclass
class FieldStat:
    field: str
    present: int
    total: int
    value_type: str
    example: str

    @property
    def percent(self) -> float:
        if self.total == 0:
            return 0.0
        return self.present / self.total * 100.0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--metadata",
        type=Path,
        default=Path("data/raw/spring-2026-metadata.json"),
        help="Path to metadata json emitted by fetch_soc_samples.py",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Optional path to dump stats as JSON",
    )
    return parser.parse_args()


def load_metadata(path: Path) -> Dict[str, object]:
    with path.open(encoding="utf-8") as fh:
        return json.load(fh)


def repo_root_from_metadata(metadata_path: Path) -> Path:
    resolved = metadata_path.resolve()
    # metadata: <repo>/data/raw/<tag>-metadata.json, so repo root == parents[2]
    return resolved.parents[2]


def load_json_list(path: Path) -> List[Dict[str, object]]:
    with path.open(encoding="utf-8") as fh:
        data = json.load(fh)
    if not isinstance(data, list):
        raise ValueError(f"Expected list in {path}, got {type(data)}")
    return data  # type: ignore[return-value]


def is_present(value: object) -> bool:
    if value is None:
        return False
    if isinstance(value, str):
        return value.strip() != ""
    if isinstance(value, (list, tuple, set, dict)):
        return len(value) > 0
    return True


def guess_type(value: object) -> str:
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, int):
        return "integer"
    if isinstance(value, float):
        return "number"
    if isinstance(value, str):
        return "string"
    if isinstance(value, list):
        inner_type = guess_type(value[0]) if value else "any"
        return f"array<{inner_type}>"
    if isinstance(value, dict):
        return "object"
    return type(value).__name__


def format_example(value: object) -> str:
    if value is None:
        return ""
    if isinstance(value, (str, int, float, bool)):
        text = str(value)
    elif isinstance(value, list):
        text = json.dumps(value[:1], ensure_ascii=False)
    elif isinstance(value, dict):
        sample_keys = list(value.keys())[:3]
        sample = {k: value[k] for k in sample_keys}
        text = json.dumps(sample, ensure_ascii=False)
    else:
        text = repr(value)
    if len(text) > 120:
        return text[:117] + "..."
    return text


def compute_stats(items: Sequence[Dict[str, object]], fields: Sequence[str]) -> List[FieldStat]:
    stats: List[FieldStat] = []
    for field in fields:
        present = 0
        sample_value = None
        for item in items:
            value = item.get(field)
            if is_present(value):
                present += 1
                if sample_value is None:
                    sample_value = value
        value_type = guess_type(sample_value) if sample_value is not None else "unknown"
        example = format_example(sample_value)
        stats.append(FieldStat(field=field, present=present, total=len(items), value_type=value_type, example=example))
    return stats


def compute_meeting_stats(
    sections: Sequence[Dict[str, object]], fields: Sequence[str]
) -> tuple[List[FieldStat], int]:
    meetings: List[Dict[str, object]] = []
    for section in sections:
        mt = section.get("meetingTimes")
        if isinstance(mt, list):
            meetings.extend([m for m in mt if isinstance(m, dict)])
    return compute_stats(meetings, fields), len(meetings)


def to_jsonable(stats: Dict[str, List[FieldStat]], totals: Dict[str, int]) -> Dict[str, object]:
    def serialize(stat: FieldStat) -> Dict[str, object]:
        return {
            "field": stat.field,
            "present": stat.present,
            "total": stat.total,
            "percent": round(stat.percent, 2),
            "type": stat.value_type,
            "example": stat.example,
        }

    return {
        "totals": totals,
        "courses": [serialize(s) for s in stats["courses"]],
        "sections": [serialize(s) for s in stats["sections"]],
        "meeting_times": [serialize(s) for s in stats["meetings"]],
    }


def print_stats(stats: Dict[str, List[FieldStat]], totals: Dict[str, int]) -> None:
    print(
        f"Totals -> courses: {totals['courses']}, sections: {totals['sections']}, "
        f"meetingTimes: {totals['meeting_times']}, unique openSection indexes: {totals['open_section_indexes']}"
    )
    for group, label in (("courses", "Course fields"), ("sections", "Section fields"), ("meetings", "Meeting time fields")):
        print(f"\n{label}")
        for stat in stats[group]:
            print(
                f"  - {stat.field}: {stat.present}/{stat.total} "
                f"({stat.percent:.1f}%) type={stat.value_type} example={stat.example}"
            )


def main() -> int:
    args = parse_args()
    metadata = load_metadata(args.metadata)
    entries = metadata.get("entries", [])
    if not isinstance(entries, list):
        raise ValueError("Metadata entries must be a list")

    repo_root = repo_root_from_metadata(args.metadata)
    course_files = [repo_root / entry["output_path"] for entry in entries if entry.get("endpoint") == "courses"]
    open_section_files = [repo_root / entry["output_path"] for entry in entries if entry.get("endpoint") == "openSections"]

    courses: List[Dict[str, object]] = []
    for path in course_files:
        courses.extend(load_json_list(path))
    sections: List[Dict[str, object]] = []
    for course in courses:
        sec = course.get("sections")
        if isinstance(sec, list):
            sections.extend([s for s in sec if isinstance(s, dict)])

    open_section_indexes = set()
    for path in open_section_files:
        with path.open(encoding="utf-8") as fh:
            data = json.load(fh)
        if isinstance(data, list):
            open_section_indexes.update(str(item) for item in data)

    course_stats = compute_stats(courses, COURSE_FIELDS)
    section_stats = compute_stats(sections, SECTION_FIELDS)
    meeting_stats, meeting_count = compute_meeting_stats(sections, MEETING_FIELDS)

    totals = {
        "courses": len(courses),
        "sections": len(sections),
        "meeting_times": meeting_count,
        "open_section_indexes": len(open_section_indexes),
    }

    stats = {"courses": course_stats, "sections": section_stats, "meetings": meeting_stats}
    print_stats(stats, totals)

    if args.output:
        payload = to_jsonable(stats, totals)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        with args.output.open("w", encoding="utf-8") as fh:
            json.dump(payload, fh, ensure_ascii=False, indent=2)
            fh.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
