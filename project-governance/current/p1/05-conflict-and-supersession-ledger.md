# P1 冲突、漂移与覆盖台账

## 1. 文档状态

- **状态**：P1 Review 已通过；冲突台账随获批P1关闭
- **作用**：把互相冲突的旧事实、当前明确覆盖、实现缺口和未知项集中呈现
- **不做的事情**：不在 P1 为历史未知项选择最终产品结论；不把“当前已经明确覆盖”误写成“旧历史不存在”

## 2. 台账状态

| 状态 | 含义 |
|---|---|
| `CURRENT_RESOLVED` | 当前权威工作流已明确裁决这项产品方向；旧状态仅保留为迁移/清理证据 |
| `SOURCE_DRIFT` | repo、branch、release、docs 或不同时点互不一致 |
| `IMPLEMENTATION_GAP` | 表面/契约/目标存在，但可达行为缺失或不满足 |
| `HISTORY_UNRESOLVED` | 旧历史自身没有足够证据解释；P1 不补猜 |
| `P2_UNRESOLVED` | 能力已恢复，但是否进入最终 all-and-only 面要等 P2 |
| `P4_OR_P7_VERIFY` | 当前目标已定，但设计、外部状态、性能或容量留待后续核验 |
| `PROCESS_INCIDENT` | 调查/执行过程中的边界或基础设施事件，不是产品能力 |

## 3. 冲突与覆盖主表

| ID | 主题 | 证据 A | 证据 B / 当前层 | 状态 | P1 结论与后续归属 |
|---|---|---|---|---|---|
| CON-001 | 旧P1禁区误扫 | 首位源码subagent的错误glob使`rg`机械执行了被禁止的内容扫描；只有路径/标题匹配行进入隔离subagent context，未报告显示其他正文。 | 用户明确禁止旧P1，目的为防止复用和污染。 | `PROCESS_INCIDENT` | 中止并丢弃该agent全部结果；clean-room重做。P1不能声称零扫描；结论链不含该agent结果。用户已在2026-07-12 P1 Review接受处置。 |
| CON-002 | “当前旧产品”是哪一个 tree | local main `2d76217`、internal dev、task-015、REL21/22 各自不同。 | public `origin/main=9c93170` 又含内部 dev 没有的 auto-refresh。 | `SOURCE_DRIFT` | 不选单一旧 snapshot 冒充真相；能力库存按时间/来源分别登记。 |
| CON-003 | Auto-refresh源码是否“未合并” | internal dev/task-015没有scheduled-fetch/auto-refresh文件。 | `e770bf2`经`b650d81`进入公开历史；当前origin/main仍有route/service/component定义，但App/server/container均未生产接线。 | `SOURCE_DRIFT` | 它在内部线缺失、公开线存在为orphan source surface；不能写成公开产品可用。server fetch与browser query refresh还须分开审计。 |
| CON-004 | 旧 release 是否可直接复用 | 旧包包含大量真实产品源码和部分 tests。 | 两代包来自不同工作树；无顶层目录、命令指向缺失脚本、runtime checkpoint、Mac metadata、repo link 等缺陷。 | `SOURCE_DRIFT` | 三个 archive 只能作为历史证据，不能作为可信 canonical build。P2/P3/P7需重新定义/构建。 |
| CON-005 | 最终交付数量 | 旧历史围绕单一本地 release、zip/tar、macOS 启动和公开源码展开。 | 当前严格两个任务、两个包：公网包 + 本地一键包；部署不是第三包。 | `CURRENT_RESOLVED` | 旧容器格式和历史包数量不决定当前交付结构。 |
| CON-006 | 后端架构 | 旧产品是 Node/TypeScript + Fastify + `better-sqlite3` + 多进程脚本。 | 当前目标是共享 React + Rust 模块化单体，`bcsp-local.exe` / `bcsp-server` 双入口。 | `CURRENT_RESOLVED` | 旧代码作为行为/迁移参考，不是最终后端。具体迁移与模块边界属 P2-P5。 |
| CON-007 | 本地一键的含义 | 旧 `.bat` 要求先装 Node 22，在线 npm install，可能要求 C++ Build Tools，并运行 Vite dev server。 | 当前本地一键包面向普通 Windows 用户，入口 `bcsp-local.exe`，不要求理解 Node/Rust/数据库。 | `CURRENT_RESOLVED` + `IMPLEMENTATION_GAP` | 旧 launcher 的流程/错误教训可恢复，但不满足当前目标。 |
| CON-008 | 首次自动抓取 | `docs/oneclick.md` 声称首次启动会迁移并 full fetch。 | launcher 先迁移创建 DB，再按 DB 存在性跳过 fetch；README 后来要求 UI 手动 Run fetch。 | `IMPLEMENTATION_GAP` | 旧首次运行行为不可靠；最终首次启动/数据空态属于 P2/P3 用户路径审计。 |
| CON-009 | SQLite 默认路径 | 历史出现 `data/local.db`、`data/fresh_local.db`、`data/courses.sqlite`；release 包相对路径可能在包目录写 DB。 | 当前双入口仍用 SQLite，但要求明确生命周期；用户曾直接报告 release folder 内生成 DB 的问题。 | `SOURCE_DRIFT` + `P2_UNRESOLVED` | P1记录路径漂移和用户问题，不选择最终目录。P3/P4按运行形态设计。 |
| CON-010 | macOS 支持 | 旧 `.command` 与文档把Windows/macOS都列为目标；用户无Mac设备但在2026-01与2026-05仍要求修好并支持Win/Mac。 | 曾收到一次真实Mac启动失败报告；允许旧史料未找到正式取消决定。用户在2026-07-12新确认当前取消macOS，以降低设备验证与分发复杂度。 | `SOURCE_DRIFT` + `CURRENT_RESOLVED` | 不宣称旧macOS已验证，也不伪称旧史料已有取消决定；当前版本和交付任务明确Windows-only。 |
| CON-011 | 邮件提醒 | 旧 SendGrid provider、dispatcher、模板、设置 UI/API、tests 都真实存在；SMTP只有形状。 | 当前两个交付任务都明确不含邮件/mail config；邮件只作 future feature。 | `CURRENT_RESOLVED` | 旧邮件能力仍进入库存，供 P2审计其专属 UI/API/worker/config/docs/tests/deps 的去向；P1不执行删除。 |
| CON-012 | Discord | Compact/Git证明 Discord sender/dispatcher 曾实现，后续提交整体删除；release无 Discord。 | 用户在P1 Review确认由其主动要求删除，理由是降低系统复杂度；当前也不新增另一通知产品。 | `CURRENT_RESOLVED` | 历史原因已解决；当前不恢复Discord。 |
| CON-013 | Subscription 持久性 | 旧模型把个人订阅、联系人、状态、quiet hours、snooze、max通知持久写入 SQLite。 | 当前active watch只在live connection和内存，不持久保存个人active subscription；用户同时明确当前仍需subscription management。 | `CURRENT_RESOLVED` | 持久active模型被覆盖，但管理用户路径没有被删除；旧表/API/UI清理和新live管理界面均须进入P2。 |
| CON-014 | 订阅对象与上限 | 旧 API按 term/campus/index建立持久 subscription，没有会话最多9项的证据。 | 当前对象为 section，单浏览器会话最多9个，并有开始/关闭开关。 | `CURRENT_RESOLVED` + `IMPLEMENTATION_GAP` | 新上限/开关是明确目标，旧实现不满足。 |
| CON-015 | Section key | schema唯一约束曾从 campus相关模型变为 `(term,index)`；API仍常携带 campus。 | 当前要求若不能证明 index跨 term/campus全局唯一就用安全复合键。 | `P4_OR_P7_VERIFY` | 已确认不能假设裸 index全局唯一；最终 key contract在设计/实现中验证。 |
| CON-016 | 断线恢复 | 旧 subscription可跨重启持久存在，local sound enabled/device也写 localStorage。 | 当前页面关闭/断线/超时/挂起使 active watch失效；可记住所选 section但不得自动恢复 active。 | `CURRENT_RESOLVED` | selection persistence与active persistence必须拆开。 |
| CON-017 | 提醒触发条件 | 旧 spec强调 Closed→Open、3分钟 bucket去重；release后期又对持续 Open每3分钟 reminder。 | 当前明确不防抖，每一条收到的 Open消息都触发声音，持续 Open也继续提醒。 | `CURRENT_RESOLVED` | 旧 edge/dedupe/reminder产品语义被覆盖；工程级重放/幂等另由 P4/P7设计。 |
| CON-018 | 声音 UX | 旧 hook一个非空批次只响一次0.35秒固定 tone，最多5 toast。 | 当前要求音量、一声/持续；用户确认WebUI toast也是明确subscription management需求。 | `CURRENT_RESOLVED` + `IMPLEMENTATION_GAP` | 旧实现不足；toast必须保留为能力，但样式、聚合和与声音的关系由P2/P3定义。 |
| CON-019 | 实时传输 | 旧浏览器每7秒 HTTP claim，本机 poller默认15秒；没有 WebSocket/SSE。 | 当前公网浏览器通过 WebSocket active watch，理想端到端 <1秒。 | `CURRENT_RESOLVED` + `P4_OR_P7_VERIFY` | transport目标已定；cadence、容量、降级、延迟口径后续设计/压测。 |
| CON-020 | `<1s` 是否等于每秒请求 Rutgers | 旧历史采用10–120秒不等 cadence。 | 当前工作流明确 `<1s` 是端到端目标，不预设每秒请求 Rutgers。 | `CURRENT_RESOLVED` | P1不得把旧 cadence或目标误写成固定 Rutgers QPS。 |
| CON-021 | 课程目录摄取周期 | 旧文档有nightly/30–60分钟，server scheduled SOC fetch源码为1–30分钟。 | 当前目录默认10分钟；本地可配，公网固定且不向普通用户开放。 | `CURRENT_RESOLVED` | 这里只覆盖从Rutgers更新目录的server/local ingest策略；旧scheduler仍是orphan。 |
| CON-022 | OpenSections 空响应 | 早期 event spec把 campus-wide empty视为软故障，不关闭。 | 后续 poller review/代码方向让空集合参与 closure，避免永久卡 Open。 | `HISTORY_UNRESOLVED` | 上游 empty、错误、真实零 open必须在新 poller contract中区分；P4/P7处理。 |
| CON-023 | `/api/sections` | 旧 query contract把它描述成真实 detail接口；route有完整 schema/logging，但handler固定 `total=0,data=[]`。 | 用户确认section必须可独立搜索、独立访问，并可打开完整详情页。 | `IMPLEMENTATION_GAP` + `CURRENT_RESOLVED` | 独立section用户能力已确定；旧stub是假表面。最终API形状由P2/P3设计，不能照抄旧route。 |
| CON-024 | Section详情是否实际存在 | 独立 sections endpoint是 stub；`/api/courses?include=sections`和数据 schema提供部分信息，UI有贫化展开。 | 用户确认默认course-centered并展开sections，同时要求点击section进入完整详情页。 | `SOURCE_DRIFT` + `CURRENT_RESOLVED` | course展开和独立详情两条路径都需要；“已有部分字段”不等于完整用户能力。 |
| CON-025 | 筛选字段全集 | 2025-11先扩张 index/status/instructor/permission/location等，后又大幅收缩；2026-01部分字段回来。 | 当前只固定“多条件筛选”，未在0A给出全部字段清单。 | `P2_UNRESOLVED` | P1已恢复完整漂移；P2必须 all-and-only 审计，不能继承某一次 rewrite为最终真相。 |
| CON-026 | Meeting-day/time语义 | 早期/错误实现可能只要任一meeting相交；用户报告选择Mon+Wed时会出现Thu section。 | P1 Review确认可用时间语义：整个section的每个已知必修meeting都必须完整落入同星期可用窗口。 | `SOURCE_DRIFT` + `CURRENT_RESOLVED` | 严格全部包含已成为当前验收方向；async/TBA/hybrid/optional/exam等边界留P2/P3。 |
| CON-027 | FilterPanel可达性 | 多条件面板功能多。 | 用户报告面板无法独立滚动，底部条件不可达。 | `IMPLEMENTATION_GAP` | UI/UX重建必须包含交互可达性，不只换视觉皮肤。P2/P7.2处理。 |
| CON-028 | Calendar/Compact/saved/share | 旧设计/Compact出现这些能力；SchedulePreview与URL helper有代码，生产App未接线。 | 用户确认Calendar是历史明确需求，但当前不进核心版本，仅作future；BCSP不替代CSP。Compact/saved/share尚未获得同等确认。 | `CURRENT_RESOLVED` + `P2_UNRESOLVED` | Calendar当前去向已解决；其余三项继续按独立历史候选审计，不能捆绑裁决。 |
| CON-029 | UI完成度 | 旧 React App可运行并包含多个功能组件。 | 无完整 frontend测试/真实设备矩阵；局部 orphan/stub；用户明确要求UI重建必须同时重做UX。 | `P2_UNRESOLVED` + `IMPLEMENTATION_GAP` | 旧UI是行为参考，不是当前正式UI。P7.2/P7.3已有独立硬门。 |
| CON-030 | API本地信任模型 | 旧API默认loopback；subscription列表、任意channel的ID-only退订、fetch子进程和mail admin均无认证。 | 当前新增公网多用户服务，安全边界不同。 | `CURRENT_RESOLVED` + `IMPLEMENTATION_GAP` | 旧route不能直接暴露公网；P2审计旧面，P4/P7设计公网auth/管理隔离（若需要）。 |
| CON-031 | 邮件凭据保存 | 旧admin API把SendGrid key明文写本地config，虽然response隐藏。 | 当前secret只能在本地或平台secret/env，不能进Git/包/公开文档。 | `CURRENT_RESOLVED` | 当前版本又明确无邮件；旧凭据面是清理和secret audit风险。 |
| CON-032 | Release私有/运行态内容 | REL21包含 poller checkpoint；旧包/仓库曾混入运行态。 | 当前两个包发布前必须 secret scan + artifact audit，私有inventory被Git忽略。 | `CURRENT_RESOLVED` + `IMPLEMENTATION_GAP` | 旧包不合格；P7.4/Release gate处理。 |
| CON-033 | 测试是否“通过” | Compact多次声称某些tests通过，源码也有不少测试文件。 | root `npm test`固定失败；frontend无已发现测试；本轮未执行；多次ABI/typecheck问题。 | `SOURCE_DRIFT` | 只能分别陈述“测试代码存在”“Compact声称运行”“当前执行未知”。 |
| CON-034 | README与实现 | README声称下载release、脚本自动抓取、刷新浏览器、邮件等。 | 包/launcher/API/当前目标分别存在缺失、漂移和覆盖；repo URL也错误。 | `SOURCE_DRIFT` | 文档不能作为代码完成证明。P2/P3/P4/P7需建立真实quickstart与release notes。 |
| CON-035 | 公网部署是否已存在/可跳阶段 | 旧历史曾讨论免费云/Google Cloud；当前可能已有外部VM记录。 | 当前明确使用Vultr EWR方向，但所有真实VM、费用、域名、安全状态部署前重核；生产部署在P7之后。 | `CURRENT_RESOLVED` + `P4_OR_P7_VERIFY` | 历史外部状态不能跳过P1-P7，也不能在P1触碰服务器。 |
| CON-036 | GitHub tags/Release当前状态 | Stage P旧报告称删掉所有tags和Release；本地/connector只见main、无tags。 | 本轮 live `git ls-remote`超时，GitHub Release对象未用API单独复验。 | `P4_OR_P7_VERIFY` | 当前证据足以理解历史，不足以替最终发布前实时核验。 |
| CON-037 | 旧 task-015是否可直接采纳 | 矩阵详细、覆盖30项，内容有价值。 | 仅存在于 `feature/task-015@5714a8f`，未合并；review因执行平面问题未通过。 | `HISTORY_UNRESOLVED` | 只把它当发现线索/旧文档，不继承其分类，更不把它当旧P1替代物。 |
| CON-038 | 关键词搜索是否真实可用 | course-search query和tests支持FTS/q；schema建立FTS表。 | 正常ingest未写/rebuild FTS，唯一insert在test fixture；旧文档却声称ingest后刷新。 | `IMPLEMENTATION_GAP` | 测试seed不能证明真实抓取链。P2必须把“搜索行为”按端到端链审计，P7需真实ingest验证。 |
| CON-039 | 周四meeting编码 | normalizer/UI/contract使用 `TH`。 | 历史真实SOC样本多次出现 `H`；当前代码对单个`H`返回null并在前端丢弃。 | `SOURCE_DRIFT` + `P4_OR_P7_VERIFY` | 旧bug高置信；当前Rutgers编码须重新probe，不能仅凭旧样本固定新映射。 |
| CON-040 | Meeting时间窗口是“任一”还是“全部” | 后端SQL只需存在一条meeting同时满足start/end。 | 前端二次过滤、UI文档和P1 Review用户确认均要求section全部已知必修meetings完整落入可用窗口。 | `SOURCE_DRIFT` + `CURRENT_RESOLVED` | 目标语义已解决为“全部完整包含”；旧后端是实现错误。未知时间等边界和多meeting测试留P2/P3。 |
| CON-041 | Ingest是否填完整数据模型 | schema/文档包含instructors、section_instructors、populations、crosslistings、FTS。 | 当前normal ingest只填course/section/meeting/core/location/status，未写上述表。 | `IMPLEMENTATION_GAP` | “表存在”不能写成“数据可用”。最终section详情字段须从真实ingest到UI逐字段验收。 |
| CON-042 | Fetch配置是否真实生效 | example/schema/docs暴露rate profile、retry policy、resume queue、recency、并发、downgrade、rebuildFts、metrics等。 | runner大多只定义/打印/转boolean；实际retry固定3次线性backoff。 | `SOURCE_DRIFT` | P2审计哪些是产品配置/内部配置；P3/P4重新设计，不继承假开关。 |
| CON-043 | Filters dictionary与真实编码 | frontend/types/docs期待examCodes及限定delivery集合，fallback给出term/campus。 | API不返回examCodes；normalizer产生`async`但route/UI不接受；fallback term/campus又与历史SOC编码不同。 | `IMPLEMENTATION_GAP` | fallback会掩盖真实DB问题。最终字典必须由真实数据contract和测试证明。 |
| CON-044 | Subscription偏好是否生效 | API可保存notifyOn、maxNotifications、quiet window、snooze、paused/suppressed；只有部分在fanout检查。 | 用户确认Max notifications是明确subscription management需求；其余旧偏好未获同等确认。 | `SOURCE_DRIFT` + `CURRENT_RESOLVED` + `P2_UNRESOLVED` | Max notifications必须进入当前能力面，但计数/上限动作/重连语义待P2/P3；不能把旧可保存字段当成完成实现。 |
| CON-045 | 邮件验证/管理闭环 | verification模板、unsubscribe token和邮件链接存在。 | 创建即active/verified，无verification endpoint；frontend无router，邮件中的manage/unsubscribe URL不可用；locale `zh`与`zh-CN`也不匹配。 | `IMPLEMENTATION_GAP` | 当前版本已明确无邮件；这些事实用于P2完整清理审计和future记录，而不是现在修复。 |
| CON-046 | Local sound是否保证送达 | API claim使用事务并立即标sent，避免重复。 | 浏览器之后才播放；autoplay阻止、tab关闭、播放失败不重投，多tab竞抢。 | `IMPLEMENTATION_GAP` + `CURRENT_RESOLVED` | 当前目标只保证打开且正常运行时可靠，但新WebSocket链仍需定义ack/播放失败行为；旧at-most-once claim不可直接继承。 |
| CON-047 | Snapshot/event保留 | 文档要求open events/notifications 30天清理。 | ingest会追加snapshot，源码无已发现retention job；poller与ingest snapshot处理也不同。 | `IMPLEMENTATION_GAP` | 本地/公网数据生命周期、清理和可观测性属P3/P4/P7。 |
| CON-048 | 当前课程列表是否提供足够section决策信息 | 数据模型/query expansion可携带meeting/instructor/location等，production CourseList却只显示课程基本信息及section index/open。 | 用户确认course-centered展开只是默认入口，section还必须有独立完整详情页。 | `IMPLEMENTATION_GAP` + `CURRENT_RESOLVED` + `P2_UNRESOLVED` | 两条用户路径已确定；详情字段all-and-only全集仍由P2定义并端到端验收。 |
| CON-049 | Fetch成功后UI是否看到新数据 | DataFetch成功会更新字典和term/campus。 | course query有45秒same-key缓存，无显式refetch；README仍要求手动刷新。 | `IMPLEMENTATION_GAP` | 当前目标中的数据状态/刷新反馈需要端到端验收。 |
| CON-050 | 本地ignored邮件配置 | 本机 `configs/mail_sender.user.json`路径存在。 | Git metadata显示该文件被ignore且未tracked；正文/secret状态因SAFE-INC-02从证据链排除，当前版本又明确不含邮件。 | `PROCESS_INCIDENT` + `P4_OR_P7_VERIFY` | P1不判断key存在或有效，不修改/显示文件。用户已接受事件处置；P2清理面与P7 secret/artifact scan仍必须覆盖。 |
| CON-051 | WebUI抓取会否清空其他term/campus | UI只提交year/season/campus且不传mode；API默认`full-init`。 | runner在任何network probe前全库DELETE配置表，不按所选term/campus限定；resolved subscription FK还可能使删除失败。 | `IMPLEMENTATION_GAP` | 旧多term/多campus记忆与真实UI行为冲突；P2必须纳入数据生命周期审计，P3/P7做多term及subscription-preservation回归。 |
| CON-052 | Browser auto-refresh与目录10分钟刷新 | 旧browser toggle约15–120秒，只重新查询本服务/刷新UI query。 | 当前10分钟决定针对课程目录摄取；workflow没有单独裁决浏览器query refresh控件。 | `P2_UNRESOLVED` | 不得把两层合并。P2/P3决定query cache/revalidation、手动刷新和是否需要普通用户toggle。 |
| CON-053 | “持续提醒”精确语义 | 当前明确要求一声/持续切换，且每条Open触发。 | 尚未定义持续声音何时停止/确认、持续多久、后续Open如何重触发、多个section如何并发。 | `P2_UNRESOLVED` + `P4_OR_P7_VERIFY` | P1不把开关存在误写成语义完整；P2-P4形成contract，P7浏览器/音频验证。 |

## 4. 当前已明确覆盖的旧能力

下表只重述当前权威决定，不是 P2清理清单：

| 旧能力/方向 | 当前覆盖 |
|---|---|
| Node/Fastify最终后端 | Rust共享核心 + Windows/Linux双入口 |
| 仅本地release产品线 | 公网包以及部署 + 本地一键包，两任务两包 |
| 邮件/SendGrid/SMTP当前通知 | 当前版本不含邮件；仅future feature |
| Discord/新增通知渠道 | 当前不新增另一套通知产品 |
| SQLite持久个人active subscription | live connection + 服务内存active watch |
| Closed→Open/3分钟去重作为产品触发 | 每一条收到的Open消息触发，不防抖 |
| 7秒HTTP claim | WebSocket active watch |
| 固定tone | 音量 + 一声/持续模式 |
| 普通用户可调公网scheduled fetch | 公网服务器固定目录刷新策略 |
| 单一本地UI与潜在公网分叉 | 两条任务线共享同一普通用户React UI |
| 把生产部署塞入打包阶段 | P7.4只产公网包；生产部署独立在P7后 |
| course与section只能二选一 | 默认course-centered展开，同时支持独立section搜索/访问/完整详情 |
| Calendar进入核心并替代CSP | 当前核心不含Calendar；仅作future，BCSP不替代CSP |
| macOS当前支持 | 当前本地一键包Windows-only；未来不预设 |
| live active不持久化等于没有管理 | 必须有subscription management界面和流程 |
| toast/Max notifications去向未知 | 两者均为明确需求；精确语义留P2/P3 |

## 5. P1 Review 解决记录

1. 用户批准能力库存作为P2输入，并补充RBCSP的原始目的：改善CSP筛选、性能/定位效率与低门槛体验，而非替代CSP。
2. 严格meeting规则确认为整个section全部已知必修meeting完整落入同星期可用窗口；未知时间等边界留P2/P3。
3. 默认UI以course为中心展开sections，同时section必须可独立搜索、独立访问并打开完整详情。
4. Calendar是用户明确旧需求，但当前仅作future；Compact/saved/share仍分开保持未决。
5. 旧史料证明macOS曾是目标且缺设备验证，但未找到旧的取消决定；当前Windows-only是本轮新决定。
6. 当前必须有subscription management；WebUI toast与Max notifications是用户明确需求，其他旧偏好不因此自动批准。
7. Discord由用户主动要求删除，原因是降低系统复杂度。
8. 用户接受SAFE-INC-01和SAFE-INC-02处置并批准P1；P1现已关闭，P2尚未自动启动。
