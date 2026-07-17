# Rutgers BetterCourseSchedulePlanner

## 完整历史、上下文与时间线（截至 RC Iteration Round 3）

### 文档控制

| 字段 | 值 |
|---|---|
| Record ID | `RBCSP-HISTORY-CONTEXT-TIMELINE-THROUGH-RC3-2026-07-17-001` |
| 状态 | `CURRENT_HISTORICAL_CONTEXT_BASELINE` |
| 截止点 | `RC_ITERATION_ROUND_3_COMPLETE` |
| 当前源码基线 | `dfabbfdcbea0cb90021ed59d11ccb38c29b19fa7` |
| 当前分支 | `codex/p7-implementation` |
| 记录日期 | `2026-07-17` |
| 用途 | 为后续 RC Iteration、Release 与部署提供连续、可追溯的历史上下文 |
| 非授权边界 | 本文不授权 Round 4 实现、GitHub Release、Vultr、DNS、证书或生产部署 |

---

## 1. 文档目的

本文将项目从旧 TypeScript/Node 版本的审计、双包产品定义、P1–P6 需求与架构闭环、P7 实施、真实世界 E2E 纠偏，一直到 RC Iteration Round 1–3 的完整演化整理为一条连续时间线。

本文解决四个问题：

1. 项目最初是什么，为什么必须重建而不是局部修补；
2. 当前“两包、共享核心、不同运行时”的产品是如何形成的；
3. 哪些阶段结论后来被真实 HumanTest 或用户裁决推翻；
4. 截至 RC3，代码、制品、Release 与部署分别处于什么状态。

RC Iteration 的专门定义、跨轮要求、当前产品不变量及 Round 4 接续边界，见同目录：

- `02-rc-iteration-authoritative-model-and-round-04-handoff.md`

Round 3 的产品和技术设计原始决策记录见：

- `00-rc-iteration-round-03-product-and-technical-design-decision-record.md`

---

## 2. 证据口径与权威顺序

### 2.1 Chatlog 导出限制

Chatlog 导出明确省略了 tool calls、tool outputs、内部 reasoning、命令记录及部分 rollback 内容。因此本文区分：

- **用户原话/批准/否决**：日志中的直接历史证据；
- **Codex 当时报告**：代表当时声称完成或通过，不能自动等同于原始命令证据；
- **当前独立核验**：通过现有 Git 对象、分支、制品、哈希和落盘测试摘要得到的当前证据。

### 2.2 冲突时的权威顺序

发生冲突时使用以下顺序：

1. 更新、更晚的用户明确裁决；
2. 更新、更晚的真实 HumanTest；
3. 当前已实现并验证的 RC3 决策记录；
4. 当前 Git 与制品的独立核验；
5. 早期阶段合同与计划；
6. 旧日志中的 PASS、旧候选包和中间 amend SHA。

因此：

- P3/P4/P5 PASS 不代表产品已经实现；
- P7.5 的旧 PASS 可被真实用户使用推翻；
- Round 1、Round 2 的部分产品合同可被 Round 3 明确替代；
- 旧候选包只能解释历史，不能作为下一轮修改基线。

### 2.3 物理文件与逻辑会话

共读取 8 个物理文件、19,207 行。2026-07-10 和 2026-07-13 各有一对同一 Codex session 的前后两次导出；将每对视为一个逻辑会话后，共对应 6 段逻辑会话。若每对只计更新、更完整的后一次导出，则最新导出语料共 17,101 个物理行。这里的行数只用于说明审阅覆盖面，不代表去重后的语义事实数量。

| 时间/导出 | 物理文件 | 作用 |
|---|---|---|
| 2026-05-12 | `docs/chat-log-2026-05-12T06-12Z.md` | 旧项目审计、Phase 1 与 ALL AND ONLY 来源 |
| 2026-07-10 | `chat-log-codex-2026-07-10-1ce70862.md` | 双产品、0A/0B/0C 与早期 NGAT 流程 |
| 2026-07-10 增量 | `chat-log-codex-2026-07-10-1ce70862-1.md` | 对话正文完整包含上一版的 1,259 行正文；追加 P1 失败和单主线纠偏 |
| 2026-07-12 | `chat-log-codex-2026-07-12-56fa8f10.md` | 重新建立 P1–P6 权威需求与数据合同 |
| 2026-07-13 简版 | `chat-log-codex-2026-07-13-86c5b28a.md` | P7 开始前快照 |
| 2026-07-13 主日志 | `chat-log-codex-2026-07-13-86c5b28a-1.md` | P7.1–P7.5 完整实施，跨至 7 月 16 日凌晨 |
| 2026-07-16 第一段 | `chat-log-codex-2026-07-16-05afa573.md` | 重做真实 E2E、HumanTest 与 RC1 |
| 2026-07-16 第二段 | `chat-log-codex-2026-07-16-6c08e851.md` | RC2、RC2 失败、RC3，跨至 7 月 17 日 06:30 |

`chat-log-codex-2026-07-13-86c5b28a-1.md` 的对话正文完整包含简版的 812 行正文，是同一 session 的更完整导出，不能把二者当成平行事实链。

---

## 3. 项目长期目标与最终产品定义

### 3.1 产品目的

当前产品的目的不是替代 Rutgers Course Schedule Planner，而是改善其课程发现、复杂筛选、性能、Section 查看和开放提醒体验。

核心产品模型是：

- 课程为主要搜索单位；
- Section 仍可独立筛选、展示、直链和监看；
- 课程条件与 Section 条件共同参与查询；
- 所有 Section 条件必须由同一个 Section 共同满足；
- 时间可用性必须覆盖同一 Section 的所有必需 meeting；
- 无法可靠判断时使用 `UNCERTAIN`，不能伪造成确定匹配。

来源：

- `chat-log-codex-2026-07-12-56fa8f10.md:816-827`
- `chat-log-codex-2026-07-12-56fa8f10.md:1588-1720`

### 3.2 严格两个包

项目最终只产生两个包：

1. Windows 本地一键包；
2. Linux 公网服务包。

生产部署是使用 Linux 公网包执行的后续操作，不是第三个包。Release 页面、测试证据、审计记录和私有 inventory 也不是第三个包。

### 3.3 共享和变体边界

共享部分：

- Rust domain、Catalog/Open、query、watch 和业务合同；
- React WebUI；
- 课程与 Section 搜索；
- FilterSchema / Query Contract；
- WebSocket Watch、toast、声音；
- 双语界面。

Windows Local：

- 包内相对 SQLite：`<package-root>/data/rbcsp.sqlite`；
- Saved Views、History、Settings、Reset；
- 用户可配置部分刷新频率；
- Local 五学期窗口和另外三个学期手动拉取。

Linux Public：

- 服务状态目录；
- 浏览器用户会话临时、fresh；
- 不保留 Saved Views、History、Reset 或其他 Local 个人能力；
- 不提供手动拉取历史学期；
- 普通用户不能配置刷新频率。

### 3.4 v1 明确不包含

- email；
- Discord；
- macOS；
- Calendar；
- Share links；
- Waitlist；
- Compact view；
- 任意年份、无限历史学期；
- Public 用户个人持久化。

---

## 4. 2026-05-12：旧项目审计与 ALL AND ONLY

### 4.1 当时的旧实现

旧项目为 Node/TypeScript、Fastify、SQLite、React/Vite 架构，包含 Rutgers SOC 抓取、API、WebUI、poller 与邮件/声音提醒。

只读审计报告：

- 中英 i18n 各 265 个叶子 key，通过；
- 前端 TypeScript 与生产构建通过；
- 根级严格 TypeScript 检查失败；
- 后端、worker、mail 共 45 个测试，42 通过、3 失败；
- 数据库迁移 001–004 和 API smoke 成功；
- Rutgers Catalog/Open 实际探测成功；
- 全程未修改源码。

来源：

- `docs/chat-log-2026-05-12T06-12Z.md:122-178`
- `docs/chat-log-2026-05-12T06-12Z.md:214-274`

### 4.2 确认的旧系统问题

- `/api/sections` 有接口壳但固定返回空数组；
- 文档却把它描述为真实 endpoint；
- fetch 文档与代码真实能力漂移；
- 数据库默认路径有多套说法；
- 本地管理和邮件表面依赖未经明确说明的无认证假设；
- 报告、worker 输出与 public main 漂移；
- public 侧存在未接线的 scheduled-fetch/auto-refresh 残留。

### 4.3 ALL AND ONLY

用户最终定义的 Phase 1 不是恢复所有历史想法，也不是只保留当前代码已有功能，而是恢复“BCSP 本应成为的产品”：

- 应有能力必须全部存在；
- 因 bug、模型能力或流程问题错误舍弃的能力应重新评估；
- 写错、写偏、写残、写旧、写危险的内容必须清理；
- 不应存在的能力必须从 UI、API、worker、配置、文档、测试和包内容中完全消失。

来源：

- `docs/chat-log-2026-05-12T06-12Z.md:742-860`

### 4.4 UI 重做的含义

用户从一开始就明确：UI rewrite 同时是 UI 与 UX 重做，不是仅更换颜色或视觉皮肤。

### 4.5 第一次 NGAT 编排失败

第一次 task-015 连续三次未形成可合并闭环：

- tmux 默认 session 名与另一项目冲突；
- 手工 follow-up 造成 provenance turn 数不一致；
- 第三次 prompt 停留在输入框，没有真正执行。

达到三次上限后停止。该失败属于执行平面和 provenance 失败，不是产品实现失败。

来源：

- `docs/chat-log-2026-05-12T06-12Z.md:1001-1140`

---

## 5. 2026-07-10：双交付、共享 Rust 架构与第二次编排失控

### 5.1 双交付路线

用户将长期目标收敛为：

- 普通 Windows 用户可下载、解压并一键运行的本地包；
- 手机和电脑浏览器都可直接访问的公网网站。

Windows 本地包继续继承 ALL AND ONLY 质量要求。

来源：

- `chat-log-codex-2026-07-10-1ce70862.md:88-94`

### 5.2 0A 产品决策

早期冻结的核心偏好：

- 两包使用同一 WebUI 与核心能力；
- 删除 email 和 mail config；
- 只保留页面打开期间的声音提醒；
- subscription/watch 以 Section 为单位；
- 一个浏览器最多监看 9 个 Section；
- watch 只存在于当前 live browser connection；
- 公网服务器集中访问 Rutgers，浏览器不直连上游；
- Catalog 默认 10 分钟刷新；
- 浏览器通过 WebSocket/SSE 获得实时状态。

来源：

- `chat-log-codex-2026-07-10-1ce70862.md:750-821`

早期“收到 OPEN 就响、不要求变化”的简单表达，后来由 P3 和 P7 的 fresh/LKG、episode 与初始 already-open 合同进一步限定；不能单独作为当前完整声音合同。

### 5.3 0B 部署平台

OCI Always Free 因注册验证失败退出主路径，最终选择：

- Vultr EWR；
- Ubuntu 24.04；
- $6/月 AMD High Performance VM；
- SSH 与私有 inventory 已建立；
- 当时没有应用、Caddy 或生产配置。

### 5.4 0C 技术架构

新架构冻结为：

- 一套 React WebUI；
- 一套 Rust 共享核心；
- Windows `bcsp-local.exe` / `RBCSP.exe`；
- Linux `bcsp-server`；
- SQLite、集中 poller、WebSocket；
- Linux 使用 Caddy 和 systemd；
- 旧 Fastify/Node 后端只作为迁移证据，不再是目标架构。

来源：

- `chat-log-codex-2026-07-10-1ce70862.md:934-948`

### 5.5 UI 分阶段要求

P7 UI 原计划为两个独立 subphase：

- P7.2：`industrial-brutalist-ui` + `design-taste-frontend`；
- P7.3：`emil-design-eng` 独立审计与打磨。

P7 阶段要求二者任务、记录和提交分开。该“分别提交”要求在 RC Iteration 建立后被替换为“同一轮内部两阶段、整轮一个最终提交”；技能顺序仍保留。

### 5.6 第二次 NGAT 失控

“拆得越细越好”被过度强化：

- 初始 P1 拆成 23 个任务；
- 又建立两套 remediation/file-based 替代链；
- 最终形成 45 个重叠任务；
- P7 才应使用的逐任务提交规则错误套用到 P1；
- 后加任务没有重新批准。

用户停止运行并冻结 organ，不回滚现场。

来源：

- `chat-log-codex-2026-07-10-1ce70862.md:1173-1265`

### 5.7 旧 P1 被拒绝

8-task P1 一度报告完成，但主线发现它只恢复了新讨论，遗漏旧项目已有的多选、时间窗口与组合筛选语义。P1 因而回到 `CHANGES_REQUESTED`，P2 冻结。

来源：

- `chat-log-codex-2026-07-10-1ce70862-1.md:1426-1462`

### 5.8 改为单主线

用户取消 NGAT 的第二权威线：

- 当前 Codex 是唯一执行、调查和 gate authority；
- 用户保留策略、批准和覆盖权；
- 子代理可处理有边界的子任务，但不能形成第二产品权威、改变范围或跨门；
- 不再让外部 organ 取代当前主线。

P1 补救最终形成 159 条行为级 legacy capability inventory。

来源：

- `chat-log-codex-2026-07-10-1ce70862-1.md:1464-1534`

---

## 6. 2026-07-12：重新建立 P1–P6 权威主线

### 6.1 新工作流和严格两包

7 月 12 日不继承 7 月 10 日旧 P1 产物，重建单主线 0A–P7。交付名称改为：

- 公网包以及部署；
- 本地一键包。

部署不是第三个包。当前权威工作流落在：

- `project-governance/current/single-mainline-delivery-workflow.md`

### 6.2 P1 Cleanroom

P1 报告 74/74，并记录两次安全事件：

- 错误排除规则机械扫描了禁止读取标题，结果被丢弃；
- 私密邮件配置只做结构分类，不使用内容。

用户接受处置并批准 P1。

### 6.3 用户冻结的产品范围

- 改善 CSP 搜索、筛选与性能；
- course-centered，保留独立 Section；
- 所有必需 meeting 必须落入时间可用窗口；
- Windows 和 Linux/Public 两包；
- subscription、toast、声音与最大通知数必须做；
- Discord 删除；
- Calendar 延后。

来源：

- `chat-log-codex-2026-07-12-56fa8f10.md:816-827`

### 6.4 P2 根管式 ALL AND ONLY

P2 报告：

- 158/158 文件；
- 38 个 mixed 区域；
- 85 个语义单元；
- 25 个 ALL；
- 22 个 ONLY；
- 57 条 reuse path。

发现：

- Section API stub；
- 无真实 WebSocket；
- 旧持久 subscription 模型不符合当前产品；
- email 分布在多层；
- meeting time 语义不完整。

P2 Review 又冻结：

- Public fresh session；
- Local persistence + Reset；
- Local-only Saved Views；
- 公网必须真正 zero surface，而非 CSS 隐藏；
- audible max 必须真实执行；
- alarm 按 episode；
- 双语；
- 同一 Section 满足全部 Section 条件。

### 6.5 P3 真实 Rutgers 数据合同

Catalog 证据：

- 21 个成功 payload；
- 10,629 courses；
- 22,069 sections；
- 30,804 meetings；
- 22,051 个唯一 `(term,campus,index)`；
- 裸 index 不足，Section identity 必须是复合键。

Open：

- 21 scope × 2 轮；
- 42/42 HTTP 200；
- 真实数据推翻 same-scope exact reverse join；
- 用户批准官方 set-membership/intersection。

刷新合同：

- Catalog 正常约 10 分钟；
- Open 正常约 30 秒；
- active-watch scope 约 10 秒；
- observation 到 UI/audio fanout 工程目标约 1 秒；
- 不承诺无法控制的 Rutgers 上游 SLA。

来源：

- `chat-log-codex-2026-07-12-56fa8f10.md:1768-1978`

### 6.6 P4 Public Delta

P4 报告 76 个 delta：

- fresh anonymous browser session；
- 不保留 local personal state；
- 浏览器不直连 Rutgers；
- poller 请求不随用户、tab 或 watch 数放大；
- 18 个 deny capabilities 在 source、DOM、route、API、storage、i18n、bundle、package 八层都必须不存在。

### 6.7 P5 共享和变体分类

76 项能力分类为：

- `SHARED=46`；
- `LOCAL_ONLY=9`；
- `PUBLIC_ONLY=19`；
- `EXCLUDED=2`。

禁止两套 query、两套 Open reconcile、两套 FilterSchema 或长期 local/public fork。

### 6.8 P6 执行 DAG

最初 P7 计划为 27 项：

- P7.1 功能：15；
- P7.2 UI：4；
- P7.3 UI 打磨：3；
- P7.4 验证与打包：5。

当时准确状态：

```text
P1/P2 = APPROVED / CLOSED
P3/P4/P5 = 当时报告 PASS
P6 = REVIEW READY
P7 = NOT STARTED
包 = 未生成
Release = 未发生
部署 = 未发生
```

P3–P6 的 PASS 只代表证据、合同和计划闭合，不代表 Rust/React 已实现。

---

## 7. 2026-07-13：P6 Review、P7 授权与实施

### 7.1 P6 Review 裁决

用户决定：

- Windows 使用包内相对单一 SQLite；
- 两包均不得预装真实课程数据；
- 顺序为功能 → 正式 UI → 独立打磨 → 打包；
- Release、Vultr、DNS、证书均在 P7 后；
- 接受严格两个包；
- 增加真实世界 E2E。

P7 从 27 项增加为 32 项，新增加 P7.5 五项。

来源：

- `chat-log-codex-2026-07-13-86c5b28a-1.md:404-625`

### 7.2 Vultr 基线检查

只读检查发现 systemd degraded，原因是 fwupd daemon/library 版本不一致。经用户两次精确授权，只执行：

1. 安装/升级对应 fwupd 最小包并重启两个 fwupd 服务；
2. 重启 `unattended-upgrades.service` 清除 needrestart 残留。

最终 systemd running、failed units=0。未安装 BCSP/Caddy，未部署应用，未创建 snapshot，未进行生产变更。

### 7.3 脏工作区隔离

P7 启动规则：

- 从当前基线建立 `codex/p7-implementation`；
- 不 reset、不 stash；
- 冻结 167 条用户路径；
- 任务只暂存 allowlist；
- 排除 chatlog、私有 inventory、密钥和用户文件。

用户于：

- `chat-log-codex-2026-07-13-86c5b28a-1.md:839-845`

正式批准进入 P7。

### 7.4 P7.1 功能阶段

P7.1 的 15 项依次覆盖：

1. 分支和基线；
2. Rust/Node、依赖、许可证、SBOM；
3. Rust workspace 和共享 React 双入口；
4. domain identity、API 与 wire schema；
5. Catalog discovery、normalize、SQLite、FTS；
6. FilterSchema、三值 predicate、same-section witness；
7. Open client、reconcile、storage、scheduler、freshness；
8. WebSocket、最多 9 个 watch、episode/toast/audio；
9. 双语 runtime；
10. Windows local runtime；
11. Saved Views、History、Settings、Reset；
12. Linux public runtime；
13. Public 144 个 zero-surface 断言；
14. Linux systemd/Caddy、备份、恢复、升级、回滚；
15. 双入口完整接线与 synthetic E2E。

### 7.5 P7.1-005 的治理递归纠偏

Catalog replay 一度生长为 write-once intent、predecessor replay、恢复状态机和多层 validator。用户明确批评“治理的治理”，只保留：

- 防真实数据泄漏；
- 用户 167 路径保护；
- 任务边界；
- 真正不可重复操作；
- 可信提交。

大量辅助治理代码随后删除，工作回到真实产品。

来源：

- `chat-log-codex-2026-07-13-86c5b28a-1.md:1959-1987`

### 7.6 真实数据带来的实现修正

- `comments` 实际为对象数组；
- NB selector 可合法返回 campus OB；
- variant signature 与 campusLocations 需要合并；
- 存在等价重复、multi-variant 和 delivery conflict；
- 不能通过粗暴去重损失数据。

### 7.7 P7.2 UI

使用 `industrial-brutalist-ui` 与 `design-taste-frontend` 实现：

- Swiss Industrial 响应式 shell；
- 课程/Section 查询、筛选、详情和 deep link；
- watch、toast、audio、freshness；
- Local Saved/History/Settings；
- Public/Local 能力隔离；
- 双语和无障碍。

### 7.8 P7.3 独立 UI 打磨

使用 `emil-design-eng` 完成：

- focus；
- 响应式；
- Settings/Toast；
- 懒加载；
- 键盘、axe 和性能；
- Public zero surface 复验。

### 7.9 P7.4 打包

真实打包陆续发现：

- Rust runtime 最初没有真正托管 Vite UI；
- deep link 资产和 CSP 不完整；
- Windows 静态 CRT、Unicode/空格路径、包外 CWD、升级路径；
- Linux ops、systemd、Caddy、备份恢复；
- build path 泄漏；
- Windows CRLF 与 Linux LF provenance 差异；
- MSVC 版本漂移和绝对路径泄漏。

因此早期候选包均已退役。

### 7.10 P7.5 真实 E2E 定义

用户定义真正的 E2E 必须是：

```text
下载包
→ 解压
→ 运行
→ 用真实 Chrome 操作真实页面
```

公网必须真实经 Linux 服务、Caddy、HTTPS/WSS 进入，后台 API probe 或 localhost 不能替代用户流程。

### 7.11 P7.5 Windows 纠偏

Windows 真实测试发现：

- Catalog 有数据但 subject=0、搜索挂起；
- 数据库 mutex 自锁；
- 巨型 publish 阻塞读连接；
- Chrome 控制会话失效；
- 测试把人工 35 次请求预算误当产品刷新规格。

用户明确：35 只是 E2E 人工预算，不能反向削弱正常约 30 秒 Open 与约 10 秒 watch。错误修改被全部撤回。

来源：

- `chat-log-codex-2026-07-13-86c5b28a-1.md:4277-4305`

### 7.12 7 月 13 日主日志的结束状态

该日志结束时：

- Windows 真实 Chrome E2E 已更正为 PASS；
- Linux 第一次 live E2E 因脚本误折叠筛选区而在搜索前失败；
- 脚本修复后普通 CI 通过；
- live 例外重试尚未批准；
- Vultr 公网 Chrome E2E 尚未执行；
- P7.5 尚未在该日志中收口；
- Release/部署未授权。

这个终点后来被 7 月 16 日的新会话继续推进，不能当成当前最终状态。

---

## 8. 2026-07-16 第一段：重做真实 E2E 与 RC Iteration Round 1

### 8.1 旧 P7.5 结论再次被推翻

用户指出 Actions artifact 虽能启动，却不能真正搜索；此前大部分 P7.5 不满足“下载、解压、运行、使用”的定义。

“standard user”被精确解释为：

- 当前 Windows 用户；
- fresh directory；
- 不设置 `BCSP_CI_NO_RUTGERS`；
- 运行当前候选 EXE；
- 使用真实 Chrome；
- 不要求创建新的 Windows OS account。

来源：

- `chat-log-codex-2026-07-16-05afa573.md:132-171`

### 8.2 P7.5 重测发现的问题

- 初始 UI 卡在空状态，Retry 后才能搜索；
- History worker 与服务连接争用 SQLite；
- STOP 后仍可能重复声音和 max toast；
- cadence gate 不一致；
- WebSocket 多会话锁反转；
- 同步维护任务阻塞 Tokio。

这些问题修复后，一度在约 06:08 报告 P7.5 PASS，但用户随后实际使用精确候选，又发现五项产品问题。

### 8.3 第一次 HumanTest 五项问题

1. 服务状态不可见；
2. 超宽屏整块空白和低效 rail；
3. 最大 2 学分仍出现 3 学分；
4. Sections 默认全部展开；
5. Core Code 只能手填。

根因：

- readiness 层次没有被用户看到；
- `100rem` max width 与全高 rail；
- course group 返回 `NO_MATCH` sibling；
- `3_0` 学分解析失败；
- `<details open>`；
- Core 对象数据未进入动态字典。

来源：

- `chat-log-codex-2026-07-16-05afa573.md:893-1198`

### 8.4 RC Iteration 正式建立

用户要求停止反复完整 E2E，改为：

```text
实际使用
→ 记录问题
→ 定向修改
→ 本地重新打包
→ 再次实际使用
```

RC Iteration 被定义为 P7 后、Release 前的独立阶段：

- 不再每小项提交；
- 不再每项 PostPush；
- 不再每项触发 Actions；
- 不再重复完整 P7.4/P7.5；
- UI 两阶段继续使用三项 Skills，但同一轮只保留一个最终提交；
- Windows 先本地构建，Linux 只做整轮必要的一次构建；
- 新问题进入下一轮。

来源：

- `chat-log-codex-2026-07-16-05afa573.md:1200-1374`

### 8.5 RC1

Round 1 实现：

- Service Status V1；
- 学分解析和 `NO_MATCH` 裁剪；
- Core 动态字典；
- 全宽布局；
- Sections 默认收起；
- 状态和 personal mutation 锁修复；
- RC 构包工作流。

Round 内 amend 链：

```text
9db7392 → 5847dca → dd7c787 → bb9700c
```

这些不是四轮，而是同一 Round 1 最终提交身份的连续替换。

最终：

- commit `bb9700c3587baf3bb29db9b549602d8d1661a502`；
- Windows SHA-256 `c7e324016661971d709abc7477d98bf3c7afd8addfde0e27959ffccb92eec99c`；
- Linux SHA-256 `d4314c21cdd0a81a8da1b3970914093622251ef16c8ec2919600eacb5a3c7701`；
- 未 Release，未部署。

来源：

- `chat-log-codex-2026-07-16-05afa573.md:2202-2225`

---

## 9. 2026-07-16 第二段：RC2 与 RC3

### 9.1 RC1 后的 12 项 HumanTest 反馈

用户报告：

- 状态文案不清、缺少总状态与进度；
- 状态区仍浪费空间；
- 删除课程目录工作区装饰；
- 删除独立 Section 一级搜索入口；
- 删除四类冗余筛选及 Core 搜索输入；
- Level/Exam/Subcampus 使用动态选项；
- Instructor/Keyword 使用动态字典；
- 筛选栏增宽；
- 页面往返保留搜索结果；
- 13222 OPEN 但未听到声音；
- 删除“WebSocket 空闲”等实现语言；
- sticky 一级导航。

来源：

- `chat-log-codex-2026-07-16-6c08e851.md:482-518`

### 9.2 RC2 持续保留的成果

- 单一课程工作区；
- Query Contract V2，18 个字段；
- 动态字典；
- SearchSession；
- sticky navigation；
- 用户语言状态；
- Watch 加入列表与 START 分离；
- START 前音频解锁；
- per-resource telemetry；
- FTS、索引、恢复优先级和合法空 Open 修复。

### 9.3 RC2 的错误全局合同

用户当时接受：

```text
Catalog 135/135
AND Open 135/135
→ 才启用整个搜索表单
```

该合同随后被真实 HumanTest 证明不可靠并由 RC3 明确替代。

### 9.4 RC2 最终产物

- commit `fd0f91bfe8e01616f94cd87cc2ffdcb737812e49`；
- Windows SHA-256 `dbd1f526ed05673cc789c384db50f22ed4b07fd813b95e045b225553fda4776f`；
- Linux SHA-256 `f3270e49f9b2df24c5513dd7aca9ffaaecafaf1dd18d47c1d1670cb2f089f364`；
- 当时 QA 可达到双 135；
- 未 Release，未部署。

来源：

- `chat-log-codex-2026-07-16-6c08e851.md:2454-2482`

### 9.5 RC2 HumanTest 失败

用户运行最终 RC2 Windows 包：

| 时间（Asia/Shanghai） | 事件 |
|---|---|
| 22:44:11 | 开始 discovery |
| 22:44:27 | 发现 9 学期 × 15 Campus = 135 |
| 22:44:27–22:45:19 | 前 40 个 Catalog 连续成功 |
| 22:45:19 | 开始 `12025/NB` |
| 22:47:19 | 约 120 秒后记为 `CATALOG_UPSTREAM_SCHEMA` |
| 此后 | 不再发起 Catalog 或 Open |
| 约 22:52 | 用户退出 HumanTest |

高置信失败链：

```text
大型 gzip/body 读取或解压过慢
→ 120 秒总超时
→ 错误同时带 decode/timeout
→ 代码先判断 decode
→ 误分类 Schema/FatalProtocol
→ 打开 origin FatalDiagnostic
→ 后续所有请求永久停止
```

真正的问题不是 SOC API 难用，而是本方：

- 错误分类；
- 全局熔断；
- 全历史串行拉取；
- 全量门禁；
- UI retry 文案与后台不一致。

用户将其定性为过度防御、过度实现。

### 9.6 RC3 决策记录

Round 3 先经过连续讨论，再落盘：

- `project-governance/current/rc-iteration/00-rc-iteration-round-03-product-and-technical-design-decision-record.md`

该记录在冲突时替代 RC2 与以下事项有关的旧合同：

- 数据范围；
- readiness；
- Pull/Apply；
- 刷新；
- retry；
- Campus；
- watch 范围。

### 9.7 RC3 当前合同

- 使用纽约时区和 Rutgers 教学日历；
- Public current+next；
- Local 前二+当前+后二；
- current+next 自动拉取；
- Local 其他三个学期手动拉取；
- 排除 ONLINE aliases；
- READY 粒度为 `(term, campus)`；
- Catalog+Open 构成完整原子快照；
- selected-target query gate；
- 最多三工作流；
- target 级故障隔离和真实 retry；
- 只有明确 429 才暂停 origin；
- LKG 在刷新失败时继续可用；
- Fast Lane 跟随 active watch；
- watch 限制 current+next；
- candidate scope 与 applied scope 分离；
- 单一“拉取/应用”按钮；
- 状态文案必须由后台真实状态驱动；
- 折叠组切换后标题进入可视区域。

### 9.8 RC3 实施与真实验证

实现和 QA 报告：

- fresh current/next 约 26 秒达到 12/12 + 12/12；
- 峰值并发 3；
- ONLINE alias 为 0；
- Summer/NB 可独立 Apply/Search，返回 1,045 门；
- Local 手动 Pull Spring 达到 12/12，Apply 后返回 4,559 门；
- scope 切换后旧响应不再回填；
- active watch 在页面 scope 切换后仍保持 Fast Lane；
- 390、1440、1920、2560 视觉 QA 通过；
- 修复 shutdown WebSocket 重连竞态；
- 联合制品 verifier 通过。

### 9.9 RC3 最终产物

- commit `dfabbfdcbea0cb90021ed59d11ccb38c29b19fa7`；
- Windows SHA-256 `96f32f31813bcf5950dbcec7722203072d462a5fbcfb0cda4f50e18c8fdee853`；
- Linux SHA-256 `93414a6e55f38e17983582f5e4e67f8367d6a385de079405040a101645afbd9a`；
- Linux workflow `29539206886`，attempt 1，success；
- 本地与远端分支一致；
- 未创建 Release；
- 未部署。

来源：

- `chat-log-codex-2026-07-16-6c08e851.md:5481-5517`
- `.cache/rc-iteration/round-03/dfabbfdcbea0cb90021ed59d11ccb38c29b19fa7/target-test-summary.md`

### 9.10 已知非 blocker 观察

大型 QA 数据关闭时：

- WAL 约 553 MB；
- checkpoint 用时 140.28 秒；
- 最终 `PRAGMA integrity_check=ok`；
- WAL/SHM 清零；
- instance lock 删除。

该现象未被宣布为 RC3 blocker，但必须保留为下一轮已知上下文。

---

## 10. 关键决策演化

| 主题 | 早期设计 | 后续证据/纠偏 | 当前权威结论 |
|---|---|---|---|
| 执行权威 | NGAT 双线 | 两次编排失控 | 当前 Codex 单主线，子代理无第二产品权威 |
| 产品包 | 本地工具、云部署等多种说法 | 7/12 重命名 | 严格 Windows Local + Linux Public 两包 |
| P7.5 | 冻结候选最终 E2E | 候选需持续修改 | RC Iteration 独立位于 P7 后、Release 前 |
| 搜索 readiness | RC1 Catalog/LKG 即可搜 | RC2 改双 135，又被 HumanTest 推翻 | 所选 target 必须有 Catalog+Open 完整快照 |
| 数据范围 | 9×15 全历史 | RC2 一个慢 target 停全局 | current+next 自动；Local 五学期窗口 |
| Campus | ONLINE aliases 当独立 Campus | 真实数据证明是筛选别名 | discovery 后排除 ONLINE aliases |
| 调度 | 全局串行 | 启动脆弱、恢复饥饿 | 最多 3 个 target 工作流 |
| 错误 | decode/schema 可升级全局 fatal | 慢请求误分类停全局 | target 级隔离；仅明确 429 暂停 origin |
| retry 文案 | UI 可显示自动重试 | 后台实际永久停止 | 必须存在真实 `nextRetryAt` 和 retry 工作 |
| Open 刷新 | 初期混合意见 | 真实数据和用户纠偏 | normal 30 秒；watch 约 10 秒；不能用 35 次测试预算削弱 |
| Fast Lane | 可能跟随页面 | 用户纠偏 | 跟随 active watch target，只提升 Open |
| UI 实施 | P7 两技能阶段分别提交 | RC 阶段避免提交浪费 | 两阶段仍执行，但整轮一个最终提交 |
| Release | 可能在 P7 后直接发布 | RC HumanTest 继续发现问题 | 只能发布 HumanTest 接受的相同字节，仍需单独授权 |

---

## 11. 已被明确替代、不得恢复为当前合同的内容

1. 9 学期 × 15 Campus 全历史笛卡尔积默认拉取；
2. 先完成全部 Catalog，再完成全部 Open；
3. Catalog 135/135 且 Open 135/135 才允许搜索；
4. 全局禁用 term/campus scope 控件；
5. 单 target timeout/decode/schema 升级 origin 永久熔断；
6. 等待不存在的显式诊断授权后恢复；
7. UI retry 只刷新页面状态；
8. ONLINE aliases 作为独立 Campus；
9. Catalog-only/Open-only 作为完整 READY；
10. 新 Catalog + 旧 Open 混合公开；
11. 未选择的失败 Campus 阻塞已选择的 READY Campus；
12. Local 任意历史学期；
13. Public 手动 Pull；
14. 用“继续拉取/正在拉取/已应用”等第三种按钮状态承载进度；
15. 将 6 小时当作 Catalog 刷新周期；
16. Fast Lane 跟随当前页面；
17. 允许监看 current/next 之外学期；
18. 为人工 E2E 请求预算削弱正常 30/10 秒能力；
19. 每小任务独立提交、每项 PostPush、每项线上构建；
20. 把任何中间 amend SHA 或旧候选哈希当成当前包。

---

## 12. 截至 RC3 的当前权威状态

### 12.1 Git

```text
branch: codex/p7-implementation
local HEAD:  dfabbfdcbea0cb90021ed59d11ccb38c29b19fa7
origin HEAD: dfabbfdcbea0cb90021ed59d11ccb38c29b19fa7
```

RC 提交链：

```text
bb9700c3587baf3bb29db9b549602d8d1661a502  RC1
fd0f91bfe8e01616f94cd87cc2ffdcb737812e49  RC2
dfabbfdcbea0cb90021ed59d11ccb38c29b19fa7  RC3 / current
```

### 12.2 当前制品

Windows：

```text
.cache/rc-iteration/round-03/
  dfabbfdcbea0cb90021ed59d11ccb38c29b19fa7/
  rbcsp-windows-x86_64-0.1.0.zip

size: 5,961,912 bytes
sha256: 96f32f31813bcf5950dbcec7722203072d462a5fbcfb0cda4f50e18c8fdee853
```

Linux：

```text
.cache/rc-iteration/round-03/
  dfabbfdcbea0cb90021ed59d11ccb38c29b19fa7/
  rbcsp-linux-x86_64-0.1.0.tar.gz

size: 6,848,381 bytes
sha256: 93414a6e55f38e17983582f5e4e67f8367d6a385de079405040a101645afbd9a
```

当前 `HumanTest/rbcsp-windows-x86_64-0.1.0.zip` 的 SHA-256 与上述 Windows RC3 包完全一致。

### 12.3 已完成

- 产品范围和排除范围；
- P1–P6；
- P7 功能、UI、打磨、双包和真实测试；
- RC1、RC2、RC3；
- 当前 RC3 双包构建与联合校验；
- 远端分支同步。

### 12.4 未完成/未授权

- RC Iteration Round 4；
- 正式 GitHub Release；
- Release tag/assets；
- Vultr 生产部署；
- 生产 DNS；
- 正式证书与生产切流。

---

## 13. Round 4 接续边界

Round 4 的唯一正确源码基线是：

```text
dfabbfdcbea0cb90021ed59d11ccb38c29b19fa7
```

唯一正确 HumanTest 输入包是当前 RC3 Windows ZIP。Round 4 应按以下顺序开始：

1. 用户全新解压 RC3 Windows ZIP；
2. 用户真实使用并返回问题、截图、预期和观察；
3. 先理解、调查和对齐，不修改代码；
4. 区分真实缺陷、产品选择、测试问题和历史已知行为；
5. 冻结 Round 4 范围与验收；
6. 写 Round 4 实现计划；
7. 用户明确开始后才实施。

Round 4 详细权威模型见：

- `02-rc-iteration-authoritative-model-and-round-04-handoff.md`

本文落盘不等于 Round 4 已开始。
