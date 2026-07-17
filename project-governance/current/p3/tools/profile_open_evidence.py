#!/usr/bin/env python3
"""Profile both frozen P3 Open rounds entirely offline."""

from __future__ import annotations

import csv
import hashlib
import json
import math
import re
import statistics
from collections import Counter, defaultdict
from datetime import datetime
from pathlib import Path
from typing import Any, Iterable


REPOSITORY_ROOT = Path(__file__).resolve().parents[4]
P3_ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = P3_ROOT / "10a-open-request-manifest.json"
STOP_PATH = P3_ROOT / "10c-open-request-stop-001.json"
LEDGER_PATH = P3_ROOT / "11-open-request-ledger.tsv"
AMENDMENT_PATH = P3_ROOT / "21b-open-round2-resumption-amendment-001.json"
DECISION_PATH = P3_ROOT / "18a-open-review-decision-001.json"
LATENCY_PATH = P3_ROOT / "19-open-cache-and-notification-latency-contract.md"
EVIDENCE_PATH = P3_ROOT / "12-open-evidence-register.tsv"
PROFILE_PATH = P3_ROOT / "13a-open-profile.json"
SCOPE_PATH = P3_ROOT / "13b-open-scope-join-summary.tsv"
CLUSTER_PATH = P3_ROOT / "13c-open-body-clusters.tsv"
TRANSITION_PATH = P3_ROOT / "13d-open-transition-summary.tsv"
SHAPE_PATH = P3_ROOT / "13e-open-value-shape.tsv"


def read_tsv(path: Path) -> list[dict[str, str]]:
    with path.open("r", encoding="utf-8", newline="") as handle:
        return list(csv.DictReader(handle, delimiter="\t"))


def write_tsv(path: Path, fieldnames: list[str], rows: Iterable[dict[str, Any]]) -> None:
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        for row in rows:
            writer.writerow({field: row.get(field, "") for field in fieldnames})


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest().upper()


def parse_utc(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def percentile(values: list[float], probability: float) -> float:
    ordered = sorted(values)
    return ordered[max(0, math.ceil(probability * len(ordered)) - 1)]


def duration_summary(values: list[float]) -> dict[str, float]:
    return {
        "min": round(min(values), 3),
        "median": round(statistics.median(values), 3),
        "mean": round(statistics.mean(values), 3),
        "p95NearestRank": round(percentile(values, 0.95), 3),
        "max": round(max(values), 3),
    }


def load_catalog(scope: dict[str, Any]) -> tuple[Counter[str], set[str], set[str]]:
    raw_path = REPOSITORY_ROOT / scope["catalogRawRelativePath"]
    body = raw_path.read_bytes()
    if hashlib.sha256(body).hexdigest().upper() != scope["catalogBodySha256"]:
        raise RuntimeError(f"Catalog hash mismatch: {scope['catalogRequestId']}")
    payload = json.loads(body)
    raw_counts: Counter[str] = Counter()
    catalog_snapshot_open: set[str] = set()
    for course in payload:
        for section in course.get("sections") or []:
            index = str(section.get("index") or "").strip()
            if not re.fullmatch(r"\d{5}", index):
                raise RuntimeError(f"Catalog section has invalid index: {scope['catalogRequestId']}")
            raw_counts[index] += 1
            if section.get("openStatus") is True:
                catalog_snapshot_open.add(index)
    return raw_counts, set(raw_counts), catalog_snapshot_open


def main() -> None:
    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    stop = json.loads(STOP_PATH.read_text(encoding="utf-8"))
    amendment = json.loads(AMENDMENT_PATH.read_text(encoding="utf-8"))
    ledger = read_tsv(LEDGER_PATH)

    if len(ledger) != 42 or [row["request_id"] for row in ledger] != [item["id"] for item in manifest["requests"]]:
        raise RuntimeError("expected the exact 42-request manifest ledger")
    if any(row["outcome"] != "SUCCESS" or row["http_status"] != "200" for row in ledger):
        raise RuntimeError("all 42 Open attempts must be HTTP 200 successes")
    if stop["completedAttempts"] != 21 or stop["reason"] != "SAME_SCOPE_OPEN_JOIN_CONTRACT_CONFLICT":
        raise RuntimeError("historical Open stop record mismatch")
    frozen_prefix = b"".join(LEDGER_PATH.read_bytes().splitlines(keepends=True)[:22])
    if hashlib.sha256(frozen_prefix).hexdigest().upper() != stop["ledgerSha256AtStop"]:
        raise RuntimeError("historical 21-row Open ledger prefix changed")
    chain = amendment["hashChain"]
    if amendment["status"] != "FROZEN_ROUND_2_RESUMPTION_AFTER_USER_REVIEW":
        raise RuntimeError("Round 2 amendment is not frozen")
    if chain["manifestSha256"] != sha256(MANIFEST_PATH) or chain["historicalStopSha256"] != sha256(STOP_PATH):
        raise RuntimeError("Round 2 amendment manifest/stop chain mismatch")
    if chain["reviewDecisionSha256"] != sha256(DECISION_PATH) or chain["latencyContractSha256"] != sha256(LATENCY_PATH):
        raise RuntimeError("Round 2 amendment decision/latency chain mismatch")
    if amendment["authorizedRequestIds"] != [item["id"] for item in manifest["requests"] if item["round"] == 2]:
        raise RuntimeError("Round 2 amendment allowlist mismatch")

    scopes = manifest["catalogReuse"]["scopes"]
    if len(scopes) != 21:
        raise RuntimeError("Open manifest Catalog scope count mismatch")
    catalog_by_scope: dict[tuple[str, str], tuple[Counter[str], set[str], set[str]]] = {}
    term_union: defaultdict[str, set[str]] = defaultdict(set)
    term_index_scopes: defaultdict[tuple[str, str], set[str]] = defaultdict(set)
    for scope in scopes:
        key = (scope["termId"], scope["campus"])
        catalog = load_catalog(scope)
        catalog_by_scope[key] = catalog
        term_union[scope["termId"]].update(catalog[1])
        for index in catalog[1]:
            term_index_scopes[(scope["termId"], index)].add(scope["campus"])

    scope_rows: list[dict[str, Any]] = []
    evidence_rows: list[dict[str, Any]] = []
    cluster_members: defaultdict[tuple[int, str, str], list[dict[str, str]]] = defaultdict(list)
    value_type_counts: Counter[str] = Counter()
    length_counts: Counter[int] = Counter()
    parsed_by_request: dict[str, set[str]] = {}
    official_open_by_request: dict[str, set[str]] = {}
    total_same_scope_orphans = 0
    total_same_scope_matches = 0

    for evidence_ordinal, row in enumerate(ledger, start=1):
        raw_path = REPOSITORY_ROOT / row["raw_relative_path"]
        body = raw_path.read_bytes()
        actual_hash = hashlib.sha256(body).hexdigest().upper()
        if actual_hash != row["body_sha256"]:
            raise RuntimeError(f"Open hash mismatch: {row['request_id']}")
        values = json.loads(body)
        if not isinstance(values, list):
            raise RuntimeError(f"Open root not array: {row['request_id']}")
        if any(not isinstance(value, str) or re.fullmatch(r"\d{5}", value) is None for value in values):
            raise RuntimeError(f"Open values are not all five-digit strings: {row['request_id']}")
        if len(values) != len(set(values)):
            raise RuntimeError(f"Open response contains duplicate raw values: {row['request_id']}")

        value_type_counts["string"] += len(values)
        length_counts.update(len(value) for value in values)
        string_set = set(values)
        parsed_by_request[row["request_id"]] = string_set

        scope_key = (row["term_id"], row["campus"])
        raw_catalog_counts, catalog_indexes, catalog_snapshot_open = catalog_by_scope[scope_key]
        official_open = string_set & catalog_indexes
        official_closed = catalog_indexes - string_set
        official_orphans = string_set - catalog_indexes
        official_open_by_request[row["request_id"]] = official_open
        unsafe_empty = not string_set and bool(catalog_indexes)
        same_scope_duplicate_catalog_keys = sum(1 for index in official_open if raw_catalog_counts[index] > 1)
        term_matches = string_set & term_union[row["term_id"]]
        term_orphans = string_set - term_union[row["term_id"]]
        term_ambiguous = sum(1 for index in term_matches if len(term_index_scopes[(row["term_id"], index)]) > 1)
        total_same_scope_orphans += len(official_orphans)
        total_same_scope_matches += len(official_open)

        scope_rows.append(
            {
                "request_id": row["request_id"],
                "round": row["round"],
                "term_id": row["term_id"],
                "campus": row["campus"],
                "open_rows": len(values),
                "unique_open_strings": len(string_set),
                "catalog_unique_sections": len(catalog_indexes),
                "official_open_matches": len(official_open),
                "official_closed_by_absence": len(official_closed) if not unsafe_empty else "NOT_APPLIED",
                "orphan_audit_count": len(official_orphans),
                "orphan_audit_rate": round(len(official_orphans) / len(string_set), 8) if string_set else 0,
                "raw_duplicate_catalog_keys_matched": same_scope_duplicate_catalog_keys,
                "term_union_matches": len(term_matches),
                "term_union_orphans": len(term_orphans),
                "term_union_multi_scope_indexes": term_ambiguous,
                "catalog_snapshot_open_disagrees_closed": len(official_open - catalog_snapshot_open),
                "catalog_snapshot_open_missing_from_live": len(catalog_snapshot_open - string_set),
                "body_sha256": actual_hash,
                "etag": row["etag"],
                "cache_control": row["cache_control"],
                "unsafe_empty": str(unsafe_empty).lower(),
                "official_join_result": "UNSAFE_EMPTY_KEEP_LAST_KNOWN_GOOD" if unsafe_empty else "PASS_INTERSECTION_ORPHANS_AUDITED",
            }
        )
        evidence_rows.append(
            {
                "evidence_id": f"OPEN-E{evidence_ordinal:03d}",
                "request_id": row["request_id"],
                "round": row["round"],
                "term_id": row["term_id"],
                "campus": row["campus"],
                "raw_relative_path": row["raw_relative_path"],
                "body_sha256": actual_hash,
                "observed_at_utc": row["completed_at_utc"],
                "observation_strength": "PAIRED_TWO_ROUND_OBSERVATION",
                "claims_supported": "OPEN_SHAPE|OFFICIAL_INTERSECTION|ORPHAN_AUDIT|TRANSITION|CACHE_HEADERS",
                "limitations": "low_volume_serial_evidence;not_an_upstream_SLA;not_sustained_capacity_proof",
            }
        )
        cluster_members[(int(row["round"]), row["term_id"], actual_hash)].append(row)

    cluster_rows = [
        {
            "round": round_number,
            "term_id": term_id,
            "body_sha256": body_hash,
            "open_rows": members[0]["top_level_rows"],
            "campus_count": len(members),
            "campuses": ",".join(member["campus"] for member in members),
            "request_ids": ",".join(member["request_id"] for member in members),
            "cache_control_values": ",".join(sorted({member["cache_control"] for member in members})),
        }
        for (round_number, term_id, body_hash), members in sorted(cluster_members.items())
    ]

    transition_rows: list[dict[str, Any]] = []
    for ordinal, scope in enumerate(scopes, start=1):
        r1 = ledger[ordinal - 1]
        r2 = ledger[21 + ordinal - 1]
        raw_r1, raw_r2 = parsed_by_request[r1["request_id"]], parsed_by_request[r2["request_id"]]
        effective_r1, effective_r2 = official_open_by_request[r1["request_id"]], official_open_by_request[r2["request_id"]]
        transition_rows.append(
            {
                "term_id": scope["termId"],
                "campus": scope["campus"],
                "round_1_request_id": r1["request_id"],
                "round_2_request_id": r2["request_id"],
                "raw_added_count": len(raw_r2 - raw_r1),
                "raw_removed_count": len(raw_r1 - raw_r2),
                "effective_open_added_count": len(effective_r2 - effective_r1),
                "effective_open_removed_count": len(effective_r1 - effective_r2),
                "body_changed": str(r1["body_sha256"] != r2["body_sha256"]).lower(),
                "etag_changed": str(r1["etag"] != r2["etag"]).lower(),
                "observation_gap_seconds": round((parse_utc(r2["completed_at_utc"]) - parse_utc(r1["completed_at_utc"])).total_seconds(), 3),
            }
        )

    round_spans: list[dict[str, Any]] = []
    for round_number in (1, 2):
        rows = [row for row in ledger if int(row["round"]) == round_number]
        round_spans.append(
            {
                "round": round_number,
                "firstAttemptStartedAtUtc": rows[0]["started_at_utc"],
                "lastAttemptCompletedAtUtc": rows[-1]["completed_at_utc"],
                "serialEvidenceSpanSeconds": round((parse_utc(rows[-1]["completed_at_utc"]) - parse_utc(rows[0]["started_at_utc"])).total_seconds(), 3),
            }
        )

    durations = [float(row["duration_ms"]) for row in ledger]
    duration_by_round = {
        str(round_number): duration_summary([float(row["duration_ms"]) for row in ledger if int(row["round"]) == round_number])
        for round_number in (1, 2)
    }
    shape_rows = [
        {"metric": "root_string_values", "count": value_type_counts["string"], "status": "OBSERVED_TWO_ROUNDS"},
        {"metric": "root_non_string_values", "count": 0, "status": "NOT_OBSERVED"},
        {"metric": "string_length_5", "count": length_counts[5], "status": "OBSERVED_TWO_ROUNDS"},
        {"metric": "string_length_not_5", "count": sum(length_counts.values()) - length_counts[5], "status": "NOT_OBSERVED"},
        {"metric": "raw_duplicate_values", "count": sum(int(row["duplicate_raw_values"]) for row in ledger), "status": "NOT_OBSERVED"},
        {"metric": "official_matches_summed_non_distinct", "count": total_same_scope_matches, "status": "OBSERVED_TWO_ROUNDS"},
        {"metric": "orphan_audit_values_summed_non_distinct", "count": total_same_scope_orphans, "status": "EXPECTED_SUPERSET_BEHAVIOR"},
    ]

    profile = {
        "schemaVersion": 2,
        "status": "ROUND_2_COMPLETE_OFFICIAL_JOIN_VALIDATED",
        "completedRounds": 2,
        "round2": "COMPLETE",
        "successfulAttempts": len(ledger),
        "valueTypeCounts": dict(value_type_counts),
        "stringLengthCounts": {str(key): value for key, value in sorted(length_counts.items())},
        "cacheControlCounts": dict(Counter(row["cache_control"] for row in ledger)),
        "headerPresence": {
            "date": sum(bool(row["response_date"]) for row in ledger),
            "etag": sum(bool(row["etag"]) for row in ledger),
            "age": sum(bool(row["age"]) for row in ledger),
            "lastModified": sum(bool(row["last_modified"]) for row in ledger),
        },
        "requestDurationMs": {"all": duration_summary(durations), "byRound": duration_by_round},
        "roundAcquisitionSpans": round_spans,
        "officialJoinContract": {
            "name": "RUTGERS_OFFICIAL_MERGED_SET_INTERSECTION",
            "scopeKey": "term_campus",
            "externalSectionKey": "term_campus_index",
            "orphanHandling": "AUDIT_AND_IGNORE_NEVER_CREATE_SECTION",
            "absenceToClosedRequires": "COMPLETE_SAFE_NON_AMBIGUOUS_SCOPE_BATCH",
            "individualEmptyWithNonemptyCatalog": "UNSAFE_EMPTY_KEEP_LAST_KNOWN_GOOD",
            "partialOrInvalidBatch": "KEEP_LAST_KNOWN_GOOD",
        },
        "scopeJoinSummary": scope_rows,
        "bodyClusters": cluster_rows,
        "transitionSummary": transition_rows,
        "totals": {
            "officialMatchesSummedNonDistinct": total_same_scope_matches,
            "orphanAuditValuesSummedNonDistinct": total_same_scope_orphans,
            "officialJoinPassObservations": sum(row["official_join_result"].startswith("PASS") for row in scope_rows),
            "unsafeEmptyObservations": sum(row["unsafe_empty"] == "true" for row in scope_rows),
            "emptyOpenObservations": sum(int(row["open_rows"]) == 0 for row in scope_rows),
            "changedRawScopePairs": sum(row["body_changed"] == "true" for row in transition_rows),
            "changedEffectiveScopePairs": sum(int(row["effective_open_added_count"]) + int(row["effective_open_removed_count"]) > 0 for row in transition_rows),
            "rawAddedSummedNonDistinct": sum(int(row["raw_added_count"]) for row in transition_rows),
            "rawRemovedSummedNonDistinct": sum(int(row["raw_removed_count"]) for row in transition_rows),
            "effectiveAddedSummedNonDistinct": sum(int(row["effective_open_added_count"]) for row in transition_rows),
            "effectiveRemovedSummedNonDistinct": sum(int(row["effective_open_removed_count"]) for row in transition_rows),
        },
        "limitations": [
            "The 42 low-volume serial observations validate shape and the approved intersection algorithm, not sustained production capacity.",
            "Cache-Control max-age=30 is response freshness metadata, not a Rutgers business-update SLA.",
            "ETag is opaque and per-URI; it is not compared across campuses, and observed changes do not establish seat-change time.",
            "Summer Catalog evidence intentionally covers six main/online scopes rather than all fifteen campuses.",
            "No natural HTTP failure was induced; partial, timeout, unsafe-empty, coalescing, and missed-tick behavior require deterministic fake-upstream tests.",
        ],
    }
    PROFILE_PATH.write_text(json.dumps(profile, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    write_tsv(EVIDENCE_PATH, ["evidence_id", "request_id", "round", "term_id", "campus", "raw_relative_path", "body_sha256", "observed_at_utc", "observation_strength", "claims_supported", "limitations"], evidence_rows)
    write_tsv(SCOPE_PATH, list(scope_rows[0].keys()), scope_rows)
    write_tsv(CLUSTER_PATH, ["round", "term_id", "body_sha256", "open_rows", "campus_count", "campuses", "request_ids", "cache_control_values"], cluster_rows)
    write_tsv(TRANSITION_PATH, list(transition_rows[0].keys()), transition_rows)
    write_tsv(SHAPE_PATH, ["metric", "count", "status"], shape_rows)
    print(json.dumps(profile["totals"], indent=2))
    print(f"evidence_rows={len(evidence_rows)} body_clusters={len(cluster_rows)} transition_rows={len(transition_rows)}")


if __name__ == "__main__":
    main()
