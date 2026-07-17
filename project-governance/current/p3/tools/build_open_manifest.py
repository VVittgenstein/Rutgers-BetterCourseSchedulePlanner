#!/usr/bin/env python3
"""Build or verify the exact P3 shared Open evidence request manifest."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
from pathlib import Path


P3_ROOT = Path(__file__).resolve().parents[1]
SCOPE_PATH = P3_ROOT / "04b-catalog-scope-summary.tsv"
LEDGER_PATH = P3_ROOT / "02-catalog-request-ledger.tsv"
OUTPUT_PATH = P3_ROOT / "10a-open-request-manifest.json"
FROZEN_AT = "2026-07-13T10:59:53+08:00"


def read_tsv(path: Path) -> list[dict[str, str]]:
    with path.open("r", encoding="utf-8", newline="") as handle:
        return list(csv.DictReader(handle, delimiter="\t"))


def build() -> dict[str, object]:
    scopes = read_tsv(SCOPE_PATH)
    if len(scopes) != 21:
        raise RuntimeError(f"expected 21 frozen Catalog scopes, found {len(scopes)}")
    ledger = [row for row in read_tsv(LEDGER_PATH) if row.get("outcome") == "SUCCESS"]
    by_request = {row["request_id"]: row for row in ledger}
    if len(by_request) != 21:
        raise RuntimeError(f"expected 21 successful Catalog requests, found {len(by_request)}")

    canonical_scope = "".join(
        f"{row['term_id']}\t{row['campus']}\t{row['body_sha256']}\n"
        for row in sorted(scopes, key=lambda item: (item["term_id"], item["campus"]))
    )
    scope_digest = hashlib.sha256(canonical_scope.encode("utf-8")).hexdigest().upper()

    scope_rows = []
    for ordinal, scope in enumerate(scopes, start=1):
        catalog = by_request[scope["request_id"]]
        scope_rows.append(
            {
                "scopeOrdinal": ordinal,
                "termId": scope["term_id"],
                "year": int(catalog["year"]),
                "term": int(catalog["term"]),
                "campus": scope["campus"],
                "catalogRequestId": scope["request_id"],
                "catalogBodySha256": scope["body_sha256"],
                "catalogRawRelativePath": catalog["raw_relative_path"],
            }
        )

    requests = []
    for round_number in (1, 2):
        for scope in scope_rows:
            requests.append(
                {
                    "id": f"OPEN-R{round_number}-{scope['scopeOrdinal']:03d}",
                    "round": round_number,
                    "roundOrdinal": scope["scopeOrdinal"],
                    "termId": scope["termId"],
                    "year": scope["year"],
                    "term": scope["term"],
                    "campus": scope["campus"],
                    "catalogRequestId": scope["catalogRequestId"],
                    "catalogBodySha256": scope["catalogBodySha256"],
                }
            )

    return {
        "schemaVersion": 1,
        "manifestId": "P3-OPEN-20260713-01",
        "status": "FROZEN_BEFORE_FIRST_OPEN_REQUEST",
        "frozenAt": FROZEN_AT,
        "source": {
            "owner": "Rutgers University Schedule of Classes",
            "baseUrl": "https://classes.rutgers.edu/soc/api",
            "endpoint": "openSections.json",
            "method": "GET",
            "authentication": "NONE",
        },
        "catalogReuse": {
            "catalogRequestsPermitted": 0,
            "scopeCount": 21,
            "scopeDigestSha256": scope_digest,
            "scopeSummarySha256": hashlib.sha256(SCOPE_PATH.read_bytes()).hexdigest().upper(),
            "scopes": scope_rows,
        },
        "policy": {
            "rounds": 2,
            "concurrency": 1,
            "minimumAttemptStartIntervalMs": 5000,
            "minimumRoundGapAfterPriorRoundCompletionMs": 60000,
            "timeoutMs": 30000,
            "automaticRetries": 0,
            "hardAttemptLimit": 42,
            "maxResponseBytes": 10485760,
            "headers": {
                "User-Agent": "RBCSP-Evidence/1.0",
                "Accept": "application/json",
                "Accept-Encoding": "identity",
            },
            "stopOn": [
                "HTTP_403",
                "HTTP_429",
                "ANY_NON_2XX",
                "OFF_ORIGIN_REDIRECT",
                "NON_JSON_CONTENT_TYPE",
                "JSON_PARSE_FAILURE",
                "ROOT_NOT_ARRAY",
                "RESPONSE_TOO_LARGE",
                "TIMEOUT_OR_NETWORK_FAILURE",
                "LEDGER_RAW_INCONSISTENCY",
            ],
        },
        "rawRoot": "data/staging/p3-rutgers-evidence-20260713/open",
        "ledger": "project-governance/current/p3/11-open-request-ledger.tsv",
        "requests": requests,
    }


def encoded(manifest: dict[str, object]) -> bytes:
    return (json.dumps(manifest, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verify", action="store_true")
    args = parser.parse_args()
    payload = encoded(build())
    if args.verify:
        if not OUTPUT_PATH.is_file() or OUTPUT_PATH.read_bytes() != payload:
            raise RuntimeError("frozen Open manifest differs from deterministic generator")
        print("OPEN_MANIFEST_VERIFY=PASS")
    else:
        if OUTPUT_PATH.exists():
            raise RuntimeError("refusing to overwrite existing frozen Open manifest")
        OUTPUT_PATH.write_bytes(payload)
        print(f"manifest={OUTPUT_PATH}")
    print(f"sha256={hashlib.sha256(payload).hexdigest().upper()}")
    print("scopes=21 rounds=2 requests=42")


if __name__ == "__main__":
    main()
