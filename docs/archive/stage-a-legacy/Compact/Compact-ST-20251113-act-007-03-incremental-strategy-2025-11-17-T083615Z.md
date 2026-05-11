# Compact — ST-20251113-act-007-03-incremental-strategy (refresh after CR)

## 已落实事实
- 维持 `docs/data_refresh_strategy.md`、`notebooks/incremental_trial.md`、`record.json` 等前序交付不变；父任务与该子任务仍标记为 done。
- `scripts/incremental_trial.ts` 在 code review 指出问题后已修复 scenario 2 分支：现在在模拟“上一版快照”时，会保留被修改的 `firstCourse` 及其 ghost section，并改为删除列表末尾的另一个课程，使 meeting 变动与 ghost 删除都能正确出现在 diff 结果中，避免误判为整门课程新增。
- `npm run data:incremental-trial -- --term 12024 --campus NB --subjects 198,640,750` 再次执行成功，输出 3 个 subject 的新增/删除/更新统计与耗时，证明模拟数据的修复仍能通过真实 SOC 调用。

## 接口 / 行为变更
- CLI 接口仍是 `npm run data:incremental-trial`，但其 scenario 2 的内部模拟逻辑改变：不再 `shift()` 掉首个课程，而是移除队列尾部的其他课程，以保证被添加 ghost section 的课程仍存在于“旧快照”。外部调用方式不变。

## 自测
- `npm run data:incremental-trial -- --term 12024 --campus NB --subjects 198,640,750`
  - 运行成功，subject=750 的输出显示 Δ sections 为 `+1 / -1 / ~1`，说明 meeting 更新与 ghost section 删除都会体现在 diff 中，验证了 CR 修复的预期行为。

## 风险 / TODO
- 与前版 Compact 相同：CLI 仍是内存模拟，尚未写回 SQLite；队列持久化、日志/指标、`openSections` 实际增量更新等依旧在文档层。
- 若 SOC payload 结构变更仍需同步更新 `scripts/incremental_trial.ts` 中的标准化逻辑，以免 hash 抖动。

## Code Review - ST-20251113-act-007-03-incremental-strategy - 2025-11-17T08:40:34Z

---
Codex Review: Didn't find any major issues. 🎉
---
