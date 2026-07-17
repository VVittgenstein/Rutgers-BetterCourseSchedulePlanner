# P3 Open 子门 — PASS

## 1. Current status

- Open raw shape：`PASS — 42/42 TWO-ROUND OBSERVATIONS`
- Rutgers official term/campus set intersection：`PASS — 42/42`
- Two-clock mapping：`APPROVED — PUBLIC 30 / LOCAL 30 RANGE 3–3600 / WATCHED TARGET 10`
- Empty/error/LKG、scheduler/backoff、observations/counters、episode/audio preconditions：`CLOSED IN 23`
- Source-change→notification hard 30s guarantee：`UNSUPPORTED / NOT PROMISED`
- Valid observation→server fanout：`ENGINEERING TARGET <=1s`
- Additional P3 network requests：`0 AUTHORIZED`
- Open subgate：`P3_OPEN_SUBGATE_PASS`
- P4–P6：`NOT STARTED`

Machine status: `P3_OPEN_SUBGATE_PASS`

## 2. Authority and handoff

`18a`冻结用户批准，`21b`冻结Round 2恢复allowlist，`22b`冻结42次完成hash，`23a/23b`已在候选验证通过后提升为`FROZEN_P3_SHARED_OPEN_CONTRACT`。本子门为`P3_OPEN_SUBGATE_PASS`；P3总体通过由`09-p3-validation-and-freeze-gate.md`记录。

P4只能继承该合同并设计公网delta；不得请求更多Rutgers数据、创建第二套Open合同或提高真实origin concurrency。
