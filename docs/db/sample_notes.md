# Spring 2026 SOC Sample Notes

## Snapshot summary
- 命令：`python3 scripts/fetch_soc_samples.py --year 2026 --term 1 --campuses NB,NK,CM --tag spring-2026`
- 抓取时间（UTC）：2025-11-15T20:25:26Z（参见 `data/raw/spring-2026-metadata.json`）
- term/campus 组合：Spring 2026 (`year=2026`,`term=1`) × New Brunswick (NB)、Newark (NK)、Camden (CM)。
- 所有响应均为 HTTP 200，`courses.json` 返回 `Cache-Control: max-age=900`，`openSections.json` 返回 `max-age=30`。

## Raw files
### courses.json（含 sections）
| Campus | Courses | Sections | Distinct Subjects | File | Size (MiB) | ETag |
| --- | --- | --- | --- | --- | --- | --- |
| NB | 4,608 | 11,680 | 238 | `data/raw/spring-2026-courses-nb.json` | 19.8 | `"166384036"` |
| NK | 1,210 | 2,397 | 86 | `data/raw/spring-2026-courses-nk.json` | 4.2 | `"1462284679"` |
| CM | 963 | 1,752 | 71 | `data/raw/spring-2026-courses-cm.json` | 3.2 | `"1538375825"` |

### openSections.json（section index 列表）
| Campus | Open section indexes | File | Size (KiB) | ETag | SHA256 |
| --- | --- | --- | --- | --- | --- |
| NB | 13,780 | `data/raw/spring-2026-openSections-nb.json` | 108 | `"592079634"` | `cb92ce87…6781` |
| NK | 13,780 | `data/raw/spring-2026-openSections-nk.json` | 108 | `"1130589031"` | `cb92ce87…6781` |
| CM | 13,780 | `data/raw/spring-2026-openSections-cm.json` | 108 | `"1759824602"` | `cb92ce87…6781` |

> 元数据（headers、payload 尺寸、SHA256、记录数等）集中在 `data/raw/spring-2026-metadata.json`，下游脚本可直接读取。

## Coverage observations
- `courseDescription` 字段在三个校区样本中全部为空；`synopsisUrl` 覆盖度差异较大（NB 约 74%、NK 21%、CM 18%），说明需要依赖 `synopsisUrl` 而非 description 以获得课程简介。
- `coreCodes`、`meetingTimes`、`sections[*].index` 等结构与 Fall 2024/Fall 2025 样本一致，所有课程均附带至少一个 section（`empty_sections = 0`）。
- `openSections.json` 在 NB/NK/CM 返回完全相同的 13,780 个 index（SHA256 一致），推测该端点在 Spring 2026 目前仍提供“全校区”开放席位列表，即使附带 `campus` 参数。因此映射时需依赖 `courses.json` 中的 `sections[*].index` 来过滤校区。
- `courses.json` gzip 体积 NB ≈0.85 MB、CM/NK ≈0.18 MB，在解压后分别为 20 MB / 4.3 MB / 3.2 MB，可作为规划存储/缓存策略的参考。

## Field coverage & sparsity

> 统计来自 `python3 scripts/analyze_soc_sample.py --metadata data/raw/spring-2026-metadata.json`，覆盖 6,781 Courses / 15,829 Sections / 22,564 meeting times。

| 字段 | 覆盖率 | 说明 |
| --- | --- | --- |
| `title` | 6,781 / 6,781 (100%) | 课程主标题完整，适合作为 courses 主表的非空约束。 |
| `synopsisUrl` | 3,832 / 6,781 (56.5%) | 只有 ~56% 课程提供 synopsis 链接，需要在 schema 中允许 `NULL` 并在 UI 层 fallback。 |
| `courseDescription` | 0 / 6,781 (0%) | 全部为空。若需展示课程简介，必须依赖其他数据源或 synopsis 页面抓取。 |
| `preReqNotes` | 1,994 / 6,781 (29.4%) | 字段存在即可视为“有先修”，建议派生 `has_prereq` 布尔列用于 FR-02 筛选。 |
| `coreCodes[]` | 1,549 / 6,781 (22.8%) | 核心代码项稀疏但关键，建议拆解成 many-to-many 表或 JSON 列。 |
| `courseNotes` | 191 / 6,781 (2.8%) | 仅少量课程提供注册说明。 |
| `sections[].instructors` | 12,612 / 15,829 (79.7%) | 约 20% section 尚未指派教师，API/DB 需允许空值且在 UI 中显示 “Staff/TBA”。 |
| `sections[].sectionNotes` | 4,938 / 15,829 (31.2%) | 包含 PREREQ/仅限某项目等文本，适合作为全文索引列。 |
| `sections[].openToText` | 4,952 / 15,829 (31.3%) | 专业/年级限制说明，缺失时视为“无特殊限制”。 |
| `sections[].commentsText` | 7,271 / 15,829 (45.9%) | 通常用于 Hybrid/Online 标签，可拆出枚举字段（`comments[].code`）。 |
| `sections[].meetingTimes[].meetingDay` | 14,203 / 22,564 (62.9%) | 剩余 37.1% 为无具体星期（Online/ARR），需要在筛选时单独处理。 |
| `sections[].meetingTimes[].campusLocation` | 18,500 / 22,564 (82.0%) | 部分 Online meeting 缺失校区信息。 |
| `sections[].meetingTimes[].buildingCode` | 13,573 / 22,564 (60.2%) | 指定了教学楼的 meeting time 不足 2/3，过滤楼宇时需容忍缺失值。 |
| `sections[].examCode` | 15,829 / 15,829 (100%) | 每个 section 都携带期末考试代码 (`O`/`S`/`Y`)，可直接用于 FR-02 的“是否有期末考试”过滤。 |

附加说明：
- `openSections.json` 仅返回 index 列表，不含 course/section 额外字段；检测席位变化需比较集合差异，再 join Section 表更新 `is_open`。
- `meetingTimes` 即使在 Online 课程也至少包含一条记录，但 `meetingDay/startTime` 可能为空，同时 `meetingModeDesc` 会显示 `ONLINE`/`REMOTE`，应在 schema 中保留“模式”字段以便兜底显示。

## Suggested usage
1. 解析 `data/raw/spring-2026-metadata.json` 动态获取文件路径、记录数和校区列表，减少硬编码。
2. 将 `openSections` 的“全校区”行为视作风险，在后续 schema 与轮询逻辑中保留 `campus` 过滤兜底，并监控 SHA 变化以检测官方行为变更。
3. 在字段分析阶段重点关注 `sections[*].meetingTimes`、`coreCodes`、`campusLocations`，以及缺失 `courseDescription` 带来的 UI 影响。
