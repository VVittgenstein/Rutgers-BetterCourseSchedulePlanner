# P1 验证、审计与停止门

## 1. 状态

- **阶段状态**：P1 Review 已通过；P1已批准并关闭
- **验证结论**：`PASS — P1_APPROVED_AND_CLOSED`
- **已披露并获接受的例外**：`SAFE-INC-01`禁区工具误扫/标题行上下文，以及`SAFE-INC-02` ignored私有配置的secret-shape分类
- **硬停点**：已满足；P2成为下一可执行阶段，但尚未自动启动
- **产品源码/远端/服务器状态**：未修改；仅当前权威工作流与P1文档按用户Review更新

用户于2026-07-12明确接受两项事件处置并批准P1。事件仍永久保留在审计记录中，不被改写为“从未发生”；但它们不再阻止P1关闭。

## 2. 必需产物核验

| 产物 | 存在 | 作用 | 状态 |
|---|---:|---|---|
| `00-p1-charter.md` | 是 | 授权、来源、禁区、工作单元、完成标准 | 完成 |
| `01-preflight-baseline.md` | 是 | branch/HEAD/worktree/source-root 启动基线 | 完成 |
| `02-source-register.md` | 是 | 来源、证据层级、禁区、Compact/release/Git/raw与Review登记 | 已批准 |
| `03-legacy-capability-inventory.md` | 是 | 行为级旧能力、实现/测试/stub/orphan/drift/unknown与Review解决项 | 已批准 |
| `04-current-decision-overlay.md` | 是 | 当前明确决定与旧能力的覆盖关系 | 已批准 |
| `05-conflict-and-supersession-ledger.md` | 是 | 53项冲突、覆盖、缺口、Review解决和后续阶段归属 | 已批准 |
| `06-p1-product-memory-candidate.md` | 是 | 获批联合产品记忆；文件名保留以维持稳定引用 | 已批准 |
| `07-p1-validation-and-stop-gate.md` | 是 | 本验证、审计、Review批准与P1关闭记录 | 已批准 |

全部 8 个文件均位于：

`project-governance/current/p1/`

UTF-8 读取检查未发现 U+FFFD replacement character。

## 3. 工作树与写入边界核验

### 3.1 权威工作流的调查锁定与Review更新

P1调查、候选形成和独立审计期间，`project-governance/current/single-mainline-delivery-workflow.md`保持锁定，SHA-256为：

`EF199DAC55CD9D919E2CC3DABEFD80709B626A0B24DF8CA0A3AB369465E52B56`

用户随后在P1 Review给出新的当前产品决定。为保持单一权威源，这些原话被有意汇总进同一工作流；更新后的获批workflow SHA-256为：

`B26451BB6A05C6E4330B9F710F38B4F3735E56FD7A444715385B4026A0F6D5A0`

这不是P1调查期间的未经授权漂移，而是P1 Review批准后的治理更新。

### 3.2 既有 dirty worktree 受到保护

用与 preflight 相同的默认 porcelain 粒度复核：

| 项目 | P1启动时 | P1 Review关闭时 | 解释 |
|---|---:|---:|---|
| porcelain entries | 45 | 45 | 顶层状态未扩张；P1文件和权威workflow都位于既有未跟踪`project-governance/`目录内 |
| tracked deletions | 24 | 24 | 未恢复、删除或修改 |
| tracked modifications | 1 | 1 | 未覆盖；该路径属于启动前现场 |
| untracked top-level paths | 20 | 20 | 未清理或改写 |

复核时 staged entries 为0。

`git diff --name-only`仍只显示P1启动前已存在的`.orchestrator/**`、`AGENTS.md`删除及旧禁读requirements文档的既有修改；没有产品路径新增tracked diff。权威workflow在既有untracked顶层目录内按Review明确更新，因此不会出现在tracked diff中，但其前后hash已在3.1节完整登记。命令产生的line-ending warning不代表本轮修改该禁读文件，本轮也未读取其正文。

证据上限：preflight没有保存每个既有dirty/untracked文件的完整path+hash manifest，因此45/24/1/20和diff类别只能证明默认porcelain粒度未扩张、没有新增tracked产品diff，不能严格证明每个既有脏文件逐字节不变。

### 3.3 本轮写入范围

本轮只用 `apply_patch` 新建/修订`project-governance/current/p1/00..07`，并在P1 Review后按用户明确回答更新唯一权威`project-governance/current/single-mainline-delivery-workflow.md`。没有：

- 修改 `api/**`、`frontend/**`、`scripts/**`、`workers/**`、`notifications/**`、`configs/**` 或 `data/**`；
- 安装依赖、运行formatter或构建产物；
- 运行可能写 DB/log/config/checkpoint/dist 的产品测试；
- 切换/reset branch；
- stage、commit、push；
- 修改 GitHub issue/PR/release/branch；
- 接触Vultr、域名、Cloudflare或生产服务器；
- 修改或显示本机被ignore的 `configs/mail_sender.user.json`；该文件内容不作为P1证据。

## 4. 禁区与污染隔离核验

### 4.1 禁区正文

除SAFE-INC-01明确披露的错误`rg`机械扫描和标题匹配行外，主线与被采纳的调查结果没有打开、引用、概括或使用：

- `docs/deliverable-a-windows-local-release-requirements.md`；
- `docs/p1-a-recovery/`；
- 旧 P1 `.ngagent`/NGAT/Organ task/report/state/worktree/log/派生物；
- 旧 P1 commit blob/diff/merge结果；
- 四份废弃流程文档；
- 根目录 July chat log。

Git 对旧 P1 `556afb3^..dev` 的检查限于 commit/path metadata。它还证明旧 P1 38 commits 没有改动本轮读取的产品源码白名单，因此 clean-room 对当前产品源码的读取没有间接复用旧 P1 产品修改。

### 4.2 SAFE-INC-01

事实：第一位源码盘点subagent的错误排除glob使`rg`对禁区执行了本来禁止的内容扫描，并向该隔离subagent context返回若干文件路径/标题匹配行；没有报告展示其他正文行。不能把这描述成绝对“禁区未扫描”。

处置：

1. 立即中止 agent；
2. 丢弃其全部结果，不从中选取任何结论；
3. 用新 agent 重新执行；
4. 新执行只使用逐项白名单，禁止 `docs/` 根枚举/glob/递归全文搜索；
5. clean-room执行完成并确认没有读取Compact、Git blob、旧P1或废弃流程；
6. 在 `02-source-register.md`、`05-conflict-and-supersession-ledger.md`、本文件和最终交付中持续披露。

结论：不能写“零扫描/零触碰”；可以准确写“发生过禁止的工具内容扫描，返回的标题级上下文与该agent全部结果均被隔离，未进入主线结论，源码调查已独立重做”。用户已在P1 Review明确接受该处置。

### 4.3 SAFE-INC-02

事实：clean-room任务最初把配置白名单写成过宽的`configs/**`。该agent随后报告对ignored本地`configs/mail_sender.user.json`做了“非占位secret-shaped”分类；具体值没有显示、复制或返回主线。但能作出shape分类说明内容被机器处理过，不能同时宣称“完全未读”。

处置：

1. 主线没有打开该文件，只用Git metadata确认路径存在且`ignored/untracked`；
2. 该shape分类从产品/secret证据链排除，不据此声称存在有效key；
3. 来源白名单收窄为tracked examples、schemas和templates；
4. ignored local/user config正文统一视为未知；
5. 不显示、修改、删除或打包该文件；
6. P2清理审计和P7 secret/artifact audit必须覆盖这类路径。

结论：P1只保留“ignored本地配置路径存在”的metadata事实，不保留或推断其内容。用户已在P1 Review明确接受该处置。

## 5. 来源覆盖验证

| 来源域 | 实际覆盖 | 验证结果 |
|---|---|---|
| 当前权威workflow | 调查阶段705行全文读取/hash锁定；Review决定并入后724行并复算新hash | 通过 |
| 当前用户指示 | P1授权、事件接受、P1批准与八项Review回答均直接记录 | 通过 |
| Compact | 执行工作单元报告74/74逐行读取；独立QA复算13组count=74及corpus digest | 语义读取由执行记录支持；count/hash独立通过 |
| 恢复目录 | 执行工作单元报告20/20读取/解析；独立QA核对文件count/path范围 | 通过，二手综合属性已降级 |
| 旧release | 3/3；entry/metadata/关键源码；SHA-256复算一致 | 通过 |
| 当前产品clean-room枚举 | clean-room执行记录：131个非文档路径，其中80个TS/TSX；SAFE-INC-02私有配置正文从有效来源排除 | 通过（带披露事件） |
| 测试代码 | clean-room计数11文件、45个test定义；未执行 | 通过“存在性”，不声称运行通过 |
| 白名单旧产品文档 | clean-room报告24份逐项读取；未对docs根扫描 | 通过 |
| Git/task-015 | 安全截止线、refs、旧产品tree和允许文档；task-015未合并地位明确 | 通过 |
| GitHub公开面 | connector repo/README/package/commits/PR + local tracking tree | 通过；live `ls-remote`超时已披露 |
| Raw sessions | 只定向RBCSP sessions，提取用户明确原话 | 通过 |
| 当前外部Rutgers/生产状态 | P1不实时probe/部署 | 非P1完成条件；后续重核 |

Compact manifest算法：按filename升序，每行`filename|FILE_SHA256_UPPER_HEX`，UTF-8编码，记录间LF连接、末尾无LF，再取SHA-256大写hex。结果：

`C60BAD39567DEDC9E0FD70DA408450034496F98E2EE2E335AF5DFE7D2C18BE62`

Release hashes复核：

| 制品 | SHA-256 |
|---|---|
| `bcsp-20260121.zip` | `F62F14D2CEE0DE4BD90931E37808141FB45DF970E39CFF7BAAB78E9A999A9A50` |
| `bcsp-20260121.tar.gz` | `827D2EF1F59357780AC70A92F489A92246D5EC4BC1EDE2BEF2EC1CBFA63951AD` |
| `bcsp-20260122.zip` | `48E976EF9B2EFCBFB692F6CC119C790BF7D4D3E9A361C464E38F61A89A5AFAD1` |

## 6. 关键结论交叉核验样本

| 结论 | 至少两层证据 | 核验 |
|---|---|---|
| `/api/sections`是假完整表面且独立section能力必要 | CODE handler固定空 + DOC描述真实endpoint + 用户确认独立搜索/访问/详情 | 通过；API形状留P2/P3 |
| Fresh ingest不维护FTS | schema/query依赖FTS + ingest无写入 + TEST fixture手工insert + DOC反向声称 | 通过 |
| 周四`H`可能丢失 | REPORT真实样本H + CODE normalizer/UI仅TH | 通过；当前SOC仍需后续probe |
| meeting时间实现冲突、目标语义已解决 | backend EXISTS任一 + frontend every全部 + UI DOC全部 + 用户确认整个section全部完整包含 | 通过；未知时间边界留P2/P3 |
| 首次launcher跳过fetch | migration先创建DB + 后检查exists + DOC声称会full fetch | 通过 |
| 旧persistent subscription与当前live watch不同 | CODE/schema持久模型 + 当前workflow明确内存/WebSocket | 通过 |
| 旧提醒不是当前语义 | CODE 15秒poll/3分钟reminder/7秒claim + 当前每条Open/WebSocket/无防抖 | 通过 |
| 旧一键包不自包含 | CODE要求Node/npm/Vite dev/native rebuild + release metadata/Mac用户报告 | 通过 |
| public/internal/release分裂 | Git tree/refs + GitHub connector + archive对照 | 通过 |
| 邮件当前不在版本 | 旧CODE/TEST完整存在 + 当前workflow明确排除 | 通过覆盖关系；未执行清理 |
| macOS历史与当前边界 | RAW证明旧时仍要求Win/Mac且无Mac设备 + 旧包失败 + Review新决定Windows-only | 通过；未找到旧取消记录已明确披露 |
| Calendar与subscription管理 | 旧orphan/文档/代码 + Review确认Calendar历史来源/future边界、管理/toast/max需求 | 通过；精确语义留后续阶段 |

## 7. P1/P2 边界审计

已执行：

- 恢复并分层登记旧能力；
- 说明实现、test code、Compact claim、release存在、删除、stub、orphan、drift、unknown；
- 把当前明确决定overlay到旧历史；
- 对当前明确覆盖项使用 `CURRENT_RESOLVED/SUPERSEDES`；
- 将Review已解决项写回workflow/库存/overlay/台账，将剩余细节交给P2/P3/P4/P7。

未执行：

- 没有给旧能力逐项作当前 `KEEP / REMOVE / REDESIGN / DEFER`；
- 没有把task-015旧分类继承为当前裁决；
- 没有实施产品代码或依赖清理；
- 没有设计Rust模块边界、实现task图或部署方案细节；
- 没有把当前排除邮件误写成“旧邮件从未存在”；
- 在用户批准前没有把P1 Review问题预先视为已批准；批准后只记录用户明确回答，没有扩张为P2裁决。

因此阶段边界通过。

## 8. 已知限制与需要用户知情的事项

1. `SAFE-INC-01`和`SAFE-INC-02`已由用户在P1 Review接受；主线仍永久披露，不能改写为未发生。
2. `D:\Document\Obsidian\Adrian\Prompt\BetterCourseSchedulePlanner`当前不存在，不能直接读取；早期意图由其他一手来源交叉恢复。
3. 当前Rutgers payload/限流/term/campus/day code不是P1实时probe结果，P4/P7必须重新核验。
4. GitHub live `ls-remote`一次网络超时；最终release前仍须复核remote refs/tags/releases。
5. 旧release未运行；结论来自archive内容/metadata/源码，不宣称包在当前机器可运行。
6. 测试未执行；P1只恢复测试面和缺口，不宣称当前通过。
7. 本机ignored mail config路径存在，当前Git metadata显示未tracked；正文与secret状态未知，后续secret/artifact audit仍需覆盖。
8. Discord删除原因已由用户解决为“主动要求删除以降低复杂度”；Git仍不能还原所有旧release未提交工作树改动的精确来源，后者保持unknown。

## 9. 独立审计

两项独立审计在主线候选形成后执行：

- **产品完整性审计**：检查旧能力遗漏、错误概括、当前决定遗漏、文件间矛盾和P2越界。
- **安全/证据审计**：检查禁区、SAFE-INC-01、hash/count、worktree、来源夸大和硬停点。

审计者没有修改文件，也没有读取禁区正文。安全/证据审计与产品完整性审计均为`PASS`；初审发现已全部并入。用户随后接受SAFE-INC-01/02并批准P1，因此此前的条件状态已解除，阶段总状态为`PASS — P1_APPROVED_AND_CLOSED`。Review后新增澄清另经边界映射、macOS定向历史核查与最终结构复核。

Review后最终复核结果：严格meeting语义`PASS`；macOS历史/当前决策分层在两处措辞精确化后`PASS`；全部八项回答与唯一workflow映射`PASS`；LEG 109/109、CUR 68/68、CON 53/53均完整追踪，无剩余阻断项。

## 10. 停止门

### 10.1 P1 完成条件

- 主要产品域和生命周期均有行为级覆盖：满足。
- Compact作为独立证据域完整处理：满足，74/74。
- 重要结论可回到允许来源：满足。
- 当前决定与旧历史覆盖/冲突清晰：满足。
- 旧P1禁区遵守：没有达到绝对“零扫描”——发生SAFE-INC-01；标题级上下文和整个污染agent结果已隔离、主线结论未使用，用户已接受处置。
- 未提前执行P2裁决：满足。
- 未修改产品、远端或服务器：满足。
- 独立完整性/安全审计：均已完成并通过；初审发现均已修订。SAFE-INC-01/02已准确披露并由用户接受。

### 10.2 当前停点

P1 Review已于2026-07-12完成：用户接受两起事件、批准P1并提交八项产品记忆澄清。当前状态是：

`P1 APPROVED AND CLOSED — P2 NOT STARTED`

- `06-p1-product-memory-candidate.md`虽然保留稳定文件名，内容已是正式获批P2输入；
- 本轮不自动开始P2，不修改产品代码，不触碰生产环境；
- 下一步若开始P2，必须单独进入本地一键包的all-and-only审计，并使用权威workflow与获批P1全套输入。
