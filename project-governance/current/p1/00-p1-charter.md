# P1 Charter — 全新产品记忆恢复

## 1. 状态与授权

- **状态**：P1 已完成并经用户明确批准关闭
- **授权日期**：2026-07-12
- **P1 Review 批准日期**：2026-07-12
- **权威工作线**：当前 Codex 主线
- **权威工作流**：`project-governance/current/single-mainline-delivery-workflow.md`
- **执行模型**：主线 Codex 负责上下文、整合和验证；可使用受控 subagent，但不形成第二权威线

## 2. 用户最新澄清

1. 禁止阅读旧 P1 产物，是为了防止偷懒复用现成答案以及造成上下文污染。
2. 旧 RBCSP 的所有一手历史都在调查范围内。
3. Compact 是旧 RBCSP 历史中的高优先级证据域，必须独立、系统、完整地调查，不能作为普通文档附带扫过。

## 3. P1 目标

P1 用于恢复因项目中断而丢失的产品记忆，并把两层信息合并成可审计候选材料：

- **历史层**：旧 RBCSP 当时想做什么、已经有什么、如何设计、哪些功能真实实现或测试、哪些能力被删除/搁置/漂移。
- **当前层**：当前权威工作流已经确认的两条交付任务、React/Rust 方向、实时状态与声音规则、UI 阶段和治理边界。

P1 必须恢复完整库存，但不负责裁决最终保留、删除、重做或延期；该裁决属于 P2。

## 4. 允许的一手来源

### 4.1 当前权威来源

- `project-governance/current/single-mainline-delivery-workflow.md`
- 本 charter 与本次 P1 新建的基线、登记和验证文件

### 4.2 旧 RBCSP 项目来源

- 当前项目中除禁读路径之外的产品源码、测试、README、设计文档、脚本、配置样例和历史 archive
- `docs/archive/stage-a-legacy/Compact/` 及其他明确属于旧 RBCSP 的 Compact
- `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner`
- 旧 release、包、运行记录和恢复材料
- Git 分支、tag、commit、tree 和 diff 中属于旧 RBCSP、且位于旧 P1 禁区之前或之外的内容
- `feature/task-015` 及其旧产品审计上下文
- 远端 `VVittgenstein/Rutgers-BetterCourseSchedulePlanner` 的仓库、分支、tag、commit 和公开历史
- 只与 RBCSP 项目相关的原始 `.claude` / `.codex` 会话记录；只在其他一手来源不足时按项目路径、repo 名称、task-015 或明确 RBCSP 关键词定向读取

### 4.3 Compact 优先规则

- Compact 单独登记来源和覆盖范围。
- 不只读文件名或摘要，必须读完整相关内容。
- 需要按时间、任务、产品域和状态整理。
- Compact 中的 Agent 总结不能自动等同于用户决定或代码事实。
- Compact 结论必须尽可能与代码、测试、Git 或用户原话交叉核验。

## 5. 禁止阅读与禁止使用

以下内容不得打开、阅读、全文搜索其内容、引用、总结、复用，也不得借其目录或结论定位答案：

- `docs/deliverable-a-windows-local-release-requirements.md`
- `docs/p1-a-recovery/`
- `.ngagent/` 中与旧 P1 有关的 task、report、record、worktree、日志和派生物
- 旧 P1 执行产生的 commit 内容、合并结果和文件 blob
- NGAT/Organ 根据旧 P1 继续生成的任何二手产物

为避免当前决定被旧流程污染，本次 P1 也不读取以下已废弃流程产物：

- `docs/dual-delivery-workflow.md`
- `docs/public-web-target.md`
- `docs/deployment-platform-decision.md`
- `docs/shared-rust-architecture-decision.md`
- 根目录中的 `chat-log-codex-2026-07-10-1ce70862*.md`

当前决定只从新的权威工作流读取。旧 RBCSP 的一手项目历史、Compact、task-015、Git、旧代码和原始项目会话不属于上述禁区。

## 6. 工作树与写入边界

- 源码、测试、旧文档、Git 历史、远端和服务器均只读。
- P1 只允许在 `project-governance/current/p1/` 下新增本次材料。
- 不修改、恢复、删除或格式化任何用户既存文件。
- 不切换或重置当前分支，不清理工作树。
- 不创建空 marker commit。
- 不暂存、提交或推送 P1 文件，除非用户另行明确要求。
- 不接触 Vultr、域名、Cloudflare 或其他生产环境。

## 7. 证据分层

每项恢复结果必须标明其证据性质：

- `USER_EXPLICIT`：用户明确表达
- `CODE`：实现代码直接证明
- `TEST`：测试明确证明
- `DOC`：旧项目文档陈述
- `COMPACT`：旧 RBCSP Compact 记录
- `GIT`：commit、branch、tag、tree 或 diff 证明
- `RELEASE`：旧包或发布制品证明
- `RAW_SESSION`：项目原始会话记录
- `INFERENCE`：主线根据多项证据作出的推断
- `CONFLICT`：来源互相冲突，尚不能裁决

Agent/Codex 历史总结不能伪装成用户原话；推断不能伪装成事实。

## 8. 有限执行拆分

P1 固定为六个实质工作单元。未经用户同意，不得机械扩张为新的任务流水线。

1. **来源与安全登记**：记录允许来源、禁区、工作树基线和 Git 安全截止线。
2. **Compact 与旧恢复材料**：系统调查 Compact、恢复目录、旧 release 和历史记录。
3. **代码、测试与旧产品文档**：恢复行为级能力、实现状态、测试状态和架构。
4. **Git、task-015、GitHub 与项目原始会话**：恢复历史演进、旧目标和未完成产品线。
5. **库存、当前层与冲突整合**：建立完整能力库存和新旧覆盖关系，不作 P2 裁决。
6. **独立完整性审计与停门**：检查来源覆盖、证据解析、禁区遵守和 P1/P2 边界。

失败的工作单元必须收敛修正原任务，不得不断注册 corrected/remediation/file-based 替代链。

## 9. P1 必需产物

所有产物均位于 `project-governance/current/p1/`：

- `00-p1-charter.md`
- `01-preflight-baseline.md`
- `02-source-register.md`
- `03-legacy-capability-inventory.md`
- `04-current-decision-overlay.md`
- `05-conflict-and-supersession-ledger.md`
- `06-p1-product-memory-candidate.md`
- `07-p1-validation-and-stop-gate.md`

## 10. 完成标准

- 旧 RBCSP 的主要产品域和生命周期表面都有行为级覆盖。
- Compact 被完整作为独立证据域处理。
- 旧功能不能只被概括为“支持筛选”“支持订阅”等宽泛词语。
- 每项重要结论都能回到允许的一手来源。
- 当前决定与旧历史的覆盖、冲突和未知项清楚可见。
- 旧 P1/NGAT/Organ 内容没有进入最终证据链；`SAFE-INC-01` 的禁止工具扫描与标题级上下文已经完整披露、隔离重做，并由用户在 P1 Review 明确接受。
- 没有提前进行 P2 的 `KEEP / REMOVE / REDESIGN / DEFER` 裁决。
- 没有修改产品源码、测试、服务器或远端。
- 验证完成后停止，不自动进入 P2。

## 11. 硬停点

P1 已在联合 P1 Review 停止，并于 2026-07-12 获得用户明确批准。P1 现已关闭；P2 只是下一可执行阶段，尚未自动启动。
