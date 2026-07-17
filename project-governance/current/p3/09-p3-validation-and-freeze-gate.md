# P3 最终验证与冻结总门

## 1. 当前结论

- Catalog evidence gate：`PASS`
- Open two-round evidence gate：`PASS`
- Shared Open contract：`FROZEN_P3_SHARED_OPEN_CONTRACT`
- Local one-click implementation plan：`P3_LOCAL_ONECLICK_PLAN_FROZEN`
- Windows local storage amendment：`P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001 — INCORPORATED`
- P3：`P3_PASS`
- P4：`AUTHORIZED — NOT STARTED`
- P5–P6：`NOT STARTED — SEQUENTIAL DEPENDENCIES`
- P7：`NOT AUTHORIZED`
- P6 Review final approval：`NOT GRANTED`
- Further P3 Rutgers requests authorized：`0`

本文件是 P3 当前唯一总门。`10c`与`17`是第一轮冲突时的历史停止快照，必须保留，但不再代表当前状态。候选验证已通过，`07`与`23`已提升为FROZEN；notebook/report已由生成器重算并纳入最终机器验证。P4获得设计授权，但仍未在本门形成时启动。2026-07-13 的 P6 Review 已批准 Windows 本地存储拓扑修订并回写本门；这不重开或改写 P3 Catalog/Open evidence、不修改机器合同 `23b`，也不构成 P6 Review 最终批准或 P7 启动授权。

## 2. Catalog 子门

| 项目 | 结果 |
|---|---:|
| Catalog ledger attempts | 22 |
| 成功 payload | 21 |
| 保留的透明 gzip client failure | 1 |
| Fall 2026 当前有效 campus | 15 / 15 |
| Summer 2026 主/线上结构样本 | 6 / 6 |
| Course raw rows | 10,629 |
| Section raw rows | 22,069 |
| Meeting raw rows | 30,804 |
| 唯一 `(term,campus,index)` | 22,051 |
| exact-scope duplicate keys | 18，全部语义等价 |
| semantic-conflict duplicate keys | 0 |

Fall campus `D`由冻结amendment-002单独补齐；没有扩成两个term×15 campus。Section外部key必须为`(term,campus,index)`，Course group必须保留offering variant，Delivery未知值走三值逻辑。

## 3. Open 两轮证据子门

| 项目 | 结果 |
|---|---:|
| Frozen planned attempts | 42 |
| Round 1 / Round 2 | 21 / 21 |
| HTTP 200 successes | 42 / 42 |
| 五位字符串 | 302,125 |
| `Cache-Control: max-age=30` | 42 / 42 |
| 官方term/campus intersection PASS | 42 / 42 |
| Empty observations | 2（均对应空Catalog） |
| Unsafe empty observations | 0 |
| Raw body changed pairs | 14 / 21 |
| Effective state changed pairs | 3 / 21 |
| Observed request p95 | 1,501.020 ms |

`21b`只授权原manifest的21个Round 2 ID，串行、无重试、无新增Catalog；`22b`冻结完整ledger与全部派生表hash。历史Stop使用ledger前缀hash，最终完成记录使用42行完整ledger hash，两者不得混用。

## 4. 已批准产品合同

- `OpenBatchKey=(term,campus)`；不同campus不得为状态变更做动态union。
- 当前每batch恰好一个同key官方数组；未来多数组也只能在同一固定batch内全部验证后union。
- Orphan只审计/忽略，不创建Section；unsafe empty、nonempty zero-intersection、失败和Catalog version race均保留LKG且不得mass-close。
- Public普通Open固定30秒；local默认30秒、范围3–3600秒；active watch相关共享batch目标10秒，effective=`min(general,10)`。
- 每target single-flight；watch/user数不放大请求；Catalog/Open共享真实origin concurrency=1；EDF且不得饥饿。
- ETag只审计；每request、target observation、section observation分层记录；stale/UNKNOWN筛选返回UNCERTAIN。
- ONE_SHOT Max audible默认3且无产品上限，达限只静默；CONTINUOUS确认后同episode不重响，可靠Closed→Open才重新arm。
- 不承诺真实seat变化到通知严格30秒；只冻结valid observation到server fanout的`<=1s`工程目标。

### 4.1 P6 Review Windows 本地存储修订

`P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001` 只修订 Windows 本地 adapter 的物理存储与包生命周期：

- 唯一 live database 是以已解析 `RBCSP.exe` 所在目录为锚的 `<package-root>/data/rbcsp.sqlite`，CWD被忽略且禁止替代路径fallback；
- 一个物理数据库内分离 `OPERATIONAL` 与 `PERSONAL` logical domains；本地用户数据Reset只清PERSONAL tables，不删除数据库文件或Operational数据；
- 任何Rutgers请求前必须先通过路径可写、SQLite transaction/WAL/checkpoint与完整备份能力门；失败即停止；
- 最终archive不含DB/WAL/SHM、seed、backup、真实Catalog/Open raw或派生课程数据；首次运行只创建schema/migration metadata，再请求真实Rutgers数据；
- 升级必须保留`data/`并在migration前完整备份整个数据库；删除整个解压目录即删除程序及全部本地数据，其他目录不得残留fallback数据。

该修订不改变§4的任何共享Open产品合同。P6 Review仍在进行，P7仍未授权。

## 5. 可复算与限制

Catalog/Open raw bodies只通过ignored路径和SHA-256登记。Notebook与技术报告必须由生成器离线重算并执行，不得手改产物；P3 validator必须分别验证manifest、amendment、历史Stop前缀、最终completion hash、derived tables、traceability和报告来源。

当前证据不建立Rutgers schema/availability SLA、内部发布时间、长期缓存行为、持续3/10/30秒容量或更高真实origin concurrency。P7只能用fake upstream验证失败、退避、饱和、无饥饿和延迟；不得对Rutgers做压力测试。

## 6. 最终机器状态

```text
catalog_gate=PASS
open_evidence_gate=PASS
shared_open_contract=FROZEN_P3_SHARED_OPEN_CONTRACT
local_plan=P3_LOCAL_ONECLICK_PLAN_FROZEN
local_storage_amendment=P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001
local_database=data/rbcsp.sqlite
local_live_database_count=1
local_archive_real_data=FORBIDDEN
p3_gate=P3_PASS
p4_eligibility=TRUE
p4_status=AUTHORIZED_NOT_STARTED
p5_p6=NOT_STARTED_SEQUENTIAL_DEPENDENCIES
p6_review_final_approval=FALSE
p7_authorized=FALSE
further_p3_network_requests_authorized=0
```
