# Project Goals

## Objective
Stage A is a cleanup, separation, and baseline-reconstruction phase for Rutgers-BetterCourseSchedulePlanner.

The goal is to establish a trustworthy picture of the existing project before any new feature work or architectural rewrite begins. This phase treats the current repository, release archives, Obsidian workflow notes, legacy `record.json` files, Compact/review records, README/docs, local runtime artifacts, and generated configs as separate evidence sources that must be reconciled rather than assumed correct.

Stage A should produce a clear handoff into Stage B refactoring: what is current, what is historical, what is unsafe or stale, what should be archived, and which modules deserve redesign.

## Success Criteria
- [ ] Current repository contents are inventoried by category: product source, tests, current docs, historical AI workflow records, generated/runtime artifacts, local-only configs, release artifacts, and unknowns.
- [ ] Current repository and release pack(s) are compared, with differences classified by significance and likely source of truth.
- [ ] Legacy planning records (`record.json`, `rEmail.json`, `rRevision.json`, `rSubscribe.json`, `Compact/`, `review/`, Obsidian notes) are classified as authoritative/current, historical evidence, stale, contradictory, or unsafe to rely on.
- [ ] The current minimal runnable path is documented from a clean checkout, including required generated data/config steps and any blockers.
- [ ] Local/private artifacts and secret-risk surfaces are identified without exposing secret values.
- [ ] A Stage B refactor candidate list is produced with evidence, priority, blast radius, and recommended sequencing.
- [ ] Each Stage A delivery task produces one normal task commit before review/merge, unless the task is explicitly found to require no repository change.
- [ ] Each Stage A task is independently reviewed through the ngagent review gate by a Claude CLI reviewer configured for Opus 4.7 Max; review must block rather than fall back to another model if that model is unavailable.
- [ ] Each reviewed and merged Stage A task is pushed to the remote repository so remote state tracks the accepted cleanup work.
- [ ] The final Stage A repository is cleaned, separated, and organized at the repository-structure/documentation/history layer: product code remains behaviorally unchanged, while current-vs-historical sources of truth are explicit.
- [ ] No product functionality, source-code architecture, or runtime behavior is changed without a separate Stage B plan.

## Out of Scope
- Adding product features.
- Rewriting architecture or refactoring source modules.
- Deleting historical records or release artifacts before explicit approval.
- Treating legacy `record.json` as the active task system without reconciliation.
- Treating any release pack as authoritative until it is compared against the repository.
- Publishing a new release pack.

## Constraints
- Use `ngagent` as the task-state system for Stage A.
- Main Agent owns planning, decomposition, tracking, and merge decisions; implementation work is delegated after plan approval.
- Stage A execution starts audit/report oriented. Source code and test files are read-only unless a later approved task explicitly changes scope.
- Stage A may include non-product cleanup after audits establish what should move or be separated. This cleanup may affect docs, planning files, archive/history placement, and repository metadata, but not product behavior.
- All Stage A findings must be evidence-backed and traceable to file paths, release contents, command outputs, or recorded history.
- Secrets and private user config values must be redacted in reports.
- Review is not modeled as a recursive implementation task. Instead, every task must pass a dedicated ngagent review gate before merge.
- Review model fallback is not allowed for Stage A. If the requested Opus 4.7 Max review path cannot run, the task is blocked/escalated instead of reviewed by a weaker or different model.
- After each successful merge into `dev`, push `dev` to `origin` unless the remote rejects or the human pauses execution.

## Stage P Objective: Public Repository Synchronization
Stage P turns the cleaned local project understanding into a safe public GitHub default branch.

The public branch should show only the project and its normal engineering surface: product source, user-facing docs, public runbooks, config examples, package manifests, schemas, and other files a reviewer should reasonably see. It should not show ngagent runtime/planning internals, `.orchestrator/`, task/review artifacts, internal historical dialogue records, local runtime data, private config, nested clones, or repository-cleanup scratch material.

The public branch should still preserve visible development activity through multiple normal task-style commits. The goal is not a single squash commit or a raw merge of internal `dev`. Instead, Stage P should produce a sanitized public history whose commits correspond to meaningful cleanup/publication steps while excluding internal workflow artifacts from the public tree and, where practical, from the public branch history.

## Stage P Success Criteria
- [ ] `origin/main` and `origin/dev` are compared as separate evidence layers, not assumed mutually compatible.
- [ ] Product-code differences between remote `main` and local/internal `dev` are classified before any default-branch update.
- [ ] Public include/exclude policy is explicit, including `.orchestrator/`, `AGENTS.md`, ngagent artifacts, `docs/archive/stage-a-legacy/`, local runtime files, private configs, nested clone `far/`, release archives, and historical workflow records.
- [ ] The proposed public branch commit sequence contains multiple normal commits, each with a clear purpose and no internal-only files.
- [ ] Secret and sensitive-history risk is checked before any public/default branch update; secret values are never quoted.
- [ ] The final public branch candidate can be reviewed before replacing or updating `main`.
- [ ] The temporary nested clone `far/` is treated as local evidence only and is deleted from the local workspace after Stage P completes.
- [ ] Executor model for Stage P delivery work is Claude Opus 4.7 Max. Reviewer model is GPT-5.5 with xhigh reasoning. Fallback to different models is not allowed unless the human explicitly changes this rule.
- [ ] No force-push to `main`, default-branch switch, or deletion of remote branches occurs without an explicit human cutover approval after the candidate branch is reviewed.
- [ ] The final public GitHub surface is closed down to the intended public refs only: `main` remains, temporary/internal/stale remote branches are removed, stale tags/releases are classified and handled or explicitly escalated, and no internal `dev`/ngagent/orchestrator branch remains visible on the public remote.

## Stage P Out of Scope
- Product refactoring.
- Feature work.
- Rewriting old Git history to purge historical secrets. If historical secrets are found, they must be treated as already leaked and handled by rotation/revocation plus a separate history-rewrite plan.
- Publishing internal ngagent or orchestrator records on the default public branch.
- Deleting the internal `dev` branch before the public branch strategy is approved.
- Keeping the temporary nested clone `far/` after it is no longer needed.

## Stage P Remote Surface Closeout
After task-011, the default public branch is clean, but the public repository may still expose old remote branches, tags, and GitHub Releases that point to historical/internal states. The closeout goal is to make the public GitHub repository look like one clean project repository, not a visible archive of the earlier confused development workflow.

Closeout target:
- Keep `origin/main` as the public branch.
- Remove public visibility of `origin/dev`, `origin/public-main-candidate`, historical task branches, sync branches, patch branches, and other stale non-main branches.
- Classify remote tags and GitHub Releases. Delete or escalate stale public tags/releases that expose old release states.
- Keep local `dev` and ngagent/orchestrator records as local/internal state only unless a later private remote is configured.
- After deleting the public `origin/dev` branch, do not push local `dev` back to the public `origin`, because that would recreate the branch and undo the closeout.
