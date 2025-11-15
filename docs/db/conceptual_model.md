# Local DB Conceptual Model (ACT-007)

> 目标：在不丢失 Rutgers SOC 任一字段的前提下，将 `courses.json` / `openSections.json` 转化为本地 SQLite（后续可换 PostgreSQL）中的“可信数据层”，既保留完整原始快照，也提供高性能的查询/通知视图，并支持跨学期、跨校区的增量更新。

## 1. Scope & guardrails

- **覆盖 FR-01/FR-02/FR-04**：所有课程、Section、meeting、空位、通知所需字段均入库，且开放查询需要 ≤200 ms（SQLite 内存 + 索引）。  
- **零字段损失**：原始 JSON 永远可复原；规范表中若无法映射字段，也需存储在 `raw_payload` 中。  
- **多 term / campus 支撑**：同一个 DB 可并行维护多个 term/campus 的“活动版本”，供 UI 选择。  
- **可回放增量**：每次拉取样本→解析→切换主表必须可追溯，允许重放或回滚到任意历史版本。  
- **低运营成本**：所有流程可在 `scripts/migrate_db.*`/`scripts/fetch_*` 中自动化，本地部署只需 Python + sqlite3。

## 2. Data lifecycle overview

```
SOC API ---> raw snapshot files (data/raw/*.json) ---> raw_snapshots (metadata + blob hash)
       ---> normalize_snapshot(year, term, campus, snapshot_id)
              ---> staging_* tables (courses/sections/meetings/open_sections + raw_payload)
              ---> diff vs active dataset_version
              ---> swap into canonical tables + update views
```

1. **拉取**：`scripts/fetch_soc_samples.py` 下载 gzip JSON，保存到 `data/raw/<term>-<endpoint>-<campus>.json` 并产出 metadata。  
2. **登记**：`ingest_raw_snapshot` 将 metadata 写入 `raw_snapshots` 表，包含 SHA256、term、campus、endpoint、路径、抓取时间。  
3. **归一化**：`normalize_snapshot` 解析 JSON，将每条 Course/Section/Meeting 及其 `raw_payload` 写入 `staging_courses`、`staging_sections` 等。  
4. **比对 + 切换**：计算 staging 与当前 `dataset_versions.active=1` 的差异，只 upsert 发生变化的记录。完成后更新 `dataset_versions`，视图 `vw_active_*` 自动切换。  
5. **通知/订阅消费**：空位提醒和 API 仅读 `vw_active_sections` + `vw_open_sections`，避免感知迁移过程。

## 3. Entity relationship map

### 3.1 Raw snapshot & metadata layer

| 表/视图 | 主键 | 关键字段 | 说明 |
| --- | --- | --- | --- |
| `raw_snapshots` | `snapshot_id (UUID)` | `term_code`, `campus_code`, `endpoint` (`courses`/`openSections`), `captured_at`, `sha256`, `file_path`, `etag`, `record_count`, `payload_bytes` | 每次 HTTP 抓取的登记表。形成“拉取事实”，可追溯至 `data/raw/*`。 |
| `raw_snapshot_blobs` | `snapshot_id` | `payload gzip/blob` | 可选：当部署不能长期依赖文件系统时，插入压缩内容，保证 DB 独立。在单机模式下只存储 metadata，payload 由文件路径引用。 |
| `dataset_versions` | `version_id (UUID)` | `term_code`, `campus_code`, `status` (`active`,`staged`,`archived`), `courses_snapshot_id`, `open_sections_snapshot_id`, `created_at`, `notes` | 表示一次成功的“归一化 + 上线”，用于回放增量。 |
| `dataset_version_events` | `event_id` | `version_id`, `event_type` (`ingest`,`diff`,`promote`,`rollback`), `payload` | 记录脚本运行日志、diff 计数等，方便可观测性。 |

### 3.2 Reference dimensions

| 表 | 主键 | 字段 | 来源 |
| --- | --- | --- | --- |
| `terms` | `term_code (e.g. 20261)` | `year`, `term_short`, `display_name`, `start_date`, `end_date` | 由配置 + SOC metadata 构建。 |
| `campuses` | `campus_code (NB/NK/CM)` | `description`, `region`, `timezone` | 固定表。 |
| `subjects` | `subject_code` | `description`, `school_code` | 由课程样本聚合而来。 |
| `schools` | `school_code` | `description` | 来自 `course.school`. |
| `buildings` | `building_code` | `campus_code`, `description` | 由 meetingTimes 聚合，供 UI 列表使用。 |

### 3.3 Canonical normalized tables

| 表 | 主键 | 关键字段 | 备注 |
| --- | --- | --- | --- |
| `courses` | `(term_code, campus_code, course_string)` | `subject_code`, `course_number`, `title`, `synopsis_url`, `credits_min/max`, `level`, `core_count`, `has_prereq`, `raw_payload` | `raw_payload` 保存 course JSON，保证 0 损失；其余列做查询索引。 |
| `course_core_codes` | `(term_code, course_string, core_code)` | `core_description` | 满足多对多。 |
| `course_attributes` | `(term_code, course_string, attr_key)` | `attr_value` | 如 `preReqNotes`, `courseNotes`, `campusLocations` 描述。键保存在标准列表中，未知字段也可落此表。 |
| `sections` | `(term_code, index)` | `course_string`, `section_number`, `open_status`, `status_text`, `campus_code`, `delivery_mode`, `comments_text`, `open_to_text`, `section_notes`, `exam_code`, `level`, `sync_version_id`, `raw_payload` | `sync_version_id` 指向 `dataset_versions` 方便定位“属于哪个版本”。 |
| `section_instructors` | `(term_code, index, instructor_hash)` | `display_name`, `sort_order` | 约 20% 为空，依赖左连接。 |
| `section_comments` | `(term_code, index, comment_code)` | `description` | 对应 `sections[].comments[]`. |
| `section_campus_locations` | `(term_code, index, campus_code, location_code)` | `location_description` | 反映 Section 层面的校区集合。 |
| `section_meetings` | `(term_code, index, meeting_id)` | `meeting_mode_code/desc`, `meeting_day`, `start_time`, `end_time`, `pm_code`, `building_code`, `room_number`, `campus_location`, `online_flags`, `raw_payload` | `meeting_id` = hash(index, start, end, meetingMode, buildingCode)；允许 null 时间。 |
| `section_attributes` | `(term_code, index, attr_key)` | `attr_value` | 捕获 `openToText`, `sectionEligibility`, `sessionDatePrintIndicator` 等稀疏字段。 |
| `open_section_states` | `(term_code, index)` | `is_open`, `last_open_change_at`, `source_version_id` | 由 `openSections` diff 驱动。 |
| `open_section_events` | `event_id` | `index`, `term_code`, `change_type (opened/closed)`, `from_version`, `to_version`, `detected_at` | 触发通知/队列的事实表。 |
| `subscriptions` *(后续任务)* | `subscription_id` | `index`, `contact_type`, `contact_value`, `filters_snapshot`, `created_at`, `archived_at` | schema 规划阶段需要 `index` 外键。 |

### 3.4 Derived views（供 API/UI 使用）

| 视图 | 定义 | 用途 |
| --- | --- | --- |
| `vw_active_courses` | 连接 `courses` + `terms` + `campuses`，限定 `courses.sync_version_id = dataset_versions.active` | 课程列表、搜索 API。 |
| `vw_active_sections` | `sections` + `section_meetings` + `section_instructors` + `open_section_states` | UI 主查询面，具备 `is_open`、时间、教师等列。 |
| `vw_open_sections` | 仅含 `index`, `term_code`, `campus_code`, `is_open`, `last_open_change_at` | 空位提醒轮询，快速取得 diff。 |
| `vw_subscription_candidates` | 预留视图，联合 `subscriptions` 与 `vw_open_sections` 触发通知。 |

## 4. Field mapping & retention strategy

1. **原始 JSON**：  
   - 文件层：继续保存 `data/raw/<term>-<endpoint>-<campus>.json`，遵守“不可覆盖，只追加 + 软链接 latest”。  
   - 数据库层：`raw_snapshots` 记录文件路径与 SHA；`courses.raw_payload`、`sections.raw_payload`、`section_meetings.raw_payload` 保存最小 JSON 对象，类型为 `TEXT`（UTF-8 JSON）。这样即使 `field_dictionary` 未来新增字段，也能从旧版本原样读取。  
2. **字段映射**：  
   - `courseString` ➜ `courses.course_string` (PK)。  
   - `sections[].index` ➜ `sections.index` (PK) + `open_section_states.index`。  
   - `meetingTimes[]` ➜ `section_meetings`。  
   - `openSections.json` ➜ `open_section_states.is_open`（TRUE if index in latest list）。  
   - 稀疏字段（`preReqNotes`, `sectionEligibility`, `comments[]`, `coreCodes[]`）映射到 attribute/多值表 + 保留在 `raw_payload`。  
   - 未知字段（未来 Rutgers 新增）直接写入 `section_attributes`/`course_attributes`，`attr_key` 保存 JSON path，保证“0 schema 阻塞”。  
3. **索引建议**（将在 `db/schema.sql` 实现）：  
   - `courses(term_code, campus_code, subject_code)` for subject filters。  
   - `sections(term_code, campus_code, open_status)` + covering index `(term_code, is_open, campus_code, meeting_mode, start_minutes)` to满足 UI 筛选。  
   - `section_meetings(term_code, meeting_day, start_minutes)` for时间筛选。  
   - `open_section_states(is_open, last_open_change_at)` for通知轮询。

## 5. Full vs incremental updates

### Full refresh（新 term / cold start）
1. `fetch_soc_samples` 抓取所需 term×campus，写入 `raw_snapshots`。  
2. `normalize_snapshot` 将课程、Section、Meeting 批量插入 `staging_*`，字段全部填充/或 null。  
3. `promote_version`：  
   - 创建 `dataset_versions` 记录（status=`staged`）。  
   - `INSERT INTO courses SELECT ... FROM staging_courses ON CONFLICT DO UPDATE`，同理 sections 等。  
   - 重建 `open_section_states`：所有 index 先置 `is_open=false`，再根据 openSections 快照更新 true。  
   - 设置 `dataset_versions.active=1`，并让 `vw_active_*` 指向 `version_id`。  
4. 写 `dataset_version_events`（记录新增/更新/删除数），供 `docs/db/sample_notes.md` 或未来监控引用。

### Incremental refresh（同 term/campus）
1. 仅对变动 campus 调用 SOC API；如果 ETag 未变化，可跳过。  
2. 对于有变化的 snapshot：  
   - 将原始 JSON 与最新 `raw_snapshots.payload` 做 SHA 比对，确认 diff。  
   - `normalize_snapshot` 生成新的 staging。  
   - **Diff 逻辑**：  
     - Courses：以 `course_string` 比较 hash（title+credits+attributes JSON）。发生变化的课程写入 `courses`，并记录 `course_change_log`（course_string, version_id, change_mask）。  
     - Sections：以 `index` 为主键比较 `raw_payload_hash`；若 `open_status` 变化，也写入 `open_section_events`。  
     - Meetings/子表：先删除 `version_id=active` 且 `index` 在变更列表中的记录，再整体插入 staging 中 `index` 的 meeting 子集。  
   - `open_section_states`：读取新的 openSections 列表，与当前 `is_open=1` 做集合差 Diff，生成 opened/closed events（`change_type` + `detected_at`），写入 `open_section_events`，再更新 `last_open_change_at`。  
3. 将新版本标记 active，同时把上一版本设为 `archived`（保留 30 天可回滚）。  
4. 若某次脚本失败，可直接删除 `staging_*` + `dataset_versions` 中 `status='staged'` 的记录并重跑，正式表不受影响。

### Rollback
- 通过 `dataset_versions` 选择目标版本，更新 `courses.sync_version_id`、`sections.sync_version_id` 以及 `open_section_states.source_version_id` 即可。  
- 因为 `raw_snapshots` 与 `raw_payload` 均保留，可重新运行 `normalize_snapshot(version_id)` 还原任何旧版本。

## 6. Operational notes

- **Retention**：原始文件按 term/campus/endpoint 存储，至少保留最近 10 次拉取；DB 中的 `raw_snapshots` 提供 `sha256` 检测重复，可在 CI 中验证文件未被篡改。  
- **Migration scripts**：`scripts/migrate_db.*` 将创建上述所有表、索引、视图，并支持 `--from-empty`（全量）与 `--promote-staging version_id`（增量）两种模式。  
- **Observability**：`dataset_version_events.payload` 使用 JSON（包含新增/更新/删除计数、持续时间、source_snapshot_ids）并输出到日志，供 `docs/db/sample_notes.md` 汇总。  
- **Future extensions**：  
  - Subscription 逻辑可直接引用 `open_section_events`，无需额外 Join `openSections.json`。  
  - 若需要持久化更多历史，可引入 `section_status_history`（index, detected_at, open_status）表；其原始数据来自 `open_section_events`。  
  - 当迁移到 PostgreSQL 时，`raw_payload` 可改为 `JSONB` 以支持索引查询，但 conceptual model 不变。

该模型保障原始数据的完整保留、结构化查询性能与增量更新可追溯性，满足 T-20251113-act-007-local-db-schema-ST-03 的验收标准，并为后续 schema (`db/schema.sql`) 与迁移脚本提供清晰蓝图。
