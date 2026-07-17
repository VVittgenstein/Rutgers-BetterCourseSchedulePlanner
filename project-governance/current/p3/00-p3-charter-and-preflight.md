# P3 Charter — 课程数据证据门与本地一键包实现计划

## 1. 状态与授权

- **阶段**：P3
- **状态**：`P3 PASS — FROZEN`
- **启动时间**：2026-07-13T02:47:33+08:00
- **scope correction**：初次人工转录漏掉官方 campus `D`；用户明确批准只补 Fall D，`01d/01e` amendment-002 已冻结，`CAT-C021` 已成功取得；不扩成两个 term × 15 campus
- **阶段边界修订**：按用户确认的产品心智模型，真实 `openSections` 共享基线属于完成本地一键包计划的 P3 输入；P4 只做公网包及部署 delta
- **Open停止、Review与恢复记录**：第一轮21个scope全部HTTP 200后，20/21 scope推翻旧orphan hard-failure；第二轮曾取消。用户随后批准Rutgers官方set-membership/intersection与双时钟；`21`冻结恢复amendment，第二轮21/21成功，`22`冻结完成，`23`冻结最终shared Open合同
- **用户授权**：`确认支持 Rutgers SOC 全部有效 campus；授权依序执行 P3–P6，证据冲突即停，最终停在 P6 Review。`
- **上游门**：P2于2026-07-13获得用户最终批准，状态为`P2 APPROVED — CLOSED`
- **产品范围**：Rutgers SOC当前官方选择器中全部有效campus；term不永久硬编码，证据运行按`01/01a`冻结的当期与相邻已发布term采样

P3批准范围只包含受控只读取证、分析、治理文档、notebook和实现计划。不得修改产品源码、安装/迁移产品依赖、构建用户包、删除旧链、运行生产部署或访问私有服务器/凭据。

## 2. 权威输入

1. `project-governance/current/single-mainline-delivery-workflow.md`
2. `project-governance/current/p2/00-p2-charter-and-preflight.md`
3. `project-governance/current/p2/02-file-semantic-matrix.md`
4. `project-governance/current/p2/03-capability-all-matrix.md`
5. `project-governance/current/p2/04-only-closure-matrix.md`
6. `project-governance/current/p2/05-reuse-and-port-matrix.md`
7. `project-governance/current/p2/06-filter-section-watch-contract.md`
8. `project-governance/current/p2/07-p2-validation-and-review-gate.md`
9. 当前允许的产品源码、测试、Compact历史证据和Git历史；旧chat log、deprecated workflow、旧P1禁区正文继续禁止读取

## 3. P3输出边界

P3必须先通过Catalog与共享Open真实证据门，随后才可冻结完整本地一键包计划。最终至少形成：

- 冻结的请求manifest、逐attempt ledger、raw hash/evidence register；
- 可复算notebook与数据质量技术报告；
- 字段完整性/类型/缺失率profile；
- Delivery与meeting oracle；
- section identity、collision、ingest/FTS/query证据；
- `openSections` shape、Rutgers官方merged-batch membership join、empty/error证据、QPS上界与共享poller contract；
- 本地一键包完整实现计划与P2→P7 traceability；
- P3验证与P4公网delta handoff gate。

P3在Open证据完成前不得宣称本地计划完成。`openSections` merged-batch join、合法空/异常空、reconcile、retry/QPS、3/10/30秒时钟映射与通知延迟边界必须在P3闭合或触发Review停止；P4不得重复取证后再通过P5回写P3。

## 4. 强制停止条件

以下任一发生时停止当前阶段并回到共同Review，不得静默修改P2合同或扩大取证：

- 需要突破`01a`的term/campus/request硬上限；
- 收到HTTP 403/429、非预期HTML、schema根部不是JSON array或超过响应大小上限；
- `(term,campus,index)`在同一scope内仍不能唯一标识section；
- REQUIRED筛选字段在已批准范围完全没有可信来源，且不能用明确`UNCERTAIN`语义安全承载；
- 真实raw迫使改变22行筛选全集、严格availability、三值代数或Delivery canonical边界；
- 需要猜测generic Online为Sync/Async、静默丢弃新值或用现有classifier自证；
- 请求预算耗尽仍无法关闭Catalog硬门。

单个罕见值未观察到不是自动冲突；必须标`NOT_OBSERVED`并保留unknown/fixture路径，不能声称“不存在”。

## 5. 原始证据边界

- 完整响应只写入Git已忽略的`data/staging/p3-rutgers-evidence-20260713/`。
- 当前治理目录只保存manifest、ledger、SHA-256、统计、最小fixture与解释。
- 不保存cookie、凭据、完整请求header、机器用户名或私有绝对路径。
- Rutgers课程与教师字段虽然来自公开来源，仍不把整份payload纳入Git或最终用户包。
