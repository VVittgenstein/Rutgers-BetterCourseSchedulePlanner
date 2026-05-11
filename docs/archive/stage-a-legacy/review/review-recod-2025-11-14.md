diff --git a/record.json b/record.json
index 5c4002c..5b7c3d9 100644
--- a/record.json
+++ b/record.json
@@ -1,7 +1,7 @@
 {
   "project_context": {
     "brief": "构建一个基于 Rutgers SOC 公开数据的选课查询与空位提醒网站，在“浏览/筛选课程 + 订阅空位提醒”场景下替代 CSP 和 WebReg 的日常使用体验，同时保持低运营成本与开源可一键部署能力。",
-    "mvp_goal": "在静态托管的网站上，实现针对多学期与多校区（覆盖本科与研究生课程）的课程浏览、多维筛选、邮箱/Discord 空位提醒闭环，以及基础多语言支持，并提供可在本地快速部署的完整方案。",
+    "mvp_goal": "在每个部署环境内运行本地 SQLite 数据库 + 轻量 API 服务 + 前端 UI，实现针对多学期与多校区课程的浏览、多维筛选与邮箱/Discord 空位提醒闭环，并保持低运营成本与可在个人电脑/服务器一键部署。",
     "non_goals": [
       "不接入 WebReg 或任何需要登录的系统页面，不在本项目中执行自动选课/退课操作。",
       "不实现完整的学位规划、毕业审核或复杂先修关系校验，仅在 UI 展示先修与核心课程标签。",
@@ -66,18 +66,20 @@
     },
     "T-20251113-act-001-soc-json-scraper": {
       "seed_id": "ACT-001",
-      "title": "封装 Rutgers SOC 数据抓取与静态 JSON 生成脚本",
+      "title": "封装 Rutgers SOC 数据抓取与本地数据库初始化脚本",
       "type": "coding",
-      "summary": "基于决策 D-20251113-data-source-soc，从 Rutgers SOC 公开接口（courses.json / openSections）按学期与校区批量抓取课程与 Section，生成静态 JSON，为前端筛选与通知服务提供统一数据源，并验证当前学期 API 可用性与字段覆盖范围。",
+      "summary": "基于 D-20251113-data-source-soc，从 Rutgers SOC 接口按学期与校区批量抓取课程与 Section，使用事务将结果写入本地 SQLite 数据库，为查询 API 与通知进程提供统一数据源，并验证字段覆盖 FR-01/FR-02。",
       "acceptance_draft": [
-        "脚本支持以 term + campus 为输入，或接收 term/campus 列表批量处理，在多学期、多校区场景下调用 SOC API 抓取对应组合下全部课程与 Section，并在失败时重试或打印清晰错误日志。",
-        "生成的 JSON 字段覆盖 FR-01/FR-02 中列出的关键字段（课程标题、代码、学分、院系、Index、Section、时间地点、核心代码、先修、授课模式等）。",
-        "对至少两个真实学期以及至少两个不同校区（覆盖本科与研究生课程）完成抓取，随机抽样 ≥10 门课与官方 SOC 页面比对，课程数量与字段值一致。"
+        "脚本支持以 term + campus 为输入，或接收 term/campus 列表批量处理，针对每个组合调用 SOC API 并将课程/Section 写入 SQLite 数据库（事务 + 幂等 upsert），失败时提供重试与清晰错误日志。",
+        "数据库中的 courses/sections 等表字段覆盖 FR-01/FR-02 中列出的关键字段（课程标题、代码、学分、院系、Index、Section、时间地点、核心代码、先修、授课模式等），随机抽样 ≥10 门课与官方 SOC 页面比对值一致。",
+        "脚本输出更新摘要（新增/更新/删除条数、学期/校区范围），支持“全量初始化”和“增量更新”两种模式，增量模式仅替换状态发生变化的记录。"
       ],
       "lane": "now",
       "priority_suggested": "P0",
       "status": "todo",
-      "dependencies": [],
+      "dependencies": [
+        "T-20251113-act-007-local-db-schema"
+      ],
       "blocked": true,
       "blocked_by": [
         {
@@ -89,14 +91,18 @@
           "note": "接口无正式公开文档，速率限制与长期稳定性未知；结构或访问策略变更将直接影响数据抓取与通知准确性。"
         }
       ],
-      "unblock_plan": "优先完成对 SOC API 的实测（见 T-20251113-soc-api-validation），确认稳定参数与速率限制后再固定脚本实现，并为结构变更预留版本检测与回滚路径。",
+      "unblock_plan": "先完成 T-20251113-soc-api-validation 与 T-20251113-act-007-local-db-schema，锁定字段/索引定义后再实现写入与增量更新逻辑，并在 CI 中加入数据库一致性校验。",
       "estimate_h": 6,
       "risk": "high",
       "confidence": 0.7,
-      "interfaces_touched": ["data-source", "build-script"],
+      "interfaces_touched": [
+        "data-source",
+        "database",
+        "build-script"
+      ],
       "artifacts": [
         "scripts/fetch_soc_data.ts",
-        "data/<term>-<campus>.json"
+        "data/courses.sqlite"
       ],
       "citations": [
         "EVID-001",
@@ -116,19 +122,20 @@
     },
     "T-20251113-act-002-frontend-filter-mvp": {
       "seed_id": "ACT-002",
-      "title": "实现基于静态 JSON 的课程列表与多维筛选（MVP）",
+      "title": "实现基于本地查询 API 的课程列表与多维筛选（MVP）",
       "type": "coding",
-      "summary": "在静态前端中加载 ACT-001 生成的课程 JSON，在浏览器端实现课程列表展示与多维组合筛选（课程名、代码、学分、院系、核心代码、先修、时间地点、授课模式等），并提供基础周视图/时间过滤，以满足 FR-01/FR-02/FR-03 的功能与性能目标。",
+      "summary": "前端 SPA 不再一次性加载全部课程 JSON，而是通过本地查询 API 按需请求课程/Section 数据，提供与 FR-01/FR-02/FR-03 对齐的多维筛选、周视图及分页体验，确保 UI 与数据库保持一致。",
       "acceptance_draft": [
-        "用户在选择学期 + 校区后，可看到完整课程与 Section 列表，字段与 SOC 页面一致，随机抽样 ≥10 门课核对无缺失。",
-        "前端支持对标题/代码关键字、学分、院系、课程层次（本科/研究生）、是否有先修要求、核心代码、Index/Section、教师、状态、上课星期与时间区间、校区/教学楼/教室、授课模式等字段的单条件与多条件组合筛选。",
-        "在约 1000 条记录下，常见筛选/排序交互在浏览器端完成，响应时间 < 0.5 秒；提供按星期 + 起始时间过滤课程的简单周视图，列表与视图结果一致。"
+        "用户选择 term + campus 后，前端调用本地 API 拉取分页课程/Section 数据并展示完整字段，随机抽样 ≥10 门课核对与 SOC 数据库存储一致。",
+        "筛选 UI 支持标题/代码关键字、学分、院系、课程层次、先修、核心代码、Index/Section、教师、状态、星期/时间区间、校区/教学楼/教室、授课模式等字段的单条件与多条件组合，前端会将条件映射为 API 查询参数并保证结果一致。",
+        "在约 4500 门课程（含 1 万+ Section）的数据量下，常见筛选、分页与视图切换的“API 往返 + 前端渲染”整体响应 < 0.5 秒；当本地 API 不可用时给出明确的错误提示。"
       ],
       "lane": "now",
       "priority_suggested": "P0",
       "status": "todo",
       "dependencies": [
-        "T-20251113-act-001-soc-json-scraper"
+        "T-20251113-act-001-soc-json-scraper",
+        "T-20251113-act-008-local-query-api"
       ],
       "blocked": false,
       "blocked_by": [],
@@ -138,13 +145,15 @@
       "confidence": 0.7,
       "interfaces_touched": [
         "frontend",
+        "api-client",
         "filter-logic",
         "calendar-view"
       ],
       "artifacts": [
         "src/pages/courses.tsx",
         "src/components/filters/*",
-        "src/components/calendar-view/*"
+        "src/components/calendar-view/*",
+        "src/api/hooks/useCourses.ts"
       ],
       "citations": [
         "EVID-001",
@@ -165,19 +174,21 @@
     },
     "T-20251113-act-003-mail-notify-function": {
       "seed_id": "ACT-003",
-      "title": "设计并上线基于 SOC openSections 的邮件通知云函数",
+      "title": "实现课程空位邮件通知功能（本地定时任务）",
       "type": "coding",
-      "summary": "根据 D-20251113-arch-static-frontend-b 与 D-20251113-notify-channels-email-discord，使用无服务器云函数轮询 SOC openSections，监控用户订阅的 Section 从 Closed 到 Open 的变化，并通过邮件发送包含课程关键信息的提醒，满足 FR-04/FR-05/FR-06 与 NFR-02 中的延迟与可靠性要求。",
+      "summary": "依据 D-20251113-notify-channels-email-discord，在每个部署环境中运行本地后台进程定期轮询 SOC openSections，结合本地数据库中的订阅记录判断 Closed→Open 事件，并通过可配置的邮件发送模块通知用户，满足 FR-04/FR-05/FR-06 与 NFR-02 的延迟与可靠性要求。",
       "acceptance_draft": [
-        "在测试环境模拟 Closed → Open 场景时，记录从状态变化到收到邮件的时间，平均延迟 < 30 秒，最大延迟 < 60 秒，且同一开放事件同一邮箱只收到 1 封邮件。",
-        "邮件正文包含课程名称、Index、Section、上课时间地点等关键信息，并提供安全的退订/管理链接；退订后同一订阅不再收到后续通知。",
-        "云函数对 SOC API 与邮件服务调用失败具备重试与日志记录机制，错误不会静默丢失。"
+        "在本地测试环境模拟 Closed→Open 场景时，记录从状态变化到邮件发送的时间，平均延迟 < 30 秒、最大延迟 < 60 秒，且同一开放事件同一邮箱只收到 1 封邮件。",
+        "邮件正文包含课程名称、Index、Section、上课时间地点与订阅人信息，并提供安全的本地退订/管理链接；退订后同一订阅不再收到后续通知。",
+        "本地通知进程支持自定义轮询频率、日志与错误重试，能够在网络异常/进程重启后恢复状态，不会静默丢失事件。"
       ],
       "lane": "now",
       "priority_suggested": "P0",
       "status": "todo",
       "dependencies": [
-        "T-20251113-act-001-soc-json-scraper"
+        "T-20251113-act-009-subscription-management",
+        "T-20251113-act-010-local-polling-notify",
+        "T-20251113-act-011-mail-sender-module"
       ],
       "blocked": true,
       "blocked_by": [
@@ -198,18 +209,20 @@
           "note": "免费额度有限且需要正确配置发件域名与内容策略，以降低被判为垃圾邮件的风险。"
         }
       ],
-      "unblock_plan": "在 T-20251113-soc-api-validation 中摸清 SOC API 行为；基于免费额度设计轮询频率与批量聚合策略，并在配置层支持更换邮件服务商。",
+      "unblock_plan": "基于 T-20251113-act-010-local-polling-notify 提供的轮询结果与 T-20251113-act-011-mail-sender-module 中的发送适配器，设计本地 cron/worker，验证多次重试与恢复逻辑后再开放配置。",
       "estimate_h": 8,
       "risk": "high",
       "confidence": 0.65,
       "interfaces_touched": [
-        "cloud-functions",
+        "local-service",
         "notification-service",
-        "email-provider"
+        "email-provider",
+        "scheduler"
       ],
       "artifacts": [
-        "functions/notify_email.ts",
-        "schemas/subscription.json"
+        "services/notifier/email_worker.ts",
+        "templates/email/*",
+        "config/email.json"
       ],
       "citations": [
         "EVID-001",
@@ -232,17 +245,18 @@
       "seed_id": "ACT-004",
       "title": "实现 Discord 通知通道并评估私信与频道策略",
       "type": "coding",
-      "summary": "在通知管道中新增 Discord Bot 通道，支持向用户或频道发送课程开放提醒，并结合 Q-04 评估“私信 vs 频道广播”的可行性、噪音与速率限制表现，为正式默认策略提供依据。",
+      "summary": "在本地通知管道中新增 Discord Bot 通道，运行本地 Bot 客户端将课程开放提醒发送给用户或频道，并结合 Q-04 评估“私信 vs 频道广播”的可行性、噪音与速率限制表现，为默认策略提供依据。",
       "acceptance_draft": [
-        "在测试服务器中，Bot 能根据订阅记录成功向指定用户或频道发送包含课程名、Index 与时间地点的提醒消息。",
-        "实现对 Discord API 速率限制的节流与重试逻辑，高频触发时仍保持稳定，不出现长期封禁或严重错误。",
+        "在本地或测试服务器中，Bot 能根据数据库内的订阅记录成功向指定用户或频道发送包含课程名、Index 与时间地点的提醒消息。",
+        "实现对 Discord API 速率限制的节流与重试逻辑，本地轮询高频触发时仍保持稳定，不出现长期封禁或严重错误。",
         "文档中记录不同通知模式（私信/频道）的优缺点与推荐默认值，并与产品负责人达成一致。"
       ],
       "lane": "next",
       "priority_suggested": "P2",
       "status": "todo",
       "dependencies": [
-        "T-20251113-act-001-soc-json-scraper",
+        "T-20251113-act-009-subscription-management",
+        "T-20251113-act-010-local-polling-notify",
         "T-20251113-act-003-mail-notify-function"
       ],
       "blocked": true,
@@ -264,17 +278,17 @@
           "note": "受速率限制和内容政策约束，过于频繁或模板化的消息可能被限流或封禁。"
         }
       ],
-      "unblock_plan": "阅读 Discord Bot API 最新文档并在测试服务器进行小规模实验，确定安全的速率与消息格式；结合 Q-04 的业务决策选择默认模式。",
+      "unblock_plan": "在本地环境运行 Bot，复用 T-20251113-act-010-local-polling-notify 的事件推送，结合 Discord Sandbox 逐步提升频率以摸清速率限制，再决定默认通知模式。",
       "estimate_h": 6,
       "risk": "mid",
       "confidence": 0.6,
       "interfaces_touched": [
-        "cloud-functions",
+        "local-service",
         "notification-service",
         "discord-bot"
       ],
       "artifacts": [
-        "functions/notify_discord.ts",
+        "services/notifier/discord_bot.ts",
         "config/discord.json"
       ],
       "citations": [
@@ -343,10 +357,10 @@
       "seed_id": "ACT-006",
       "title": "编写并验证本地一键部署文档与辅助脚本",
       "type": "docs",
-      "summary": "围绕“静态前端 + 本地通知进程”架构，整理本地部署步骤与环境要求，编写 README 与辅助脚本，使普通用户在个人电脑上无需修改代码即可完成依赖安装、配置和本地运行。",
+      "summary": "围绕“本地数据库 + 轻量 API 服务 + 前端 UI + 后台通知进程”的架构，整理本地部署步骤与环境要求，编写 README 与辅助脚本，使普通用户在个人电脑或服务器上无需修改代码即可初始化数据库、运行 API/前端/通知服务并完成配置。",
       "acceptance_draft": [
-        "README 提供针对至少两种主流桌面操作系统（例如 Windows 与 macOS 或 Linux）的本地部署步骤，包含：前置依赖安装（如 Node.js/bun、包管理器等）、项目初始化、环境变量与密钥配置，以及如何同时启动前端与本地通知进程。",
-        "仓库包含可直接使用的本地辅助脚本（例如 scripts/setup_local_env.sh 和 scripts/run_local.sh），在一台全新环境的个人电脑上，用户 clone 仓库后仅需运行脚本即可完成依赖安装校验、环境检查并成功启动服务。"
+        "README 提供针对至少两种主流桌面操作系统（例如 Windows 与 macOS 或 Linux）的本地部署步骤，包含：安装 Node.js/bun、SQLite 或嵌入式数据库依赖、项目初始化、环境变量与密钥配置、如何执行数据库初始化/增量更新脚本，以及如何同时启动前端、API 服务与本地通知进程。",
+        "仓库包含可直接使用的本地辅助脚本（例如 scripts/setup_local_env.sh 和 scripts/run_local.sh），在一台全新环境的个人电脑上，用户 clone 仓库后运行脚本即可完成依赖安装校验、数据库初始化、服务启动与通知模块验证。"
       ],
       "lane": "next",
       "priority_suggested": "P1",
@@ -354,11 +368,14 @@
       "dependencies": [
         "T-20251113-act-001-soc-json-scraper",
         "T-20251113-act-002-frontend-filter-mvp",
-        "T-20251113-act-003-mail-notify-function"
+        "T-20251113-act-003-mail-notify-function",
+        "T-20251113-act-008-local-query-api",
+        "T-20251113-act-009-subscription-management",
+        "T-20251113-act-010-local-polling-notify"
       ],
       "blocked": false,
       "blocked_by": [],
-      "unblock_plan": "以本地单机运行为一等公民目标设计部署流程：在至少两种主流桌面环境中验证脚本与文档可用，避免对特定云服务或托管平台产生刚性依赖。",
+      "unblock_plan": "以本地单机运行与容器部署为一等公民目标设计流程：在至少两种主流桌面环境验证脚本可初始化 SQLite、启动 API/前端/通知服务，再整理成 README。",
       "estimate_h": 6,
       "risk": "mid",
       "confidence": 0.7,
@@ -388,6 +405,264 @@
         "deployment",
         "local-deploy"
       ]
+    },
+    "T-20251113-act-007-local-db-schema": {
+      "seed_id": "ACT-007",
+      "title": "设计本地数据库模式与增量更新机制",
+      "type": "coding",
+      "summary": "定义 SQLite 等嵌入式数据库中的课程、Section、会议信息与订阅表结构，设计索引与约束，并实现初始化/迁移与增量更新策略，使数据成为筛选与通知的单一可信源。",
+      "acceptance_draft": [
+        "提供 schema 定义（课程、Section、meeting_times、订阅等表）与必要索引/约束，字段覆盖 FR-01/FR-02/FR-04 所需信息，并附带 ER/说明文档。",
+        "实现 scripts/migrate_db.* 可初始化或升级数据库，重复运行不会破坏已有数据；随机抽样记录验证字段与 SOC 返回一致。",
+        "定义增量更新策略（例如基于 upsert、事务或临时表交换），在模拟部分课程状态变化的场景下，仅更新发生变化的数据且不中断前端/API 查询。"
+      ],
+      "lane": "now",
+      "priority_suggested": "P0",
+      "status": "todo",
+      "dependencies": [
+        "T-20251113-soc-api-validation"
+      ],
+      "blocked": true,
+      "blocked_by": [
+        {
+          "dep_id": "DEP-001",
+          "name": "Rutgers SOC API（courses.json / openSections）",
+          "type": "api",
+          "status": "unknown",
+          "doc_url": "https://github.com/anxious-engineer/Rutgers-Course-API",
+          "note": "接口结构与速率限制需要先通过 T-20251113-soc-api-validation 实测后，才能确定 schema 设计所需字段。"
+        }
+      ],
+      "unblock_plan": "在 T-20251113-soc-api-validation 中确认字段/速率后，设计 schema 与索引并用小样本跑通迁移脚本，再推广到完整数据集。",
+      "estimate_h": 8,
+      "risk": "mid",
+      "confidence": 0.65,
+      "interfaces_touched": [
+        "database",
+        "data-pipeline"
+      ],
+      "artifacts": [
+        "db/schema.sql",
+        "scripts/migrate_db.ts",
+        "docs/db_schema.md"
+      ],
+      "citations": [
+        "EVID-001",
+        "EVID-002",
+        "EVID-006",
+        "R-20251113-frontend-perf"
+      ],
+      "created_at": "2025-11-13T00:00:00Z",
+      "updated_at": "2025-11-13T00:00:00Z",
+      "owner": "tbd",
+      "tags": [
+        "seed:ACT-007",
+        "backend",
+        "database",
+        "data-pipeline"
+      ]
+    },
+    "T-20251113-act-008-local-query-api": {
+      "seed_id": "ACT-008",
+      "title": "实现本地课程查询 API 服务",
+      "type": "coding",
+      "summary": "基于 SQLite 数据库实现 RESTful API（如 /api/courses、/api/sections、/api/filters），支持多字段过滤、分页与排序，为前端筛选与订阅面板提供一致的数据访问层。",
+      "acceptance_draft": [
+        "实现至少 /api/courses 与 /api/sections 两个端点，接受 term/campus/subject/关键字/时间/core_code/level 等查询参数，返回 JSON 包含分页信息，任意组合筛选均能得到与数据库一致的结果。",
+        "在 4500+ 课程数据规模下，常见查询（含 3~5 个过滤条件）响应时间 < 300 ms，提供必要索引与缓存并通过压测记录指标。",
+        "API 提供错误处理与健康检查端点（例如 /api/health），当前端无法连接时能返回明确错误码与原因，便于 UI 友好提示。"
+      ],
+      "lane": "now",
+      "priority_suggested": "P0",
+      "status": "todo",
+      "dependencies": [
+        "T-20251113-act-007-local-db-schema",
+        "T-20251113-act-001-soc-json-scraper"
+      ],
+      "blocked": false,
+      "blocked_by": [],
+      "unblock_plan": "复用抓取脚本产出的 SQLite 数据，挑选轻量框架（Express/Fastify/FastAPI 等）实现端点，并通过自动化测试覆盖主要组合筛选。",
+      "estimate_h": 8,
+      "risk": "mid",
+      "confidence": 0.7,
+      "interfaces_touched": [
+        "backend",
+        "api",
+        "database"
+      ],
+      "artifacts": [
+        "services/api/server.ts",
+        "services/api/routes/courses.ts",
+        "tests/api/courses.test.ts"
+      ],
+      "citations": [
+        "EVID-001",
+        "EVID-006",
+        "R-20251113-frontend-perf"
+      ],
+      "created_at": "2025-11-13T00:00:00Z",
+      "updated_at": "2025-11-13T00:00:00Z",
+      "owner": "tbd",
+      "tags": [
+        "seed:ACT-008",
+        "backend",
+        "api",
+        "database"
+      ]
+    },
+    "T-20251113-act-009-subscription-management": {
+      "seed_id": "ACT-009",
+      "title": "实现订阅管理接口与本地存储",
+      "type": "coding",
+      "summary": "为课程/Section 提供订阅与退订 UI，以及与数据库衔接的 /api/subscribe、/api/unsubscribe 等接口，将用户邮箱/Discord 标识与目标课程索引安全写入本地订阅表。",
+      "acceptance_draft": [
+        "在课程列表或详情中提供“订阅空位”入口，点击后调用 /api/subscribe 写入数据库，并支持校验输入（邮箱或 Discord 标识）以及重复订阅提示。",
+        "实现 /api/unsubscribe（含带签名的退订链接）与订阅列表查询接口，用户可从邮件/Discord 链接或 UI 中退订，数据库记录同步更新。",
+        "在本地运行环境中执行一次完整流程（订阅→模拟开放→退订），确认数据库记录、API 响应与通知任务读取到的数据一致。"
+      ],
+      "lane": "now",
+      "priority_suggested": "P0",
+      "status": "todo",
+      "dependencies": [
+        "T-20251113-act-002-frontend-filter-mvp",
+        "T-20251113-act-007-local-db-schema",
+        "T-20251113-act-008-local-query-api"
+      ],
+      "blocked": false,
+      "blocked_by": [],
+      "unblock_plan": "在完成课程列表与 API 基础后，扩展 UI 和接口，编写端到端测试验证订阅/退订流程与数据库同步。",
+      "estimate_h": 6,
+      "risk": "mid",
+      "confidence": 0.7,
+      "interfaces_touched": [
+        "frontend",
+        "api",
+        "database",
+        "notification-service"
+      ],
+      "artifacts": [
+        "src/components/subscription/*",
+        "services/api/routes/subscriptions.ts",
+        "db/subscriptions.sql"
+      ],
+      "citations": [
+        "EVID-001",
+        "EVID-005",
+        "R-20251113-notify-delay"
+      ],
+      "created_at": "2025-11-13T00:00:00Z",
+      "updated_at": "2025-11-13T00:00:00Z",
+      "owner": "tbd",
+      "tags": [
+        "seed:ACT-009",
+        "frontend",
+        "backend",
+        "notification"
+      ]
+    },
+    "T-20251113-act-010-local-polling-notify": {
+      "seed_id": "ACT-010",
+      "title": "本地空位轮询与通知调度服务",
+      "type": "coding",
+      "summary": "实现常驻后台任务或定时器，每 30–60 秒轮询 Rutgers openSections 接口，结合本地订阅数据判断课程状态变化，触发事件管道供邮箱/Discord 等通道消费，并处理幂等与错误恢复。",
+      "acceptance_draft": [
+        "可配置轮询频率与 term/campus 范围，模拟 Section 状态从 Closed→Open 的过程，确认在一个轮询周期内生成事件并写入去重缓存。",
+        "同一开放事件最多触发一次通知，短时间内频繁开关的 Section 不会造成重复推送；提供幂等键或状态表来记录已通知事件。",
+        "轮询任务具备日志、指标与错误恢复能力（例如网络失败自动重试、进程重启后继续），并可将事件推送到邮箱/Discord 等通道的队列。"
+      ],
+      "lane": "now",
+      "priority_suggested": "P0",
+      "status": "todo",
+      "dependencies": [
+        "T-20251113-act-007-local-db-schema",
+        "T-20251113-act-009-subscription-management"
+      ],
+      "blocked": true,
+      "blocked_by": [
+        {
+          "dep_id": "DEP-001",
+          "name": "Rutgers SOC API（courses.json / openSections）",
+          "type": "api",
+          "status": "unknown",
+          "doc_url": "https://github.com/anxious-engineer/Rutgers-Course-API",
+          "note": "需要确认 openSections 的速率限制与字段稳定性后才能设定本地轮询与节流策略。"
+        }
+      ],
+      "unblock_plan": "基于 T-20251113-soc-api-validation 成果，选定轮询频率与批量参数，先在小范围 term/campus 上验证事件生成、幂等与日志，再扩展到完整数据集。",
+      "estimate_h": 8,
+      "risk": "high",
+      "confidence": 0.6,
+      "interfaces_touched": [
+        "local-service",
+        "notification-service",
+        "scheduler"
+      ],
+      "artifacts": [
+        "services/notifier/poller.ts",
+        "services/notifier/event_store.ts",
+        "tests/notifier/poller.test.ts"
+      ],
+      "citations": [
+        "EVID-001",
+        "EVID-003",
+        "R-20251113-notify-delay"
+      ],
+      "created_at": "2025-11-13T00:00:00Z",
+      "updated_at": "2025-11-13T00:00:00Z",
+      "owner": "tbd",
+      "tags": [
+        "seed:ACT-010",
+        "backend",
+        "notification",
+        "scheduler"
+      ]
+    },
+    "T-20251113-act-011-mail-sender-module": {
+      "seed_id": "ACT-011",
+      "title": "邮件发送模块多平台支持",
+      "type": "coding",
+      "summary": "抽象邮件发送接口并提供至少 SendGrid API 与 SMTP 两种实现，允许通过配置切换，同时支持多语言模板、速率限制与失败重试，以方便不同部署者使用自有邮件服务。",
+      "acceptance_draft": [
+        "提供统一的 MailSender 接口与配置（API key、SMTP host、发件人等），可在不改代码的情况下切换 SendGrid 与 SMTP，实现都能发送测试邮件。",
+        "邮件内容模板支持中英文版本，可注入课程名、Index、订阅者信息等变量；模板缺少翻译或字段时能在构建/测试阶段捕获。",
+        "在模拟配额用尽或网络错误时，模块能降级重试或记录清晰错误日志，不会导致主进程崩溃。"
+      ],
+      "lane": "next",
+      "priority_suggested": "P1",
+      "status": "todo",
+      "dependencies": [
+        "T-20251113-act-009-subscription-management"
+      ],
+      "blocked": false,
+      "blocked_by": [],
+      "unblock_plan": "在完成订阅存储后，先实现 SendGrid 版本，再扩展 SMTP/本地邮件发送，并接入邮件模板与单元测试。",
+      "estimate_h": 5,
+      "risk": "mid",
+      "confidence": 0.7,
+      "interfaces_touched": [
+        "notification-service",
+        "email-provider",
+        "i18n"
+      ],
+      "artifacts": [
+        "services/notify/mail_sender.ts",
+        "config/email.example.json",
+        "templates/email/*"
+      ],
+      "citations": [
+        "EVID-001",
+        "EVID-005",
+        "R-20251113-spam"
+      ],
+      "created_at": "2025-11-13T00:00:00Z",
+      "updated_at": "2025-11-13T00:00:00Z",
+      "owner": "tbd",
+      "tags": [
+        "seed:ACT-011",
+        "backend",
+        "email",
+        "notification"
+      ]
     }
   },
   "questions_for_human": [
@@ -495,30 +770,31 @@
     },
     {
       "dep_id": "DEP-004",
-      "name": "静态前端托管平台（GitHub Pages / Netlify 等）",
+      "name": "本地运行环境/容器（Node.js + SQLite + 定时任务）",
       "type": "tool",
       "status": "approved",
       "doc_url": "",
-      "note": "若未来访问量或功能需求超出免费/轻量级平台能力，可能需要迁移到更专业的托管与 CDN。"
+      "note": "每个部署实例需要具备运行 Node.js 服务、SQLite 数据库与后台定时任务的能力，可通过本地运行或容器化实现；不再依赖纯静态托管平台。"
     }
   ],
   "facts": [
     "本 JSON 基于 2025-11-11-dr 研究结论整理，数据源为 Rutgers SOC 与相关开源/社区资料。:contentReference[oaicite:0]{index=0}",
     "项目目标是在“浏览筛选课程 + 订阅空位提醒”场景下替代 CSP 和 WebReg 的日常使用体验，而不是替代官方 WebReg 的选课注册流程。",
     "系统统一使用 Rutgers SOC 公开接口（courses.json、openSections 等）作为课程与容量数据来源，不抓取需要登录的 WebReg 页面。",
-    "架构采用静态前端 + 云函数通知，前端在浏览器内完成主要筛选逻辑，后端仅负责数据抓取与通知，从而在控制成本的同时支撑高并发访问。",
+    "架构采用“本地 SQLite 数据库 + 轻量 API 服务 + 前端 SPA + 本地后台通知”的组合，各部署实例独立运行数据库与通知进程，避免浏览器一次性加载几十 MB JSON。",
     "通知渠道 MVP 聚焦邮箱和 Discord，并通过显式退订/管理入口满足隐私与反垃圾邮件规范。",
     "项目从 day-1 起按多语言架构设计，至少支持中英文 UI，并通过 i18n 资源文件管理文案以方便后续扩展其他语言。"
   ],
   "decisions": [
     {
       "id": "D-20251113-arch-static-frontend-b",
-      "summary": "v1.x 阶段采用“静态前端 + 云函数/定时任务通知”的方案 B，而不是自建长期运行数据库后端或深度改造现有排课项目。",
-      "rationale": "静态前端可托管在 GitHub Pages/Netlify 等平台，结合云函数按量计费，既满足高并发浏览和低延迟筛选，又能显著降低长期运维成本，符合“低运营成本、无登录、注重 UI 体验”的目标。",
+      "summary": "v1.x 阶段采用“本地 SQLite 数据库 + 轻量 API 服务 + 前端 SPA”的一体化架构，而不是继续依赖纯静态前端加载大 JSON 与云函数通知。",
+      "rationale": "courses.json 体积已达 20–40 MB，纯前端加载造成性能、带宽与更新成本过高；将数据落地到本地 SQLite，并由轻量 API/后台任务统一负责筛选与通知，可以增量更新、降低前端负担，并保持每个部署实例独立运行。",
       "citations": [
         "EVID-001",
         "EVID-006",
-        "EVID-007"
+        "R-20251113-frontend-perf",
+        "R-20251113-notify-delay"
       ],
       "date": "2025-11-13T00:00:00Z"
     },
@@ -534,11 +810,12 @@
     },
     {
       "id": "D-20251113-client-side-filter",
-      "summary": "课程筛选和排序逻辑尽可能全部在浏览器端完成，后端仅负责数据抓取与通知，不提供复杂查询 API。",
-      "rationale": "在典型课程数量规模下，本地筛选可在毫秒级完成，避免后端查询瓶颈；同时简化服务器架构、提升扩展性并降低成本。",
+      "summary": "课程筛选与排序主要由嵌入式数据库与本地查询 API 承担，前端仅负责构建条件并展示结果，不再预加载全量数据。",
+      "rationale": "在多学期/多校区场景下数据量动辄数千课程，浏览器内存筛选难以兼顾性能与增量更新；利用 SQLite 索引与 API 分页可以维持 <0.5 秒响应并复用同一份数据给通知管道。",
       "citations": [
         "EVID-001",
-        "EVID-006"
+        "EVID-006",
+        "R-20251113-frontend-perf"
       ],
       "date": "2025-11-13T00:00:00Z"
     },
