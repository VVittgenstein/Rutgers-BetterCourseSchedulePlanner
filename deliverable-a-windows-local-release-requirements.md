# Deliverable A Windows Local Release Requirements — Mainline Evidence Ledger

> **P1 WORKING EVIDENCE — NOT FINAL**
>
> This is a provenance-preserving P1 working artifact, not an approved product contract, a P2 `all and only` decision, or an implementation plan. It records only the required mainline export. Evidence from the recovery corpus is intentionally left for the separately scoped successor pass, after which the Human-led P1 Review must correct and approve the A target.

## Scope and evidence rules

Deliverable A is the Windows-local BCSP release. This pass recovers what the direct user said about A, the P1/P7 workflow that governs A, later direct-user corrections, and unresolved A questions. It does not decide which current product surfaces to keep, repair, remove, or defer.

The following controls apply throughout this document:

- A statement is direct-user evidence only when its excerpt is inside a genuine `### N. User` block and is authored by the user. User-role wrappers can also contain injected metadata or quoted branch text; those are classified separately.
- A Codex proposal remains Codex synthesis even when a later user message accepts it elliptically. The acceptance is direct-user evidence, but the proposal's detailed wording is not silently converted into a direct quote.
- Later, more specific user corrections control over earlier or broader statements. Supersession is recorded without deleting the earlier evidence.
- B-only product and deployment discussion is excluded from the A ledger. Shared A/B behavior is included only where the user explicitly linked A and B or later made the behavior common.
- The canonical source path is repeated literally in every ledger row to make path corruption mechanically visible.

## Source fingerprint and completeness

| Property | Mechanically observed value |
|---|---|
| Canonical source | `Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md` |
| SHA-256 | `378c1e41e2b3dbb483e92c54d5e8939d63da20df9a6e6024201f7e10d4a3c607` |
| Physical lines | `1,132` using `[System.IO.File]::ReadAllLines(...).Count` |
| Exported message blocks | `69` headings matching `^### [0-9]+\.` |
| User-role blocks | `29` headings matching `^### [0-9]+\. User` |
| Codex-role blocks | `40` |
| Export-declared counts | Line 12 independently declares `69 (29 user, 40 Codex)` |
| Complete-read coverage | Lines `1-1,132`, including export metadata, all 69 message blocks, quoted branch text, injected metadata, and the trailing incomplete `pm-log` commentary |

“Physical line” means an element returned by `ReadAllLines`, including blank-line-separated Markdown structure. `Measure-Object -Line` was not used because it can undercount this export.

## Canonical direct-user A and workflow ledger

The `A` class is intentionally gap-free from `A-001` through `A-046`. “Normalized English statement” is a concise interpretation; the adjacent excerpt is the controlling historical text.

<!-- LEDGER:A BEGIN -->
| ID | Normalized English statement | Provenance class | Canonical source path | Message | Lines | Short verbatim excerpt | Ambiguity or supersession note |
|---|---|---|---|---|---|---|---|
| A-001 | Read the recovery directory and project history files to regain complete awareness and memory of the project. | P1 scope/workflow directive | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M1 | L21 | 请看Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner以及项目中的历史记录文件，获得完整的对项目的感知和记忆。 | Discovery directive; it does not itself establish a product feature. |
| A-002 | Inspect the current state read-only and recover the purpose of the entire previously dispatched round, not only task-015. | P1 scope/workflow directive | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M7 | L60 | 我们去只读当前的状况，然后我们要讨论出一个新的任务。但，首先请你先从历史记录中恢复出我们上一次派发了但是没有完成的任务是什么？不是task015这一个，是一整轮派发的任务的目的是什么？ | The immediate question was answered by Codex in M8; its evidence-recovery method remains relevant to P1. |
| A-003 | Deliver A as a Windows package that any person can use and start with one BAT action. | Direct user statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M9 | L91 | A. 一个任何一个人都可以使用的WINDOWS包，通过bat一键启动。 | “Any person,” packaging contents, and clean-machine acceptance still require later product review and tests. |
| A-004 | Keep the prior task requirements for A unchanged unless a later direct-user correction supersedes them. | Direct user statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M9 | L92 | 对于A来说，先前的任务要求不变 | This is a reference to external history, not a complete inline specification. Later explicit corrections such as removal of email take precedence. |
| A-005 | Turn the historically runnable but drifted project into a real local BCSP release: promised Phase 1 functions must work, be verifiable and documented, while obsolete entry points, stubs, fake UI or docs, and deprecated configuration must be removed, hidden, or explicitly deferred. | Direct user statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M9 | L92 | Phase 1 本地 release 承诺的功能必须真的能用、能验证、能文档化；不该出现在 Phase 1 的旧入口、stub、假 UI、假文档、废弃配置要删除、隐藏或明确延期。 | This is the governing principle, not the final itemized P2 surface decision. |
| A-006 | Recover the user's exact earlier wording and requirements for A, then ensure the eventual product satisfies them. | P1 scope/workflow directive | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M9 | L92 | 具体我当时是怎么说的、提出了什么要求，这些需要被提取然后满足 | This mainline-only pass cannot prove completeness against the separately named recovery and raw-history sources. |
| A-007 | Discuss the complete task line from zero to completion, divide it into phases, and only then ask NGAT to decompose phases into tasks. | P1 scope/workflow directive | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M12 | L139 | 先把完整的任务线从0到完成讨论清楚。拆成一个个phase然后交给NGAT要它去拆成一个个task。 | Corrects Codex's earlier inclination to begin with a single task. |
| A-008 | Operate two distinct lines: the present discussion/decision line and the NGAT execution line, with different responsibilities. | P1 scope/workflow directive | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M14 | L204 | 如果使用NGAT我们会是双线进行：我们现在讨论的线和NGAT执行的线，不同的线负责不同的工作。 | The injected image wrapper around this request is non-user material recorded in N-006 and N-007. |
| A-009 | In P1, NGAT must read-only extract A's descriptions, goals, constraints, purpose, and expectations from the project, recovery directory, and possibly identified Codex or Claude JSONL histories, then persist the result. | P1 scope/workflow directive | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M14 | L213 | P1:先从项目目录/Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner（或许还有Z:\.claude和Z:\.codex的对话jsonl）中，把A的描述、目标、约束、目的、期望以只读的形式提取出来，落盘成文件。 | “Perhaps” makes raw JSONL conditional rather than mandatory; this row does not authorize an indiscriminate raw-history search. |
| A-010 | At the end of P1, NGAT must stop and return to the main discussion line for final review, discussion, correction, and persistence of A's requirements. | P1 scope/workflow directive | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M14 | L213 | 这个时候NGAT线需要停下来，回到我们的这个线，我们来对这些要求进行最终的审核、讨论并且再落盘成文件。 | This document is deliberately pre-review working evidence. |
| A-011 | P2 must determine A's `all and only` delivery surface using the local release, remote GitHub repository, goals, and local historical files. | P1 scope/workflow directive | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M14 | L215 | 我们的目标、本地的历史文件共同得出一个我们最终到底要交付什么“all and only”的东西 | This is a future P2 directive; no keep, repair, remove, or defer decision is made here. |
| A-012 | P3 must design and persist the complete implementation plan for A. | P1 scope/workflow directive | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M14 | L217 | 这一阶段是设计的阶段，需要设计完整的对于A的实现计划，需要落盘。 | Planning directive only. |
| A-013 | P5 must classify A/B capabilities by ownership, conflict, reuse, and sharing, then persist the conclusions. | P1 scope/workflow directive | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M14 | L221 | 什么东西是A的什么东西是B的，哪些东西是 冲突/可以复用/共用 的。并且把结论下到本地 | Exact taxonomy was later expanded by Codex; only the user's listed categories are direct evidence here. |
| A-014 | P6 must merge P5's results into the separate A and B implementation plans, persist them, and stop NGAT for main-line pre-execution audit. | P1 scope/workflow directive | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M14 | L223 | 把P5的结果分别和A/B的实现计划合并，落盘本地。然后NGAT需要停下来，我们这条线来进行执行前的审计。 | Stop gate is mandatory. |
| A-015 | P7 is the approved execution phase and must produce two packages, with A delivered as a release package. | P1 scope/workflow directive | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M14 | L225 | 对于A的包应该是一个release包，对于B的包应该是一个部署包。 | B's deployment package is outside the A product ledger but remains workflow context. |
| A-016 | Every P7 task must be committed to the remote, with a clean and visually satisfactory GitHub contribution history. | P1 scope/workflow directive | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M14 | L225 | 我要求这个PHASE每个task都需要给远端提交一遍，如图所示我希望这个图看起来好看一些。 | Later Codex added safety/branch qualifications; those qualifications are Codex synthesis unless separately approved. |
| A-017 | If feasible, place A's release package and B's deployment package on the GitHub Releases page. | P1 scope/workflow directive | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M14 | L225 | release包和远端部署包如果可以的话应该放在GITHUB上的RELEASE界面。 | Explicitly conditional: “if possible.” |
| A-018 | Produce a detailed end-to-end process and discussion checklist, including what must be discussed on the main line. | P1 scope/workflow directive | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M16 | L275 | 我希望你先写一个详细流程+讨论清单，从开始到结束，完整的流程是什么？到我们的流程的时候我们需要讨论什么？ | Workflow documentation request. |
| A-019 | Summarize the complete line and explicitly identify which line owns every task. | P1 scope/workflow directive | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M17 | L279 | 请你现在汇总一下我们整条完整的线。你需要明确每个任务是哪条线负责的。 | Consecutive refinement of A-018. |
| A-020 | Accept the Codex-authored M18 workflow summary for persistence, then move discussion to 0A and 0B. | Later accepted decision | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M19 | L322 | 没问题，请把你刚刚写的“完整流程”落盘，然后我们来讨论0A和0B | Acceptance is elliptical; the detailed workflow remains Codex synthesis and is separately identified in N-008. |
| A-021 | Remove email reminders from both A and B and mark the removed feature on GitHub as future development. | Conflict or superseded statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M21 | L358 | 我们不再附带邮箱提醒了，不论是公网网站还是本地部署的包。这个功能删除并且在GTIHUB上标记为待开发 | This later explicit correction controls over any historical email promise referenced generically by A-004. “GTIHUB” is preserved from the source. |
| A-022 | The public B experience and UI must be the same as the WebUI opened from local A. | Direct user statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M21 | L359 | 公网网站的体验、UI应该完全和本地部署然后在WEBUI中打开是一样的 | Shared ordinary-user UI; deployment and administration surfaces were explicitly deferred for later UI discussion. |
| A-023 | Correct the reminder model: there is no email-versus-sound tradeoff; A and B have sound reminders only. | Conflict or superseded statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M23 | L416 | 我说了本地A和公网B都没有邮件提醒只有声音提醒。 | Reinforces and sharpens A-021; supersedes Codex suggestions of other reminder tradeoffs. |
| A-024 | Because A and B are the same in this respect, do not introduce unrequested Web Push, app, or system-notification concepts. | Conflict or superseded statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M27 | L525 | 我说了 是A和B是相同的，所以你为什么要提“Web Push / app / 系统通知”，没人提过这个 | Scope correction; current delivery behavior is WebUI sound, not an additional notification system. |
| A-025 | Cached course information must itself have an update mechanism. | Direct user statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M27 | L526 | 课程资料本地缓存也需要有更新啊 | Spoken during B discussion; A relevance comes from the user's repeated shared A/B behavior and later A-specific refresh configuration in A-041. |
| A-026 | A's computation and reminders run locally on the user's computer. | Conflict or superseded statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M32 | L634 | 公网A的运算、提醒肯定都是电脑本地啊 | “公网A” is an apparent slip in a contrast with public B; normalized as local A because the same sentence says computation/reminders are local. |
| A-027 | Use an active-session subscription switch: the user can start or stop only while the browser is open; closing the browser deactivates watching; starting sends a request to the service. | Direct user statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M34 | L672 | 开始订阅/关闭订阅，他只能在浏览器开的时候点击开始/关闭，如果关闭浏览器就自动变成关闭状态，开始的时候就自动向服务器发请求。 | Expressed for B; later A-028 explicitly reasserts that A and B have the same ordinary-user behavior. Transport implementation is not decided for A here. |
| A-028 | Public B must have the same functionality as local A; this is a correction of Codex's repeated attempt to narrow B. | Conflict or superseded statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M38 | L752 | 公网B需要和本地A相同的，我前面已经说过了 | “Same” applies to ordinary-user scope; later workflow still distinguishes A/B runtime and deployment. |
| A-029 | Both A and B must support subscriptions by section; verify whether the section number is sufficiently unique. | Ambiguity / needs user review | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M38 | L753 | A 和 B 都需要能订阅section，因为我记得所有section的数字是唯一的？ | Subscription is required; identifier uniqueness is explicitly uncertain and needs evidence before choosing the key. |
| A-030 | The WebUI must provide audio-volume adjustment. | Direct user statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M38 | L754 | UI上需要有音量大小的调整 | Applies simultaneously to A and B. |
| A-031 | The WebUI must let the user switch between one-shot and continuous audio. | Direct user statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M38 | L754 | 一声还是持续的切换 | Exact continuous-alert stop/acknowledgement semantics remain unspecified. |
| A-032 | Do not debounce alert playback. | Direct user statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M38 | L754 | 不做防抖。 | A later design must reconcile this with one-shot versus continuous behavior without silently adding debounce. |
| A-033 | Do not trigger only on state transitions: while a subscribed section remains Open, every status message from the serving layer must trigger sound. | Direct user statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M38 | L755 | 订阅不能靠状态变化触发，如果一个section一直是open，也应该提醒，并且每次服务器发送消息就要触发声音。 | For A, “server” means its local serving layer; exact message cadence and continuous-mode behavior need later design. |
| A-034 | Accept that sound is not guaranteed while the device is locked or the browser is in the background. | Later accepted decision | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M38 | L756 | 锁屏/后台时不保证声音，这个限制接受 | Acceptance is explicit. |
| A-035 | For both A and B, ideally keep the interval from course-status update to audible alert under one second, while recognizing that A and B differ. | Ambiguity / needs user review | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M38 | L757 | 对于A和B，这个课程状态更新到提醒，最好能压在1秒以内，但是A和B毕竟是有区别的 | “Ideally” is a target, not yet a hard acceptance threshold; measurement start/end and tolerance remain unresolved. |
| A-036 | Refresh the course catalog every 10 or 15 minutes. | Conflict or superseded statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M38 | L758 | 课程目录建议15分钟或者10分钟更新一次 | Superseded by A-041: default 10 minutes, configurable only for A. |
| A-037 | Defer discussion of the precise A/B UI-consistency boundary until the UI is written. | Ambiguity / needs user review | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M38 | L759 | UI 一致性边界这个写UI的时候再讨论 | This is an explicit unresolved decision, not authority to invent the boundary during P1. |
| A-038 | Keep active subscription state only in live connection memory. | Direct user statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M38 | L760 | active subscription 只存在连接内存就行了 | Expressed in shared A/B discussion; persistent selection history is not established by this sentence. |
| A-039 | A single browser must not subscribe to 10 sections simultaneously. | Conflict or superseded statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M38 | L762 | 在浏览器上，一个人不能同时订阅10个section | Ambiguous lower ceiling resolved by A-040 as nine. |
| A-040 | Set the per-browser active-subscription maximum to nine sections. | Later accepted decision | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M40 | L799 | 9个吧 | Direct answer to Codex's M39 request for the exact limit; supersedes A-039's open ceiling. |
| A-041 | Refresh the course catalog every 10 minutes by default; expose interval configuration only in local A. | Later accepted decision | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M40 | L800 | 课程目录刷新默认10分钟，但是只有本地A是可以配置的 | Supersedes A-036. B's fixed policy is context; A's configurability is direct A evidence. |
| A-042 | P7 must contain a UI subphase that uses the `industrial-brutalist-ui` and `design-taste-frontend` skills to build the UI. | P1 scope/workflow directive | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M53 | L1009 | 其中一个是使用[$industrial-brutalist-ui](C:\\Users\\Administrator\\.codex\\skills\\industrial-brutalist-ui\\SKILL.md) 和[$design-taste-frontend](C:\\Users\\Administrator\\.codex\\skills\\design-taste-frontend\\SKILL.md) 做UI | The doubled backslashes are the literal exported Markdown link text. Skill execution belongs to future P7, not this evidence task. |
| A-043 | P7 must contain another subphase that uses `emil-design-eng` to polish the UI. | P1 scope/workflow directive | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M53 | L1009 | 另外一个是使用[$emil-design-eng](C:\\Users\\Administrator\\.codex\\skills\\emil-design-eng\\SKILL.md) 去打磨UI | Must follow, not merge into, the build subphase. |
| A-044 | The UI-build and UI-polish activities must be two independent P7 subphases. | P1 scope/workflow directive | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M53 | L1009 | 这必须是两个独立的SUBPHASE。 | Independence is explicit; later Codex details about task/commit structure are synthesis unless approved elsewhere. |
| A-045 | Rewrite the workflow file because the branch introduced unnecessary changes. | Conflict or superseded statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M57 | L1025 | 你需要重写这个流程文件 因为支线引入了不必要的改动 | Immediately narrowed by M58/M59; do not use as authority to rewrite 0A-P6. |
| A-046 | Apply the correction only to P7 because the branch accidentally changed P7; preserve the other workflow phases. | Conflict or superseded statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M59 | L1033 | 或者只修改P7的，因为支线只不小心动了P7的 | Latest correction controlling A-045. M58 is an almost identical duplicate and is accounted for as X-020. |
<!-- LEDGER:A END -->

## Non-user provenance controls

All 40 blocks whose headings say `Codex` are non-user by role and are excluded from the direct-user ledger. The rows below identify the highest-risk synthesis and wrapper material: content that is easy to misread as user intent because it summarizes requirements, is later accepted elliptically, or appears inside a `User` block.

<!-- LEDGER:N BEGIN -->
| ID | Normalized English statement | Provenance class | Canonical source path | Message | Lines | Short verbatim excerpt | Ambiguity or supersession note |
|---|---|---|---|---|---|---|---|
| N-001 | The export header is machine-generated session metadata, including declared message counts and rollback information. | Machine-injected metadata | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | META | L1-L15 | Exported messages: 69 (29 user, 40 Codex) | Not a conversation message and never direct-user intent. |
| N-002 | Codex's first project-memory summary describes Phase 1, task-015, drift, and current NGAT state. | Codex summary or historical synthesis | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M6 | L41-L56 | Phase 1 的目标是做一个可本地发布、可启动、可抓取 SOC 数据、可搜索筛选、可订阅/通知的完整本地 release。 | Useful lead only; source evidence must be checked before treating any listed feature as user-required. |
| N-003 | Codex reconstructs the purpose and feature outline of the prior task-015-to-task-025 round. | Codex summary or historical synthesis | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M8 | L62-L86 | Phase 1：Feature-complete local release | Directly answers M7 but remains Codex synthesis. |
| N-004 | Codex proposes a dual-delivery contract and enumerates candidate A acceptance areas. | Codex summary or historical synthesis | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M11 | L101-L135 | A：Windows Local Release Contract | Proposal, not direct-user wording. |
| N-005 | Codex proposes an initial Phase 0-6 line before the user corrects it with the dual-line P1-P7 model. | Conflict or superseded statement | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M13 | L143-L192 | Phase 0：项目恢复与产品契约冻结 | Superseded as the controlling workflow by the user's M14 correction. |
| N-006 | A machine-injected local-image declaration appears at the start of M14's User block. | Machine-injected metadata | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M14 | L196-L203 | [local_images 1: str] | The authored request begins after `My request for Codex`; the wrapper is not user intent. |
| N-007 | A machine-injected image wrapper and omission marker appear at the end of M14's User block. | Machine-injected metadata | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M14 | L230-L234 | [input_image omitted] | No visual requirement is inferred from the omitted image; only the user's surrounding text is ledgered. |
| N-008 | Codex turns the dual-line discussion into a detailed workflow table that M19 later accepts for persistence. | Codex summary or historical synthesis | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M18 | L283-L318 | 对，我现在把整条线整理成一个明确的“双线流程”。 | M19 proves acceptance of this synthesis, recorded as A-020, but does not make M18 a verbatim user statement. |
| N-009 | Codex consolidates the then-current 0A choices and proposes unified polling plus live push, which M42 accepts only for B. | Codex summary or historical synthesis | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M41 | L804-L821 | 我建议明确采用统一 poll + live connection 推送，浏览器不直接拉 Rutgers。 | The later acceptance is B-only and excluded as X-015. |
| N-010 | M47 contains a branch agent's handoff pasted by the user, including Vultr and Rust claims. | Quoted branch material | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M47 | L896-L920 | 已经为主线完成详细交接并落盘。 | The user's act is “please review”; the pasted branch assertions are not direct-user intent or acceptance. |
| N-011 | Codex reports that it verified and adopted the branch's 0A, 0B, and 0C state. | Codex summary or historical synthesis | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M50 | L932-L948 | 0C 已新增并接受 | This is Codex's assessment; no direct mainline user statement in M47 says “I accept 0C.” |
| N-012 | M51 is an injected plugin catalog and environment context presented under a User heading. | Machine-injected metadata | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M51 | L952-L976 | <recommended_plugins> | Entire block excluded from user intent. |
| N-013 | M52 repeats the injected plugin catalog and environment context under a User heading. | Machine-injected metadata | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M52 | L980-L1004 | <recommended_plugins> | Entire block excluded from user intent; duplicate injection does not create a requirement. |
| N-014 | Codex reports the workflow-file edits it performed after the user's P7 correction. | Codex summary or historical synthesis | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M63 | L1049-L1059 | 已仅重写 P7 及其责任边界 | Implementation report, not a direct-user statement; user corrections A-045 and A-046 are controlling. |
| N-015 | Codex provides the final dual-line workflow summary, including detailed 0C and P7 claims. | Codex summary or historical synthesis | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M66 | L1071-L1120 | 完整流程以 [dual-delivery-workflow.md] | The user asked for a summary in M64 but did not subsequently approve every detail before invoking `pm-log`. |
<!-- LEDGER:N END -->

## Explicit coverage exclusions

These are the direct-user-block segments that are B-only, contain no new A decision, duplicate a later controlling statement, or are not authored user content. They are recorded so omission cannot be mistaken for an unread source segment.

<!-- LEDGER:X BEGIN -->
| ID | Normalized English statement | Provenance class | Canonical source path | Message | Lines | Short verbatim excerpt | Ambiguity or supersession note |
|---|---|---|---|---|---|---|---|
| X-001 | The separate public website deliverable and low-cost hosting idea are B-only. | Coverage exclusion — B-only | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M9 | L93 | 对于B来说，我听到ORCL、CLOUDFLARE、GOOGLE这几家都有免费或者是价格低廉的服务器 | Does not define A; shared behavior appears only in later explicit A/B statements. |
| X-002 | Pre-NGAT discussion of B content, platform, purchases, and local SSH keys is B/deployment-only. | Coverage exclusion — B-only | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M14 | L208-L209 | 到底使用哪一台设备？Oracle Cloud / OCI Always Free？Cloudflare Pages + Workers + D1/R2？Google Cloud Run？ | No A product decision. |
| X-003 | P4's B implementation-plan requirement is outside this A evidence ledger. | Coverage exclusion — B-only | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M14 | L219 | P4:我们在前面已经有了B的目标线，这一阶段需要设计出完整的对B的实现计划 | P4 remains workflow context but does not define A. |
| X-004 | Main-line deployment of B after NGAT finishes is B-only. | Coverage exclusion — B-only | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M14 | L227 | 我会希望你带着我拿着这个部署包去部署在那台机器上面。 | No A decision. |
| X-005 | Concern about long-term-free server capacity and B optimization is B-only. | Coverage exclusion — B-only | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M21 | L360 | 我希望能是那种长时间免费的服务器，但是配置可能不够高？ | No A decision. |
| X-006 | Questions about placing B's database on users' computers and waiting for provider capacity are B-only. | Coverage exclusion — B-only | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M23 | L414-L415 | 但是这个能做到把数据库放在用户的电脑上吗？ | Later B discussion moves computation back to the server; neither option defines A. |
| X-007 | The accessibility purpose for non-GitHub, non-installing, non-Windows users defines B's audience only. | Coverage exclusion — B-only | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M23 | L417 | 就算不会上github、不会安装、不是windows也能直接访问这个服务。 | Not an A audience statement. |
| X-008 | The initial B-specific one-second polling, YouTube analogy, and hardware explanation request are precursors, not independent A requirements. | Coverage exclusion — B-only or later restated | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M25 | L468-L470 | 至少需要1秒更新一次状态 | The later explicit shared A/B target is captured at A-035; the hardware explanation request adds no A decision. |
| X-009 | OCI suitability, capacity, and server-centered multi-device delivery questions are B-only. | Coverage exclusion — B-only | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M27 | L527 | OCI Always Free的目前看起来是最合适的？ | No A decision; the shared corrections in M27 are separately captured as A-024 and A-025. |
| X-010 | Browser-side Rutgers polling, mobile behavior, fan-out, latency, and server-load questions concern B architecture. | Coverage exclusion — B-only | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M30 | L584-L586 | 100台设备同时拉罗格斯的openSections会不会压力大？ | No direct A architecture decision. |
| X-011 | The confirmation question about a public server tracking Open status and notifying subscribed browsers is B-only. | Coverage exclusion — B-only | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M32 | L635 | 我们的服务器维护一个OPEN状态和被订阅的课程 | A's local-computation correction in the preceding line is captured as A-026. |
| X-012 | The question of how a public server detects which browsers remain open is B-only. | Coverage exclusion — B-only | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M34 | L672 | 我不知道服务器要怎么去知道现在还有哪些浏览器开着在。 | The shared active-session behavior in the same line is captured as A-027. |
| X-013 | Asking what remains undiscussed adds no A decision. | Coverage exclusion — no new A decision | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M36 | L716 | 现在根据完整流程，我们还有什么没讨论的？ | Codex's answer is non-user synthesis. |
| X-014 | Rejecting anonymous or Rutgers-only discussion is a B scope correction, not an A requirement. | Coverage exclusion — B-only | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M38 | L761 | 匿名？Rutgers-only？你讨论这个干什么？ | Records the exclusion without importing an A authentication decision. |
| X-015 | The user accepts Codex's unified-poll/live-connection recommendation for public B. | Coverage exclusion — B-only accepted decision | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M42 | L825 | 你的建议我采纳 | Elliptical acceptance points to Codex M41 and is limited by that B-only context; it is not direct A architecture. |
| X-016 | Starting the 0B discussion adds no A decision. | Coverage exclusion — B-only | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M45 | L842 | 好，我们现在来讨论0B | Transition only. |
| X-017 | The user asks the main line to review a pasted branch response; the pasted assertions are not authored mainline intent. | Coverage exclusion — quoted branch material | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M47 | L894-L920 | 我刚刚开启了支线，以下是支线的回复，请查阅 | Quoted material is classified as N-010; review is not blanket acceptance. |
| X-018 | The first recommended-plugin and environment block is machine-injected. | Coverage exclusion — machine metadata | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M51 | L952-L976 | Here is a list of plugins that are available but not installed. | Classified as N-012; no authored user request. |
| X-019 | The repeated recommended-plugin and environment block is machine-injected. | Coverage exclusion — machine metadata | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M52 | L980-L1004 | Here is a list of plugins that are available but not installed. | Classified as N-013; no authored user request. |
| X-020 | M58 is an almost identical intermediate copy of the controlling M59 P7-only correction. | Coverage exclusion — duplicate correction | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M58 | L1029 | 或者只修改P7的，因为支线只不小心动了P7D1 | The apparent `D1` typo is corrected in M59; A-046 records the latest controlling wording. |
| X-021 | Asking for the complete workflow summary adds no new A decision. | Coverage exclusion — no new A decision | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M64 | L1063 | 现在完整的双线交付工作流是什么？ | Codex M66 is a synthesis, classified as N-015, and was not followed by line-item user approval. |
| X-022 | Invoking `pm-log` requests conversation archival and adds no A product or workflow decision. | Coverage exclusion — no new A decision | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M67 | L1124 | [$pm-log](C:\\Users\\Administrator\\.codex\\skills\\pm-log\\SKILL.md) | Archival request only. |
<!-- LEDGER:X END -->

## Unresolved A questions and deferred decisions

These `Q` records duplicate the controlling source lines intentionally so downstream P1 Review cannot overlook what remains unresolved.

<!-- LEDGER:Q BEGIN -->
| ID | Normalized English statement | Provenance class | Canonical source path | Message | Lines | Short verbatim excerpt | Ambiguity or supersession note |
|---|---|---|---|---|---|---|---|
| Q-001 | What exact prior A requirements exist in the project, recovery corpus, and any justified raw histories? | Ambiguity / needs user review | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M9 | L92 | 具体我当时是怎么说的、提出了什么要求 | This file answers only for the required mainline export; successor evidence passes and Human P1 Review remain necessary. |
| Q-002 | Is a Rutgers section number unique in the required scope, or must the subscription key include term, campus, or other dimensions? | Ambiguity / needs user review | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M38 | L753 | 我记得所有section的数字是唯一的？ | Must be verified against authoritative data/contracts before implementation. |
| Q-003 | What measurable A-specific acceptance rule implements the “ideally under one second” status-to-audio target? | Ambiguity / needs user review | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M38 | L757 | 最好能压在1秒以内，但是A和B毕竟是有区别的 | Define measurement endpoints, load, percentile, Rutgers latency treatment, and whether failure blocks release. |
| Q-004 | Where exactly does shared ordinary-user UI end and A/B-specific configuration or administration begin? | Ambiguity / needs user review | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M38 | L759 | UI 一致性边界这个写UI的时候再讨论 | Explicitly deferred; P1 must preserve the question without deciding it. |
| Q-005 | Which concrete current surfaces belong in A's final `all and only` delivery boundary? | Ambiguity / needs user review | Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md | M14 | L215 | 到底什么东西是属于“该有的功能必须完整可用”，什么是属于“不该有的东西一个不留”。 | Reserved for P2 after Human approval of the P1 requirements; prohibited in this evidence pass. |
<!-- LEDGER:Q END -->

## User-role block coverage index

Every one of the export's 29 User-role blocks is accounted for below. Mixed blocks can map to both retained evidence and exclusions.

<!-- USER-COVERAGE BEGIN -->
| User message | Header line | Retained evidence | Excluded or controlled material |
|---|---:|---|---|
| M1 | L19 | A-001 | None |
| M7 | L58 | A-002 | None |
| M9 | L88 | A-003-A-006; Q-001 | X-001 |
| M12 | L137 | A-007 | None |
| M14 | L194 | A-008-A-017; Q-005 | X-002-X-004; N-006-N-007 |
| M16 | L273 | A-018 | None |
| M17 | L277 | A-019 | None |
| M19 | L320 | A-020 | N-008 controls the accepted Codex synthesis |
| M21 | L355 | A-021-A-022 | X-005 |
| M23 | L412 | A-023 | X-006-X-007 |
| M25 | L466 | None | X-008 |
| M27 | L523 | A-024-A-025 | X-009 |
| M30 | L582 | None | X-010 |
| M32 | L632 | A-026 | X-011 |
| M34 | L670 | A-027 | X-012 |
| M36 | L714 | None | X-013 |
| M38 | L750 | A-028-A-039; Q-002-Q-004 | X-014 |
| M40 | L797 | A-040-A-041 | None |
| M42 | L823 | None | X-015; N-009 controls referenced synthesis |
| M45 | L840 | None | X-016 |
| M47 | L892 | None | X-017; N-010 |
| M51 | L950 | None | X-018; N-012 |
| M52 | L978 | None | X-019; N-013 |
| M53 | L1006 | A-042-A-044 | None |
| M57 | L1023 | A-045 | Superseded by A-046 |
| M58 | L1027 | None | X-020 duplicate correction |
| M59 | L1031 | A-046 | None |
| M64 | L1061 | None | X-021; N-015 controls answer |
| M67 | L1122 | None | X-022 |
<!-- USER-COVERAGE END -->

## Supersession and interpretation summary

- A-021 and A-023 are the controlling direct-user correction for email: the current A delivery has sound reminders and no email reminder. The generic “prior requirements unchanged” in A-004 cannot revive a superseded email promise.
- A-024 rejects expansion into Web Push, a native app, or system notifications. This does not erase the accepted WebUI-open/background limitation in A-034.
- A-036 is replaced by A-041: the catalog default is 10 minutes, and only A exposes configurability.
- A-039 is resolved by A-040: the active per-browser maximum is nine sections.
- A-045 is narrowed by the duplicate M58/M59 correction; A-046 controls and limits the rewrite to P7.
- M47's Vultr/Rust/0C material and M50/M66's summaries may be relevant to later architecture work, but this mainline export does not contain a direct-user sentence that adopts every pasted or summarized detail. They remain non-user evidence here.

## Mechanical verification method and recorded result

The validation was run from the task worktree with PowerShell against the source and this committed-path candidate. It performed these checks rather than relying on visual inspection:

1. Recomputed source SHA-256 and physical-line count.
2. Parsed all message headings, requiring exactly messages 1-69 with 29 `User` and 40 `Codex` roles.
3. Parsed each marked ledger as an eight-field Markdown table.
4. Required gap-free, unique IDs independently for `A`, `N`, `X`, and `Q`.
5. Required every ledger path field to equal the canonical path byte-for-character, which also rejects inserted spaces.
6. Parsed every `Lx` or `Lx-Ly` locator, required it to fall inside the declared message block, and required the verbatim excerpt to occur exactly within that source slice. `META` is limited to the export header.
7. Required every `A`, `X`, and `Q` source message to have the `User` role.
8. Parsed the coverage index and required its 29 message IDs to equal the complete set of User-role message IDs.
9. Checked the English title, exact working-evidence banner, source statistics, table delimiters, and `git diff --check`.

The core validation command was an inline PowerShell parser (no validator file was created):

```powershell
$source = 'Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md'
$doc = 'deliverable-a-windows-local-release-requirements.md'
$canonical = 'Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md'
$sourceLines = [System.IO.File]::ReadAllLines($source)
$docText = [System.IO.File]::ReadAllText((Resolve-Path $doc))
# Build message-number, role, start-line, and end-line maps from ^### N. Role headings.
# For LEDGER:A/N/X/Q markers: split each row on Markdown delimiters, require 8 fields,
# validate class IDs, exact path, message bounds, locator bounds, and exact excerpt containment.
# For USER-COVERAGE markers: compare listed message IDs with the parsed User-role ID set.
```

Recorded result for this version:

| Check | Result |
|---|---|
| Source SHA-256 | PASS — `378c1e41e2b3dbb483e92c54d5e8939d63da20df9a6e6024201f7e10d4a3c607` |
| Source physical lines | PASS — `1,132` |
| Message roles | PASS — `69` total, `29` User, `40` Codex |
| Direct-user ledger | PASS — `46` rows, `A-001-A-046`, unique and gap-free |
| Non-user controls | PASS — `15` rows, `N-001-N-015`, unique and gap-free |
| Coverage exclusions | PASS — `22` rows, `X-001-X-022`, unique and gap-free |
| Open questions | PASS — `5` rows, `Q-001-Q-005`, unique and gap-free |
| Field count and Markdown structure | PASS — all `88` ledger rows have exactly `8` fields and all marked tables have one header and delimiter row |
| Canonical paths | PASS — `88/88` exact; zero inserted-space variants |
| Message and line locators | PASS — `88/88` in bounds and inside the declared block/header scope |
| Verbatim excerpts | PASS — `88/88` exact source-slice matches |
| User-role coverage | PASS — `29/29` unique User messages accounted for |
| Scope/write check | PASS — only `deliverable-a-windows-local-release-requirements.md` is created by task-052 |
| Whitespace check | PASS — `git diff --check` |

## P1 stop condition

This ledger is ready for independent review and for the separately scoped recovery-evidence append pass. It must remain labeled **P1 WORKING EVIDENCE — NOT FINAL** until the Human-led P1 Review has reconciled all evidence sources, resolved or explicitly deferred the questions above, corrected normalized interpretations where necessary, and approved the A target. No P2 product-surface judgment is made by this file.
