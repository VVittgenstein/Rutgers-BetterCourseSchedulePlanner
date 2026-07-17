## Subtask
- ID: ST-20251113-act-007-02-migration-tooling
- Title: SQLite 迁移与初始化机制
- Acceptance summary: 提供版本化迁移脚本、支持 CLI 参数和日志、在空目录与已有数据库上重复执行而不破坏数据。

## 已确认事实
1. `scripts/migrate_db.ts` 新增：
   - 使用 `better-sqlite3` 连接指定数据库，开箱开启 WAL/foreign_keys，自动建表 `schema_migrations` 并记录版本、checksum、应用时间。
   - 扫描 `data/migrations` 下按 `<version>_<name>.sql` 命名的脚本，校验 SHA256 checksum，按顺序在事务中执行并在 `data/migrations.log` 追加日志；`--dry-run` 和 `--verbose` 控制执行与输出。
   - CLI 支持 `--db/--database`、`--migrations`、`--log-file` 参数，缺值会抛错，默认数据库为 `data/local.db`。
2. 新增 `data/schema.sql` 与 `data/migrations/001_init_schema.sql`，内容一致，定义了 Terms/Campuses/Subjects/Courses/Sections 等业务表、索引、订阅/事件表及 `course_search_fts` 虚拟表，作为版本化 schema 的首个迁移。
3. `package.json`/`package-lock.json`：添加依赖 `better-sqlite3@^12.4.1`，并提供 `npm run db:migrate` 脚本调用迁移 CLI。
4. 文档更新：`README.md` 新增“Database migrations”段落，说明 CLI 用法与参数；`docs/local_data_model.md` 指明 `data/schema.sql` 为模型的权威定义；`.gitignore` 忽略 `data/*.db` 和迁移日志，避免将本地 DB 产物纳入版本控制。

## 接口 / 行为变更
- 新的 npm Script `db:migrate` 及 CLI 接口对依赖本地 SQLite 的流程提出约束：其他脚本需在读写数据库前调用该命令以确保 schema 已初始化/升级。
- 项目运行多了 `better-sqlite3` 原生依赖，部署环境需要兼容其 Node ABI 并允许写入 `data/local.db` 与 `data/migrations.log`。

## 自测
1. `npm run db:migrate`（空目录）→ 成功应用 `001_init_schema`，生成 `data/local.db` 和日志。
2. `npm run db:migrate`（已有数据库）→ 输出 “Database already up to date.”，验证幂等。
3. `npm run db:migrate -- --dry-run` → 未执行 SQL，仅验证参数解析与日志分支（命令成功退出）。

## 风险 / TODO
- 当前仅提供 CLI 与 schema 文件，尚未将其集成到抓取/服务启动流程；后续需要在部署/CI 中调用 `npm run db:migrate` 以避免遗忘导致 schema 失配。
- 迁移 CLI 仅有手动测试，缺乏自动化验证来检查表/索引是否存在，建议后续补充冒烟测试或在 CI 中运行一次迁移并校验关键表。
