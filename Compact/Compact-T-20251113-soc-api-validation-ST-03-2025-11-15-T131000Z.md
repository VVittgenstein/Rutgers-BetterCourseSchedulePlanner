# Compact · T-20251113-soc-api-validation-ST-03 (2025-11-15T13:10:00Z)

## Confirmed Facts
- `docs/soc_api_handbook.md` 的 NB Fall cURL 示例现通过 `curl -sSL ... --compressed | gzip > cache/2024-9-NB-courses.json.gz` 保存 gzip 原样，避免 `.json.gz` 实为明文 JSON 的风险 (`docs/soc_api_handbook.md:15-20`).
- 端点与参数表继续强调仅 `year/term/campus` 生效，并提供 `courses.json`、`openSections.json` 与 `initJsonData` 的推荐 URL 模板 (`docs/soc_api_handbook.md:5-28`).
- 批量策略章节给出 term×campus 拆分、ETag/cache 利用与 initJsonData 更新流程，可直接指导脚本实现 (`docs/soc_api_handbook.md:30-35`).
- 安全节奏章节定义全量抓取与空位轮询的频率、并发与退避策略，并列出需记录的 headers/指标 (`docs/soc_api_handbook.md:36-46`).
- 常见错误、日志字段与落地建议条目仍要求记录请求/响应/性能指标，并将手册作为脚本策略的单一契约 (`docs/soc_api_handbook.md:47-72`).

## Interface / Behavior Impact
- 仅更新文档；手册现在明示如何在缓存目录中存储真正的 gzip 文件，避免客户端 gunzip 失败，同时继续作为 data-source/observability 组件的操作基线 (`docs/soc_api_handbook.md:5-72`).

## Risks / TODO
- 若 Rutgers 调整压缩行为或增加限流，需要重新验证并更新示例/节奏（文档末尾提醒修改策略需 PR 更新） (`docs/soc_api_handbook.md:36-72`).

## Self-Test Evidence
- 通过人工审阅确认示例命令保存 gzip，与前文缓存策略一致；未涉及运行时代码 (`docs/soc_api_handbook.md:15-20`).

## Code Review - T-20251113-soc-api-validation-ST-03 - 2025-11-15T13:10:00Z
Codex Review: Pending.

## Code Review - T-20251113-soc-api-validation-ST-03 - 2025-11-15T19:52:16Z
Codex Review: Didn't find any major issues. Chef's kiss.
