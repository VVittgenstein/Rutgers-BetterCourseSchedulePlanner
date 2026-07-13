# P7 authority and execution scope

## 1. State

- **Record**: `P7-AUTH-2026-07-13-001`
- **State**: `P7_AUTHORIZED`
- **Active task**: `P7.1-001`
- **Source branch / commit**: `dev` / `a4b035a586a4b14fc3a75698caf99badce869fd5`
- **Execution branch**: `codex/p7-implementation`
- **Authorization source**: active-thread user directive on 2026-07-13

The user explicitly directed: “批准进入 P7，并按推荐的 Git 隔离方案执行。” This supersedes only the P6 waiting state for starting P7 and authorizes the approved P7.1–P7.4 implementation, per-task validation, dedicated commits, and pushes to the isolated P7 branch.

It does **not** authorize a real Rutgers run, P7.5 real-world execution, Vultr staging mutation, GitHub Release, production discovery/deployment, DNS, Cloudflare, certificate, or production-traffic changes. Those retain their independent gates.

## 2. Frozen plan

- 32 tasks: P7.1=15, P7.2=4, P7.3=3, P7.4=5, P7.5=5.
- Hard order: `P7.1 -> P7.2 -> P7.3 -> P7.4 -> P7.5`.
- Exactly two final packages.
- P7.2 UI implementation and P7.3 UI polish remain separate tasks, records, and commits.
- P7.5 consumes immutable candidate hashes and requires later A1E/A1S action-time authority.

## 3. Git isolation

The pre-P7 working tree is user-owned and must remain byte-preserved. P7.1-001 does not clean, reset, stash, restore, delete, or silently stage any existing path. Its commit allowlist is exactly the six files recorded in `00a-p7-authority-and-scope.json`; any seventh path is a stop condition.

The 167 pre-existing dirty paths are frozen in `01-preserved-worktree-manifest.tsv`. P1 and conversation records are opaque protected inputs: only path, status, and size are recorded; their content is neither read nor hashed. `.secrets/` is an opaque ignored root and is never enumerated.

P3–P6 governance inputs and the mainline workflow are currently untracked. They remain local, preserved, SHA-256-pinned external governance inputs. This task deliberately does not backfill old phases into a P7 commit and therefore does not claim that a clean clone can independently reproduce those input bytes.

## 4. Stop rules

Stop before commit or push if any of the following occurs:

1. the active branch, source commit, user authorization, task count, DAG, or upstream hashes differ;
2. a pre-existing dirty path drifts;
3. the Git index contains anything outside the six-file allowlist;
4. a secret, private inventory, P1 path, conversation record, database, build cache, or user-owned file enters the staged set;
5. any P3–P6 validator fails;
6. the requested action would broaden into P7.5 live execution, Vultr staging, Release, or production.

## 5. Machine state

```text
record_id=P7-AUTH-2026-07-13-001
state=P7_AUTHORIZED
active_task=P7.1-001
source_branch=dev
source_commit=a4b035a586a4b14fc3a75698caf99badce869fd5
target_branch=codex/p7-implementation
p7_1_through_p7_4_authorized=TRUE
p7_5_plan_approved=TRUE
p7_5_real_world_execution_authorized=FALSE
real_world_network_test_authorized=FALSE
vultr_staging_mutation_authorized=FALSE
github_release_authorized=FALSE
production_authorized=FALSE
exact_package_count=2
preexisting_dirty_paths=167
task_commit_allowlist_count=6
```
