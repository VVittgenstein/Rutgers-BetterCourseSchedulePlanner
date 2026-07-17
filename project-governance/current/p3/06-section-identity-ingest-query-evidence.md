# P3 Section Identity、Ingest 与 Query 证据

## 1. 状态与机器来源

- **Catalog 范围**：Fall 2026 当前全部 15 campus，加 Summer 2026 六个结构样本
- **Catalog evidence**：21 成功 payload / 22 attempts；CAT-E001 至 CAT-E021
- **Identity 状态**：`PASS WITH REQUIRED AUDITED DEDUP`
- **Query/FTS 状态**：source 与计划可冻结；产品 runtime 仍待实现验证
- **Open 状态**：真实两轮证据已在P3取得；Rutgers官网set-membership/intersection与双时钟已冻结

机器来源：`04a-catalog-profile.json`、`04b-catalog-scope-summary.tsv`、`04c-catalog-field-profile.tsv`、`06a-section-identity-summary.tsv`、`06b-filter-source-profile.tsv`。

本文件中的 raw count 与 collision count 是 OBSERVED；dedup、course variant、ingest/query 结构是由 observed facts 导出的实现约束。Open 的真实证据与停止门见 14 至 17，不再标记为 P4 provisional。

## 2. Section identity 完整统计

| Metric | Count | Decision |
|---|---:|---|
| section rows | 22,069 | raw staging 输入 |
| unique `(term,campus,index)` | 22,051 | normalized identity 基础 |
| duplicate composite keys | 18 | audited normalization required |
| duplicate composite extra rows | 18 | 不可直接建立 raw 唯一约束 |
| semantically equivalent duplicate keys | 18 | 可折叠，但必须保留审计 |
| semantically conflicting duplicate keys | 0 | 当前 sample 未观察到 |
| term/index cross-campus collisions | 3,125 | campus 必须属于 key |
| bare-index cross-scope collisions | 3,344 | 裸 index 不可全局唯一 |
| multi-object courseString groups | 41 | course group normalization required |
| course groups with multiple variants | 31 | explicit variant model required |
| course groups with section overlap | 0 | 当前 sample 未观察到；fixture 仍需覆盖 |

新增 Fall `D` scope 的 3 sections 没有引入新的 identity 冲突，因而 collision/duplicate 数保持不变，但 section rows 与 unique key 都增加 3。

## 3. Raw duplicate composite keys

当前 18 个 duplicate `(term,campus,index)` keys 都满足：

- 每个 key 有两个 raw records；
- 9 个位于 `92026/NB`，9 个位于 `92026/ONLINE_NB`；
- 每个 key 的 `distinct_semantic_section_hashes=1`；
- 每个 key 的 `distinct_courses=1`；
- 0 个 key 包含语义冲突。

因此 raw payload 不能直接对 `(term,campus,index)` 建唯一约束。正确 normalization contract：

1. 外部 section key 始终为 `(term,campus,index)`；
2. staging 先按 composite key 收集全部 raw records，禁止 last-write-wins；
3. 计算保留 provenance 的 canonical semantic hash；
4. 只有同 key 的所有 semantic hash 相同时才审计折叠；
5. 折叠记录 source request、raw record count、raw hash 集、semantic hash 与 reason；
6. 任一 key 出现多个 semantic hash，立即 FAIL 并返回 Review；
7. 唯一约束只施加在 normalized table，不在 raw staging 丢弃合法等价重复。

Catalog normalized identity 的结论为：

**PASS WITH REQUIRED AUDITED DEDUP**

这不等于 Open endpoint 能在同一 campus scope 对每个返回 index 做一对一 join；P3 Open 证据已经否定该假设。

## 4. Course group 与 variant

当前有 41 个 multi-object `courseString` groups：

| Scope | Groups |
|---|---:|
| 72026/NK | 6 |
| 72026/ONLINE_NK | 2 |
| 92026/CM | 3 |
| 92026/NB | 11 |
| 92026/NK | 19 |
| **Total** | **41** |

其中 10 个 group 只有一个 distinct variant signature，31 个 group 有两个 distinct variants；当前没有观察到 variant 之间复用同一 section index。

实现约束：

- course-centered UI 可用 `(term,campus,courseString)` 作为展示 group；
- ingest 不得对同 group 的多个 raw course object 使用 last-write-wins；
- storage 必须显式保存 course variant 或等价结构；
- variant signature 至少保留 supplement、credits、title、level 与 offering-unit 差异；
- section 附着在实际 variant，query 层再按 group 聚合；
- section overlap 当前为 NOT_OBSERVED，不可外推为永远不会发生，仍需冲突 fixture。

## 5. Target-scoped ingest 约束

21 个成功 payload 中，17 个非空、4 个为空；所有 22,069 个 section row 都有 index。基于这些 facts：

1. 每个 term/campus 是独立 staging target；单 target 的成功、空或失败不得 truncate 其他 target。
2. payload 必须先完成 root/type/row/key/duplicate validation，再以单 target transaction 发布。
3. observed empty array 是有效 HTTP/JSON 响应，但不能由一次 snapshot 推断为永久空。
4. raw duplicate 保留在 staging，normalized publish 执行第 3 节的 audited dedup。
5. 21 个成功 body SHA-256 继续作为 fixture/provenance anchor；产品包不得携带完整 raw payload。
6. normalized 内容改变时推进 content version；即使内容不变，成功 refresh checkpoint 仍应推进。
7. Catalog 与 Open 的 empty/error 语义必须分别定义，不能互相复制。

Fall campus `D` 已通过单独 CAT-C021 补齐：3 courses、3 sections、3 meetings、2 instructors。无需也不允许把该修正改写成 Summer 也必须覆盖全部 15 campus。

## 6. 22 个筛选源的口径

`06b-filter-source-profile.tsv` 是完整 22 行 source map。当前分母应以 21 scopes、10,629 courses、22,069 sections、30,804 meetings 为准。

关键 source 决策：

| Filter group | Source conclusion |
|---|---|
| Term / Campus | request scope；21/21 scopes |
| Subject / Course number / Level | structured course fields |
| Text search | raw text source 已观察；FTS runtime 未观察 |
| Credits / Core / Prerequisite | structured/sparse source；empty/unknown 语义须保留 |
| Section index / number | structured；composite identity + audited dedup required |
| Open status | Catalog snapshot 已观察；实时语义以 P3 Open 证据为准 |
| Delivery | T/H/O + occurrence conflict oracle；184 rows 为 `UNKNOWN_CONFLICT` |
| Instructor | name only；stable ID 未观察到 |
| Availability | meeting day/time；requiredness 未观察到时三值判定 |
| Exam | examCode 可用；structured `finalExam` 未观察到 |
| Permission / Eligibility | 多字段 sparse source；禁止合并为无来源的模糊值 |

Source feasibility 不等于产品已经端到端实现。当前仍未观察到 shared FilterSchema、目标 SQLite schema、FTS trigger/content sync、token-AND、exact identifier ranking、pagination total、same-section witness 与三值 reason 在产品 runtime 中运行。

## 7. FTS 证据边界

已观察到构建 FTS 所需的 raw source，例如 title、expandedTitle、subject、subjectDescription、courseNumber、courseString 与部分 notes。尚未观察到：

- normalized ingest 到目标 FTS schema；
- insert/update/target replace/delete 后的 content synchronization；
- exact identifier ranking 与 token-AND；
- page/total 不被前端 post-filter 改写；
- 空 description/notes 的安全处理。

因此：

**FTS REAL INGEST / QUERY = NOT_OBSERVED AS PRODUCT RUNTIME**

P3 可以冻结 source、schema、query 与 deterministic test plan；P7 实作时必须用缓存 raw 或最小去敏 fixture 验证真实运行结果。

## 8. Query contract 的实现约束

- 服务端统一执行 course predicates、same-section predicates、三值 reason、total 与 pagination；
- 先在每个 section 内聚合全部激活的 section predicates，再对 course 的 sections 做三值 OR；
- 不同 sections 分别满足不同条件时，course 不得被判为 MATCH；
- 缺失或 unknown provenance 必须从 DB 到 UI 保留；
- Core 提供明确 ANY/ALL；同维度多选默认 OR；
- Prerequisite 只有在来源能区分 empty/unknown 后才能可靠判定 has/none；
- Instructor 当前只能用 name 作为较低可靠性值，不能虚构 stable ID；
- Availability 使用 Delivery oracle 的 requiredness-unknown 与 TBA 规则；
- Eligibility 的 majors/minors/honors/eligibility/openTo 必须分维度解释。

最低 deterministic fixtures 包括：

1. 两个不同 sections 分别满足不同条件时 course 不 MATCH；
2. 一个 section NO_MATCH、另一个 UNCERTAIN 时 course 为 UNCERTAIN；
3. 等价 raw duplicate 可审计折叠；
4. semantic-hash 冲突 duplicate 必须 FAIL；
5. 相同 index 跨 campus 与跨 term；
6. 同 courseString 多 variants；
7. instructor 缺失、prerequisite empty、eligibility sparse；
8. FTS target replace 与 pagination；
9. H/online/TBA/requiredness-unknown availability。

## 9. Open 证据与当前 Gate

Catalog `section.openStatus` 的 observed 分布为 true 16,604、false 5,465；它只属于 Catalog snapshot。

P3 已按冻结manifest完成两轮共42个 Open 响应。第一轮推翻旧的same-scope orphan hard-failure后，历史停止记录保留在`17`；用户随后批准官方set-membership/intersection与双时钟，`21`通过hash链只恢复原manifest第二轮，21/21成功且没有新增Catalog请求或重试。最终证据表明：

- 42/42 observations均为HTTP 200、五位字符串数组，并通过官方term/campus Catalog intersection；
- 14/21 raw body pairs变化，3/21 effective intersection state pairs变化；
- Catalog boolean不能替代live Open，ETag也只能用于审计；
- 固定1秒旧值已废止；public fixed30、local default30/range3–3600、active-watch相关batch目标10已获批；
- 证据不证明Rutgers发布SLA、持续生产容量或真实seat变化后30秒内通知保证。

详细证据、历史Review与最终合同位于 `14`–`23`；当前权威总门为`09-p3-validation-and-freeze-gate.md`。

| Gate item | Result |
|---|---|
| Catalog raw identity | PASS |
| normalized composite identity | PASS WITH REQUIRED AUDITED DEDUP |
| weaker key rejected by collision evidence | PASS |
| course variant model requirement | OBSERVED / REQUIRED |
| 22-filter raw source map | PASS AS SOURCE MAP ONLY |
| FTS real ingest/query | NOT OBSERVED AS PRODUCT RUNTIME |
| Delivery oracle | PASS WITH EXPLICIT UNKNOWN PATHS |
| 旧Open same-scope exact join | **FAIL / SUPERSEDED** |
| Rutgers官方term/campus set-membership/intersection | **PASS / 42 OBSERVATIONS** |
| 3/10/30秒双时钟 | **APPROVED / FROZEN IN 23** |
| P3 total gate | **FREEZE CANDIDATE；FINAL VALIDATOR PENDING** |

Catalog scope、Open shape/join/transition与3/10/30合同均已解决；P4只等待P3最终计划、traceability、report和总validator通过。
