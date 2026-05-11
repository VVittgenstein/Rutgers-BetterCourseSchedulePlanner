## file: {{2025-11-11-dr.md}}  
project: "{{BetterCourseSchedulePlanner}}"  
version: 0.1  
date: {{2025-11-13T00:11:00}}  
derived_from: "Deep Research (raw notes in context)"

# {{PROJECT_TEXT}} · DR 结论（只读）

> 本文是从上游调研原文蒸馏的**结论性文档**，用于人审阅与 6b 机读抽取。  
> 约定：文中方括号标注 [EVID-###] 以指向“参考文献”。

## 1. 背景&目标

- 目标（What/Why）：构建一个基于 **Rutgers Schedule of Classes (SOC) 公开数据** 的选课查询与空位提醒网站，在“浏览和筛选课程 + 订阅空位提醒”层面取代官方 CSP 与 WebReg 的体验，提供更细粒度的筛选能力、实时性更高且成本更低的通知，并支持开源与一键部署。[EVID-001][EVID-002]
    
- 成功判据（对 MVP 的可验证结果）：[EVID-001][EVID-006][EVID-007]
    
    - 课程筛选覆盖 ≥95% 官方课程字段（课程名、代码、学分、院系、先修、核心代码、Index、Section 状态、时间地点、授课模式等），支持多条件组合过滤。
        
    - 在约 1000 门课程规模下，常见筛选操作端到端响应时间 < 0.5 秒；首屏加载（静态资源 + 数据） < 3 秒；在 ≥200 并发浏览情况下无明显性能劣化（依托静态站点 + CDN）。
        
    - 监控课程空位变更时，从“Closed → Open”到邮件/Discord 提醒发出平均延迟 < 30 秒，最坏不超过 60 秒，漏报率为 0。
        
    - UI 提供课程时间的可视化展示（至少支持按星期/时间段直观查看），经小规模可用性测试，目标用户满意度 ≥90%。
        
    - 项目在 GitHub 上完全开源，附带清晰的一键部署文档，使一般开发者在 <1 小时内完成部署。
        
- 非目标 / 暂不覆盖：[EVID-001]
    
    - 不实现任何登录/账号体系，不接入 WebReg 执行自动选课，用户仍需在 WebReg 手动输入 Index 完成加课/退课。
        
    - 不实现完整的学位规划、毕业审核或复杂的先修关系校验，仅在 UI 展示先修与核心课标签。
        
    - 不处理除邮箱地址、Discord 用户/频道标识之外的任何个人敏感信息。
        
    - 不承担对 Rutgers 官方数据的权威解释责任；课程数据以官方 SOC 为准，本项目仅做“镜像+增强 UI + 通知”。
        

## 2. 结论总览（Top Insights）

- (C1) **架构层面推荐采用“静态前端 + 云函数通知”的方案 B**：课程数据离线抓取为 JSON，由前端在浏览器内完成筛选；空位通知由无服务器函数轮询 Rutgers SOC/openSections 并通过邮件/Discord 推送，在高并发场景下几乎不增加服务器负载，且可依赖 GitHub Pages/Netlify 等低成本托管平台。[EVID-001][EVID-006][EVID-007]
    
- (C2) **Rutgers SOC 公开 API（含 courses.json + openSections）可提供实现所需的全部课程与余量字段**，但文档有限，需要自行封装和容错；这决定了数据层必须设计为可替换的数据抓取模块，以应对接口变更或限流风险。⚠️（接口说明来源较旧）[EVID-001][EVID-002]
    
- (C3) **课程“狙击/空位提醒”在 Rutgers 学生群体中已有成熟先例（Lightning、Sniper、SwiftRU 等），验证了“监控 SOC + 邮件/Discord 通知”的可行性和用户价值**，但现有工具多为闭源、单一渠道或已停止维护，因此新项目应在保留高实时性的基础上，提供更友好的 Web UI 与开源自托管能力。[EVID-001][EVID-003][EVID-004][EVID-005]
    

## 3. 关键决策与约束（Decision & Constraints）

> 6b 将据此拆出决策/约束类任务与校验。

- D1. 决策：**采用方案 B：前端静态站点 + 云函数/定时任务通知**，而非自建长期运行的数据库后端（方案 A）或深度改造 QuACS/YACS 等现有项目（方案 C）。适用范围：整个 v1.x 阶段；若未来需求超出（如复杂课表推荐算法），再评估演进方案。[EVID-001][EVID-006][EVID-007]
    
- D2. 决策：**课程数据来源统一使用 Rutgers SOC 公开接口（courses.json / openSections 等）**，禁止直接抓取 WebReg 或需要登录的系统页面。适用范围：所有课程列表与空位信息。[EVID-001][EVID-002]
    
- D3. 决策：**筛选逻辑尽可能全部在客户端完成**（浏览器内对 JSON 做过滤与排序），后端仅负责数据抓取与通知，不提供复杂查询 API，以减少服务器成本与运维复杂度。[EVID-001][EVID-006][EVID-007]
    
- D4. 决策：**通知渠道优先支持邮箱与 Discord**：邮件通过类似 SendGrid 的商用 API 实现；Discord 通过 Bot API 在私信或频道发送提醒。短信等其他渠道暂不支持，未来根据成本和需求另行评估。[EVID-001][EVID-003][EVID-004][EVID-005]
    
- D5. 决策：**项目从 day-1 即按多语言架构设计（至少中英）**，通过标准 i18n 库管理所有文案，确保后续扩展其他语言时不用大改代码结构。[EVID-001][EVID-008]
    
- Z1. 约束：**不做登录、不做自动选课、不要求用户提供 NetID 或任何学号信息**；通知仅基于用户自填的邮箱地址或 Discord ID。此为长期约束，除非未来合规评估与用户需求发生根本变化。[EVID-001]
    
- Z2. 约束：**严格限定数据来源为“公开可访问的 Rutgers 课程信息”**，不得突破访问控制或模拟 WebReg 会话；如 SOC API 访问策略变化，必须优先寻求官方允许的用法（例如降低调用频率或通过 HTML 页面解析）。⚠️（SOC API 文档缺失）[EVID-001][EVID-002]
    
- Z3. 约束：**运营成本需长期控制在“基本免费 / 极低开销”**：前端托管优先选 GitHub Pages/Netlify 免费方案；云函数与邮件服务遵循免费额度上限设计轮询频率与用户规模。[EVID-001][EVID-003][EVID-006][EVID-007]
    
- Z4. 约束：**必须满足基础隐私与反垃圾邮件规范**：仅在用户主动订阅后发送通知，所有邮件提供显式退订路径；Discord Bot 使用频率与权限需符合平台政策，避免封禁风险。[EVID-001][EVID-003][EVID-004][EVID-005]
    

## 4. 需求提炼（可执行）

> 每条均含“验收要点(acceptance-hint)”便于 6b 生成 acceptance。

### 4.1 功能性需求（FR）

- FR-01：作为 Rutgers 学生，我在选课网站选择**学期 + 校区**后，可以看到该组合下的**完整课程与 Section 列表**，包括课程标题、课程代码、学分、院系、Index、Section 号、教师、状态、时间地点、核心代码、先修、授课模式等关键字段。[EVID-001][EVID-002]  
    验收要点：对指定学期（如 2025 Fall NB），随机抽样 ≥10 门课，与官方 SOC 页面逐条核对，确保课程数量和字段完整性一致，且 Section 信息不缺失。
    
- FR-02：我可以在同一界面上，对课程进行**多维组合筛选**，至少支持：课程标题/代码关键字、学分范围、开设学院/院系、是否有先修/核心要求、核心代码（如 WCr/CCO）、Index/Section、教师、Section 状态（Open/Closed/Waitlist）、上课星期（周一~周日）、上课起止时间区间、校区/教学楼/教室、授课模式（In-Person/Online/Hybrid）、是否有期末考试代码、是否为 Cross-listed 课程。[EVID-001][EVID-002][EVID-006]  
    验收要点：为每个筛选字段设计 ≥1 组测试用例，验证单条件与多条件组合的结果与手工筛选一致（包括“无结果”情况）；对 1000 条课程数据进行常见查询时，本地筛选函数执行时间 < 200ms。
    
- FR-03：我可以在页面中切换到**周视图/日历视图**，查看符合当前筛选条件的课程时间分布；MVP 至少支持“按星期几 + 起始时间段”过滤课程。更复杂的“框选空闲时间找课程”逻辑可在后续版本实现，但需在数据模型中预留支持（Section 的 meeting time 为结构化字段）。[EVID-001]  
    验收要点：在测试数据中构造多门时间有交集/无交集课程，验证日历视图与列表视图展示内容一致；通过 UI 选择“只看周一 9:00–12:00 上课”时，所有返回课程的上课时间均落在此范围内。
    
- FR-04：我可以在课程列表或详情中，对某个具体 Section（Index）点击“订阅空位提醒”，选择通知方式（邮箱/Discord），并提交订阅；系统对输入格式做基本校验（邮箱格式、Discord 用户/频道标识）并反馈订阅成功或失败原因。[EVID-001][EVID-003][EVID-005]  
    验收要点：在前端使用假数据和本地 Mock API 的情况下，验证：合法输入可成功创建订阅记录；非法输入（邮箱格式错误等）会给出明确错误提示；重复订阅同一个 Section 时行为符合设计（例如提示已订阅或允许多渠道订阅）。
    
- FR-05：当被订阅的 Section 的状态从“Closed”变为“Open”时，系统会在约定的轮询周期内自动检测到变化，并通过邮箱或 Discord 发送至少一次通知（消息中包含课程名、Index、Section、时间地点等关键信息）。默认策略为“首次开放时发送一次通知”，是否持续监控为后续可配置项（见开放问题）。[EVID-001][EVID-003][EVID-004][EVID-005]  
    验收要点：在测试环境中通过模拟数据切换 Section 状态（Closed→Open），记录从状态变化到用户收到通知的时间；连续多次实验平均延迟 < 30 秒，最大延迟 < 60 秒；同一开放事件同一渠道只收到 1 条通知。
    
- FR-06：我可以通过邮件中的“退订/管理订阅”链接或网站提供的界面，查看并取消自己对某些 Index 的订阅；退订后不再收到与该 Index 相关的任何后续通知。[EVID-001]  
    验收要点：创建多个订阅后，通过退订入口删除其中 1 个，验证后续模拟开放时只对未退订的订阅发送通知；退订链接无需登录即可识别订阅（例如通过签名 Token），但不可被轻易猜测或篡改。
    
- FR-07：用户可以在界面中切换 UI 语言（至少支持中英文），切换后所有静态文案（菜单、按钮、字段说明等）立即更新；课程数据本身保持原始英文（例如 Course Title/Instructor 名称），但可在需要时为核心代码、字段标签提供本地化说明。[EVID-001][EVID-008]  
    验收要点：在浏览器中反复切换语言，确保界面无“混合语言”或乱码；新增功能时通过 i18n 资源文件自动检测缺失翻译（如构建时报警），确保多语言版本保持同步。
    
- FR-08：项目仓库提供**一键部署**能力：至少包括对 GitHub Pages/Netlify 部署前端和对某一云函数平台部署通知服务的脚本或说明，使第三方开发者在不修改代码的前提下即可部署自己的实例。[EVID-001][EVID-006][EVID-007]  
    验收要点：选取 1 名未参与开发的工程师，按照 README 部署指南在全新环境中操作，在 ≤1 小时内成功获得可访问的前端站点和可用的通知服务（使用测试 API 密钥）。
    

### 4.2 非功能性需求（NFR）

- NFR-01（性能）：前端在典型网络条件下（50Mbps 下载、北美地区）首次访问指定学期课程列表时，总加载时间 < 3 秒；数据量约 1000 门课时，任意组合筛选交互（包括重新排序）在用户端反馈 < 0.5 秒。[EVID-001][EVID-006]  
    验收要点：使用浏览器性能工具和合成数据进行测试，记录首屏加载时间与筛选响应时间，确保达到 SLO。
    
- NFR-02（通知延迟）：在 SOC 数据正常的前提下，通知服务的平均延迟 < 30 秒，峰值不超过 60 秒；系统对轮询任务失败具备重试与告警机制，确保长期运行稳定。[EVID-001][EVID-003][EVID-005]  
    验收要点：在测试环境连续运行轮询任务 ≥1 周，模拟多次课程状态变更，统计通知延迟分布与失败重试情况。
    
- NFR-03（可扩展性与并发）：系统需在不增加服务器资源的前提下支撑≥200 并发用户浏览课程（静态资源依赖 CDN 横向扩展），通知服务可通过增加云函数并发或拆分任务队列扩容，避免成为瓶颈。[EVID-001][EVID-006][EVID-007]  
    验收要点：通过负载测试模拟并发访问和高频订阅，观测前端响应时间与云函数执行错误率不显著上升。
    
- NFR-04（成本）：以免费/低价服务为基准设计轮询频率与邮件发送策略，使得在合理用户规模下（例如活跃订阅数 < 5000）整体云资源费用保持在免费额度或极低月费。SendGrid 免费每日 100 封邮件等额度作为参考上限。[EVID-001][EVID-003][EVID-007]  
    验收要点：基于实际订阅数据与轮询参数估算 API 调用与邮件量，在账单中验证费用符合预期。
    
- NFR-05（安全与隐私）：系统不存储除订阅必需的联系信息（邮箱/Discord 标识）以外的任何个人数据，不使用 Cookie 跟踪用户行为；所有敏感配置（API 密钥）仅存于服务端环境变量，前端仓库不包含任何密钥。[EVID-001]  
    验收要点：安全审计确认前端代码库中无硬编码密钥；数据存储结构中不包含多余个人信息；退订后数据删除符合设计。
    
- NFR-06（可维护性与开源治理）：代码需通过基本 Lint 与单元测试，具备 ≥80% 核心逻辑覆盖率；仓库提供贡献指南和发布流程，方便社区参与与后续维护。[EVID-001]  
    验收要点：CI 管线在 PR 时自动运行 Lint 与测试；新开发者在 1 天内可通过文档理解架构并完成一次小改动。
    

## 5. 技术路径与方案对比（若适用）

|方案|适用场景|优点|风险/代价|证据|
|---|---|---|---|---|
|A：后端数据库 + 动态服务|需要复杂服务端逻辑、长生命周期数据以及潜在的登录/权限扩展时|经典三层架构，可在数据库层实现复杂查询与统计；后续扩展为完整教务系统较自然。[EVID-001]|需长期维护服务器与数据库，月度费用较高；对 1–2 人团队开发压力大；容易因数据库扩展与备份变成运维负担。[EVID-001][EVID-007]|[EVID-001][EVID-007]|
|B：静态前端 + 云函数通知（推荐）|以“浏览课程 + 订阅空位提醒”为主、无登录、追求低成本高并发|前端与课程数据完全静态，可托管在 GitHub Pages/Netlify 上，通过 CDN 支撑高并发，成本极低；课程筛选逻辑在浏览器执行，延迟极低；通知服务以云函数实现，按量计费、易于扩缩容。[EVID-001][EVID-006][EVID-007]|需设计数据抓取与静态构建流程；对 SOC API 变动敏感；在极端订阅量下需仔细控制轮询频率以避免触达第三方服务限额。[EVID-001][EVID-002]|[EVID-001][EVID-002][EVID-006][EVID-007]|
|C：改造现有开源项目（如 QuACS + Sniper）|团队熟悉对应开源项目且希望快速搭建基础功能|可复用成熟 UI 与部分后端代码；QuACS 验证了纯静态浏览器侧排课的性能与体验；Sniper/Lightning 等项目提供可参考的通知实现。[EVID-001][EVID-003][EVID-005][EVID-006]|需要深入理解并重构他人代码以适配 Rutgers 数据模型；QuACS 的 Rust/WASM 代码栈有学习成本；不同项目整合会引入额外耦合和维护风险。[EVID-001][EVID-006]|[EVID-001][EVID-003][EVID-005][EVID-006]|

> 推荐：在 v1.x 阶段采用 **方案 B：静态前端 + 云函数通知**。该方案已被 QuACS 等项目证明在课程筛选场景下具备高性能和易扩展性，同时结合 Lightning/Sniper 等项目实践，可低成本实现 SOC 轮询 + 邮件/消息通知；对当前“低运维成本、无登录、注重 UI 体验”的目标最为契合。[EVID-001][EVID-003][EVID-005][EVID-006][EVID-007]

## 6. 外部依赖与阻断

- Rutgers SOC API（courses.json/openSections）：提供课程与空位的官方数据来源，目前可公开访问但文档有限，速率限制未知，长期稳定性存疑 → 现状：**unknown（可用但⚠️风险）** → 若接口结构或访问策略变更，将直接影响课程更新与通知准确性。[EVID-001][EVID-002]  
    最小解锁路径：封装统一数据访问模块；实现调用失败的降级与告警；必要时支持从 HTML 页面解析作为备选方案。
    
- 邮件服务（如 SendGrid）：用于发送课程开放提醒邮件 → 现状：**unblocked**（有免费额度可用） → 若超出免费额度则需要调整发送策略或更换服务商。[EVID-001][EVID-003]  
    最小解锁路径：在配置层支持替换邮件服务商；对批量邮件增加聚合逻辑（一次邮件包含多门课的提醒）以节省额度。
    
- Discord Bot API：用于发送课程开放提醒的即时消息 → 现状：**unblocked**，但需遵守速率限制与内容政策 → 过度频繁或模式化的消息可能触发反垃圾机制。[EVID-001][EVID-004]  
    最小解锁路径：对 Bot 消息发送做节流与重试；必要时改用频道广播代替大量私信。
    
- 静态站点托管平台（GitHub Pages/Netlify）：提供前端静态资源托管与 CDN 分发 → 现状：**unblocked**，成本极低 → 若未来访问量极大，可考虑迁移到更专业的 CDN，但不影响短期实现。[EVID-001][EVID-006][EVID-007]
    

目前不存在明显的硬性“blocked”依赖，但 **SOC API 的非正式性** 是最大外部不确定因素，需要在开发与运维流程中重点监控。[EVID-001][EVID-002]

## 7. 风险清单（含缓解）

- R-01：Rutgers SOC API 发生变更或限流（概率：中；影响：高）[EVID-001][EVID-002]
    
    - 监测指标：API 调用成功率、返回结构校验结果、轮询任务错误日志。
        
    - 触发器：短时间内连续失败或数据字段缺失。
        
    - 缓解措施：封装数据访问层，提供结构版本检测与快速回滚；实现请求失败重试与退避策略；预备基于 HTML 页面解析的备选抓取方式；限制抓取频率并避免暴力轮询。
        
- R-02：通知延迟或漏报（概率：中；影响：高）[EVID-001][EVID-003][EVID-005]
    
    - 监测指标：从状态变更到通知发出的时间分布、通知失败/重试次数、用户投诉。
        
    - 触发器：平均延迟大幅上升、出现漏发或重复发送。
        
    - 缓解措施：合理设置轮询周期（如 30–60 秒），优先使用 openSections 等批量接口；对关键任务设置心跳与告警；在通知管道中实现幂等逻辑避免重复通知。
        
- R-03：前端性能在低端设备或大量数据下退化（概率：中；影响：中）[EVID-001][EVID-006]
    
    - 监测指标：客户端渲染与筛选耗时、DOM 节点数量、浏览器错误日志。
        
    - 触发器：用户明显感知卡顿或筛选超时。
        
    - 缓解措施：采用虚拟列表、分页或延迟渲染技术；对筛选逻辑进行优化（尽量 O(n) 扫描、不做多余排序）；必要时引入 Web Worker 或 WASM 模块。
        
- R-04：邮件/Discord 被视为垃圾信息（概率：中；影响：中）[EVID-001][EVID-003][EVID-004][EVID-005]
    
    - 监测指标：邮件退信率、Spam 标记率、Discord API 错误码（速率限制、封禁）。
        
    - 触发器：短时间内大量失败或平台警告。
        
    - 缓解措施：使用成熟邮件服务，控制发送频率与内容个性化；邮件正文内附退订链接和订阅信息说明；Discord 侧设置合理的节流策略，将部分通知集中到频道而非大量私信。
        
- R-05：多语言内容维护滞后（概率：低；影响：中）[EVID-001][EVID-008]
    
    - 监测指标：构建时 i18n 缺失统计、用户反馈中的“某些界面未翻译”。
        
    - 触发器：新增功能上线时部分语言未更新。
        
    - 缓解措施：所有界面文案必须走 i18n 资源文件；在 CI 中加入缺失翻译检查；对未翻译条目统一展示英文并标记 TODO。
        
- R-06：开源治理负担与维护中断（概率：中；影响：中）[EVID-001]
    
    - 监测指标：Issue/PR 堆积情况、依赖安全告警数量。
        
    - 触发器：长期无维护者响应或安全漏洞未修复。
        
    - 缓解措施：制定贡献指南与代码规范；使用自动化工具处理部分维护（依赖升级、Lint）；鼓励社区维护者加入协作；明确 README 中的“官方维护实例”与“第三方部署”界限。
        

## 8. 开放问题（需要结论的人/时间）

- Q-01：**SOC API 抓取策略**：是否存在官方支持的“一次抓取全校课程”的参数组合（如 subject=ALL），或必须按院系逐一请求？若仅支持按院系请求，如何在不触发限流的前提下高效抓完所有数据？
    
    - 责任人：后端/数据抓取负责人（TBD）
        
    - 需要的证据：实际调用测试结果，或 Rutgers IT 文档/答复。[EVID-001][EVID-002]
        
    - 期望截止：待项目立项后尽早确定，用于指导抓取脚本设计。
        
- Q-02：**课程列表与空位信息的更新频率**：静态课程基础信息（名称、描述、学分等）计划每日更新一次是否足够？是否需要在选课高峰期提高频率？
    
    - 责任人：产品负责人 + 后端负责人
        
    - 需要的证据：历史学期课程变更频率统计或经验；用户对实时性的期望调研。[EVID-001]
        
    - 期望截止：在 MVP 上线前确定，以便配置自动化更新任务。
        
- Q-03：**订阅通知策略**：当课程反复在“满员/有空位”之间切换时，是只在首次开放时通知一次并自动删除订阅，还是每次开放都通知，或提供用户可配置的策略？
    
    - 责任人：产品负责人
        
    - 需要的证据：用户访谈与使用场景分析；第三方工具（SwiftRU、Schedru 等）实践对比。[EVID-001][EVID-004]
        
    - 期望截止：在通知模块设计前确定，以避免后期大改。
        
- Q-04：**Discord 通知形式**：使用 Bot 私信还是频道广播？如何在保证送达率的同时控制噪音与权限要求？
    
    - 责任人：后端/DevOps 负责人
        
    - 需要的证据：Discord Bot API 政策、SwiftRU 等项目经验、内部测试数据。[EVID-001][EVID-004]
        
    - 期望截止：在 Discord 通知模块开发前确定。
        
- Q-05：**周历“框选空闲时间找课”的业务语义**：用户框选一段时间时，是筛选所有“上课时间完全落在该范围”的课程，还是筛选“与该时间段不冲突的所有课程”（即找到空闲时间可选的课程）？
    
    - 责任人：产品负责人
        
    - 需要的证据：用户调研对该功能的期望；CSP 等工具的行为对比。[EVID-001]
        
    - 期望截止：可延后至 v1.1/v2.0 设计前决定，MVP 先实现简单时间过滤。
        
- Q-06：**多校区与研究生课程支持范围**：MVP 是否仅支持 New Brunswick 本科课程？何时扩展到 Newark/Camden 以及研究生课程（level=GR）？
    
    - 责任人：产品负责人
        
    - 需要的证据：目标用户构成与优先级；SOC API 在不同校区/层次的表现。[EVID-001][EVID-002]
        
    - 期望截止：MVP 规划阶段确认范围，以便数据抓取与 UI 设计。
        

## 9. 术语与域模型（可用于统一命名）

- 术语定义：[EVID-001][EVID-002]
    
    - **CSP (Course Schedule Planner)**：Rutgers 官方课表规划工具；用于组合课程生成不冲突的时间表并将结果导入 WebReg。
        
    - **WebReg**：Rutgers 官方选课注册系统；通过输入 Index 号完成加课/退课，不负责课程搜索。
        
    - **SOC (Schedule of Classes)**：官方课程目录系统，公开提供按学期/院系统一查询的课程数据；本项目的主要数据源。
        
    - **Course（课程）**：对应某门课的逻辑实体，包含课程代码、标题、描述、学分、核心代码等。
        
    - **Section（班次）**：课程在某个学期的具体开课实例，具有唯一的 Index 号、Section 号、教师及时间地点安排。
        
    - **Index**：用于在 WebReg 中注册课程的 5 位数字索引，每个 Section 唯一。
        
    - **Core Code（核心课程代码）**：Rutgers 通识要求中的标签（如 WCr、CCO 等），表示课程满足哪些通识要求。
        
    - **Cross-listed Course（交叉课程）**：同一门课被列在多个院系/课程代码下的情况。
        
    - **Exam Code**：用于期末考试安排的代码字段。
        
    - **Mode of Instruction（授课模式）**：In-Person/Online/Hybrid 等授课形式。
        
- 域模型（简要对象/关系）：[EVID-001][EVID-006]
    
    - Student（用户，无登录实体）
        
    - Course：包含多个 Section。
        
    - Section：关联一个 Course，包含多个 Meeting（上课时间段）及状态、容量信息。
        
    - Subscription：用户对某个 Section 的订阅，包含通知渠道与状态。
        
    - Notification：订阅被触发后产生的消息记录（可选是否长期存储）。
        
    - DataSource：对 SOC API 的抽象封装，提供按学期/院系的 Course+Section 列表以及 openSections 座位信息。
        
    - 前端以 `term + campus` 为主键加载课程数据；Subscription 以 `sectionIndex + contact` 为主键管理。
        

## 10. 证据一致性与时效

- 互相矛盾点：当前证据中未出现明显互相矛盾的结论；但某些第三方工具（如 TrackRU、Schedru 等）是否仍在维护存在不确定性，且不同工具声称的通知延迟存在差异。由于我们不直接依赖这些工具的服务，而是仅借鉴架构与实现思路，对核心决策影响有限。[EVID-001][EVID-003][EVID-004][EVID-005]
    
- 证据时效：
    
    - **⚠️ 较旧证据**：Rutgers Course API README 等关于 SOC 接口的描述来自约 2018 年；Sniper 项目 README 大致来自 2017 年，SwiftRU 信息来自 2022 年。[EVID-002⚠️][EVID-005⚠️][EVID-004⚠️]
        
    - **较新证据**：关于静态站与成本、React i18n 等资料在 2023–2024 年，时效性较好。[EVID-006][EVID-007][EVID-008]
        
    - 结论：与 SOC API 相关的实现细节在落地前需要通过实际调用做一次“二次验证”；静态站架构与多语言最佳实践可以认为是当前业界共识。
        
- 数据缺口：
    
    - SOC API 的正式文档与限流策略尚未明确，需要进一步确认调用边界。[EVID-002]
        
    - SendGrid 免费额度、Discord 速率限制等细节需要以官方最新文档为准定期复核。[EVID-001]
        
    - Rutgers 各校区/研究生课程在 SOC 数据结构上的差异尚未系统验证，对“全校覆盖”的规划存在信息缺口。[EVID-001][EVID-002]
        

---

## 11. Action Seeds（供 6b 机读转 JSON；YAML，不等于最终任务）

```yaml
action_seeds:
  - id: ACT-001
    title: "封装 Rutgers SOC 数据抓取与静态 JSON 生成脚本"
    category: build
    rationale: "为静态前端提供完整、结构化的课程与 Section 数据，并验证 SOC API 在当前学期的可用性与字段覆盖情况。"
    evidence: ["EVID-001","EVID-002"]
    acceptance_hint: "支持按 term+campus 批量抓取课程并生成 JSON；对至少一个学期完成抓取并通过人工抽样验证字段完整性。"
    priority_guess: P0
    depends_on: ["DEP-001"]

  - id: ACT-002
    title: "实现基于静态 JSON 的课程列表与多维筛选前端（MVP）"
    category: build
    rationale: "在浏览器端完成主要筛选逻辑，以获得低延迟体验并削减服务器负载。"
    evidence: ["EVID-001","EVID-006","EVID-007"]
    acceptance_hint: "在本地加载约 1000 条课程记录时，完成 FR-01/FR-02/FR-03 定义的筛选能力，常见操作响应 <0.5 秒。"
    priority_guess: P0
    depends_on: ["ACT-001"]

  - id: ACT-003
    title: "设计并上线邮件通知云函数（基于 SOC openSections）"
    category: build
    rationale: "实现从课程空位变化到邮件提醒的闭环，满足核心“狙击”价值。"
    evidence: ["EVID-001","EVID-003","EVID-005"]
    acceptance_hint: "在测试环境模拟 Closed→Open 场景，邮件通知平均延迟 <30 秒且无漏报；支持退订后不再发送。"
    priority_guess: P0
    depends_on: ["DEP-001","DEP-002","ACT-001"]

  - id: ACT-004
    title: "实现 Discord 通知通道并评估私信与频道策略"
    category: build
    rationale: "提供即时性更强的通知方式，并探索与社区工具（如 SwiftRU）类似的使用体验。"
    evidence: ["EVID-001","EVID-004"]
    acceptance_hint: "在测试服务器中，通过 Bot 成功向指定用户或频道发送通知消息，遵守速率限制且未触发平台警告。"
    priority_guess: P1
    depends_on: ["DEP-001","DEP-003","ACT-001"]

  - id: ACT-005
    title: "搭建前端 i18n 架构并完成中英文界面"
    category: build
    rationale: "从架构上支持多语言扩展，满足不同语言背景学生的使用需求。"
    evidence: ["EVID-001","EVID-008"]
    acceptance_hint: "所有 UI 文案通过 i18n 资源文件管理；在中英切换时界面无缺失翻译或乱码，新增页面时自动提示需要翻译的 key。"
    priority_guess: P1
    depends_on: ["ACT-002"]

  - id: ACT-006
    title: "编写并验证开源一键部署文档与 CI/CD 流程"
    category: doc
    rationale: "降低外部贡献者与自托管用户的使用门槛，实现“一键部署”目标。"
    evidence: ["EVID-001","EVID-006","EVID-007"]
    acceptance_hint: "第三方开发者按 README 指引能在 <1 小时内完成部署；CI 能在 push 时自动构建并部署前端/云函数。"
    priority_guess: P1
    depends_on: ["ACT-001","ACT-002","ACT-003"]

```

## 12.External Dependencies（供 6b 识别依赖态；YAML）

```yaml
external_dependencies:
  - id: DEP-001
    name: "Rutgers SOC API（courses.json / openSections）"
    status: unknown
    blocker: "接口无正式公开文档，速率限制与长期稳定性未知；任何结构或访问策略变更都会影响数据抓取与通知准确性。"
    unblock_plan: "通过实际调用摸索可用参数与频率；为数据访问层增加版本检测与快速回滚；准备 HTML 解析等备选方案。"
    evidence: ["EVID-001","EVID-002"]

  - id: DEP-002
    name: "SendGrid 或同类邮件发送服务"
    status: unblocked
    blocker: "免费额度有限，若用户规模增长可能超限；需要正确配置发件域名以降低被判为垃圾邮件的风险。"
    unblock_plan: "在配置层支持更换邮件服务商；对通知进行聚合以减少邮件数；如有需要升级到付费套餐。"
    evidence: ["EVID-001","EVID-003"]

  - id: DEP-003
    name: "Discord Bot API"
    status: unblocked
    blocker: "受速率限制和内容政策约束，过于频繁或模板化的消息可能被限流或封禁。"
    unblock_plan: "按官方文档设置合理的节流与重试；在设计上避免大量重复私信，可考虑频道广播或摘要消息。"
    evidence: ["EVID-001","EVID-004"]

  - id: DEP-004
    name: "静态前端托管平台（GitHub Pages / Netlify 等）"
    status: unblocked
    blocker: "若未来访问量或功能需求超出免费/轻量级平台能力，可能需要迁移到更专业的托管与 CDN。"
    unblock_plan: "使用标准化构建产物（纯静态文件），以便日后迁移到任意静态托管/CDN；在文档中保留多种部署方式。"
    evidence: ["EVID-001","EVID-006","EVID-007"]

```

## 13.参考文献（附来源日期）

统一 ISO-8601；若发布日期缺失，写 published: unknown。

- [EVID-001] 2025-11-11-dr 调研报告 — 项目组 — published: 2025-11-11 — accessed: 2025-11-12 — URL: internal://2025-11-11-dr.md
    
- [EVID-002] Rutgers Course API (README) — David Parsons / Rutgers CS — published: unknown — accessed: 2025-11-11 — URL: [https://github.com/anxious-engineer/Rutgers-Course-API](https://github.com/anxious-engineer/Rutgers-Course-API)
    
- [EVID-003] Lightning: Course Sniper for Rutgers Schedule of Classes (GitHub README) — Anitej Biradar — published: 2020-12-03 — accessed: 2025-11-11 — URL: [https://github.com/anitejb/lightning](https://github.com/anitejb/lightning)
    
- [EVID-004] SwiftRU: an optimal solution for course sniping at Rutgers（Reddit 帖文） — hattvr — published: 2022-05-01 — accessed: 2025-11-10 — URL: [https://www.reddit.com/r/rutgers/comments/ug7hvc/swiftru_an_optimal_solution_for_course_sniping_at/](https://www.reddit.com/r/rutgers/comments/ug7hvc/swiftru_an_optimal_solution_for_course_sniping_at/)
    
- [EVID-005] Sniper: Rutgers Course Sniper (GitHub README) — Rui Zhang 等 — published: unknown — accessed: 2025-11-11 — URL: [https://github.com/v/sniper](https://github.com/v/sniper)
    
- [EVID-006] QuACS – Questionably Accurate Course Scheduler (README) — RCOS / RPI — published: 2021-08-15 — accessed: 2025-11-11 — URL: [https://github.com/quacs/quacs](https://github.com/quacs/quacs)
    
- [EVID-007] How Much Does a Static and Dynamic Website Cost? — Alena James / Dev.to — published: 2023-08-01 — accessed: 2025-11-11 — URL: [https://dev.to/alenajames/how-much-does-a-static-and-dynamic-website-cost-3lk4](https://dev.to/alenajames/how-much-does-a-static-and-dynamic-website-cost-3lk4)
    
- [EVID-008] How to Build Multilingual Apps with i18n in React — freeCodeCamp — published: 2024-12-04 — accessed: 2025-11-11 — URL: [https://www.freecodecamp.org/news/build-multilingual-apps-with-i18n-in-react/](https://www.freecodecamp.org/news/build-multilingual-apps-with-i18n-in-react/)
    

## 14. 变更记录

v0.1（{{**2025-11-13T00:11:00**}}）：首次从 DR 蒸馏，建立结论与 YAML 种子。 
