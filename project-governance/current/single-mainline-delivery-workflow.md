# Rutgers Better Course Schedule Planner

## 单主线双任务、双包交付工作流

### 文档控制

- **状态**：当前权威工作流基线
- **生效日期**：2026-07-12
- **最后修订**：2026-07-13（纳入`P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001`：Windows本地包以exe目录为锚使用包内相对单库；该修订不授权P7）
- **权威工作线**：当前 Codex 主线
- **最终交付任务**：`公网包以及部署`、`本地一键包`
- **最终包数量**：严格为两个——`公网包`、`本地一键包`
- **适用范围**：从 0A 产品定义开始，经过 P1-P7，直到公网生产部署完成

本文件只定义当前工作流、产品基线、阶段产物和审核门。它不自动宣告 P1-P7 中任何阶段已经完成；阶段状态必须由当前主线依据本文件重新建立，并在规定的审核门获得用户确认。

## 1. 废弃边界与权威性

此前会话流程中生成的工作流文件、P1 文件、NGAT/Organ 产物及其修订版本全部视为废弃，不得再充当当前工作流、阶段完成状态或产品要求的权威来源。

### 1.1 禁止阅读的旧 P1 执行产物

以下旧 P1 执行产物不得打开、阅读、搜索其内容、引用、总结、使用，亦不得把它们当作定位结论或发现一手来源的捷径：

- `docs/deliverable-a-windows-local-release-requirements.md`
- `docs/p1-a-recovery/`
- `.ngagent/` 中与旧 P1 流程有关的任务、报告、worktree 和状态
- NGAT/Organ 根据旧 P1 结果继续生成的计划、校验报告或派生物

这里的禁止范围针对旧 P1 执行所产生的二手材料，不禁止未来 P1 独立调查旧项目代码、Git/GitHub、task-015、原始历史记录和其他一手来源。

### 1.2 其他废弃流程产物

其他废弃流程产物包括但不限于：

- `docs/dual-delivery-workflow.md`
- `docs/public-web-target.md`
- `docs/deployment-platform-decision.md`
- `docs/shared-rust-architecture-decision.md`

这些文件可以原样保留作为历史现场，但不再具有规范性。必须遵守以下规则：

1. 不得覆盖本文件。
2. 不得用于证明当前 P1 或任何后续阶段已经完成。
3. 不得作为跳过重新调查、重新验证或用户审核的捷径。
4. 如果未来需要核验某项历史事实，应直接回到独立的一手来源，而不是继承废弃流程文件的结论。
5. 本文件不依赖任何废弃产物才能解释或执行。

## 2. 两个交付任务与两个包

为避免交付任务名称与阶段编号 `0A`、`0B` 混淆，不再使用单字母产品别名。

### 2.1 公网包以及部署

这是第一条最终交付任务线，包含两个彼此分开的步骤：

1. **公网包**：P7 产出的、面向 Linux 服务器的经过验证的部署包。
2. **部署**：P7 结束后，使用公网包完成真实服务器加固、安装、HTTPS 配置、上线和生产验证。

“公网包”是两个最终包中的一个；“部署”是对真实生产环境的变更，不是第三个包。二者共同构成第一条最终交付任务“公网包以及部署”，但生产部署不得被偷偷并入 P7。

### 2.2 本地一键包

这是第二条最终交付任务线，产出两个最终包中的另一个。它面向普通 Windows 用户：

- 用户下载并解压；
- 通过 `.bat` 一键启动；
- 本地服务、数据库、课程数据处理、状态监控和提醒链均在用户电脑运行；
- 唯一live database固定为`<package-root>/data/rbcsp.sqlite`，其中`<package-root>`由`RBCSP.exe`自身位置解析，不取决于CWD且没有其他目录fallback；
- 最终archive不预装数据库或真实Catalog/Open数据；首次运行先建立schema-only数据库，再从真实Rutgers获取数据；
- 升级保留`data/`并在migration前完整备份整个数据库；本地用户Reset只清PERSONAL logical tables；删除整个解压目录即删除程序和全部本地数据；
- 普通用户不需要理解 GitHub、Node、Rust、数据库或服务器部署。

以上存储规则引用`P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001`。它只适用于Windows本地一键包；公网包继续使用其Linux service-state adapter。该决定已纳入P6 Review材料，但P6 Review尚未最终批准，`P7 NOT AUTHORIZED`。

### 2.3 最终包数量

整个工作流最终严格产出两个包：

1. **公网包**：面向 Linux 服务器的部署包。
2. **本地一键包**：面向 Windows 用户的 release archive。

生产部署、GitHub Release、文档、审计报告和私有 inventory 都不是第三个包。

两个最终包均不得携带预装真实课程/Open数据。Windows archive还不得携带任何DB/WAL/SHM、数据库seed或backup；`data/rbcsp.sqlite`只能由最终用户首次运行时创建。

### 2.4 阶段编号

`0A`、`0B`、`0C`、`P1` 至 `P7` 只表示流程阶段，不表示产品名称。

## 3. 单主线治理模型

当前只有一条权威工作线：本对话所在的 Codex 主线。

### 3.1 Codex 执行模式

Codex 负责：

- 调查与读取一手材料；
- 设计、实现、测试和验证；
- 编写当前阶段的落盘产物；
- 在需要时派发范围明确的 subagent；
- 整合并独立复核 subagent 结果；
- 保护用户原有工作树和未提交改动；
- 遵守阶段输入、输出和审核门；
- 在达到硬停点后停止，不自动进入下一阶段。

### 3.2 共同审核/讨论模式

用户与 Codex 共同负责：

- 产品目标和边界裁决；
- 冲突、取舍和未决问题；
- 阶段产物审核；
- 批准、拒绝或要求修改；
- P1 Review、P2 Review 与 P6 Review；
- 发布判断；
- 真实生产部署和最终上线判断。

### 3.3 Subagent 边界

Subagent 可以承担独立、有限、可验证的执行任务，但：

- 不能形成第二条权威工作线；
- 不能替用户作产品裁决；
- 不能改变 Phase 目标或产物；
- 不能自行进入下一阶段；
- 不能绕过 P1 Review、P2 Review、P6 Review 或 P7 内部顺序门；
- 不能把局部结果直接当作主线结论；
- 所有结果必须由主线 Codex 整合和复核。

### 3.4 阶段推进原则

1. 先确认阶段输入，再执行阶段工作。
2. 每个阶段必须形成可读、可审计、可验证的落盘产物。
3. P1、P2 和 P6 完成后是强制硬停点。
4. P3-P5 没有另行设定强制联合审核门，但不得越过输入依赖；出现需要用户裁决的问题时必须暂停。
5. 任何审核通过都只批准其明确范围，不自动批准后续阶段。
6. 服务器已经存在、代码看似可运行或旧产物已经生成，都不能成为跳过阶段的理由。

## 4. 当前产品与架构基线

本节把已经由用户明确确认的决定重新写入当前权威文件，不继承旧产物的权威性。

### 4.1 普通用户体验

BCSP 的产品定位是优化 Rutgers CSP 的使用体验，而不是替代 CSP。最初且持续成立的核心痛点是：原 CSP 的筛选条件不足、定位目标课程效率低、使用体验和性能不足；BCSP 应以低门槛服务尽可能多的普通学生。

两条交付任务线所对应的产品共享同一套普通用户 React WebUI 和核心体验，包括：

- 课程搜索；
- 多条件筛选；
- 课程与 section 信息；
- open/closed 状态；
- 按 section 订阅；
- WebUI 声音提醒；
- 一致的页面结构、主要交互，以及至少`en-US`与`zh-CN`两套完整产品语言。

默认信息架构必须以 course 为中心：用户先找到课程，例如 CS 111，再展开课程卡片查看其 Section 01、02 等。与此同时，section 必须支持独立搜索、独立访问；点击 section 后进入可直接访问的完整详情页，不能只依附在 course 预览中。

meeting 日期/时间筛选表达的是“用户可以上课的时间”。以整个 section 为判断单位：section 的每一个具有明确星期和起止时间的必修 meeting occurrence，都必须完整落入同星期的某个用户可用窗口；只命中一条 meeting 或仅与窗口部分相交都不算匹配。异步、TBA/unknown、hybrid、optional meeting、exam 和特殊日期按P2 `06-filter-section-watch-contract.md`的三值/occurrence规则处理，并必须由真实Rutgers数据门校验raw映射。

多条件筛选的组合语义已经确认：不同筛选维度之间默认使用 `AND`；同一维度多选默认使用 `OR`，只有 core 等明确提供 `ANY / ALL` 模式的维度才能改变该规则。Course 只有在自身满足全部 course 级条件，并且存在同一个 section 同时满足全部 section 级条件时才匹配；不得由不同 sections 分别满足不同条件后拼成虚假的精确结果。独立 section 搜索必须复用同一 section predicate。

Course 卡片默认只展开真正满足全部条件的 sections；用户可以显式查看其他 sections，并看到不匹配原因。TBA/unknown 不算“确定匹配”，必须进入单独的“不确定候选”结果面，不能静默混入匹配结果或静默消失。Delivery以modality与synchronicity两轴表达；generic Online可保留为`ONLINE + UNSPECIFIED`，不得被静默丢弃或伪装成同步/异步。

Calendar 周课程表是用户过去明确要求过、但旧项目没有真正完成的历史能力。当前核心版本不包含 Calendar；它只作为 future feature 保留。BCSP 不因 Calendar 而扩张为 Rutgers CSP 的替代品。

运行位置和部署管理能力可以不同，但不能形成两套普通用户产品或两个独立 UI。

### 4.2 邮件与通知边界

- 两条交付任务线所对应的产品当前版本都不包含邮件提醒。
- 公网包以及部署不包含 mail config。
- SMTP、SendGrid、邮件设置 UI、邮件 worker、邮件配置文档和邮件凭据均不进入当前交付面。
- 邮件提醒只作为 GitHub future feature 记录。
- 当前不增加 Web Push、原生 App 或系统通知等另一套通知产品。
- 声音提醒只要求 WebUI 打开并正常运行时可靠工作。

### 4.3 Section 订阅

- 订阅对象是 section。
- 单个浏览器会话最多同时订阅 9 个 section。
- UI 必须提供“开始订阅 / 关闭订阅”开关。
- active watch 只存在于当前 live connection 和服务内存中。
- 页面关闭、断网、连接超时或浏览器被挂起后，active watch 自动失效或不再保证提醒。
- 服务端不需要持久保存个人 active subscription。
- 当前外部section key采用`(term,campus,index)`；后续真实数据必须验证碰撞与openSections join，即使样本中裸index唯一也不得静默降级。
- 当前产品必须具有明确的 subscription management 用户界面和流程，使用户能够查看、管理和停止所选 section 与 active watch；“active 不持久化”不等于“没有订阅管理”。
- 公网包每次top-level document load（首次打开、新tab、reload）都建立新用户session；当前filters、selected sections、volume、sound mode/duration、Max audible设置、history与active全部恢复默认，language每次按浏览器/系统初始化且不持久。公网不提供Saved views：无入口、definition、storage、API或隐式URL替代。
- 本地一键包持久保存非active filters、selected sections、language/音频/Max audible/refresh偏好与Open episode/watch history；但active watch、connection、已用audible计数和未确认alarm永不跨页面/启动恢复。
- 本地WebUI必须提供“重置本地用户数据”：确认后先停止全部active watches，再清除上述用户偏好、selection、Saved views、history和计数，恢复公网默认；不删除catalog、应用二进制或服务运行配置。
- WebUI toast/alert与`Max audible notifications`是subscription management能力。Max audible仅用于ONE_SHOT，默认3、接受任意正整数且无产品上限；达到上限后只将该section转为静默提醒，watch、toast、状态和history继续。显式重新start watch或“恢复声音/重置计数”使计数归零。

#### 4.3.1 Saved views

- 用户已明确Saved views进入当前核心版本，并最终裁决为本地一键包`REQUIRED / LOCAL_ONLY`；公网包不提供，它不是future。
- 旧UI只提供Save/PresetManager设计、active chips、Reset、sticky filter栏和未消费`dirtyFields`骨架，没有真实CRUD/storage；只抽取信息架构与交互意图，不能声称旧功能完成。
- View保存带revision的versioned canonical filter snapshot，包含term/campus和所有注册筛选；当前不保存sort、page、result/cache、selected/watch、audio、refresh、language或history。Apply后从page 1查询；rename/update/delete使用revision/CAS或等价冲突保护。
- 必须支持save、apply、rename、explicit update/overwrite、duplicate、delete、delete-all，以及clean/modified/incompatible状态；不得静默覆盖、淘汰或丢弃unknown/失效字段。普通“清除当前筛选”不得删除Saved definitions；delete-all只删library而保留当前filters；只有带确认的本地用户数据Reset清除local library。
- 本地definitions存本地用户数据store并由Reset清除。公网source graph、DOM、route、API、browser storage、i18n catalog和compiled bundle均不得出现Saved views产品表面。
- Saved views不恢复Share links、filter URL state、账号/cloud sync、自动default view或导入/导出。
- 使用单一versioned `FilterSchema`/stable field registry驱动两包共享filter UI、query与chips；本地Saved views codec/migration/tests从同一registry派生，使未来新增筛选字段无需维护第二份preset字段清单，同时不把本地manager/codec编入公网制品。

### 4.4 声音提醒语义

- UI 必须支持音量调整。
- UI 必须支持“一声提醒 / 持续提醒”切换。
- WebUI只发通用提醒声；用户在UI的alert/section状态中查看是哪门课Open，不引入语音、邮件、系统通知或另一套通知链。
- 每次Open attempt都产生新的`RefreshObservation`；只有valid且安全reconcile的pull才产生per-section `OpenObservation`，即使状态没有变化也要新建。Failure保留last-known status且不转换episode；声音再区分`OpenEpisode`与`AudibleNotification`。
- ONE_SHOT：section持续Open时，每条新的Open observation尝试播放一次有限cue，直到该section达到Max audible；达到后只静默，不停止watch。
- CONTINUOUS：使用闹钟确认语义，默认持续10分钟并可设置为Unlimited。watch开始后首次Open或可靠`Closed -> Open`创建per-section episode；同一episode持续Open只更新时间/次数。用户确认/关闭A当前episode后，A持续Open不再重新响，必须先Closed后再Open；C未确认时共享alarm继续，新D Open时立即响。
- CONTINUOUS支持逐section确认、全部确认、超时后显式resume；Max audible不适用于此模式。共享mixer不得为多个sections叠加无界音轨。
- 页面关闭、手机锁屏或浏览器被操作系统暂停时，不保证继续播放。

### 4.5 数据刷新与实时目标

- 课程信息与Open信息是两种独立上游刷新；browser query revalidation只是课程信息成功后的消费行为，不是第三种上游刷新。
- 本地一键包：课程信息默认10分钟，允许`1–1440`分钟；普通Open刷新默认30秒，允许`3–3600`秒。
- 公网包以及部署：课程信息固定10分钟、普通Open刷新固定30秒，普通用户不可修改，也不能通过页面reload放大成Rutgers上游请求。
- Open `1秒`默认/固定值已被2026-07-13 Review明确替代。双时钟合同为：public普通刷新固定30秒；local普通刷新默认30秒、范围3–3600秒；active watch相关共享batch目标10秒；实际间隔为`min(general,10)`并保持per-batch single-flight。详见P3 `18`–`20`。
- 每次target刷新/pull都必须给出新的可观察结果，即使payload和状态未变化；课程信息与Open信息分别显示最近成功时间截点，最新失败必须另显失败时间/状态。
- 公网显示当前Rutgers自然日的Open pull `attempted/succeeded/failed`；本地同时显示本次运行与今天的计数。计数按上游target请求，不按section、browser或WebSocket fanout；“今天”按`America/New_York`并明确标注。
- 旧15–120秒普通用户auto-refresh toggle与45秒无界cache排除，不得覆盖本规则。
- BCSP接受新的valid Open observation后，到WebSocket fanout与开始WebUI声音的工程目标是1秒以内。
- Open poll interval、Rutgers端点快照可见时间与“BCSP首次观察到状态后到用户听到声音”的延迟是不同指标；必须分别测上游采样、HTTP/batch、normalize/DB、fanout与browser audio。当前只把valid observation→fanout/audio的目标冻结为1秒以内，不能承诺Rutgers真实状态变化→通知严格30秒以内。
- 若真实数据/限流证据不能证明固定值安全，必须在P3停止并回到共同Review，不得静默改变默认、范围或公网策略。P7只验证获批合同的实现容量，P4不能替P3补做共享上游合同裁决。

### 4.6 公网实时架构

- 公网服务器维护统一课程/section 数据库。
- 公网服务器集中刷新课程目录。
- 公网服务器集中轮询 Rutgers `openSections`。
- Open poll target由已批准catalog/service scope决定；没有active watch时也必须刷新供搜索、筛选、course card和section详情使用。Active watch可把相关共享batch从general cadence提升至10秒；同一batch保持single-flight，用户数或watch数不会进一步增加Rutgers请求。
- 手机和电脑浏览器只访问本服务，不得各自直接轮询 Rutgers。
- 浏览器通过 WebSocket 建立 active watch。
- 服务端以内存映射维护 section 与当前在线连接。
- 连接断开或心跳超时后移除。
- 多个用户关注同一 section 时，Rutgers 侧仍由服务器集中获取状态，再分发给在线连接。

### 4.7 部署平台基线

- 公网包以及部署的目标环境是 Vultr EWR Linux VM。
- 记录中的初始规格为 `$6/月` AMD High Performance、Ubuntu 24.04。
- 当前本地一键包只支持 Windows；macOS 不进入当前版本或当前交付任务。历史上曾以 Windows/macOS 为支持目标，但缺少真实 Mac 测试设备，并至少收到过一次旧 `.command` 在 Mac 上启动失败的真实报告；若未来重新支持 macOS，必须由用户另行决定并具备真实设备验证，不能沿用旧 `.command` 宣称已支持。
- OCI 已退出当前主路径。
- Sites、纯 Cloudflare Serverless 和 Cloud Run 不作为实时后端。
- Cloudflare可以在后续用于 DNS、CDN 或代理，但不是主计算平台。
- 域名、Cloudflare DNS/代理方式和生产服务器实际状态必须在最终部署前重新核验。
- 任何历史服务器状态都不能直接视为当前事实。

### 4.8 共享 Rust 架构

目标是模块化单体：

- 一套共享 React WebUI；
- 一套 Rust 共享核心；
- Windows 入口：`bcsp-local.exe`；
- Linux 入口：`bcsp-server`；
- SQLite；
- 集中 poller；
- WebSocket；
- Linux 生产侧使用 systemd 与 Caddy；
- 现有 Node/Fastify 代码是迁移和行为参考，不是最终后端目标。

### 4.9 凭据与敏感信息

- SSH 私钥、token、服务器清单和云平台凭据只保存在本地或平台 secret/env。
- 敏感信息不得进入 Git、公开文档、本地一键包或公网包。
- 精确 IP、UUID、key fingerprint 等私有 inventory 必须位于明确被 Git 忽略的位置。
- 发布和部署前必须执行 secret scan 与 artifact audit。

### 4.10 真实 Rutgers 数据证据原则

- 旧schema、type、stub、文档或历史样本只能证明候选字段/风险，不能证明当前Rutgers raw值、完整性或实时行为。
- P3冻结完整本地一键包计划前，必须以受控、只读、串行、低请求量方式核验届时有效term与批准scope的课程数据及同scope `openSections`/join；P4完整复用这套共享证据，不重复下载。
- 证据至少覆盖raw字段profile与缺失率、Delivery modality/synchronicity映射、H/TH和时间、section key碰撞/open join、FTS、instructor、permission、eligibility以及empty/error/429/5xx区分。
- 每阶段开始前冻结request manifest（endpoint、term×批准campus scope、请求硬上限、串行间隔、重试上限、停止条件）；扩大scope回Review。每次采样记录参数、时间、HTTP结果、payload hash和provenance；完整raw进入ignored evidence，仓库只保留最小去敏fixture/hash/manifest。
- 429/5xx/HTML/timeout只可自然观察或用本地fixture/injection验证；禁止发送无效、突发或压力请求主动制造。P3的Open低量样本不证明3/10/30秒调度的持续容量或端到端新鲜度，必须另给QPS上界、single-flight/coalescing/cooldown与P7验证方案。
- 结论区分`OBSERVED_ONCE / OBSERVED_REPEATED / OBSERVED_MULTI_SCOPE / INFERRED / NOT_OBSERVED`；未观察到不能写成不存在，单次样本不能外推全校/全term。
- 真实证据与当前contract冲突时必须停止并回到共同Review，不得静默改字段、映射、刷新值或安全规则。

## 5. 从 0A 到最终部署的阶段总表

| 阶段 | 工作模式 | 目标 | 必需产物 / 停点 |
|---|---|---|---|
| 0A | 共同讨论 | 定义“公网包以及部署”的产品行为，以及两条交付任务线的普通用户一致性 | 当前“公网包以及部署”目标基线 |
| 0B | 共同讨论 | 选择服务器、平台、域名方向和凭据边界 | 当前部署平台决策与私有 inventory 规则 |
| 0C | 共同讨论 | 确定共享 React/Rust 架构和双入口 | 当前共享架构基线 |
| P1 | Codex 执行 | 恢复本地一键包的旧产品记忆，并与当前新决定合并 | P1 候选材料；随后强制停止 |
| P1 Review | 共同审核 | 审核、纠正并批准 P1 结果 | 正式产品目标；未批准不得进入 P2 |
| P2 | Codex 执行 | 对本地一键包执行逐文件、逐语义的根管式 `all and only` 与复用审计 | 可复算文件基线、五张审计矩阵、精确contract、复用途径与零遗漏裁决 |
| P2 Review | 共同审核 | 审核本地基线的ALL/ONLY、复用/重写证据、删除闭包、共享/本地/公网差异和全部未决裁决 | 用户明确批准后才能进入P3 |
| P3 | Codex 执行 | 完成Catalog与共享Open真实证据门，再设计没有Open provisional缺口的本地一键包完整计划 | Catalog/Open证据manifest/profile/容量口径 + 本地一键包实现计划 |
| P4 | Codex 执行 | 继承P3共享基线，只设计公网包及生产部署delta | 公网包以及部署实现计划；不得重复上游取证 |
| P5 | Codex 执行 | 分析两条交付任务线的共享、分化与冲突 | 分化与复用矩阵 |
| P6 | Codex 执行 | 合并 P3-P5，形成最终执行计划 | 最终执行计划；随后强制停止 |
| P6 Review | 共同审核 | 执行前审核范围、风险、UI、提交、发布和部署 | 用户明确批准后才能进入 P7 |
| P7.1 | Codex 执行 | 实现共享核心、双入口、API、数据链路和功能基础 | 可工作的完整功能基础 |
| P7.2 | Codex 执行 | 独立完成正式 UI 设计与实现 | 完整桌面端、移动端及状态 UI |
| P7.3 | Codex 执行 | 独立审计并打磨真实 UI | `Before | After | Why` 审计和打磨结果 |
| P7.4 | Codex 执行 | 集成、确定性验证、构建并冻结候选双包 | 公网包与本地一键包候选及不变hash |
| P7.5 | Codex 执行 | 独立real-world E2E：干净Windows、GitHub Actions Linux、Vultr staging与恢复 | 三环境同hash证据与最终P7 completion record |
| P7 Release | 条件步骤 | 发布经过审计的两个包 | 可选 GitHub Release assets |
| P7 后部署 | 共同执行 | 加固服务器、部署公网包并完成生产验证 | 可公开访问的生产网站 |

## 6. 0A：定义公网包以及部署

### 6.1 目的

先定义“公网包以及部署”提供什么，不让平台免费额度或已有实现反向决定产品。

### 6.2 必须回答的问题

- 普通用户体验与本地一键包如何保持一致；
- 哪些差异只属于运行、部署或管理面；
- 课程数据和 open 状态由哪里维护；
- section watch、WebSocket 和声音提醒如何工作；
- 手机与桌面浏览器的可用边界；
- 刷新目标、实时目标和容量验证要求；
- 明确排除的通知与账号类产品范围。

### 6.3 产物

一份无需依赖旧文件即可说明“公网包以及部署”的目标、用户行为、非目标与验收方向的当前基线。本文件第 4 节已经重新承载其核心决定。

## 7. 0B：选择部署平台与凭据边界

### 7.1 目的

确认“公网包以及部署”运行在哪里，以及注册、购买、域名、SSH 和 secret 如何处理。

### 7.2 当前方向

- Vultr EWR Linux VM；
- Cloudflare只作为可选 DNS/CDN/代理层；
- 公网包面向常驻 Linux 主机，而不是纯 serverless runtime；
- 生产部署前重新核验 VM、账户、域名、费用和安全状态。

### 7.3 产物

- 公开的部署平台决策；
- 被 Git 忽略的私有服务器 inventory；
- 凭据保存边界；
- 域名与 HTTPS 待决项；
- 部署前安全检查项。

## 8. 0C：确定共享架构

### 8.1 目的

在恢复和审计旧功能前，确定两条交付任务线最终共享的架构方向，避免继续围绕旧 Node/Fastify 形态制定长期计划。

### 8.2 产物

共享 React WebUI、Rust 核心、Windows/Linux 双入口、SQLite、集中 poller、WebSocket、systemd/Caddy 的架构基线。

0C 只固定架构方向，不在此阶段实现代码，也不替 P5 提前完成模块边界设计。

## 9. P1：恢复产品记忆

### 9.1 目的

解决项目中断后对旧产品意图、功能、设计和历史状态的记忆丢失，并把旧历史与当前新决定放在同一份可审计材料中。

### 9.2 调查范围

必须独立检查一手来源，例如：

- 当前项目代码和测试；
- README、设计文档和架构记录；
- 本地持久化历史；
- 恢复目录；
- 旧 release；
- Git 与远端 GitHub 历史；
- 被废弃的 task-015 线；
- 必要的原始会话记录；
- 当前主线已经确认的新产品和架构决定。

旧 P1 产物不属于一手来源，必须遵守第 1.1 节的禁止阅读边界；不得打开、阅读、引用或用来代替上述调查。

### 9.3 证据分层

必须区分：

- 用户明确表达；
- Agent/Codex 历史总结；
- 代码、测试或 Git 状态直接证明的事实；
- 基于证据作出的推断；
- 存在冲突或无法确认的内容。

### 9.4 边界

- P1 只恢复、记录和合并，不作 `all and only` 裁决。
- 新决定可以覆盖旧决定，但旧决定仍须保留为可解释历史。
- 不修改产品代码、服务器或远端。
- 不把 P7 的逐 task 远端提交规则套到 P1。

### 9.5 产物与硬停点

P1 应形成产品记忆、能力库存、证据映射、冲突记录和当前目标候选材料。完成后必须停止并进入 P1 Review，不得自动启动 P2。

## 10. P1 Review

用户与 Codex 共同：

- 审核来源覆盖；
- 检查旧能力是否被遗漏或被过度概括；
- 检查新旧决定的覆盖关系；
- 修正推断与事实混淆；
- 决定是否接受为 P2 输入。

未获得用户明确批准时，P1 仍未关闭。

### 10.1 2026-07-12 P1 Review 结果

- 用户明确接受 `SAFE-INC-01` 与 `SAFE-INC-02` 的隔离处置。
- 用户明确批准 P1，并逐项回答产品记忆问题。
- 本次回答新增或澄清的产品决定已经写入第 4 节：产品定位、严格 meeting 可用时间规则、course-centered UI、独立 section 搜索/访问/详情、Calendar 仅作 future、当前 Windows-only、subscription management、WebUI toast 与当时称为Max notifications的能力；该名称和行为已由2026-07-13 P2 Review修订为`Max audible notifications`。
- 用户确认 Discord 是其主动要求删除，理由是降低系统复杂度；该事实保留在获批 P1 产品记忆中，当前仍不恢复 Discord。
- P1 Review 门已经通过，P1 正式关闭。该批准只使 P2 成为下一可执行阶段，不自动启动 P2。

### 10.2 2026-07-12 P2 启动前确认

- 用户确认其心智模型是：本地一键包构成完整产品基线，公网包是在该基线上作受控修改的后续分支。
- 该模型不实现为两个长期漂移的代码树。P2只对本地一键包作ALL and ONLY；P4只定义公网相对本地/共享基线的必要delta；P5再把共享核心、adapter和运行差异正式合并。
- P2发现的公网专属内容必须标为`PUBLIC_DELTA / CARRY_TO_P4`，不得因“不属于本地包”就在P2误判为从整个产品或仓库删除。
- 用户确认第4.1节的筛选组合语义：跨维度AND、同维度多选通常OR、同一个section同时见证全部section条件、course默认只展开匹配sections、TBA/unknown单列为不确定候选。
- P1已经回答“产品应该是什么”；P2必须回答“旧代码的每一部分如何服务该产品”。P2不能把“全部重写”或“尽量照搬”设为默认答案，必须主动寻找可安全复用的实现、算法、contract、数据模型、UI、测试与资产，并对必须重写/删除的部分给出证据。
- 本节在启动前只固定P2输入与方法。用户随后于2026-07-12单独正式启动P2；执行结果和当前停点见第11.9节。

## 11. P2：本地一键包的 `all and only` 审计

### 11.1 目的

依据获批P1结果、当前本地项目、旧release、Git/GitHub和历史状态，对本地一键包执行“根管治疗”式审计：不仅判断功能要或不要，还必须查看本地产品文件与文件内语义，使每个当前需求、现存内容、依赖和打包项都有且只有一个明确去向。

P2同时是复用审计。目标不是预设全部重写，而是识别哪些旧代码可以原样复用、修后复用、拆分抽取、按行为/算法/contract移植，哪些因架构、安全、错误链或产品边界必须重写或删除。复用比例不是目标；可证明地服务获批产品才是目标。

本地一键包是完整产品基线；公网包在后续P4中只定义相对该基线的必要修改。P2不对公网部署面做第二次ALL and ONLY，也不得把公网专属内容误删为“整个产品不要”。

### 11.2 审计语料与启动基线

P2启动时必须先冻结可复算的本地产品语料：

- 根目录产品入口、依赖、配置和启动文件；
- `api/**`、`frontend/**`、`scripts/**`、`workers/**`、`notifications/**`；
- tracked配置样例/模板、schema/migrations、测试、产品文档、构建与打包定义；
- 与本地运行或制品有关的assets、i18n、styles、fixtures、runtime/generated路径；
- 当前tracked/untracked状态、文件count、路径与hash；
- 旧release、允许Git历史和获批P1只作为发现、漂移和意图证据，不能替代当前本地文件全集。

P2必须保护既有dirty worktree。ignored/private配置只登记路径、ignore状态和是否可能进入打包链，不读取、显示或复制secret正文。

### 11.3 三轴裁决

每项能力和现存内容必须同时得到三轴结论：

1. **能力结论**：`REQUIRED / EXCLUDED / FUTURE / INTERNAL_ONLY`；
2. **复用/处置结论**：`REUSE_AS_IS / REUSE_WITH_FIXES / EXTRACT / PORT / REWRITE / SPLIT / MERGE / REMOVE / DEFER / OUT_OF_SCOPE`；
3. **交付归属**：`BASELINE_SHARED / LOCAL_ONLY / PUBLIC_DELTA / FUTURE / INTERNAL_TOOLING / HISTORICAL_EVIDENCE`。

“能力保留”不等于“旧文件保留”。同一旧文件也可能同时包含可复用UI/算法/测试和必须移除的旧runtime或邮件路径，因此必须拆分判定。目标架构使用Rust不代表Node时期的查询语义、normalizer、schema、fixtures和tests全部作废；它们可以分别判为`EXTRACT`或`PORT`。`PUBLIC_DELTA`表示不进入本地包、留给P4处理，不等于从产品或仓库删除。

任何`REUSE_AS_IS / REUSE_WITH_FIXES / EXTRACT / PORT`都必须指出目标消费者、已知缺陷、修复边界和验证方式；任何`REWRITE`都必须说明旧实现为什么不能安全复用。不得用整项目“技术栈不同”作为无差别重写理由，也不得用“代码已经存在”作为无差别复用理由。

混合文件不能只给整文件标签；必须继续拆到symbol、route、component、hook、config key、schema object、migration、protocol/event或稳定line anchor，直到每段语义只有一个去向。

### 11.4 `All`

所有应保留能力必须：

- 完整可用；
- 可测试、可验证；
- 文档与实现一致；
- 能进入启动、运行和打包链；
- 不以 stub、假 UI 或过期说明冒充完成。

每项`REQUIRED`能力都必须双向映射到完整链路：用户场景、UI、API/protocol、query/data/schema、worker、配置、测试、文档、启动和package。任何只有历史标签、类型、schema、测试fixture或孤立组件的表面都不能算`All`。

### 11.5 `Only`

不应存在的能力不能只隐藏按钮，还必须审计和处理其专属：

- UI；
- API/route；
- worker；
- 配置与 secret；
- 文档；
- 测试；
- 启动脚本；
- 依赖；
- 打包内容。

还必须覆盖schema/migration、assets、i18n、styles、browser storage、protocol/event、fixtures、observability、runtime state、generated artifacts和直接依赖消费关系。清理时必须保护仍被健康功能复用的共享组件。

删除结论必须区分：`REMOVE_FROM_LOCAL_PACKAGE`、`REMOVE_FROM_CURRENT_PRODUCT`和`REMOVE_FROM_REPOSITORY`。历史证据可以保留在治理/归档区，但不得继续进入当前runtime、启动链或制品。`DEFER/FUTURE`也不得以stub、死代码、假UI或隐藏route残留在当前运行和打包面。

### 11.6 必需产物

P2至少生成五张互相核对的矩阵：

1. **文件全集矩阵**：`path | hash/snapshot | 类型 | tracked/ignored/generated | runtime/package可达性 | 三轴结论 | 依据ID | 后续动作`；
2. **文件内语义矩阵**：`path | symbol/route/component/key/schema object | anchor | 当前行为 | 目标行为 | 三轴结论 | inbound refs | outbound deps | replacement/removal closure | 验收方向`；
3. **能力ALL矩阵**：`需求/CUR ID | 用户场景 | UI | API/protocol | query/data/schema | worker | config | tests | docs | startup | package | 完整性结论`；
4. **ONLY清除矩阵**：`非目标/旧能力 | UI | API | worker | config/secret | schema/data | docs | tests | dependency | startup | package/runtime residue | shared-code保护`。
5. **复用与移植矩阵**：`path/symbol | 可复用价值层（源码/UI/算法/contract/schema/test/asset） | 复用/处置结论 | 目标消费者/位置 | 已知缺陷 | 必要修复 | 不复用理由 | 验证方向`。

筛选必须另有独立contract：每个历史/当前候选字段都记录course级或section级、数据来源和可靠性、操作符、AND/OR/ANY/ALL、同section见证、unknown/TBA策略、结果展示、UI/API/query/test映射，以及保留/删除/重设计/延期结论。不能用“支持多条件筛选”代替逐字段审计。

### 11.7 执行边界

P2是审计，不是实现：

- 不修改、删除、格式化或生成产品源码；
- 不安装依赖，不构建或打包；
- 不运行会写DB、日志、config、checkpoint、dist或runtime state的测试；
- 不设计最终Rust模块边界或P7 task/commit图；
- 不修改远端、Release或服务器；
- 只允许读取、静态引用分析、路径/hash/count核验和不会改变状态的检查。

### 11.8 完成标准与硬停点

P2只有同时满足以下条件才可提交Review：

- in-scope本地产品文件`N/N`已分类，零漏项、零重复、零无去向；
- 所有混合文件完成文件内语义级裁决；
- 每个当前需求都有完整交付链，每个现存runtime/package表面都有当前需求依据；
- 每个直接依赖都有保留消费者或删除结论；
- 每个旧代码单元都有复用/处置结论；所有复用项有目标消费者、缺陷闭包和验证方向，所有重写项有明确理由；
- 每个非目标完成跨UI/API/worker/config/schema/docs/tests/deps/startup/package的传递闭包；
- `DEFER/FUTURE`不进入当前本地runtime或package；
- 当前本地交付范围内没有未获用户裁决的`UNKNOWN`；
- 文件count/hash、覆盖统计和交叉引用可独立复算；
- 公网差异均明确标为`PUBLIC_DELTA / CARRY_TO_P4`，没有被误删或混入本地包。

完成后必须停止在P2 Review。用户批准前不得进入P3；P2批准也不自动启动P3。

### 11.9 2026-07-12 P2执行结果

- 用户明确要求正式启动P2。
- 当前主线依照本节边界完成本地一键包的只读根管式ALL/ONLY与复用审计。
- P2产物位于`project-governance/current/p2/`，包括可复算的158/158文件矩阵、邻接表面清单、文件内语义矩阵、能力ALL矩阵、ONLY闭包矩阵、复用/移植矩阵、筛选/section/watch精确contract和验证停止门。
- P2没有修改产品源码、安装依赖、构建/打包、运行写状态测试、读取禁读旧P1正文、修改远端/Release/服务器或读取私有secret正文。
- P2原审计完成后按要求强制停止在P2 Review；用户已于2026-07-13在全部修订同步后明确批准P2。产品字段、精确contract、复用/重写和删除闭包结论现为获批基线；P3随后已按授权执行并通过。

### 11.10 2026-07-13 P2 Review批准状态

- 用户已回复P2停止门原§8的10项，并在Saved views仅本地包等全部后续修订完成、验证通过后明确回复“批准P2”；状态为`P2 APPROVED — CLOSED`。
- 已批准/修订：三值结果、availability、`(term,campus,index)`、排除Share links/Waitlist/Compact、Saved views为`REQUIRED / LOCAL_ONLY`且公网不提供、public每页面current state全新与local persistence/reset、Max audible、CONTINUOUS episode、双refresh、至少`en-US`/`zh-CN`。
- 对筛选完整性的回答是“尚不能证明”：历史有效维度大体已命名，但当前实现只有少数基础字段闭环，Delivery存在跨normalizer/API/UI漂移；P3/P4冻结前加入第4.10节真实数据硬门。
- Delivery从单一枚举拆为modality与synchronicity；generic Online允许`ONLINE + UNSPECIFIED`，raw code/description/provenance不得丢失。
- local新增无contact、无active恢复的Open episode/watch history、Saved views和Reset；旧personal subscription/history schema仍删除。public current filters/settings/history不持久，也不提供Saved views或definitions。
- Max更名为`Max audible notifications`，仅ONE_SHOT、默认3、任意正整数、达限只静默；CONTINUOUS默认10分钟/可Unlimited并使用Closed→Open闹钟确认状态机。
- 用户后续明确“Saved views做”，再最终裁决只在本地包提供。Git/UI审查确认旧项目从未实现PresetManager CRUD/storage，只有设计与UI骨架；当前按第4.3.1和P2 `06`§7.4重写为共享FilterSchema上的LOCAL_ONLY能力。该裁决已随P2整体批准。

## 12. P3：本地一键包实现计划

基于获批P2设计完整本地计划。P3首先执行第4.10节Catalog与共享Open reality check并形成可审计证据；证据与P2 contract冲突时停止回到Review。课程摄取/筛选与依赖真实`openSections`的poller/join/empty/error/QPS必须在P3一起闭合，不能把本地计划留成大量provisional再由P4/P5回写。计划至少覆盖：

- Rust 本地入口；
- Windows包根相对的单一SQLite：`<package-root>/data/rbcsp.sqlite`，以exe路径而非CWD为锚、零fallback、Rutgers请求前可写fail-fast、OPERATIONAL/PERSONAL逻辑分域、完整备份、升级保留`data/`、Reset只清PERSONAL；
- Rutgers 课程数据摄取、raw provenance、两轴Delivery映射和安全target事务；
- 查询、22行筛选contract、section信息与真实FTS/instructor/permission/eligibility链；
- shared central FilterSchema与LOCAL_ONLY Saved views domain：canonical snapshot/dirty compare/migration/revision-CAS，local persistent store与Reset；
- 两种本地refresh scheduler：catalog 10m(1–1440m)，以及普通Open默认30秒/范围3–3600秒、active-watch相关共享batch目标10秒的双时钟cadence；保留attempt/valid observation、时间截点和run/today分类计数；
- active watch、ONE_SHOT Max audible和CONTINUOUS episode/共享mixer；
- React WebUI、`en-US`/`zh-CN`、本地非active持久化与history/reset；
- `.bat` 一键启动；
- 依赖与错误处理；
- 干净 Windows 环境验证；
- release archive；
- quickstart、troubleshooting 和 release notes。

P3只取证、设计和落盘，不实施产品代码。P3“完整计划”要求Open数据假设已经由P3共享证据闭合；仍属P7容量/实现实测的项目必须有明确安全上界和验证任务，不能以P4回流代替。

## 13. P4：公网包以及部署实现计划

基于0A、0B、0C、获批P2本地ALL and ONLY基线和已闭合Catalog/Open共享证据的P3本地计划设计。P4不得再次请求Catalog或`openSections`，也不得建立第二套Open contract；公网包不是从零开始的第二套产品。P4必须先继承`BASELINE_SHARED`，再逐项列出`PUBLIC_DELTA`及其理由、影响和验证。

P4的必要delta至少覆盖：

- Linux Rust 入口；
- 统一数据库和课程目录刷新；
- 集中 openSections poller；
- public catalog固定10分钟；Open cadence只继承P3最终裁决，不在P4重做安全证据或建立第二套值；并设计每attempt refresh result/counter与每valid pull OpenObservation/成功时间截点；
- 明确不实现public Saved views adapter；公网source graph、DOM、route、API、browser storage、i18n catalog和compiled bundle均验证无该能力，每新页面current filters保持默认；
- WebSocket 连接生命周期；
- active watch 内存模型；
- 每次新页面为ephemeral普通用户session：system language、默认filters/selection/audio/settings且无persistent history；
- 获批Open cadence与端到端目标的分段测量、容量和降级策略；
- 手机与桌面 WebUI；
- systemd、Caddy 与 HTTPS；
- 日志、备份、升级和恢复；
- Linux 公网包；
- 生产部署、验证和回滚说明。

P4 只设计和落盘，不触碰真实生产服务器。

## 14. P5：两条交付任务线的分化与复用分析

形成明确矩阵：

- 共享核心；
- 本地一键包专属：persistent prefs/history、local persistent Saved views store与manager、Reset、本地refresh范围与run/today计数；
- 公网包以及部署专属：ephemeral current page state、无Saved views能力、固定refresh policy与service-wide today计数；
- 冲突；
- 需要 adapter，特别是local user-state/history/Saved views、public capability deny boundary、refresh policy/counter和Windows/Linux entry；
- 必须采用受控adapter或隔离；
- 可共享但配置不同；
- 不得进入任一当前产品的遗留能力。

P5 的结论决定 Rust 模块边界、入口适配、UI 条件能力和测试复用方式。默认目标是单一共享代码基线加明确adapter/entrypoint/config差异，不建立长期漂移的本地/公网代码fork；只有无法共享且有证据支持的部分才能隔离。

P5直接消费P3已冻结的共享Open contract，分析本地与公网adapter/config差异；不得回写一套新的join、empty/error、single-flight、QPS、observation或声音前置语义。若公网delta无法遵守共享contract，停止回Review。

## 15. P6：合并最终执行计划

把 P5 结论分别合入 P3 与 P4，形成：

- 最终本地一键包计划；
- 最终公网包以及部署计划；
- 共享实现顺序；
- 依赖图；
- P7 task 结构；
- 验证与打包顺序；
- UI 两个独立 subphase；
- 提交与远端策略；
- GitHub Release 条件；
- 生产部署边界。

Open/poller共享语义在P3证据门冻结；P4只加公网delta，P5/P6只解决两包adapter、配置、构建与部署冲突，不再制造证据回流循环。

完成后必须停止，进入 P6 Review。

## 16. P6 Review

用户与 Codex 共同审核：

- 产品范围和非目标；
- `P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001`的Windows包根单库、首启无预装数据、完整备份/升级/卸载和零fallback边界；
- 依赖清理风险；
- Rust 与 React 结构；
- task 拆分和依赖；
- P7.2/P7.3 的独立性；
- 测试和容量验证；
- 逐 task 提交策略；
- 两个包的内容；
- GitHub Release；
- 域名、HTTPS 和 Cloudflare 方向；
- 真实生产部署的授权边界。

只有用户明确批准最终执行计划后，当前主线才能进入 P7。

## 17. P7：批准后的实现阶段

P7 是唯一的大规模产品实现阶段，也是允许“每个 task 都形成远端提交”的阶段。

Task 必须是实质、独立、可验证的工作单元。不得为追求数量或贡献图制造空任务、marker commit、重复 remediation 链或未经批准的范围扩张。

### 17.1 P7.1：功能实现

实现：

- Rust 共享核心；
- Windows/Linux 双入口；
- 数据获取与存储，包括Windows包根相对单库与Linux service-state adapter；
- 查询、筛选和 section contract；
- catalog/Open双scheduler、refresh observations/checkpoints/counters；
- openSections集中poller；
- WebSocket；
- active watch；
- ONE_SHOT Max audible与CONTINUOUS Open episode声音状态机；
- shared central FilterSchema、LOCAL_ONLY Saved views domain/local persistent prefs/history/reset与public ephemeral-current无Saved capability边界；
- `en-US`/`zh-CN`完整message catalog；
- React WebUI 功能基础。

P7.1 不能宣称正式 UI 已完成。

### 17.2 P7.2：UI 设计与实现

必须作为独立 subphase，使用：

- `$industrial-brutalist-ui`
- `$design-taste-frontend`

完成同一套普通用户 React WebUI 的：

- 桌面端；
- 移动端；
- 响应式布局；
- 搜索、两轴Delivery等精确筛选、section和订阅流程；
- 仅本地包显示Saved views manager：CRUD/duplicate/apply/modified/incompatible/quota/revision-conflict，且不出现Share/URL restore；公网无入口；
- 双refresh时间截点/计数、本地interval设置与公网固定状态；
- 本地history/Saved views/reset与公网新页面current默认；公网无Saved library；
- 音量、Max audible、静默/恢复声音和一声/持续episode确认控制；
- `en-US`/`zh-CN`的全部状态与locale格式；
- loading、empty、error、disabled 等完整状态；
- 可访问性与交互反馈；
- 正式视觉体系。

### 17.3 P7.3：UI 审计与打磨

必须在 P7.2 已实现、集成并完成视觉验证后，作为另一个独立 subphase 使用：

- `$emil-design-eng`

强制要求：

- 与 P7.2 使用不同 task；
- 不同完成记录；
- 不同提交序列；
- 先形成 `Before | After | Why` 审计；
- 再实施打磨；
- 最后重新进行真实 UI 验证。

### 17.4 P7.4：集成、验证与打包

验证至少覆盖：

- 功能；
- 数据与 API contract；
- 桌面和移动浏览器；
- Windows 一键启动；
- Windows从不同CWD启动仍锚定`<package-root>/data/rbcsp.sqlite`，不可写目录在任何Rutgers请求前fail fast且不产生fallback；
- Windows archive无DB/WAL/SHM/seed/backup和真实Catalog/Open数据，首次运行schema-only后才取数，升级完整备份并保留`data/`，Reset只清PERSONAL tables，删除整个解压目录后无外部残留；
- Linux 服务安装与运行；
- 数据刷新；
- 每次无变化也产生新refresh observation、时间截点和public/local计数；
- WebSocket 与连接清理；
- section watch、ONE_SHOT audible cap不停止watch、CONTINUOUS A/C/D episode/Closed→Open/10m/Unlimited语义；
- public reload/new-tab current filters默认，并验证DOM/route/API/storage/catalog/bundle无Saved views；local prefs/history/Saved views跨启动和Reset scope；普通filter Reset保库、library delete-all保当前filters、local user-data Reset才清库；仅local验证revision/CAS冲突不静默覆盖；
- `en-US`/`zh-CN` key parity、system detection、raw Rutgers原文；
- 安全；
- 容量与延迟目标；
- 文档；
- 包内容与可重复构建。

P7.4严格构建并冻结两个候选包：

1. **公网包**：Linux deployment package。
2. **本地一键包**：Windows release archive。

不存在第三个“部署包”或“部署结果包”；P7.5验证证据不是第三个包，生产部署是P7.5之后使用公网包执行的独立操作。

本地一键包的archive只声明运行时相对路径，不携带`data/rbcsp.sqlite`本身或其WAL/SHM/backup；公网包同样不携带预装真实课程/Open数据库。

P7.4不访问真实Vultr。它只输出`P7_5_ELIGIBLE`候选门，不宣告最终P7完成或Release资格。

### 17.5 P7.5：独立 real-world E2E

P7.5必须在P7.4冻结候选后按硬序执行：

1. 固定两个candidate SHA-256、live endpoint allowlist、请求预算、去敏规则及环境串行lease；
2. 在干净Windows标准用户环境解压真实archive，首次创建包根`data/rbcsp.sqlite`，连接真实Rutgers完成Catalog/Open、搜索、多筛选、Course/Section、freshness/lag/counters、真实浏览器WS、当前Open watch/toast与可用时声音链路；
3. 通过人工`workflow_dispatch`在GitHub Actions Ubuntu安装真实Linux candidate，以真实Caddy HTTPS/WSS和desktop/mobile浏览器完成等价流程，证明多个浏览器不会各自直连Rutgers、reload为新session且local-only表面为0；
4. 对用户指定且另行精确授权的Vultr staging实例创建可验证恢复点，安装同一hash，以测试内部CA/hosts映射完成真实Caddy/desktop/mobile流程；测试后恢复snapshot或按批准方案重装并做残留审计；
5. 汇总三环境同hash证据，全部mandatory assertion PASS后才形成最终P7 completion record。

真实Rutgers继续保持origin concurrency=`1`、per-target single-flight=`1`、有界请求与单次run；禁止压力/故障测试、cache bust、自动retry和Actions matrix。若没有当前真实Open section，记录`LIVE_PRECONDITION_NOT_MET`并停止，不无限等待或伪造。fake upstream仍负责确定性失败与Closed→Open状态转换。

P7.5中的Vultr身份是test/preproduction，不是production；P7批准不自动授权snapshot/install/systemd/Caddy/DB/restart/restore，必须另有命名实例exact diff。不得修改真实DNS、Cloudflare、ACME、生产证书或生产流量。使用同一机器测试不等于批准生产。

### 17.6 P7 提交规则

每个 P7 task 必须：

- 形成干净、实质、可公开的 commit；
- 通过对应验证；
- 推送到 P6 Review 批准的远端分支；
- 经审核后再进入公开默认分支；
- 不包含秘密、本地 inventory、私有历史或内部执行状态。

该规则只适用于 P7，不得扩散到 0A-P6。

### 17.7 P7 Release

只有同时满足以下条件时，才将两个包发布为 GitHub Release assets：

- P6 Review 已批准；
- `P7.5-005=P7_REAL_WORLD_E2E_PASS`，三环境消费同一两个candidate hash；
- 两个包的测试和验证通过；
- secret scan 通过；
- artifact audit 通过；
- 内容适合公开；
- 发布不会泄露服务器、凭据或内部执行状态。

GitHub Release 是条件步骤，不是强制步骤。

## 18. P7 后：生产部署

生产部署是独立阶段，不属于 P7 实现任务。

### 18.1 进入条件

- 公网包已经由P7.4产出并由P7.5三环境以同一hash验证；
- Vultr staging测试后已经恢复或重装，并在生产转换前重新取得只读discovery授权；
- 必要的发布/制品审计完成；
- 用户同意开始真实服务器变更；
- 已重新核验 Vultr 账户、VM、费用、备份、域名和私有 inventory。

### 18.2 服务器加固

至少包括：

- 建立 non-root 管理与运行用户；
- 使用 SSH key；
- 禁用 SSH 密码认证；
- 限制或禁用直接 root SSH；
- 配置最小必要防火墙端口；
- 启用安全更新；
- 配置日志、轮转和必要的监控；
- 验证备份与恢复路径。

### 18.3 域名与 HTTPS

- 确认域名；
- 决定 Cloudflare DNS/代理策略；
- 配置 Caddy；
- 配置并验证 HTTPS；
- 不在公开配置中写入私有服务器信息或 secret。

### 18.4 安装与启动

- 上传或取得经过审计的公网包；
- 以生产 secret/env 注入私有配置；
- 安装数据库、服务和前端制品；
- 配置 systemd；
- 启动 `bcsp-server`；
- 验证重启、失败恢复和日志。

### 18.5 生产验证

至少验证：

- 公网 URL；
- HTTPS；
- 手机与桌面 WebUI；
- 课程搜索、筛选和 section 信息；
- Delivery modality/synchronicity与真实raw映射；
- 课程目录固定10分钟刷新、每次课程信息时间截点；
- openSections普通刷新public固定30秒；active-watch相关共享batch按10秒fast lane与共享single-flight合同验证；每次Open信息时间截点与today attempted/succeeded/failed/empty计数；
- WebSocket 生命周期；
- 最多 9 个 section 的 active watch；
- ONE_SHOT每条Open observation触发至Max audible、达限只静默不停止watch；
- CONTINUOUS 10分钟/Unlimited、per-section确认、同episode不重响及Closed→Open；
- 新tab/reload为全新用户session，语言按system，filters/selection/audio/settings/history恢复默认；
- 本地Saved views跨启动、CRUD/migration/quota/incompatible/revision-conflict正常且不auto-apply default view，并且无Share/URL filter restore/cloud sync；公网入口/API/storage/catalog/bundle零存在；
- `en-US`/`zh-CN`完整UI；
- 连接断开后的清理；
- 日志、资源占用和容量；
- 回滚路径。

生产验证通过后，才能把“公网包以及部署”视为最终完成。

## 19. 强制审核门汇总

### P1 Review

- P1 完成后必须停止。
- 用户批准前不得进入 P2。
- 旧 P1 产物不能证明该门已经通过。

### P2 Review

- P2必须先达到文件`N/N`分类、三轴裁决、五矩阵闭环、复用/重写理由完备、零未分类和零当前交付`UNKNOWN`。
- P2完成后必须停止；用户批准前不得进入P3。
- 2026-07-13修订、“Saved views做”及其仅本地包裁决已纳入；P2为`P2 APPROVED — CLOSED`。P3初次人工漏读active campus `D`，用户明确纠正为只补Fall D而不扩成两term×15；`CAT-C021`已按amendment-002成功取得。Open两轮42/42完成；Rutgers官方term/campus set-membership/intersection、双时钟、empty/error、backoff/counter与通知前置合同已冻结。P4公网包及部署delta已通过76行baseline分类、144行zero-surface与258行trace验证；P5以76项唯一归属、106项测试、12项零未决泄漏与194行trace冻结单一共享基线。P6现包含保留的27个P7 task和新增5个P7.5 task（总计32）、真实世界E2E、Vultr staging恢复、验证/容量/依赖/双包/发布/生产边界。当前为`P3 PASS / P4 PASS / P5 PASS / P6 REVIEW READY / P7 NOT AUTHORIZED`，详见P3 `09`、P4 `07/07a`、P5 `07/07a`与P6 `09/09a/10/10a`。
- P2批准后，P3必须同时通过Catalog与openSections真实证据门；该门发现冲突时回到Review。P3现为`P3_PASS`；P4、P5均为`PASS`；P6现为`P6_REVIEW_READY`并已硬停。P4只设计公网/部署delta且没有重复取证，P7必须等待用户明确批准。
- P2批准只批准本地一键包ALL and ONLY基线及明确的公网delta标记，不自动批准P3/P4实现设计。

### P6 Review

- P6 完成后必须停止。
- 用户批准前不得开始 P7。
- 已批准并纳入：`P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001`；这项局部决定不等于P6 Review整体批准。
- 新增P7.5五个task、32总数、live预算与Vultr staging exact mutation gate仍须本次P6 Review明确批准；已完成的Vultr只读preflight不等于P7或mutation授权。
- 当前状态：`P6_REVIEW_READY / P7 NOT_STARTED_AWAITING_USER_APPROVAL / PRODUCTION NOT_AUTHORIZED`。

### P7.2 -> P7.3

- P7.2 必须先实现、集成和视觉验证。
- P7.3 必须保持独立 task、记录和提交序列。

### P7.4 -> P7.5 -> GitHub Release

- P7.4先冻结两个candidate；P7.5再以同一hash完成Windows、Actions、Vultr staging与恢复验证。
- `P7.5-005`通过后才可申请Release。
- Release 是条件步骤。

### P7 -> 生产部署

- P7.5只能在独立精确授权下把指定Vultr当作staging使用并恢复，不自动转为生产。
- 开始部署前必须重新取得用户授权并核验外部状态。

## 20. 明确禁止恢复的旧流程错误

当前工作流禁止：

- 恢复 NGAT/Organ 作为第二条权威线；
- 让 subagent 替用户作产品裁决；
- 让 subagent 自动跨阶段；
- 用旧 P1 产物证明 P1 已完成；
- 在 P1 提前决定旧能力的保留或删除；
- 把 P7 的逐 task 远端提交规则套到 0A-P6；
- 为任务数量或贡献图制造空 marker commit；
- 失败后无限叠加 corrected/remediation 替代链；
- 弱化、删除或合并 P7.2/P7.3；
- 省略三个指定 UI skill；
- 省略 `Before | After | Why` 审计；
- 把真实 Vultr 加固和生产部署塞入 P7；
- 恢复旧的 P7-R1 至 P7-R4 扩张；
- 因为服务器已经存在而跳过 P1-P7；
- 将敏感信息写入 Git、文档或发布包。

## 21. 当前基线的使用方式

1. 后续所有 Phase 讨论与执行都引用本文件。
2. 如果本文件与旧流程/P1产物冲突，以本文件为准。
3. 如果用户作出新裁决，应在当前主线审核后直接修订本文件或创建明确的新版本。
4. 不得通过修改旧废弃文件间接改变当前工作流。
5. 本文件定义流程；阶段状态由当前主线依据产物和验证记录更新。只有规定的硬门、产品范围裁决、发布判断和真实生产部署授权需要用户明确确认。
