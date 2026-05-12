# Stage P · Public Tags and Releases Closeout (task-014)

> Execution report for task-014. This task deleted the single GitHub
> Release object on `origin` (id `264993969`, `tag_name` `Release-0122`,
> display name `Release-0121`) together with its sole asset
> (`bcsp-20260122.zip`), and then deleted all 8 remote Git tags on
> `origin`. It made **no changes** to `refs/heads/main`, did **not**
> force-push, did **not** modify any source or product file, and did
> **not** push local `dev` back to `origin` under any refspec.
>
> Authority and scope are pinned by the Main Agent (see §1.1).

---

## 1. Closeout metadata

### 1.1 Authority (verbatim from pinned Main-Agent context for task-014)

- "Human has explicitly approved task-014 after the task-012 audit."
- "Current remote heads should be exactly `refs/heads/main` at
  `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4`; preserve that head and do
  not force-push or alter `main`."
- "Current remote tags should initially be 8: `Finalrelease`,
  `First-release`, `Release`, `Release-0118`, `Release-0121`,
  `Release-0122`, `Release-1124`, `Second-release`."
- "GitHub Release object from audit: `id=264993969`,
  `tag_name=Release-0122`, display name `Release-0121`, asset
  `bcsp-20260122.zip`."
- "Attempt to delete the GitHub Release object only through available
  authenticated tooling. Do not print, log, commit, or expose tokens.
  Safe auth sources may include `GH_TOKEN`/`GITHUB_TOKEN`, an
  already-authenticated `gh` CLI if present, or Git Credential Manager
  read into process memory without echoing secrets."
- "If the release object is deleted successfully, delete all 8 tags,
  with `Release-0122` after release deletion. If no authenticated
  release cleanup is available or release deletion fails for auth
  reasons, do not delete `Release-0122` because the live Release object
  still references it; delete the other 7 stale tags and document the
  remaining Release URL, API DELETE endpoint, asset, tag, and exact
  manual/API action needed."
- "Verify final heads still only `main`, tags reflect the outcome (0 if
  release deletion succeeded, or exactly `Release-0122` if release
  deletion was unavailable), and no source/product files changed. Do
  not push local `dev` back to origin."

This task obeys all of the above. The release object was deleted
successfully (§3), all 8 tags were deleted with `Release-0122` last
(§4), and post-flight verification (§5) confirms heads=`main` only and
tags=0.

### 1.2 Repository and worktree

| Field | Value |
|---|---|
| Remote URL (fetch / push) | `https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner` |
| Worktree absolute path | `Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\task-014` |
| Branch (worktree HEAD) | `feature/task-014` |
| Task | `task-014` (Stage P · close stale public tags and releases) |
| Depends on | `task-013` (`.orchestrator/stage-p/06-public-branch-closeout.md`) — branch closeout |
| Underlying audit | `task-012` (`.orchestrator/stage-p/05-public-remote-surface-audit.md`) — exact deletion plan |
| Approval source | Pinned Main-Agent context for task-014 (§1.1 above) |
| Execution session timestamp (UTC) | `2026-05-12T00:26:07Z` |
| Worktree HEAD at start of task | `b0fd60be2ad459c2da13e24dd912cc0a2369e938` (matches `main` at task-013 merge commit) |
| Reported `origin/main` SHA at preflight | `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4` |

### 1.3 Tooling and constraints

| Tool / capability | Used? | Notes |
|---|---|---|
| `git fetch origin --prune --tags` | Yes | Preflight refresh of remote-tracking refs and tags. Non-mutating to the remote. |
| `git ls-remote --heads origin` | Yes | At preflight and post-verification. |
| `git ls-remote --tags origin` | Yes | At preflight (initial inventory of 8 tags) and post-verification (final 0 tags). |
| `git push origin --delete refs/tags/<name>` | Yes (×8) | Form A from audit §5.3: one tag per push, sequential. Non-force; fails closed if the named ref does not exist. |
| `git push origin --force` / `--force-with-lease` | **Not used** | No force push of any kind. `main` was never pushed. |
| `git push origin dev` (re-push local dev) | **Not used** | Explicitly forbidden by §1.1; not invoked under any refspec. |
| `git push origin --delete <branch>` | **Not used** | Branch deletion was task-013's job. |
| `gh` CLI | **Not used** / not available in session. Confirmed via `Get-Command gh` returning no source. | — |
| Environment variable `GH_TOKEN` | **Not used** / not set. | — |
| Environment variable `GITHUB_TOKEN` | **Not used** / not set. | — |
| Git Credential Manager (`credential.helper=manager`) | **Used** | A credential for `host=github.com` was retrieved by piping `protocol=https\nhost=github.com\n\n` into `git credential fill`. The secret was read into a single PowerShell string variable, used **once** as an `Authorization: Bearer …` header inside the same script block, then cleared (`$token = $null; [System.GC]::Collect()`). The token value was never echoed to stdout/stderr, never written to a file, never embedded in a `git` or `curl` command line, and is not present in this report. |
| GitHub REST API (authenticated, write) | Yes | One `DELETE /repos/{owner}/{repo}/releases/264993969` call via `Invoke-WebRequest` with the `Authorization` header set in-process. Asset deletion was implicit (per documented GitHub behavior: deleting a release also deletes its uploaded assets). |
| GitHub REST API (authenticated, read) | Yes | `GET /releases/264993969` once before the DELETE (preflight) and once after (verification); `GET /releases` after the DELETE to confirm the listing collapsed to `[]`. |
| GitHub REST API (anonymous, read) | Yes | Independent re-verification of the post-state from a header-set without `Authorization`, mirroring the audit's anonymous probe in `.orchestrator/stage-p/05-public-remote-surface-audit.md` §3.3. |
| HTTP HEAD on release HTML page and asset URL | Yes | Browser-facing confirmation that the user-visible surfaces 404. |
| Rate limit at probe time | Comfortable | Authenticated `GET` reported `X-RateLimit-Remaining: 4983` (out of the 5,000/hour authenticated bucket); no risk of throttling for the operations below. |

---

## 2. Preflight verification (before any mutation)

Performed in order at the start of execution, before issuing any DELETE
or any `git push`.

### 2.1 Refresh remote-tracking refs and tags

`git fetch origin --prune --tags` ran with no output (no remote-tracking
refs required pruning; no new tags to fetch). Exit code `0`.

### 2.2 Confirm `origin/main` matches the approved SHA

```text
$ git ls-remote --heads origin
9c93170c5dc8e3b767312b4877d87ee0d2ce19e4    refs/heads/main
```

`origin/main` is exactly `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4`,
matching the SHA pinned by the Main Agent and the SHA preserved by
task-013. **The "stop if mismatch" preflight gate did not trigger.**
There was exactly **1** remote head and it was `main`; the post-state
required by task-013 was intact at the start of task-014.

### 2.3 Confirm the initial 8 tags match audit §3.2

```text
$ git ls-remote --tags origin
81a4818ce58bba4810be10032ae785f328859996    refs/tags/Finalrelease
81a4818ce58bba4810be10032ae785f328859996    refs/tags/First-release
81a4818ce58bba4810be10032ae785f328859996    refs/tags/Release
2d762179025e68ee05f853c3e7b5e8b43837893c    refs/tags/Release-0118
b650d81cdd8e11e08f145a44352b13c04c37b8f9    refs/tags/Release-0121
b650d81cdd8e11e08f145a44352b13c04c37b8f9    refs/tags/Release-0122
81a4818ce58bba4810be10032ae785f328859996    refs/tags/Release-1124
81a4818ce58bba4810be10032ae785f328859996    refs/tags/Second-release
```

The 8 tag names and their tip SHAs exactly match
`.orchestrator/stage-p/05-public-remote-surface-audit.md` §3.2 (the
six-tag cluster at `81a4818…`, the orphan tag `Release-0118` at
`2d76217…`, and the twin tags `Release-0121` / `Release-0122` both at
`b650d81…`). Nothing was created, retargeted, or moved between the
audit timestamp and this preflight. No new tag was introduced.

### 2.4 Authenticated tooling probe (no secrets echoed)

The pinned context lists the three permitted authenticated paths in
priority order. Each was probed:

1. `gh` CLI — `Get-Command gh -ErrorAction SilentlyContinue` returned
   nothing (`gh: not found`). Skipped.
2. Environment variables — `$env:GH_TOKEN` and `$env:GITHUB_TOKEN` were
   both empty. The probe printed only `present`/`not set` and a length
   if present. Skipped.
3. Git Credential Manager — `git config --get credential.helper`
   returned `manager`, so GCM is active. A probe `git credential fill`
   with the input `protocol=https\nhost=github.com\n\n` returned
   `username_present=True`, `password_present=True`,
   `password_length=40` (typical of a classic GitHub PAT or an OAuth
   token; the format is suitable for `Authorization: Bearer …`). The
   probe did **not** print the password.

GCM was therefore the authenticated path used by §3. No secret was
emitted to stdout, stderr, a file, or a process command line at any
step.

### 2.5 Token capability check (read-only, authenticated)

Before the DELETE, an authenticated `GET /repos/.../releases/264993969`
was issued. The response confirmed:

| Field | Value | Source |
|---|---|---|
| HTTP status | `200` | Response status line |
| `id` | `264993969` | Response body |
| `tag_name` | `Release-0122` | Response body |
| `name` | `Release-0121` | Response body |
| `draft` | `false` | Response body |
| `prerelease` | `false` | Response body |
| `assets.length` | `1` | Response body |
| `assets[0].id` | `344432236` | Response body |
| `assets[0].name` | `bcsp-20260122.zip` | Response body |
| `assets[0].size` | `245913` bytes | Response body |
| `X-OAuth-Scopes` | `gist, repo, workflow` | Response header |
| `X-RateLimit-Remaining` | `4983` | Response header |

The `repo` scope subsumes `public_repo`, which is the minimum scope
GitHub requires to delete a Release in a public repository, so the
DELETE in §3 is expected to succeed. All fields matched the values
recorded in the audit
(`.orchestrator/stage-p/05-public-remote-surface-audit.md` §3.3)
verbatim — no drift between the audit and this execution.

---

## 3. GitHub Release object deletion (AC-003)

### 3.1 Method

A single REST API call was issued from inside the same in-memory script
block that held the GCM-supplied token. The call was built with
`Invoke-WebRequest` (PowerShell native HTTP client; does not spawn a
child process, so the `Authorization` header is not exposed via
process-listing or audit log of `curl` invocations):

```text
DELETE https://api.github.com/repos/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/releases/264993969
Headers:
  Accept: application/vnd.github+json
  Authorization: Bearer <REDACTED — held in process memory only>
  X-GitHub-Api-Version: 2022-11-28
  User-Agent: ngagent-task-014
```

This is the call documented in audit §5.4 "Option A — delete the
release object (asset is deleted with it)". Option B (asset-only
deletion) was **not** invoked; per the audit it is not the recommended
path and would have left the Release object orphaned.

### 3.2 Result

| Probe | Result | Expected |
|---|---|---|
| `DELETE /releases/264993969` HTTP status | `204` | `204 No Content` per GitHub REST API spec |
| Response body length | `0` bytes | `0` (the API returns no body on success) |
| Subsequent authenticated `GET /releases/264993969` | `404` | `404 Not Found` — the release object no longer exists |
| Subsequent authenticated `GET /releases` | `200`, JSON `[]`, count `0` | `[]` — there are no releases at all |
| Anonymous `GET /releases/264993969` (no `Authorization`) | `404` | `404` — public view also gone |
| Anonymous `GET /releases` (no `Authorization`) | `200`, JSON `[]`, count `0` | `[]` |
| Anonymous `HEAD https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/releases/tag/Release-0122` | `404` | `404` — release HTML page gone |
| Anonymous `HEAD https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/releases/download/Release-0122/bcsp-20260122.zip` | `404` | `404` — asset download URL gone (GitHub deletes assets along with their parent release) |

The Release object `264993969` and its sole asset
(`id=344432236`, file `bcsp-20260122.zip`, ≈240 KiB) are both gone from
the public surface. The repository's Releases tab is empty.

### 3.3 Token hygiene

The token retrieved from GCM was:
- **Never** written to a file (including this report).
- **Never** echoed to stdout or stderr.
- **Never** passed on a `curl` or `git` command line (which would be
  visible to other processes via `tasklist`/`ps`-style inspection while
  the call was in-flight).
- **Never** persisted across script blocks. After the DELETE plus
  verification calls completed, the holding variable was set to `$null`
  and `[System.GC]::Collect()` was invoked.
- **Not** modified, reissued, or revoked by this task. GCM's stored
  credential is unchanged.

This report contains the token's character length only (40) — a generic
property consistent with GitHub's public documentation of token
formats; it leaks no value-bearing material.

### 3.4 Branch-protection / branch-default assertions

This task did not query `GET /repos/{owner}/{repo}/branches/main/protection`
because the operation under §3 is a release deletion, not a branch
operation. `origin/main` was not touched in any way by this task, and
the only `git push` operations were `--delete` against tag refs (§4),
which do not interact with branch protection.

---

## 4. Tag deletion sequence (AC-002)

Per the pinned ordering rule (audit §5.5 "Critical ordering rule": if
`refs/tags/Release-0122` were deleted **before** the release object
`264993969`, the release object would be left orphaned and the
download URL might keep resolving from cache for a brief window),
`Release-0122` was deleted **last**, after the §3 DELETE returned `204`
and the subsequent GET returned `404`. The other seven tags were
deleted first, in audit §5.3 listed order. Form A (one ref per push)
was used for safe single-ref deletion and per-step logging.

Each command was the canonical `git push origin --delete refs/tags/<name>`
form (fully qualified to disambiguate tag namespace from branch
namespace). All eight commands returned exit code `0` and a
`[deleted]` confirmation line from the remote.

### 4.1 Sequence

| # | Command | Exit | Remote reply | Notes |
|---:|---|---:|---|---|
| 1 | `git push origin --delete refs/tags/Finalrelease` | `0` | `- [deleted]         Finalrelease` | Cluster tag at `81a4818…`. Not referenced by any release. |
| 2 | `git push origin --delete refs/tags/First-release` | `0` | `- [deleted]         First-release` | Cluster tag at `81a4818…`. Not referenced by any release. |
| 3 | `git push origin --delete refs/tags/Release` | `0` | `- [deleted]         Release` | Cluster tag at `81a4818…`. Not referenced by any release. |
| 4 | `git push origin --delete refs/tags/Release-0118` | `0` | `- [deleted]         Release-0118` | Orphan tag at `2d76217…`. Not referenced by any release. |
| 5 | `git push origin --delete refs/tags/Release-0121` | `0` | `- [deleted]         Release-0121` | Twin tag at `b650d81…`. Not the `tag_name` of the (now-deleted) release. Audit §3.3 confirmed the release's `tag_name` was `Release-0122`, not `Release-0121`, so this tag deletion was safe independent of release deletion ordering. |
| 6 | `git push origin --delete refs/tags/Release-1124` | `0` | `- [deleted]         Release-1124` | Cluster tag at `81a4818…`. Not referenced by any release. |
| 7 | `git push origin --delete refs/tags/Second-release` | `0` | `- [deleted]         Second-release` | Cluster tag at `81a4818…`. Not referenced by any release. |
| 8 | `git push origin --delete refs/tags/Release-0122` | `0` | `- [deleted]         Release-0122` | Twin tag at `b650d81…`. **Deleted last**, only after §3 confirmed the release object had been removed (DELETE→204, GET→404). |

### 4.2 Local prune

After step 8, `git fetch origin --prune --prune-tags` was run to
synchronise the local tag namespace with the now-empty remote tag
namespace. The fetch reported eight `[deleted]` lines (one per pruned
local tag reference, matching the eight remote deletions above) and
no new heads. The local tracking view is consistent with the remote.

### 4.3 Form choice

Form A (one ref per command) was selected over Form B (batched refspec
push) for the same reason as task-013: it produces per-step exit codes
and per-step remote replies, making partial failure recoverable and
easy to log. The audit (§5.3 / §5.5) explicitly permits either form
provided the `Release-0122` / release-object ordering is respected.

---

## 5. Post-flight verification (AC-004)

All assertions performed after step 4.2 and **before** writing this
report.

### 5.1 Remote heads

```text
$ git ls-remote --heads origin
9c93170c5dc8e3b767312b4877d87ee0d2ce19e4    refs/heads/main

$ git ls-remote --heads origin | wc -l
1
```

Exactly one remote head: `refs/heads/main` at
`9c93170c5dc8e3b767312b4877d87ee0d2ce19e4`. **Unchanged** from the
preflight snapshot in §2.2 and from the SHA pinned by both task-013
and task-014. `main` was not force-pushed, not retargeted, not
deleted, not modified. The post-state required by AC-004 first
clause holds.

### 5.2 Remote tags

```text
$ git ls-remote --tags origin
(no output)

$ git ls-remote --tags origin | wc -l
0
```

Zero remote tags. The pinned context's "0 if release deletion
succeeded" branch is the realised outcome. The post-state required
by AC-004 second clause holds.

### 5.3 Release surface (authenticated + anonymous)

| Surface | Authenticated probe | Anonymous probe |
|---|---|---|
| `GET /repos/.../releases/264993969` | `404` | `404` |
| `GET /repos/.../releases` (listing) | `200`, `[]`, count `0` | `200`, `[]`, count `0` |
| `HEAD https://github.com/.../releases/tag/Release-0122` (HTML) | — | `404` |
| `HEAD https://github.com/.../releases/download/Release-0122/bcsp-20260122.zip` (asset) | — | `404` |

The GitHub Releases tab is empty. The asset URL is gone. No
release-side surface remains that could leak repository contents,
versions, or asset hashes.

### 5.4 No source/product modifications

`git status --short` reported a clean working tree before this report
was written. The only file modified by this task is the present
report (`.orchestrator/stage-p/07-public-tags-releases-closeout.md`),
which is within the allowed write paths and outside every forbidden
write path (`api`, `frontend`, `workers`, `notifications`, `configs`,
`data`). No source, fixture, or product file was touched. No file
was deleted. The post-state required by AC-006 holds.

### 5.5 No `git push` of local `dev`

`git push origin dev` was **not** invoked under any refspec. The only
`git push` operations issued by this task are the eight
`git push origin --delete refs/tags/<name>` lines in §4. There were
no force pushes and no pushes to `refs/heads/*`. The post-state
required by the second sentence of AC-006 ("do not push local dev
back to origin") holds.

---

## 6. Remaining blockers

**None.**

- The pinned context's "release deletion succeeded" branch is the
  realised outcome.
- The pinned context's "release deletion unavailable" fallback branch
  was not entered: GCM provided a credential with `repo` scope, the
  authenticated DELETE returned `204`, and the asset was removed
  implicitly with the release.
- There is no remaining manual/API action needed for tags or releases.
  The `gh` release URL (`/releases/tag/Release-0122`) and the asset
  URL (`/releases/download/Release-0122/bcsp-20260122.zip`) both
  return `404` and the Releases listing is `[]`.
- `main` is preserved at `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4`.
- The fallback documentation that would have been required if the
  release object had been undeletable (remaining release URL, API
  DELETE endpoint, asset, tag, manual action) is therefore not
  applicable and is intentionally **not** included; including it
  would be misleading.

If a future audit task wishes to re-verify the post-state, the exact
checks are in §5 of this report and §5.6 of the audit
(`.orchestrator/stage-p/05-public-remote-surface-audit.md`); both
match.

---

## 7. Acceptance criteria mapping

| ID | Criterion (abbreviated) | Met? | Evidence section |
|---|---|---|---|
| AC-001 | Task-012 audit consulted; explicit human approval confirmed before any deletion. | ✓ | §1.1 (Main-Agent pinned context, including the literal phrase "Human has explicitly approved task-014 after the task-012 audit"); §2 (audit's `tag_name`, asset id, asset name, and SHA bands re-verified live against §3.2 / §3.3 of the audit). |
| AC-002 | All 8 stale remote tags deleted (no justified keeper); `main` untouched. | ✓ | §4 (eight `[deleted]` confirmations, exit `0` each); §5.1 (`main` SHA unchanged); §5.2 (`git ls-remote --tags origin` is empty). |
| AC-003 | GitHub Release object cleanup attempted via available authenticated tooling; success or detailed remaining-blocker documentation, no faked success. | ✓ | §1.3 (GCM is the available authenticated path; `gh`, `GH_TOKEN`, `GITHUB_TOKEN` confirmed unavailable); §2.5 (capability check: token scopes `repo` cover `public_repo`); §3 (DELETE→`204`; GET→`404`; listing→`[]`; asset HEAD→`404`; release HTML HEAD→`404`); §6 (no remaining blockers; fallback documentation not applicable). |
| AC-004 | `git ls-remote --heads origin` returns only `main`; `git ls-remote --tags origin` reflects the intended final state. | ✓ | §5.1 (heads exactly 1 line: `main` at `9c93170c…`); §5.2 (tags exactly 0 lines). |
| AC-005 | `.orchestrator/stage-p/07-public-tags-releases-closeout.md` documents tag-deletion results, release status, remaining blockers, with no secret values. | ✓ | This file. §3.3 records token hygiene: the token's value is not present anywhere in this report; only the generic length (40) is mentioned. No PAT, no OAuth token, no asset signature, no internal-only URL beyond the public ones already enumerated in the audit. |
| AC-006 | No source/product files modified; local `dev` not pushed to `origin`. | ✓ | §5.4 (clean working tree apart from this report; allowed writes only; forbidden paths untouched); §5.5 (no `git push origin dev` and no force push of any kind). |

---

## 8. Open-ended notes for the Main Agent / future audits

- The audit's §5.6 post-flight checklist is fully satisfied: heads is
  exactly the single `main` line at the pinned SHA, tags are `0`, the
  releases listing is `[]`, the asset URL is `404`, and the release
  HTML page is `404`. A future audit can re-run those four commands
  verbatim and expect the same results.
- `public-main-candidate` (audit §4.3) was already removed by task-013
  and is therefore absent from this task's preflight. The pinned
  task-014 context's "exactly `main`" requirement is consistent with
  the task-013 outcome; no special handling was needed.
- The 40-character token length suggests GCM is holding either a
  classic-style PAT or an OAuth token. Modern fine-grained PATs begin
  with the `github_pat_` prefix and are longer than 40 characters;
  neither this report nor any tool invocation discloses which kind it
  is, and the user is free to rotate or revoke the credential
  independently.
- The repository's other public-surface configurables (`has_issues`,
  `has_wiki`, `has_pages`, `has_discussions`, `has_projects`) recorded
  in audit §1.1 were **not** touched by this task. Adjusting those is
  out of scope for tag/release closeout.
- If the user wishes to revoke or rotate the GCM-stored token after
  this task, doing so will not affect the deletions performed here
  (they are already realised in GitHub's state) and is unrelated to
  the public surface.
