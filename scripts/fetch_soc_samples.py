#!/usr/bin/env python3
"""Fetch Rutgers SOC course/openSections snapshots for a given term/campuses."""
from __future__ import annotations

import argparse
import datetime as dt
import gzip
import hashlib
import json
import os
import pathlib
import sys
import urllib.error
import urllib.parse
import urllib.request
from typing import Dict, List

API_BASE = "https://sis.rutgers.edu/soc/api"
ENDPOINTS = {
    "courses": "courses.json",
    "openSections": "openSections.json",
}
DEFAULT_CAMPUSES = ("NB", "NK", "CM")
UA = "BetterCourseSchedulePlanner/0.1 (spring-2026-sample)"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--year", type=int, default=2026, help="Academic year (e.g. 2026)")
    parser.add_argument("--term", type=int, default=1, help="Term code (0=Winter,1=Spring,7=Summer,9=Fall)")
    parser.add_argument(
        "--campuses",
        type=str,
        default=",".join(DEFAULT_CAMPUSES),
        help="Comma-separated campus codes (e.g. NB,NK,CM)",
    )
    parser.add_argument(
        "--tag",
        type=str,
        default="spring-2026",
        help="File prefix under output directory",
    )
    parser.add_argument(
        "--output-dir",
        type=pathlib.Path,
        default=pathlib.Path("data/raw"),
        help="Directory where JSON snapshots will be stored",
    )
    parser.add_argument(
        "--metadata",
        type=pathlib.Path,
        default=None,
        help="Optional metadata json path (default: <output>/<tag>-metadata.json)",
    )
    return parser.parse_args()


def build_url(endpoint: str, year: int, term: int, campus: str) -> str:
    params = urllib.parse.urlencode({"year": year, "term": term, "campus": campus})
    return f"{API_BASE}/{ENDPOINTS[endpoint]}?{params}"


def fetch(url: str) -> Dict[str, object]:
    req = urllib.request.Request(
        url,
        headers={
            "User-Agent": UA,
            "Accept": "application/json",
            "Accept-Encoding": "gzip",
            "Connection": "close",
        },
    )
    with urllib.request.urlopen(req, timeout=60) as resp:
        headers = {k: v for k, v in resp.getheaders()}
        status = resp.getcode()
        raw = resp.read()
    encoding = headers.get("Content-Encoding", "").lower()
    if encoding == "gzip":
        payload = gzip.decompress(raw)
    else:
        payload = raw
    return {
        "status": status,
        "headers": headers,
        "raw_bytes": raw,
        "payload_bytes": payload,
    }


def save_payload(path: pathlib.Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("wb") as fh:
        fh.write(payload)


def summarize_payload(endpoint: str, payload_bytes: bytes) -> Dict[str, int]:
    data = json.loads(payload_bytes.decode("utf-8"))
    summary: Dict[str, int] = {"record_count": len(data)}
    if endpoint == "courses":
        sections = sum(len(item.get("sections", [])) for item in data)
        summary["section_count"] = sections
        subjects = {item.get("subject") for item in data if "subject" in item}
        summary["distinct_subjects"] = len(subjects)
    return summary


def main() -> int:
    args = parse_args()
    campuses = [c.strip().upper() for c in args.campuses.split(",") if c.strip()]
    timestamp = dt.datetime.utcnow().replace(microsecond=0).isoformat() + "Z"
    metadata_path = args.metadata or args.output_dir / f"{args.tag}-metadata.json"
    entries: List[Dict[str, object]] = []

    for campus in campuses:
        for endpoint in ENDPOINTS:
            url = build_url(endpoint, args.year, args.term, campus)
            sys.stderr.write(f"Fetching {endpoint} for {campus} -> {url}\n")
            try:
                result = fetch(url)
            except urllib.error.HTTPError as exc:
                sys.stderr.write(f"HTTPError {exc.code} for {url}\n")
                return 1
            except urllib.error.URLError as exc:
                sys.stderr.write(f"URLError {exc.reason} for {url}\n")
                return 1

            payload_bytes: bytes = result["payload_bytes"]  # type: ignore[index]
            # Validate JSON and capture summary before writing
            summary = summarize_payload(endpoint, payload_bytes)

            campus_tag = campus.lower()
            file_name = f"{args.tag}-{endpoint}-{campus_tag}.json"
            output_path = args.output_dir / file_name
            save_payload(output_path, payload_bytes)

            entry = {
                "campus": campus,
                "endpoint": endpoint,
                "url": url,
                "term": args.term,
                "year": args.year,
                "fetched_at_utc": timestamp,
                "status_code": result["status"],
                "content_length_bytes": len(result["raw_bytes"]),
                "payload_size_bytes": len(payload_bytes),
                "content_type": result["headers"].get("Content-Type"),
                "cache_control": result["headers"].get("Cache-Control"),
                "etag": result["headers"].get("ETag"),
                "content_encoding": result["headers"].get("Content-Encoding"),
                "sha256": hashlib.sha256(payload_bytes).hexdigest(),
                "output_path": str(output_path),
            }
            entry.update(summary)
            entries.append(entry)
            sys.stderr.write(
                f"Saved {endpoint} {campus} -> {output_path} ({summary.get('record_count', 0)} records)\n"
            )

    metadata = {
        "generated_at": timestamp,
        "script": "scripts/fetch_soc_samples.py",
        "tag": args.tag,
        "year": args.year,
        "term": args.term,
        "campuses": campuses,
        "entries": entries,
    }
    metadata_path.parent.mkdir(parents=True, exist_ok=True)
    with metadata_path.open("w", encoding="utf-8") as fh:
        json.dump(metadata, fh, indent=2)
        fh.write("\n")
    sys.stderr.write(f"Wrote metadata -> {metadata_path}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
