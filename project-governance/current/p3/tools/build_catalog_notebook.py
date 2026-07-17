#!/usr/bin/env python3
"""Build and execute the P3 Catalog data-quality companion notebook.

The environment has no nbformat/nbclient. This bounded builder uses the official
nbformat v4 JSON shape, executes every code cell in one Python namespace, captures
stdout, validates the resulting structure, and writes the notebook atomically.
"""

from __future__ import annotations

import contextlib
import io
import json
import traceback
from pathlib import Path
from typing import Any


P3_ROOT = Path(__file__).resolve().parents[1]
NOTEBOOK_PATH = P3_ROOT / "notebooks" / "p3_catalog_data_quality.ipynb"


def markdown(source: str) -> dict[str, Any]:
    return {"cell_type": "markdown", "metadata": {}, "source": source.splitlines(keepends=True)}


def code(source: str) -> dict[str, Any]:
    return {"cell_type": "code", "execution_count": None, "metadata": {}, "outputs": [], "source": source.splitlines(keepends=True)}


def execute(cells: list[dict[str, Any]]) -> None:
    namespace: dict[str, Any] = {"__name__": "__notebook__"}
    execution_count = 0
    for cell in cells:
        if cell["cell_type"] != "code":
            continue
        execution_count += 1
        output = io.StringIO()
        try:
            with contextlib.redirect_stdout(output), contextlib.redirect_stderr(output):
                exec(compile("".join(cell["source"]), f"notebook-cell-{execution_count}", "exec"), namespace)
        except Exception:
            output.write(traceback.format_exc())
            cell["outputs"] = [{"output_type": "stream", "name": "stderr", "text": output.getvalue().splitlines(keepends=True)}]
            cell["execution_count"] = execution_count
            raise
        cell["outputs"] = [{"output_type": "stream", "name": "stdout", "text": output.getvalue().splitlines(keepends=True)}]
        cell["execution_count"] = execution_count


def validate(notebook: dict[str, Any]) -> None:
    if notebook.get("nbformat") != 4 or not isinstance(notebook.get("cells"), list):
        raise RuntimeError("invalid notebook root")
    code_cells = [cell for cell in notebook["cells"] if cell.get("cell_type") == "code"]
    if not code_cells or any(cell.get("execution_count") is None for cell in code_cells):
        raise RuntimeError("notebook has unexecuted code cells")
    if any(not cell.get("outputs") for cell in code_cells):
        raise RuntimeError("notebook code cell lacks output")
    headings = "".join("".join(cell.get("source", [])) for cell in notebook["cells"] if cell.get("cell_type") == "markdown")
    for required in ("## tl;dr", "## Context & Methods", "## Data", "## Results", "## Takeaways"):
        if required not in headings:
            raise RuntimeError(f"missing notebook section: {required}")


def main() -> None:
    cells = [
        markdown(
            "# P3 Rutgers Catalog Data Quality Evidence\n\n"
            "## tl;dr\n\n"
            "- The frozen run produced 21 usable Catalog payloads covering all 15 currently exposed Fall 2026 campus values plus six Summer 2026 main/online scopes. Campus D was added through frozen amendment-002.\n"
            "- The payloads contain 10,629 course rows, 22,069 section rows, and 30,804 meeting rows.\n"
            "- Campus is mandatory in section identity: 3,125 `(term,index)` values collide across campuses. Eighteen raw `(term,campus,index)` duplicates are semantically equivalent after order-insensitive comment normalization; zero contradictory duplicates were observed.\n"
            "- All 22 approved filter dimensions have a candidate source, but sparsity and reliability differ. Instructors expose names without stable IDs; 41 course strings have multiple objects, including 31 true supplement/credit/title variants.\n"
            "- Open evidence contains 42/42 successful observations across two rounds. All values are five-digit strings; 14/21 raw target pairs changed and 3/21 Catalog-intersected states changed.\n"
            "- The two-clock model is approved: public fixed 30 seconds, local default 30 seconds with range 3–3,600, and a 10-second related-batch target when actively watched.\n"
            ""
        ),
        markdown(
            "## Context & Methods\n\n"
            "This is a data-quality companion for the approved P2 contract. It reads only the Git-ignored raw responses named in the frozen request ledger and the deterministic profile emitted by `tools/profile_catalog_evidence.py`. It does not make network requests.\n\n"
            "### Key Assumptions\n\n"
            "- Counts describe the observed 2026 scopes, not an API SLA or all historical terms.\n"
            "- Empty arrays for currently exposed off-campus values are treated as observed empty Catalogs, not source failures.\n"
            "- Missing or sparse fields are not automatically defects; downstream three-valued matching must preserve uncertainty.\n"
            "- Open values are interpreted only by official `(term,campus)` Catalog-set intersection; orphan values are audited and never create Sections.\n"
            "- The notebook verifies frozen local evidence only and performs no Rutgers network requests.\n"
            ""
        ),
        code(
            "import csv, hashlib, json\n"
            "from pathlib import Path\n"
            "p3 = Path.cwd() / 'project-governance' / 'current' / 'p3'\n"
            "profile = json.loads((p3 / '04a-catalog-profile.json').read_text(encoding='utf-8'))\n"
            "with (p3 / '02-catalog-request-ledger.tsv').open(encoding='utf-8', newline='') as f:\n"
            "    ledger = list(csv.DictReader(f, delimiter='\\t'))\n"
            "success = [row for row in ledger if row['outcome'] == 'SUCCESS']\n"
            "assert len(success) == profile['successPayloads'] == 21\n"
            "for row in success:\n"
            "    raw = Path(row['raw_relative_path']).read_bytes()\n"
            "    assert hashlib.sha256(raw).hexdigest().upper() == row['body_sha256']\n"
            "print(f\"ledger rows={len(ledger)}; usable payloads={len(success)}; raw hashes verified={len(success)}\")\n"
            ""
        ),
        code(
            "open_profile = json.loads((p3 / '13a-open-profile.json').read_text(encoding='utf-8'))\n"
            "open_completion = json.loads((p3 / '22b-open-round2-completion.json').read_text(encoding='utf-8'))\n"
            "open_decision = json.loads((p3 / '18a-open-review-decision-001.json').read_text(encoding='utf-8'))\n"
            "open_contract = json.loads((p3 / '23b-shared-open-final-contract.json').read_text(encoding='utf-8'))\n"
            "with (p3 / '11-open-request-ledger.tsv').open(encoding='utf-8', newline='') as f:\n"
            "    open_ledger = list(csv.DictReader(f, delimiter='\\t'))\n"
            "assert len(open_ledger) == open_profile['successfulAttempts'] == open_completion['attempts']['successful'] == 42\n"
            "assert sum(row['round'] == '1' for row in open_ledger) == 21\n"
            "assert sum(row['round'] == '2' for row in open_ledger) == 21\n"
            "assert all(row['outcome'] == 'SUCCESS' and row['http_status'] == '200' for row in open_ledger)\n"
            "for row in open_ledger:\n"
            "    raw = Path(row['raw_relative_path']).read_bytes()\n"
            "    assert hashlib.sha256(raw).hexdigest().upper() == row['body_sha256']\n"
            "assert open_profile['status'] == 'ROUND_2_COMPLETE_OFFICIAL_JOIN_VALIDATED'\n"
            "assert open_profile['valueTypeCounts']['string'] == open_profile['stringLengthCounts']['5'] == 302125\n"
            "assert open_profile['totals']['officialJoinPassObservations'] == 42\n"
            "assert open_profile['totals']['changedRawScopePairs'] == 14\n"
            "assert open_profile['totals']['changedEffectiveScopePairs'] == 3\n"
            "assert open_decision['proposedClockMapping']['status'] == 'APPROVED_BY_USER'\n"
            "assert open_contract['status'] in {'P3_SHARED_OPEN_CONTRACT_FREEZE_CANDIDATE', 'FROZEN_P3_SHARED_OPEN_CONTRACT'}\n"
            "print('Open: 42/42 hashes verified; 2 rounds; 302125 five-digit values')\n"
            "print('Round-pair changes: raw=14/21; Catalog-intersected=3/21')\n"
            "print('Clock mapping: public=30 fixed; local=30 range 3-3600; active-watch target=10')\n"
            ""
        ),
        markdown("## Data\n\nEach row below is one successfully cached term/campus Catalog payload. Four Fall off-campus scopes legitimately returned zero courses."),
        code(
            "columns = ['term_id','campus','courses','sections','meetings','decoded_bytes']\n"
            "print(' | '.join(columns))\n"
            "print('-' * 88)\n"
            "for row in profile['scopeSummary']:\n"
            "    print(' | '.join(str(row[col]) for col in columns))\n"
            ""
        ),
        markdown("## Results\n\n### Composite section identity survives raw duplication only with explicit normalization"),
        code(
            "for item in profile['identity']['summary']:\n"
            "    print(f\"{item['metric']}: {item['count']} [{item['severity']}]\")\n"
            ""
        ),
        markdown("### Delivery and meeting fields are rich but cannot be collapsed into one enum"),
        code(
            "print('Top raw meeting mode tuples:')\n"
            "for row in profile['deliveryRawValues'][:15]:\n"
            "    print(f\"{row['meeting_mode_code']:>3} {row['meeting_mode_desc']:<34} meetings={row['meeting_count']} scopes={row['scope_count']}\")\n"
            "print('Meeting-day tokens:', {row['meeting_day']: row['meeting_count'] for row in profile['meetingDayValues']})\n"
            "print('Time shapes:', profile['timeShapeCounts'])\n"
            ""
        ),
        markdown("### Every approved filter has a candidate source, with explicit sparsity"),
        code(
            "for row in profile['filterSourceProfile']:\n"
            "    pct = 100 * row['non_empty_rate']\n"
            "    print(f\"{row['filter_id']} {row['label']:<28} {row['non_empty']}/{row['denominator']} ({pct:6.2f}%) — {row['evidence_note']}\")\n"
            ""
        ),
        markdown("### Two Open rounds validate target-local intersection while ETag remains audit-only"),
        code(
            "print('Open request duration (ms):', open_profile['requestDurationMs']['all'])\n"
            "print('Headers:', open_profile['headerPresence'])\n"
            "print('Transition totals:', {k: open_profile['totals'][k] for k in [\n"
            "    'changedRawScopePairs', 'changedEffectiveScopePairs',\n"
            "    'effectiveAddedSummedNonDistinct', 'effectiveRemovedSummedNonDistinct']})\n"
            "print('Official join:', open_profile['officialJoinContract'])\n"
            ""
        ),
        markdown(
            "## Takeaways\n\n"
            "1. Preserve `(term,campus,index)` and reject bare index or `(term,index)` joins. Collapse only semantically equivalent duplicate raw sections; quarantine contradictory duplicates.\n"
            "2. Model a course-centered group separately from offering variants. A single course string may have supplement/credit/title variants or multiple disjoint section chunks.\n"
            "3. Use official `sectionCourseType` for modality and retain every raw meeting-mode tuple. Synchronicity needs a conservative oracle; generic online remains unspecified unless independent source facts establish sync/async.\n"
            "4. Treat empty Catalogs, sparse prerequisites/core/permission/eligibility, missing instructor IDs, and by-arrangement meetings as explicit data states. Do not replace them with fallback data or binary assumptions.\n"
            "5. The 42 Open observations close the P3 evidence portion: the official target-local set intersection, two-clock mapping, empty/failure safety, scheduler, backoff, observations, counters, and episode semantics are consolidated in the final shared Open contract.\n"
            "6. `Cache-Control: max-age=30` is observed response freshness metadata, not a Rutgers seat-publication SLA. The product may target <=1 second from an accepted valid observation to server fanout, but it must not promise a real seat change will alert within 30 seconds.\n"
            ""
        ),
    ]

    execute(cells)
    notebook = {
        "cells": cells,
        "metadata": {
            "kernelspec": {"display_name": "Python 3", "language": "python", "name": "python3"},
            "language_info": {"name": "python", "version": "3"},
            "rbcsp_validation": {"method": "bounded custom nbformat-v4 executor", "status": "executed_top_to_bottom"},
        },
        "nbformat": 4,
        "nbformat_minor": 5,
    }
    validate(notebook)
    NOTEBOOK_PATH.parent.mkdir(parents=True, exist_ok=True)
    temporary = NOTEBOOK_PATH.with_suffix(".ipynb.tmp")
    temporary.write_text(json.dumps(notebook, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    temporary.replace(NOTEBOOK_PATH)
    print(f"notebook={NOTEBOOK_PATH}")
    print(f"code_cells={sum(1 for cell in cells if cell['cell_type'] == 'code')}")
    print("validation=executed_top_to_bottom")


if __name__ == "__main__":
    main()
