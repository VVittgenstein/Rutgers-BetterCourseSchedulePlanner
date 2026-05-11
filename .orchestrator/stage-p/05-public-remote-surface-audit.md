# Stage P · Public Remote Surface Audit (post-task-011)

> Stage P task-012 audit of the public GitHub remote surface as of the
> session timestamp recorded in §1.3. Authority over the **enumeration**,
> **classification (keep / delete / escalate)**, and **exact deletion
> plan** for non-`main` remote refs, tags, and GitHub Release objects for
> the remainder of Stage P — but **subordinate** to the two Stage P design
> documents on policy
> (`.orchestrator/stage-p/01-public-divergence-and-exposure-policy.md`,
> `.orchestrator/stage-p/02-public-commit-sequence.md`), the task-010
> construction report
> (`.orchestrator/stage-p/03-public-candidate-build.md`), and the task-011
> cutover report
> (`.orchestrator/stage-p/04-public-cutover-and-local-cleanup.md`).
>
> Scope discipline: **this task is audit-only**. The only write inside the
> worktree is the present file. No remote ref is created, modified, or
> deleted. No tag is created, modified, or deleted. No GitHub Release
> object is created, modified, or deleted. No source or product file is
> edited. `origin/main` is read but not altered. The exact deletion plan
> below is documented for a separate human-approved execution task and is
> **not** executed here.

---

## 1. Audit metadata and tooling

### 1.1 Repository under audit

| Field | Value |
|---|---|
| Remote URL (fetch) | `https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner` |
| Remote URL (push) | `https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner` |
| Owner login | `VVittgenstein` (`type=User`) |
| Repo full name | `VVittgenstein/Rutgers-BetterCourseSchedulePlanner` |
| Visibility | `public` (`private=false`) |
| Default branch (per GitHub API `/repos/.../`) | `main` |
| Archived | `false` |
| Disabled | `false` |
| `has_issues` | `true` (open issue count = `0`) |
| `has_wiki` | `false` |
| `has_pages` | `false` |
| `has_discussions` | `false` |
| `has_projects` | `true` |
| Reported `pushed_at` | `2026-05-11T22:34:45Z` |
| Reported `updated_at` | `2026-05-11T22:21:39Z` |

### 1.2 Local worktree

| Field | Value |
|---|---|
| Worktree absolute path | `Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\task-012` |
| Branch (worktree HEAD) | `feature/task-012` |
| Configured remote | `origin` (single fetch / push URL above) |

### 1.3 Tooling and authentication limitations

| Tool / capability | Available? | Method used by this audit | Notes |
|---|---|---|---|
| `git ls-remote --heads origin` | Yes | Used to enumerate the 147 remote branches (§3.1). | Reads remote refs without modification; non-mutating. |
| `git ls-remote --tags origin` | Yes | Used to enumerate the 8 remote tags (§3.2). | Non-mutating. |
| `git fetch origin --prune` | Yes | Ran once at audit start to refresh remote-tracking refs against the live remote. | Non-mutating to the remote. |
| GitHub REST API (unauthenticated) | Yes | Queried `GET /repos/{owner}/{repo}` and `GET /repos/{owner}/{repo}/releases` via `curl` (Schannel TLS). | Anonymous access; subject to the 60-request-per-hour core rate limit. Verified: the audit consumed 1 request at probe time and the response carried `X-RateLimit-Limit: 60` / `X-RateLimit-Remaining: 59`. |
| GitHub REST API (authenticated, write) | **Not available in this session** | Not invoked. | A Personal Access Token with `public_repo` (or broader `repo`) scope is required to delete a Release object or a release asset via the API. No PAT was used by this audit; the deletion plan in §5 lists the calls but does not execute them. |
| `gh` CLI | **Not available** | Confirmed via `which gh` (PATH lookup returns "no gh in <PATH>") and `gh --version` (`command not found`). | This matches the pinned Main-Agent context. All Release-object enumeration and any Release-object deletion procedure must use the REST API directly. |
| `jq` | **Not available** | Confirmed via `which jq` (PATH lookup returns "no jq in <PATH>"). | JSON parsing done via the system `python` interpreter against the on-disk `curl` response files. |
| Branch-protection / default-branch settings | **Not inspected** | Not invoked. | `GET /repos/{owner}/{repo}/branches/main/protection` requires authentication. The audit does not assert whether `main` has branch-protection rules, nor whether other branches do; any future deletion task must verify that deletion of the listed branches is not blocked by protection rules. |

The unauthenticated 60/hour rate limit is sufficient for the read-only
enumeration done here, but is **not** sufficient for the future deletion
task: deleting 146 branches, 8 tags, 1 release object, and 1 release
asset over the unauthenticated API would exhaust the limit and fail.
Future execution must authenticate.

---

## 2. Authority and goal recap (verbatim from pinned Main-Agent context)

The audit's classification target is pinned by the Main Agent:

- "Human approved ngagent remote surface closeout planning."
- "Current public goal: GitHub should present one clean project; target public refs are only `main`."
- "Preliminary audit found 147 remote branches, 8 tags, and 1 GitHub Release (tag `Release-0122`, name `Release-0121`); gh CLI is unavailable."
- "Known refs: `origin/main = 9c93170c5dc8e3b767312b4877d87ee0d2ce19e4`, `origin/dev = bfa69a63df00011b33c0123448b744ed859f8850`, `origin/public-main-candidate = 9c93170c5dc8e3b767312b4877d87ee0d2ce19e4`."
- "This task is audit-only: do not delete or change any branch, tag, release, or origin/main. Produce an exact deletion plan only."
- "After origin/dev is deleted later, local dev must not be pushed back to public origin."

The three pinned ref values were re-verified live by this audit (§3.1
table for `dev`/`main`/`public-main-candidate`) and all three match
exactly.

---

## 3. Remote surface inventory

### 3.1 Remote branches — count and full list

`git ls-remote --heads origin` returned exactly **147** branch refs.
Counted by line count of the raw output; cross-checked by
`awk '{print $2}' | sort | wc -l` after extracting the `refs/heads/...`
column. The two counts agree.

The three pinned anchor branches verify:

| Anchor branch | SHA (live) | Pinned-expected SHA | Match |
|---|---|---|---|
| `refs/heads/main` | `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4` | `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4` | ✓ |
| `refs/heads/dev` | `bfa69a63df00011b33c0123448b744ed859f8850` | `bfa69a63df00011b33c0123448b744ed859f8850` | ✓ |
| `refs/heads/public-main-candidate` | `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4` | `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4` | ✓ |

Auxiliary equality scan: branches whose tip SHA equals `origin/main`'s
tip exactly are `refs/heads/main` and `refs/heads/public-main-candidate`
— no other branch is at the public-main SHA. `origin/main` is **not** an
ancestor of `origin/dev` (verified by
`git merge-base --is-ancestor 9c93170…  bfa69a63…` returning exit `1`);
`dev` has its own divergent history.

The full enumeration, sorted case-insensitively by branch name, follows.
**SHAs are public Git identifiers, not secrets.** All 147 rows are
listed without abbreviation so a future deletion task can ingest this
table verbatim.

| # | Branch (`refs/heads/<name>`) | Tip SHA |
|---:|---|---|
| 1 | `auto-refresh-tasks` | `e770bf26a4fd154dc53818f83930b495071624fd` |
| 2 | `Before-Final` | `5c526aece8bd1d77d46dc225f0adfb53d2d7f143` |
| 3 | `boli` | `9283ba1a821f6126659363e40957ed01e2c806e9` |
| 4 | `clean-public-surface` | `c55352ba9a50aced927eac020400e1b74df06037` |
| 5 | `CRLF` | `803e472be588e09889791c94024fede16f6dbc64` |
| 6 | `dev` | `bfa69a63df00011b33c0123448b744ed859f8850` |
| 7 | `eMAIL` | `d24bfa56c3f10571983850bf21662097e1b10ff3` |
| 8 | `Everythingbeforeemail` | `e4bc1554e0b981edc0ee65c807bedcc009d9468e` |
| 9 | `Fin-Sync-1125` | `f6b10e2288214962265d4dc43304c87ba6d633b7` |
| 10 | `main` | `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4` |
| 11 | `New-Subtask` | `01aa2124707a43fd33ef66cb908003d5b869091d` |
| 12 | `public-main-candidate` | `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4` |
| 13 | `RealST-20251122-filter-rewrite-02-api-schema-query` | `0e0c55eb2aae64e0255e1be86a4a0fb69ee8d598` |
| 14 | `RealST-20251122-filter-rewrite-02-api-schema-query-FIN` | `42050f73a536cb9e6038508c665ae610bea26339` |
| 15 | `Reorder-Subtask` | `0f73bcbf4f6373cf9f45e07e15958aa662fa4f09` |
| 16 | `ST-20251113-act-001-01-pipeline-config` | `cd1e96194b2f5df87e59bc004dcb608a40a91b61` |
| 17 | `ST-20251113-act-001-01-pipeline-config-Fin` | `5bac266ce2bbf01f56aa8e7efeac39c241e43a18` |
| 18 | `ST-20251113-act-001-01-pipeline-config-v1` | `7674c7b037d396098c6540592787b702fd002689` |
| 19 | `ST-20251113-act-001-01-pipeline-config-v2` | `43243a9d3eb639c24c72eb1e76e438c1c7e5d711` |
| 20 | `ST-20251113-act-001-02-ingest-impl` | `59fb0ab6a6391df37c5c4af07b89e11f7c9fd365` |
| 21 | `ST-20251113-act-001-02-ingest-impl-Fin` | `81f50f1facbdf2e20fa77f887885f5507ae599bd` |
| 22 | `ST-20251113-act-001-02-ingest-impl-v1` | `caf31ca892cadb1b6dab1ea418ce0f5ab8b7273b` |
| 23 | `ST-20251113-act-001-03-data-verification` | `6f8485cff4e63c42570cba0c552148c656a735e2` |
| 24 | `ST-20251113-act-001-03-data-verification-Fin` | `6af04e4145ef3f4dbe6d7adbef2c9ae23e49d0f0` |
| 25 | `ST-20251113-act-002-01-ui-architecture` | `e05ecd0e0a0301123a7ac1aa431a929e4916b740` |
| 26 | `ST-20251113-act-002-01-ui-architecture-Fin` | `7c5e3654288a71bc7c48464b19b4d109af661893` |
| 27 | `ST-20251113-act-002-01-ui-architecture-v1` | `fd5f3a425ed950a03aadb577d8545942aa0ad3d8` |
| 28 | `ST-20251113-act-002-02-filter-components` | `ec4c8d307a20d86998ae5bf99d618a688a008ca9` |
| 29 | `ST-20251113-act-002-02-filter-components-Fin` | `0e93c7c5ffc867651c07d5cfaa25d00b27a4f86a` |
| 30 | `ST-20251113-act-002-02-filter-components-v1` | `2d0dbb2673e6b0a088a62d3f4539b873a85c0ac9` |
| 31 | `ST-20251113-act-002-03-api-integration` | `0002ad83699ab4e10beccef69ee985d1a5418f89` |
| 32 | `ST-20251113-act-002-03-api-integration-Fin` | `2fd63054ccf45e860da7b5c705ddd4d447ae6389` |
| 33 | `ST-20251113-act-002-03-api-integration-v1` | `ea13570b0f1a998eec6b41430a63c512360649e7` |
| 34 | `ST-20251113-act-003-01-worker-contract` | `39e9f7d144d410feb1817ad50d36edde4c016a08` |
| 35 | `ST-20251113-act-003-01-worker-contract-Fin` | `7f65e67d93f04a1f6395889e62e53486cf0407f1` |
| 36 | `ST-20251113-act-003-01-worker-contract-v1` | `717f868da32f3844ef0d9cc314e784682d84c895` |
| 37 | `ST-20251113-act-003-01-worker-contract-v2` | `23438a52fc53cdc63fbfefce8e2a7903b9097657` |
| 38 | `ST-20251113-act-003-01-worker-contract-v3` | `a9359b2670bcadbeb0f1ec5af2d2ede6cb841c79` |
| 39 | `ST-20251113-act-003-01-worker-contract-v4` | `83a735aae788a8294fe3e58f5b71c877e7d18699` |
| 40 | `ST-20251113-act-003-01-worker-contract-v5` | `6da2fdafa4935a80cfe6f5b7614a79ee56e1f36c` |
| 41 | `ST-20251113-act-003-02-worker-implementation` | `cc1e7247a4a4431246a5926eb3026f074d285b17` |
| 42 | `ST-20251113-act-003-02-worker-implementation-Fin` | `df7741a532cb3aadbc8d82d7fac9cf158b04e29f` |
| 43 | `ST-20251113-act-003-03-end-to-end-validation` | `b03ebf01ca552401d523a3553efb1342f1f0b8ab` |
| 44 | `ST-20251113-act-003-03-end-to-end-validation-Fin` | `a06e56e7f05d081f9c1c5c1260e448fff4e573d4` |
| 45 | `ST-20251113-act-003-03-end-to-end-validation-v1` | `5db8874d4ec1a1affa3f00db2754caac7f5e0bf4` |
| 46 | `ST-20251113-act-004-01-strategy` | `5da6721b861551848a43303cef9227ee2b125ea7` |
| 47 | `ST-20251113-act-004-01-strategy-Fin` | `831deb610852f8c685d61e4665f4c96774158ae5` |
| 48 | `ST-20251113-act-004-01-strategy-v1` | `cd41dc35330bf1f0d3ff17858d15efbf7ce0313e` |
| 49 | `ST-20251113-act-004-02-bot-sending` | `8eff5c08535719a4ff4118ae17b8641020802a74` |
| 50 | `ST-20251113-act-004-02-bot-sending-Fin` | `47ae128fc0777baa39274681e964af6aeaf7fe9a` |
| 51 | `ST-20251113-act-004-02-bot-sending-v1` | `694e0ad6e3eb7a60ac8b5024642cad1df870e1c2` |
| 52 | `ST-20251113-act-004-03-event-integration` | `6e9992f81d4885ba2e38509f7c5025c812df2eb1` |
| 53 | `ST-20251113-act-004-03-event-integration-Fin` | `163fcd4d656431b0f710ebb1c932485adf49837f` |
| 54 | `ST-20251113-act-004-03-event-integration-v1` | `c08d63a53024c38f52c7f53b6612fc5f2a63563f` |
| 55 | `ST-20251113-act-005-01-copy-audit` | `5480c844e0c0e947a3b6b585b7296ff1b81ffea5` |
| 56 | `ST-20251113-act-005-01-copy-audit-Fin` | `829f9134e60810d74e1cf4a5836e65bbe0afe44d` |
| 57 | `ST-20251113-act-005-01-copy-audit-v1` | `37d3a0ff75ed2ccf96120873543b92d3621be548` |
| 58 | `ST-20251113-act-005-02-i18n-integration` | `83e89385aaebe1a0df200216665a6f501a23a40c` |
| 59 | `ST-20251113-act-005-02-i18n-integration-Fin` | `13deb258b396e4c3f3ac025387836d026d0946c0` |
| 60 | `ST-20251113-act-005-03-language-toggle` | `3f76aab35ac0ca2b2ab92e3c876e08a5beef71b1` |
| 61 | `ST-20251113-act-006-01-deploy-playbook-Fin` | `8f6f2110c2b0af2d33ed251d0fbabdfa0807b361` |
| 62 | `ST-20251113-act-006-02-automation-scripts` | `df507930dc2bb76523763005222678c32d40adfb` |
| 63 | `ST-20251113-act-006-02-automation-scripts1` | `12f2d81c806491999f6903c2d3199299789f3169` |
| 64 | `ST-20251113-act-006-02-automation-scripts1-Fin` | `70c487497e397be3555a6759dbaee3ce8b9e0001` |
| 65 | `ST-20251113-act-006-02-automation-scripts1-v1` | `0c91582c0a4125cea713d2ae1e636119bf9f5f51` |
| 66 | `ST-20251113-act-006-03-fresh-run` | `b32010d96ed8af4a1d45e404134f86b66ec6d0cf` |
| 67 | `ST-20251113-act-006-03-fresh-run-Fin` | `2f584d2e62388f0acbbe1914b13b166f0bcd57bc` |
| 68 | `ST-20251113-act-007-01-entity-design` | `1b8fa0b722a685476c4bac33fbe1fb707fc5e711` |
| 69 | `ST-20251113-act-007-01-entity-design-Fin` | `89b8f989160f44dae338453223fc3a046476348c` |
| 70 | `ST-20251113-act-007-01-entity-design-v1` | `877439c1e37ddc71713c6fa179a97a80a06bb298` |
| 71 | `ST-20251113-act-007-01-entity-design-v2` | `75c12180e82199b1e48c8dfa18befabaee292993` |
| 72 | `ST-20251113-act-007-02-migration-tooling` | `0430f47ca73f1f13cc1be1475e695091a8d82958` |
| 73 | `ST-20251113-act-007-02-migration-tooling-Fin` | `beb14ad12a287b3ccda0f534384646d2398cd59f` |
| 74 | `ST-20251113-act-007-02-migration-tooling-Fin-1` | `69d3676c71bab15bb162794fbd5cec245e440a1a` |
| 75 | `ST-20251113-act-007-02-migration-tooling-v1` | `30ec2b8eecdf4954d816b466a4aca8b56dbeb648` |
| 76 | `ST-20251113-act-007-02-migration-tooling-v2` | `28f277ac52b2207a1abc96ffd6d5379f20f8298b` |
| 77 | `ST-20251113-act-007-02-migration-tooling-v3-1` | `4e8f2efcb7e22a009cb402b87d8dd1261a6fa3e8` |
| 78 | `ST-20251113-act-007-03-incremental-strategy` | `23acbf5f9ed690bc6994b6feda57b73609c938e4` |
| 79 | `ST-20251113-act-007-03-incremental-strategy-Fin` | `f466d1e3e246014c7a3361f3db501f66851d6937` |
| 80 | `ST-20251113-act-007-03-incremental-strategy-v1` | `a7f8032cc653a2fd1d2bc63d95f5404657ccefe0` |
| 81 | `ST-20251113-act-008-01-contract` | `64e405abb95f94dcc030c7f7d47c9c71d66056b2` |
| 82 | `ST-20251113-act-008-01-contract-Fin` | `ba5f8ce49906110b3b5dd2bdc15c82e89f61041d` |
| 83 | `ST-20251113-act-008-01-contract-v1` | `d7a11a7408a703dd8ff3a04e9c91803e6807d730` |
| 84 | `ST-20251113-act-008-02-filter-engine` | `a13eb38e5ef1872f0ba7e690e97b2fff642360ed` |
| 85 | `ST-20251113-act-008-02-filter-engine-Fin` | `b8db8fb9a39ac4f405543cb585d45b6d33c15845` |
| 86 | `ST-20251113-act-008-02-filter-engine-v1` | `dddb7146a3c683fa694ac6e1ef7a161bdf4454ac` |
| 87 | `ST-20251113-act-008-03-api-hardening` | `55585019341e555b2d124d3972d7d337bcc0f9e3` |
| 88 | `ST-20251113-act-008-03-api-hardening-Fin` | `ed2072bea086c353c33709d33e757020e4b5be52` |
| 89 | `ST-20251113-act-009-01-subscription-model` | `d97801b4587add8ded5fd3e433efe4ca19e3886b` |
| 90 | `ST-20251113-act-009-01-subscription-model-Fin` | `4470ff034d19ed7133973919556a1ad9d4853488` |
| 91 | `ST-20251113-act-009-01-subscription-model-v1` | `4f92874179f84f82677bc9aca2e0ef60d96d693e` |
| 92 | `ST-20251113-act-009-01-subscription-model-v2` | `033bcfc7667567fdf0a57c05e0c04b12e1853300` |
| 93 | `ST-20251113-act-009-01-subscription-model-v3` | `4ad0c2c439050073b898c61d270c61b64085fe24` |
| 94 | `ST-20251113-act-009-01-subscription-model-v4` | `af387fa22a910a1f256824e6884ee6a425ea4fff` |
| 95 | `ST-20251113-act-009-02-subscribe-endpoints` | `7212f924be691773aa89052eaa4d4d8ad96de181` |
| 96 | `ST-20251113-act-009-02-subscribe-endpoints-Fin` | `7d53d9ec657949f5eabe06a8f6e0e774c318f109` |
| 97 | `ST-20251113-act-009-02-subscribe-endpoints-v1` | `708830aec6f1cbc3b401215897a46ebeff06b19a` |
| 98 | `ST-20251113-act-009-02-subscribe-endpoints-v2` | `7c8f6f0c777cab95a57c12d3bfc49c6ecfdbbb82` |
| 99 | `ST-20251113-act-009-03-frontend-flow` | `14d7e61e0ada531bc4aaf5cc947c019e58d58213` |
| 100 | `ST-20251113-act-009-03-frontend-flow-Fin` | `27a31fda314596cfa2d8ed466e4b38c708c67f29` |
| 101 | `ST-20251113-act-009-03-frontend-flow-v1` | `4ace7f1ba607ec08ac38958bad889728b587a866` |
| 102 | `ST-20251113-act-010-01-event-spec` | `d44f78402f7b9db63d8b8a9b32edbe606d44795e` |
| 103 | `ST-20251113-act-010-01-event-spec-Fin` | `f81c35a687a91f564c320f18c157d6daeb080887` |
| 104 | `ST-20251113-act-010-02-polling-worker` | `5d2707422fd251b5361d6f9f33f28af1b12318fe` |
| 105 | `ST-20251113-act-010-02-polling-worker-Fin` | `b10767771e4ff1e141532a03aee83c09c3b22f3a` |
| 106 | `ST-20251113-act-010-02-polling-worker-v1` | `d029a34c226f0f4e13916123beb66fee792cc7fa` |
| 107 | `ST-20251113-act-010-02-polling-worker-v2` | `a61faade5caae7ef92967f4c28223a117b7e4687` |
| 108 | `ST-20251113-act-010-03-resume-tests` | `3e9266034757b33569a20d90d609646ba9d787e3` |
| 109 | `ST-20251113-act-010-03-resume-tests-Fin` | `2c42d38dbfa3d01828803c3af0651a4016fa0e2e` |
| 110 | `ST-20251113-act-010-03-resume-tests-v1` | `2aad10dfab65be295ad92737973f926bf152c90e` |
| 111 | `ST-20251113-act-010-03-resume-tests-v2` | `e16e1e828c843eca7e24a3f03109f2c61ee5a167` |
| 112 | `ST-20251113-act-011-01-mail-interface` | `f97fbaff1c3c7d03bd7e9ed756eefe9688a6ee71` |
| 113 | `ST-20251113-act-011-01-mail-interface-Fin` | `b065d3aaec4d660be0ca5bb5acee7861b49aad23` |
| 114 | `ST-20251113-act-011-02-provider-adapter` | `2f066a93c5b90e7e73f150c8c12e9c9b15079be7` |
| 115 | `ST-20251113-act-011-02-provider-adapter-Fin` | `c8d3c2e42a3fb42b11920a80a2b603d53bf3a17d` |
| 116 | `ST-20251113-act-011-03-retry-tuning` | `a6324de3dc5e78cb6dac5fc2d6f57ad1ce35c18d` |
| 117 | `ST-20251113-act-011-03-retry-tuning-Fin` | `e7e6e222c9f518164e9ed13fd3b18bb3fc300f7c` |
| 118 | `ST-20251113-soc-api-validation-01-probe` | `6548379b8a0bedd79240f9ab57c9dd3a7ea99f7d` |
| 119 | `ST-20251113-soc-api-validation-01-probe-Fin` | `39c454a626923f95f362c03c23c5571a75345005` |
| 120 | `ST-20251113-soc-api-validation-01-probe-Fin-v1` | `39c454a626923f95f362c03c23c5571a75345005` |
| 121 | `ST-20251113-soc-api-validation-01-probe-v1` | `1afdf48e0498564976cb8787cdfa3b5b1290e9c3` |
| 122 | `ST-20251113-soc-api-validation-02-field-matrix` | `9a1f13be8b1b2e360a51a973cf5c3b8c2fc59919` |
| 123 | `ST-20251113-soc-api-validation-02-field-matrix-Fin` | `d5fada25c45210adb6ae48d51ab42c9a7512bf41` |
| 124 | `ST-20251113-soc-api-validation-02-field-matrix-v1` | `9878dd64dc56e3dea97f292ef3196ce3e81f65da` |
| 125 | `ST-20251113-soc-api-validation-03-limit-profile` | `ae7798dbab169f69e177459a9432f5021aa871ec` |
| 126 | `ST-20251113-soc-api-validation-03-limit-profile-Fin` | `dfba0843ef710b98c02909f55f47859da6ec6427` |
| 127 | `ST-20251113-soc-api-validation-03-limit-profile-v1` | `9f605616666196cd6c0cbb061fdb0e28acafdfb1` |
| 128 | `ST-20251122-filter-rewrite-01-frontend-state-ui` | `fe48fbb340ada97aa23579216c53dc862dc79c46` |
| 129 | `ST-20251122-filter-rewrite-03-data-pipeline-dicts` | `43a396a34aa926b5ae41ffc7fb3e834092763c3a` |
| 130 | `ST-20251122-filter-rewrite-03-data-pipeline-dicts-Fin` | `a5c3b25c59021f7570226b892f667e6476e2ae71` |
| 131 | `Sync` | `b250cfe1f17a2edbd13501f95231fa57a25bd20b` |
| 132 | `Sync-001` | `b297cb23a2a3c9b6304758328b764bb8dfe5ed29` |
| 133 | `sync-003` | `59d2091099a199d28a34d1c38ee47da3b8c4b264` |
| 134 | `sync-1` | `0aa1f417fa0d3cb13e8857efd95e14e86f8d57ac` |
| 135 | `Sync-1120` | `b68252b2795ad4c7ed03d4b3a1ab4172b7143530` |
| 136 | `sYNC-1123` | `60a45a4ae85f2d428389cffc4ec0e06aae09161b` |
| 137 | `Sync-record-based-on-Compact` | `ab322191166dc71e4019869e28982560de45e960` |
| 138 | `Sync-with-compact` | `0133e103d6d0917a383edb741196c5584bc7eb8d` |
| 139 | `T-20251113-act-002-frontend-filter-mvp` | `d3b7b913a1ee3a61ab3687cc1f74e323a2d10ca5` |
| 140 | `T-20251113-act-002-frontend-filter-mvp-Fin` | `9bd76c25c87e68eeffae00b94a6661be6959171a` |
| 141 | `T-20251113-act-002-frontend-filter-mvp-v1` | `d845a09f5840214ccc58163b9d6c592feac0247c` |
| 142 | `T-20251113-act-002-frontend-filter-mvp-v2` | `c43b07f4886c06add9c186d85bb3c0f62d4e1d42` |
| 143 | `tasks_bugfix` | `71f2a2b6d6a5afab234f1686965f657af5c3652c` |
| 144 | `Update-record-based-on-compact` | `d17d54f94f627a8c44807c67c46bcba43e458dcf` |
| 145 | `VVittgenstein-patch-1` | `2445c605c0c2eb82f928faed975248b2cfa4be46` |
| 146 | `VVittgenstein-patch-2` | `208fb67a66fe537d6425476fbd1358ab86313c16` |
| 147 | `VVittgenstein-patch-3` | `1c99c239330fb2220d10d55d7f976a8312c7b427` |

### 3.2 Remote tags — count and full list

`git ls-remote --tags origin` returned exactly **8** tag refs. Local
inspection via `git for-each-ref --format='%(refname) %(objecttype)
%(*objectname) %(objectname)' refs/tags` shows the empty
`*objectname` ("peeled" tag object) field on every one, meaning all 8
tags are **lightweight** (direct commit pointers, not annotated tag
objects). No tag carries a tag-object signature or tagger metadata.

| # | Tag (`refs/tags/<name>`) | Target SHA | Object kind | Notes |
|---:|---|---|---|---|
| 1 | `Finalrelease` | `81a4818ce58bba4810be10032ae785f328859996` | lightweight (commit) | Shares target SHA with `First-release`, `Release`, `Release-1124`, `Second-release`. |
| 2 | `First-release` | `81a4818ce58bba4810be10032ae785f328859996` | lightweight (commit) | Same target SHA as above. |
| 3 | `Release` | `81a4818ce58bba4810be10032ae785f328859996` | lightweight (commit) | Same target SHA as above. |
| 4 | `Release-0118` | `2d762179025e68ee05f853c3e7b5e8b43837893c` | lightweight (commit) | Unique SHA in tag set. |
| 5 | `Release-0121` | `b650d81cdd8e11e08f145a44352b13c04c37b8f9` | lightweight (commit) | Shares target SHA with `Release-0122`. **Not** referenced by any GitHub Release object (the Release object's `tag_name` is `Release-0122`, see §3.3). |
| 6 | `Release-0122` | `b650d81cdd8e11e08f145a44352b13c04c37b8f9` | lightweight (commit) | Same target SHA as `Release-0121`. **Is** the `tag_name` of the live GitHub Release object — deletion ordering matters (§5.5). |
| 7 | `Release-1124` | `81a4818ce58bba4810be10032ae785f328859996` | lightweight (commit) | Same target SHA as the first three entries. |
| 8 | `Second-release` | `81a4818ce58bba4810be10032ae785f328859996` | lightweight (commit) | Same target SHA as the first three entries. |

None of the 8 tag targets equals the current `origin/main` tip
`9c93170c5dc8e3b767312b4877d87ee0d2ce19e4`, so no tag is currently
labelling the public default. The target SHAs (`81a4818…`,
`2d76217…`, `b650d81…`) are historical points that pre-date the
Stage P cutover.

### 3.3 GitHub Releases API findings

Endpoint exercised:
`GET https://api.github.com/repos/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/releases`
(unauthenticated; `Accept: application/vnd.github+json`,
`X-GitHub-Api-Version: 2022-11-28`). HTTP `200`, body length `4296`
bytes, content is a JSON array.

`release_count = 1`. The single release object's fields:

| Field | Value |
|---|---|
| `id` | `264993969` |
| `name` | `Release-0121` |
| `tag_name` | `Release-0122` |
| `target_commitish` | `main` |
| `draft` | `false` |
| `prerelease` | `false` |
| `created_at` | `2026-01-21T14:41:59Z` |
| `published_at` | `2025-11-25T03:53:09Z` |
| `html_url` | `https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/releases/tag/Release-0122` |
| `author.login` / `author.id` / `author.type` | `VVittgenstein` / `158061732` / `User` |
| `tarball_url` | `https://api.github.com/repos/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/tarball/Release-0122` |
| `zipball_url` | `https://api.github.com/repos/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/zipball/Release-0122` |
| `body` length (chars) | `12` |
| `body` preview | `Release-0122` |
| `asset_count` | `1` |

The release's single asset:

| Asset field | Value |
|---|---|
| `id` | `344432236` |
| `name` | `bcsp-20260122.zip` |
| `size` (bytes) | `245913` |
| `content_type` | `application/x-zip-compressed` |
| `state` | `uploaded` |
| `download_count` | `2` |
| `browser_download_url` | `https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/releases/download/Release-0122/bcsp-20260122.zip` |

**Important asymmetry, surfaced for the future deletion task:** the
release's `name` is `Release-0121` but its `tag_name` is `Release-0122`,
which matches the pinned Main-Agent context exactly. As a result the
release-tag relationship is *unambiguous* (the release is keyed by
`Release-0122`), but the human-readable label visible on the public
Releases page does **not** match the tag name. Any deletion plan must
reference the release by `id` (`264993969`) or `tag_name`
(`Release-0122`), **not** by `name` (`Release-0121`), to avoid acting
on the wrong release object should additional releases appear in the
future.

Additional observations from the unauthenticated probe:

- The `target_commitish` is `main`. This is the branch GitHub records
  the release as being "on" — it is **not** a SHA reference, and the
  release object will still exist after `origin/main` moves; the
  release's actual immutable anchor is the `tag_name` it points at
  (which resolves to commit `b650d81cdd8e11e08f145a44352b13c04c37b8f9`,
  per §3.2 row 6).
- The release `target_commitish=main` does **not** mean that the public
  default branch `main` currently contains this release's tagged
  commit. `b650d81…` is not an ancestor of `9c93170…` (the current
  `origin/main` tip); the release tag therefore exposes content that is
  **off the current public-main history**. This is non-fatal — release
  tags are independent of the default branch — but it is the kind of
  inconsistency a "one clean project on `main` only" presentation
  should resolve.
- The `body` is the 12-character literal `"Release-0122"` (no rich
  description). No release notes content to preserve.
- The `assets` array contains exactly one downloadable artifact,
  `bcsp-20260122.zip` (≈240 KiB). Its `browser_download_url` is a
  stable public URL that will return `404` after the release or asset
  is deleted; the deletion plan in §5 notes this.

---

## 4. Classification — keep / delete / escalate

The keep target, per pinned Main-Agent context, is "target public refs
are only `main`." Per AC-002, the keep category is `origin/main` only
unless a concrete exception is justified.

### 4.1 KEEP

| Ref | SHA | Reason |
|---|---|---|
| `refs/heads/main` | `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4` | The public default branch. Holds the human-approved, exclusion-scrubbed tree promoted in task-011. This is the only ref required to satisfy "one clean project on `main` only." |

No other ref is in KEEP. No concrete exception has been recorded by
the Main Agent for any other branch or tag.

### 4.2 DELETE

#### 4.2.1 Branches (145 entries)

All 147 remote branches enumerated in §3.1 **except** `main` and
`public-main-candidate` are classified DELETE. `public-main-candidate`
is moved to ESCALATE (§4.3) because task-011 §7 explicitly deferred its
deletion to "a separate human decision," even though it now shares its
tip SHA with `main`.

DELETE branches (145):

`auto-refresh-tasks`, `Before-Final`, `boli`, `clean-public-surface`,
`CRLF`, `dev`, `eMAIL`, `Everythingbeforeemail`, `Fin-Sync-1125`,
`New-Subtask`,
`RealST-20251122-filter-rewrite-02-api-schema-query`,
`RealST-20251122-filter-rewrite-02-api-schema-query-FIN`,
`Reorder-Subtask`,
`ST-20251113-act-001-01-pipeline-config`,
`ST-20251113-act-001-01-pipeline-config-Fin`,
`ST-20251113-act-001-01-pipeline-config-v1`,
`ST-20251113-act-001-01-pipeline-config-v2`,
`ST-20251113-act-001-02-ingest-impl`,
`ST-20251113-act-001-02-ingest-impl-Fin`,
`ST-20251113-act-001-02-ingest-impl-v1`,
`ST-20251113-act-001-03-data-verification`,
`ST-20251113-act-001-03-data-verification-Fin`,
`ST-20251113-act-002-01-ui-architecture`,
`ST-20251113-act-002-01-ui-architecture-Fin`,
`ST-20251113-act-002-01-ui-architecture-v1`,
`ST-20251113-act-002-02-filter-components`,
`ST-20251113-act-002-02-filter-components-Fin`,
`ST-20251113-act-002-02-filter-components-v1`,
`ST-20251113-act-002-03-api-integration`,
`ST-20251113-act-002-03-api-integration-Fin`,
`ST-20251113-act-002-03-api-integration-v1`,
`ST-20251113-act-003-01-worker-contract`,
`ST-20251113-act-003-01-worker-contract-Fin`,
`ST-20251113-act-003-01-worker-contract-v1`,
`ST-20251113-act-003-01-worker-contract-v2`,
`ST-20251113-act-003-01-worker-contract-v3`,
`ST-20251113-act-003-01-worker-contract-v4`,
`ST-20251113-act-003-01-worker-contract-v5`,
`ST-20251113-act-003-02-worker-implementation`,
`ST-20251113-act-003-02-worker-implementation-Fin`,
`ST-20251113-act-003-03-end-to-end-validation`,
`ST-20251113-act-003-03-end-to-end-validation-Fin`,
`ST-20251113-act-003-03-end-to-end-validation-v1`,
`ST-20251113-act-004-01-strategy`,
`ST-20251113-act-004-01-strategy-Fin`,
`ST-20251113-act-004-01-strategy-v1`,
`ST-20251113-act-004-02-bot-sending`,
`ST-20251113-act-004-02-bot-sending-Fin`,
`ST-20251113-act-004-02-bot-sending-v1`,
`ST-20251113-act-004-03-event-integration`,
`ST-20251113-act-004-03-event-integration-Fin`,
`ST-20251113-act-004-03-event-integration-v1`,
`ST-20251113-act-005-01-copy-audit`,
`ST-20251113-act-005-01-copy-audit-Fin`,
`ST-20251113-act-005-01-copy-audit-v1`,
`ST-20251113-act-005-02-i18n-integration`,
`ST-20251113-act-005-02-i18n-integration-Fin`,
`ST-20251113-act-005-03-language-toggle`,
`ST-20251113-act-006-01-deploy-playbook-Fin`,
`ST-20251113-act-006-02-automation-scripts`,
`ST-20251113-act-006-02-automation-scripts1`,
`ST-20251113-act-006-02-automation-scripts1-Fin`,
`ST-20251113-act-006-02-automation-scripts1-v1`,
`ST-20251113-act-006-03-fresh-run`,
`ST-20251113-act-006-03-fresh-run-Fin`,
`ST-20251113-act-007-01-entity-design`,
`ST-20251113-act-007-01-entity-design-Fin`,
`ST-20251113-act-007-01-entity-design-v1`,
`ST-20251113-act-007-01-entity-design-v2`,
`ST-20251113-act-007-02-migration-tooling`,
`ST-20251113-act-007-02-migration-tooling-Fin`,
`ST-20251113-act-007-02-migration-tooling-Fin-1`,
`ST-20251113-act-007-02-migration-tooling-v1`,
`ST-20251113-act-007-02-migration-tooling-v2`,
`ST-20251113-act-007-02-migration-tooling-v3-1`,
`ST-20251113-act-007-03-incremental-strategy`,
`ST-20251113-act-007-03-incremental-strategy-Fin`,
`ST-20251113-act-007-03-incremental-strategy-v1`,
`ST-20251113-act-008-01-contract`,
`ST-20251113-act-008-01-contract-Fin`,
`ST-20251113-act-008-01-contract-v1`,
`ST-20251113-act-008-02-filter-engine`,
`ST-20251113-act-008-02-filter-engine-Fin`,
`ST-20251113-act-008-02-filter-engine-v1`,
`ST-20251113-act-008-03-api-hardening`,
`ST-20251113-act-008-03-api-hardening-Fin`,
`ST-20251113-act-009-01-subscription-model`,
`ST-20251113-act-009-01-subscription-model-Fin`,
`ST-20251113-act-009-01-subscription-model-v1`,
`ST-20251113-act-009-01-subscription-model-v2`,
`ST-20251113-act-009-01-subscription-model-v3`,
`ST-20251113-act-009-01-subscription-model-v4`,
`ST-20251113-act-009-02-subscribe-endpoints`,
`ST-20251113-act-009-02-subscribe-endpoints-Fin`,
`ST-20251113-act-009-02-subscribe-endpoints-v1`,
`ST-20251113-act-009-02-subscribe-endpoints-v2`,
`ST-20251113-act-009-03-frontend-flow`,
`ST-20251113-act-009-03-frontend-flow-Fin`,
`ST-20251113-act-009-03-frontend-flow-v1`,
`ST-20251113-act-010-01-event-spec`,
`ST-20251113-act-010-01-event-spec-Fin`,
`ST-20251113-act-010-02-polling-worker`,
`ST-20251113-act-010-02-polling-worker-Fin`,
`ST-20251113-act-010-02-polling-worker-v1`,
`ST-20251113-act-010-02-polling-worker-v2`,
`ST-20251113-act-010-03-resume-tests`,
`ST-20251113-act-010-03-resume-tests-Fin`,
`ST-20251113-act-010-03-resume-tests-v1`,
`ST-20251113-act-010-03-resume-tests-v2`,
`ST-20251113-act-011-01-mail-interface`,
`ST-20251113-act-011-01-mail-interface-Fin`,
`ST-20251113-act-011-02-provider-adapter`,
`ST-20251113-act-011-02-provider-adapter-Fin`,
`ST-20251113-act-011-03-retry-tuning`,
`ST-20251113-act-011-03-retry-tuning-Fin`,
`ST-20251113-soc-api-validation-01-probe`,
`ST-20251113-soc-api-validation-01-probe-Fin`,
`ST-20251113-soc-api-validation-01-probe-Fin-v1`,
`ST-20251113-soc-api-validation-01-probe-v1`,
`ST-20251113-soc-api-validation-02-field-matrix`,
`ST-20251113-soc-api-validation-02-field-matrix-Fin`,
`ST-20251113-soc-api-validation-02-field-matrix-v1`,
`ST-20251113-soc-api-validation-03-limit-profile`,
`ST-20251113-soc-api-validation-03-limit-profile-Fin`,
`ST-20251113-soc-api-validation-03-limit-profile-v1`,
`ST-20251122-filter-rewrite-01-frontend-state-ui`,
`ST-20251122-filter-rewrite-03-data-pipeline-dicts`,
`ST-20251122-filter-rewrite-03-data-pipeline-dicts-Fin`,
`Sync`, `Sync-001`, `sync-003`, `sync-1`, `Sync-1120`, `sYNC-1123`,
`Sync-record-based-on-Compact`, `Sync-with-compact`,
`T-20251113-act-002-frontend-filter-mvp`,
`T-20251113-act-002-frontend-filter-mvp-Fin`,
`T-20251113-act-002-frontend-filter-mvp-v1`,
`T-20251113-act-002-frontend-filter-mvp-v2`,
`tasks_bugfix`, `Update-record-based-on-compact`,
`VVittgenstein-patch-1`, `VVittgenstein-patch-2`,
`VVittgenstein-patch-3`.

Rationale categories (informational; all rows are DELETE regardless):

- **`dev`** — internal development line carrying ngagent / Stage-A
  artifacts (`.orchestrator/`, `AGENTS.md`, `far/`, `reports/`,
  `notebooks/`, `data/`, runtime checkpoints, etc.) that the public
  divergence policy
  (`.orchestrator/stage-p/01-public-divergence-and-exposure-policy.md`)
  classifies as private. `origin/main` is **not** an ancestor of
  `origin/dev`, so `dev` cannot be promoted forward without violating
  the policy. The pinned Main-Agent context explicitly names `dev` for
  later deletion.
- **`clean-public-surface`** — name suggests prior public-surface work
  but its tip `c55352b…` is not the approved public head
  (`9c93170…`); it predates the Stage P candidate and is not the
  approved artifact. Not safe to retain as a public ref.
- **`ST-*`, `RealST-*`, `T-*` working / sub-task / `-Fin` / `-v1..vN`
  branches** (≈115 entries) — historical sub-task work branches from
  internal multi-agent execution. None of these reflect the approved
  Stage P public tree.
- **`Sync*`, `sync*`, `sYNC*`, `*Sync*Compact*`, `*compact*`, `Compact`
  references in names, `*-record-based-on-compact*`** (10 entries) —
  internal sync / compaction snapshots. Not part of the public surface.
- **`VVittgenstein-patch-1/2/3`** — GitHub web-UI "edit on web" patch
  branches; not on the public head.
- **`Before-Final`, `eMAIL`, `Everythingbeforeemail`, `Fin-Sync-1125`,
  `New-Subtask`, `Reorder-Subtask`, `auto-refresh-tasks`, `boli`,
  `CRLF`, `tasks_bugfix`** — miscellaneous internal branches.

#### 4.2.2 Tags (8 entries)

All 8 tags in §3.2 are classified DELETE. None of them point at the
current public-main tip, none of them was promoted as a public release
endpoint by Stage P, and the goal "target public refs are only `main`"
admits no tag exception. Tags:

`Finalrelease`, `First-release`, `Release`, `Release-0118`,
`Release-0121`, `Release-0122`, `Release-1124`, `Second-release`.

`Release-0122` carries a deletion ordering dependency on the GitHub
Release object (§5.5) — the release object should be deleted first to
avoid leaving a release that points at a missing tag.

### 4.3 ESCALATE

These items are not auto-classifiable under the pinned policy alone
and should be confirmed by the Main Agent / human before the deletion
task executes.

| Ref / object | Why ESCALATE | Suggested resolution (advisory only) |
|---|---|---|
| `refs/heads/public-main-candidate` (`9c93170c5dc8e3b767312b4877d87ee0d2ce19e4`) | Same tip SHA as `main`, so deleting it removes a visible branch name without removing any reachable commit object. However, task-011 §7 explicitly states "Deletion of [`origin/public-main-candidate`] is a separate human decision and is not part of this task." The pinned Main-Agent context for task-012 also does not explicitly authorize its deletion. | Confirm: should the deferred deletion now execute, or should the candidate ref be retained as a review-history anchor? Recommendation: DELETE alongside the others, since "one clean project on `main` only" implies no parallel ref tracking the same commit. |
| GitHub Release object `id=264993969` (`tag_name=Release-0122`, `name=Release-0121`, asset `bcsp-20260122.zip`, 245 913 bytes, `download_count=2`) | Public-facing artifact. Deleting it invalidates the public download URL `https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/releases/download/Release-0122/bcsp-20260122.zip` and the release page. The asset has been downloaded twice; downstream consumers may exist. | Confirm: should the release object and its asset be deleted (recommended for "one clean project" presentation), or should it be repointed/repackaged onto a new `main`-based artifact? This audit recommends DELETE (the release predates the Stage P public tree and is therefore inconsistent with current `main`), with the §5 procedure executed in the order: release object → asset (auto-deleted with release) → tag `Release-0122`. |

No ref or object is classified KEEP via concrete exception.

---

## 5. Exact deletion plan (no execution)

> **AC-003 / AC-005 compliance:** this section documents the exact
> commands and API calls a future task would execute to remove the
> non-`main` surface. **No command in this section is executed by
> task-012.** Each block is provided verbatim so the future task can
> ingest it without re-deriving the names.

### 5.1 Pre-flight checks (mandatory before any deletion)

To be performed by the future deletion task immediately before any
mutating call:

1. `git fetch origin --prune` — refresh local view of the remote refs.
2. Re-verify the three pinned anchor SHAs (`main`, `dev`,
   `public-main-candidate`) match the values in §3.1 of this report.
   If `main` has moved beyond `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4`,
   stop and re-audit before deleting anything.
3. Probe `GET /repos/VVittgenstein/Rutgers-BetterCourseSchedulePlanner`
   and confirm `default_branch == "main"`, `visibility == "public"`,
   `archived == false`.
4. Probe `GET /repos/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/branches/main/protection`
   (authenticated). If protection rules forbid branch deletion or
   require status checks, surface that to the Main Agent before
   proceeding. Note that this audit could not perform this probe
   because no PAT is available in the audit session.
5. Verify the GitHub Release object still has `id=264993969` and
   `tag_name="Release-0122"` (it could have been updated between this
   audit and the deletion task).
6. Confirm the executing user/PAT holds at minimum `public_repo` scope
   for Release-object deletion and the appropriate `git push --delete`
   credential for branch/tag deletion. If a PAT is not available,
   stop.

### 5.2 Branch deletions (146 branches)

Per `git push` semantics, deleting a remote branch uses either the
`--delete` form or the empty-source refspec form. Both are equivalent;
both are non-force operations that fail closed if the ref does not
exist at the expected name. The plan below documents both syntaxes.
The future task should pick one and run it (not both).

#### Form A — explicit `--delete` invocations, one ref at a time

Order in this block matches §3.1 sort order, except `main` is excluded.
`public-main-candidate` is listed only if §4.3 is resolved as DELETE;
if it remains ESCALATE, the future task must skip it.

```text
git push origin --delete auto-refresh-tasks
git push origin --delete Before-Final
git push origin --delete boli
git push origin --delete clean-public-surface
git push origin --delete CRLF
git push origin --delete dev
git push origin --delete eMAIL
git push origin --delete Everythingbeforeemail
git push origin --delete Fin-Sync-1125
git push origin --delete New-Subtask
git push origin --delete public-main-candidate     # only if §4.3 resolves to DELETE
git push origin --delete RealST-20251122-filter-rewrite-02-api-schema-query
git push origin --delete RealST-20251122-filter-rewrite-02-api-schema-query-FIN
git push origin --delete Reorder-Subtask
git push origin --delete ST-20251113-act-001-01-pipeline-config
git push origin --delete ST-20251113-act-001-01-pipeline-config-Fin
git push origin --delete ST-20251113-act-001-01-pipeline-config-v1
git push origin --delete ST-20251113-act-001-01-pipeline-config-v2
git push origin --delete ST-20251113-act-001-02-ingest-impl
git push origin --delete ST-20251113-act-001-02-ingest-impl-Fin
git push origin --delete ST-20251113-act-001-02-ingest-impl-v1
git push origin --delete ST-20251113-act-001-03-data-verification
git push origin --delete ST-20251113-act-001-03-data-verification-Fin
git push origin --delete ST-20251113-act-002-01-ui-architecture
git push origin --delete ST-20251113-act-002-01-ui-architecture-Fin
git push origin --delete ST-20251113-act-002-01-ui-architecture-v1
git push origin --delete ST-20251113-act-002-02-filter-components
git push origin --delete ST-20251113-act-002-02-filter-components-Fin
git push origin --delete ST-20251113-act-002-02-filter-components-v1
git push origin --delete ST-20251113-act-002-03-api-integration
git push origin --delete ST-20251113-act-002-03-api-integration-Fin
git push origin --delete ST-20251113-act-002-03-api-integration-v1
git push origin --delete ST-20251113-act-003-01-worker-contract
git push origin --delete ST-20251113-act-003-01-worker-contract-Fin
git push origin --delete ST-20251113-act-003-01-worker-contract-v1
git push origin --delete ST-20251113-act-003-01-worker-contract-v2
git push origin --delete ST-20251113-act-003-01-worker-contract-v3
git push origin --delete ST-20251113-act-003-01-worker-contract-v4
git push origin --delete ST-20251113-act-003-01-worker-contract-v5
git push origin --delete ST-20251113-act-003-02-worker-implementation
git push origin --delete ST-20251113-act-003-02-worker-implementation-Fin
git push origin --delete ST-20251113-act-003-03-end-to-end-validation
git push origin --delete ST-20251113-act-003-03-end-to-end-validation-Fin
git push origin --delete ST-20251113-act-003-03-end-to-end-validation-v1
git push origin --delete ST-20251113-act-004-01-strategy
git push origin --delete ST-20251113-act-004-01-strategy-Fin
git push origin --delete ST-20251113-act-004-01-strategy-v1
git push origin --delete ST-20251113-act-004-02-bot-sending
git push origin --delete ST-20251113-act-004-02-bot-sending-Fin
git push origin --delete ST-20251113-act-004-02-bot-sending-v1
git push origin --delete ST-20251113-act-004-03-event-integration
git push origin --delete ST-20251113-act-004-03-event-integration-Fin
git push origin --delete ST-20251113-act-004-03-event-integration-v1
git push origin --delete ST-20251113-act-005-01-copy-audit
git push origin --delete ST-20251113-act-005-01-copy-audit-Fin
git push origin --delete ST-20251113-act-005-01-copy-audit-v1
git push origin --delete ST-20251113-act-005-02-i18n-integration
git push origin --delete ST-20251113-act-005-02-i18n-integration-Fin
git push origin --delete ST-20251113-act-005-03-language-toggle
git push origin --delete ST-20251113-act-006-01-deploy-playbook-Fin
git push origin --delete ST-20251113-act-006-02-automation-scripts
git push origin --delete ST-20251113-act-006-02-automation-scripts1
git push origin --delete ST-20251113-act-006-02-automation-scripts1-Fin
git push origin --delete ST-20251113-act-006-02-automation-scripts1-v1
git push origin --delete ST-20251113-act-006-03-fresh-run
git push origin --delete ST-20251113-act-006-03-fresh-run-Fin
git push origin --delete ST-20251113-act-007-01-entity-design
git push origin --delete ST-20251113-act-007-01-entity-design-Fin
git push origin --delete ST-20251113-act-007-01-entity-design-v1
git push origin --delete ST-20251113-act-007-01-entity-design-v2
git push origin --delete ST-20251113-act-007-02-migration-tooling
git push origin --delete ST-20251113-act-007-02-migration-tooling-Fin
git push origin --delete ST-20251113-act-007-02-migration-tooling-Fin-1
git push origin --delete ST-20251113-act-007-02-migration-tooling-v1
git push origin --delete ST-20251113-act-007-02-migration-tooling-v2
git push origin --delete ST-20251113-act-007-02-migration-tooling-v3-1
git push origin --delete ST-20251113-act-007-03-incremental-strategy
git push origin --delete ST-20251113-act-007-03-incremental-strategy-Fin
git push origin --delete ST-20251113-act-007-03-incremental-strategy-v1
git push origin --delete ST-20251113-act-008-01-contract
git push origin --delete ST-20251113-act-008-01-contract-Fin
git push origin --delete ST-20251113-act-008-01-contract-v1
git push origin --delete ST-20251113-act-008-02-filter-engine
git push origin --delete ST-20251113-act-008-02-filter-engine-Fin
git push origin --delete ST-20251113-act-008-02-filter-engine-v1
git push origin --delete ST-20251113-act-008-03-api-hardening
git push origin --delete ST-20251113-act-008-03-api-hardening-Fin
git push origin --delete ST-20251113-act-009-01-subscription-model
git push origin --delete ST-20251113-act-009-01-subscription-model-Fin
git push origin --delete ST-20251113-act-009-01-subscription-model-v1
git push origin --delete ST-20251113-act-009-01-subscription-model-v2
git push origin --delete ST-20251113-act-009-01-subscription-model-v3
git push origin --delete ST-20251113-act-009-01-subscription-model-v4
git push origin --delete ST-20251113-act-009-02-subscribe-endpoints
git push origin --delete ST-20251113-act-009-02-subscribe-endpoints-Fin
git push origin --delete ST-20251113-act-009-02-subscribe-endpoints-v1
git push origin --delete ST-20251113-act-009-02-subscribe-endpoints-v2
git push origin --delete ST-20251113-act-009-03-frontend-flow
git push origin --delete ST-20251113-act-009-03-frontend-flow-Fin
git push origin --delete ST-20251113-act-009-03-frontend-flow-v1
git push origin --delete ST-20251113-act-010-01-event-spec
git push origin --delete ST-20251113-act-010-01-event-spec-Fin
git push origin --delete ST-20251113-act-010-02-polling-worker
git push origin --delete ST-20251113-act-010-02-polling-worker-Fin
git push origin --delete ST-20251113-act-010-02-polling-worker-v1
git push origin --delete ST-20251113-act-010-02-polling-worker-v2
git push origin --delete ST-20251113-act-010-03-resume-tests
git push origin --delete ST-20251113-act-010-03-resume-tests-Fin
git push origin --delete ST-20251113-act-010-03-resume-tests-v1
git push origin --delete ST-20251113-act-010-03-resume-tests-v2
git push origin --delete ST-20251113-act-011-01-mail-interface
git push origin --delete ST-20251113-act-011-01-mail-interface-Fin
git push origin --delete ST-20251113-act-011-02-provider-adapter
git push origin --delete ST-20251113-act-011-02-provider-adapter-Fin
git push origin --delete ST-20251113-act-011-03-retry-tuning
git push origin --delete ST-20251113-act-011-03-retry-tuning-Fin
git push origin --delete ST-20251113-soc-api-validation-01-probe
git push origin --delete ST-20251113-soc-api-validation-01-probe-Fin
git push origin --delete ST-20251113-soc-api-validation-01-probe-Fin-v1
git push origin --delete ST-20251113-soc-api-validation-01-probe-v1
git push origin --delete ST-20251113-soc-api-validation-02-field-matrix
git push origin --delete ST-20251113-soc-api-validation-02-field-matrix-Fin
git push origin --delete ST-20251113-soc-api-validation-02-field-matrix-v1
git push origin --delete ST-20251113-soc-api-validation-03-limit-profile
git push origin --delete ST-20251113-soc-api-validation-03-limit-profile-Fin
git push origin --delete ST-20251113-soc-api-validation-03-limit-profile-v1
git push origin --delete ST-20251122-filter-rewrite-01-frontend-state-ui
git push origin --delete ST-20251122-filter-rewrite-03-data-pipeline-dicts
git push origin --delete ST-20251122-filter-rewrite-03-data-pipeline-dicts-Fin
git push origin --delete Sync
git push origin --delete Sync-001
git push origin --delete sync-003
git push origin --delete sync-1
git push origin --delete Sync-1120
git push origin --delete sYNC-1123
git push origin --delete Sync-record-based-on-Compact
git push origin --delete Sync-with-compact
git push origin --delete T-20251113-act-002-frontend-filter-mvp
git push origin --delete T-20251113-act-002-frontend-filter-mvp-Fin
git push origin --delete T-20251113-act-002-frontend-filter-mvp-v1
git push origin --delete T-20251113-act-002-frontend-filter-mvp-v2
git push origin --delete tasks_bugfix
git push origin --delete Update-record-based-on-compact
git push origin --delete VVittgenstein-patch-1
git push origin --delete VVittgenstein-patch-2
git push origin --delete VVittgenstein-patch-3
```

Total: 146 branch-deletion commands (145 always + 1 conditional on §4.3
resolution). `main` is intentionally absent.

#### Form B — single batched `git push` (faster but less surgical)

The equivalent batched call is allowed by Git's wire protocol. The
empty-source refspec `:refs/heads/<name>` deletes the named remote ref:

```text
git push origin \
  :refs/heads/auto-refresh-tasks \
  :refs/heads/Before-Final \
  :refs/heads/boli \
  :refs/heads/clean-public-surface \
  :refs/heads/CRLF \
  :refs/heads/dev \
  :refs/heads/eMAIL \
  :refs/heads/Everythingbeforeemail \
  :refs/heads/Fin-Sync-1125 \
  :refs/heads/New-Subtask \
  :refs/heads/public-main-candidate \
  :refs/heads/RealST-20251122-filter-rewrite-02-api-schema-query \
  :refs/heads/RealST-20251122-filter-rewrite-02-api-schema-query-FIN \
  ...
  :refs/heads/VVittgenstein-patch-3
```

(Truncated for readability — every branch name from Form A's `--delete`
list maps 1:1 to a `:refs/heads/<name>` token in Form B. The future
task can construct the full list mechanically from Form A.)

Form B has one atomic-ish push (all-or-some-fail, per branch
independently) and triggers GitHub's branch-deletion event hooks **once
per push** rather than once per delete. Form A is easier to log and
restart on partial failure. Either is acceptable.

### 5.3 Tag deletions (8 tags)

```text
git push origin --delete refs/tags/Finalrelease
git push origin --delete refs/tags/First-release
git push origin --delete refs/tags/Release
git push origin --delete refs/tags/Release-0118
git push origin --delete refs/tags/Release-0121
git push origin --delete refs/tags/Release-0122     # MUST run AFTER §5.4 (release object deletion)
git push origin --delete refs/tags/Release-1124
git push origin --delete refs/tags/Second-release
```

Equivalent refspec form:

```text
git push origin \
  :refs/tags/Finalrelease \
  :refs/tags/First-release \
  :refs/tags/Release \
  :refs/tags/Release-0118 \
  :refs/tags/Release-0121 \
  :refs/tags/Release-1124 \
  :refs/tags/Second-release
git push origin :refs/tags/Release-0122   # SEPARATE PUSH, AFTER §5.4
```

The `refs/tags/...` qualification is required (not just the bare tag
name) to disambiguate tag-namespace from branch-namespace when the
underlying `git push` builds its refspec, since some tag names are also
plausible branch names.

### 5.4 GitHub Release object deletion (1 release + 1 asset)

Because `gh` CLI is not available in this environment, the deletion
must use the REST API directly. The `Authorization` header value
`Bearer <PAT_WITH_PUBLIC_REPO_SCOPE>` is a **placeholder**; the future
task supplies a real PAT and **never commits, prints, or logs the
token value** anywhere.

#### Option A — delete the release object (asset is deleted with it)

```text
curl -sS -X DELETE \
  -H "Accept: application/vnd.github+json" \
  -H "Authorization: Bearer <PAT_WITH_PUBLIC_REPO_SCOPE>" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  -o /dev/null \
  -w "http_status=%{http_code}\n" \
  https://api.github.com/repos/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/releases/264993969
```

Expected response: HTTP `204 No Content`. Per GitHub's documented
behavior, deleting the release object also deletes its uploaded assets
(here: asset `id=344432236`, file `bcsp-20260122.zip`). The release
object's underlying Git tag `Release-0122` is **not** removed by this
call; the tag must be removed separately in §5.3.

#### Option B — delete the asset alone (release object retained)

Only run this if §4.3 is resolved as "keep the release object but
remove the downloadable artifact." Not the recommended path.

```text
curl -sS -X DELETE \
  -H "Accept: application/vnd.github+json" \
  -H "Authorization: Bearer <PAT_WITH_PUBLIC_REPO_SCOPE>" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  -o /dev/null \
  -w "http_status=%{http_code}\n" \
  https://api.github.com/repos/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/releases/assets/344432236
```

### 5.5 Ordering and dependency requirements

| Step | Pre-conditions | Effect on later steps |
|---|---|---|
| Pre-flight (§5.1) | Audit refs match §3.1 | Establishes the audit window. If a SHA has moved, halt. |
| 5.2 branch deletions (excluding `main`) | Pre-flight passed; no branch protection forbids it | None of the branches in §4.2.1 is referenced by a tag or release; deletion order among branches is irrelevant. `main` is untouched. |
| 5.4 release object deletion | Pre-flight passed; PAT scoped to `public_repo` or broader | After this, the tag `Release-0122` has no referencing release object; safe to delete the tag in §5.3. |
| 5.3 tag deletions | Tags `Release-0122`'s release object already deleted | After this, no tag points at any commit on the public surface; only `main` remains. |
| Post-flight verification | All of the above completed | See §5.6. |

**Critical ordering rule:** if `refs/tags/Release-0122` is deleted
**before** the release object `264993969`, GitHub will leave the
release object orphaned (its `tag_name` no longer resolves to a tag);
the release object remains accessible via `/releases/{id}` and the
download URL may continue to resolve from cache for a brief window.
The orphaned state is recoverable but produces inconsistent public-side
behavior in the meantime. Avoid by always deleting the release object
first, then the tag.

### 5.6 Post-flight verification (after the future deletion task)

After the future task executes the plan above, verification commands
that should be re-run by an audit task:

```text
git fetch origin --prune

git ls-remote --heads origin
  → expect exactly 1 line: 9c93170c5dc8e3b767312b4877d87ee0d2ce19e4 refs/heads/main
    (or 2 lines if §4.3 keeps public-main-candidate)

git ls-remote --tags origin
  → expect 0 lines

curl -sS -H "Accept: application/vnd.github+json" \
     -H "X-GitHub-Api-Version: 2022-11-28" \
     https://api.github.com/repos/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/releases
  → expect JSON body "[]" (release_count = 0)

curl -sSI https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/releases/download/Release-0122/bcsp-20260122.zip
  → expect HTTP 404

curl -sS -H "Accept: application/vnd.github+json" \
     -H "X-GitHub-Api-Version: 2022-11-28" \
     https://api.github.com/repos/VVittgenstein/Rutgers-BetterCourseSchedulePlanner
  → expect default_branch = "main", visibility = "public", archived = false
```

If any post-flight check fails, halt and re-audit.

---

## 6. Local-dev push prohibition (AC-004)

After the future deletion task removes `origin/dev` per §5.2:

- **The local branch `dev` MUST NOT be pushed back to `origin`.**
- This includes, but is not limited to:
  - `git push origin dev`
  - `git push origin HEAD:dev`
  - `git push origin <sha>:refs/heads/dev`
  - `git push origin --all` (which would re-push every local branch,
    including `dev`)
  - `git push origin --mirror` (which is even more dangerous: it
    re-creates *all* local refs on the remote, undoing the entire
    deletion)
- The reason: the local `dev` branch carries internal-only tooling
  (`.orchestrator/`, `AGENTS.md`, ngagent task state, `far/` history,
  internal runtime checkpoints, etc.) that the divergence policy
  (`.orchestrator/stage-p/01-public-divergence-and-exposure-policy.md`)
  excludes from the public surface. Re-pushing `dev` to public `origin`
  would re-expose every excluded path in one operation and would
  invalidate the Stage P cutover.
- The local `dev` branch may continue to exist in the **local**
  repository / worktrees for internal development; it simply must not
  have a public remote-tracking destination on `origin`. If internal
  collaboration on `dev` is needed, the team must use a separate,
  private remote (not `origin`).
- Suggested permanent guard for the future deletion task to add (out
  of scope for this audit; documented here so the human can choose to
  apply it): a local Git config entry such as
  `git config --local --add remote.origin.push '!refs/heads/dev'`
  or a `pre-push` hook that rejects pushes whose refspec resolves to
  `refs/heads/dev` on `origin`. The exact mechanism is a separate
  decision; what matters is that re-exposure of `dev` is prevented by
  design, not just by convention.
- The pinned Main-Agent context already states this prohibition
  ("After origin/dev is deleted later, local dev must not be pushed
  back to public origin"). This audit re-asserts and amplifies it.

---

## 7. Negative confirmations (AC-005)

Re-asserted explicitly so the audit's blast radius is auditable:

- **No remote branch was created, modified, or deleted** by task-012.
  All 147 branches enumerated in §3.1 retain their pre-audit SHAs.
- **No remote tag was created, modified, or deleted** by task-012.
  All 8 tags enumerated in §3.2 retain their pre-audit SHAs and
  lightweight-tag status.
- **No GitHub Release object was created, modified, or deleted** by
  task-012. Release `id=264993969` retains its pre-audit fields and
  its single asset (`id=344432236`, `bcsp-20260122.zip`).
- **No release asset was uploaded, modified, downloaded, or deleted**
  by task-012. The asset's `download_count` of `2` reflects pre-audit
  state and was not incremented by this audit (the audit fetched only
  the JSON metadata via `/repos/.../releases`, not the asset payload).
- **`origin/main` was not modified.** The audit read its SHA via
  `git ls-remote --heads origin` and never pushed to `refs/heads/main`.
- **No source or product file was edited** by this task. Specifically:
  - No file under `api/`, `frontend/`, `workers/`, `notifications/`,
    `configs/`, or `data/` was created, edited, or deleted (these are
    the task's explicit blocklist paths).
  - No file under `scripts/` was edited.
  - No package manifest (`package.json`, `package-lock.json`,
    `frontend/package.json`, `frontend/package-lock.json`,
    `tsconfig.json`, `frontend/tsconfig.json`, `pyproject.toml`) was
    edited.
  - No file under `.orchestrator/` other than the present report was
    created or edited.
  - No `.env`, `.env.*`, `*.user.json`, or `*.local.json` was created,
    read, or written.
- **No PR was opened, commented on, merged, or closed** by this task.
- **No branch-protection rule was created, modified, or deleted** by
  this task. The audit did not have credentials to read protection
  state; it likewise did not attempt to write protection state.
- **No GitHub repo settings (default branch, visibility, archived
  state, features) were modified.** The audit only read `GET
  /repos/.../`.
- **No file on any branch other than `feature/task-012`** was modified.

---

## 8. Scope and write inventory

| Write target | Path | Mode |
|---|---|---|
| This report | `.orchestrator/stage-p/05-public-remote-surface-audit.md` | created on `feature/task-012` |
| (none) | — | no remote ref change |
| (none) | — | no tag change |
| (none) | — | no GitHub Release object or asset change |
| (none) | — | no GitHub repo setting change |

Read inventory (informational, non-mutating):

| Read target | Method | Purpose |
|---|---|---|
| `git ls-remote --heads origin` | git over HTTPS, fetch-only | §3.1 branch enumeration |
| `git ls-remote --tags origin` | git over HTTPS, fetch-only | §3.2 tag enumeration |
| `git fetch origin --prune` | git over HTTPS, fetch-only | refresh remote-tracking refs |
| `git for-each-ref refs/tags` | local | §3.2 lightweight vs annotated tag check |
| `git merge-base --is-ancestor 9c93170…  bfa69a63…` | local | §3.1 dev-vs-main ancestry note |
| `GET https://api.github.com/repos/VVittgenstein/Rutgers-BetterCourseSchedulePlanner` | curl, unauthenticated | §1.1 repo metadata |
| `GET https://api.github.com/repos/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/releases` | curl, unauthenticated | §3.3 release enumeration |
| `.orchestrator/stage-p/04-public-cutover-and-local-cleanup.md` | local read | scope context and `dev`/`main`/`candidate` pre-audit values |

---

## 9. Handoff and next steps (audit-only)

This audit produces the inventory, classification, and exact deletion
plan required by task-012. It does **not** authorize execution. The
next task in the closeout sequence — referred to here generically as
"the future deletion task" — should:

1. Reconfirm classification of items in §4.3 (ESCALATE) with the
   Main Agent / human, and either move them to DELETE or keep them.
2. Provision an authenticated GitHub credential with at least
   `public_repo` scope (for the release-object deletion) and a Git
   push credential (for branch/tag deletion).
3. Execute the plan in §5 in the documented order, with the §5.1
   pre-flight and §5.6 post-flight verification gates.
4. Add a permanent local guard against `git push origin dev` per §6,
   so the public surface cannot be regressed by an accidental push.
5. Produce a sibling report
   `.orchestrator/stage-p/06-public-remote-surface-cleanup.md` (or
   similar numbering) recording the actual deletions performed,
   command outputs verbatim, and the post-flight verification results.

Stage P concludes when the public remote surface matches the goal
stated by the Main Agent: `origin` exposes exactly `refs/heads/main`
at SHA `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4` (or a fast-forward
descendant), no tags, no releases, and no other branches — at which
point the repository "presents as one clean project on `main` only."
