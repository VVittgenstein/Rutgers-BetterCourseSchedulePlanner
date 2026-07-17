#!/usr/bin/env python3
"""Build a bounded Data Analytics report artifact from frozen P3 outputs."""

from __future__ import annotations

import csv
import json
import math
import sqlite3
from datetime import datetime, timezone
from pathlib import Path


P3_ROOT = Path(__file__).resolve().parents[1]
PROFILE_PATH = P3_ROOT / "04a-catalog-profile.json"
OPEN_PROFILE_PATH = P3_ROOT / "13a-open-profile.json"
OUTPUT_PATH = P3_ROOT / "reports" / "p3-catalog-data-quality-artifact.json"


def main() -> None:
    profile = json.loads(PROFILE_PATH.read_text(encoding="utf-8"))
    open_profile = json.loads(OPEN_PROFILE_PATH.read_text(encoding="utf-8"))
    generated_at = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")

    scope_inputs = []
    for row in profile["scopeSummary"]:
        scope_inputs.append(
            {
                "scope": f'{row["term_id"]}/{row["campus"]}',
                "term_id": row["term_id"],
                "campus": row["campus"],
                "courses": row["courses"],
                "sections": row["sections"],
                "meetings": row["meetings"],
                "instructor_assignments": row["instructor_assignments"],
                "decoded_bytes": row["decoded_bytes"],
                "duration_ms": row["duration_ms"],
                "duplicate_composite_keys": row["duplicate_composite_keys"],
            }
        )

    identity_inputs = [
        {
            "metric": row["metric"],
            "count": row["count"],
            "severity": row["severity"],
        }
        for row in profile["identity"]["summary"]
    ]

    scope_sql = (
        "SELECT scope, term_id, campus, courses, sections, meetings, "
        "instructor_assignments, decoded_bytes, duration_ms, duplicate_composite_keys "
        "FROM p3_catalog_scope_summary ORDER BY sections DESC, scope ASC"
    )
    identity_sql = (
        "SELECT metric, count, severity FROM p3_catalog_identity_summary "
        "ORDER BY count DESC, metric ASC"
    )
    connection = sqlite3.connect(":memory:")
    connection.execute(
        "CREATE TABLE p3_catalog_scope_summary ("
        "scope TEXT, term_id TEXT, campus TEXT, courses INTEGER, sections INTEGER, meetings INTEGER, "
        "instructor_assignments INTEGER, decoded_bytes INTEGER, duration_ms REAL, duplicate_composite_keys INTEGER)"
    )
    connection.executemany(
        "INSERT INTO p3_catalog_scope_summary VALUES " + "(" + ",".join(["?"] * 10) + ")",
        [tuple(row.values()) for row in scope_inputs],
    )
    connection.execute(
        "CREATE TABLE p3_catalog_identity_summary (metric TEXT, count INTEGER, severity TEXT)"
    )
    connection.executemany(
        "INSERT INTO p3_catalog_identity_summary VALUES (?,?,?)",
        [tuple(row.values()) for row in identity_inputs],
    )
    scope_columns = [description[0] for description in connection.execute(scope_sql).description]
    scopes = [dict(zip(scope_columns, row)) for row in connection.execute(scope_sql).fetchall()]
    identity_columns = [description[0] for description in connection.execute(identity_sql).description]
    identity = [dict(zip(identity_columns, row)) for row in connection.execute(identity_sql).fetchall()]
    connection.close()

    with (P3_ROOT / "11-open-request-ledger.tsv").open(encoding="utf-8", newline="") as handle:
        open_ledger = list(csv.DictReader(handle, delimiter="\t"))
    with (P3_ROOT / "13d-open-transition-summary.tsv").open(encoding="utf-8", newline="") as handle:
        transition_inputs = list(csv.DictReader(handle, delimiter="\t"))
    open_durations_seconds = sorted(float(row["duration_ms"]) / 1000 for row in open_ledger)
    open_p95_seconds = open_durations_seconds[math.ceil(0.95 * len(open_durations_seconds)) - 1]
    latency_sql = (
        "WITH scenarios(scenario, poll_interval_seconds) AS ("
        "SELECT 'Local configured fast interval', 3 UNION ALL "
        "SELECT 'Approved active-watch target', 10 UNION ALL "
        "SELECT 'Public/local general default', 30), "
        f"observed(open_p95_seconds) AS (SELECT {open_p95_seconds:.6f}) "
        "SELECT scenario, poll_interval_seconds, 30 AS normal_cache_budget_seconds, "
        "ROUND(open_p95_seconds, 3) AS observed_single_request_p95_seconds, "
        "1 AS fanout_target_seconds, "
        "ROUND(30 + poll_interval_seconds + open_p95_seconds + 1, 3) AS known_component_budget_seconds, "
        "'EXCLUDED / UNKNOWN' AS unknown_rutgers_publish_delay, "
        "'NO' AS hard_30_second_guarantee FROM scenarios CROSS JOIN observed "
        "ORDER BY poll_interval_seconds ASC"
    )
    latency_connection = sqlite3.connect(":memory:")
    latency_cursor = latency_connection.execute(latency_sql)
    latency_columns = [description[0] for description in latency_cursor.description]
    latency_scenarios = [dict(zip(latency_columns, row)) for row in latency_cursor.fetchall()]
    latency_connection.close()

    transition_sql = (
        "SELECT term_id || '/' || campus AS scope, term_id, campus, "
        "CAST(raw_added_count AS INTEGER) AS raw_added_count, "
        "CAST(raw_removed_count AS INTEGER) AS raw_removed_count, "
        "CAST(effective_open_added_count AS INTEGER) AS effective_open_added_count, "
        "CAST(effective_open_removed_count AS INTEGER) AS effective_open_removed_count, "
        "body_changed, etag_changed, CAST(observation_gap_seconds AS REAL) AS observation_gap_seconds "
        "FROM p3_open_transition_summary "
        "ORDER BY (CAST(effective_open_added_count AS INTEGER) + CAST(effective_open_removed_count AS INTEGER)) DESC, "
        "scope ASC"
    )
    transition_connection = sqlite3.connect(":memory:")
    transition_connection.execute(
        "CREATE TABLE p3_open_transition_summary ("
        "term_id TEXT, campus TEXT, round_1_request_id TEXT, round_2_request_id TEXT, "
        "raw_added_count TEXT, raw_removed_count TEXT, effective_open_added_count TEXT, "
        "effective_open_removed_count TEXT, body_changed TEXT, etag_changed TEXT, observation_gap_seconds TEXT)"
    )
    transition_connection.executemany(
        "INSERT INTO p3_open_transition_summary VALUES (?,?,?,?,?,?,?,?,?,?,?)",
        [tuple(row.values()) for row in transition_inputs],
    )
    transition_cursor = transition_connection.execute(transition_sql)
    transition_columns = [description[0] for description in transition_cursor.description]
    open_transitions = [dict(zip(transition_columns, row)) for row in transition_cursor.fetchall()]
    transition_connection.close()

    source_profile = {
        "id": "src-profile",
        "label": "P3 frozen Catalog offline profile",
        "path": "project-governance/current/p3/04a-catalog-profile.json",
        "query": {
            "description": "Offline, deterministic Python traversal of the 21 successful Catalog bodies registered by SHA-256, including Fall campus D from amendment-002.",
            "tables_used": [
                "project-governance/current/p3/02-catalog-request-ledger.tsv",
                "project-governance/current/p3/03-catalog-evidence-register.tsv",
            ],
            "filters": ["ledger outcome = SUCCESS", "frozen P3 request scope only"],
            "metric_definitions": {
                "courses": "Count of top-level course rows across each successful scoped payload.",
                "sections": "Count of raw nested section rows before equivalent-duplicate collapse.",
                "semantic_conflicts": "Duplicate (term,campus,index) groups whose conservative normalized section hashes differ.",
            },
        },
    }
    source_scope = {
        "id": "src-scope-query",
        "label": "P3 Catalog scope summary query",
        "path": "project-governance/current/p3/04b-catalog-scope-summary.tsv",
        "query": {
            "sql": scope_sql,
            "description": "Query the in-memory SQLite table deterministically loaded from the frozen scope summary.",
            "tables_used": ["p3_catalog_scope_summary"],
            "filters": ["P3 frozen successful Catalog scopes only"],
            "metric_definitions": {
                "sections": "Raw nested section-row count before equivalent-duplicate collapse."
            },
        },
    }
    source_identity = {
        "id": "src-identity-query",
        "label": "P3 Catalog identity summary query",
        "path": "project-governance/current/p3/06a-section-identity-summary.tsv",
        "query": {
            "sql": identity_sql,
            "description": "Query the in-memory SQLite table deterministically loaded from the frozen identity summary.",
            "tables_used": ["p3_catalog_identity_summary"],
            "filters": ["all P3 identity metrics"],
            "metric_definitions": {
                "count": "Exact count for the named identity or collision metric."
            },
        },
    }
    source_latency = {
        "id": "src-open-latency",
        "label": "P3 Open cache and notification latency contract",
        "path": "project-governance/current/p3/19-open-cache-and-notification-latency-contract.md",
        "href": "https://www.rfc-editor.org/rfc/rfc9111.html",
        "query": {
            "sql": latency_sql,
            "engine": "SQLite",
            "language": "SQL",
            "description": "Deterministic latency-risk scenarios combining the observed two-round Open request p95 with the response's 30-second freshness allowance and the approved one-second BCSP fanout target.",
            "tables_used": ["project-governance/current/p3/11-open-request-ledger.tsv"],
            "filters": ["42 successful P3 Open requests across two rounds", "no source-change SLA assumed"],
            "metric_definitions": {
                "known_component_budget_seconds": "30-second normal cache budget + poll interval + observed single-request p95 + one-second application fanout target; excludes unknown Rutgers publish delay and full batch duration.",
                "hard_30_second_guarantee": "Whether current evidence supports a hard real-seat-change-to-notification guarantee within 30 seconds.",
            },
        },
    }
    source_open_profile = {
        "id": "src-open-profile",
        "label": "P3 frozen two-round Open profile",
        "path": "project-governance/current/p3/13a-open-profile.json",
        "query": {
            "description": "Deterministic offline profile of all 42 hash-registered Open observations and their official term/campus Catalog intersections.",
            "tables_used": [
                "project-governance/current/p3/11-open-request-ledger.tsv",
                "project-governance/current/p3/12-open-evidence-register.tsv",
                "project-governance/current/p3/13b-open-scope-join-summary.tsv",
                "project-governance/current/p3/13d-open-transition-summary.tsv",
            ],
            "filters": ["two frozen rounds", "42 HTTP 200 responses", "official term/campus intersection"],
            "metric_definitions": {
                "changed_raw_scope_pairs": "Round pairs whose canonical Open string set changed.",
                "changed_effective_scope_pairs": "Round pairs whose Catalog-intersected Open state changed.",
                "official_join_pass_observations": "Valid observations that completed official set-membership intersection without an unsafe-empty condition.",
            },
        },
    }
    source_transition = {
        "id": "src-open-transition-query",
        "label": "P3 Open round-pair transition query",
        "path": "project-governance/current/p3/13d-open-transition-summary.tsv",
        "query": {
            "sql": transition_sql,
            "engine": "SQLite",
            "language": "SQL",
            "description": "Query all 21 frozen same-target round pairs, preserving raw and Catalog-intersected additions/removals plus ETag and observation-gap audit fields.",
            "tables_used": ["p3_open_transition_summary"],
            "filters": ["one Round 1 / Round 2 pair per frozen term/campus target"],
            "metric_definitions": {
                "effective_open_added_count": "Indexes absent from the first accepted intersection and present in the second.",
                "effective_open_removed_count": "Indexes present in the first accepted intersection and absent from the second.",
            },
        },
    }
    source_open_decision = {
        "id": "src-open-decision",
        "label": "Approved P3 Open review decision",
        "path": "project-governance/current/p3/18a-open-review-decision-001.json",
        "query": {
            "description": "Frozen user-approved official join and public/local/active-watch clock mapping.",
            "tables_used": ["project-governance/current/p3/18a-open-review-decision-001.json"],
            "filters": ["status = FROZEN_JOIN_AND_RATE_CLOCK_MAPPING"],
            "metric_definitions": {
                "active_watch_target_seconds": "Requested interval for a related shared term/campus batch while at least one active watch exists."
            },
        },
    }
    source_open_completion = {
        "id": "src-open-completion",
        "label": "Frozen P3 Open Round 2 completion record",
        "path": "project-governance/current/p3/22b-open-round2-completion.json",
        "query": {
            "description": "Hash manifest and final aggregate record for the 42-request, two-round Open evidence run.",
            "tables_used": ["project-governance/current/p3/22b-open-round2-completion.json"],
            "filters": ["status = FROZEN_ROUND_2_COMPLETE"],
            "metric_definitions": {
                "successful": "Completed Open requests with HTTP 200 and a valid five-digit-string array."
            },
        },
    }
    source_open_contract = {
        "id": "src-open-contract",
        "label": "P3 shared Open final contract",
        "path": "project-governance/current/p3/23b-shared-open-final-contract.json",
        "query": {
            "description": "Frozen target identity, classification, scheduler, transport, backoff, observation, freshness, race, latency, and validation contract.",
            "tables_used": ["project-governance/current/p3/23b-shared-open-final-contract.json"],
            "filters": ["status = FROZEN_P3_SHARED_OPEN_CONTRACT"],
            "metric_definitions": {
                "valid_observation_to_server_fanout_target_seconds": "Engineering target after a valid target observation is accepted; not a Rutgers source-change SLA."
            },
        },
    }
    title = "P3 Rutgers Catalog and Open Timing Report"
    artifact = {
        "surface": "report",
        "manifest": {
            "version": 1,
            "surface": "report",
            "title": title,
            "description": "Technical evidence report for the frozen P3 Catalog scope, two-round Open evidence, and approved shared Open contract.",
            "generatedAt": generated_at,
            "sources": [source_profile, source_scope, source_identity, source_open_profile, source_transition, source_open_decision, source_open_completion, source_open_contract, source_latency],
            "charts": [
                {
                    "id": "scope-sections-chart",
                    "type": "bar",
                    "title": "Catalog section rows by term/campus scope",
                    "subtitle": "Raw section rows in 21 sampled payloads: all 15 current Fall campuses plus six Summer main/online scopes.",
                    "dataset": "scope_summary",
                    "encodings": {
                        "x": {"field": "scope", "type": "nominal", "title": "Term/campus scope"},
                        "y": {"field": "sections", "type": "quantitative", "title": "Raw section rows"},
                    },
                    "options": {"orientation": "horizontal", "showLegend": False},
                    "source": source_scope,
                }
            ],
            "tables": [
                {
                    "id": "identity-table",
                    "title": "Section identity checks",
                    "dataset": "identity_summary",
                    "columns": [
                        {"field": "metric", "label": "Metric"},
                        {"field": "count", "label": "Count", "format": "number"},
                        {"field": "severity", "label": "Interpretation"},
                    ],
                    "defaultSort": {"field": "count", "direction": "desc"},
                    "source": source_identity,
                },
                {
                    "id": "open-transition-table",
                    "title": "Open changes between the two frozen rounds",
                    "subtitle": "Raw endpoint changes and effective Catalog-intersected changes are intentionally separated; ETag is audit-only.",
                    "dataset": "open_transitions",
                    "columns": [
                        {"field": "scope", "label": "Term/campus"},
                        {"field": "raw_added_count", "label": "Raw +", "format": "number"},
                        {"field": "raw_removed_count", "label": "Raw -", "format": "number"},
                        {"field": "effective_open_added_count", "label": "Effective +", "format": "number"},
                        {"field": "effective_open_removed_count", "label": "Effective -", "format": "number"},
                        {"field": "body_changed", "label": "Body changed"},
                        {"field": "etag_changed", "label": "ETag changed"},
                        {"field": "observation_gap_seconds", "label": "Gap (s)", "format": "number"},
                    ],
                    "defaultSort": {"field": "effective_open_added_count", "direction": "desc"},
                    "source": source_transition,
                },
                {
                    "id": "open-latency-table",
                    "title": "Open detection latency risk budget",
                    "subtitle": "Illustrative known-component budgets exclude Rutgers' unknown internal publication delay and full multi-campus batch duration.",
                    "dataset": "open_latency_scenarios",
                    "columns": [
                        {"field": "scenario", "label": "Scenario"},
                        {"field": "poll_interval_seconds", "label": "Poll", "format": "number"},
                        {"field": "normal_cache_budget_seconds", "label": "Cache budget", "format": "number"},
                        {"field": "observed_single_request_p95_seconds", "label": "Observed request p95", "format": "number"},
                        {"field": "fanout_target_seconds", "label": "Fanout target", "format": "number"},
                        {"field": "known_component_budget_seconds", "label": "Known-component budget", "format": "number"},
                        {"field": "hard_30_second_guarantee", "label": "Hard <=30s?"},
                    ],
                    "defaultSort": {"field": "poll_interval_seconds", "direction": "asc"},
                    "source": source_latency,
                }
            ],
            "blocks": [
                {"id": "title", "type": "markdown", "body": f"# {title}"},
                {
                    "id": "technical-summary",
                    "type": "markdown",
                    "body": f"## Technical Summary\n\nThe frozen Catalog corpus contains {profile['successPayloads']} successful payloads, {profile['counts']['courses']:,} course rows, {profile['counts']['sections']:,} raw section rows, and {profile['counts']['meetings']:,} meeting rows. Fall 2026 covers all 15 current official campus values; Summer remains the intentional six-scope main/online structural sample. Open evidence contains {open_profile['successfulAttempts']} successful observations across two rounds. All {open_profile['totals']['officialJoinPassObservations']} pass the approved official term/campus set-intersection contract; {open_profile['totals']['changedRawScopePairs']} of 21 raw scope pairs changed, but only {open_profile['totals']['changedEffectiveScopePairs']} changed after Catalog intersection. The frozen two-clock model is public fixed 30 seconds, local default 30 seconds configurable from 3 to 3,600 seconds, and a 10-second active-watch target for the related shared batch. Current evidence does not support a hard real-seat-change-to-notification guarantee within 30 seconds.",
                },
                {"id": "scope-chart", "type": "chart", "chartId": "scope-sections-chart"},
                {
                    "id": "key-findings",
                    "type": "markdown",
                    "body": "## Key Findings\n\n`(term,campus,index)` is the minimum Section identity: 3,125 `(term,index)` collisions occur across campuses and 3,344 naked indexes collide across scopes. The 18 exact-scope duplicates can be collapsed only after semantic-equivalence verification; any future contradictory duplicate must quarantine the target transaction. Forty-one same-scope course-string groups contain multiple raw course objects, including 31 groups with distinct supplement/credit/title variants, so the course-centered model needs explicit offering variants.",
                    "sourceId": "src-profile",
                },
                {"id": "identity-detail", "type": "table", "tableId": "identity-table"},
                {
                    "id": "open-evidence-result",
                    "type": "markdown",
                    "body": "## Two rounds validate the official intersection contract, not a whole-endpoint campus merge\n\nEach `(term,campus)` target is evaluated independently against that target's Catalog Section set. The current source count is exactly one official array per batch; any orphan values are audited and ignored, and never create Sections. A valid nonempty response with a nonempty Catalog but zero intersection is unsafe and keeps last-known-good state. Across 21 target pairs, 14 raw endpoint bodies changed and three Catalog-intersected states changed. ETag changed for every pair, so it remains audit-only and cannot trigger state or audio.",
                    "sourceId": "src-open-profile",
                },
                {"id": "open-transition-detail", "type": "table", "tableId": "open-transition-table"},
                {
                    "id": "open-timing-result",
                    "type": "markdown",
                    "body": "## A 30-second cache allowance prevents a hard 30-second alert guarantee\n\n`Cache-Control: max-age=30` defines HTTP response freshness, not Rutgers' seat-state publication SLA. The auditable latency model is `U + C + P + B + F`: Rutgers publication delay, cache/representation delay, poll phase, complete-batch work, and BCSP fanout. `U` is unknown and the complete batch has not yet been measured. The product can verify a <=1-second target from a newly accepted valid observation to WebUI/audio, while source-change-to-alert must remain best effort.",
                    "sourceId": "src-open-latency",
                },
                {"id": "open-latency-detail", "type": "table", "tableId": "open-latency-table"},
                {
                    "id": "scope-definitions",
                    "type": "markdown",
                    "body": "## Scope, Data, and Metric Definitions\n\nThe chart and counts cover Fall 2026 across all 15 current official campuses plus Summer 2026 across the six main/online campuses. Summer off-campus values are not required for this adjacent-term structural sample. Counts are raw row counts from decoded JSON bodies. A semantic duplicate ignores only collection order and the corresponding derived display-text fields; it does not merge contradictory values.",
                },
                {
                    "id": "methodology",
                    "type": "markdown",
                    "body": "## Methodology\n\nEach successful Catalog and Open response was ledgered with request scope, timing, encoding, decoded size, raw-body SHA-256, and ignored evidence path. One Catalog gzip client failure was retained as non-evidentiary and replaced through a frozen amendment. Open Round 2 resumed only through a hash-chained allowlist after user approval; all 21 resumed requests succeeded, with no retries or additional Catalog requests. Profiling and the notebook run offline against registered bodies, and the notebook executes top-to-bottom with captured outputs.",
                },
                {
                    "id": "limitations",
                    "type": "markdown",
                    "body": "## Limitations and Uncertainty\n\nEach Catalog scope was observed once, so the corpus is not an availability or schema-stability SLA. Zero-row scopes prove a valid response at the observation instant, not permanent absence. The 42 low-volume serial Open observations establish current response shape, cache headers, two-round transitions, and the approved intersection algorithm. They do not establish Rutgers' internal publish delay, sustained 3/10/30-second production capacity, multi-target scheduler capacity, or a hard source-change notification bound. Malformed, timeout, 429, 5xx, unsafe-empty, coalescing, and saturation behavior require deterministic fake-upstream tests rather than pressure against Rutgers. The latency table is a risk budget, not a forecast or SLA.",
                },
                {
                    "id": "next-steps",
                    "type": "markdown",
                    "body": "## Recommended Next Step\n\nUse this frozen evidence and contract as P4's immutable input. P4 should design only the public-package delta: ephemeral sessions, fixed public intervals, capability removal, Linux service packaging, and degradation/operations behavior. No more Rutgers evidence requests are authorized; capacity and failure semantics move to P7 fake-upstream verification.",
                },
                {
                    "id": "further-questions",
                    "type": "markdown",
                    "body": "## Further Questions\n\nP7 must determine the sustainable target count at 3, 10, and 30 seconds under the frozen single-origin concurrency of one, verify earliest-due scheduling without starvation, and measure observation-to-fanout latency. Any proposal for higher real-origin concurrency or a hard source-change alert SLA requires new evidence and Review rather than an implementation shortcut.",
                },
            ],
        },
        "snapshot": {
            "version": 1,
            "status": "ready",
            "generatedAt": generated_at,
            "datasets": {
                "scope_summary": scopes,
                "identity_summary": identity,
                "open_transitions": open_transitions,
                "open_latency_scenarios": latency_scenarios,
            },
        },
        "sources": [source_profile, source_scope, source_identity, source_open_profile, source_transition, source_open_decision, source_open_completion, source_open_contract, source_latency],
        "package_info": {
            "report_kind": "technical_data_quality",
            "evidence_scope": "P3_CATALOG_AND_TWO_ROUND_OPEN_COMPLETE_SHARED_CONTRACT_FROZEN",
            "not_product_runtime": True,
        },
    }

    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT_PATH.write_text(json.dumps(artifact, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"artifact={OUTPUT_PATH}")
    print(f"scope_rows={len(scopes)} identity_rows={len(identity)}")


if __name__ == "__main__":
    main()
