diff --git a/record.json b/record.json
index af485ee..5c4002c 100644
--- a/record.json
+++ b/record.json
@@ -1,7 +1,7 @@
 {
   "project_context": {
     "brief": "构建一个基于 Rutgers SOC 公开数据的选课查询与空位提醒网站，在“浏览/筛选课程 + 订阅空位提醒”场景下替代 CSP 和 WebReg 的日常使用体验，同时保持低运营成本与开源可一键部署能力。",
-    "mvp_goal": "在静态托管的网站上，实现针对单一学期 + 校区的课程浏览、多维筛选、邮箱/Discord 空位提醒闭环，以及基础多语言支持，并提供可在 GitHub 快速自托管的完整方案。",
+    "mvp_goal": "在静态托管的网站上，实现针对多学期与多校区（覆盖本科与研究生课程）的课程浏览、多维筛选、邮箱/Discord 空位提醒闭环，以及基础多语言支持，并提供可在本地快速部署的完整方案。",
     "non_goals": [
       "不接入 WebReg 或任何需要登录的系统页面，不在本项目中执行自动选课/退课操作。",
       "不实现完整的学位规划、毕业审核或复杂先修关系校验，仅在 UI 展示先修与核心课程标签。",
@@ -14,15 +14,65 @@
   "md_source": "2025-11-11-dr.md",
   "generated_at": "2025-11-13T00:00:00Z",
   "tasks": {
+    "T-20251113-soc-api-validation": {
+      "seed_id": null,
+      "title": "验证 Rutgers SOC API 参数、限流与字段行为",
+      "type": "research",
+      "summary": "围绕 C2、R-20251113-soc-api-change 与开放问题 Q-01，对 Rutgers SOC API 的参数组合、拉取全校课程的策略、速率限制与返回字段进行实测，产出可复用的调用指南，降低后续抓取脚本与通知轮询的风险。",
+      "acceptance_draft": [
+        "整理出在多学期、多校区场景下获取全部课程的推荐调用策略（例如按 term + campus / subject 分批），并估算完整抓取一次全量以及常规增量更新所需时间。",
+        "记录 SOC API 的关键参数与典型响应样例，确认字段覆盖 FR-01/FR-02 所需信息，并标记可能缺失或不稳定的字段。",
+        "通过高频调用实验观察速率限制与错误码，给出可接受的最大抓取与轮询频率建议，并形成一份内部说明文档。"
+      ],
+      "lane": "now",
+      "priority_suggested": "P0",
+      "status": "todo",
+      "dependencies": [],
+      "blocked": true,
+      "blocked_by": [
+        {
+          "dep_id": "DEP-001",
+          "name": "Rutgers SOC API（courses.json / openSections）",
+          "type": "api",
+          "status": "unknown",
+          "doc_url": "https://github.com/anxious-engineer/Rutgers-Course-API",
+          "note": "需要在开发前期通过实测确认可用参数、速率限制和结构稳定性。"
+        }
+      ],
+      "unblock_plan": "在测试环境以较低频率尝试多种参数组合，逐步提高频率观察限流行为，将结果记录到 docs/soc_api_notes 中并同步到抓取脚本与通知配置。",
+      "estimate_h": 4,
+      "risk": "high",
+      "confidence": 0.65,
+      "interfaces_touched": [
+        "data-source",
+        "observability"
+      ],
+      "artifacts": [
+        "docs/soc_api_notes.md"
+      ],
+      "citations": [
+        "EVID-001",
+        "EVID-002",
+        "R-20251113-soc-api-change",
+        "Q-01"
+      ],
+      "created_at": "2025-11-13T00:00:00Z",
+      "updated_at": "2025-11-13T00:00:00Z",
+      "owner": "tbd",
+      "tags": [
+        "research",
+        "soc-api"
+      ]
+    },
     "T-20251113-act-001-soc-json-scraper": {
       "seed_id": "ACT-001",
       "title": "封装 Rutgers SOC 数据抓取与静态 JSON 生成脚本",
       "type": "coding",
       "summary": "基于决策 D-20251113-data-source-soc，从 Rutgers SOC 公开接口（courses.json / openSections）按学期与校区批量抓取课程与 Section，生成静态 JSON，为前端筛选与通知服务提供统一数据源，并验证当前学期 API 可用性与字段覆盖范围。",
       "acceptance_draft": [
-        "脚本支持以 term + campus 为输入，批量调用 SOC API 抓取对应组合下全部课程与 Section，并在失败时重试或打印清晰错误日志。",
+        "脚本支持以 term + campus 为输入，或接收 term/campus 列表批量处理，在多学期、多校区场景下调用 SOC API 抓取对应组合下全部课程与 Section，并在失败时重试或打印清晰错误日志。",
         "生成的 JSON 字段覆盖 FR-01/FR-02 中列出的关键字段（课程标题、代码、学分、院系、Index、Section、时间地点、核心代码、先修、授课模式等）。",
-        "对至少一个真实学期（如 2025 Fall NB）完成抓取，随机抽样 ≥10 门课与官方 SOC 页面比对，课程数量与字段值一致。"
+        "对至少两个真实学期以及至少两个不同校区（覆盖本科与研究生课程）完成抓取，随机抽样 ≥10 门课与官方 SOC 页面比对，课程数量与字段值一致。"
       ],
       "lane": "now",
       "priority_suggested": "P0",
@@ -71,7 +121,7 @@
       "summary": "在静态前端中加载 ACT-001 生成的课程 JSON，在浏览器端实现课程列表展示与多维组合筛选（课程名、代码、学分、院系、核心代码、先修、时间地点、授课模式等），并提供基础周视图/时间过滤，以满足 FR-01/FR-02/FR-03 的功能与性能目标。",
       "acceptance_draft": [
         "用户在选择学期 + 校区后，可看到完整课程与 Section 列表，字段与 SOC 页面一致，随机抽样 ≥10 门课核对无缺失。",
-        "前端支持对标题/代码关键字、学分、院系、是否有先修/核心要求、核心代码、Index/Section、教师、状态、上课星期与时间区间、校区/教学楼/教室、授课模式等字段的单条件与多条件组合筛选。",
+        "前端支持对标题/代码关键字、学分、院系、课程层次（本科/研究生）、是否有先修要求、核心代码、Index/Section、教师、状态、上课星期与时间区间、校区/教学楼/教室、授课模式等字段的单条件与多条件组合筛选。",
         "在约 1000 条记录下，常见筛选/排序交互在浏览器端完成，响应时间 < 0.5 秒；提供按星期 + 起始时间过滤课程的简单周视图，列表与视图结果一致。"
       ],
       "lane": "now",
@@ -189,7 +239,7 @@
         "文档中记录不同通知模式（私信/频道）的优缺点与推荐默认值，并与产品负责人达成一致。"
       ],
       "lane": "next",
-      "priority_suggested": "P1",
+      "priority_suggested": "P2",
       "status": "todo",
       "dependencies": [
         "T-20251113-act-001-soc-json-scraper",
@@ -291,13 +341,12 @@
     },
     "T-20251113-act-006-deploy-docs-cicd": {
       "seed_id": "ACT-006",
-      "title": "编写并验证开源一键部署文档与 CI/CD 流程",
+      "title": "编写并验证本地一键部署文档与辅助脚本",
       "type": "docs",
-      "summary": "围绕“静态前端 + 云函数通知”架构，整理部署步骤与环境要求，编写 README 与脚本，使外部开发者在不改代码的情况下即可在 GitHub Pages/Netlify 与至少一个云函数平台上完成一键部署，同时配置基础 CI/CD 工作流。",
+      "summary": "围绕“静态前端 + 本地通知进程”架构，整理本地部署步骤与环境要求，编写 README 与辅助脚本，使普通用户在个人电脑上无需修改代码即可完成依赖安装、配置和本地运行。",
       "acceptance_draft": [
-        "README 提供针对 GitHub Pages/Netlify 的前端部署步骤，以及针对至少一个云函数平台的通知服务部署说明，覆盖环境变量与密钥配置。",
-        "仓库包含可直接使用的 CI/CD 配置（如 GitHub Actions），在 push 或合并到主分支时自动构建并部署前端与云函数。",
-        "找一名未参与开发的同学，按照文档在全新环境中能在 1 小时内完成部署并成功收到至少一条测试通知。"
+        "README 提供针对至少两种主流桌面操作系统（例如 Windows 与 macOS 或 Linux）的本地部署步骤，包含：前置依赖安装（如 Node.js/bun、包管理器等）、项目初始化、环境变量与密钥配置，以及如何同时启动前端与本地通知进程。",
+        "仓库包含可直接使用的本地辅助脚本（例如 scripts/setup_local_env.sh 和 scripts/run_local.sh），在一台全新环境的个人电脑上，用户 clone 仓库后仅需运行脚本即可完成依赖安装校验、环境检查并成功启动服务。"
       ],
       "lane": "next",
       "priority_suggested": "P1",
@@ -308,28 +357,21 @@
         "T-20251113-act-003-mail-notify-function"
       ],
       "blocked": false,
-      "blocked_by": [
-        {
-          "dep_id": "DEP-004",
-          "name": "静态前端托管平台（GitHub Pages / Netlify 等）",
-          "type": "tool",
-          "status": "approved",
-          "doc_url": "",
-          "note": "若未来访问量或功能需求超出免费/轻量级平台能力，可能需要迁移到更专业的托管与 CDN。"
-        }
-      ],
-      "unblock_plan": "以纯静态构建产物为目标设计发布流程，确保可以在 GitHub Pages、Netlify 等平台之间平滑迁移；文档中保留至少两种部署路径以降低耦合。",
+      "blocked_by": [],
+      "unblock_plan": "以本地单机运行为一等公民目标设计部署流程：在至少两种主流桌面环境中验证脚本与文档可用，避免对特定云服务或托管平台产生刚性依赖。",
       "estimate_h": 6,
       "risk": "mid",
       "confidence": 0.7,
       "interfaces_touched": [
         "docs",
-        "ci-cd",
+        "local-dev",
         "deployment"
       ],
       "artifacts": [
         "README.md",
-        ".github/workflows/deploy.yml"
+        ".env.example",
+        "scripts/setup_local_env.sh",
+        "scripts/run_local.sh"
       ],
       "citations": [
         "EVID-001",
@@ -344,57 +386,7 @@
         "seed:ACT-006",
         "docs",
         "deployment",
-        "ci-cd"
-      ]
-    },
-    "T-20251113-soc-api-validation": {
-      "seed_id": null,
-      "title": "验证 Rutgers SOC API 参数、限流与字段行为",
-      "type": "research",
-      "summary": "围绕 C2、R-20251113-soc-api-change 与开放问题 Q-01，对 Rutgers SOC API 的参数组合、拉取全校课程的策略、速率限制与返回字段进行实测，产出可复用的调用指南，降低后续抓取脚本与通知轮询的风险。",
-      "acceptance_draft": [
-        "整理出获取“单学期 + 校区全部课程”的推荐调用策略（例如按 subject/院系分批），并估算完整抓取一次所需时间。",
-        "记录 SOC API 的关键参数与典型响应样例，确认字段覆盖 FR-01/FR-02 所需信息，并标记可能缺失或不稳定的字段。",
-        "通过高频调用实验观察速率限制与错误码，给出可接受的最大抓取与轮询频率建议，并形成一份内部说明文档。"
-      ],
-      "lane": "now",
-      "priority_suggested": "P0",
-      "status": "todo",
-      "dependencies": [],
-      "blocked": true,
-      "blocked_by": [
-        {
-          "dep_id": "DEP-001",
-          "name": "Rutgers SOC API（courses.json / openSections）",
-          "type": "api",
-          "status": "unknown",
-          "doc_url": "https://github.com/anxious-engineer/Rutgers-Course-API",
-          "note": "需要在开发前期通过实测确认可用参数、速率限制和结构稳定性。"
-        }
-      ],
-      "unblock_plan": "在测试环境以较低频率尝试多种参数组合，逐步提高频率观察限流行为，将结果记录到 docs/soc_api_notes 中并同步到抓取脚本与通知配置。",
-      "estimate_h": 4,
-      "risk": "high",
-      "confidence": 0.65,
-      "interfaces_touched": [
-        "data-source",
-        "observability"
-      ],
-      "artifacts": [
-        "docs/soc_api_notes.md"
-      ],
-      "citations": [
-        "EVID-001",
-        "EVID-002",
-        "R-20251113-soc-api-change",
-        "Q-01"
-      ],
-      "created_at": "2025-11-13T00:00:00Z",
-      "updated_at": "2025-11-13T00:00:00Z",
-      "owner": "tbd",
-      "tags": [
-        "research",
-        "soc-api"
+        "local-deploy"
       ]
     }
   },
@@ -404,11 +396,11 @@
     "Q-03：空位订阅策略——当课程在满员/有空位之间频繁切换时，是只在首次开放时发送一次通知并自动删除订阅，还是每次开放都发送，或允许用户在界面中自定义策略？",
     "Q-04：Discord 通知形态——更偏向 Bot 私信（更即时但可能更易触发限流）还是频道广播（噪音更大但更稳），以及是否已有规划好的服务器/频道用于测试与正式使用？",
     "Q-05：周历“框选时间找课”的语义——你更希望框选的是“上课时间完全落在选中时间段内的课程”，还是“与该时间段不冲突的所有可选课程”？该功能期望在 MVP、v1.1 还是 v2.0 交付？",
-    "Q-06：校区与课程层次范围——MVP 阶段是否只支持 New Brunswick 本科课程？是否需要同时覆盖 Newark/Camden 或研究生课程（level=GR），以便提前规划数据抓取与 UI 维度？"
+    "Q-06：具体覆盖范围——在“需要同时支持多个学期、多校区以及本科/研究生课程”的前提下，MVP 期望最少覆盖哪些 term（例如当前学期 + 下一学期）以及哪些校区组合，以便规划数据抓取批量任务和前端筛选维度？"
   ],
   "assumptions": [
     "假设 Rutgers SOC 公开接口在 2025–2026 学年仍保持可匿名访问，且字段结构与调研时基本一致。",
-    "假设 MVP 阶段的主要用户为 New Brunswick 本科生，其他校区和研究生课程可以在后续迭代中扩展。",
+    "假设 MVP 阶段需要同时覆盖多个学期、多个校区以及本科和研究生课程，主要用户群体为 Rutgers 各校区的在读学生（含本科与研究生）。",
     "假设目标用户可以接受通过邮箱或 Discord 接收课程空位通知，不强依赖短信等其他通知渠道。",
     "假设无需用户登录即可通过带签名的退订链接完成订阅管理，用户能够理解并接受这种轻量级身份验证方式。"
   ],
@@ -571,6 +563,16 @@
         "EVID-008"
       ],
       "date": "2025-11-13T00:00:00Z"
+    },
+    {
+      "id": "D-20251113-coverage-multi-term-campus",
+      "summary": "MVP 阶段即支持多学期、多校区以及本科/研究生课程的联合浏览与筛选，不以单一校区本科课程为起点。",
+      "rationale": "真实选课场景中，学生往往需要在不同学期与不同校区的本科/研究生课程之间做整体规划；从一开始按多维度覆盖设计数据抓取与前端筛选，可以避免后续为扩展范围进行架构级重构。",
+      "citations": [
+        "EVID-001",
+        "Q-06"
+      ],
+      "date": "2025-11-13T00:00:00Z"
     }
   ]
 }
