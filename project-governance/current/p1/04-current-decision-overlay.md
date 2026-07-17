# P1 当前决定覆盖层

## 1. 文档状态

- **状态**：P1 Review 已通过；本覆盖层属于获批 P1 输入
- **唯一当前汇总来源**：`project-governance/current/single-mainline-delivery-workflow.md`，已并入2026-07-12 Review决定
- **作用**：把已经明确生效的当前决定，与 P1 恢复出的旧 RBCSP 能力和行为放到同一坐标系中
- **不做的事情**：本文件不替 P2 作 `KEEP / REMOVE / REDESIGN / DEFER` 裁决，不把旧实现状态当成当前目标，也不因新决定覆盖旧决定而删除历史证据

## 2. 关系标签

| 标签 | 含义 |
|---|---|
| `CURRENT_FIXED` | 当前权威工作流已经明确固定的目标或边界 |
| `CURRENT_SUPERSEDES` | 当前决定明确替代某项旧决定或旧行为；旧行为仍保留在历史库存中 |
| `CURRENT_REFRAMES` | 旧能力域仍相关，但当前产品模型、运行位置或语义已经改变 |
| `CURRENT_ADDS` | 当前决定新增了旧 RBCSP 没有证明具备的目标 |
| `HISTORY_COMPATIBLE` | 旧能力与当前方向相容，但“相容”不等于 P2 已决定保留其旧实现 |
| `P2_UNRESOLVED` | P1 只能恢复事实；最终交付面仍须由 P2 审计裁决 |
| `P4_OR_P7_VERIFY` | 当前工作流明确把设计、测量、容量或生产核验留到后续阶段 |

这些标签描述“关系”，不是实现任务状态。

## 3. 交付、产品身份与普通用户路径

| 当前 ID | 当前固定决定 | 与旧历史的关系 | P1 解释 |
|---|---|---|---|
| CUR-DEL-01 | 最终只有两个交付任务：`公网包以及部署`、`本地一键包`。 | `CURRENT_FIXED` | 任务名称不再使用容易与阶段号混淆的产品字母别名。 |
| CUR-DEL-02 | 最终严格只有两个包：`公网包`、`本地一键包`。 | `CURRENT_FIXED` | 文档、报告、Release 和部署结果都不是第三个包。 |
| CUR-DEL-03 | `公网包`由 P7.4 产出；真实生产部署是 P7 后的独立操作。 | `CURRENT_ADDS` | 旧本地发布历史不能证明公网包或生产部署已经完成。 |
| CUR-DEL-04 | `本地一键包`面向普通 Windows 用户：下载、解压、双击 `.bat` 即可使用。 | `CURRENT_REFRAMES` | 旧 `.bat`/`.command`、Node 安装和旧 release 是历史参考，不自动满足“普通用户一键包”。 |
| CUR-DEL-05 | 公网包面向 Linux 常驻服务器。 | `CURRENT_ADDS` | 旧 Node 本地 release 与旧公开源码都不是当前 Linux 公网包。 |
| CUR-UX-01 | 两条交付任务线共享同一套普通用户 React WebUI 和核心体验。 | `CURRENT_FIXED` | 不允许形成两个普通用户产品或两套独立 UI。 |
| CUR-UX-02 | 共享核心体验至少覆盖课程搜索、多条件筛选、课程/section 信息、open/closed、按 section 订阅、WebUI 声音提醒。 | `HISTORY_COMPATIBLE` + `CURRENT_REFRAMES` | 旧实现为恢复行为和缺口提供证据；具体旧组件是否进入最终产品仍属 P2。 |
| CUR-UX-03 | 页面结构、语言和主要交互在两条交付任务线中保持一致；部署管理能力可以不同。 | `CURRENT_FIXED` | 运行面差异不能泄漏成普通用户产品分叉。 |
| CUR-UX-04 | P7.2 必须完成正式桌面、移动与响应式 UI；P7.3 独立审计和打磨。 | `CURRENT_ADDS` | 旧 React UI、设计稿和组件只能作为行为/问题参考，不能被宣告为正式 UI。 |
| CUR-UX-05 | BCSP用于优化Rutgers CSP的筛选、课程定位效率与低门槛体验，不以替代CSP为目标。 | `CURRENT_FIXED` | 用户痛点是原CSP筛选少、性能/定位效率不足；Calendar等扩展不能改变产品边界。 |
| CUR-UX-06 | 默认UI以course为中心：先找到课程，再展开课程卡片查看sections。 | `CURRENT_FIXED` | 旧CourseList的贫化展示不是最终设计，但course→section层级是明确用户模型。 |
| CUR-UX-07 | Section必须支持独立搜索、独立访问；点击后进入可直接访问的完整详情页。 | `CURRENT_FIXED` + `CURRENT_ADDS` | 旧`/api/sections`空stub不满足该需求；不能只保留course内预览。 |
| CUR-UX-08 | Calendar周课程表不进入当前核心版本，只作为future feature保留。 | `CURRENT_FIXED` | Calendar是用户历史明确要求而未完成的能力，但BCSP不替代CSP。 |
| CUR-UI-01 | P7.2是独立正式UI subphase，必须使用`$industrial-brutalist-ui`与`$design-taste-frontend`，覆盖桌面/移动/响应式、完整状态、可访问性和反馈。 | `CURRENT_FIXED` | 这是后续执行约束；P1不调用这些skill或实施UI。 |
| CUR-UI-02 | P7.3必须在P7.2实现、集成、视觉验证后，另用`$emil-design-eng`进行审计和打磨。 | `CURRENT_FIXED` | 必须是不同task、完成记录和commit序列，先`Before | After | Why`，再打磨并做真实视觉复验。 |
| CUR-QRY-01 | 用户可用时间筛选以整个section为单位；每个具有明确星期/起止时间的必修meeting都必须完整落入同星期可用窗口。 | `CURRENT_FIXED` + `P2_UNRESOLVED` | “全部meeting完整包含”已固定；async、TBA、hybrid、optional、exam等边界由P2/P3统一contract。 |

当前共享普通用户主路径可表述为：

`首次进入 → 获得/等待课程数据 → 搜索与严格筛选 → 查看course并展开sections/独立打开section详情 → 选择最多9个section → 在subscription management中明确开始订阅 → 页面保持连接时接收Open状态、显示toast并播放声音 → 明确关闭订阅或连接失效`

其中“最多 9 个”“明确开始”“live connection”“每条 Open 消息”均是当前新增或重定义的语义，不能从旧持久订阅实现反推。

## 4. 邮件、通知与声音

| 当前 ID | 当前固定决定 | 与旧历史的关系 | P1 解释 |
|---|---|---|---|
| CUR-NOT-01 | 两条交付任务线当前版本都不包含邮件提醒。 | `CURRENT_SUPERSEDES` | 旧 SendGrid、SMTP 形状、mail worker、模板、邮件设置 UI 和邮件文档仍登记为旧能力，但不再是当前目标。 |
| CUR-NOT-02 | 公网包以及部署不包含 mail config。 | `CURRENT_SUPERSEDES` | 旧配置和凭据面不能进入公网包。 |
| CUR-NOT-03 | 邮件提醒只作为 GitHub future feature 记录。 | `CURRENT_FIXED` | P1 不负责创建 issue；只记录这一当前去向。 |
| CUR-NOT-04 | 当前不新增 Web Push、原生 App、系统通知或另一套通知产品。 | `CURRENT_FIXED` | 旧 Discord 历史也不能自动恢复成当前通知渠道。 |
| CUR-NOT-05 | 声音提醒仅保证 WebUI 打开且正常运行时可靠工作。 | `CURRENT_REFRAMES` | 不承诺页面关闭、手机锁屏、系统挂起后的后台播放。 |
| CUR-SND-01 | UI 必须提供音量调节。 | `CURRENT_ADDS` | 旧 Web Audio 固定增益不满足这一目标。 |
| CUR-SND-02 | UI 必须提供“一声提醒 / 持续提醒”切换。 | `CURRENT_ADDS` | 旧实现的“每批通知播放一次短音”不是该模式开关。 |
| CUR-SND-03 | 不做防抖。active watch 期间每一条收到的 Open 状态消息都触发声音。 | `CURRENT_SUPERSEDES` | 覆盖旧 Closed→Open 边沿、3 分钟 bucket 去重和持续 Open 周期 reminder 语义。 |
| CUR-SND-04 | section 持续为 Open 时，收到后续 Open 消息仍继续提醒。 | `CURRENT_SUPERSEDES` | 当前事件语义由收到的 Open 消息驱动，不只看数据库状态变化。 |
| CUR-SND-05 | “持续提醒”开关必须存在，但停止/确认、单次持续时长、后续Open重启或叠加、多section并发混音/排队尚未定义。 | `P2_UNRESOLVED` + `P4_OR_P7_VERIFY` | P1不补猜；P2-P4定义可验收语义，P7用真实浏览器验证。 |

“不做防抖”不等于允许网络重复包、重连重放或多 poller 重复产生无界消息；这些工程边界要在 P4/P7 定义，但不得把它们重新解释为产品层的 Closed→Open 防抖。

## 5. Section watch 与连接生命周期

| 当前 ID | 当前固定决定 | 与旧历史的关系 | P1 解释 |
|---|---|---|---|
| CUR-WATCH-01 | 订阅对象是 section。 | `CURRENT_FIXED` | 旧代码中的 term/campus/index 组合和 section 解析历史是相关证据。 |
| CUR-WATCH-02 | 单个浏览器会话最多同时订阅 9 个 section。 | `CURRENT_ADDS` | 旧持久订阅 API 没有证明该会话上限。 |
| CUR-WATCH-03 | UI 必须有“开始订阅 / 关闭订阅”开关。 | `CURRENT_REFRAMES` | 旧“创建持久订阅/退订”的状态机不能直接代替 live watch 开关。 |
| CUR-WATCH-04 | active watch 只存在于当前 live connection 与服务内存。 | `CURRENT_SUPERSEDES` | 覆盖旧 SQLite 持久 subscription、联系人验证、paused/suppressed/snooze 等个人 active 状态模型。 |
| CUR-WATCH-05 | 页面关闭、断网、连接超时或浏览器挂起后，active watch 失效或不再保证提醒。 | `CURRENT_FIXED` | 连接清理是产品语义的一部分。 |
| CUR-WATCH-06 | 浏览器可本地记住所选 section，但不得自动恢复“正在提醒”。 | `CURRENT_FIXED` | “选择恢复”与“active 状态恢复”必须严格区分。 |
| CUR-WATCH-07 | 服务端不持久保存个人 active subscription。 | `CURRENT_SUPERSEDES` | 旧 subscription 表可作为迁移和历史审计证据，不能成为当前 active watch 的默认设计。 |
| CUR-WATCH-08 | 若 section/index number 不能证明跨 term、campus 全局唯一，必须使用安全复合键。 | `P4_OR_P7_VERIFY` | 旧 schema 曾出现 term/index 与 term/campus/index 不一致；P1 只登记风险。 |
| CUR-WATCH-09 | 当前产品必须有明确的subscription management界面和流程，用于查看、管理和停止所选section与active watch。 | `CURRENT_FIXED` + `CURRENT_REFRAMES` | live/non-persistent描述active生命周期，不取消普通用户的订阅管理需求。 |
| CUR-WATCH-10 | WebUI toast与Max notifications是subscription management的明确需求。 | `CURRENT_FIXED` + `P2_UNRESOLVED` | P2/P3必须定义计数范围、达到上限后的动作、重连行为，以及与每条Open和一声/持续模式的关系。 |

## 6. 数据刷新、实时链路与公网集中模型

| 当前 ID | 当前固定决定 | 与旧历史的关系 | P1 解释 |
|---|---|---|---|
| CUR-DATA-01 | 课程目录从Rutgers重新抓取/更新的默认周期为10分钟。 | `CURRENT_SUPERSEDES` | 覆盖旧nightly、30–60分钟和server scheduled SOC fetch 1–30分钟等目录摄取策略；不等同于浏览器重新查询本服务。 |
| CUR-DATA-02 | 本地一键包的课程目录摄取默认10分钟，并允许本地用户配置。 | `CURRENT_REFRAMES` | 旧环境变量、fetch pipeline/config和server scheduler是参考；不直接规定最终配置UX。 |
| CUR-DATA-03 | 公网服务器使用固定课程目录摄取策略，不向普通用户暴露该配置。 | `CURRENT_SUPERSEDES` | 覆盖普通用户可调server scheduled SOC fetch的方向。 |
| CUR-UIREFRESH-01 | 浏览器何时重新查询本服务、是否提供15–120秒auto-refresh toggle，是与课程目录摄取不同的UI/query-cache问题。 | `P2_UNRESOLVED` | 当前工作流没有单独裁决该控件；不能用“目录10分钟刷新”自动覆盖或批准旧browser auto-refresh。 |
| CUR-RT-01 | 从课程状态更新到 WebUI 提醒的理想端到端目标为 1 秒以内。 | `CURRENT_ADDS` + `P4_OR_P7_VERIFY` | 这是待设计、测量和压测的目标，不等于预先规定每秒请求 Rutgers。 |
| CUR-RT-02 | Rutgers 限流、轮询频率、延迟口径、容量和降级策略在 P4/P7 处理。 | `P4_OR_P7_VERIFY` | P1 不根据旧 10–20 秒、15 秒或 60–120 秒轮询历史提前决定新 cadence。 |
| CUR-PUB-01 | 公网服务器维护统一课程/section 数据库并集中刷新目录。 | `CURRENT_ADDS` | 旧单机 SQLite 是数据模型参考；公网多用户运行语义是新的。 |
| CUR-PUB-02 | 公网服务器集中轮询 Rutgers `openSections`。 | `CURRENT_REFRAMES` | 多个浏览器关注同一 section 时也只由服务器集中获取一次状态。 |
| CUR-PUB-03 | 手机和电脑浏览器只访问本服务，不直接轮询 Rutgers。 | `CURRENT_FIXED` | 旧浏览器轮询本地 claim API不等于直连 Rutgers，但也不满足当前 WebSocket 链路。 |
| CUR-PUB-04 | 浏览器通过 WebSocket 建立 active watch。 | `CURRENT_ADDS` | 旧 HTTP claim/polling 链路没有 WebSocket/SSE。 |
| CUR-PUB-05 | 服务端以内存映射维护 section 与在线连接；断开或心跳超时后移除。 | `CURRENT_ADDS` | 这是公网 active watch 的核心生命周期。 |

## 7. 架构、入口、部署与安全

| 当前 ID | 当前固定决定 | 与旧历史的关系 | P1 解释 |
|---|---|---|---|
| CUR-ARCH-01 | 目标形态是模块化单体：共享 React WebUI + Rust 共享核心。 | `CURRENT_SUPERSEDES` | 现有 Node/Fastify/TypeScript 后端是行为与迁移参考，不是最终后端目标。 |
| CUR-ARCH-02 | Windows 入口为 `bcsp-local.exe`；Linux 入口为 `bcsp-server`。 | `CURRENT_ADDS` | 旧 Node launcher 和多进程脚本不是最终双入口。 |
| CUR-ARCH-03 | 两条任务线共享 SQLite、集中 poller、WebSocket 相关核心。 | `CURRENT_REFRAMES` | P5 才决定具体共享、adapter、fork 和配置边界。 |
| CUR-ARCH-04 | Linux 生产侧使用 systemd 与 Caddy。 | `CURRENT_ADDS` | 旧本地启动和旧部署文档不能证明生产服务管理已完成。 |
| CUR-PLAT-01 | 公网目标环境为 Vultr EWR Linux VM；记录中的初始规格是 `$6/月` AMD High Performance、Ubuntu 24.04。 | `CURRENT_FIXED` + `P4_OR_P7_VERIFY` | 规格和真实外部状态在最终部署前必须重新核验。 |
| CUR-PLAT-02 | OCI 退出当前主路径；Sites、纯 Cloudflare Serverless、Cloud Run 不作为实时后端。 | `CURRENT_SUPERSEDES` | 旧云端/免费托管探索不再决定当前主路径。 |
| CUR-PLAT-03 | Cloudflare 仅可作为后续 DNS/CDN/代理层，不是主计算平台。 | `CURRENT_FIXED` | 域名、代理方式和生产状态仍是部署前待核验项。 |
| CUR-PLAT-04 | 当前本地一键包只支持Windows；macOS不进入当前版本或当前交付任务。 | `CURRENT_FIXED` + `CURRENT_SUPERSEDES` | 旧史料证明macOS曾是目标且缺少真实设备验证，但未找到旧的正式取消记录；当前取消是2026-07-12用户新决定。 |
| CUR-SEC-01 | SSH key、token、服务器清单和云凭据只存在于本地或平台 secret/env。 | `CURRENT_FIXED` | 不得进入 Git、公开文档或任一包。 |
| CUR-SEC-02 | 精确 IP、UUID、fingerprint 等私有 inventory 必须存入明确 Git-ignore 的位置。 | `CURRENT_FIXED` | P1 不创建或读取生产 secret。 |
| CUR-SEC-03 | 发布/部署前必须执行 secret scan 与 artifact audit。 | `CURRENT_ADDS` | 旧 release 已出现 runtime checkpoint 等泄漏风险，因此不能直接复用。 |

## 8. 阶段与裁决边界

| 当前 ID | 当前固定决定 | P1 约束 |
|---|---|---|
| CUR-GOV-01 | P1 只恢复、记录和合并。 | 不作 `all and only` 裁决。 |
| CUR-GOV-02 | 新决定可以覆盖旧决定，但旧决定仍须保留为可解释历史。 | 本文件明确使用覆盖标签而非删除历史。 |
| CUR-GOV-03 | P2 才审计本地一键包的 `all and only` 交付面。 | P1 不给遗留能力写最终保留/删除标签。 |
| CUR-GOV-04 | P3/P4 分别设计本地与公网实现；P5 分析共享/分化；P6 合并。 | 当前 overlay 不能越级定义 Rust 模块和 task 结构。 |
| CUR-GOV-05 | P1 完成后必须停在 P1 Review。 | 未经用户明确批准不得进入 P2。 |
| CUR-GOV-06 | P6完成后必须停在P6 Review。 | 用户明确批准最终执行计划后才能进入P7。 |
| CUR-GOV-07 | “每个实质task形成验证过的远端commit”只适用于P7。 | 不得扩散到0A-P6；不得制造空marker或remediation链。 |
| CUR-GOV-08 | GitHub Release是条件步骤。 | 两个包先通过测试、secret scan和artifact audit，且内容适合公开，才可发布。 |
| CUR-GOV-09 | P7.4只打两个包，不修改真实Vultr。 | 生产部署前必须重新取得用户授权并核验外部状态。 |
| CUR-GOV-10 | 2026-07-12用户接受SAFE-INC-01/02并批准P1。 | P1已关闭；P2成为下一可执行阶段，但本次批准没有自动启动P2。 |

## 9. 当前覆盖关系总览

| 旧能力域 | 当前关系 | 仍保留给 P2及后续阶段的问题 |
|---|---|---|
| Node/Fastify 本地后端 | `CURRENT_SUPERSEDES` 为 Rust 共享核心 | 哪些行为契约、数据模型和错误语义需要迁移，由 P2-P5 决定。 |
| React 课程浏览与筛选 | `HISTORY_COMPATIBLE`，但 UI/UX 要重建 | course-centered、独立section详情和严格meeting规则已固定；其余字段、未知时间、移动端和状态UI由P2/P3细化。 |
| 独立sections contract | `CURRENT_FIXED` 为独立搜索/访问/完整详情能力 | 旧空stub不可保留为假表面；API形状和与course expansion的边界由P2/P3设计。 |
| Calendar | 当前核心版本明确不包含，只作future feature | 历史用户需求保留，但当前不实施且不用于替代CSP。 |
| SQLite 课程/section 数据 | `CURRENT_REFRAMES` 为双入口共享核心 | schema、键、迁移、数据目录和生命周期需要重新设计。 |
| 旧持久 subscriptions | `CURRENT_SUPERSEDES` 为 live WebSocket watch，但保留subscription management用户能力 | 旧API、表、UI、测试和依赖如何处理，以及新管理界面如何表达live状态，属于P2/P3。 |
| Closed→Open event + dedupe + fan-out | `CURRENT_SUPERSEDES` 为每条 Open 消息触发 | 数据采样、网络重放和工程级幂等仍需后续设计。 |
| 浏览器 HTTP claim + 固定短音 | `CURRENT_SUPERSEDES` 为 WebSocket + 音量/模式 | WebUI toast与Max notifications已确认需要；精确计数、上限动作和重连语义待P2/P3。 |
| SendGrid/SMTP/邮件 UI/worker | `CURRENT_SUPERSEDES`，当前版本不含邮件 | P2 必须审计其专属 UI、route、worker、config、docs、tests、deps；P1 不执行删除。 |
| Discord | 当前未纳入且禁止新增另一通知产品 | 用户确认当时主动删除以降低复杂度；不恢复为当前目标。 |
| 一键脚本与旧 release | `CURRENT_REFRAMES` 为 Windows `bcsp-local.exe` 一键包 | 旧启动错误处理和跨平台教训可迁移；旧包不可直接复用。 |
| server scheduled SOC fetch | `CURRENT_REFRAMES` 为课程目录默认10分钟策略 | 本地配置UX与公网固定策略的准确实现由P3/P4决定。 |
| browser auto-refresh / query revalidation | `P2_UNRESOLVED` | 它不抓Rutgers，只重新请求本服务；是否需要toggle、何时失效cache由P2/P3审计。 |
| macOS launcher | `CURRENT_SUPERSEDES`：当前只支持Windows | 历史存在/失败事实保留；未来是否重启macOS支持不预设，必须另行决定并有真实设备验证。 |
| 邮件/Discord/旧云部署文档 | 当前明确不构成现版本产品 | 历史仍解释代码、依赖和 release 漂移。 |

## 10. P1 Review 后仍保留给后续阶段的事项

P1 Review已经解决独立section路径、严格meeting核心规则、Calendar、当前macOS边界、subscription management、toast/Max notifications与Discord原因。下列细节仍不得在P1补猜：

1. 旧筛选字段的最终all-and-only集合、被删字段是否恢复，以及async/TBA/hybrid/optional/exam的严格时间规则。
2. 独立section API、course expansion与完整详情页的最终contract边界。
3. Compact view、saved views、share links的历史来源与当前去向。
4. 旧server scheduled fetch的哪些行为映射到10分钟目录策略，以及独立的browser query refresh/cache UX是否需要保留。
5. toast/Max notifications的精确计数、达到上限后的动作、重连行为，以及subscription history/quiet hours/snooze等旧偏好的去向。
6. 旧实现中任何标为 `complete/recover/repair/remove/defer/unclear` 的分类是否仍成立。

这些事项随获批P1进入P2及后续设计/验证阶段；本文件不提前作all-and-only裁决。
