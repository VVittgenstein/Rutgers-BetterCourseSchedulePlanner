# Term + Campus DB Ready Checklist

> 目标：把“建库完成”的含义具体化，确保任意 term + campus 在 SQLite 中具备完整数据、必备索引、可量化的校验与可追溯的元数据记录，从而支撑抓取脚本、查询 API 与空位提醒流程。

## 1. 就绪定义
- 面向对象：单个 `term_code` × `campus_code` 的数据切片。
- 信号：存在一条 `dataset_versions.status = 'active'` 记录指向最新 `courses`/`openSections` snapshot，且所有规范表/视图均已切换到该 version。
- 对外能力：`vw_active_courses` / `vw_active_sections` / `vw_open_sections` 可被 API 查询，空位提醒可读取 `open_section_states` 与 `open_section_events`，订阅视图可被消费。

## 2. 最小完备条件

### 2.1 原始快照与 metadata 层
| # | 必要产物 | 要求 | 校验方式 |
| --- | --- | --- | --- |
| 1 | `data/raw/<term>-courses-<campus>.json` | gzip + metadata (`sha256`,`etag`,`record_count`) 已写入 `raw_snapshots` | `SELECT record_count, sha256 FROM raw_snapshots WHERE term_code=? AND campus_code=? AND endpoint='courses';` |
| 2 | `data/raw/<term>-openSections-<campus>.json` | 与上，同步 `record_count = open_index_count` | 同上，endpoint=`openSections` |
| 3 | `dataset_versions` | `status='active'`、`courses_snapshot_id`/`open_sections_snapshot_id` 均指向最新快照；`promoted_at`/`notes` 已填 | `SELECT * FROM dataset_versions WHERE term_code=? AND campus_code=? AND status='active';` |
| 4 | `dataset_version_events` | 至少包含 `ingest`,`diff`,`promote` 三种事件，payload 记录新增/更新/删除计数 | `SELECT event_type, payload FROM dataset_version_events WHERE version_id=?;` |

### 2.2 规范化表、字段与索引
| 表/视图 | 关键字段/要求 | 索引/视图约束 | 覆盖率准线 |
| --- | --- | --- | --- |
| `courses` | 非空：`course_string`,`title`,`level`,`raw_payload`; 含 `synopsis_url`,`credits_min/max`,`has_prereq` | `PK(term_code,campus_code,course_string)` + `idx_courses_term_subject(term_code,campus_code,subject_code)` | `COUNT(*)` = `raw_snapshots(record_count)` for endpoint=`courses` per campus |
| `course_core_codes` | 多对多展开，允许 0..N | `idx_course_core(term_code,core_code)` | `core_code` 覆盖样本中全部 `coreCodes[]` |
| `course_attributes` | `attr_key` 标准化（`preReqNotes`,`courseNotes`,`campusLocations`…） | `idx_course_attr(term_code,attr_key)` | 稀疏字段全部下沉，无丢失 |
| `sections` | 非空：`index`,`course_string`,`section_number`,`open_status`,`status_text`,`campus_code`,`delivery_mode`,`exam_code`,`raw_payload` | `PK(term_code,index)` + `idx_sections_term_campus(term_code,campus_code)` + `idx_sections_open(term_code,campus_code,is_open)` | `COUNT(*)` = Σ `raw_sections`（即 snapshot `record_count_sections`） |
| `section_instructors` | 保留排序与 hash | `idx_section_instr(term_code,index)` | 与 `sections[].instructors` 条数一致（允许空 Section） |
| `section_comments` / `section_attributes` | 捕获 `comments[]`、`openToText`,`sectionEligibility` 等 | term/index 复合索引 | 不得遗漏任意 comment/attribute |
| `section_meetings` | `meeting_id`（hash）唯一；字段允 NULL 但 `meeting_mode_code` 必填 | `idx_section_meetings_term_day(term_code,meeting_day,start_minutes)` | 行数 ≥ `raw` meetingTimes 数；`meeting_mode_code` 非空率 = 100% |
| `section_campus_locations` | 映射到 buildings/campus | term/index 索引 | 行数 ≥ `sections[].sectionCampusLocations` 数 |
| `open_section_states` | 每个 `sections.index` 均存在；`is_open` 布尔；`source_version_id` 指向 active version | `idx_open_states_term_isopen(term_code,is_open,last_open_change_at)` | 行数 = `sections` 行数；`is_open` 与最新 openSections diff 一致 |
| `open_section_events` | 记录 opened/closed | `idx_open_events_index(term_code,index,detected_at)` | 变更时记录 `change_type`，最近 N 次脚本不为空 |
| `subscriptions`（可为空表） | schema 预留字段齐全 | `idx_subscriptions_index(term_code,index)` | 允许 0 行，但列定义不可缺 |
| `vw_active_courses` / `vw_active_sections` / `vw_open_sections` / `vw_subscription_candidates` | 视图引用 active version；列类型符合 API 约定 | `SELECT sql FROM sqlite_master WHERE type='view' AND name='vw_active_sections';` | 对应查询返回 ≥1 行；`EXPLAIN QUERY PLAN` 命中索引 |

### 2.3 数据完整性准线
- `courses.title`, `sections.index`, `sections.course_string`, `section_meetings.meeting_mode_code`, `open_section_states.is_open` 不允许 NULL。
- `sections.sync_version_id` 与 `open_section_states.source_version_id` 必须等于 `dataset_versions.version_id`。
- `open_section_states` 的 `is_open=1` 计数与 `openSections` 快照集合大小相符（允许跨 campus diff，但 NB/NK/CM 样本目前相同，应记录 SHA）。
- `raw_payload` 列（courses/sections/meetings）100% 非空，SHA256 可与原始 JSON 单条对象比对。

## 3. 准入校验步骤

### 3.1 自动脚本（建议命令）
```
python3 scripts/db_ready_check.py \
  --db data/courses.sqlite \
  --term 20261 --campus NB \
  --expected-courses 4608 --expected-sections 11680
```

脚本输出（stdout/JSON）必须包含：
```json
{
  "term_code": "20261",
  "campus_code": "NB",
  "dataset_version_id": "v2025-11-15-nb",
  "snapshots": {
    "courses": "snap_c5d1...",
    "open_sections": "snap_f32a..."
  },
  "metrics": {
    "courses": {"rows": 4608, "record_count": 4608},
    "sections": {"rows": 11680, "record_count": 11680},
    "open_index_count": 13780,
    "meeting_rows": 22564
  },
  "checks": {
    "schema": "pass",
    "indexes": "pass",
    "not_null": "pass",
    "sample_compare": {"sample_size": 10, "mismatches": 0}
  }
}
```
> 注：脚本名称可调整，但需至少提供 JSON/表格化指标供 CI 使用，同时以 exit code 判定通过/失败。

### 3.2 手动 SQL 校验（脚本内部亦应执行）
1. **库完整性**：`PRAGMA integrity_check;` 返回 `ok`。
2. **表存在**：`SELECT name FROM sqlite_master WHERE name IN (...);` 至少覆盖本清单列出的表/视图。
3. **索引存在**：`PRAGMA index_list('<table>');` 包含上文枚举索引名称，且 `origin='c'` 表示由 schema 创建而非自动。
4. **计数对齐**：
   ```sql
   SELECT 'courses' AS entity, COUNT(*) AS rows
   FROM courses WHERE term_code='20261' AND campus_code='NB';
   ```
   结果需与 `raw_snapshots.record_count` 相等（同理 `sections`,`section_meetings`）。
5. **非空检查**：
   ```sql
   SELECT COUNT(*) FROM sections
   WHERE term_code='20261' AND campus_code='NB' AND open_status IS NULL;
   ```
   应返回 0；同理检查 `courses.title`,`section_meetings.meeting_mode_code`,`open_section_states.is_open`。
6. **版本一致性**：
   ```sql
   SELECT DISTINCT sync_version_id FROM sections WHERE term_code=? AND campus_code=?;
   ```
   仅允许单一 version_id，且与 `dataset_versions.active` 相等。
7. **openSections Diff**：
   ```sql
   SELECT SUM(CASE WHEN is_open THEN 1 ELSE 0 END) FROM open_section_states WHERE term_code=? AND campus_code=?;
   ```
   与 `raw_snapshots` 中 openSections `record_count` 一致；若 Rutgers 返回全校区列表，应在备注中确认。

### 3.3 抽样对照流程
1. 以脚本随机抽取 ≥10 门课程与其 Sections，记录 `index`/`open_status`/`meetingTimes`。
2. 通过 Rutgers SOC 官方页面（或缓存 HTML）人工比对标题、Section 状态、授课模式、教师姓名、时间地点；允许 ≤1 个轻微格式差异，其余必须严格一致。
3. 将抽样结果写入 `dataset_version_events`（`event_type='sample_check'`）并追加到本文件的 metadata 表 `notes` 字段。

## 4. term/campus 元数据记录

### 4.1 填写模板
> 将下表追加在本文件底部，每次 Build 完成后新增一行；`dataset_version_id`/`snapshot_id` 使用数据库真实值；`script_sha256` 为运行脚本对应 Git 提交或文件 hash。

| term_code | campus | dataset_version_id | promoted_at (UTC) | courses_snapshot_id | open_sections_snapshot_id | fetch_command | script_sha256 | course_rows | section_rows | open_index_rows | sample_checked_by | notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |

### 4.2 示例（来自 Spring 2026 样本数据）
| term_code | campus | dataset_version_id | promoted_at (UTC) | courses_snapshot_id | open_sections_snapshot_id | fetch_command | script_sha256 | course_rows | section_rows | open_index_rows | sample_checked_by | notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 20261 | NB | v2025-11-15-nb | 2025-11-15T20:40:12Z | snap_courses_nb_c5d1 | snap_open_nb_f32a | `python3 scripts/fetch_soc_samples.py --year 2026 --term 1 --campuses NB` | `f7b38f9c…` | 4,608 | 11,680 | 13,780 | a.li | openSections SHA 与 NK/CM 相同，记录 diff=0 |

> 示例数据引用 `docs/db/sample_notes.md`；真实运行时请替换为实际 version/snapshot ID 与脚本哈希。

## 5. Go / No-Go 快速提问
- `dataset_versions` 是否存在 active 记录且日志事件完整？
- 表/索引/视图是否均通过自动脚本校验？
- 原始计数与入库计数一致？
- 随机抽样与官方 SOC 页面比对是否 100% 一致？
- Ready checklist 表格是否更新并提交？

满足以上条件即可宣布某个 term + campus “数据库已建好”，否则视为未完成。
