#!/usr/bin/env python3
"""Independent offline checks for P3 Catalog evidence, notebook, and report."""

from __future__ import annotations

import csv
import hashlib
import json
from collections import Counter, defaultdict
from datetime import datetime
from pathlib import Path
from typing import Any


REPOSITORY_ROOT = Path(__file__).resolve().parents[4]
P3_ROOT = Path(__file__).resolve().parents[1]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def semantic_normalize(value: Any) -> Any:
    if isinstance(value, dict):
        return {
            key: semantic_normalize(child)
            for key, child in sorted(value.items())
            if key not in {"commentsText", "instructorsText", "crossListedSectionsText"}
        }
    if isinstance(value, list):
        normalized = [semantic_normalize(child) for child in value]
        return sorted(
            normalized,
            key=lambda child: json.dumps(child, ensure_ascii=False, sort_keys=True, separators=(",", ":")),
        )
    if isinstance(value, str):
        return value.strip()
    return value


def semantic_hash(section: dict[str, Any]) -> str:
    payload = json.dumps(
        semantic_normalize(section),
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(payload).hexdigest().upper()


def read_tsv(path: Path) -> list[dict[str, str]]:
    with path.open("r", encoding="utf-8", newline="") as handle:
        return list(csv.DictReader(handle, delimiter="\t"))


def main() -> None:
    ledger = read_tsv(P3_ROOT / "02-catalog-request-ledger.tsv")
    successes = [row for row in ledger if row["outcome"] == "SUCCESS"]
    failures = [row for row in ledger if row["outcome"] == "FAILURE"]
    require(len(ledger) == 22, f"expected 22 attempts, found {len(ledger)}")
    require(len(successes) == 21, f"expected 21 successes, found {len(successes)}")
    require(len(failures) == 1, f"expected one retained failure, found {len(failures)}")
    require(failures[0]["error_class"] == "CLIENT_GZIP_NOT_DECODED", "unexpected failure class")

    register = read_tsv(P3_ROOT / "03-catalog-evidence-register.tsv")
    require(len(register) == 21, f"expected 21 evidence rows, found {len(register)}")
    require(len({row["evidence_id"] for row in register}) == 21, "duplicate evidence ids")
    by_request = {row["request_id"]: row for row in register}
    require(set(by_request) == {row["request_id"] for row in successes}, "ledger/register request mismatch")

    counts: Counter[str] = Counter()
    section_groups: defaultdict[tuple[str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in successes:
        raw_path = REPOSITORY_ROOT / row["raw_relative_path"]
        require(raw_path.is_file(), f"missing raw evidence: {row['raw_relative_path']}")
        body = raw_path.read_bytes()
        actual_hash = hashlib.sha256(body).hexdigest().upper()
        require(actual_hash == row["body_sha256"], f"raw hash mismatch: {row['request_id']}")
        require(by_request[row["request_id"]]["body_sha256"] == actual_hash, "register hash mismatch")
        courses = json.loads(body)
        require(isinstance(courses, list), f"top-level body is not array: {row['request_id']}")
        require(len(courses) == int(row["top_level_rows"]), f"top-level count mismatch: {row['request_id']}")
        counts["courses"] += len(courses)
        for course in courses:
            sections = course.get("sections") or []
            require(isinstance(sections, list), "course.sections is not array")
            counts["sections"] += len(sections)
            for section in sections:
                counts["meetings"] += len(section.get("meetingTimes") or [])
                counts["instructor_assignments"] += len(section.get("instructors") or [])
                if section.get("instructors"):
                    counts["sections_with_instructor"] += 1
                index = str(section.get("index") or "").strip()
                require(index, f"missing section index in {row['request_id']}")
                section_groups[(row["term_id"], row["campus"], index)].append(section)

    profile = json.loads((P3_ROOT / "04a-catalog-profile.json").read_text(encoding="utf-8"))
    require(dict(counts) == profile["counts"], f"profile count mismatch: {dict(counts)} != {profile['counts']}")
    duplicates = [sections for sections in section_groups.values() if len(sections) > 1]
    require(len(duplicates) == 18, f"expected 18 duplicate keys, found {len(duplicates)}")
    conflicts = sum(1 for sections in duplicates if len({semantic_hash(section) for section in sections}) > 1)
    require(conflicts == 0, f"found {conflicts} contradictory duplicate groups")

    open_ledger = read_tsv(P3_ROOT / "11-open-request-ledger.tsv")
    require(len(open_ledger) == 42, f"expected 42 Open attempts, found {len(open_ledger)}")
    require(sum(row["round"] == "1" for row in open_ledger) == 21, "Open Round 1 count drifted")
    require(sum(row["round"] == "2" for row in open_ledger) == 21, "Open Round 2 count drifted")
    require(all(row["outcome"] == "SUCCESS" and row["http_status"] == "200" for row in open_ledger), "Open ledger contains a non-success")
    for row in open_ledger:
        raw_path = REPOSITORY_ROOT / row["raw_relative_path"]
        require(raw_path.is_file(), f"missing Open raw evidence: {row['raw_relative_path']}")
        require(hashlib.sha256(raw_path.read_bytes()).hexdigest().upper() == row["body_sha256"], f"Open raw hash mismatch: {row['request_id']}")

    open_profile = json.loads((P3_ROOT / "13a-open-profile.json").read_text(encoding="utf-8"))
    open_completion = json.loads((P3_ROOT / "22b-open-round2-completion.json").read_text(encoding="utf-8"))
    open_decision = json.loads((P3_ROOT / "18a-open-review-decision-001.json").read_text(encoding="utf-8"))
    open_contract = json.loads((P3_ROOT / "23b-shared-open-final-contract.json").read_text(encoding="utf-8"))
    require(open_profile["status"] == "ROUND_2_COMPLETE_OFFICIAL_JOIN_VALIDATED", "Open profile is not complete")
    require(open_profile["round2"] == "COMPLETE" and open_profile["successfulAttempts"] == 42, "Open two-round totals drifted")
    require(open_profile["valueTypeCounts"]["string"] == 302125, "Open string total drifted")
    require(open_profile["stringLengthCounts"]["5"] == 302125, "Open five-digit total drifted")
    require(open_profile["totals"]["officialJoinPassObservations"] == 42, "Open official-join pass total drifted")
    require(open_profile["totals"]["changedRawScopePairs"] == 14, "Open raw transition total drifted")
    require(open_profile["totals"]["changedEffectiveScopePairs"] == 3, "Open effective transition total drifted")
    require(open_completion["status"] == "FROZEN_ROUND_2_COMPLETE", "Open completion record is not frozen")
    require(open_completion["attempts"] == {"planned": 42, "completed": 42, "successful": 42, "failed": 0, "round1": 21, "round2": 21}, "Open completion attempts drifted")
    require(open_completion["furtherP3EvidenceRequestsAuthorized"] == 0, "new P3 network requests were authorized")
    require(open_decision["proposedClockMapping"]["status"] == "APPROVED_BY_USER", "two-clock mapping is not approved")
    require(open_contract["status"] in {"P3_SHARED_OPEN_CONTRACT_FREEZE_CANDIDATE", "FROZEN_P3_SHARED_OPEN_CONTRACT"}, "shared Open contract has an invalid status")

    notebook = json.loads((P3_ROOT / "notebooks" / "p3_catalog_data_quality.ipynb").read_text(encoding="utf-8"))
    code_cells = [cell for cell in notebook["cells"] if cell.get("cell_type") == "code"]
    require(len(code_cells) == 7, f"expected 7 code cells, found {len(code_cells)}")
    require(all(cell.get("execution_count") for cell in code_cells), "notebook has an unexecuted code cell")
    require(all(cell.get("outputs") for cell in code_cells), "notebook code cell missing captured output")
    notebook_text = json.dumps(notebook, ensure_ascii=False)
    for stale in ("P3 remains Review-blocked", "waiting for the exact 3/10/30", "resume the cancelled Open round 2"):
        require(stale not in notebook_text, f"notebook contains stale text: {stale}")
    require("302125 five-digit values" in notebook_text, "notebook lacks the two-round Open validation output")

    artifact = json.loads(
        (P3_ROOT / "reports" / "p3-catalog-data-quality-artifact.json").read_text(encoding="utf-8")
    )
    scope_rows = artifact["snapshot"]["datasets"]["scope_summary"]
    require(len(scope_rows) == 21, "report scope dataset is not complete")
    require(sum(row["sections"] for row in scope_rows) == counts["sections"], "report section total mismatch")
    require(artifact["snapshot"]["status"] == "ready", "report snapshot must be ready")
    require(len(artifact["snapshot"]["datasets"]["open_transitions"]) == 21, "report Open transition dataset is incomplete")
    latency_rows = artifact["snapshot"]["datasets"]["open_latency_scenarios"]
    require(len(latency_rows) == 3, "report latency scenarios are incomplete")
    require(all(abs(float(row["observed_single_request_p95_seconds"]) - 1.501) < 0.0001 for row in latency_rows), "report Open p95 is stale")
    completed_at = datetime.fromisoformat(open_completion["completedAtUtc"].replace("Z", "+00:00"))
    generated_at = datetime.fromisoformat(artifact["snapshot"]["generatedAt"].replace("Z", "+00:00"))
    require(generated_at >= completed_at, "report predates Open Round 2 completion")
    artifact_text = json.dumps(artifact, ensure_ascii=False)
    for stale in ("P3 remains Review-blocked", "Confirm whether 10 seconds", "resuming the cancelled Open round 2", "RATE_CLOCK_PENDING"):
        require(stale not in artifact_text, f"report contains stale text: {stale}")
    require("42 successful observations" in artifact_text, "report lacks the two-round Open finding")
    source_paths = {source.get("path") for source in artifact["manifest"]["sources"]}
    for required_source in (
        "project-governance/current/p3/13a-open-profile.json",
        "project-governance/current/p3/13d-open-transition-summary.tsv",
        "project-governance/current/p3/18a-open-review-decision-001.json",
        "project-governance/current/p3/22b-open-round2-completion.json",
        "project-governance/current/p3/23b-shared-open-final-contract.json",
    ):
        require(required_source in source_paths, f"report missing Open source: {required_source}")

    print("P3_CATALOG_ANALYSIS_VALIDATION=PASS")
    print(f"attempts={len(ledger)} successes={len(successes)} failures={len(failures)}")
    print("counts=" + json.dumps(dict(counts), sort_keys=True))
    print(f"duplicate_keys={len(duplicates)} semantic_conflicts={conflicts}")
    print(f"open_attempts={len(open_ledger)} open_join_pass={open_profile['totals']['officialJoinPassObservations']}")
    print(f"notebook_code_cells={len(code_cells)} report_scope_rows={len(scope_rows)} report_open_pairs={len(artifact['snapshot']['datasets']['open_transitions'])}")


if __name__ == "__main__":
    main()
