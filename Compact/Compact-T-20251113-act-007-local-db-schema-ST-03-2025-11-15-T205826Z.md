## Confirmed facts
- `docs/db/conceptual_model.md` 给出了 ACT-007 的完整概念模型：自 SOC API 拉取后，先落盘 `data/raw/*.json`，再将 metadata 写入 `raw_snapshots`，经 `normalize_snapshot` 进入 staging，对比 `dataset_versions.active` 后切换到 canonical tables 并通过 `vw_active_*` 视图供 API/UI 使用，空位提醒只读 `vw_active_sections`/`vw_open_sections`。
- 文档枚举了 raw/normalized/derived 三层实体：`raw_snapshots`、`dataset_versions`、`courses`、`sections`、`section_meetings`、`open_section_states`、`open_section_events`、`subscriptions` 等，并指定主键/外键、需要保留 `raw_payload` TEXT 以保证 0 字段损失、未知字段落入 `*_attributes` 表的方式。
- 明确字段映射与索引策略：courseString→`courses.course_string`、section index→`sections.index` & `open_section_states.index`、meetingTimes→`section_meetings` 等；建议的查询索引覆盖 subject、校区、is_open、meeting_day/start_minutes，并约定 derived 视图供 API 直接消费。
- 描述全量刷新、增量刷新与 rollback 流程：staging→`dataset_versions`→差异 upsert、openSections 集合差生成 `open_section_events`，失败可丢弃 staging 并重跑；同时要求 `dataset_version_events`/日志记录 diff 数和来源 snapshot。
- Operational guardrails/retention：原始 JSON 至少保留 10 次快照，DB 中 `raw_snapshots` + `raw_payload` 可回放任意版本；迁移脚本需支持 `--from-empty` 和 `--promote-staging`, `openSections` 仍默认全校列表并需持续监控。

## Interface / artifact impact
- 新交付 `docs/db/conceptual_model.md` 作为后续 `db/schema.sql` 与 `scripts/migrate_db.*` 的唯一蓝图，也约定 `raw_snapshots`/`dataset_versions`/`open_section_events` 等新表与视图契约。
- `record.json` 中 ST-03 状态更新为 done，并将该文档列为 `latest_compact` 溯源，供上游/下游任务引用同样的模型假设。

## Risks / TODO
- 当前仅有文档，无实际 schema/migration 实现；后续任务需将 `raw_snapshots`、staging、views 等落地，否则 API/订阅仍不可用。
- 模型假设 `openSections` 返回全校索引并依赖 sections join；若 Rutgers 改变行为，需要更新 `open_section_states` 与 diff 逻辑。

## Self-test
- 未执行（documentation-only 变更）

## Code Review - T-20251113-act-007-local-db-schema-ST-03 - 2025-11-15T21:02:29Z
Codex Review: Didn't find any major issues. Can't wait for the next one!
