#!/usr/bin/env python3
"""Execute only the frozen P3 Rutgers Catalog request manifest.

The tool deliberately has no arbitrary URL, term, campus, retry, or force option.
Each request ID can be attempted once. Raw payloads live under the repository's
already-ignored data/staging directory; governance retains only hashes/metrics.
"""

from __future__ import annotations

import argparse
import csv
import gzip
import hashlib
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


REPOSITORY_ROOT = Path(__file__).resolve().parents[4]
P3_ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = P3_ROOT / "01a-catalog-request-manifest.json"
AMENDMENT_PATH = P3_ROOT / "01c-catalog-request-amendment-001.json"
SCOPE_AMENDMENT_PATH = P3_ROOT / "01e-catalog-request-amendment-002.json"
LEDGER_PATH = P3_ROOT / "02-catalog-request-ledger.tsv"
HEADER_ALLOWLIST = {
    "content-type": "content_type",
    "content-encoding": "content_encoding",
    "date": "response_date",
    "etag": "etag",
    "last-modified": "last_modified",
    "cache-control": "cache_control",
    "age": "age",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--offset", type=int, default=0, help="zero-based manifest offset")
    parser.add_argument("--limit", type=int, default=4, help="maximum manifest entries for this invocation")
    parser.add_argument("--dry-run", action="store_true", help="print selected requests without network access")
    args = parser.parse_args()
    if args.offset < 0 or args.limit < 1 or args.limit > 4:
        parser.error("offset must be >=0 and limit must be 1..4")
    return args


def load_manifest() -> dict[str, Any]:
    manifest_bytes = MANIFEST_PATH.read_bytes()
    manifest = json.loads(manifest_bytes)
    if manifest.get("status") != "FROZEN_BEFORE_FIRST_COURSES_REQUEST":
        raise RuntimeError("manifest is not frozen")
    requests = manifest.get("requests")
    policy = manifest.get("policy", {})
    if not isinstance(requests, list) or len(requests) != policy.get("hardAttemptLimit"):
        raise RuntimeError("manifest request count does not equal hard attempt limit")
    if policy.get("automaticRetries") != 0 or policy.get("concurrency") != 1:
        raise RuntimeError("unsafe manifest retry/concurrency policy")
    amendment = json.loads(AMENDMENT_PATH.read_text(encoding="utf-8"))
    base_sha256 = hashlib.sha256(manifest_bytes).hexdigest().upper()
    if base_sha256 != amendment.get("baseManifestSha256"):
        raise RuntimeError("base manifest hash does not match frozen amendment")
    replacements = amendment.get("replacementRequests")
    if amendment.get("status") != "FROZEN_TECHNICAL_CORRECTION" or not isinstance(replacements, list):
        raise RuntimeError("catalog amendment is not frozen")
    if amendment.get("effectiveHardAttemptLimit") != len(requests) + len(replacements):
        raise RuntimeError("amendment effective hard limit mismatch")
    scope_amendment_bytes = SCOPE_AMENDMENT_PATH.read_bytes()
    scope_amendment = json.loads(scope_amendment_bytes)
    prior_amendment_sha256 = hashlib.sha256(AMENDMENT_PATH.read_bytes()).hexdigest().upper()
    if scope_amendment.get("status") != "FROZEN_SCOPE_CORRECTION":
        raise RuntimeError("catalog scope amendment is not frozen")
    if scope_amendment.get("baseManifestSha256") != base_sha256:
        raise RuntimeError("scope amendment base manifest hash mismatch")
    if scope_amendment.get("priorAmendmentSha256") != prior_amendment_sha256:
        raise RuntimeError("scope amendment chain hash mismatch")
    additions = scope_amendment.get("additionalRequests")
    if not isinstance(additions, list) or len(additions) != 1:
        raise RuntimeError("scope amendment must contain exactly one request")
    if scope_amendment.get("effectiveHardAttemptLimit") != len(requests) + len(replacements) + len(additions):
        raise RuntimeError("scope amendment effective hard limit mismatch")
    manifest["requests"] = requests + replacements + additions
    manifest["effectiveHardAttemptLimit"] = scope_amendment["effectiveHardAttemptLimit"]
    ids = [item.get("id") for item in manifest["requests"]]
    if len(ids) != len(set(ids)):
        raise RuntimeError("manifest has duplicate request IDs")
    return manifest


def load_attempted_ids() -> set[str]:
    if not LEDGER_PATH.exists():
        return set()
    with LEDGER_PATH.open("r", encoding="utf-8", newline="") as handle:
        return {row["request_id"] for row in csv.DictReader(handle, delimiter="\t") if row.get("request_id")}


def ensure_interval(minimum_ms: int) -> None:
    if not LEDGER_PATH.exists():
        return
    elapsed = time.time() - LEDGER_PATH.stat().st_mtime
    remaining = minimum_ms / 1000 - elapsed
    if remaining > 0:
        time.sleep(remaining)


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def append_ledger(row: dict[str, Any]) -> None:
    with LEDGER_PATH.open("r", encoding="utf-8", newline="") as source:
        fieldnames = next(csv.reader(source, delimiter="\t"))
    with LEDGER_PATH.open("a", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames, delimiter="\t", lineterminator="\n")
        writer.writerow({key: row.get(key, "") for key in fieldnames})


def execute_request(manifest: dict[str, Any], item: dict[str, Any]) -> dict[str, Any]:
    policy = manifest["policy"]
    base_url = manifest["source"]["baseUrl"].rstrip("/")
    endpoint = manifest["source"]["endpoint"]
    query = urllib.parse.urlencode({"year": item["year"], "term": item["term"], "campus": item["campus"]})
    url = f"{base_url}/{endpoint}?{query}"
    raw_root = REPOSITORY_ROOT / manifest["rawRoot"]
    raw_root.mkdir(parents=True, exist_ok=True)
    raw_name = f"{item['id']}_{item['termId']}_{item['campus']}_courses.json"
    raw_path = raw_root / raw_name
    metadata_path = raw_root / raw_name.replace(".json", ".metadata.json")
    if raw_path.exists() or metadata_path.exists():
        raise RuntimeError(f"raw/metadata already exists without ledger-safe retry: {item['id']}")

    request = urllib.request.Request(url, headers=policy["headers"], method="GET")
    started_at = utc_now()
    started_clock = time.perf_counter()
    base_row: dict[str, Any] = {
        "request_id": item["id"],
        "term_id": item["termId"],
        "year": item["year"],
        "term": item["term"],
        "campus": item["campus"],
        "url": url,
        "started_at_utc": started_at,
        "raw_relative_path": "",
    }

    try:
        with urllib.request.urlopen(request, timeout=policy["timeoutMs"] / 1000) as response:
            status = int(response.status)
            headers = {key.lower(): value for key, value in response.headers.items()}
            wire_body = response.read(policy["maxResponseBytes"] + 1)
    except urllib.error.HTTPError as error:
        completed = utc_now()
        row = {
            **base_row,
            "completed_at_utc": completed,
            "http_status": error.code,
            "duration_ms": round((time.perf_counter() - started_clock) * 1000, 3),
            "outcome": "FAILURE",
            "error_class": f"HTTP_{error.code}",
        }
        append_ledger(row)
        return row
    except Exception as error:  # timeout/network are both terminal under the frozen manifest
        completed = utc_now()
        row = {
            **base_row,
            "completed_at_utc": completed,
            "duration_ms": round((time.perf_counter() - started_clock) * 1000, 3),
            "outcome": "FAILURE",
            "error_class": type(error).__name__,
        }
        append_ledger(row)
        return row

    completed_at = utc_now()
    duration_ms = round((time.perf_counter() - started_clock) * 1000, 3)
    header_values = {target: headers.get(source, "") for source, target in HEADER_ALLOWLIST.items()}
    content_type = header_values["content_type"].lower()
    content_encoding = header_values["content_encoding"].lower().strip()
    error_class = ""
    top_level_rows: int | str = ""
    outcome = "SUCCESS"
    decoded_body = b""

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
                parsed = json.loads(decoded_body)
                if not isinstance(parsed, list):
                    outcome, error_class = "FAILURE", "ROOT_NOT_ARRAY"
                else:
                    top_level_rows = len(parsed)
            except (json.JSONDecodeError, UnicodeDecodeError):
                outcome, error_class = "FAILURE", "JSON_PARSE_FAILURE"

    wire_sha256 = hashlib.sha256(wire_body).hexdigest().upper()
    body_sha256 = hashlib.sha256(decoded_body).hexdigest().upper() if decoded_body else ""
    row = {
        **base_row,
        "completed_at_utc": completed_at,
        "http_status": status,
        "duration_ms": duration_ms,
        "bytes": len(wire_body),
        "decoded_bytes": len(decoded_body),
        **header_values,
        "wire_sha256": wire_sha256,
        "body_sha256": body_sha256,
        "top_level_rows": top_level_rows,
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
    requests = manifest["requests"]
    selected = requests[args.offset : args.offset + args.limit]
    if not selected:
        raise RuntimeError("selected manifest slice is empty")

    attempted = load_attempted_ids()
    duplicate = [item["id"] for item in selected if item["id"] in attempted]
    if duplicate:
        raise RuntimeError(f"request IDs already attempted; retries are forbidden: {','.join(duplicate)}")

    if args.dry_run:
        print(json.dumps(selected, ensure_ascii=False, indent=2))
        return 0

    minimum_ms = manifest["policy"]["minimumAttemptStartIntervalMs"]
    for index, item in enumerate(selected):
        if index > 0 or LEDGER_PATH.stat().st_size > 0:
            ensure_interval(minimum_ms)
        row = execute_request(manifest, item)
        print(json.dumps(row, ensure_ascii=False, separators=(",", ":")))
        if row["outcome"] != "SUCCESS":
            print(f"terminal manifest stop: {item['id']} {row['error_class']}", file=sys.stderr)
            return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
