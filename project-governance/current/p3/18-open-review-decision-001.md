# P3 Open Review Decision 001 — JOIN 与 RATE 双时钟均已批准

## 1. 状态

- Decision status：`FROZEN_JOIN_AND_RATE_CLOCK_MAPPING`
- P3 gate at decision freeze：`P3_OPEN_ROUND_2_AMENDMENT_REQUIRED`
- P4–P6：`NOT STARTED`
- 新网络请求：`NOT AUTHORIZED UNTIL A NEW AMENDMENT IS FROZEN`

## 2. 用户已明确批准的内容

### JOIN-REV-01 — RESOLVED

采用 Rutgers 当前官网自己的处理方向：

1. 对一个 term/campus batch 分别取得合法 `openSections` arrays；
2. 合并、去重为该 batch 的 Open set；
3. 遍历 Catalog Sections，以 `openSet.contains(section.index)` 判断 Open；
4. Open set 中不能解释的值只做 orphan audit，不创建 Section，也不使批次失败；
5. 外部 Section identity 仍是 `(term,campus,index)`，不能退化为裸 index；
6. 只有完整且安全的 batch 才允许 absence → Closed；任一 target 失败、shape 异常或批次不完整时保留 last-known-good，不产生 Closed；
7. Closed → Open 只在新的 valid batch 中创建 episode，后续声音仍服从已批准的 ONE_SHOT / CONTINUOUS 语义。

### RATE-REV-01 — RESOLVED

- 公网包普通 Open refresh：沿用既有“普通用户不可配置”边界，固定值解释为 30 秒；
- 本地包普通 Open refresh：默认 30 秒；
- 本地包配置范围：3–3600 秒；
- 用户希望通知精度在 30 秒之内；
- active watch 所关联的共享 batch 目标为 10 秒。

## 3. 已批准的双时钟映射

用户于 2026-07-13 明确接受以下“两时钟”模型：

- `general_open_refresh_interval_sec`：无 active watch 的普通 Open 状态刷新。公网固定 30；本地默认 30、可配置 3–3600。
- `active_watch_poll_target_sec`：某 term batch 存在 active watch 时为 10 秒。
- 实际调度：`effective_interval = min(general_interval, 10)`；同一 batch 的两种到期只合并为一个 single-flight 请求，不重复请求。

因此，本地显式设为 3 秒时实际为 3 秒；设为 30 或 3600 秒但存在 active watch 时，watch batch 目标为 10 秒。公网无 watch 时 30 秒，有 active watch 时目标 10 秒。该模型修订此前“watch 完全不增加 Rutgers 请求”的合同：active watch 可以把相关共享 batch 提升到 10 秒；同一 batch 仍保持 single-flight，用户数或 watch 数不会进一步放大上游请求。

## 4. 下一动作：冻结 Round 2 恢复 amendment

`10b/10c` 已冻结“第二轮取消”的历史事实。恢复取证必须先新增 amendment，串联原 Open manifest、停止记录、本决策与延迟合同，并明确剩余请求 ID、预算、完整性前置条件和终止条件。amendment 冻结前，当前采集脚本不得继续；双时钟获批本身不等于网络恢复授权。

Post-decision progression：`21a/21b-open-round2-resumption-amendment-001` 已随后冻结；当前运行状态以 `20-p3-open-decision-gate.md` 为准。本文件继续保存裁决冻结时的门状态。
