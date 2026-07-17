# P1 获批产品记忆

## 1. 批准状态

- **状态**：用户已于2026-07-12批准P1；P1关闭，P2尚未启动
- **来源**：`02-source-register.md` 登记的一手/受控来源，以及`04-current-decision-overlay.md`对当前权威workflow决定的索引/映射
- **目的**：把“旧 RBCSP 到底想做什么、真实做到了什么、哪里漂移”和“当前已经明确改变了什么”合成获批P1产品记忆
- **硬边界**：本文件不是 P2 的 `KEEP / REMOVE / REDESIGN / DEFER` 清单，不是实现计划，也不修改源码、包或服务器

## 2. P1 恢复结论

旧 RBCSP 不是一个可以用单一 commit、单一 release 或单一文档准确概括的产品。它至少存在以下彼此分裂的状态：

1. 2025-11 Compact 所描述的快速分任务实现；
2. 2026-01-21/22三个archive文件：REL21 zip/tar payload相同、容器metadata不同，REL22内容不同；
3. 本地旧 `main`；
4. 内部 `dev`；
5. 未合并的 `feature/task-015`；
6. 含未生产接线的 auto-refresh/scheduled-fetch源码表面的公开 `origin/main`；
7. 用户原始会话中对真实失败和不同步的直接报告。

因此，旧产品记忆必须采用“能力 + 行为 + snapshot + 证据状态”的模型，而不能采用“旧项目已经支持 X”的宽泛列表。

本轮恢复出的旧产品主干是：

> 一个本机运行的 Rutgers 课程发现与空位提醒工具。它从 Rutgers SOC 抓取课程和 section 数据到 SQLite，通过 Fastify API 和 React WebUI 提供搜索、筛选和 section 信息；用户建立持久 section subscription，由集中于本机的 openSections poller 产生事件，再通过浏览器本地声音或可选 SendGrid 邮件提醒。它有 Windows/macOS 启动脚本和源码型 release，但启动、数据、筛选、通知、打包和文档都存在真实漂移。

这段描述只代表旧产品的主干，不代表当前目标，也不宣称旧 release 达到可交付质量。

## 3. 当前产品目标

结合当前权威工作流与已完成的P1 Review，产品目标可以表述为：

> RBCSP 将成为同一套普通用户 React WebUI、同一套 Rust 共享核心驱动的 Rutgers 课程搜索、筛选、section 查看与实时 section watch 产品，并以两个交付任务落地：`公网包以及部署` 与 `本地一键包`。最终严格产出 `公网包` 和 `本地一键包` 两个包；真实公网部署在 P7 之后使用公网包独立执行。两种运行形态共享普通用户体验，但一个运行在集中式 Linux 服务器，另一个通过 Windows `bcsp-local.exe` 在用户电脑上一键运行。

该目标中的当前固定行为包括：

- BCSP优化Rutgers CSP的筛选、课程定位效率与低门槛体验，不以替代CSP为目标；
- 搜索 Rutgers 课程；
- 多条件筛选；
- 默认以course为中心展示，展开课程卡片查看sections；
- section可独立搜索、独立访问，点击后进入完整详情页；
- meeting可用时间筛选要求整个section的全部已知必修meetings完整落入同星期可用窗口；未知时间等边界留P2/P3；
- 查看课程和 section 决策信息；
- 查看 open/closed；
- 每个浏览器会话选择并 watch 最多 9 个 section；
- 用户明确打开/关闭 active watch；
- 公网浏览器通过 WebSocket 连接本服务，不直接轮询 Rutgers；
- active watch 只在 live connection 和服务内存中，断线后失效；
- 可在浏览器本地记住 section 选择，但不得自动恢复 active；
- 必须有subscription management界面和流程，管理所选section与active watch；
- WebUI toast与Max notifications是明确需求，精确计数、上限动作和重连语义留P2/P3；
- WebUI 打开且正常运行时，以声音提醒；
- 音量可调，可切换一声/持续提醒；持续模式的停止、时长、重触发和多section并发尚待后续contract定义（CUR-SND-05 / CON-053）；
- active watch 期间，每一条收到的 Open 状态消息都触发声音，不做产品层防抖；
- 课程目录默认 10 分钟刷新：本地用户可配，公网服务器固定且不向普通用户暴露；
- 公网端集中维护数据库、目录刷新和 openSections poller；
- 从状态更新到 WebUI 提醒的理想目标为 1 秒以内，但必须在 P4/P7设计、测量和压测；
- 当前版本不含邮件、SMTP、SendGrid、邮件 UI/worker/config/docs；也不新增 Web Push、原生 App、系统通知或 Discord；
- Calendar不进入当前核心版本，只作future feature；
- 本地一键包当前只支持Windows，不支持macOS；
- 公网目标环境为 Vultr EWR Linux VM 方向，生产状态在最终部署前重新核验；
- 发布前执行 secret scan 和 artifact audit。

上述决定已汇总进当前权威工作流；其中产品定位、严格meeting规则、course/section路径、Calendar、Windows-only、subscription management、toast/Max notifications和Discord原因由用户在P1 Review直接确认。

当前结论追踪：

| 获批主题 | Overlay IDs |
|---|---|
| 两任务、两包、部署独立 | CUR-DEL-01～05 |
| 产品定位、course/section路径、Calendar边界与正式UI阶段 | CUR-UX-01～08、CUR-UI-01～02、CUR-QRY-01 |
| 无邮件/无新增通知产品 | CUR-NOT-01～05 |
| section watch、9项、live lifecycle、key与subscription management | CUR-WATCH-01～10 |
| 每条Open、音量、一声/持续及待定义的持续模式细节 | CUR-SND-01～05 |
| 目录10分钟与browser query refresh分层 | CUR-DATA-01～03、CUR-UIREFRESH-01 |
| 公网集中poller/WebSocket | CUR-RT-01～02、CUR-PUB-01～05 |
| Rust双入口/Vultr/systemd/Caddy/Windows-only/security | CUR-ARCH-01～04、CUR-PLAT-01～04、CUR-SEC-01～03 |
| P1批准、P6/P7/Release/部署门 | CUR-GOV-01～10 |

## 4. 获批普通用户路径

### 4.1 两条任务线共享的核心路径

1. 用户进入同一套 React WebUI。
2. 系统显示数据可用性、刷新/等待、错误或空态，而不是呈现假数据或静默失败。
3. 用户选择 term、campus、subject 等范围，并按自身可用时间等条件搜索/筛选课程；全部已知必修meetings必须完整落在允许窗口内。
4. 结果默认以course为中心；用户展开课程卡片查看sections。
5. 用户也可独立搜索/访问section，点击进入完整详情页，查看时间、地点、授课方式、教师、open/closed等最终经P2确认的字段。
6. 用户选择最多9个section，并在subscription management中查看和管理选择。
7. 用户明确点击“开始订阅”，建立active live watch。
8. 服务集中获得状态并向在线浏览器发送Open消息。
9. 每一条Open消息显示WebUI反馈，并按用户选定的音量和提醒模式触发声音；toast/Max notifications精确语义留P2/P3。
10. 用户关闭订阅，或页面/连接失效后active watch被清理。

### 4.2 本地一键包特有的进入路径

1. 普通 Windows 用户下载并解压本地一键包。
2. 双击 `.bat`。
3. `bcsp-local.exe` 管理本地服务、SQLite、课程数据、状态监控与 WebUI。
4. 启动过程应提供明确的首次数据、端口、浏览器、错误和停止行为。

旧 launcher 的 Node/npm/native build/Vite dev 行为不能被当作当前路径已经实现；它只提供失败模式和迁移参考。
当前版本不提供macOS一键包或macOS支持声明。

### 4.3 公网包以及部署特有的进入路径

1. P7.4 先产出并验证 Linux 公网包。
2. P7 后经用户授权，重新核验 VM、费用、备份、域名和私有 inventory。
3. 加固服务器，以 systemd/Caddy 安装 `bcsp-server`，配置 HTTPS。
4. 用户通过公网 URL 使用与本地一致的普通用户 WebUI。

公网生产部署不是第三个包，也不是 P7.4 的隐藏步骤。

## 5. 必须带入 P2 的旧能力域

下表表示“P2 必须看见并作 all-and-only 审计”，不表示 P1 推荐保留。

| 能力域 | 已恢复的行为范围 | P2不能忽略的历史事实 |
|---|---|---|
| SOC 获取 | courses/openSections、term/campus、field matrix、retry/rate-limit、staging | 上游参数/字段有限；旧限流样本不能代表当前；周四 day code 存在 `H/TH` 风险。 |
| SQLite 数据 | schema/migrations、course/section/meeting/instructor/population/crosslist/core/status/event/FTS | section key不应假设裸 index全局唯一；DB路径和生命周期曾漂移。 |
| Full/incremental ingest | hash、subject slice、upsert/delete、summary、open-status reconciliation | 正常 ingest 未证明维护 FTS；旧 queue/resume有实现与文档计划混合。 |
| 课程目录摄取 | WebUI fetch job、server scheduled SOC fetch | 公开main、内部dev、release不一致；scheduler源码在公开main/REL也未生产接线；当前目录默认10分钟策略已重定义。 |
| 浏览器query refresh | 15–120秒auto-refresh toggle、45秒query cache、手动刷新 | 只重新查询本服务，不等同于抓Rutgers；当前10分钟目录策略没有裁决这一UI/缓存行为，留P2/P3。 |
| Course search | FTS/q、campus/subject/level/credits/core/prereq/campus location；course number没有独立filter，只可能理论上经q匹配或用于sort | tests手工seed FTS能证明query代码，但真实ingest未写FTS会使q及号码文本匹配链失效。 |
| Section关联filter | 当前可达delivery/open/exam/meeting day/time/location；早期instructor/permission/waitlist等曾出现后删除或只留在stub/data面 | “全部已知必修meeting完整落入同星期可用窗口”已固定；async/TBA/hybrid/optional/exam等边界与字段全集留P2/P3。 |
| Section信息 | course expansion、独立sections contract、meeting/instructor/location等 | course-centered展开与独立section搜索/访问/完整详情均为明确需求；旧`/api/sections`固定空结果stub不能冒充完成。 |
| Filters dictionary | DB字典和fallback | fallback可掩盖真实DB空/失败；字段面经历多次扩张/收缩。 |
| React UI/UX | App shell、FilterPanel、CourseList、DataFetch、subscription、local sound、i18n | 旧UI有不可达滚动、orphan组件、无完整frontend tests；重建必须落实course-centered、独立section详情和subscription management。 |
| Calendar | SchedulePreview、toggle历史和设计记录 | 用户确认是明确旧需求但未完成；当前核心不含，仅作future，不能据此把BCSP做成CSP替代品。 |
| Compact/saved/share | URL helper和设计记录 | 仍是orphan/文档候选，历史来源和当前去向留P2审计。 |
| 旧持久subscription | create/list/unsubscribe、unresolved、dedupe、状态、偏好、历史 | 当前active watch非持久，但必须有subscription management；旧route/table/UI/tests/deps与新管理模型都要完整审计。 |
| Poller/event | 15秒、auto discovery、checkpoint、Closed/Open、3分钟reminder、empty/miss | 历史cadence与empty语义漂移；当前每条Open触发语义已覆盖旧产品语义。 |
| 本地声音 | 7秒HTTP claim、device id、固定tone、toast | 当前改为WebSocket、音量、一声/持续；WebUI toast明确保留为能力，精确反馈/聚合语义留P2/P3。 |
| 邮件 | SendGrid、dispatcher、templates、admin UI/API、config、tests、SMTP形状 | 当前版本明确不含邮件；P2仍须审计整个专属依赖面，避免残留。 |
| Discord | 曾实现/测试后整体删除 | 用户确认主动删除以降低复杂度；当前不恢复。 |
| 启动/打包 | `.bat/.command`、Node/npm/native rebuild、多进程、三旧包 | 旧包不自包含、首次fetch有bug、Mac失败、命令/文件漂移、checkpoint泄漏；当前Windows-only，旧史料未证明早期正式取消Mac。 |
| 健康/日志/测试 | health/ready、trace、poller metrics、分散tests、reports | root test入口失败、frontend tests缺失、Compact成功声称不能等同当前通过。 |
| 本地安全假设 | loopback、无auth admin/subscription、明文key config | 不能迁移为公网安全模型；当前又已排除邮件/个人持久active。 |

能力域稳定追踪：

| 能力域组 | Inventory IDs | Conflict IDs |
|---|---|---|
| 产品身份与历史目标 | LEG-ID-01～07 | CON-005～006、CON-010、CON-012 |
| 启动与archive | LEG-RUN-01～09、LEG-PKG-01～05 | CON-004、CON-007～010、CON-032～034 |
| SOC与数据链 | LEG-SOC-01～05、LEG-DAT-01～12 | CON-008～009、CON-020～022、CON-038～043、CON-047、CON-049、CON-051～052 |
| 查询/筛选/section | LEG-QRY-01～13 | CON-023～026、CON-038～040、CON-043、CON-048 |
| React UI/UX/i18n | LEG-UI-01～12 | CON-027～029、CON-048～049、CON-052 |
| subscription/poller | LEG-SUB-01～08、LEG-POLL-01～08 | CON-013～017、CON-022、CON-030、CON-044、CON-047、CON-051 |
| 声音/邮件/Discord | LEG-SND-01～04、LEG-MAIL-01～06、LEG-DISC-01 | CON-011～012、CON-017～019、CON-031、CON-045～046、CON-050、CON-053 |
| 运行/测试/安全/Git | LEG-OPS-01～10、LEG-SEC-01～03、LEG-GIT-01～06 | CON-002～003、CON-030～037、CON-050 |
| 调查过程事件 | — | CON-001 |

## 6. 必须保留的旧失败记忆

以下不是普通“技术债摘要”，而是会直接改变产品审计与验收方式的失败记忆：

1. **repo、公开 main 和 release 不同步**：用户自己曾怀疑并要求清洗；Git和archive已证实。
2. **完整契约不代表真实能力**：`/api/sections`有完整schema但永远空。
3. **测试能通过人工seed，不代表真实数据链可用**：FTS测试自己写入FTS，而正常ingest没有已发现的FTS维护。
4. **筛选文案与SQL语义可能不同**：meeting目标已确认为“整个section的全部已知必修meetings完整包含”；旧后端“至少一条符合”是实现错误，前后端总数/分页必须统一。
5. **上游编码不能凭常识归一化**：历史样本周四为`H`，旧normalizer/UI只识别`TH`。
6. **“一键”表面不能代替干净机验证**：旧入口要求Node、在线npm、native rebuild，甚至可能要求C++ Build Tools。
7. **初始化顺序会使首次用户路径失效**：迁移先创建DB，随后存在性判断跳过full fetch。
8. **单次WebUI抓取可能破坏多term数据**：UI默认full-init，runner按全库表DELETE而非所选term/campus清理。
9. **代码存在不代表生产接线**：SchedulePreview、URL helper、scheduled/auto-refresh曾出现orphan状态。
10. **本地安全假设不能暴露公网**：无auth subscription/admin只因loopback勉强成立。
11. **release内容必须审计**：旧包含runtime checkpoint、错误repo链接、缺失命令目标和跨平台metadata问题。
12. **声音行为是产品语义，不是实现细节**：旧edge/dedupe/reminder/HTTP claim均与当前明确目标不同。
13. **UI redesign必须包含UX redesign**：旧功能堆叠、滚动不可达和不完整状态面不能通过换皮解决。

## 7. 当前明确非目标

这些非目标已经由当前权威工作流固定，不需等待 P2再次裁决方向：

- 当前版本的 email/SMTP/SendGrid/mail config/mail worker/mail settings UI；
- Discord；
- 当前核心版本中的Calendar；
- 当前版本的macOS支持或macOS一键包；
- Web Push、原生 App、系统通知等另一套通知产品；
- 浏览器直接轮询 Rutgers；
- 服务端持久保存个人 active watch；
- 页面关闭/系统挂起后仍保证声音；
- 纯 Cloudflare Serverless、Cloud Run、Sites 作为实时后端；
- 把生产部署伪装成第三个包；
- 把旧 Node/Fastify 实现当最终后端；
- 把旧 release 或旧 P1 产物直接复用为当前答案。

P2仍需审计这些旧能力的专属残留面；“当前非目标”不等于 P1 已执行清理。

## 8. P1 Review 批准与用户澄清

用户于2026-07-12接受SAFE-INC-01/02处置、批准P1，并确认：

1. RBCSP源于原CSP筛选条件少、性能和定位效率不足；目标是低门槛改善绝大多数学生的选课发现体验，不替代CSP。
2. 严格时间筛选表示用户可上课窗口；整个section的每个已知必修meeting都必须完整落入同星期可用窗口。12 credits只是说明剩余时间有限的场景背景，不自动产生课表导入或学分规则。
3. 默认UI以course为中心；搜索到课程后展开查看sections。同时section必须可独立搜索、独立访问，点击进入完整详情页。
4. Calendar是用户历史明确要求、因早期Agent与项目经验限制未完成的功能；当前不进核心版本，只作future feature。Compact/saved/share未被此次回答一并确认。
5. macOS历史上确为支持目标，用户无Mac设备且Mac启动失败。允许史料未找到后来正式取消的旧记录；当前Windows-only是本轮新确认决定，理由是缺少设备并降低复杂度。
6. 当前必须有subscription management；WebUI toast与Max notifications是用户明确需求。quiet hours、snooze、history等其他旧偏好没有因本次回答自动批准。
7. Discord由用户主动要求删除，理由是降低系统复杂度。
8. 产品始终围绕用户自身痛点：更丰富筛选、更快定位目标课程，并尽可能降低普通同学使用门槛。

## 9. P2 的获批输入边界

P2启动时应以以下组合为输入：

- `project-governance/current/single-mainline-delivery-workflow.md`：唯一当前权威源，必须列为第一输入；
- 本文件的获批产品记忆；
- `03-legacy-capability-inventory.md` 的行为级完整库存；
- `04-current-decision-overlay.md` 的当前决定索引/映射；它不能替代上述权威workflow；
- `05-conflict-and-supersession-ledger.md` 的冲突与未知；
- `02-source-register.md` 的证据等级和禁区边界；
- 当前项目/旧release/公开main的重新可验证事实。

P2随后才可以为本地一键包逐项给出正式 `KEEP / REMOVE / REDESIGN / DEFER`，并审计每项能力的 UI、API、worker、config、secret、docs、tests、dependencies、startup 和 package 影响。

P1已批准并关闭。本轮没有自动启动P2；进入P2仍应作为下一阶段单独开始并遵守其all-and-only边界。
