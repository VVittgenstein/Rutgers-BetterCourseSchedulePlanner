# RC Iteration Round 4 Recovered Local Baseline

## Authority

RC I Round 4 starts from the user-confirmed recovered project tree on 2026-07-17. The repository state at the start of the round is `master`, zero commits, and zero remotes. The deleted historical `.git`, `.ngagent`, old `AGENTS.md`, and NGAT state are not inputs and must not be restored.

The authoritative design inputs are `01`, `02`, `03`, and the five stable HumanTest images under `assets/round-04-human-test/`. The four named chat logs and the complete `HumanTest/` directory remain protected local evidence and are intentionally excluded from Git.

## Local Git identity

The round uses exactly two local commits:

1. this recovered baseline;
2. one RC I Round 4 product, test, and implementation-evidence commit.

The repository must keep zero remotes. No push, pull request, tag, upload, remote workflow, release, or deployment is authorized.

## Manifests

- `04a-rc-iteration-round-04-source-baseline-manifest.json` records every file staged for the recovered baseline commit. The two generated manifests are self-excluded from their own source hash set.
- `04b-rc-iteration-round-04-protected-assets-manifest.json` records local chat logs, HumanTest files, and the preserved historical ZIP that must not be staged or modified.

All HumanTest database work must use a hash-verified copy of the SQLite database, WAL, and SHM as a group. The original `HumanTest/` directory is read-only evidence for this round.
