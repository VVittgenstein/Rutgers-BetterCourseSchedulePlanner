# Project Digest

> 此文件由 `/pm-save` 自动维护. `/pm-review` 直接消费此文件进行全局综述.
> 每个 Phase 条目 = `/pm-review` 的 subagent 原本会现算的分析结果, 提前算好存着.
> 配方版本 / Recipe version: v1
> 最后更新 / Last updated: 2026-05-12

---

*This file is intentionally empty at init time. Phase analyses will be appended by `/pm-save` as work progresses.*

*For pre-existing Stage A and Stage P internal reports, see `.orchestrator/stage-a/` and `.orchestrator/stage-p/`.*

---

## Phase 1: Public Remote Closeout Savepoint (2026-05-12)

**背景与动机**: 用户触发 `$pm-save`，需要把一个很长的 Stage A/P 会话保存到三层记忆系统中。该会话的核心不是单个 Git 命令，而是把早期混乱项目重新整理为本地 internal `dev` 与公开 clean `main` 分离的状态，并保留用户关于 Git/GitHub、release、远端选择性披露的关键认知过程。

**过程**: 会话先明确用户的三点历史判断：项目开发流早期且不成熟，后续修改仓促导致 drift，早期技能/工作流低级导致实现和设计可能有问题。随后用户把工作拆成 Stage A（清洗/剥离/整理）和 Stage B（重构），并要求 Stage A 用 ngagent 执行、每个任务配 review gate。Stage A 完成后，问题转向 GitHub public surface：用户看到 `dev had recent pushes` 和 `Can’t automatically merge`，并指出这不是“整个库好的样子”。最终采取 sanitized replay 而不是 raw `dev` -> `main` merge，先把 `origin/main` 切到 reviewed clean candidate，再执行 task-012/013/014 完成远端 refs/releases 收口。`task-012` 审计发现 147 branches、8 tags、1 GitHub Release；用户明确批准后，`task-013` 删除 146 个非 main branches，`task-014` 删除 GitHub Release `264993969`、asset `bcsp-20260122.zip` 和全部 8 个 tags。本次 pm-save 又启动 4 个工作单元，分别提取事实、决策、原文和验证产物，并据此写入本保存点。

**关键决策**: 用户决定 Stage A 与 Stage B 分离，Stage A 只做旧状态清理和组织，不做重构。用户决定使用 ngagent，并要求 review gate 可递归影响计划。用户澄清“提交”指每个 task 形成正常工作提交，而不是把 ngagent 工程文件公开。用户指出本地 `.gitignore` 和远端 GitHub 都不能被当作绝对真相，只能作为证据层。主 agent 将用户“完全重建 / 整个库好的样子”的要求落实为 sanitized public `main` + 删除远端 `dev`/candidate/history refs/tags/releases；用户在 task-012 审计后明确批准 destructive deletion。

**结果**: ngagent `task-001` through `task-014` 全部完成/合并。最终 public remote state 经 git 和 GitHub API 验证为 exactly one head: `refs/heads/main@9c93170c5dc8e3b767312b4877d87ee0d2ce19e4`，0 tags，0 GitHub Releases。旧 `dev`、`public-main-candidate`、8 个 release tags、Release `Release-0122` / display `Release-0121` 和 asset `bcsp-20260122.zip` 均已从 public surface 删除。

**对项目的影响**: 项目现在有清晰边界：local `dev` 是内部协调线，public `origin/main` 是对外展示面。后续工作可以从 Stage B refactor、maintenance 或 new features 中选择，而不必继续处理 GitHub public surface drift。

**不一致检查**: 工作单元 D 验证了远端 heads/tags/releases、ngagent 状态和 Stage P reports，未发现 public closeout 结果与文件记录冲突。需要注意的是 memory docs 当前是本地未跟踪文件；`docs/sessions/2026-05-11-pm-init-setup.md` 中部分近似行数已过时，本次保存使用当前主线程实测行数。

**原始文件**: `docs/sessions/2026-05-12-public-remote-closeout-savepoint.md`
