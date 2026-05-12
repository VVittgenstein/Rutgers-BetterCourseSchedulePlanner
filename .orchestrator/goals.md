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

## Phase 1 Objective: Local Release BCSP
Phase 1 is the first product delivery phase after Stage A/P cleanup. Its goal is to produce a complete, releasable local version of Rutgers BetterCourseSchedulePlanner.

The target is not a minimal MVP and not a blind continuation of the current working UI. Phase 1 should recover the intended local-tool product that the project was trying to become before earlier model/tooling limitations, bugs, skipped work, and incomplete reviews distorted the implementation. It must also remove or hide surfaces that should not exist in the local release.

The working interpretation of "all features must exist" is:
- Every capability that belongs to the Phase 1 local-release product contract must be end-to-end real, documented, tested or otherwise verified, and reachable through an understandable user flow.
- Historical features that were clearly intended for the local BCSP product but were skipped, disabled, or abandoned because of implementation trouble should be treated as `recover` candidates, not as new feature creep.
- Existing code/UI/API/docs that expose fake, stubbed, obsolete, or misleading functionality should be treated as `repair` or `remove` candidates. Current existence is not sufficient reason to ship.
- Ideas that belong to cloud deployment, multi-user hosting, public auth/security, or a future full refactor should be explicitly deferred to Phase 2 or Phase 3.

Phase 1 should ship a local web tool with:
- Rutgers SOC data fetch/import into local SQLite for the release-supported term/campus/subject scope.
- Reliable course and section exploration, including the section-level data users need to decide what to take and what to monitor.
- Search/filter UX that is coherent, fast enough for local use, and aligned with the real API contract.
- Local subscription/watch workflows for open-seat monitoring.
- Local notifications, including local sound. Email may ship only if its setup, provider support, configuration, and failure states are honest and complete.
- A redesigned UI/UX, not just visual polish. The redesign must cover information architecture, primary flows, empty/loading/error states, accessibility basics, responsiveness, and day-to-day ergonomics.
- A release/startup path suitable for non-expert local users. On Windows, the release candidate must be validated directly. macOS support must either be validated on macOS before being claimed or documented as prepared but not yet externally verified.
- User-facing documentation that matches the shipped local product and does not advertise deferred, removed, or fake functionality.

## Phase 1 Success Criteria
- [ ] A Phase 1 feature matrix exists and classifies major product surfaces as `complete`, `recover`, `repair`, `remove`, `defer`, or `unclear`, with evidence from current code, Stage A records, legacy records, and user product intent.
- [ ] No UI, API route, CLI/startup path, or user-facing doc implies support for a feature that is stubbed, fake, or intentionally deferred.
- [ ] Core local flow works from a fresh release candidate: unpack/start, fetch or load Rutgers data, search/filter courses, inspect sections, subscribe to a target, run open-seat polling, receive a local notification, and manage the subscription.
- [ ] The frontend UI/UX is rebuilt through a two-stage design process: first using `gpt-tasteskill` for product-level UI/UX direction, then using `emil-design-eng` for polish and interaction refinement.
- [ ] Root/frontend/API validation is brought to a release-credible state. Known TypeScript and test failures must be fixed, intentionally removed, or explicitly re-scoped with evidence.
- [ ] Runtime defaults are coherent: database paths, fetch configs, generated artifacts, logs, checkpoints, and local user config are predictable and documented.
- [ ] The local release package/start script works on Windows without requiring project-specific hidden state. macOS support is prepared and either verified on a real macOS environment or clearly marked pending external validation.
- [ ] Product docs, quickstart, troubleshooting, and release notes describe the actual shipped local release.

## Phase 1 Out of Scope
- Google Cloud deployment, hosted multi-user operation, cloud databases, cloud queues, and production auth.
- A full architectural rewrite for its own sake. Phase 1 may repair and reshape code where required to make the local release real, but the complete refactor remains Phase 2.
- Adding speculative new features that were not part of the local BCSP product intent.
- Publishing internal ngagent/orchestrator artifacts to the public release.
- Claiming macOS "validated" support without an actual macOS validation run.
