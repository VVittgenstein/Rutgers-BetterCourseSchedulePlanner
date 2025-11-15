# Compact · T-20251113-soc-api-validation-ST-02

## Confirmed Facts
- `docs/soc_api_test_report.md` 记录了 term+campus 组合的完整调用矩阵，明确 `courses.json` 仅识别 `year/term/campus`，所有 `subject/level/keyword` 参数均被忽略，典型响应体规模为 NB Fall ≈21 MB 解压/4,367 课程、NK Spring ≈4.67 MB/1,290 课程；`openSections.json`（NB Fall）约 70 KB/8,614 索引，可作为轮询主源 (`docs/soc_api_test_report.md:5-25`).
- 异常参数试验覆盖非法 campus、非法 term 以及缺失必填参数的 400 HTML 响应，为脚本配置错误检测提供标准 (`docs/soc_api_test_report.md:17-25`).
- 速率限制实验以 1→4→10 req/s 连续 58 次请求均返回 200，最大耗时 1.73s，文档推荐 `openSections` <=5 req/s、`courses.json` 遵守 15 min 缓存窗口 (`docs/soc_api_test_report.md:27-39`).
- 成功/失败响应样例（含 headers 与片段）与 `openSections` 列表被写入文档，可直接引用构建解析与监控 (`docs/soc_api_test_report.md:40-105`).
- 调用/监控建议列出了错误分类、退避策略与需要监控的 headers/字段；后续手册可直接复用 (`docs/soc_api_test_report.md:106-117`).
- `record.json` 将 ST-02 状态置为 `done`、`blocked=false`，更新时间 `2025-11-15T09:55:03Z`，并把 `docs/soc_api_test_report.md` 作为产出 (`record.json:110-155`).

## Interface / Behavior Impact
- 新文档定义了 SOC API 的可用参数、返回形态及速率建议，供抓取脚本和 ST-03 手册直接引用，属于对数据源调用策略的事实基线 (`docs/soc_api_test_report.md:5-117`).

## Risks / TODO
- 仍未触发官方限流，10 req/s 以上的上限与 429/503 恢复机制尚未知，需要后续并发/条件请求实验（已在文档 TODO 中提示） (`docs/soc_api_test_report.md:36-39`, `docs/soc_api_test_report.md:115-117`).

## Self-Test Evidence
- 自测通过 `requests` 脚本在生产端点顺序执行所有调用矩阵与速率实验，UA=`BetterCourseSchedulePlanner/0.1` 并启用 gzip (`docs/soc_api_test_report.md:1-3`).
