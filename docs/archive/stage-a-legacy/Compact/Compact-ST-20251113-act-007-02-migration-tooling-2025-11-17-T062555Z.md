## Subtask
- ID: ST-20251113-act-007-02-migration-tooling
- Title: SQLite 迁移与初始化机制
- Context: Second compact after addressing checksum regression (new migration v002).

## 已确认事实
1. 保持 `001_init_schema.sql` 与最初版本一致（`sections.index_number` 列级 UNIQUE 仍存在），确保已应用该迁移的环境不会因 checksum 变化而阻塞后续升级。
2. 新增 `002_relax_section_index_scope.sql`：
   - 临时构建 `sections_new` 表，去掉列级 UNIQUE 约束，同时保留所有字段/外键。
   - 将原 `sections` 数据全量拷贝到新表并替换旧表，恢复 `idx_sections_term_index`（唯一）和 `idx_sections_term_subject_status` 索引，使 index number 仅在 term 内唯一。
   - 迁移执行前关闭 foreign_keys，结束后重新打开，以避免重建期间触发约束。
3. 迁移链条测试：清理 DB 后运行 `npm run db:migrate`，CLI 依次应用 v001 和 v002，表示新迁移可冷启。

## 接口 / 行为变更
- 数据库 schema 版本新增 v002，部署/CI 需要重新运行 `npm run db:migrate` 以移除全局唯一性；应用端无需改动即可受益。

## 自测
1. 删除 `data/local.db` 并执行 `npm run db:migrate` → 依序应用 001 与 002，成功生成无列级 UNIQUE 的 `sections` 表。

## 风险 / TODO
- 迁移使用“建新表 + 拷贝 + 替换”的方式，若数据库中 `sections` 数据量极大，执行此次迁移可能暂时占用额外磁盘和锁；需在部署文档中提示。
- 已经运行旧 schema 的环境需要确保在升级前备份数据库，以防迁移重建过程中意外中断导致数据不一致。
