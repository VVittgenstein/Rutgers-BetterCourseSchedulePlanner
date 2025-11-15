# Spring 2026 SOC Field Dictionary

> 数据来源：`data/raw/spring-2026-*.json` 与 `data/raw/spring-2026-field-stats.json`（由 `scripts/analyze_soc_sample.py` 生成）。统计口径覆盖 Spring 2026 term 的 NB/NK/CM 三校区共 6,781 门课程、15,829 个 section 以及 22,564 条 meeting time 记录。

## Hierarchy & identity

| 实体 | 数据来源 | 主键/唯一标识 | 关系 | 备注 |
| --- | --- | --- | --- | --- |
| Course | `courses.json` | `courseString`（示例 `01:198:111`） | 拥有 1~N 个 Section；字段包含院系、学分、简介、core codes 等。 | 满足 FR-01 “列出完整课程信息” 的基础单位。 |
| Section | `courses.json` 内嵌 | `sections[].index`（5 位或 6 位字符串） | 从属于某 Course；包含授课模式、教师、状态等；`index` 也是空位提醒的主键。 | 满足 FR-01/FR-02 对 Section 状态、教师、时间地点的展示与筛选。 |
| MeetingTime | `sections[].meetingTimes[]` | 组合键 `index + meetingDay + meetingModeCode + startTimeMilitary + buildingCode` | 描述 Section 在某天/地点/模式下的一段时间段，可为空（Online/ARR）。 | 支撑 FR-02 的“按星期/时间段/授课模式筛选”。 |
| OpenSectionIndex | `openSections.json` | 字符串 index | 独立列表，表示当前开放的 Section index，需与 `sections[].index` join。 | 三个校区样本均返回 13,780 个 index（SHA 相同），因此必须在 join 时再按 campus/subject 过滤。 |

## Course-level fields

| 字段 | 类型 | 必填 | FR 映射 | 示例 | 备注 |
| --- | --- | --- | --- | --- | --- |
| `title` | string | ✅ | FR-01（列表展示/搜索） | `ODASIS PROGRAM` | 100% 覆盖，课程主标题。 |
| `subject` + `courseNumber` | string | ✅ | FR-01/FR-02（代码筛选） | `001` + `161` | 组合成官方课程代码；`courseString` 已拼接 School:Subject:Course。 |
| `subjectDescription` | string | ✅ | FR-02（按院系/subject 分组） | `Exchange` | 与 `subject` 组合呈现 subject 名称。 |
| `school` | object `{code, description}` | ✅ | FR-02（按学院/School 筛选） | `{"code":"01","description":"School of Arts and Sciences"}` | `offeringUnitTitle` 在样本中恒为空，因此需使用 `school.description` 作为院系名称。 |
| `offeringUnitCode` | string | ✅ | FR-02（学院代码过滤） | `01` | 与 School code 一致，可作为数据库外键。 |
| `credits` / `creditsObject` | number / object | `credits`: 89.7%；`creditsObject`: ✅ | FR-01/FR-02（学分展示 & 范围过滤） | `creditsObject.description = "1.0 credit"` | `credits` 为空时，可退回 `creditsObject.code`。 |
| `synopsisUrl` | string | 56.5% | FR-01（课程简介链接） | `https://www.amesall.rutgers.edu/...` | `courseDescription` 在样本中 100% 为空，需依赖 synopsis 页面补充简介。 |
| `preReqNotes` | string (HTML 片段) | 29.4% | FR-02（“是否有先修”过滤 + 展示） | `(01:013:140 ... )<em> OR </em>(01:074:140 ...)` | 字段存在即代表官方标注了先修/共修要求。 |
| `courseNotes` | string | 2.8% | FR-02（特殊说明过滤） | `Register in person at any SAS Advising Center` | 常用于注册说明/限制，此字段较稀疏。 |
| `coreCodes[]` | array<object> | 22.8% | FR-02（核心代码筛选） | `{"coreCode":"HST","description":"Historical Analysis"}` | 每个元素含 `coreCode` 与描述，可多选。 |
| `campusLocations[]` | array<object `{code, description}` | ✅ | FR-01/FR-02（校区/校区简称） | `{"code":"2","description":"Busch"}` | Course 级别的校区集合，与 Section 级别字段交叉验证。 |
| `openSections` | integer | ✅ | FR-01（整体开放 Section 数） | `0` | 反映课程下“开放 Section 数”。若需细粒度状态必须读取 Section 列表。 |
| `level` | string (`U`/`G`) | ✅ | FR-02（本科/研究生过滤） | `U` | 可作为“课程级别”过滤的直接来源。 |

## Section-level fields

| 字段 | 类型 | 必填 | FR 映射 | 示例 | 备注 |
| --- | --- | --- | --- | --- | --- |
| `sections[].index` | string | ✅ | FR-01（Index 展示）/FR-02（精确搜索、订阅主键） | `24075` | 全局唯一，openSections.json 同样返回该值。 |
| `sections[].number` | string | ✅ | FR-01（Section 展示） | `Z2` | 与 index 组合匹配 Rutgers 页面。 |
| `sections[].openStatus` + `openStatusText` | boolean/string | ✅ | FR-01/FR-02（状态过滤） | `False` / `CLOSED` | `openStatus` 为布尔值，`openStatusText` 包含 `OPEN/CLOSED/WL`。 |
| `sections[].campusCode` | string | ✅ | FR-02（校区过滤） | `NB` | 与 `sectionCampusLocations[].description` 配合展示校区和教学楼。 |
| `sections[].sectionCampusLocations[]` | array<object | ✅ | FR-01（教学楼/校区展示） | `{"code":"2","description":"Busch"}` | 可映射到 buildings/campus 表。 |
| `sections[].meetingTimes[]` | array<object | ✅ | FR-01/FR-02（时间地点/授课模式） | 见下节 | 每个 Section 至少包含 1 个 meetingTime，Online 课 meetingDay 可能为空。 |
| `sections[].instructors[]` + `instructorsText` | array<object/string | 79.7% | FR-01/FR-02（教师列表/过滤） | `KOERBER` | 约 20% Section 尚未指派教师，UI 需容忍为空。 |
| `sections[].subtitle` | string | 10.3% | FR-01（子标题展示） | `ELEMENTARY PERSIAN II` | 子标题常用于专题课/语言课。 |
| `sections[].sectionNotes` | string | 31.2% | FR-02（特殊限制提示） | `PREREQ: ...` | 常含“仅限 majors/minors”等文案。 |
| `sections[].openToText` | string | 31.3% | FR-02（专业/年级限制） | `MAJ: 014 ...` | 搭配筛选“仅展示对我开放的 Section”。 |
| `sections[].sectionEligibility` | string | 6.3% | FR-02（年级/项目限制） | `JUNIORS AND SENIORS` | 极少数 Section 提供更明确的 Eligibility。 |
| `sections[].commentsText` | string | 45.9% | FR-01（在线/Hybrid 等信息） | `Hybrid Section - Some Meetings Online ...` | 搭配 `comments[]` 结构化标签（code+description）。 |
| `sections[].examCode` + `examCodeText` | string | ✅ | FR-02（是否有期末考试） | `O` / `No final exam` | Rutgers 官方编码：`O`=无期末，`Y`=有期末，等等。 |
| `sections[].sessionDatePrintIndicator` | string (`Y`/`N`) | ✅ | FR-02（短学期/日期信息） | `Y` | 用于判定 session Dates 是否在 SOC 页面打印。 |

## Meeting time fields (`sections[].meetingTimes[]`)

| 字段 | 类型 | 必填 | FR 映射 | 示例 | 备注 |
| --- | --- | --- | --- | --- | --- |
| `meetingModeCode` + `meetingModeDesc` | string | ✅ | FR-02（授课模式过滤） | `98` / `ROOM BLOCK` | 所有记录均存在。在线课程通常显示 `ONLINE`/`REMOTE`. |
| `meetingDay` | string (`M`~`U`) | 62.9% | FR-02（按星期筛选） | `M` | 37.1% 的 meetingTime 没有具体星期，通常表示 ARR/ONLINE，需要结合 `meetingModeDesc`。 |
| `startTimeMilitary` / `endTimeMilitary` + `pmCode` | string | 62.9% | FR-02（起止时间过滤） | `1701` / `2230` / `P` | 缺失时多为 `ARR` 或未排定时间，需在 UI 中标记为 “Time TBA”。 |
| `campusLocation` + `campusName` | string | 82.0% | FR-02（按校区/地点筛选） | `2` / `BUSCH` | 主要针对线下/Hybrid Meeting。 |
| `buildingCode` / `roomNumber` | string | 60% | FR-02（教学楼/教室过滤） | `ARC` / `203` | 线上课程无值。 |

## openSections.json

- 结构：简单的字符串数组（示例 `["00053", "00054", ...]`），共 13,780 个 index。
- 三个 campus 调用结果 SHA256 完全一致，说明当前 API 忽略 `campus` 参数并返回全校区开放列表。
- 由于 `openSections` 不携带课程/校区信息，必须与 `sections[].index` join 并通过 `sections[].campusCode`/`sectionCampusLocations` 再次过滤，才能满足 FR-02 “按校区/时间筛选空位”。
- 增量更新策略：对 `openSections` 列表取集合差异，即可判定 Index 新增/关闭；该策略将用于空位提醒任务（依赖 FR-02 的“及时感知 OPEN 状态”）。

## Key observations for schema design

1. **课程简介需依赖外链**：`courseDescription` 在 Spring 2026 样本中完全为空，仅 `synopsisUrl` 具有 56.5% 覆盖，因此数据库 Schema 应保留 synopsis URL，并允许前端 fallback 到学校/院系页面（FR-01）。
2. **Prereq/Core 稀疏但关键**：`preReqNotes`（29.4%）和 `coreCodes`（22.8%）代表 FR-02 中“有先修/核心”筛选，字段存在即可视为 true，入库时建议额外维护 `has_prereq` / `core_code_list` 派生列提升查询效率。
3. **时间地点存在“ARR/Online”缺口**：仅 62.9% meetingTime 具备 `meetingDay/start/end`，但 100% 提供 `meetingModeCode`，说明 DB/查询 API 需要将“无具体时间”作为特殊值，避免筛选时误判（FR-02）。
4. **教师字段并非 100%**：20.3% section 没有 `instructors`，过滤 “按教师” 时必须包含 “未指定/Staff” 选项，同时空值不应阻塞订阅（FR-01/FR-02）。
5. **openSections 端点需配合 Section 列表使用**：因为返回全校区索引，schema 必须以 Section.index 为事实主键，并额外维护布尔列 `is_open`（由 openSections 列表驱动），以保证筛选与空位提醒一致（FR-01/FR-02）。

上述字典为后续数据库 schema（T-20251113-act-007-local-db-schema）建模提供依据，可直接将字段/类型映射到 SQLite 表结构与索引设计中。
