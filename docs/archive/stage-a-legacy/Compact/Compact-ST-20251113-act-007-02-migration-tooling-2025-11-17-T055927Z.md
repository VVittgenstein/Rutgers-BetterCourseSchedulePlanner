## Subtask
- ID: ST-20251113-act-007-02-migration-tooling
- Title: SQLite 迁移与初始化机制
- Context: Follow-up fix after first code review (global uniqueness issue on `sections.index_number`).

## 已确认事实
1. `data/schema.sql` 与 `data/migrations/001_init_schema.sql` 均已移除 `sections.index_number` 列级 `UNIQUE` 约束，仅保留 `idx_sections_term_index (term_id, index_number)`，确保 index 号只在 term 内唯一，可并存多学期数据。
2. 迁移脚本内容仍与数据模型一致（该列 NOT NULL，其他字段未变），因此后续迁移链条不受影响。

## 接口 / 行为变更
- 迁移结果：同一 index number 可在不同 term 中重用，避免导入多学期数据时触发唯一性冲突；对上层 ingestion/订阅逻辑无额外步骤需求。

## 自测
1. 删除 `data/local.db` 后执行 `npm run db:migrate` → 成功重新应用 `001_init_schema`，验证 CLI 及 schema 仍可冷启。

## 风险 / TODO
- 若已有环境曾运行带列级 UNIQUE 约束的旧 schema，需要重新初始化或执行手动 `DROP/CREATE` 以匹配最新迁移；目前尚无自动补丁（首个迁移版本直接更新）。
