# RC Iteration 权威模型与 Round 4 接续记录

### 文档控制

| 字段 | 值 |
|---|---|
| Record ID | `RC-ITERATION-AUTHORITATIVE-MODEL-ROUND4-HANDOFF-2026-07-17-001` |
| 状态 | `ROUND_3_COMPLETE_ROUND_4_NOT_STARTED` |
| 当前基线分支 | `codex/p7-implementation` |
| 当前基线提交 | `dfabbfdcbea0cb90021ed59d11ccb38c29b19fa7` |
| 当前 HumanTest 输入 | RC3 最终 Windows ZIP |
| Round 4 实现授权 | `FALSE` |
| Release 授权 | `FALSE` |
| 生产部署授权 | `FALSE` |
| 记录日期 | `2026-07-17` |

本文与以下文档共同构成 Round 4 前的本地权威上下文：

- `01-project-history-context-and-timeline-through-rc3.md`：完整历史、上下文和时间线；
- `00-rc-iteration-round-03-product-and-technical-design-decision-record.md`：RC3 产品与技术设计原始记录；
- 本文：RC Iteration 的持续规则、当前产品不变量、废弃设计和 Round 4 交接。

若三者出现冲突：

1. 更新的用户明确裁决优先；
2. 更新的真实 HumanTest 优先于旧 PASS；
3. 当前已实现的 RC3 合同优先于 Round 1/2 历史合同；
4. 本文不主动创造尚未由用户决定的新产品要求。

---

## 1. RC I 的精确定义

“RC I”是 **RC Iteration（候选迭代）**。`I` 表示 `Iteration`，不是罗马数字一；`Round 1`、`Round 2`、`Round 3` 才表示轮次。

RC Iteration 位于：

```text
P7 实现与 P7.5 候选形成
→ RC Iteration Round N
→ 正式 GitHub Release（若另行授权）
→ 生产部署（若另行授权）
```

原始定义来源：

- `chat-log-codex-2026-07-16-05afa573.md:1338-1374`

### 1.1 为什么不再属于 P7.5

P7.5 的原性质是：

- 候选包已经冻结；
- 不再修改代码；
- 在 Windows、Linux Actions 与 Vultr staging 中消费相同候选；
- 只产生最终真实 E2E 证据。

当真实使用发现问题，后续需要修改代码和重新构包时，候选已不再不可变，因此不能继续把它叫 P7.5。

### 1.2 RC Iteration 的目的

1. 让真实用户使用成为 Release 前最高级别的运行证据；
2. 消化自动化、fixture、小数据和旧验收遗漏的产品问题；
3. 以短反馈循环修复问题，不反复重跑完整 P7.4/P7.5；
4. 维持一轮一个最终源码身份和同源双包；
5. 防止治理、诊断或测试反向伤害正常产品功能；
6. 在 Release 前逐轮提高实际可用性，而不是保卫旧 PASS 声明。

### 1.3 RC Iteration 不是什么

- 不是 P7.6、P8 或新的大阶段矩阵；
- 不是每轮完整发布验收；
- 不是每个问题建立 ledger/gate 的治理流程；
- 不是 GitHub Release；
- 不是生产部署；
- 不自动授权 Vultr、DNS、证书、Cloudflare 或切流；
- 不要求重写全部产品；
- 不允许把旧候选或中间 amend SHA 冒充当前候选。

---

## 2. 每轮标准生命周期

每个 RC Round 默认执行：

```text
上一轮最终 Windows ZIP
→ 用户全新解压并真实使用
→ 用户报告观察、问题、截图和预期
→ Codex 只读调查与复述
→ 用户/Codex 对齐行为和范围
→ 写本轮实现计划
→ 用户明确授权实施
→ 定向修改
→ 必要自动化、真实 Windows QA 与回归
→ 收敛为本轮一个最终提交
→ Windows 本地构包
→ 推送最终 SHA
→ 同 SHA Linux-only 构包
→ 联合制品验证
→ 将最终 Windows 包交回 HumanTest
→ 新问题进入下一轮
```

### 2.1 HumanTest 输入规则

- 必须使用上一轮最终 Windows ZIP；
- 解压到全新目录；
- 不复用旧 EXE、旧 `data`、旧 SQLite、旧 runtime 目录；
- 不设置 `BCSP_CI_NO_RUTGERS` 或其他隐藏 `BCSP_*` 环境变量；
- 无参数直接运行 `RBCSP.exe`；
- 使用真实 Chrome；
- 使用真实 Rutgers；
- package verifier、API smoke、开发服务器或 fake upstream 不能冒充 HumanTest。

### 2.2 对齐规则

实施前必须区分：

- `OBSERVED`：用户、HumanTest 文件或运行记录直接证明；
- `INFERRED`：与证据吻合但并非完整现场复原；
- `ACCEPTED`：用户与 Codex 已对齐；
- `SUPERSEDED`：被更新决定明确替代；
- `OUT_OF_ROUND`：真实但不进入本轮；
- `NOT_STARTED`：尚未实施。

用户尚未批准实施时：

- 可以只读查看源码、HumanTest、SQLite、日志和制品；
- 可以解释根因、提出设计和写计划；
- 不得修改产品代码、构包、提交或启动下一轮。

### 2.3 范围规则

- 本轮只处理已冻结问题；
- 不把一个反馈无限扩张为全系统重做；
- 定向验证直接发现且会破坏本轮目标的缺陷，可在同轮做最小修复；
- 非本轮必要的新问题进入下一轮；
- substantial 产品/架构改变可像 RC3 一样落一份决策记录；
- 普通问题不强制创建新的治理层。

---

## 3. HumanTest 与证据权威

### 3.1 证据顺序

1. 最新用户决定；
2. 最新 HumanTest；
3. 当前已实现的决策记录；
4. 当前 Git、源码、测试、provenance 和制品；
5. 历史阶段合同；
6. 旧 PASS、旧包和中间候选。

### 3.2 旧 PASS 可以被推翻

已发生的实例：

- P7.5 报告 PASS 后，用户发现精确包无法按真实用户路径工作；
- 再次 P7.5 PASS 后，HumanTest 发现服务状态、布局、学分、Section 和 Core 问题；
- RC1 完成后，HumanTest 发现 12 项 UI、查询和 Watch 问题；
- RC2 自动化与一次 QA 达到双 135，但用户 HumanTest 证明一个慢 target 可停止全部拉取。

因此 Round 4 不得用“RC3 已测试通过”预先否定新的用户观察。

### 3.3 自动化的角色

自动化用于：

- 确定性合同；
- 错误边界；
- 防回归；
- 构包完整性；
- 多视口、键盘和无障碍；
- fake clock、fake upstream 的难复现场景。

自动化不能代替：

- 用户理解状态文案；
- 实际布局是否浪费空间；
- 真实 Windows 解压/启动；
- 真实 Rutgers 网络链；
- 真实 Chrome 使用；
- 人耳是否真正听到声音。

---

## 4. UI/UX 强制两阶段流程

只要本轮涉及画面或交互，就必须按顺序使用三项 Skills。

### 4.1 第一阶段：结构与功能

使用：

- `industrial-brutalist-ui`；
- `design-taste-frontend`。

职责：

- 信息架构；
- 结构与层级；
- 空间效率；
- 响应式网格；
- 控件行为；
- 状态与错误；
- 键盘和无障碍；
- Local/Public 共享与变体表面。

### 4.2 第二阶段：交互打磨

使用：

- `emil-design-eng`。

要求：

- 针对真实实现形成 `Before | After | Why` 复核；
- 检查按钮、加载、错误、焦点、滚动、折叠、触控和键盘；
- 复核 reduced-motion；
- 立即落实必要修正。

### 4.3 两阶段共同边界

- 两阶段是同一 RC Round 的内部步骤；
- 不分别提交；
- 不分别构包；
- 不形成两个独立候选；
- 第二阶段不扩大本轮功能范围；
- 用户明确需求高于 Skill 的一般美学偏好。

### 4.4 当前视觉语言默认继承

除非用户在新 Round 明确改变：

- 延续 Swiss Industrial Print；
- 硬网格；
- 纸色/黑/白/单一红色强调；
- 高信息密度与空间效率；
- 不默认引入渐变、重阴影、圆角卡片、装饰图形；
- 不引入新 UI/动画/图标依赖；
- 不使用 `transition: all`、scale-from-zero 或常驻动画；
- 按钮反馈约 100–160ms；
- hover 仅用于精细指针；
- 动画只在有功能意义时使用，并尊重 reduced-motion。

---

## 5. Git、工作区和用户资产保护

### 5.1 当前工作区事实

主工作区不是 clean，存在历史删除、修改和未跟踪文件。这些内容属于用户，不能因为 RC Round 被清理或覆盖。

### 5.2 禁止动作

- 不 `git reset`；
- 不 `git stash`；
- 不 `git clean`；
- 不 checkout 覆盖用户文件；
- 不 `git add .`；
- 不 `git add -A`；
- 不整目录暂存；
- 不删除 `HumanTest`；
- 不删除 chatlog、历史文档和用户未跟踪文件；
- 不将上述内容带入构建 worktree。

### 5.3 本轮基线和 allowlist

实施前记录：

- 当前 branch；
- 基线 SHA；
- dirty baseline；
- 本轮允许修改的源码、测试、文档和打包文件；
- 明确禁止进入提交的路径。

提交前：

- 逐文件暂存；
- 审核 staged diff；
- 证明 HumanTest/chatlog/用户文件为 0；
- 证明没有意外删除和无关改动。

### 5.4 一轮一个最终提交

- 一个 Round 最终只保留一个相对上一轮的提交；
- Round 内可以 amend；
- QA 发现本轮 blocker 时继续 amend 同一提交；
- 中间 SHA 全部视为退役；
- 中间包进入 rejected/evidence，不能成为 HumanTest 输入；
- “一轮一个最终提交”不是禁止多次运行 `git commit --amend`，而是最终祖先链只出现一个该轮提交。

---

## 6. 构包、验证与外部动作边界

### 6.1 Windows 优先

Windows 必须：

- 从短路径、clean detached worktree 构建；
- worktree 设置 `core.autocrlf=false`；
- 使用最终候选 SHA；
- 先通过 package verifier；
- 再执行真实 Windows、真实 Rutgers、真实 Chrome 的本轮目标 QA；
- 只有 Windows 达标后才 push 最终 SHA。

### 6.2 Linux-only 构建

- Linux 只从同一最终 SHA 构建；
- 一轮只做一次必要的 Linux-only workflow；
- 不在线重新构建 Windows；
- 不访问 Rutgers；
- 不部署；
- 不发布；
- 失败时先诊断，不盲目重复 dispatch；
- 若必须超出已约定的外部运行预算，明确说明原因和新边界。

### 6.3 双包联合验证

必须证明：

- 同一 source commit；
- 同一 source date epoch；
- 相同版本；
- provenance 正确；
- manifest 和 release-input allowlist 正确；
- 共享 SBOM 组件一致；
- 嵌入式前端共享组件一致；
- 共享前端能力一致；
- Local/Public 特有能力符合变体边界。

### 6.4 Round 产物

至少保存：

- Windows ZIP；
- Windows SHA-256；
- Linux TAR.GZ；
- Linux SHA-256；
- Windows 文件清单；
- 本轮目标测试与 `Before | After | Why` 摘要。

推荐目录：

```text
.cache/rc-iteration/round-0N/<final-sha>/
```

### 6.5 Release 与部署

RC Round 实施授权不包含：

- GitHub Release；
- tag；
- Release asset 上传；
- Vultr 安装；
- systemd/Caddy 生产变更；
- DNS、Cloudflare、ACME 或证书；
- 生产流量切换。

将来若进入 Release：

- 必须上传已经 HumanTest 接受的相同字节；
- 不得在 Release 阶段重新构建“同源码但不同字节”的包；
- 两个 hash、版本、repo、tag 和资产需另行确认。

---

## 7. 当前双产品边界

### 7.1 严格两个产品

1. Windows Local 一键包；
2. Linux Public 服务包。

公网部署是 Linux 包的后续操作，不是第三个产品。

### 7.2 共享产品能力

- Rust domain、Catalog、Open、query、watch；
- React UI；
- 课程工作区；
- Section 筛选、结果和独立详情；
- Query Contract V2；
- 状态、freshness；
- WebSocket、watch、toast、audio；
- 双语；
- 响应式和无障碍。

### 7.3 Local-only

- 包相对 SQLite；
- Saved Views；
- History；
- Settings；
- Reset；
- Local 刷新频率配置；
- 五学期窗口；
- 另外三个学期手动 Pull。

### 7.4 Public 零表面

Public 不得在以下任何层出现 Local-only 能力：

- source；
- DOM；
- route；
- API；
- storage；
- i18n；
- bundle；
- package。

Public 尤其不得包含：

- Saved Views；
- History；
- Reset；
- Local 设置；
- 手动 Pull；
- 隐藏的 Local endpoint 或配置。

### 7.5 v1 排除项

- email；
- Discord；
- Calendar；
- macOS；
- Share links；
- Waitlist；
- Compact view。

---

## 8. 当前产品不变量：学期、Campus 与 Scope

### 8.1 当前学期

- 使用 `America/New_York`；
- 使用 Rutgers 实际教学学期；
- 顺序为 Winter → Spring → Summer → Fall；
- 学期间空档归入下一即将开始的学期；
- 官方日期尚未发布时 fail closed，不自行推算冒充官方日历。

### 8.2 Public 范围

- 只显示 current + next；
- 两者自动首次拉取；
- 无手动 Pull；
- 无任意年份或历史范围。

### 8.3 Local 范围

- 显示前二、当前、后二，共五个学期；
- current + next 自动首次拉取；
- 另外三个窗口内学期可手动 Pull；
- 手动 Pull 成功不自动 Apply；
- 不允许无限历史学期；
- 窗口外旧数据可留库，但不进入普通选择器或自动刷新。

### 8.4 Campus

- Campus membership 来自 discovery；
- 当前观察到 12 个真实 Campus，但不硬编码永久 allowlist；
- 排除 `ONLINE_NB`、`ONLINE_NK`、`ONLINE_CM`；
- ONLINE 课程继续归属 NB/NK/CM，通过“授课方式：线上”筛选；
- current/next 当前观察为 24 个自动 target，但 24 不是代码常量。

### 8.5 Candidate 与 Applied Scope

- fresh session 默认 candidate term 为 current；
- 不自动选择 NB 或任何 Campus；
- candidate scope 与 applied scope 分开；
- 改 candidate 不清空当前结果；
- 显式点击 Apply 才改变查询 scope；
- Apply 不自动搜索；
- Apply 不直接请求 SOC；
- scope 真正改变后，清除不属于新 scope 的旧结果；
- target-bound 字典值必须重新验证；
- Saved View 失效时拒绝 Apply，不能静默放宽查询。

### 8.6 单一按钮合同

主按钮业务文案只允许：

- `拉取`；
- `应用`。

禁止：

- `继续拉取`；
- `正在拉取`；
- `已应用`；
- 用按钮文案承载后台进度。

queued、fetch、process、publish、retry 和失败原因必须在按钮外显示。

---

## 9. 当前产品不变量：数据、READY 与查询门禁

### 9.1 数据主链

保持简单：

```text
获取原始 JSON
→ 解析
→ 规范化
→ SQLite 原子发布
```

不得重新长出全局审批、庞大状态机或不成比例的防御层。

### 9.2 READY 粒度

READY 的唯一粒度是：

```text
(term, real campus)
```

完整定义：

```text
Catalog 有效取得并处理
AND 对应 Open 有效取得、关联验证并处理
AND Catalog + Open 作为完整快照原子提交
= READY
```

- Catalog-only 不 READY；
- Open-only 不 READY；
- 合法空集合可以是完整快照的一部分；
- 不能把所有空响应无条件当成功；
- 不能把合法空数组无条件当 Schema 错误。

### 9.3 查询门禁

- 先解析请求中的精确 term/campuses；
- 只检查请求中实际选择的 targets；
- 所选 targets 全 READY 后查询；
- 未选择的失败 Campus 不阻塞；
- READY + refresh retry 的 LKG 继续允许查询；
- 未 READY target 返回可重试 `503 SEARCH_DATA_NOT_READY` 和精确 target 信息；
- Term/Campus 控件始终可操作；
- 不恢复 RC2 全局 fieldset 门禁。

### 9.4 原子快照

首次发布：

```text
Catalog candidate
→ normalize/stage
→ 对应 Open
→ 关联验证
→ Catalog + Open 一次提交
→ READY
```

Catalog 刷新：

```text
Cn + Om（当前完整快照）
→ stage Cn+1
→ 获取并验证 On+1
→ 全部成功后替换为 Cn+1 + On+1
```

失败时继续提供 `Cn + Om`。

高频 Open：

```text
Om+1
→ 对当前 serving Cn 验证
→ 成功后原子更新为 Cn + Om+1
```

禁止向用户暴露新 Catalog + 旧 Open 的混合版本。

---

## 10. 当前产品不变量：调度、错误与重试

### 10.1 有界并发

- 最多 3 个 target 工作流；
- 不建立自适应并发控制器；
- 不预留永久 worker；
- 所有优先级共享同一工作池；
- 网络可并行，SQLite 最终发布使用短事务；
- 不积攒整个学期全部 JSON 后一次发布；
- 一个 target 完成即可独立 READY。

### 10.2 工作流

仅保留：

- `CompleteSnapshot`；
- `OpenRefresh`。

同一 target：

- 不允许重复同类工作；
- Catalog candidate fetch/process 时，旧 serving Catalog 的 active-watch Open 可并行；
- candidate 进入配套 Open/commit 时合并其他 Open 请求；
- 不允许发布混合版本。

### 10.3 Target 级错误

以下只影响当前 target：

- connection/timeout；
- 408；
- 普通 5xx；
- gzip/body 读取或解码中断；
- 截断 JSON；
- target 级 schema/normalize 失败。

要求：

- timeout 分类优先于 decode 包装；
- 内容错误 retry 必须重新 GET；
- 不重复 CPU 处理同一失败 payload；
- 自动 retry 有界退避；
- 不能永久放弃仍被自动范围、手动 Pull、applied scope 或 active watch 需要的 target。

### 10.4 Origin 暂停

只有明确来源级限流信号，例如：

```text
429 + Retry-After
```

才能暂停 origin 新请求。

- 暂停到期自动恢复；
- 缺少 Retry-After 时使用有界默认值；
- 不取消已在运行的其他 target；
- 普通 timeout/decode/schema 不得触发 origin 永久熔断；
- Round 3 生产路径不再依赖 `FatalDiagnostic` 或显式诊断授权。

### 10.5 真实重试和诊断

- UI 只有在后台写入 `nextRetryAt` 后才能显示“正在重试”；
- 用户 Retry 必须调用真实后端入口；
- 仅刷新 Service Status 不能被称为 retry；
- 诊断至少保留 target、stage、HTTP status、Content-Type、字节数、错误类别、脱敏 error chain 和 trace ID；
- 诊断用于解释和恢复，不能驱动不成比例的全局状态机。

---

## 11. 当前产品不变量：刷新时钟与 Watch

### 11.1 刷新频率

| 场景 | Public | Windows Local |
|---|---:|---:|
| Catalog normal | 固定 10 分钟 | 默认 10 分钟；1–1440 分钟可配 |
| Open normal | 固定 30 秒 | 默认 30 秒；3–3600 秒可配 |
| Watch Fast Lane | 固定 10 秒 | 默认 10 秒；3–60 秒可配 |

Local watched Open：

```text
min(normal Open, Watch Fast Lane)
```

进入 Fast Lane 不能反而降低用户配置的更快 Open 频率。

`6 小时`只属于 discovery/metadata 低频重新发现，不是 Catalog 课程数据刷新周期。

### 11.2 Fast Lane

- 跟随真实 active watch 的 `(term,campus)`；
- 不跟随当前页面；
- 不要求 watch target 属于当前 applied search scope；
- 只提升有 active watch 的 target；
- 不提升整个 term；
- 只提升 Open，不提升 Catalog；
- 无 active watch 的非应用 target 不进入 Fast Lane。

### 11.3 Watch 范围

- Local/Public 只能 watch current + next；
- Local 其他三个手动 term 即使可搜索也不能 watch；
- Selection、Section 直链、Saved 状态和直接 WebSocket START 不能绕过；
- 时间窗口滚动后，越界 active watch 必须停止；
- 停止时明确说明学期已不在可监看范围；
- 不得静默继续请求。

### 11.4 Watch 与声音语义

- “加入监看列表”不等于“开始监看”；
- 加入后明确显示尚未开始；
- 只有 START 成功后显示 active；
- START 前先等待 AudioContext 解锁尝试；
- 音频解锁失败不取消 watch，但明确显示声音未启用；
- fresh already-OPEN 在 START 后可以生成 episode/cue；
- stale/LKG OPEN 不误响；
- 只有浏览器报告 `STARTED` 才消耗 maxAudible；
- STOP 后不得重复 episode、toast 或声音；
- 一个浏览器最多 9 个 active Section。

---

## 12. 当前产品不变量：Query V2 与结果语义

### 12.1 Query Contract

- 当前合同版本 V2；
- 18 个有效字段；
- V1 只用于兼容迁移；
- 不恢复被删除的 eligibility、building/room、section number、重复 course location。

### 12.2 动态字段

来自已发布 Catalog：

- Core Code；
- Instructor；
- Keyword；
- Level；
- Exam；
- Subcampus。

要求：

- 输入只筛选权威候选；
- 不允许把任意输入创建为有效 token；
- 大小写不敏感；
- Instructor/Keyword 支持子串，例如 `smi/Smi/SMI → Smith`；
- 候选随 target 变化；
- loading、无结果和失效 Saved View 有明确状态。

### 12.3 组合语义

- 不同字段之间 AND；
- 单字段多选按冻结合同 OR；
- 所有 Section 条件必须由同一个 Section 满足；
- meeting availability 必须由同一 Section 的所有 required meetings 满足；
- Subcampus 支持 ANY_MEETING 和 ALL_REQUIRED_MEETINGS；
- requiredness、TBA 或证据不足时使用 `UNCERTAIN`；
- 不得为减少结果数伪造 `MATCH` 或 `NO_MATCH`。

### 12.4 学分和结果后代

- 支持 Rutgers `3_0`、`1_5` 等格式；
- 可变学分按完整包含语义；
- `BA`、缺失或不可解析值保留为 `UNCERTAIN`；
- 确定 `NO_MATCH` 不得进入可见搜索结果；
- Course search 只 materialize witness 后代；
- Course detail 仍返回完整信息；
- 前端防御性忽略意外 `NO_MATCH`。

### 12.5 SearchSession

当前 WebUI 会话内保留：

- filter draft；
- submitted query；
- results；
- page/sort；
- Section disclosure；
- filter rail scrollTop；
- page scroll；
- candidate/applied scope。

关闭或刷新程序后自动恢复旧搜索结果并不是已授权产品能力，不得擅自新增。

---

## 13. 当前产品不变量：UI 状态与导航

### 13.1 信息架构

- 课程是唯一搜索一级入口；
- Section 条件和结果属于课程；
- 独立 Section 详情仍存在；
- `/sections` 不恢复为第二套独立搜索工作区；
- Section 详情保持“课程”一级导航高亮。

### 13.2 导航

- 大型品牌页头可以滚走；
- 一级导航 sticky；
- 正文从导航下方开始；
- 焦点不能被导航遮挡；
- 桌面和移动端多行导航均需测量实际高度。

### 13.3 状态文案

普通用户界面必须说明：

- 总体正在做什么；
- 当前步骤；
- 真实进度；
- 哪些 target 可用；
- 用户能否搜索；
- 是否继续提供 LKG；
- 是否真实安排 retry。

以下只进入诊断：

- WebSocket；
- phase 名；
- trace；
- 内部 target identity；
- HTTP/code/error chain。

不得出现：

- 状态后台永久停止但 UI 声称正在 retry；
- 学科/选项未准备好时无解释空白；
- stale/LKG 被表现成 fresh；
- 一个全局状态掩盖 target 级真实状态。

### 13.4 折叠组

`03–09` 与 `10–18` 双向切换后：

- 新打开组 summary 进入可视区域顶部；
- 焦点可见；
- sticky navigation 不遮挡；
- 桌面调整内部 rail；
- 移动端调整 window；
- 鼠标、Enter、Space 一致；
- 已填值不丢失；
- 单纯关闭不强制滚动。

---

## 14. 已明确废弃、Round 4 不得恢复的设计

1. 9 学期 × 15 Campus 全历史默认拉取；
2. 先全部 Catalog、再全部 Open；
3. Catalog 135/135 + Open 135/135 全局门；
4. 将多个独立 target 组成不可部分成功的全局事务；
5. 在全局完成前禁用 term/campus scope；
6. 一个 target timeout/decode/schema 永久封锁 origin；
7. 等待生产路径不存在的显式诊断授权；
8. UI retry 只刷新状态、不真实重试；
9. ONLINE aliases 作为独立 Campus；
10. Catalog-only/Open-only 标记完整 READY；
11. 公开新 Catalog + 旧 Open 混合快照；
12. 刷新失败时收回旧完整快照可用性；
13. 未选择的失败 Campus 阻塞已选择的 READY target；
14. Local 无限历史 Pull；
15. Public 手动 Pull；
16. 第三种按钮文案承载后台进度；
17. NB 成功替代其他 Campus 成功；
18. 整个 term 全部 Campus 成功后才能使用其中一个 READY target；
19. 将 discovery 的 6 小时误写成 Catalog 刷新；
20. Fast Lane 跟随页面；
21. Fast Lane 使更快的 Local Open 配置反而降速；
22. watch current/next 之外的学期；
23. 独立 Section 一级搜索入口；
24. Instructor/Keyword/Level/Exam/Subcampus 任意自由 token；
25. “+监看”看似已经 active；
26. “WebSocket 空闲”作为主用户文案；
27. 为人工请求预算削弱 30/10 秒能力；
28. 每小任务一次提交；
29. 每项修改触发一次 Actions；
30. 每发现一个问题完整重跑 P7.4/P7.5；
31. 将中间 amend SHA 或旧 Round 包交给下一轮。

正式 Round 3 supersession 来源：

- `00-rc-iteration-round-03-product-and-technical-design-decision-record.md:476-496`

---

## 15. Round 1–3 结果与继承关系

### 15.1 Round 1

```text
baseline: 32c1d72a510d0daf2b88762cccfb10287c9ec103
final:    bb9700c3587baf3bb29db9b549602d8d1661a502
```

持续遗产：

- RC Iteration 流程；
- Service Status；
- 学分解析；
- `NO_MATCH` 裁剪与 `UNCERTAIN`；
- Core 动态字典；
- Sections 默认收起；
- 全宽高效布局；
- SQLite 读写和 personal mutation 解耦；
- 一轮一个最终提交与同源双包。

已替代：

- Catalog-only 搜索门禁；
- 单 operation V1 状态；
- Round 1 状态带位置和部分文案；
- Round 1 包和哈希。

### 15.2 Round 2

```text
baseline: bb9700c3587baf3bb29db9b549602d8d1661a502
final:    fd0f91bfe8e01616f94cd87cc2ffdcb737812e49
```

持续遗产：

- 单一课程工作区；
- Query V2 / 18 字段；
- 动态字典；
- SearchSession；
- sticky nav；
- 用户语言状态；
- 加入列表与 START 分离；
- START 前音频解锁；
- per-resource telemetry；
- FTS、FK index、合法空 Open、V2 local state 等修复。

已替代：

- 9×15 全历史；
- 双 135 全局门；
- 全局 fieldset disabled；
- 全局 fatal circuit；
- 串行 Catalog/Open；
- RC2 包和哈希。

### 15.3 Round 3

```text
baseline: fd0f91bfe8e01616f94cd87cc2ffdcb737812e49
final:    dfabbfdcbea0cb90021ed59d11ccb38c29b19fa7
```

当前最高权威：

- term window；
- current/next 自动范围；
- Local 五学期；
- ONLINE alias 排除；
- target READY；
- Catalog+Open 原子快照；
- 三并发 supervisor；
- target 级 retry；
- selected-target query gate；
- Pull/Apply；
- Fast Lane 和 watch 范围；
- Service Status V2；
- 折叠组定位；
- RC3 双包和验证证据。

---

## 16. 当前 Git、制品与 HumanTest 接续点

### 16.1 Git

```text
branch: codex/p7-implementation
HEAD:   dfabbfdcbea0cb90021ed59d11ccb38c29b19fa7
origin: dfabbfdcbea0cb90021ed59d11ccb38c29b19fa7
```

RC chain：

```text
RC1 bb9700c3587baf3bb29db9b549602d8d1661a502
RC2 fd0f91bfe8e01616f94cd87cc2ffdcb737812e49
RC3 dfabbfdcbea0cb90021ed59d11ccb38c29b19fa7
```

工作区仍非 clean；该事实必须在 Round 4 继续保护。

### 16.2 当前唯一权威候选

Windows：

```text
.cache/rc-iteration/round-03/
dfabbfdcbea0cb90021ed59d11ccb38c29b19fa7/
rbcsp-windows-x86_64-0.1.0.zip

size:   5,961,912 bytes
sha256: 96f32f31813bcf5950dbcec7722203072d462a5fbcfb0cda4f50e18c8fdee853
```

Linux：

```text
.cache/rc-iteration/round-03/
dfabbfdcbea0cb90021ed59d11ccb38c29b19fa7/
rbcsp-linux-x86_64-0.1.0.tar.gz

size:   6,848,381 bytes
sha256: 93414a6e55f38e17983582f5e4e67f8367d6a385de079405040a101645afbd9a
```

当前：

```text
HumanTest/rbcsp-windows-x86_64-0.1.0.zip
```

已核验与上述 RC3 Windows ZIP 的 SHA-256 完全相同。

### 16.3 当前制品证据

- Windows package verifier：PASS；
- Linux workflow `29539206886`，attempt 1：success；
- Rust workspace：PASS；
- 前端 161/161：PASS；
- Public zero-surface：76/76；
- Windows 12 files；
- Linux 21 files；
- 169 shared SBOM components；
- 10 matching embedded frontend components；
- 11 shared frontend capabilities。

### 16.4 当前外部状态

- GitHub Release：未创建；
- tag/assets：未创建；
- Vultr 生产部署：未执行；
- DNS/证书/切流：未执行；
- Round 4：未开始。

---

## 17. 已知残余与非阻断观察

### 17.1 大型 WAL 关闭时间

RC3 fresh QA 大数据关闭时：

- WAL：约 553 MB；
- checkpoint：140.28 秒；
- 最终 SQLite integrity：`ok`；
- WAL/SHM：0；
- instance lock：已删除。

它未被宣布为 RC3 blocker，但若 Round 4 涉及：

- shutdown；
- 大规模数据库；
- checkpoint；
- 状态写入；
- 用户认为关闭太慢；

则必须纳入本轮调查和验证。

### 17.2 官方未来学期日期

- 尚未由 Rutgers 官方发布的 Summer/Winter 日期 fail closed；
- 不通过算法猜测冒充官方日期；
- 如果 HumanTest 证明已经发布的当前日期未被识别，才是实现缺陷。

### 17.3 人耳可闻

- 自动化能验证 AudioContext、播放调用和 `STARTED` outcome；
- 最终是否真正听到声音仍需要 HumanTest。

### 17.4 当前无预设 Round 4 blocker

RC3 没有已宣布的产品 blocker。Round 4 范围必须来自新的真实 HumanTest，不得从 RC1/RC2 旧问题猜测。

---

## 18. Round 4 开始门

开始 Round 4 代码修改前，必须完成：

### 18.1 输入确认

- [ ] 用户使用的包 SHA 为 RC3 Windows SHA；
- [ ] 使用全新解压目录；
- [ ] 不复用旧 data；
- [ ] 记录实际复现顺序、截图和用户预期；
- [ ] 必要时记录启动、退出和错误时间。

### 18.2 只读调查

- [ ] 读取相关 HumanTest 文件但不修改；
- [ ] 区分直接观察与推断；
- [ ] 检查是否与 RC3 合同冲突；
- [ ] 确认是产品缺陷、环境问题、测试问题还是新需求；
- [ ] 不用旧 PASS 否定用户观察。

### 18.3 范围对齐

- [ ] 逐项复述用户问题；
- [ ] 明确预期行为；
- [ ] 明确验收标准；
- [ ] 明确不进入 Round 4 的问题；
- [ ] 若需要改变本文不变量，由用户明确覆盖；
- [ ] 若涉及 UI，纳入两阶段 Skills；
- [ ] 若涉及 shared code，确认双包同步。

### 18.4 实现计划

- [ ] 记录基线 `dfabbfd...`；
- [ ] 记录 dirty baseline；
- [ ] 建立精确 write/commit allowlist；
- [ ] 列出自动化和真实 Windows QA；
- [ ] 列出一个最终提交策略；
- [ ] 列出 Windows 先行和同 SHA Linux 构建；
- [ ] 明确不 Release、不部署；
- [ ] 用户明确批准实施。

在上述条件完成前：

```text
RC_ITERATION_ROUND_4 = NOT_STARTED
```

---

## 19. Round 4 完成门模板

Round 4 真正完成时，最终报告至少应包括：

```text
baseline commit
final commit
round scope
user-visible changes
retained product invariants
explicit supersessions, if any
automated verification
real Windows/Chrome/Rutgers verification
Before | After | Why, if UI changed
Windows artifact path / size / SHA-256
Linux artifact path / size / SHA-256
Linux workflow id / attempt / result
cross-package verification
dirty/untracked preservation result
Release status
deployment status
known residuals
next HumanTest input
```

Round 4 结束后才能将本文的状态更新或由后续 Round 5 接续；不得提前把计划或中间候选写成完成。

---

## 20. 接续摘要

Round 4 的正确起点是：

```text
source:  dfabbfdcbea0cb90021ed59d11ccb38c29b19fa7
package: RC3 final Windows ZIP
evidence: new user HumanTest
phase:   discussion/alignment before implementation
```

Round 4 必须继承：

- RC Iteration 的短反馈循环；
- 最新 HumanTest 高于旧 PASS；
- UI 两阶段 Skills；
- 一轮一个最终提交；
- Windows 优先、同 SHA 双包；
- 用户资产保护；
- Release/部署独立授权；
- RC3 target-level、完整快照、三并发与真实 retry 模型；
- Query V2、单一课程工作区、SearchSession 与 Watch/声音合同。

Round 4 不得继承：

- RC2 全历史双 135；
- 全局 fatal circuit；
- 假 retry；
- ONLINE aliases；
- Catalog-only/Open-only READY；
- 逐小项提交和反复完整 P7.5；
- 任何旧候选或中间 amend SHA。

本文落盘仅完成 Round 4 前的知识接续，不等于 Round 4 已被授权或已经开始。
