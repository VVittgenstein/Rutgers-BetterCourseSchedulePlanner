# P2 Charter — 本地一键包 ALL/ONLY 与复用审计

## 1. 状态

- **阶段**：P2
- **状态**：`P2 APPROVED — CLOSED`
- **正式启动授权**：用户于 2026-07-12 明确要求“请正式启动P2”
- **启动时间**：2026-07-12T22:54:58+08:00
- **原审计完成时间**：2026-07-12T23:30:24+08:00
- **Review修订日期**：2026-07-13；用户对`07`§8第1–10项作出回复，相关裁决已回写P2矩阵与contract
- **最终批准**：用户于2026-07-13明确回复“批准P2”
- **当前产品对象**：Windows `本地一键包`
- **硬停点结论**：P2 Review门已通过；P3现在具备启动资格，但P2批准不自动启动P3

第1项要求证明历史筛选覆盖并引入真实Rutgers数据证据门；第2–10项分别修订三值规则、availability、section key、明确排除项、包级持久化、Max audible、持续闹钟、双刷新和I18N。用户随后明确Saved views要做，并最终裁决它只属于本地一键包；旧UI证据已核对，当前能力定为`REQUIRED / LOCAL_ONLY`，公网包不提供入口、API或definition storage。用户已在全部修订同步并通过验证后明确批准P2；本文件作为已关闭审计基线保留。

## 2. 权威输入顺序

1. `project-governance/current/single-mainline-delivery-workflow.md`
2. `project-governance/current/p1/06-p1-product-memory-candidate.md`
3. `project-governance/current/p1/03-legacy-capability-inventory.md`
4. `project-governance/current/p1/04-current-decision-overlay.md`
5. `project-governance/current/p1/05-conflict-and-supersession-ledger.md`
6. 当前工作树中允许读取的产品源码、测试、配置、文档、构建和打包表面
7. 旧 release、Compact、Git/GitHub 只作为发现、漂移和历史行为证据

本地一键包是完整产品基线。公网专属内容只登记为 `PUBLIC_DELTA / CARRY_TO_P4`，不得因为不属于本地包就被误判为从整个产品或仓库删除。

## 3. 禁止读取与隔离边界

本轮不得打开、搜索正文、引用或使用：

- `docs/deliverable-a-windows-local-release-requirements.md`
- `docs/p1-a-recovery/**`
- `.ngagent/**` 中的旧 P1/NGAT/Organ 任务、报告、状态和派生物
- NGAT/Organ 根据旧 P1 继续生成的计划或校验报告
- 废弃 workflow 文档及其派生物

为降低污染，本轮也不使用 chat-log 代替当前权威输入。`.secrets/**`、ignored 私有配置和私有 inventory 只登记路径、ignore 状态和潜在制品可达性；不读取、显示或复制正文。

## 4. P2 执行边界

允许：

- 读取允许语料；
- 静态引用、route、symbol、schema、protocol、storage 和 dependency 分析；
- 路径、文件数、字节数、Git 状态和 SHA-256 复算；
- 只读查看旧 archive 的目录 metadata 和容器 hash；
- 在本目录写入 P2 治理矩阵、审计结论和可复算 inventory 工具。

禁止：

- 修改、删除、格式化或生成产品源码；
- 安装依赖、构建或打包；
- 运行会写 DB、日志、config、checkpoint、dist 或 runtime state 的测试/脚本；
- 设计最终 Rust 模块边界或 P7 task/commit 图；
- 修改远端、Release、真实服务器或私有配置；
- 把旧 Node/Fastify 技术栈差异当作整项目重写理由；
- 把已有代码当作无条件复用理由。

## 5. 工作树保护基线

P2 启动时工作树已经存在用户变化：

- tracked 删除：`.orchestrator/**` 与 `AGENTS.md`；
- tracked 修改：被禁止读取的 `docs/deliverable-a-windows-local-release-requirements.md`；
- 多个既有 untracked 文档、chat-log、废弃 workflow、旧 P1 目录和当前 `project-governance/**`；
- ignored runtime、DB sidecar、local config、release、`node_modules`、`dist`、secret 和 worktree 路径。

这些状态不由 P2 回滚、清理、覆盖或解释为本轮产品变更。当前允许审计的 158 个 tracked 产品拥有文件在启动时均为 clean。

## 6. 文件全集的口径

### 6.1 `OWNED_PRODUCT` — 必须逐文件 N/N 裁决

共 **158** 个 tracked 文件：

| 组 | 数量 |
|---|---:|
| 根产品入口/依赖/控制文件 | 7 |
| `api/**` | 22 |
| tracked `configs/**` | 11 |
| `data/schema.sql` + `data/migrations/**` | 5 |
| tracked `frontend/**` | 53 |
| `notifications/**` | 10 |
| `scripts/**` | 16 |
| `workers/**` | 4 |
| 非禁读、非 archive 的 tracked 产品 docs | 23 |
| `reports/**` | 7 |

`01-file-universe.tsv` 必须逐文件给出 hash、状态、类型、可达性、三轴裁决和后续动作。

### 6.2 `GENERATED_VENDOR_RUNTIME` — 登记而不把 vendor 当作产品源码

- root `node_modules/**`
- `frontend/node_modules/**`（启动时 2,843 files）
- `frontend/dist/**`（3 个 ignored 旧制品）
- `frontend/tsconfig.tsbuildinfo`
- ignored DB sidecar、migration log、runtime fetch job、poller checkpoint
- ignored local/user config
- ignored `release/**` 和根旧 zip

这些路径必须进入 package/runtime residue 审计，但不进入 158 个产品拥有文件的语义 N/N 分母。

### 6.3 `HISTORICAL_EVIDENCE`

- `docs/archive/stage-a-legacy/**`，包括获批 P1 已优先调查的 74 份 Compact
- 三个旧 archive
- `read_only.md`
- 当前获批 P1 治理材料
- Git/公开 main 的允许 metadata 与历史 tree

它们不进入当前 runtime/package，不替代当前源码全集，也不接受旧分类作为 P2 结论。

### 6.4 `PROHIBITED_OR_PRIVATE_METADATA_ONLY`

禁读旧 P1、`.ngagent/**`、`.secrets/**`、private inventory、ignored secret-bearing config 只做边界登记，不进入内容证据链。

## 7. 三轴与混合文件规则

每个单元必须同时获得：

1. 能力：`REQUIRED / EXCLUDED / FUTURE / INTERNAL_ONLY`
2. 复用处置：`REUSE_AS_IS / REUSE_WITH_FIXES / EXTRACT / PORT / REWRITE / SPLIT / MERGE / REMOVE / DEFER / OUT_OF_SCOPE`
3. 交付归属：`BASELINE_SHARED / LOCAL_ONLY / PUBLIC_DELTA / FUTURE / INTERNAL_TOOLING / HISTORICAL_EVIDENCE`

文件级出现 `MIXED_SEE_02` 时，必须在 `02-file-semantic-matrix.md` 下钻到 symbol/route/component/config key/schema object/protocol/storage/稳定 line anchor，直至每段语义只有一个去向。

## 8. 必需产物

1. `01-file-universe.tsv`
2. `01b-adjacent-surface-inventory.md`
3. `02-file-semantic-matrix.md`
4. `03-capability-all-matrix.md`
5. `04-only-closure-matrix.md`
6. `05-reuse-and-port-matrix.md`
7. `06-filter-section-watch-contract.md`
8. `07-p2-validation-and-review-gate.md`
9. `tools/generate-file-universe.ps1`
10. `tools/validate-p2.ps1`

任何矩阵中的保留、移植或重写结论都必须给出目标消费者、缺陷/清理边界和验证方向；任何非目标都必须完成跨 UI/API/worker/config/schema/docs/tests/deps/startup/package 的传递闭包。
