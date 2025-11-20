# Compact — ST-20251113-act-007-03-incremental-strategy

## 已落实事实
- 文档 `docs/data_refresh_strategy.md` 写明增量链路：term×campus×subject 队列调度、`courses.json` 阶段化→哈希比较→事务 upsert→缺失行删除、`openSections` 与订阅的职责、全量重建触发器与 runbook。
- 新 CLI `npm run data:incremental-trial` (`scripts/incremental_trial.ts`) 复用 `performProbe`，对指定 ≥3 个 subject 抓取 `courses.json`，标准化课程/Section 数据并计算 SHA1 `source_hash`，再通过模拟“上一版快照”来输出新增/更新/删除统计与耗时。
- `notebooks/incremental_trial.md` 记录了 NB 校区 198/640/750 三个 subject 的一次试跑结果（207 courses / 1,369 sections，Δ courses +2/-1/~1，Δ sections +4/-0/~1，总耗时≈700 ms），满足验收里的“≥3 个 subject 增量试跑”。
- `package.json` 新增脚本入口 `data:incremental-trial`，方便通过 npm script 调用。
- `record.json` 将父任务 `T-20251113-act-007-local-db-schema` 和此子任务状态标记为 done，并把上述文档、脚本、notebook 登记进 artifacts。

## 接口 / 行为变更
- 新增 CLI 接口：`npm run data:incremental-trial -- --term <code> --campus <code> --subjects <commaSeparated>`，用于本地验证哈希 diff 策略。仅做内存模拟，不触碰 SQLite。
- 文档新增对增量刷新流程与全量重建条件的正式描述，后续实现/运维需遵循该 runbook。

## 自测
- `npm run data:incremental-trial -- --term 12024 --campus NB --subjects 198,640,750`
  - 所有 subject 均成功抓取，分别输出课程/Section 统计及 Δ 结果，合计耗时 ~0.7 s，证明脚本在真实 SOC 数据上可运行并给出增量指标。

## 风险 / TODO
- CLI 当前只模拟前一快照（通过“ghost” 课程/section），尚未与 SQLite staging 表或真实 ingest 代码打通；后续需要把同样的标准化/哈希逻辑迁移到正式数据管道。
- 队列持久化、日志/指标输出、`openSections` 增量写入仍在文档层说明，未落地代码实现。
- 当 SOC 字段结构调整时需要同步更新 `scripts/incremental_trial.ts` 正常化逻辑，否则 hash 可能抖动。
