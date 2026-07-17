# P3 Rutgers Catalog 字段 Profile

## 1. 状态、范围与证据边界

- **Catalog profile 状态**：`COMPLETE / CATALOG SUB-GATE PASS`
- **Fall 2026 范围**：Rutgers SOC 当前 15 个有效 campus：`NB`、`NK`、`CM`、`ONLINE_NB`、`ONLINE_NK`、`ONLINE_CM`、`B`、`CC`、`H`、`CU`、`MC`、`L`、`AC`、`J`、`D`
- **Summer 2026 范围**：6 个结构样本：`NB`、`NK`、`CM`、`ONLINE_NB`、`ONLINE_NK`、`ONLINE_CM`
- **成功 payload**：21
- **HTTP attempts**：22；其中 CAT-C001 是客户端未解 gzip 的透明、非证据性失败，替代请求 CAT-C001-R1 成功
- **证据登记**：`03-catalog-evidence-register.tsv` 中 CAT-E001 至 CAT-E021

本文件只解释 Catalog 证据。P3 的真实 `openSections` 证据、join 冲突、轮询频率与停止门已经记录在 `14-open-profile-join-and-error-evidence.md` 至 `17-p3-open-review-stop-gate.md`；不得再把它们推迟为 P4 provisional。

机器可复算来源：

| Source | 用途 | 当前状态 |
|---|---|---|
| `02-catalog-request-ledger.tsv` | request、HTTP、bytes、body hash、raw 路径 | 22 attempts：21 SUCCESS、1 FAILURE |
| `03-catalog-evidence-register.tsv` | CAT-E001 至 CAT-E021 的 provenance | 21 evidence rows |
| `04a-catalog-profile.json` | 聚合 profile、identity、分布与 raw body hashes | `successPayloads=21` |
| `04b-catalog-scope-summary.tsv` | 逐 scope 行数、bytes、duration、hash | 21 rows |
| `04c-catalog-field-profile.tsv` | 全量字段 presence/type/sparsity | 字段级权威表 |

## 2. 请求与 payload 完整性

| Metric | Value | Evidence status |
|---|---:|---|
| ledger attempts | 22 | OBSERVED |
| successful payloads | 21 | OBSERVED |
| non-evidentiary client failures | 1 | OBSERVED |
| course rows | 10,629 | OBSERVED_MULTI_SCOPE |
| section rows | 22,069 | OBSERVED_MULTI_SCOPE |
| meeting rows | 30,804 | OBSERVED_MULTI_SCOPE |
| instructor assignments | 19,202 | OBSERVED_MULTI_SCOPE |
| sections with at least one instructor | 18,425 | OBSERVED_MULTI_SCOPE |
| successful non-empty scopes | 17 | OBSERVED_MULTI_SCOPE |
| valid empty JSON-array scopes | 4 | OBSERVED_MULTI_SCOPE |

四个空 payload 均来自 Fall 2026：`B`、`CU`、`MC`、`J`。它们都是 HTTP 200 和有效 JSON array，只证明本次 snapshot 为空；不能外推为 campus 永久无课程、无效或不受支持。

补充的 Fall 2026 campus `D` 请求为 CAT-C021，返回 3 courses、3 sections、3 meetings、2 instructor assignments。它补齐 Fall 当前 campus 覆盖，没有把证据范围扩张为“两个 term × 全部 campus”。

## 3. Scope 口径

逐 scope 精确数值、body SHA-256 和 request duration 以 `04b-catalog-scope-summary.tsv` 为权威。本文件不复制全部 hash，以避免人工文档与机器产物漂移。

范围结构如下：

| Term | Scope | 数量 | 用途 |
|---|---|---:|---|
| Fall 2026 (`92026`) | 当前全部 15 campus | 15 | 当前完整 campus 覆盖 |
| Summer 2026 (`72026`) | 3 个主 campus + 3 个 online campus | 6 | 跨 term 与 online 结构样本 |
| **Total** | 成功 payload | **21** | Catalog profile 输入 |

所有 22,069 个 section row 都有非空 index；所有 observed section 都至少有一个 `meetingTimes` 对象。后者只说明数组存在 meeting 对象，不表示每个 meeting 都有可判定的星期、时间、地点或 requiredness。

## 4. 字段 Profile 的读法

`04c-catalog-field-profile.tsv` 是字段 presence/type/sparsity 的权威表。读取时应区分：

- `parent_rows`：该对象类型的 observed 总行数；
- `non_null`：字段值不是 null；
- `non_empty`：按 profile 工具规则判断为非空；
- `scope_count`：出现该对象的非空 payload scope 数，不是请求总数；
- key 存在不等于值可用，也不等于字段长期稳定或具备产品语义。

当前对象总量为：course 10,629、section 22,069、meeting 30,804、instructor assignment 19,202。Instructor 只有 `name` 被观察到，没有 stable instructor ID；`finalExam` 结构仍未观察到有效对象。

## 5. 关键离散分布

| Field/distribution | Observed counts | Interpretation |
|---|---|---|
| `section.openStatus` | true 16,604；false 5,465 | Catalog snapshot，不等于实时 Open 证据 |
| `section.sectionCourseType` | T 14,972；H 808；O 6,289 | 官方 T/H/O raw code；冲突处理见 05 |
| instructor object type | object 19,202 | assignment 均为对象，但仅观察到名称来源 |
| sections with instructor | 18,425 / 22,069 | 合法 sparse；不可虚构 instructor |

Delivery/meeting 的 raw mode、day/time、location、冲突交叉表继续以 `05a` 至 `05e` 为准。当前共有 184 个 delivery raw conflict rows，必须进入 `UNKNOWN_CONFLICT`，不得静默覆盖。

## 6. Identity 结论

| Metric | Count | Decision |
|---|---:|---|
| section rows | 22,069 | raw rows |
| unique `(term,campus,index)` | 22,051 | normalized identity 基础 |
| duplicate composite keys | 18 | 必须 audited dedup |
| semantically equivalent duplicate keys | 18 | 可审计折叠 |
| semantically conflicting duplicate keys | 0 | 当前样本未观察到 |
| term/index cross-campus collisions | 3,125 | 证明 campus 不可从 key 移除 |
| bare-index cross-scope collisions | 3,344 | 证明裸 index 不可作为全局 key |

完整 duplicate、course variant 与 ingest/query 约束见 `06-section-identity-ingest-query-evidence.md`。

## 7. Catalog 能支持与不能支持的结论

可以冻结的 observed facts：

- 21 个成功 payload 已由 ledger、evidence register、scope summary 与 raw body hash 闭合；
- 课程、section、meeting 和 instructor 的当前字段 grain/type/sparsity 已可复算；
- 外部 section key 必须保留 `(term,campus,index)`；
- raw duplicate 只能在 semantic hash 相同后审计折叠；
- 缺失、null、空字符串和空数组必须作为真实数据状态保留；
- T/H/O 与 occurrence 冲突必须显式产生 unknown，而不是猜测。

仍不能从 Catalog 单独证明：

| Claim | Status | Reason |
|---|---|---|
| raw 字段长期稳定 | NOT_OBSERVED | 每个 scope 只有一个成功 snapshot |
| stable instructor ID | NOT_OBSERVED | 仅观察到 `instructor.name` |
| structured final-exam occurrence | NOT_OBSERVED | `finalExam` 未出现有效对象 |
| 空 prerequisite 等价于“无先修” | NOT_OBSERVED | empty 与 unknown 的来源语义未闭合 |
| Catalog `openStatus` 可替代实时 Open | REJECTED | P3 Open 证据已证明二者不能互相替代 |
| 空 Catalog payload 证明 campus 永久无课 | NOT_OBSERVED | 只有一次空数组 snapshot |
| FTS 已在产品 runtime 正确运行 | NOT_OBSERVED | 本阶段只冻结 source/schema/query/test 计划 |

## 8. P3 Open 边界与当前 Gate

真实 Open 已在 P3 采集，而不是留给 P4。第一轮证据推翻旧的“orphan即失败”规则；用户随后批准Rutgers官网set-membership/intersection与双时钟。恢复amendment冻结后，第二轮21/21成功；最终合同见`23a/23b`。

因此最终状态为：

| Gate item | Result |
|---|---|
| Catalog scope / hash / field profile | PASS |
| Fall 2026 当前 15 campus 覆盖 | PASS |
| Summer 2026 六个结构样本 | PASS |
| Delivery/meeting oracle | PASS WITH EXPLICIT UNKNOWN PATHS |
| Identity normalization | PASS WITH REQUIRED AUDITED DEDUP |
| FTS product runtime | NOT_OBSERVED；留待实现验证 |
| 旧Open same-scope exact join | FAIL / SUPERSEDED BY APPROVED OFFICIAL JOIN；见14、18 |
| Rutgers官方set-membership/intersection | APPROVED + 42/42 observations PASS |
| 3/10/30秒双时钟 | APPROVED；见18–23 |
| P3 总门 | **OPEN EVIDENCE PASS；FINAL PLAN/VALIDATOR FREEZE** |

Catalog与Open数据子门均已闭合；P3只剩最终计划、traceability、report与总validator冻结，不存在Catalog campus缺口或“两个term × 全部campus”要求。
