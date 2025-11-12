---
file: 2025-11-11-dr.md
project: |-
  我要做一个选课网址，旨在代替掉罗格斯大学csp和webreg选课系统。目前这两个系统臃肿且匮乏必要的功能。

  **目前现存的问题有：**

  1. 目前csp的筛选功能只能筛选：Course Types、Section Status、Day & Time、Campus Location，而且只能在已经选择了的课程中进行筛选，这极度不便。
  2. 现如今的选课系统不能为你想要的课做出提醒，这一点在很多时候都非常不方便。

  **所以我想到的解决方案是：**

  1. 构建一个实时更新的工具，用它来代替csp中对课程的浏览。它需要具有高级的筛选功能，需要能对以下属性进行筛选：
     课程标题、课程全称、课程代码、学分、开设学院、学科描述、是否要求先修课程、核心课程要求、注册索引号、班次号、任课教师、课程状态、交叉课程（指交叉列出的课程）、考试代码、上课星期、上课时间、下课时间、校区、教学楼、教室、授课模式

  2. 建立一个和邮箱、discord等相关联的机器人，当用户标记的课程有空位的时候自动发送提醒。

  **我的要求有：**

  1. 性能良好、实现高效、架构合理
  2. 高可拓展性，我可能会在以后添加更复杂的筛选功能，比如满足一个复杂要求（例如这学期需要满足WRC和CCO两个core并且只想在星期一到星期三上课）。
  3. 多语言可能，应该能在以后便捷的添加新语言的支持，比如对罗格斯大学部分小语种学生要求小语种支持，应当能便捷快速的添加。
  4. 实现一个高度可视化的用户界面，简约、直观是设计中需要遵守的。
  5. 提供流畅的用户体验，比如可以直接在一个星期日历上框选一个（或多个）时间段，筛选这个区域的课程。
  6. 低运营成本，我希望在保持极低服务器成本的同时，支持中等并发访问与弹性扩容。
  7. 我希望这个项目既可以在 GitHub 上开源、供他人一键部署，也可以由我托管一份在线服务供大家使用。所以需要同时考虑实现功能的核心部分和与服务化部署相关组件。

  **约束与限制：**

  8. 不做登录、不接入webreg进行自动选课、不要求提供任何信息，仅通过公开可访问数据提供服务。
version: 0.1
date: 2025-11-11
derived_from: Deep Research (raw notes in context)
---

# 我要做一个选课网址，旨在代替掉罗格斯大学csp和webreg选课系统。目前这两个系统臃肿且匮乏必要的功能。 · DR 结论（只读）

> 本文是从上游调研原文蒸馏的**结论性文档**，用于人审阅与 6b 机读抽取。  
> 约定：文中方括号标注 [EVID-###] 以指向“参考文献”。

## 1. 背景&目标

- 目标（What/Why）：构建一个**替代 Rutgers CSP/WebReg 浏览能力**的公开课程浏览与提醒网站；提供**多字段高级筛选**与**空位自动提醒（Email/Discord）**，以解决官方工具筛选受限与无提醒的痛点。[EVID-001][EVID-002]
    
- 成功判据（对 MVP 的可验证结果）：（1）支持以 SOC 公开数据为源进行全量课程浏览与组合筛选；（2）在模拟与真实环境中，课程空位出现后 60 秒内触发通知；（3）前端筛选交互在千级课程规模下 <0.5s 响应；（4）开源并可一键部署的托管方案。[EVID-002][EVID-008]
    
- 非目标 / 暂不覆盖：不做登录；不与 WebReg 集成自动加课；不采集敏感个人信息；不做学位规划等超出筛选/提醒范围的功能。[EVID-001][EVID-002]
    

## 2. 结论总览（Top Insights）

- (C1) **推荐采用“前端静态站点 + 无服务器通知”架构**：前端在浏览器本地完成筛选，通知由云函数轮询 SOC 并推送，兼顾**高并发、低成本与易维护**。[EVID-002][EVID-008][EVID-009]
    
- (C2) **数据源可用**：Rutgers 提供公开 SOC JSON 接口（示例含 courses.json），虽文档化不足但可稳定获取课程列表与状态字段，适合作为唯一数据源。[EVID-003]
    
- (C3) **通知方案已被先例验证**：社区已有脚本/服务（Lightning、Sniper、SwiftRU）长期监控 SOC 并邮件/Discord 通知，技术可行性与用户价值均得到验证。[EVID-005][EVID-007][EVID-006]
    

## 3. 关键决策与约束（Decision & Constraints）

> 6b 将据此拆出决策/约束类任务与校验。

- D1. 决策：**以“静态前端 + 云函数通知”作为 v1 架构**（至少覆盖 NB 本科场景，后续再扩展校区/层次）。适用范围：MVP～v1；滚动评审。依据 [EVID-002][EVID-008][EVID-009]
    
- Z1. 约束：**仅使用公开可访问数据**（SOC），不接入 WebReg、不做自动加课；不要求登录。依据 [EVID-001][EVID-003]
    
- Z2. 约束：**低运维与低成本优先**；前端静态托管+CDN，通知以云函数定时任务为主，避免长驻服务器与自管数据库。依据 [EVID-002][EVID-009]
    
- Z3. 约束：**国际化可拓展**（i18n 框架与文案资源分离），保证后续快速加语种。依据 [EVID-010]
    

## 4. 需求提炼（可执行）

> 每条均含“验收要点(acceptance-hint)”便于 6b 生成 acceptance。

### 4.1 功能性需求（FR）

- FR-01：基于 SOC 公开接口拉取指定学期/校区课程数据并在前端展示（免登录、公开访问）。依据 [EVID-003]  
    验收要点：可在 UI 中切换学期/校区并看到课程与 section 基本字段；与 SOC 页面随机抽查 10 门课字段一致。
    
- FR-02：**高级筛选**覆盖：课程标题/全称、课程代码、学分、开设学院、学科描述、先修、核心要求(Core Codes)、注册索引号(Index)、班次号、任课教师、课程状态、交叉课程、考试代码、上课星期/时间段、校区/楼宇/教室、授课模式。依据 [EVID-001]  
    验收要点：对上述每一字段提供筛选控件；至少 5 组多条件组合（AND/OR）用例返回结果与人工比对一致。
    
- FR-03：**空位提醒订阅**：用户提交索引号+联系方式（Email/或 Discord），系统检测目标 section 从 Closed→Open 时发送提醒。依据 [EVID-005][EVID-007][EVID-006]  
    验收要点：在测试环境模拟状态切换，记录事件→通知到达时间线，平均<60s，且不重复骚扰（一次事件一次通知）。
    
- FR-04：**周视图时间交互**：在周历中框选/点击时间段进行筛选（v1 可先支持“按星期+起止时间”简单筛选，冲突消解与空闲反选可留待 v2）。依据 [EVID-001]  
    验收要点：选择“周一 9:00–12:00”后，结果仅包含任一 meeting time 落入该区间的 sections。
    
- FR-05：**跨列表(Cross-listed) 合并显示**：同一教学活动跨院系编号时，在列表中聚合展示，避免重复。依据 [EVID-002]
      
    验收要点：对 3 门已知跨列课程，页面仅出现一次卡片，展开可见全部代码。
    
- FR-06：**多语言**（至少中/英），运行时可切换；后续可加语言包。依据 [EVID-010]  
    验收要点：语言切换后 UI 文案与表头/筛选项同步变化，无乱码与截断。
    
- FR-07：**开源与一键部署**：提供公共仓库与 README，含数据抓取脚本、前端构建脚本与托管说明。依据 [EVID-001][EVID-008]  
    验收要点：新环境按文档 ≤1 小时完成部署并成功展示样例学期数据。
    

### 4.2 非功能性需求（NFR）

- NFR-01：**筛选响应**：在 ~1000 门课数据集上，常见筛选操作（≤5 条件）客户端响应 <0.5s（95 分位）。依据 [EVID-002][EVID-008]  
    验收要点：使用性能计时与 30 次交互的 p95 指标。
    
- NFR-02：**首次可用渲染**：典型网络下首屏可用时间 <3s（含必要静态资源与首批数据）。依据 [EVID-002][EVID-009]  
    验收要点：Lighthouse/Web Vitals 指标达标。
    
- NFR-03：**通知延迟**：状态变化→通知发出平均 <60s，峰值 ≤120s。依据 [EVID-002]  
    验收要点：端到端测试 20 次，满足平均/峰值阈值。
    
- NFR-04：**成本与可运维性**：前端静态托管（如 GitHub Pages/等价），通知用云函数定时任务，避免自管长驻后端。依据 [EVID-009][EVID-008]  
    验收要点：月度固定成本≈0（不含自愿捐助/域名）。
    
- NFR-05：**隐私**：最小化收集，仅存 Email/Discord 标识用于通知，支持一键退订与删除。依据 [EVID-001][EVID-002]  
    验收要点：提交退订后，后台不再包含该联系人记录。
    

## 5. 技术路径与方案对比（若适用）

|方案|适用场景|优点|风险/代价|证据|
|---|---|---|---|---|
|A. 后端数据库 + 动态服务|需要复杂后端查询与持久化|灵活查询；可集中调度通知|自管服务器与 DB 成本；维护复杂|[EVID-005][EVID-007]|
|B. 静态前端 + 云函数通知（推荐）|高并发浏览、低成本运营|CDN 扩展性强、纯前端筛选快；部署简单|需设计数据离线打包与前端性能优化|[EVID-008][EVID-009][EVID-002]|
|C. 改造现有开源|期望快速复用|有成熟 UI/流程可借鉴|适配 Rutgers 字段/接口需投入；上游停更风险|[EVID-008][EVID-007]|

> 推荐：选择 **B**。理由：客户端筛选+静态托管验证于 QuACS，成本与并发优势显著；通知以云函数轮询 SOC 可由社区先例证明可行。[EVID-008][EVID-009][EVID-002][EVID-005][EVID-007]

## 6. 外部依赖与阻断

- Rutgers **SOC JSON 接口**（公开课程数据）→ 现状：**unblocked**（可访问但文档不足）→ 影响：数据可用性与模式变化将直接影响系统运行。[EVID-003]
    
- 邮件/消息通道（SendGrid、Discord Bot 等）→ 现状：**unblocked**（需各自 API Key 与速率遵守）→ 影响：通知可达性与时延。[EVID-005][EVID-007]
    
- 静态托管/CDN（GitHub Pages/同类）→ 现状：**unblocked** → 影响：前端访问性能与成本。[EVID-009][EVID-008]
    
- 如存在阻断：最小解锁路径为**降低拉取频率+缓存策略**、在接口变更时**快速适配抓取模块**并以 HTML 解析为备份方案。[EVID-002]

## 7. 风险清单（含缓解）

- R-01：**数据源变动/限流**（中概率/高影响）。监测接口可用性与字段变更；封装适配层，必要时回退到页面解析备援。依据 [EVID-003][EVID-002]
    
- R-02：**通知延迟/漏报**（低-中概率/高影响）。采用定时轮询与失败重试，记录心跳，出现异常告警；多渠道（Email/Discord）冗余。依据 [EVID-005][EVID-007][EVID-002]
    
- R-03：**前端性能在弱设备退化**（中概率/中影响）。使用虚拟列表、Web Worker；数据分片加载。依据 [EVID-008][EVID-002]
    
- R-04：**开源治理与维护负担**（中概率/中影响）。制定贡献指南与 CI 质量门槛，减少引入回归。依据 [EVID-002]
    

## 8. 开放问题（需要结论的人/时间）

- Q-01：SOC 是否存在**批量开放索引**的高效查询端点及使用边界？需要基于实测与源代码/文档确认。（负责人：架构；证据：SOC 端点实测；截止：M1 前）[EVID-003][EVID-002]
    
- Q-02：**通知策略**是否“一次开放仅一次通知”还是“持续监控直至用户停用”？需结合用户测试确定。（负责人：产品；证据：试点反馈；截止：M1 评审）[EVID-002]
    
- Q-03：**时间交互语义**（框选表示“包含”还是“无冲突”）的产品定义与算法落地次序。（负责人：产品/前端；截止：M2 前）[EVID-001][EVID-002]
    

## 9. 术语与域模型（可用于统一命名）

- **SOC (Schedule of Classes)**：Rutgers 公开课程目录系统，提供 JSON 接口（示例 courses.json）。[EVID-003]
    
- **Index（注册索引号）**：用于 WebReg 加课的 5 位数字，每个 section 唯一。[EVID-002]
    
- **Cross-listed（交叉课程）**：同一课程内容在不同院系列表下的多代码表示，展示时需合并。[EVID-002]
    
- **Core Codes（核心要求）**：本科通识体系标签（如 WCr、CCO 等），筛选可按代码过滤。[EVID-002]
    

## 10. 证据一致性与时效

- 互相矛盾点：未发现直接矛盾；但 SOC 文档**不足/非正式**与社区先例**实现细节差异**并存，需以实测为准。[EVID-003][EVID-005][EVID-007]
    
- 证据时效：最早来源 2017–2018（Sniper、Rutgers Course API README）⚠️；需在首轮 Spike 中复查端点有效性与字段定义。[EVID-003][EVID-007]
    
- 数据缺口：SOC 批量端点与速率限制、课程状态字段变更频率、学期代码映射表等仍需补充实测。_[EVID-003][EVID-002]_
    

---

## 11. Action Seeds（供 6b 机读转 JSON；YAML，不等于最终任务）

```yaml
action_seeds:
  - id: ACT-001
    title: "SOC 端点勘探与样本抓取"
    category: spike
    rationale: "验证公开接口的可用性与字段结构，为前端数据模型与离线打包提供依据"
    evidence: ["EVID-003","EVID-002"]
    acceptance_hint: "给出 NB 本科某院系 JSON 样本与字段说明；记录请求参数与成功率"
    priority_guess: P0
    depends_on: []

  - id: ACT-002
    title: "前端筛选内核（千级数据）"
    category: build
    rationale: "实现本地多条件过滤，满足 <0.5s 响应的性能目标"
    evidence: ["EVID-008","EVID-002"]
    acceptance_hint: "在 1000 条样本上 5 组合条件 p95 <0.5s，含单元测试"
    priority_guess: P0
    depends_on: ["ACT-001"]

  - id: ACT-003
    title: "课程列表与详情 UI（含 Cross-listed 合并）"
    category: build
    rationale: "提供可用的浏览与信息架构"
    evidence: ["EVID-002"]
    acceptance_hint: "3 门跨列课程仅出现一条聚合卡片，展开含全部代码"
    priority_guess: P1
    depends_on: ["ACT-002"]

  - id: ACT-004
    title: "周历筛选交互 MVP"
    category: build
    rationale: "支持按星期+时间段初步过滤，满足核心用户需求"
    evidence: ["EVID-001"]
    acceptance_hint: "选择周一9–12点后过滤结果正确，附 10 个用例通过"
    priority_guess: P1
    depends_on: ["ACT-002"]

  - id: ACT-005
    title: "通知订阅 API（Email/Discord）"
    category: build
    rationale: "实现最小可用的提醒链路与退订能力"
    evidence: ["EVID-005","EVID-007","EVID-006"]
    acceptance_hint: "模拟 Closed→Open 事件，平均 <60s 触达；含退订接口"
    priority_guess: P0
    depends_on: ["ACT-001"]

  - id: ACT-006
    title: "数据离线打包与增量更新脚本"
    category: build
    rationale: "降低运行期对 SOC 的压力并优化首屏加载"
    evidence: ["EVID-002","EVID-009"]
    acceptance_hint: "按学期/院系列出 JSON 分片，首屏加载 <3s"
    priority_guess: P1
    depends_on: ["ACT-001"]

  - id: ACT-007
    title: "i18n 架构与中英双语落地"
    category: build
    rationale: "满足多语言可拓展约束"
    evidence: ["EVID-010","EVID-001"]
    acceptance_hint: "运行时切换中/英，所有 UI 文案覆盖率 100%"
    priority_guess: P2
    depends_on: ["ACT-003"]

  - id: ACT-008
    title: "托管与 CI/CD（静态前端+云函数）"
    category: build
    rationale: "一键部署与自动发布，保证低运维"
    evidence: ["EVID-009","EVID-008","EVID-002"]
    acceptance_hint: "新环境 ≤1 小时完成部署；push 自动发布"
    priority_guess: P1
    depends_on: ["ACT-002","ACT-005","ACT-006"]

  - id: ACT-009
    title: "端到端性能与可靠性测试"
    category: spike
    rationale: "验证 NFR 指标与稳定性"
    evidence: ["EVID-002","EVID-008"]
    acceptance_hint: "E2E 测试集覆盖筛选/通知路径，指标达成"
    priority_guess: P1
    depends_on: ["ACT-008"]

  - id: ACT-010
    title: "开源合规与文档完善"
    category: doc
    rationale: "确保仓库许可、使用说明与贡献流程清晰"
    evidence: ["EVID-001","EVID-008"]
    acceptance_hint: "README/贡献指南/许可齐备；他人按文档完成部署"
    priority_guess: P2
    depends_on: ["ACT-008"]
```

## 12.External Dependencies（供 6b 识别依赖态；YAML）

```yaml
external_dependencies:
  - id: DEP-001
    name: "Rutgers SOC JSON 接口"
    status: unblocked
    blocker: "文档欠缺、端点变化与潜在限流风险"
    unblock_plan: "封装抓取层；设置缓存与退避；监控字段变更并快速适配"
    evidence: ["EVID-003","EVID-002"]

  - id: DEP-002
    name: "SendGrid（或等价邮件 API）"
    status: unblocked
    blocker: "需要 API Key 与基础域名配置；遵守反垃圾策略"
    unblock_plan: "创建试用账号；配置发信域/或验证收件；实现退订"
    evidence: ["EVID-005","EVID-002"]

  - id: DEP-003
    name: "Discord Bot API"
    status: unblocked
    blocker: "需 Bot Token；遵守速率限制与服务器权限配置"
    unblock_plan: "创建测试服务器与 Bot；验证私信/频道两种通知路径"
    evidence: ["EVID-006","EVID-002"]

  - id: DEP-004
    name: "静态托管/CDN（GitHub Pages 或等价）"
    status: unblocked
    blocker: "自定义域名与 HTTPS 配置"
    unblock_plan: "按托管文档配置；接入自动化部署流水线"
    evidence: ["EVID-009","EVID-008"]

```

## 13.参考文献（附来源日期）

> 统一 ISO-8601；若发布日期缺失，写 published: unknown。

- [EVID-001] 项目简介（用户提供） — 作者/机构：用户 — published: unknown — accessed: 2025-11-11 — URL: internal:chat
    
- [EVID-002] Research Report — 我要做一个选课网址 (Rutgers Course Scheduler & Notifier) — 作者/机构：用户/内部调研 — published: 2025-11-11 — accessed: 2025-11-11 — URL: internal:file
    
- [EVID-003] Rutgers Course API (README) — 作者/机构：David Parsons (Rutgers CS) — published: unknown — accessed: 2025-11-11 — URL: https://github.com/anxious-engineer/Rutgers-Course-API
    
- [EVID-005] Lightning – Rutgers course sniper (GitHub README) — 作者/机构：Anitej Biradar — published: 2020-12-03 — accessed: 2025-11-11 — URL: https://github.com/anitejb/lightning#overview
    
- [EVID-006] SwiftRU — an optimal solution for course sniping at Rutgers (Reddit post) — 作者/机构：hattvr — published: 2022-05-01 — accessed: 2025-11-10 — URL: https://www.reddit.com/r/rutgers/comments/ug7hvc/swiftru_an_optimal_solution_for_course_sniping_at/
    
- [EVID-007] Sniper (GitHub README, archived) — 作者/机构：Rui Zhang 等 — published: 2017-07-15 — accessed: 2025-11-11 — URL: https://github.com/v/sniper
    
- [EVID-008] QuACS – Questionably Accurate Course Scheduler (README) — 作者/机构：RCOS (RPI OSS) — published: 2021-08-15 — accessed: 2025-11-11 — URL: https://github.com/quacs/quacs#quacs-philosophy
    
- [EVID-009] How Much Does a Static and Dynamic Website Cost? (Dev.to) — 作者/机构：Alena James — published: 2023-08-01 — accessed: 2025-11-11 — URL: https://dev.to/alenajames/how-much-does-a-static-and-dynamic-website-cost-3lk4
    
- [EVID-010] How to Build Multilingual Apps with i18n in React (freeCodeCamp) — 作者/机构：freeCodeCamp — published: 2024-12-04 — accessed: 2025-11-11 — URL: https://www.freecodecamp.org/news/build-multilingual-apps-with-i18n-in-react/
    
```

## 14. 变更记录

v0.1（2025-11-11）：首次从 DR 蒸馏，建立结论与 YAML 种子。 
