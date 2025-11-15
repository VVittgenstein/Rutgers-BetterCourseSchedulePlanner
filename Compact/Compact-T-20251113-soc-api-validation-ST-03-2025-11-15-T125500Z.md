# Compact · T-20251113-soc-api-validation-ST-03

## Confirmed Facts
- `docs/soc_api_handbook.md` 梳理了 `courses.json`、`openSections.json` 以及 `initJsonData` 的基础 URL、必填参数与示例 cURL，并强调只有 `year/term/campus` 会影响响应 (`docs/soc_api_handbook.md:1-28`).
- 手册给出 term×campus 拆分策略、缓存/ETag 利用、initJsonData 提取与跨学期循环的执行步骤，可直接转化为抓取脚本逻辑 (`docs/soc_api_handbook.md:30-35`).
- “安全节奏”章节定义了全量抓取与空位轮询的推荐频率、最大并发和退避策略，并记录需采集的 headers/指标 (`docs/soc_api_handbook.md:36-46`).
- 常见错误表以及日志/监控列表覆盖 campus 代码错误、缺参 400、并发限制、openSections 对齐与告警条件，确保调用者记录必要字段 (`docs/soc_api_handbook.md:47-66`).
- 落地建议要求脚本统一写入 `data/raw/{term}-{campus}-{endpoint}.json.gz` 并约束“策略改动需更新手册并挂验收”，将该文档设为唯一契约 (`docs/soc_api_handbook.md:70-72`).

## Interface / Behavior Impact
- 新增 `docs/soc_api_handbook.md` 作为 data-source 与 observability 组件的运行手册，固化参数模板、节奏、日志字段及报警要求，为抓取与通知轮询提供统一约束 (`docs/soc_api_handbook.md:5-72`).

## Risks / TODO
- 手册依据当前速率与参数行为撰写；若 Rutgers 引入新的限流或 API 字段变动，需要重新实测并更新本手册（文档结尾提醒需通过 PR 更新） (`docs/soc_api_handbook.md:36-43`, `docs/soc_api_handbook.md:70-72`).

## Self-Test Evidence
- 文档开头声明引用 `docs/soc_api_map.md` 与 `docs/soc_api_test_report.md` 的结论作为事实依据，未新增运行时代码，仅进行文档整合 (`docs/soc_api_handbook.md:1-4`).

## Code Review - T-20251113-soc-api-validation-ST-03 - 2025-11-15T12:55:00Z
Codex Review: Pending.

## Code Review - T-20251113-soc-api-validation-ST-03 - 2025-11-15T19:45:55Z
docs/soc_api_handbook.md
Comment on lines +15 to +20
# NB 2024 Fall 全量课程
curl --compressed \
  'https://sis.rutgers.edu/soc/api/courses.json?year=2024&term=9&campus=NB' \
  -H 'Accept: application/json' \
  -H 'User-Agent: BetterCourseSchedulePlanner/0.1' \
  -o cache/2024-9-NB-courses.json.gz


@chatgpt-codex-connector

P2 Badge Avoid saving decompressed JSON as .gz

The cURL snippet uses curl --compressed … -o cache/2024-9-NB-courses.json.gz, but --compressed both requests gzip and transparently decompresses the payload before writing it. The resulting file is plain JSON despite the .json.gz extension, while the rest of the document instructs readers to cache gzip files for later reuse. Any script that later tries to gunzip or rely on the compressed size will fail or misreport metrics. Consider dropping --compressed and letting gzip be stored, or piping the decompressed output through gzip before saving.

Useful? React with 👍 / 👎.
