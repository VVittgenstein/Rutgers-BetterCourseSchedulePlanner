# P2 验证、结论与 Review 停止门

## 1. 状态

- **P2执行状态**：`AUDIT COMPLETE — REVIEW REVISIONS INTEGRATED — CLOSED`
- **审核状态**：`P2 APPROVED`
- **硬停状态**：`CLEARED — P3 ELIGIBLE, NOT STARTED`
- **启动**：2026-07-12T22:54:58+08:00
- **原审计完成**：2026-07-12T23:30:24+08:00
- **Review修订**：2026-07-13；用户已回复本文件原§8第1–10项，并在全部后续修订完成后明确回复“批准P2”
- **Git快照**：branch `dev`，HEAD `a4b035a586a4b14fc3a75698caf99badce869fd5`

## 2. 语料和文件全集核验

| Check | Result |
|---|---:|
| Repo tracked entries | 279 |
| `OWNED_PRODUCT`文件 | 158 |
| 文件矩阵rows / unique paths | 158 / 158 |
| 文件矩阵启动hash | `582B28E693B4E8638CB9EF9B5EFD90C799B07478C8511AB6D191A7FCF9C0BFAC` |
| 启动时这158个文件的worktree状态 | 158 clean |
| Mixed files | 38 |
| Mixed files下钻 | 38 / 38 |
| 文件内语义rows | 91 |
| 能力ALL rows | 28（20用户/交付 + 8内部质量） |
| ONLY闭包rows | 24 |
| Reuse/port rows | 59 |
| Filter rows | 原21个概念字段；Delivery拆为两条正交轴后为22行（10 course/context + 12 section） |
| Invalid capability/disposition/ownership | 0 |
| Duplicate/missing/unassigned product files | 0 |
| 当前交付未分类token (`P2_UNRESOLVED/TBD/TODO/UNCLEAR`) | 0 |
| secret-like material in P2 artifacts | 0 |

复算命令：

```powershell
& project-governance/current/p2/tools/generate-file-universe.ps1
& project-governance/current/p2/tools/validate-p2.ps1
```

最终一次验证输出：

```text
file_rows=158
unique_paths=158
mixed_files=38
semantic_rows=91
capability_rows=28
only_rows=24
reuse_rows=59
filter_rows=22
failures=0
```

## 3. Dirty worktree保护

P2启动前已经存在：

- 24个tracked删除；
- 1个tracked修改（禁读旧P1文件）；
- 多组既有untracked/ignored历史、workflow、chat、runtime、vendor、release、secret和worktree表面。

P2没有回滚、覆盖、恢复或清理它们。158个产品拥有文件的当前hash全部仍与P2冻结矩阵一致，因此本轮产品源码写入为0。

本轮新增/修改只位于：

- `project-governance/current/p2/**`
- 当前权威workflow中的P2状态记录

## 4. 禁区与私有信息核验

- 没有打开、读取、搜索正文或引用`docs/deliverable-a-windows-local-release-requirements.md`。
- 没有打开、读取、搜索正文或引用`docs/p1-a-recovery/**`。
- 没有读取`.ngagent/**`或NGAT/Organ旧P1派生物正文。
- 没有使用废弃workflow或chat-log替代当前权威输入。
- `.secrets/**`、private inventory、ignored secret-bearing config仅做根路径/ignore/package风险登记；没有读取、hash、显示或复制正文/子项。
- P2治理产物的secret-like pattern检查为0。

## 5. ALL结论

P2已把本地一键包需求映射成经用户批准、可计划、可测试的基线链；产品裁决与P2 Review已闭合，真实字段映射仍必须通过§9证据门：

1. 自包含Windows一键启动；
2. 共享React产品壳和完整状态；
3. catalog与Open双刷新：local为10分钟(1–1440分钟)/1秒(1–3600秒)，public固定10分钟/1秒；Open按批准catalog/service scope运行、无watch仍刷新；attempt/valid结果分层、时间截点和分类计数；
4. 10个context/course筛选维度；
5. 原历史/当前候选共有21个概念字段；Delivery一项拆为两条正交轴，因此contract表为22行（10 course/context + 12 section）。modality包含On Campus/In Person、Online、Hybrid、Other、Unknown；synchronicity包含Sync、Async、Mixed、Unspecified、Unknown；
6. 三值结果、同section见证、逐日多availability windows；
7. course-centered匹配sections和other/mismatch reasons；
8. 独立section搜索、直接URL、完整详情；
9. 最多9项selection、live WS watch和断线清理；
10. 新Open episode的toast/alert；ONE_SHOT的`Max audible notifications`默认3、无产品上限，达限只静默不停止watch；
11. WebUI通用声音：ONE_SHOT逐Open observation至cap，CONTINUOUS按Closed→Open episode、10分钟/Unlimited和用户确认；
12. 可靠Open状态/聚合、新鲜度、`en-US`/`zh-CN`与包级用户状态差异；
13. 本地非active偏好及Open episode/watch history持久化与Reset；公网每个新页面current state恢复默认且无Saved views；
14. 本地Saved views：`REQUIRED / LOCAL_ONLY`、versioned FilterSchema、完整CRUD/dirty/incompatible与local persistence；普通筛选Reset保留library，delete-all保留当前filters，只有带确认的本地用户数据Reset清library；公网入口/API/storage/bundle表面零存在，并且两包都不恢复Share/URL filters；
15. migrations、安全、真实数据证据门、测试、文档和正向package allowlist。

每项都映射到UI、API/protocol、query/data/schema、worker/runtime、config、tests、docs、startup/package。旧实现没有完成的部分统一标为`MAPPED_GAP`，没有用stub、type、schema、fixture或旧文档冒充完成。

这里的“映射为可计划候选链”不等于“筛选已由当前实现或真实数据完全证明”。旧历史中的多数筛选只有部分链路，当前`/api/sections`仍是stub；因此P3/P4冻结设计前必须通过§9真实数据门。

## 6. ONLY结论

以下当前旧表面已形成完整删除/隔离闭包，而不是只隐藏按钮：

- email/SMTP/SendGrid/mail config/UI/API/worker/templates/tests/docs/deps/launcher/schema/package；
- persistent personal active subscription/contact/token/DB fanout；
- HTTP local claim；
- 旧通用Closed→Open/3分钟bucket与last_notified；但必须保护CONTINUOUS的per-section Open episode状态机；
- macOS当前支持与`.command`入口；
- Calendar当前runtime；
- quiet hours/snooze/paused/suppressed与旧personal notification history；新LOCAL_ONLY episode/watch history明确保留；
- Node/Fastify/npm/Vite dev作为最终用户runtime；
- old auto-refresh toggle/45秒无界cache；新的双refresh contract明确保留；
- named Compact view、Share links、Waitlist filter/alert；
- legacy `keywords/tags` no-op preset control/state/query/URL keys；保护raw course tags provenance与仍有消费者的generic `TagChip`；
- Saved views旧Share/URL restore/auto-apply/stub机制排除；新Saved views能力明确为本地包`REQUIRED / LOCAL_ONLY`，公网完整排除；
- 假fallback数据、Playground/orphan/dead状态；
- internal probes/tests/reports/runtime/vendor/private/old archives进入用户包。

Discord、Web Push、native App和system notifications在当前允许源码范围保持零实现表面。

Linux部署脚本/运维意图明确标为`PUBLIC_DELTA / CARRY_TO_P4`，没有被误删，也没有混入本地包。

## 7. 复用结论

### 高价值保留/移植

- React bootstrap、HTTP adapter、FilterPanel、course card、`en-US`/`zh-CN` i18n、UI utility；
- 旧UI的sticky筛选栏、active chips、Reset、命名preset/modified交互意图；不存在可直接复用的PresetManager CRUD/storage；
- query参数绑定、排序、same-section EXISTS骨架、dynamic dictionary；
- Rutgers client、normalizer、stable hash、target-scoped ingest思路；normalizer只移植抽取算法，不继承错误Delivery分类；
- migration命名/checksum/transaction；
- SQLite course/section/meeting/core/status/FTS模型的健康部分；
- poller target coalescing、miss/resilience、metrics/checkpoint思路；
- course/health/poller tests和field samples中的稳定fixtures；
- `.bat`薄入口、launcher的path/error/open-browser/shutdown教训。

### 必须重写或删除

- `/api/sections`空stub；
- meeting time EXISTS错误、client后分页过滤、TBA丢失、FTS断链；
- persistent subscriptions、claim、mail与安全漏洞；
- poller 3分钟dedupe/DB fanout/stop-on-cap；CONTINUOUS episode按新contract重写；
- destructive full-init和Node child-process runner；
- self-contained Rust runtime、WebSocket、section详情、正式UI、双refresh observations/counters、本地history/reset和build/package链。

所有reuse项有目标消费者、缺陷、修复和验证；所有rewrite项有具体行为/安全/交付理由。没有整项目盲目重写或盲目照搬。

## 8. 2026-07-13 P2 Review回复与当前裁决

| # | 用户回复 | 已纳入的当前裁决 | Review状态 |
|---:|---|---|---|
| 1 | 询问当前筛选是否完全满足旧历史，特别是online/hybrid/on-campus | **不能证明完全满足。** 原21个概念字段因Delivery拆两轴成为22行候选contract，不是当前实现完成度。历史有效维度大体已命名，但多数链路为partial/data-only/stub；只排除legacy keywords/tags no-op preset表面，保护raw/generic组件。Delivery保留raw；精确映射等§9真实数据门 | `APPROVED WITH REAL-DATA GATE` |
| 2 | 批准三值结果 | `MATCH / UNCERTAIN / NO_MATCH`；缺失/TBA不算确定匹配 | `APPROVED` |
| 3 | 接受availability | 逐星期多窗口；全部required known occurrences必须完整落窗 | `APPROVED` |
| 4 | 批准`(term,campus,index)`，并要求后续拉真实数据 | 保持复合key；即使样本index唯一也不降级；真实collision/open join进入§9 | `APPROVED` |
| 5 | 明确不做Share links、Waitlist、Compact view；后续明确“Saved views做”，再裁决只在本地提供 | 前三者`EXCLUDED`；Saved views为`REQUIRED / LOCAL_ONLY`，definitions跨本地启动并由Reset清除，无URL/share/cloud sync；公网入口/API/storage/bundle表面全部排除 | `APPROVED / REVISED` |
| 6 | quiet/snooze等排除；公网每次打开全新；本地persistent history+Reset | public每个top-level document load不持久current filters/selection/settings/subscriptions/history，language按system，且无Saved views。local持久non-active prefs/history/Saved views；Reset停止watch并清用户数据但不删catalog/app | `APPROVED / REVISED` |
| 7 | Max无上限、默认3、达限只静默，改名Max audible notifications | 仅ONE_SHOT；任意正整数、无产品上限；成功开始cue计数；达限不停止watch/toast/history；显式watch start或“恢复声音/重置计数”归零 | `APPROVED / REVISED` |
| 8 | WebUI通用声音；持续模式用闹钟确认与Closed→Open重触发 | CONTINUOUS默认10分钟、可Unlimited；per-section episode + shared mixer；A确认后同episode不再响，必须Closed→Open；C未确认则继续，新D立即响；支持逐项/全部确认与resume | `APPROVED / REVISED` |
| 9 | 本地双interval可配，公网固定；每次刷新给新结果、时间截点和计数 | local catalog 10m/1–1440m、Open 1s/1–3600s；public固定10m/1s；每attempt新RefreshObservation、valid/safe pull新OpenObservation、failure保留last-known；课程/Open时间截点；public today、local run+today的attempted/succeeded/failed/empty计数 | `APPROVED / REVISED` |
| 10 | 同意旧非目标清理，但必须保留I18N且至少EN/CN | mail/macOS/current Calendar/persistent active/HTTP claim等旧链继续清除；I18N明确保护并固定至少`en-US`与`zh-CN`，Rutgers raw保持原文 | `APPROVED / REVISED` |

Saved views的旧UI证据已复核：历史设计有Save view、PresetManager和unsaved preset概念，真实源码只有active chips、Reset、sticky filter栏与未消费的manual `dirtyFields`，没有任何PresetManager CRUD或storage实现。因此只把信息架构和交互意图用于本地包，不复用假闭环；公网明确不提供该能力。该裁决已随P2整体批准。

## 9. 真实Rutgers数据硬门与后续设计边界

用户明确要求：没有具体课程/Open数据时不得凭猜测冻结产品事实。因此本轮P2不发网络请求，也不把历史样本冒充当前数据；但以下门已成为P3/P4计划的强制前置输入。

### 9.1 P3冻结本地数据/筛选设计前

执行受控、只读、串行、低请求量的课程数据reality check：

- 开始前冻结request manifest：endpoint、当期/相邻已发布term × 已批准交付范围campus、请求硬上限、串行间隔、重试上限与停止条件；扩大scope必须回Review。不永久硬编码term ID，也不按subject重复下载已证明为全量的payload。
- 每次请求记录scope、完整参数、开始/完成时间、HTTP status、RTT、bytes、response header（存在时）、body SHA-256与重试/等待；完整raw放ignored evidence区。
- 独立profile raw meeting mode code/description × day/time × physical location，冻结Delivery modality/synchronicity映射；generic Online无法区分同步性时必须是`ONLINE + UNSPECIFIED`。
- 核验H/TH/MTH/TTH、invalid/TBA、多meeting、optional/exam/special requiredness、FTS真实ingest、structured instructor、permission、eligibility/major/minor/honors与unknown语义。
- 计算catalog内裸index、`(term,index)`、`(term,campus,index)`碰撞并冻结join contract；不在P3额外拉`openSections`冒充P4证据。

### 9.2 P4冻结Open轮询/公网设计前

- 复用P3已缓存/取证的catalog，不重复下载；对同scope受控采样`openSections`并验证每个index恰好join一个section。HTTP 200空数组、malformed/HTML、timeout、429、5xx只可来自自然观察或本地fixture/injection；不得发送无效请求、burst或压力流量主动制造。未观察到则标`NOT_OBSERVED`。
- 重复采样只能表述为`OBSERVED_ONCE / OBSERVED_REPEATED / OBSERVED_MULTI_SCOPE / INFERRED / NOT_OBSERVED`，`NOT_OBSERVED`不得写成“不存在”。
- 分开测量上游采样间隔、HTTP RTT、normalize/DB、fanout与browser-to-audio延迟；不得把poll interval或HTTP RTT冒充端到端1秒。
- 低量串行样本不能证明持续1秒轮询容量安全；P4必须给出按target数计算的QPS上界、single-flight/coalescing/cooldown规则和P7容量验证方案。未证实时标`UNKNOWN`并回Review。
- 真实证据如与public固定1秒/10分钟、字段contract或上游安全约束冲突，立即停止并回到共同Review，不能静默改值。

### 9.3 通过产物

至少输出request manifest/raw hashes、field profile、Delivery oracle、section-key collision/open join矩阵、H/TH与empty/error最小fixtures、refresh/backoff观察表，以及每项`PASS / FAIL / UNKNOWN`。P3可在课程门后冻结课程/筛选设计，但Open/poller/join/error/QPS仅为provisional；P4的Open证据作为`BASELINE_SHARED`输入在P5回流两包，最终到P6才冻结。字段映射、join/错误保护或请求预算不足时不得伪装通过。

Rust具体crate/模块、正式UI视觉、生产容量、clean Windows/browser、systemd/Caddy/HTTPS与artifact audit仍按P3/P4/P7处理；它们已有阶段归属，不授权本轮实施。

## 10. 批准结论与阶段边界

P2原审计、2026-07-13 Review修订及Saved views仅本地包的最终裁决均已同步并通过机械验证。用户随后明确回复“批准P2”，因此P2 Review门已经通过，P2正式关闭。

该批准的边界是：

- 批准本地一键包ALL/ONLY基线、复用/重写/删除裁决及明确公网delta标记；
- 不代表P2审计结论已经实现，也不批准产品源码修改、构建、打包、Release或服务器操作；
- P3现在具备启动资格，但必须由用户另行明确启动，不能由P2批准自动触发；
- P3/P4仍须执行§9真实数据门；证据冲突时必须停止并回到共同Review。

在P3获得明确启动前，主线保持在“P2已关闭、P3未启动”状态。
