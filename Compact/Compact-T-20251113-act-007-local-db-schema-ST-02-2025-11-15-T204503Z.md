## Confirmed facts
- `docs/db/field_dictionary.md` 现已梳理 Course→Section→Meeting→openSections 的层级关系，逐字段记录类型、是否必填、示例值与 FR-01/FR-02 映射，并附带对 synopsis 缺失、core/prereq 稀疏、openSections 仅含 index 等 schema 设计要点。
- `scripts/analyze_soc_sample.py` 新增为通用分析脚本，可读取 `data/raw/spring-2026-metadata.json` 中列出的 NB/NK/CM 样本，整合 courses/sections/meetingTimes/openSections 后输出字段覆盖统计（stdout + 可选 JSON），当前运行结果写入 `data/raw/spring-2026-field-stats.json`。
- `docs/db/sample_notes.md` 补充 “Field coverage & sparsity” 表格，列出 ≥10 个关键字段的存在比例（例如 `synopsisUrl` 56.5%、`courseDescription` 0%、`meetingDay` 62.9%、`instructors` 79.7%、`examCode` 100%）以及 openSections/meetingTimes 的特殊取值说明，为 schema 设计提供事实依据。

## Interface / artifact impact
- `scripts/analyze_soc_sample.py` 引入新的 CLI 接口：`python3 scripts/analyze_soc_sample.py --metadata <metadata.json> [--output <stats.json>]`，它依赖 `scripts/fetch_soc_samples.py` 生成的 metadata 契约，并输出字段覆盖统计供文档/后续任务引用。
- 新交付物 `docs/db/field_dictionary.md` 和 `data/raw/spring-2026-field-stats.json` 成为 T-20251113-act-007-local-db-schema 后续子任务的输入，要求消费最新统计而不是手工揣测字段。

## Risks / TODO
- 样本特征依赖 Spring 2026 快照；若 Rutgers SOC 字段或行为变动，需要重新运行 analyzer 并同步更新字典/稀疏度表，否则 DB schema 可能与真实数据漂移。
- `openSections.json` 目前忽略 campus 参数且返回全校索引，仅在文档中做出提醒；后续实现增量更新与通知时必须加上 campus 过滤兜底并监控 SHA 漂移。

## Self-test
- `python3 scripts/analyze_soc_sample.py --metadata data/raw/spring-2026-metadata.json --output data/raw/spring-2026-field-stats.json`
