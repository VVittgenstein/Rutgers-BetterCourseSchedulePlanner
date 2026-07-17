# Stage A/P 远端收口保存点 / Public Remote Closeout Savepoint

**状态**: ✅ 成功  
**日期**: 2026-05-12  

## 目标

本次保存记录的是一个长会话的最终状态：项目先完成 Stage A（清洗、剥离、整理），再完成 Stage P（把公开 GitHub 仓库收口到干净 public surface）。用户的核心诉求不是单个 Git 命令，而是把一个早期、混乱、曾经错误使用 GitHub 的项目重新整理成“本地内部线清楚、远端公开线干净”的状态。

工作在 2026-05-11 完成，pm-save 写入发生在 2026-05-12。最终结果是：ngagent `task-001` 到 `task-014` 全部完成/合并，公开远端 `origin` 只剩 `main`，没有 tags，没有 GitHub Releases；本地 `dev` 保留为内部协调线，不能推回 public `origin`。

## 做了什么

### 1. 明确项目历史问题与 Stage A 目标

**操作**:
- 用户先要求详细理解项目，特别是记录文件，并明确先不走 ngagent 流程。
- 用户把问题框定为三个层面：早期开发流不成熟、后续仓促修改导致 drift、早期技能/工作流低级导致实现和设计可能有问题。
- 用户提出把工作拆成两个阶段：Stage A = 清洗/剥离/整理；Stage B = 重构。

**关键对话原文**:
> 我想和你聊几点，先不走ngagent的流程
> 1.整个项目的开发流是一个非常不成熟、早期的...
> 2.在初步开发后我做了很多次修改，这些修改有很多都是仓促的...
> 3.还有就是当时的技能和工作流都属于是早期而且是比较低级的...

> 我建议分成两个阶段，（清洗/剥离/整理）和（重构），因为一个旧的，不产出新的东西；一个是新的

**产出**:
- Stage A 被定义为旧状态清理和基线重建，不做产品重构。
- Stage B 被保留为后续新结构/新实现的重构阶段。

### 2. 使用 ngagent 完成 Stage A 与 Stage P

**操作**:
- 用户确认使用 ngagent 流程完成任务 A。
- 每个 delivery task 配对应 review gate；审查结果可以触发 retry/replan。
- Stage A 最终形成 7 个 delivery task 和 7 个 review work unit 的工作模型。
- Stage P 后续扩展出 `task-008` 到 `task-014`，用于公开远端同步与收口。

**关键对话原文**:
> 没问题，我的想法是走ngagent的流程去完成任务A

> 那现在应该是14个任务啊？还有就是你应该意识到了但是我还是提一嘴，就是加入审查以后就变成“递归规划”了，你可能需要根据审查的结果动态修改任务。

> 对，我希望你能用ngagent的流程。不过这次excutor是opus4.7max reviewer是5.5xhigh 你理解吗？

**产出**:
- `.orchestrator/stage-a/01-inventory.md` 到 `.orchestrator/stage-a/07-cleanup-application.md`。
- `.orchestrator/stage-p/01-public-divergence-and-exposure-policy.md` 到 `.orchestrator/stage-p/07-public-tags-releases-closeout.md`。
- ngagent 当前状态：`task-001` through `task-014` 全部 merged。

### 3. 解释 GitHub 上的 dev / PR / merge 困惑

**操作**:
- 用户发现 GitHub 仍显示 `dev had recent pushes` 和 `Can’t automatically merge`。
- 分析后确认：`main` 干净不等于整个 GitHub public surface 干净；远端其他 branches、tags、releases 也会构成公开展示面。
- 目标从“更新 main”扩展为“清理整个 public remote surface”。

**关键对话原文**:
> 我看到dev了 但这不是我想要的 我不懂gt和github 我就跟你说一下我到底想要什么 就是我想要整个库就是好的样子 哎 我不知道怎么表达

> 就 我的疑惑是 为什么会有dev？按理来说不应该就纯main吗？... 我真正想要的是一个完全的，“联通”的任务A。

**产出**:
- 决策：不做 raw `dev` -> `main` merge。
- 决策：public GitHub 只保留 sanitized `main`。
- 决策：local `dev` 保留为内部工作线，但 `origin/dev` 必须删除且不能再推回。

### 4. Stage P public remote closeout

**操作**:
- `task-012`：审计 public remote surface，只报告不删除。
- `task-013`：删除所有非 `main` 远端 branches。
- `task-014`：删除旧 tags、GitHub Release object 和 release asset。
- 每个 destructive step 都在审计和用户批准之后执行。

**关键对话原文**:
> `task-012` 已完成、通过 review、并已 merge。它没有改任何远端内容，只产出了审计报告...

> 远端现在还有 `147` 个 branch。
> 目标状态是只保留 `main@9c93170...`。
> 下一步 `task-013` 会删除 `146` 个非 `main` 分支，包括 `dev` 和 `public-main-candidate`。
> 后续 `task-014` 会删除 `8` 个旧 tag。
> 还有 1 个 GitHub Release：`Release-0122`，显示名 `Release-0121`，包含资产 `bcsp-20260122.zip`。

> 没问题 做

**产出**:
- `.orchestrator/stage-p/05-public-remote-surface-audit.md`（当前实测 962 行）— 审计 147 branches、8 tags、1 GitHub Release，并给出删除计划。
- `.orchestrator/stage-p/06-public-branch-closeout.md`（当前实测 436 行）— 记录删除 146 个非 main branches，0 failures。
- `.orchestrator/stage-p/07-public-tags-releases-closeout.md`（当前实测 353 行）— 记录删除 1 个 GitHub Release、1 个 asset、8 个 tags。

### 5. 项目记忆系统初始化并进入 pm-save

**操作**:
- 已存在三层记忆系统：`CLAUDE.md`、`docs/TIMELINE.md`、`docs/sessions/`，以及蒸馏层 `docs/DIGEST.md` + `docs/registry.json`。
- 本次 `$pm-save` 按 skill 要求启动四个工作单元：
  - A：事实与产出提取。
  - B：决策与推理链提取。
  - C：关键对话原文提取。
  - D：产出验证。

**产出**:
- `D:\Document\Temp\pm-save-8c1d2f86\subagent-a-facts.md`
- `D:\Document\Temp\pm-save-8c1d2f86\subagent-b-decisions.md`
- `D:\Document\Temp\pm-save-8c1d2f86\subagent-c-quotes.md`
- `D:\Document\Temp\pm-save-8c1d2f86\subagent-d-verification.md`

## 决策记录

| 决策 | 上下文 | 结论 | 提出者 | 状态 |
|------|--------|------|--------|------|
| Stage A / Stage B 拆分 | 项目历史混乱，不能直接重构 | Stage A 做清洗/剥离/整理；Stage B 再重构 | 用户 | 已确认 |
| Stage A 使用 ngagent | 用户希望任务有计划、审查、提交和可追踪状态 | 用 ngagent task/spec/review/merge 执行 | 用户 | 已确认 |
| 每个任务配 review gate | 用户指出 review 会带来递归规划 | 审查结果可触发 retry/replan | 用户 | 已确认 |
| 远端不是唯一真相 | 远端可能是先前选择性披露，本地 ignore 也可能错误 | 本地、远端、release、记录都作为 evidence layer 审计 | 用户 | 已确认 |
| 不 raw merge `dev` 到 `main` | 本地 `dev` 包含 ngagent / `.orchestrator` / `AGENTS.md` 等内部内容 | 构造 sanitized public main | 主 agent 提案，用户确认 | 已完成 |
| Public remote closeout 三步走 | main 已干净但 GitHub 仍显示 dev/PR/merge prompt | 审计 -> 删除非 main branches -> 删除 tags/releases | 主 agent 提案，用户批准 | 已完成 |
| 删除所有非 main branches | task-012 审计发现 147 branches | task-013 删除 146 个非 main branches，包括 `dev` 和 `public-main-candidate` | 用户批准 | 已完成 |
| 删除 tags/releases | task-012 审计发现 8 tags 和 1 Release | task-014 删除 8 tags、Release `264993969`、asset `bcsp-20260122.zip` | 用户批准 | 已完成 |
| local `dev` 保留但不能推回 public origin | public `dev` 删除后，误推会重新暴露内部记录 | 禁止 `git push origin dev`、`git push --all`、`git push --mirror` | 主 agent 提醒，用户接受 | 已确认 |

## 关键发现

1. **GitHub public surface 不只等于 default branch**: 即使 `origin/main` 已经干净，只要 `origin/dev`、历史 branches、tags 或 Releases 还存在，GitHub 仍会展示让用户困惑的 public state。
   - 影响: Stage P 必须覆盖 heads、tags、GitHub Releases 和 assets。

2. **用户真正要的是“联通”的 Stage A，而不是单纯 git merge**: “同步到远端”最终被解释为 public/internal 边界对齐，而不是把 local `dev` 原样推上去。
   - 影响: 采用 sanitized replay 和 remote closeout，而不是 raw `dev` -> `main`。

3. **Release pack drift 是真实风险**: GitHub Release `Release-0122` 显示名为 `Release-0121`，资产为 `bcsp-20260122.zip`；命名和历史状态本身就说明 release surface 不应继续保留。
   - 影响: task-014 删除 Release object、asset 和全部 tags。

4. **memory docs 当前是本地未跟踪文件**: `git status --short --branch` 显示 `CLAUDE.md` 和 `docs/` 下 memory docs 为 untracked。
   - 影响: 本次 pm-save 更新的是本地文档系统，不会自动提交或推送；后续是否纳入内部 `dev` 由用户另行决定。

## 失败与教训

- **只清理 `main` 不够**: task-011 后 public `main` 已经 clean，但 GitHub 仍显示 `dev had recent pushes` 和 `Can’t automatically merge`。教训是 public repo 的展示面包括 branches/tags/releases，不只是 default branch。
- **不能把“同步远端”理解为 raw push 本地 dev**: local `dev` 包含 ngagent 和内部记录，直接推送会重新暴露内部文件。教训是 public sync 要以 public contract 为准。
- **GitHub Release object 和 Git tag 是不同东西**: 删除 tag 不会自动删除 Release object/asset。task-014 先通过 authenticated REST 删除 Release，再删除 `Release-0122` tag。
- **Windows PATH 需要显式注入**: 已有 agent 进程不会自动继承用户 PATH 持久更新。执行 Claude Code 时需要 prepend `Z:\.npm-global\node_modules\@anthropic-ai\claude-code\bin`。
- **ngagent merge 新增报告文件会留下本地删除状态**: task-012/013/014 merge 后都出现新增 `.orchestrator/stage-p/*.md` 被标记删除的问题；恢复对应文件自 HEAD 后工作区恢复干净。

## 指代关系与术语澄清

- “任务 A” -> Stage A：清洗、剥离、整理；不是 Stage B 重构。
- “那个 release pack” -> 早期指本地 release artifacts，后续具体落到 GitHub Release `Release-0122` / display `Release-0121` / asset `bcsp-20260122.zip`。
- “dev had recent pushes” -> GitHub 远端 `refs/heads/dev` 分支仍存在时的 UI 提示，不是 `main` 本身不干净。
- “完全重建” -> 不是重写所有历史，而是构造 clean public `main` 并删除不应展示的 remote refs。
- “联通的任务 A” -> 本地整理、远端公开展示、release/tags/branches 状态在用户心智上对齐。

## 结果

- ✅ ngagent `task-001` 到 `task-014` 全部 completed/merged。
- ✅ Final public remote heads: exactly `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4 refs/heads/main`。
- ✅ Final public remote tags: `0`。
- ✅ Final GitHub Releases: `0`。
- ✅ Deleted branches: `146` non-main remote branches, including `dev` and `public-main-candidate`。
- ✅ Deleted tags: `Finalrelease`, `First-release`, `Release`, `Release-0118`, `Release-0121`, `Release-1124`, `Second-release`, `Release-0122`。
- ✅ Deleted release: GitHub Release `id=264993969`, `tag_name=Release-0122`, display `Release-0121`, asset `bcsp-20260122.zip`。
- ⚠️ Local `dev` still exists and is internal-only.
- ⚠️ Memory docs are currently untracked local files.

## 结论

Stage A/P 的实质目标已经完成：项目从早期混乱状态中被分离出一个内部工作线和一个干净公开线。公开 GitHub 现在只保留 `main@9c93170c5dc8e3b767312b4877d87ee0d2ce19e4`，没有 `dev`、没有候选分支、没有历史 tags、没有旧 Release。下一阶段不应再围绕 GitHub public surface 打转，而应在用户确认后进入 Stage B refactor、维护或新功能规划。

最重要的后续约束是：不要把 local `dev` 推回 public `origin`。`dev` 是内部协调和记忆线，包含 ngagent、`.orchestrator` 和 memory docs；public `origin/main` 是对外展示面。

## 下一步

- [ ] 用户决定下一阶段方向：Stage B refactor / maintenance / new features。
- [ ] 决定 memory docs 是否只留本地，还是纳入内部 `dev` 的正式提交。
- [ ] 如进入 Stage B，先从 `.orchestrator/stage-a/05-module-surface-map.md` 和 `06-final-baseline.md` 提取 refactor task plan。

## 相关产出

- `CLAUDE.md` — 当前状态仪表盘。
- `docs/TIMELINE.md` — 项目时间线。
- `docs/DIGEST.md` — pm-save 蒸馏层。
- `docs/registry.json` — session/phase registry。
- `.orchestrator/stage-p/05-public-remote-surface-audit.md` — 远端 refs/releases 审计。
- `.orchestrator/stage-p/06-public-branch-closeout.md` — branch closeout 报告。
- `.orchestrator/stage-p/07-public-tags-releases-closeout.md` — tags/releases closeout 报告。

## 相关

- **前序**: `docs/sessions/2026-05-11-pm-init-setup.md`
- **全程记录**: `docs/TIMELINE.md`
