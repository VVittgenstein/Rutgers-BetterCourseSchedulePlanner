# P3 Catalog 最小 fixture 规则

本目录只保存为后续实现测试而人工构造的最小 fixture；它们不是 Rutgers 当前响应的副本，也不能替代 `03-catalog-evidence-register.tsv` 中按 SHA-256 登记的冻结原始证据。

## 证据政策

- 完整响应只位于被 Git ignore 的 `data/staging/p3-rutgers-evidence-20260713/catalog/`。
- 真实结论只能回溯到请求 ledger、原始 body SHA-256 与离线 profile。
- 本目录中的对象全部标记 `fixture_origin: CONSTRUCTED_FROM_OBSERVED_SHAPES`。
- fixture 不包含教师姓名、凭据、私有 inventory 或可识别个人的信息。
- `NOT_OBSERVED` 分支可用 constructed fixture 验证防护逻辑，但不得因此升级为 `OBSERVED`。

## 文件

- `delivery-and-time-cases.json`：覆盖 T/H/O、90/91/92/93、Thursday `H`、无安排时间与未知代码。
- `identity-duplicate-cases.json`：覆盖等价重复、矛盾重复和相同 course string 的 offering variant。

这些 fixture 的最终 schema 由 P7 实现任务确认；P3 只冻结需要覆盖的语义分支。
