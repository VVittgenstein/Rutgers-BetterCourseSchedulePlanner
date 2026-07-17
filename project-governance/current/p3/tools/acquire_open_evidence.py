#!/usr/bin/env python3
"""Execute the next entries in the frozen P3 shared Open request manifest."""

from __future__ import annotations

import argparse
import csv
import gzip
import hashlib
import json
import os
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


REPOSITORY_ROOT = Path(__file__).resolve().parents[4]
P3_ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = P3_ROOT / "10a-open-request-manifest.json"
STOP_PATH = P3_ROOT / "10c-open-request-stop-001.json"
LEDGER_PATH = P3_ROOT / "11-open-request-ledger.tsv"
REVIEW_DECISION_PATH = P3_ROOT / "18a-open-review-decision-001.json"
LATENCY_CONTRACT_PATH = P3_ROOT / "19-open-cache-and-notification-latency-contract.md"
AMENDMENT_PATH = P3_ROOT / "21b-open-round2-resumption-amendment-001.json"
EXPECTED_MANIFEST_SHA256 = "707705756EEA4D269EDD1822F8529453B1E8C7838E23B1FC843789379B947703"
EXPECTED_AMENDMENT_SHA256 = "4C3B9FA7FD1F7DE4DF35570A55D0479DE15B82F1D510CDBC7D34B9EA2F332E2B"
HEADER_ALLOWLIST = {
    "content-type": "content_type",
    "content-encoding": "content_encoding",
    "date": "response_date",
    "etag": "etag",
    "last-modified": "last_modified",
    "cache-control": "cache_control",
    "age": "age",
    "retry-after": "retry_after",
}


class SameOriginRedirectHandler(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req: urllib.request.Request, fp: Any, code: int, msg: str, headers: Any, newurl: str) -> urllib.request.Request | None:
        old = urllib.parse.urlsplit(req.full_url)
        new = urllib.parse.urlsplit(newurl)
        if (old.scheme, old.netloc) != (new.scheme, new.netloc):
            raise urllib.error.HTTPError(newurl, code, "OFF_ORIGIN_REDIRECT", headers, fp)
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--limit", type=int, default=5, help="next sequential attempts to execute (1..5)")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    if not 1 <= args.limit <= 5:
        parser.error("limit must be 1..5")
    return args


def read_tsv(path: Path) -> list[dict[str, str]]:
    with path.open("r", encoding="utf-8", newline="") as handle:
        return list(csv.DictReader(handle, delimiter="\t"))


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest().upper()


def frozen_ledger_prefix_sha256(row_count: int) -> str:
    lines = LEDGER_PATH.read_bytes().splitlines(keepends=True)
    if len(lines) < row_count + 1:
        raise RuntimeError("Open ledger is shorter than the frozen prefix")
    return hashlib.sha256(b"".join(lines[: row_count + 1])).hexdigest().upper()


def load_manifest() -> dict[str, Any]:
    body = MANIFEST_PATH.read_bytes()
    if hashlib.sha256(body).hexdigest().upper() != EXPECTED_MANIFEST_SHA256:
        raise RuntimeError("frozen Open manifest SHA-256 mismatch")
    manifest = json.loads(body)
    policy = manifest.get("policy", {})
    requests = manifest.get("requests")
    if manifest.get("status") != "FROZEN_BEFORE_FIRST_OPEN_REQUEST":
        raise RuntimeError("Open manifest is not frozen")
    if not isinstance(requests, list) or len(requests) != 42 or policy.get("hardAttemptLimit") != 42:
        raise RuntimeError("Open manifest request budget mismatch")
    if policy.get("automaticRetries") != 0 or policy.get("concurrency") != 1:
        raise RuntimeError("unsafe Open retry/concurrency policy")
    if manifest.get("catalogReuse", {}).get("catalogRequestsPermitted") != 0:
        raise RuntimeError("Open manifest unexpectedly permits Catalog requests")
    if len({item.get("id") for item in requests}) != len(requests):
        raise RuntimeError("duplicate Open request IDs")

    amendment_body = AMENDMENT_PATH.read_bytes()
    if hashlib.sha256(amendment_body).hexdigest().upper() != EXPECTED_AMENDMENT_SHA256:
        raise RuntimeError("frozen Open Round 2 amendment SHA-256 mismatch")
    amendment = json.loads(amendment_body)
    if amendment.get("status") != "FROZEN_ROUND_2_RESUMPTION_AFTER_USER_REVIEW":
        raise RuntimeError("Open Round 2 amendment is not frozen")
    chain = amendment.get("hashChain", {})
    if chain.get("manifestSha256") != EXPECTED_MANIFEST_SHA256:
        raise RuntimeError("Open amendment manifest hash mismatch")
    if chain.get("historicalStopSha256") != sha256(STOP_PATH):
        raise RuntimeError("Open amendment historical stop hash mismatch")
    if chain.get("reviewDecisionSha256") != sha256(REVIEW_DECISION_PATH):
        raise RuntimeError("Open amendment review decision hash mismatch")
    if chain.get("latencyContractSha256") != sha256(LATENCY_CONTRACT_PATH):
        raise RuntimeError("Open amendment latency contract hash mismatch")
    if chain.get("ledgerSha256AtAmendmentFreeze") != frozen_ledger_prefix_sha256(21):
        raise RuntimeError("Open amendment frozen ledger prefix hash mismatch")

    authorized = amendment.get("authorizedRequestIds")
    expected_round_two = [item["id"] for item in requests if item.get("round") == 2]
    if authorized != expected_round_two or len(authorized) != 21:
        raise RuntimeError("Open amendment Round 2 allowlist mismatch")
    budget = amendment.get("budget", {})
    execution = amendment.get("executionPolicy", {})
    if budget.get("completedBeforeAmendment") != 21 or budget.get("remainingAuthorizedAttempts") != 21 or budget.get("effectiveHardAttemptLimit") != 42:
        raise RuntimeError("Open amendment attempt budget mismatch")
    if budget.get("catalogRequestsPermitted") != 0 or budget.get("additionalOpenRequestIdsPermitted") != 0:
        raise RuntimeError("Open amendment unexpectedly expands request scope")
    for key in ("concurrency", "minimumAttemptStartIntervalMs", "timeoutMs", "automaticRetries", "maxResponseBytes"):
        if execution.get(key) != policy.get(key):
            raise RuntimeError(f"Open amendment policy mismatch: {key}")
    if execution.get("cacheBustingPermitted") or execution.get("conditionalRequestExperimentPermitted"):
        raise RuntimeError("Open amendment unexpectedly permits additional cache experiments")
    manifest["round2Amendment"] = amendment
    return manifest


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def parse_utc(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def wait_until_interval(last_time: str, minimum_ms: int) -> None:
    elapsed = (datetime.now(timezone.utc) - parse_utc(last_time)).total_seconds()
    remaining = minimum_ms / 1000 - elapsed
    if remaining > 0:
        time.sleep(remaining)


def append_ledger(row: dict[str, Any]) -> None:
    with LEDGER_PATH.open("r", encoding="utf-8", newline="") as source:
        fieldnames = next(csv.reader(source, delimiter="\t"))
    with LEDGER_PATH.open("a", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames, delimiter="\t", lineterminator="\n")
        writer.writerow({key: row.get(key, "") for key in fieldnames})


def json_type(value: Any) -> str:
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, str):
        return "string"
    if isinstance(value, (int, float)):
        return "number"
    if isinstance(value, list):
        return "array"
    if isinstance(value, dict):
        return "object"
    return type(value).__name__


def raw_duplicate_count(values: list[Any]) -> int:
    keys = [json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) for value in values]
    return len(keys) - len(set(keys))


def execute(manifest: dict[str, Any], item: dict[str, Any]) -> dict[str, Any]:
    policy = manifest["policy"]
    query = urllib.parse.urlencode({"year": item["year"], "term": item["term"], "campus": item["campus"]})
    url = f"{manifest['source']['baseUrl'].rstrip('/')}/{manifest['source']['endpoint']}?{query}"
    raw_root = REPOSITORY_ROOT / manifest["rawRoot"]
    raw_root.mkdir(parents=True, exist_ok=True)
    raw_name = f"{item['id']}_{item['termId']}_{item['campus']}_openSections.json"
    raw_path = raw_root / raw_name
    metadata_path = raw_root / raw_name.replace(".json", ".metadata.json")
    if raw_path.exists() or metadata_path.exists():
        raise RuntimeError(f"raw/metadata exists without ledger-safe state: {item['id']}")

    base_row: dict[str, Any] = {
        "request_id": item["id"],
        "round": item["round"],
        "round_ordinal": item["roundOrdinal"],
        "term_id": item["termId"],
        "year": item["year"],
        "term": item["term"],
        "campus": item["campus"],
        "url": url,
        "started_at_utc": utc_now(),
    }
    request = urllib.request.Request(url, headers=policy["headers"], method="GET")
    opener = urllib.request.build_opener(SameOriginRedirectHandler())
    started_clock = time.perf_counter()
    try:
        with opener.open(request, timeout=policy["timeoutMs"] / 1000) as response:
            status = int(response.status)
            final_url = response.geturl()
            if urllib.parse.urlsplit(final_url)[:2] != urllib.parse.urlsplit(url)[:2]:
                raise urllib.error.HTTPError(final_url, status, "OFF_ORIGIN_REDIRECT", response.headers, None)
            headers = {key.lower(): value for key, value in response.headers.items()}
            wire_body = response.read(policy["maxResponseBytes"] + 1)
    except urllib.error.HTTPError as error:
        row = {
            **base_row,
            "completed_at_utc": utc_now(),
            "http_status": error.code,
            "duration_ms": round((time.perf_counter() - started_clock) * 1000, 3),
            "outcome": "FAILURE",
            "error_class": "OFF_ORIGIN_REDIRECT" if str(error.reason) == "OFF_ORIGIN_REDIRECT" else f"HTTP_{error.code}",
        }
        append_ledger(row)
        return row
    except Exception as error:
        row = {
            **base_row,
            "completed_at_utc": utc_now(),
            "duration_ms": round((time.perf_counter() - started_clock) * 1000, 3),
            "outcome": "FAILURE",
            "error_class": type(error).__name__,
        }
        append_ledger(row)
        return row

    header_values = {target: headers.get(source, "") for source, target in HEADER_ALLOWLIST.items()}
    content_type = header_values["content_type"].lower()
    content_encoding = header_values["content_encoding"].lower().strip()
    outcome, error_class = "SUCCESS", ""
    decoded_body = b""
    parsed: list[Any] = []
    if status < 200 or status >= 300:
        outcome, error_class = "FAILURE", f"HTTP_{status}"
    elif len(wire_body) > policy["maxResponseBytes"]:
        outcome, error_class = "FAILURE", "RESPONSE_TOO_LARGE"
    elif "json" not in content_type:
        outcome, error_class = "FAILURE", "NON_JSON_CONTENT_TYPE"
    else:
        try:
            if content_encoding in ("", "identity"):
                decoded_body = wire_body
            elif content_encoding == "gzip":
                decoded_body = gzip.decompress(wire_body)
            else:
                outcome, error_class = "FAILURE", "UNSUPPORTED_CONTENT_ENCODING"
        except (OSError, EOFError):
            outcome, error_class = "FAILURE", "CONTENT_DECODE_FAILURE"
        if outcome == "SUCCESS" and len(decoded_body) > policy["maxResponseBytes"]:
            outcome, error_class = "FAILURE", "DECODED_RESPONSE_TOO_LARGE"
        if outcome == "SUCCESS":
            try:
                value = json.loads(decoded_body)
                if not isinstance(value, list):
                    outcome, error_class = "FAILURE", "ROOT_NOT_ARRAY"
                else:
                    parsed = value
            except (json.JSONDecodeError, UnicodeDecodeError):
                outcome, error_class = "FAILURE", "JSON_PARSE_FAILURE"
    if outcome == "SUCCESS" and any(not isinstance(value, str) or re.fullmatch(r"\d{5}", value) is None for value in parsed):
        outcome, error_class = "FAILURE", "NON_FIVE_DIGIT_STRING_VALUE"

    types = Counter(json_type(value) for value in parsed)
    strings = [value for value in parsed if isinstance(value, str)]
    numerics = [value for value in parsed if isinstance(value, (int, float)) and not isinstance(value, bool)]
    row = {
        **base_row,
        "completed_at_utc": utc_now(),
        "http_status": status,
        "duration_ms": round((time.perf_counter() - started_clock) * 1000, 3),
        "bytes": len(wire_body),
        "decoded_bytes": len(decoded_body),
        **header_values,
        "wire_sha256": hashlib.sha256(wire_body).hexdigest().upper(),
        "body_sha256": hashlib.sha256(decoded_body).hexdigest().upper() if decoded_body else "",
        "top_level_rows": len(parsed) if outcome == "SUCCESS" else "",
        "root_value_types": json.dumps(dict(sorted(types.items())), separators=(",", ":")),
        "string_count": len(strings),
        "numeric_count": len(numerics),
        "duplicate_raw_values": raw_duplicate_count(parsed) if outcome == "SUCCESS" else "",
        "unique_string_values": len(set(strings)),
        "outcome": outcome,
        "error_class": error_class,
    }
    if outcome == "SUCCESS":
        row["raw_relative_path"] = str(Path(manifest["rawRoot"]) / raw_name).replace(os.sep, "/")
        raw_path.write_bytes(decoded_body)
        metadata_path.write_text(json.dumps(row, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    append_ledger(row)
    return row


def main() -> int:
    args = parse_args()
    manifest = load_manifest()
    ledger = read_tsv(LEDGER_PATH)
    if len(ledger) >= manifest["policy"]["hardAttemptLimit"]:
        raise RuntimeError("Open hard attempt limit already reached")
    if any(row.get("outcome") != "SUCCESS" for row in ledger):
        raise RuntimeError("terminal Open failure exists; further attempts are forbidden")
    expected_prefix = [item["id"] for item in manifest["requests"][: len(ledger)]]
    if [row.get("request_id") for row in ledger] != expected_prefix:
        raise RuntimeError("Open ledger is not an exact manifest prefix")
    round_two_rows = [row for row in ledger if row.get("round") == "2"]
    authorized = manifest["round2Amendment"]["authorizedRequestIds"]
    if [row.get("request_id") for row in round_two_rows] != authorized[: len(round_two_rows)]:
        raise RuntimeError("Open ledger contains a Round 2 request outside the amendment prefix")
    pending = manifest["requests"][len(ledger) : len(ledger) + args.limit]
    if not pending:
        print("OPEN_ACQUISITION_COMPLETE")
        return 0
    if args.dry_run:
        print(json.dumps(pending, ensure_ascii=False, indent=2))
        return 0

    if any(item["id"] not in authorized for item in pending):
        raise RuntimeError("pending request is not authorized by the frozen Round 2 amendment")

    for item in pending:
        ledger = read_tsv(LEDGER_PATH)
        if ledger:
            wait_until_interval(ledger[-1]["started_at_utc"], manifest["policy"]["minimumAttemptStartIntervalMs"])
        if item["round"] == 2:
            round_one = [row for row in ledger if row.get("round") == "1"]
            if len(round_one) != 21:
                raise RuntimeError("round two cannot start before all 21 round-one successes")
            wait_until_interval(round_one[-1]["completed_at_utc"], manifest["policy"]["minimumRoundGapAfterPriorRoundCompletionMs"])
        row = execute(manifest, item)
        print(json.dumps(row, ensure_ascii=False, separators=(",", ":")))
        if row["outcome"] != "SUCCESS":
            print(f"terminal Open manifest stop: {item['id']} {row['error_class']}", file=sys.stderr)
            return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
