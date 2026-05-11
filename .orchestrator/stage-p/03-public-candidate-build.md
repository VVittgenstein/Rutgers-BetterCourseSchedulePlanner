# Stage P · Public Candidate Build Report

> Stage P task-010 construction report. Authority over construction details
> (branch name, commit SHAs, push result) for the remainder of Stage P, but
> subordinate to the Stage P design documents on policy:
> `.orchestrator/stage-p/01-public-divergence-and-exposure-policy.md` (the
> include/exclude/redaction policy) and
> `.orchestrator/stage-p/02-public-commit-sequence.md` (the construction
> plan). This task **executes** the design; it does not amend it.
>
> Scope discipline: this task's only write inside the worktree is the present
> file. The two sanitized commits on the public candidate branch (P-COMMIT-1
> and P-COMMIT-2) live on a separate Git branch (`public-main-candidate`),
> not on the task branch `feature/task-010`.
>
> Supersession note (AC-002): Stage P task-009's
> `02-public-commit-sequence.md` §1 contains the table row *"Pushed?: Not by
> this task or by the construction task. Pushing the candidate branch to the
> remote is a separate, human-approved cutover action."* That line is
> **superseded for the candidate branch** by task-010 spec v2 acceptance
> criterion AC-002 and by the Main-Agent pinned context for this task:
> the candidate branch is pushed to `origin/public-main-candidate` as a
> non-default branch for review. The supersession is narrow: cutover
> (default-branch change, force-push of `main`, deletion of any branch,
> deletion of local `far/`) remains explicitly human-gated and is NOT
> performed by this task.

---

## 1. Candidate branch identity (AC-001, AC-005)

| Field | Value |
|---|---|
| **Candidate branch name** | `public-main-candidate` |
| **Local branch tip** | `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4` (`9c93170`) |
| **Remote branch tip** | `origin/public-main-candidate` = `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4` (`9c93170`, identical to local) |
| **Base commit** | `98748e1466ec55e93115f651a6a84f84e227daa9` (`98748e1`, `Merge pull request #206 from VVittgenstein/clean-public-artifacts-20260509`) — `origin/main` tip at construction time, unchanged by this task |
| **Construction host** | The same local clone hosting `origin/main` and `origin/dev`. `git fetch origin --prune` was run immediately before branching; then `git switch -c public-main-candidate 98748e1466ec55e93115f651a6a84f84e227daa9` from this clone's `feature/task-010` worktree (`Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\task-010`). The nested clone `far/Rutgers-BetterCourseSchedulePlanner` was not used as the host and was not read. |
| **Construction strategy** | `P-BUILD-1` (build-on-main): branch from `origin/main` tip and add sanitized commits on top. The rejected alternative (orphan replay) was explicitly considered and rejected by Stage P task-009; that decision is honored here. |
| **Commits since base** | exactly 2 (`f819d3c`, `9c93170`); see §2. |
| **Author and committer identity for both new commits** | `VVittgenstein <adrianyuanzhengze@gmail.com>` (the human's published Git identity, the same identity that authored `c55352b`, `45455c1`, `7abd5f1`, and `e770bf2` on the existing public history). Confirms `P-RED-1`. No ngagent, executor, reviewer, or model identifier appears in either commit's author or committer fields. |

### 1.1 Source refs consumed

| Reference | SHA at construction time | Role |
|---|---|---|
| `origin/main` tip | `98748e1466ec55e93115f651a6a84f84e227daa9` | Base for `P-BUILD-1`. Not modified. |
| `origin/dev` tip | `92b62c2da990f69253c205f82801fb54cf06b589` | Source of the runtime-hygiene ignore intent (`data/refresh_queue.json`, `scripts/poller_checkpoint.json`, `.worktrees/`) merged into P-COMMIT-1 and P-COMMIT-2. Not modified. |
| Merge base of `origin/main` and `origin/dev` | `2d762179025e68ee05f853c3e7b5e8b43837893c` | Audit reference; not a construction input. |
| `origin/main:.gitignore` blob (carried at base) | `b3522eda3134bef75f76c082c183a80e5b4b399a` | Starting `.gitignore`; base tree. |
| `origin/dev:.gitignore` blob (intent source) | `fe8ce7f58e173f1533cc9bc6e3ab07ed947cc283` | Source of the dev-side runtime-hygiene and `.worktrees/` additions whose intent is merged in (not cherry-picked from). |
| Candidate `.gitignore` blob (post P-COMMIT-2) | `e201b7a793c37e882ea18a707f37166755d19baf` | Final `.gitignore` on the candidate; differs from `b3522eda…` only by **seven appended lines total** (2 runtime patterns added by P-COMMIT-1 plus the 5-line defensive block added by P-COMMIT-2), documented in §3. P-COMMIT-2's own diff is `+5 / −0`; the candidate-vs-`origin/main` cumulative diff is `+7 / −0` because P-COMMIT-1 contributes the other 2 inserted lines. |
| `scripts/poller_checkpoint.json` blob on `origin/main` (removed from index by P-COMMIT-1) | `e7b0fe8e2fc89091cd5194d3a850a8c2a1d3021e` | Removed via `git rm --cached`; on-disk copy preserved as ignored. |

### 1.2 Visible-history math (AC-004 anticipation)

Commits since the merge base `2d76217` on the candidate branch:

```
9c93170  chore: add defensive ignores for nested clone and local agent scaffolding   <- P-COMMIT-2 (this task)
f819d3c  chore: untrack stray runtime checkpoint and tighten runtime ignores          <- P-COMMIT-1 (this task)
98748e1  Merge pull request #206 from VVittgenstein/clean-public-artifacts-20260509   <- base (inherited)
7abd5f1  Clean public repository surface                                              <- inherited
45455c1  Clean public repository surface                                              <- inherited
6863093  Merge pull request #204 from VVittgenstein/clean-public-surface              <- inherited
c55352b  Clean public repository surface                                              <- inherited
b650d81  Merge pull request #203 from VVittgenstein/auto-refresh-tasks                <- inherited
e770bf2  auto-refresh-tasks                                                           <- inherited
```

Total: 9 task-style commits visible since the merge base (7 inherited + 2 new). Six of the nine are non-merge commits. None of the nine has been rebased, reworded, or squashed by this task; the seven inherited commits retain their original SHAs and author identities. This satisfies AC-004's "preserve multiple normal commits with meaningful public commit messages rather than one squash commit."

---

## 2. The two sanitized commits

### 2.1 P-COMMIT-1 · `f819d3c7d983e7333cdd3888fd9bd9a40a12d764`

| Field | Value |
|---|---|
| **SHA (full)** | `f819d3c7d983e7333cdd3888fd9bd9a40a12d764` |
| **SHA (short)** | `f819d3c` |
| **Parent** | `98748e1466ec55e93115f651a6a84f84e227daa9` (base) |
| **Author / committer** | `VVittgenstein <adrianyuanzhengze@gmail.com>` (both) |
| **Subject** | `chore: untrack stray runtime checkpoint and tighten runtime ignores` |
| **Body (verbatim)** | `scripts/poller_checkpoint.json is a per-environment poller state file, not a tracked artifact, and data/refresh_queue.json is the symmetric per-environment queue state.` |
| **Files changed** | 2 (`.gitignore` modified, `scripts/poller_checkpoint.json` removed from index) |
| **Insertions / deletions** | `+2 / −14` (the 14 deletions are the lines of the removed JSON blob, not edits to other files) |
| **Tree mutations** | Append two lines to the existing "Local database artifacts" block of `.gitignore` in this order: `data/refresh_queue.json`, `scripts/poller_checkpoint.json`. Remove `scripts/poller_checkpoint.json` from the index via `git rm --cached` (no on-disk delete). All other paths unchanged. |
| **Conformance** | Satisfies `P-EXC-4` (local runtime / generated state — excluded from tracking) for the previously-leaked `scripts/poller_checkpoint.json` and aligns with `P-INC-6` clauses (b) and (c) for the dev-side ignore intent. Does **not** touch any path outside `.gitignore` and the one removed index entry; in particular, no `.orchestrator/`, `AGENTS.md`, `docs/archive/`, `reports/`, `notebooks/`, `far/`, product source, tests, schema, migrations, or package manifests. Subject and body contain no task ID, run ID, model name, or reviewer name (`P-RED-2`). Author and committer identities are the human's published identity (`P-RED-1`). |

### 2.2 P-COMMIT-2 · `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4`

| Field | Value |
|---|---|
| **SHA (full)** | `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4` |
| **SHA (short)** | `9c93170` |
| **Parent** | `f819d3c7d983e7333cdd3888fd9bd9a40a12d764` (P-COMMIT-1) |
| **Author / committer** | `VVittgenstein <adrianyuanzhengze@gmail.com>` (both) |
| **Subject** | `chore: add defensive ignores for nested clone and local agent scaffolding` |
| **Body (verbatim)** | `far/ is a same-repo nested clone used only as local evidence, .git/ngagent/ is local agent state stored inside the Git directory, and .worktrees/ is the root for local multi-worktree development.` |
| **Files changed** | 1 (`.gitignore`) |
| **Insertions / deletions** | `+5 / −0` (one blank-line separator + one comment line + three pattern lines) |
| **Tree mutations** | Append a new comment block to `.gitignore` titled `# Local agent scaffolding and nested clones` followed by, in this order: `far/`, `.git/ngagent/`, `.worktrees/`. No index removals. No on-disk changes outside `.gitignore`. |
| **Conformance** | Satisfies `P-INC-6` clause (c) and `P-EXC-9` (`.worktrees/`). Provides the forward-defense aspect of `P-EXC-10` for `far/`; deletion of the local `far/` directory remains explicitly out of scope (see AC-006 in §5 below). The `.git/ngagent/` entry is technically redundant because everything inside `.git/` is already untrackable, but it is included explicitly as a defense against tooling that scans relative paths. Subject and body contain no task ID, run ID, model name, or reviewer name (`P-RED-2`). Author and committer identities are the human's published identity (`P-RED-1`). |

### 2.3 Deferred candidates explicitly not produced

The three deferred candidates from `02-public-commit-sequence.md` §"Editorial / deferred candidates" are NOT produced by this task:

- **P-DEFERRED-A · README selection**: the base commit's `README.md` blob (`7763643f0785068413e865aeeb812370bdd6e370`) is carried forward unchanged. No README edit by this task.
- **P-DEFERRED-B · Runbook wording**: `docs/data_load_runbook.md`, `docs/data_refresh_strategy.md`, and `docs/notify_runbook.md` are carried forward at their base blobs. No runbook edit by this task.
- **P-DEFERRED-C · `.env.example`**: no `.env.example` is added by this task.

---

## 3. Included / excluded path policy verification (AC-003)

### 3.1 Tree-equivalence check vs `origin/main`

Method: enumerate `git ls-tree -r --name-only origin/main` and
`git ls-tree -r --name-only public-main-candidate`, compute the symmetric
difference, and for shared paths compare blob SHAs.

| Quantity | Value |
|---|---|
| Tracked file count on `origin/main` | 161 |
| Tracked file count on `public-main-candidate` | 160 |
| Paths only on `origin/main` (not on candidate) | `scripts/poller_checkpoint.json` (1 path) |
| Paths only on candidate (not on `origin/main`) | none (0 paths) |
| Shared paths with differing blob SHAs | `.gitignore` only — `b3522eda…` on main, `e201b7a7…` on candidate (delta: **+7 lines total, −0** — i.e. 2 runtime patterns inserted by P-COMMIT-1 + a 5-line defensive block inserted by P-COMMIT-2; `git diff --stat origin/main..origin/public-main-candidate -- .gitignore` reports `1 file changed, 7 insertions(+)`. The per-commit `+5 / −0` figure for P-COMMIT-2 in §2.2 is the diff of that single commit only, not the cumulative candidate-vs-`origin/main` diff.) |

Conclusion: the candidate tree is byte-identical to `origin/main`'s tree
**except for** (a) the removal of `scripts/poller_checkpoint.json` and (b)
**seven appended `.gitignore` lines** in total — 2 runtime-hygiene pattern
lines added by P-COMMIT-1 and a 5-line defensive block (one blank
separator + one comment line + three pattern lines) added by P-COMMIT-2.
This matches the construction plan in `02-public-commit-sequence.md` §6
step 6 exactly.

### 3.2 Excluded-path scan on the candidate tree

Pattern scanned (case-sensitive) against `git ls-tree -r --name-only public-main-candidate`:

```
^(\.orchestrator/|AGENTS\.md|docs/archive/|reports/|notebooks/|far/|\.worktrees/|read_only\.md|record\.json|rEmail\.json|rRevision\.json|rSubscribe\.json|Compact/|review/|Rutgers-dr/|configs/.*\.user\.json|configs/.*\.local\.json|\.env$|\.env\.|tasks_bugfix\.json|auto-refresh-tasks\.json|scripts/poller_checkpoint\.json|release/|.*\.tar\.gz$|.*\.zip$)
```

Result: **zero matches**. The candidate tree does not contain any path under `.orchestrator/`, `AGENTS.md`, `docs/archive/stage-a-legacy/`, ngagent artifacts, `far/`, local runtime files, private configs, release archives, or historical workflow records.

### 3.3 Auto-Refresh / Scheduled-Fetch slice preservation (per pinned Main-Agent context)

The Auto-Refresh / Scheduled-Fetch product slice from `e770bf2` is preserved on the candidate by construction (build-on-main). Per-file SHA verification on `public-main-candidate`:

| Path | Candidate blob SHA |
|---|---|
| `api/src/routes/scheduled-fetch.ts` | `25821a71004fec16313a8cb435d3938dc801e88a` |
| `api/src/services/scheduledFetcher.ts` | `e23f87b2211ce1cac520d091c0a1167213a4af97` |
| `frontend/src/api/scheduledFetch.ts` | `30ccb1003b357600df82f10ddcf9e0f0391e9b27` |
| `frontend/src/components/AutoRefreshToggle.tsx` | `49c4bdbf1718fcf55a043821649ce6f4810ec497` |
| `frontend/src/components/AutoRefreshToggle.css` | `91179aab53638b0efa4b7188befd598c45e6d00a` |
| `frontend/src/components/ScheduledFetchPanel.tsx` | `2bf755c413e192d2a84fd6801bba6ccf8d05c777` |
| `frontend/src/components/ScheduledFetchPanel.css` | `1eeabe38d34588e8f37856237a4874b9d80e94ff` |
| `frontend/src/hooks/useAutoRefresh.ts` | `ea551b8252218a0e538f15469111c798928e475a` |
| `frontend/src/hooks/useScheduledFetch.ts` | `72b09d0b6bd0807bd5738ac966b94842d7be1584` |

All nine files are present on the candidate at the same SHAs they have on `origin/main`. Conforms to `02-public-commit-sequence.md` §3.1 "Auto-Refresh / Scheduled Fetch slice".

### 3.4 Final candidate `.gitignore` (the merged-intent ignore)

The candidate's `.gitignore` (blob `e201b7a7…`) is the union of:

- The full `origin/main:.gitignore` (`b3522eda…`) — carried forward by the base commit, including the workflow-name ignores (`Compact/`, `review/`, `Rutgers-dr/`, `reports/`, `notebooks/`, `record.json`, `read_only.md`, `rEmail.json`, `rRevision.json`, `rSubscribe.json`, `auto-refresh-tasks.json`, `tasks_bugfix.json`) which are kept as forward defenses.
- Two new runtime-hygiene entries added by P-COMMIT-1: `data/refresh_queue.json`, `scripts/poller_checkpoint.json`.
- Three new defensive entries added by P-COMMIT-2 under the new comment block `# Local agent scaffolding and nested clones`: `far/`, `.git/ngagent/`, `.worktrees/`.

This satisfies `P-INC-6` in full and matches `02-public-commit-sequence.md` §3.5 "Ignore rules (merged intent)" exactly.

---

## 4. Push result (AC-002)

| Field | Value |
|---|---|
| **Push command** | `git push -u origin public-main-candidate` (no `--force`, no `--force-with-lease`, no refspec rewriting `main`) |
| **Push outcome** | `* [new branch]      public-main-candidate -> public-main-candidate`. Upstream set: local `public-main-candidate` tracks `origin/public-main-candidate`. |
| **Remote tip after push** | `origin/public-main-candidate` = `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4` (verified via `git fetch origin --prune` + `git rev-parse origin/public-main-candidate`). Matches local tip. |
| **`origin/main` after push** | `98748e1466ec55e93115f651a6a84f84e227daa9` — **unchanged** from pre-push value. Verified via `git rev-parse origin/main` after the fetch. |
| **`origin/dev` after push** | `92b62c2da990f69253c205f82801fb54cf06b589` — **unchanged** from pre-push value. Verified via `git rev-parse origin/dev` after the fetch. |
| **Default branch on origin** | Not modified by this task. No `gh api repos/...` or equivalent default-branch change was issued. |
| **Force-push to any branch** | None. |
| **Branch deletion on origin** | None. |
| **Pull request opened** | None. Opening a PR is out of scope for AC-002, which only requires a non-default branch push. |

The post-push remote message *"Create a pull request for 'public-main-candidate' on GitHub by visiting: https://…/pull/new/public-main-candidate"* is a GitHub courtesy hint, not a triggered action; this task neither follows that link nor opens a PR.

---

## 5. No-secret-value verification (AC-005)

### 5.1 Positive-pattern scans against the candidate tree

The patterns from `01-public-divergence-and-exposure-policy.md` §5 were re-run against every tracked blob on `public-main-candidate`. Results:

| Pattern | Description | Matches |
|---|---|---|
| `SG\.[A-Za-z0-9_-]{20,}` | SendGrid live API token shape | 0 |
| `sk_live_[A-Za-z0-9_-]{16,}` | Stripe live secret key shape | 0 |
| `AKIA[0-9A-Z]{16}` | AWS access key ID shape | 0 |
| `ghp_[A-Za-z0-9]{20,}` | GitHub personal access token shape | 0 |
| `xox[baprs]-[A-Za-z0-9-]{10,}` | Slack token shape | 0 |
| `(?i)(password\|secret\|api[_-]?key\|private[_-]?key)\s*[:=]\s*['"][^'"]{12,}['"]` | Generic `key=...` assignment with a 12+ char string literal | 0 |

### 5.2 Placeholder verification

`configs/mail_sender.example.json` on the candidate uses the `example.edu` placeholder domain for all addresses (`alerts@example.edu`, `support@example.edu`, `smtp.example.edu`) and references credentials only by environment-variable name (`SENDGRID_API_KEY`, `SMTP_PASSWORD`), never by literal value. The blob is identical to `origin/main:configs/mail_sender.example.json` (per the tree-equivalence check in §3.1).

### 5.3 Negative confirmation

No literal secret value, password, token, or credential was reproduced in
this report. The patterns above describe shapes only. The historical
`configs/mail_sender.user.json` file removed by `c55352b` is unreachable
from `origin/main`'s tip and is therefore also absent from the candidate
tree; no attempt was made by this task to reconstruct its content.

---

## 6. Negative confirmations (AC-006 and scope discipline)

Re-asserted scope discipline for this task:

- **`far/` is NOT deleted.** The local nested clone at
  `far/Rutgers-BetterCourseSchedulePlanner` was not read, written, copied,
  or referenced by this task beyond P-COMMIT-2's defensive `.gitignore`
  addition. The architecture plan reserves deletion of local `far/` for
  after Stage P cutover; this task is not the cutover task.
- **`origin/main` was NOT updated.** SHA unchanged at `98748e1…`.
- **`origin/dev` was NOT updated.** SHA unchanged at `92b62c2…`.
- **Default branch on `origin` was NOT changed.**
- **No branch was force-pushed.** The candidate branch was pushed as a new ref (`* [new branch]`), not as a force-update over an existing ref.
- **No branch was deleted on `origin` or locally.**
- **No PR was opened.**
- **No file under `api/`, `frontend/`, `workers/`, `notifications/`,
  `scripts/` (other than the index removal of `scripts/poller_checkpoint.json`),
  `configs/`, `data/migrations/`, `data/schema.sql`, or `docs/` was edited.**
- **No package manifest** (`package.json`, `package-lock.json`,
  `frontend/package.json`, `frontend/package-lock.json`, `tsconfig.json`,
  `frontend/tsconfig.json`) was edited. Manifests are byte-identical between
  `origin/main` and the candidate (per §3.1).
- **No `.env`, `.env.*`, `*.user.json`, or `*.local.json` was committed.** None exist on the candidate tree.
- **No release archive** (`*.tar.gz`, `*.zip`, `release/*`) was committed. None exist on the candidate tree.
- The only file written by this task inside the worktree is this report
  (`.orchestrator/stage-p/03-public-candidate-build.md`), committed on
  `feature/task-010`. The two sanitized commits (`f819d3c`, `9c93170`)
  live on `public-main-candidate`, not on `feature/task-010`, and contain
  no file under `.orchestrator/`.

---

## 7. Handoff to review and to cutover

Hand-off targets:

1. **GPT-5.5 xhigh review** of `origin/public-main-candidate` against this
   report and the two Stage P design documents. The reviewer should
   independently verify (a) commit count since `98748e1` is exactly 2;
   (b) both new commits have the human's published identity; (c) both
   subjects contain no task ID, run ID, model name, or reviewer name;
   (d) the candidate tree matches `origin/main`'s tree minus
   `scripts/poller_checkpoint.json` plus **seven new `.gitignore` lines**
   in total (2 runtime-hygiene patterns from P-COMMIT-1 + a 5-line
   defensive block from P-COMMIT-2), with nothing else differing; (e) no `.orchestrator/`, `AGENTS.md`,
   `docs/archive/`, `reports/`, `notebooks/`, `far/`, `*.user.json`,
   `*.local.json`, or `.env*` (except a future `.env.example`, not added
   here) appears anywhere in the candidate tree.

2. **Human cutover approval** for any later task that promotes
   `public-main-candidate` to the public default. Cutover is **not**
   performed here. Cutover-class actions reserved explicitly for that
   later, human-approved task: default-branch change, force-push of
   `main`, fast-forward of `main` to the candidate, deletion of
   `public-main-candidate`, deletion of any other branch, deletion of
   the local `far/` directory, and re-publication of release archives.

Open editorial decisions (carried forward from `01-public-divergence-and-exposure-policy.md` §6 and `02-public-commit-sequence.md` §"Editorial / deferred candidates"): README selection (P-DEFERRED-A), runbook wording (P-DEFERRED-B), `.env.example` (P-DEFERRED-C). None of these are blockers for review of the present candidate; each requires its own task before cutover.
