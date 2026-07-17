# P3 Delivery、Meeting 与筛选 Oracle

## 1. 状态与证据边界

- **Catalog 范围**：Fall 2026 当前全部 15 campus，加 Summer 2026 六个结构样本
- **Catalog 输入**：21 个成功 payload / 22 attempts（含一次透明 gzip 客户端失败）
- **Delivery/meeting 状态**：`PASS WITH EXPLICIT UNKNOWN/CONFLICT PATHS`
- **Open 状态**：真实两轮证据属于 P3；Rutgers官网intersection与双时钟已冻结在`23a/23b`
- **payload provenance**：`03-catalog-evidence-register.tsv` 的 CAT-E001 至 CAT-E021
- **机器来源**：`04a-catalog-profile.json`、`04c-catalog-field-profile.tsv`、`05a` 至 `05e`

本 oracle 不把历史 classifier 或字段名称直觉当作真值。它冻结当前 Rutgers Catalog raw fields、官方 T/H/O 分类、meeting occurrence 和显式 unknown/conflict 处理。

## 2. 当前总量

| Metric | Count |
|---|---:|
| courses | 10,629 |
| sections | 22,069 |
| meetings | 30,804 |
| instructor assignments | 19,202 |
| sections with instructor | 18,425 |
| Fall 2026 campus scopes | 15 |
| Summer 2026 structural scopes | 6 |

新增的 Fall 2026 `D` scope 贡献 3 courses、3 sections、3 meetings、2 instructor assignments。该请求补齐 Fall campus，不产生“两个 term × 全部 15 campus”的新要求。

## 3. Delivery modality source oracle

基础 modality 以 `section.sectionCourseType` 为主来源：

| Raw code | Count | Base interpretation |
|---|---:|---|
| T | 14,972 | Traditional / Face-to-Face |
| H | 808 | Hybrid |
| O | 6,289 | Online / Remote Instruction |

基础类型必须和 meeting occurrence 交叉验证。`05c-section-course-type-meeting-cross-tab.tsv`、`05d-section-course-type-time-evidence.tsv` 与 `05e-section-modality-consistency.tsv` 是精确机器表。

决策规则：

1. T/H/O 是基础 modality，不从地点为空或时间为空反向猜测类型。
2. 若 section type 与 meeting mode/location/time evidence 一致，保留基础类型并输出其 provenance。
3. 若二者冲突，结果必须为 `UNKNOWN_CONFLICT`，同时保留 raw type 与全部 occurrence。
4. 当前 observed delivery conflict 为 184 raw rows；不得以优先级、last-write-wins 或 UI 默认值静默覆盖。
5. 未观察到的新 code/description 必须保留 raw 并映射为 `UNKNOWN`，不能自动归入 T/H/O。

## 4. Synchronicity 与 meeting occurrence

同步性不能只从 O 或 H 推断。判定输入至少包括：

- meeting mode code/description；
- `meetingDay`、`startTime`、`endTime`；
- `baClassHours`；
- `campusLocation`、building、room；
- 官方 by-arrangement 规则；
- raw value 是否已在当前映射中明确解释。

规则：

- 有明确 scheduled day/time 的 occurrence 可以作为同步时间证据；
- 官方明确 by-arrangement/asynchronous 的值才可判为相应语义；
- generic Online 不等于 asynchronous；
- 缺时间、TBA、地点空或未知 raw code 均不能靠猜测补齐；
- H 只能说明 Hybrid，不能自动推出每个 meeting 的同步/异步结构；
- 新 raw tuple 必须进入 `UNKNOWN` 或 `UNKNOWN_CONFLICT`，并保留来源。

## 5. Availability 严格规则

用户定义的可用时间是硬约束。对每个 section：

1. 收集所有已知 meeting occurrences；
2. 对每个 occurrence 判定是否 required；
3. 每个已知 required occurrence 都必须完整落在用户可用区间内；
4. 任一已知 required occurrence 不满足，section 为 `NO_MATCH`；
5. requiredness、day 或 time 缺失且可能影响结果时，section 为 `UNCERTAIN`，不能偷偷当作 MATCH；
6. optional occurrence 只有在来源明确证明 optional 时才可忽略；
7. 多个 section predicate 必须在同一个 section 上同时成立，不能由不同 sections 各自满足后拼成 course MATCH。

当前证据没有提供通用、可靠的 required/optional 字段。因此安全实现必须保留三值结果 `MATCH / NO_MATCH / UNCERTAIN` 及 reason，而不是将未知 occurrence 排除。

## 6. 筛选字段 source 决策

`06b-filter-source-profile.tsv` 是 22 个筛选源的机器表。与 Delivery/meeting 直接相关的冻结口径：

| Filter | Frozen source | Evidence/result |
|---|---|---|
| Delivery modality | `section.sectionCourseType` + occurrence conflict check | 22,069 section rows；184 conflict rows → `UNKNOWN_CONFLICT` |
| Delivery synchronicity | mode code/desc、day/time、`baClassHours`、location | generic Online 不猜 Sync/Async |
| Availability | day/start/end + requiredness knowledge | requiredness 未观察到时保留 `UNCERTAIN` |
| Meeting campus/location | campusLocation/campusName | structured raw；合法 sparse |
| Building/room | buildingCode/roomNumber | 合法 sparse，不用 fallback 填充 |
| Exam | examCode/examCodeText | `finalExam` structured object 未观察到 |
| Special permission | add code/description | raw code 与 description 同时保留 |
| Eligibility | majors/minors/honors/eligibility/openTo | 多来源 sparse，不合并成模糊字符串 predicate |

## 7. 与 Open 证据的关系

本文件中的 Catalog `section.openStatus` 只是 snapshot 字段。P3 已经单独采集真实 `openSections`，并在以下文件闭合：

- `14-open-profile-join-and-error-evidence.md`：shape、cluster、join 与 error 证据；
- `15-poller-qps-latency-and-safety-model.md`：QPS、latency、cache 与安全模型；
- `16-shared-open-contract-review-options.md`：待审查合同选项；
- `17-p3-open-review-stop-gate.md`：证据冲突停止门。

旧orphan hard-failure被真实证据推翻；用户已批准Rutgers官网set-membership/intersection，Catalog boolean仍不能冒充实时状态。3/10/30秒双时钟、LKG和error/empty合同均已在P3冻结；P4只继承共享能力并设计公网差异。

## 8. Gate 结论

| Item | Result |
|---|---|
| Fall 当前 15 campus + Summer 六个结构样本 | PASS |
| T/H/O Section modality source | PASS |
| Section-level type/mode/time/location cross-tab | PASS |
| 184 raw conflict rows 不被静默覆盖 | PASS — `UNKNOWN_CONFLICT` |
| generic Online 不猜 Sync/Async | PASS |
| required/optional source | NOT_OBSERVED；三值 availability 安全承接 |
| 新/未知 raw value | PASS — preserve raw + `UNKNOWN` |
| Delivery/meeting oracle | **PASS WITH EXPLICIT UNKNOWN PATHS** |
| 旧Open same-scope exact join | **FAIL / SUPERSEDED** |
| Rutgers官方merged-set/intersection | **APPROVED / ROUND 2 PENDING AMENDMENT** |
| 3/10/30秒双时钟 | **APPROVED / FROZEN IN 23** |

Delivery/meeting 的 Catalog 子结论与Open合同均可冻结；仍须P3总validator通过后才授权进入P4。
