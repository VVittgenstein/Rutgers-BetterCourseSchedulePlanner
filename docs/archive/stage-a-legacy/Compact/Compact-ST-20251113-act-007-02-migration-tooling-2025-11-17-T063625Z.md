## Subtask
- ID: ST-20251113-act-007-02-migration-tooling
- Title: SQLite 迁移与初始化机制
- Context: Third follow-up after code review flagged that v001 must remain immutable for checksum-based migrator.

## 已确认事实
1. `data/migrations/001_init_schema.sql` 维持与最初提交一致的定义，`sections.index_number` 列级 UNIQUE 未再改动，保证历史环境里存储的 checksum 不会失效。
2. `data/migrations/002_relax_section_index_scope.sql` 负责真正的行为调整：通过“新表 + 数据复制 + 重命名”方式去掉列级 UNIQUE，仅保留 `(term_id, index_number)` 唯一索引，从而让 index 号在不同 term 中可复用。
3. `npm run db:migrate` 在清空数据库后可顺利依序执行 v001 → v002，证明现有迁移链条可冷启且不会触发 checksum 冲突。

## 接口 / 行为变更
- 新增 migration v002 仍是此子任务的对外行为；本轮改动确认 v001 不再变化，因此已有部署可直接运行 migrator 获取 v002，无需删除数据库。

## 自测
1. 删除 `data/local.db` 并运行 `npm run db:migrate`：CLI 依次应用 001 和 002，生成去除全局 UNIQUE 的 `sections` 表。

## 风险 / TODO
- v002 仍通过重建整张 `sections` 表来移除约束，对大体量数据库会暂占额外空间并持锁，需在部署手册中提示。
- 若已有部署曾手动篡改 `schema_migrations` 或跳过 v001，依赖 checksum 的 CLI 仍会拒绝执行；应确保运维场景遵循默认流程。

## Code Review - ST-20251113-act-007-02-migration-tooling - 2025-11-17T06:51:28Z
---review-start---
Codex Review
Here are some automated review suggestions for this pull request.

ℹ️ About Codex in GitHub
data/migrations/001_init_schema.sql
Comment on lines +103 to +106
    subject_code TEXT NOT NULL,
    section_number TEXT,
    index_number TEXT NOT NULL UNIQUE,
    open_status TEXT,

@chatgpt-codex-connector
chatgpt-codex-connector bot

P1 Badge Avoid globally unique section index_number

The sections table declares index_number as UNIQUE, which forces every section across all terms to have a distinct index number. The same migration later creates a uniqueness index on (term_id, index_number), implying index numbers should only be unique within a term. With the column-level uniqueness in place, loading sections for another term that reuses the same index number will fail with a constraint violation, preventing the database from holding multiple terms concurrently.

data/migrations/001_init_schema.sql
Comment on lines 104 to +105

    section_number TEXT,
    index_number TEXT NOT NULL UNIQUE,
    index_number TEXT NOT NULL,

@chatgpt-codex-connector
chatgpt-codex-connector bot

P1 Badge Avoid mutating applied migration 001

Altering the contents of 001_init_schema.sql (dropping the column-level UNIQUE on index_number) changes the checksum of version 001, but the migration runner in scripts/migrate_db.ts aborts when a previously applied migration’s checksum differs. Any environment that has already applied v001 will now hit Checksum mismatch for migration 001... on npm run db:migrate and cannot apply future migrations unless the database or schema_migrations entry is manually dropped. This effectively breaks upgrades for existing databases; the change should be delivered as a new migration version instead of mutating the original.

data/migrations/001_init_schema.sql
Comment on lines 103 to +105

    subject_code TEXT NOT NULL,
    section_number TEXT,
    index_number TEXT NOT NULL,
    index_number TEXT NOT NULL UNIQUE,

@chatgpt-codex-connector
chatgpt-codex-connector bot

P1 Badge Rewriting v001 migration breaks checksumed upgrades

Line 105 adds a column-level UNIQUE constraint to sections.index_number, which changes the contents of the already-applied 001_init_schema.sql. The migration runner (scripts/migrate_db.ts lines 136-146) stores and validates SHA256 checksums for each applied version; with this change any database that previously applied v001 will see a checksum mismatch and abort before reaching the new v002, blocking upgrades on all existing deployments that have run the migration once. This needs to be handled via a new migration instead of modifying the original file.
---review-end---

## Code Review - ST-20251113-act-007-02-migration-tooling - 2025-11-17T06:55:44Z
---review-start---
Codex Review: Didn't find any major issues. Keep it up!
---review-end---
