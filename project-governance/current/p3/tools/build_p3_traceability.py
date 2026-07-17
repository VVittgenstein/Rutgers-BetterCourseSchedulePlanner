#!/usr/bin/env python3
"""Build the deterministic P2 -> P3 traceability matrix.

The matrix is intentionally generated from the current, approved P2 artifacts
instead of a hand-maintained list.  It records every P2 file, semantic row,
capability, ONLY closure, reuse decision, and filter contract.  P3 Catalog
evidence and the shared P3 Open review boundary are separate columns.  Open
rows point to the P3 stop evidence so they cannot be mistaken for a P4
handoff or for a completed implementation contract.
"""

from __future__ import annotations

import csv
import hashlib
import re
from collections import OrderedDict
from pathlib import Path
from typing import Iterable


REPOSITORY_ROOT = Path(__file__).resolve().parents[4]
P2_ROOT = REPOSITORY_ROOT / "project-governance" / "current" / "p2"
P3_ROOT = REPOSITORY_ROOT / "project-governance" / "current" / "p3"
OUTPUT_PATH = P3_ROOT / "08-p3-traceability-matrix.tsv"

EXPECTED_COUNTS = OrderedDict(
    [
        ("CAP", 28),
        ("ONLY", 24),
        ("REUSE", 59),
        ("SEM", 93),
        ("FLT", 22),
        ("FILE", 158),
    ]
)

SOURCE_SPECS = OrderedDict(
    [
        (
            "SEM",
            (
                P2_ROOT / "02-file-semantic-matrix.md",
                re.compile(r"^\|\s*(SEM-[A-Z]+-\d+)\b"),
            ),
        ),
        (
            "CAP",
            (
                P2_ROOT / "03-capability-all-matrix.md",
                re.compile(r"^\|\s*(CAP-(?:I)?\d+)\b"),
            ),
        ),
        (
            "ONLY",
            (
                P2_ROOT / "04-only-closure-matrix.md",
                re.compile(r"^\|\s*(ONLY-\d+)\b"),
            ),
        ),
        (
            "REUSE",
            (
                P2_ROOT / "05-reuse-and-port-matrix.md",
                re.compile(r"^\|\s*(R-[A-Z]+-\d+)\b"),
            ),
        ),
        (
            "FLT",
            (
                P2_ROOT / "06-filter-section-watch-contract.md",
                re.compile(r"^\|\s*(FLT-[CS]\d{2}[a-z]?)\b"),
            ),
        ),
    ]
)

CATALOG_CAPABILITIES = {
    "CAP-003",
    "CAP-004",
    "CAP-005",
    "CAP-006",
    "CAP-007",
    "CAP-013",
    "CAP-016",
    "CAP-I01",
    "CAP-I02",
    "CAP-I03",
    "CAP-I04",
    "CAP-I05",
    "CAP-I06",
    "CAP-I07",
    "CAP-I08",
}

OPEN_CAPABILITIES = {
    "CAP-003",
    "CAP-008",
    "CAP-009",
    "CAP-010",
    "CAP-011",
    "CAP-012",
    "CAP-013",
    "CAP-019",
    "CAP-I04",
    "CAP-I08",
}

OPEN_ONLY = {
    "ONLY-002",
    "ONLY-003",
    "ONLY-004",
    "ONLY-011",
    "ONLY-014",
    "ONLY-015",
    "ONLY-022",
}

OPEN_REUSE = {
    "R-UI-014",
    "R-UI-015",
    "R-BE-011",
    "R-BE-012",
    "R-BE-013",
    "R-DAT-008",
    "R-DAT-009",
    "R-DAT-011",
    "R-DAT-012",
}

CATALOG_TERMS = (
    "catalog",
    "course",
    "section",
    "filter",
    "query",
    "rutgers",
    "soc_",
    "normaliz",
    "ingest",
    "sqlite",
    "schema",
    "migration",
    "dictionary",
    "delivery",
    "meeting",
    "campus",
    "subject",
    "fts",
    "refresh",
)

OPEN_TERMS = (
    "opensections",
    "open_sections",
    "open status",
    "open observation",
    "openepisode",
    "poller",
    "live watch",
    "websocket",
    "audible",
    "continuous",
    "one_shot",
    "notification claim",
)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest().upper()


def compact(value: str, limit: int = 260) -> str:
    value = re.sub(r"\s+", " ", value.replace("\t", " ")).strip()
    return value if len(value) <= limit else value[: limit - 1] + "…"


def first_description_cell(line: str) -> str:
    cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
    return compact(cells[1] if len(cells) > 1 else cells[0])


def parse_markdown_rows(kind: str, path: Path, pattern: re.Pattern[str]) -> list[dict[str, str]]:
    source_hash = sha256(path)
    rows: list[dict[str, str]] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        match = pattern.match(line)
        if not match:
            continue
        source_id = match.group(1)
        rows.append(
            {
                "source_kind": kind,
                "source_id": source_id,
                "source_path": path.relative_to(REPOSITORY_ROOT).as_posix(),
                "source_anchor": f"line:{line_number}",
                "source_sha256": source_hash,
                "source_summary": first_description_cell(line),
                "source_text": compact(line, 2000),
                "p2_decision": "P2_APPROVED_BASELINE",
            }
        )
    return rows


def parse_file_rows() -> list[dict[str, str]]:
    path = P2_ROOT / "01-file-universe.tsv"
    source_hash = sha256(path)
    rows: list[dict[str, str]] = []
    # The frozen P2 TSV is emitted by PowerShell with a UTF-8 BOM.
    with path.open("r", encoding="utf-8-sig", newline="") as handle:
        for row in csv.DictReader(handle, delimiter="\t"):
            source_id = row["row_id"]
            decision = "/".join((row["capability"], row["disposition"], row["ownership"]))
            source_text = " ".join(
                (
                    row["path"],
                    row["type"],
                    row["runtime_package_reachability"],
                    decision,
                    row["next_action"],
                )
            )
            rows.append(
                {
                    "source_kind": "FILE",
                    "source_id": source_id,
                    "source_path": path.relative_to(REPOSITORY_ROOT).as_posix(),
                    "source_anchor": f"row:{source_id}",
                    "source_sha256": source_hash,
                    "source_summary": compact(row["path"]),
                    "source_text": compact(source_text, 2000),
                    "p2_decision": decision,
                }
            )
    return rows


def contains_any(text: str, terms: Iterable[str]) -> bool:
    lowered = text.casefold()
    return any(term.casefold() in lowered for term in terms)


def classify(row: dict[str, str]) -> tuple[str, str, str, str, str]:
    kind = row["source_kind"]
    source_id = row["source_id"]
    text = f"{row['source_summary']} {row['source_text']}"

    is_catalog = (
        kind == "FLT"
        or source_id in CATALOG_CAPABILITIES
        or (kind in {"FILE", "SEM", "REUSE"} and contains_any(text, CATALOG_TERMS))
    )
    is_open = (
        source_id in OPEN_CAPABILITIES
        or source_id in OPEN_ONLY
        or source_id in OPEN_REUSE
        or source_id == "FLT-S03"
        or (kind in {"FILE", "SEM"} and contains_any(text, OPEN_TERMS))
    )

    if kind == "ONLY":
        catalog_class = "ONLY_CLOSURE"
        plan_target = "P3-ONLY-CLOSURE"
    elif kind == "FLT":
        catalog_class = "CATALOG_EVIDENCE_AND_FILTER_PLAN"
        plan_target = "P3-FILTER-QUERY"
    elif source_id.startswith("CAP-I"):
        catalog_class = "CATALOG_EVIDENCE_AND_PLAN" if is_catalog else "INTERNAL_QUALITY_PLAN"
        plan_target = "P3-INTERNAL-QUALITY"
    elif is_catalog:
        catalog_class = "CATALOG_EVIDENCE_AND_PLAN"
        plan_target = "P3-CATALOG-DATA"
    else:
        catalog_class = "LOCAL_IMPLEMENTATION_PLAN"
        plan_target = "P3-LOCAL-IMPLEMENTATION"

    refs = {"00-p3-charter-and-preflight.md"}
    lowered = text.casefold()
    if is_catalog:
        refs.update(
            {
                "01a-catalog-request-manifest.json",
                "02-catalog-request-ledger.tsv",
                "03-catalog-evidence-register.tsv",
                "04a-catalog-profile.json",
                "04b-catalog-scope-summary.tsv",
                "04c-catalog-field-profile.tsv",
                "06b-filter-source-profile.tsv",
            }
        )
    if kind == "FLT" or "delivery" in lowered or "meetingmode" in lowered:
        refs.add("05a-delivery-raw-values.tsv")
    if kind == "FLT" or contains_any(lowered, ("meeting", "availability", "time", "day")):
        refs.add("05b-meeting-raw-values.tsv")
    if kind == "FLT" or contains_any(lowered, ("section", "index", "identity", "composite key")):
        refs.add("06a-section-identity-summary.tsv")

    open_boundary = "RESOLVED_P3_SHARED_OPEN_CONTRACT" if is_open else "NOT_APPLICABLE"
    if is_open:
        refs.update(
            {
                "14-open-profile-join-and-error-evidence.md",
                "15-poller-qps-latency-and-safety-model.md",
                "16-shared-open-contract-review-options.md",
                "17-p3-open-review-stop-gate.md",
                "18-open-review-decision-001.md",
                "19-open-cache-and-notification-latency-contract.md",
                "20-p3-open-decision-gate.md",
                "21b-open-round2-resumption-amendment-001.json",
                "22b-open-round2-completion.json",
                "23a-shared-open-final-contract.md",
                "23b-shared-open-final-contract.json",
            }
        )
        plan_target = f"{plan_target}|P3-SHARED-OPEN-CONTRACT"
        trace_status = "MAPPED_P3_OPEN_FROZEN"
    elif kind == "ONLY":
        trace_status = "MAPPED_ONLY_GUARD"
    elif is_catalog:
        trace_status = "MAPPED_CATALOG"
    else:
        trace_status = "MAPPED_P3_PLAN"

    return catalog_class, ";".join(sorted(refs)), plan_target, open_boundary, trace_status


def validate_source_rows(rows: list[dict[str, str]]) -> None:
    by_kind: dict[str, list[dict[str, str]]] = {kind: [] for kind in EXPECTED_COUNTS}
    for row in rows:
        by_kind[row["source_kind"]].append(row)

    failures: list[str] = []
    for kind, expected in EXPECTED_COUNTS.items():
        actual = len(by_kind[kind])
        if actual != expected:
            failures.append(f"{kind} count expected {expected}, got {actual}")
        ids = [row["source_id"] for row in by_kind[kind]]
        if len(ids) != len(set(ids)):
            failures.append(f"{kind} contains duplicate source IDs")
    if failures:
        raise RuntimeError("; ".join(failures))


def main() -> None:
    rows: list[dict[str, str]] = []
    rows.extend(parse_file_rows())
    for kind, (path, pattern) in SOURCE_SPECS.items():
        rows.extend(parse_markdown_rows(kind, path, pattern))
    validate_source_rows(rows)

    order = {kind: position for position, kind in enumerate(EXPECTED_COUNTS)}
    rows.sort(key=lambda row: (order[row["source_kind"]], row["source_id"]))

    output_rows: list[dict[str, str]] = []
    for row in rows:
        catalog_class, refs, target, open_boundary, status = classify(row)
        output_rows.append(
            {
                "trace_id": f"P3T-{row['source_kind']}-{row['source_id']}",
                "source_kind": row["source_kind"],
                "source_id": row["source_id"],
                "source_path": row["source_path"],
                "source_anchor": row["source_anchor"],
                "source_sha256": row["source_sha256"],
                "source_summary": row["source_summary"],
                "p2_decision": row["p2_decision"],
                "p3_catalog_class": catalog_class,
                "p3_evidence_refs": refs,
                "p3_plan_target": target,
                "open_boundary": open_boundary,
                "trace_status": status,
            }
        )

    fieldnames = list(output_rows[0])
    temporary = OUTPUT_PATH.with_suffix(OUTPUT_PATH.suffix + ".tmp")
    with temporary.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        writer.writerows(output_rows)
    temporary.replace(OUTPUT_PATH)

    print(f"traceability={OUTPUT_PATH.relative_to(REPOSITORY_ROOT).as_posix()}")
    for kind, expected in EXPECTED_COUNTS.items():
        print(f"{kind.lower()}_rows={expected}")
    print(f"total_rows={len(output_rows)}")
    print(
        "open_contract_rows="
        f"{sum(row['open_boundary'] == 'RESOLVED_P3_SHARED_OPEN_CONTRACT' for row in output_rows)}"
    )


if __name__ == "__main__":
    main()
