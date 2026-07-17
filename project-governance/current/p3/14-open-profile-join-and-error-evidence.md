# P3 Open Shape、Join 与 Error 证据

## 1. Gate status

`HISTORICAL EVIDENCE FINDING — SAME_SCOPE_EXACT_JOIN CONTRADICTED`

> Post-Review status：用户已在后续Review中批准采用Rutgers官网的merged-set/intersection。本文的20/21 failure仍是推翻旧合同的有效证据，但不再表示新JOIN合同未决；当前门见`18`–`20`。

证据来自冻结 manifest 的第一轮 21 个 scope；全部 HTTP 200，第二轮在语义冲突确认后按用户规则取消。原始响应位于 Git ignored evidence，`OPEN-E001`–`OPEN-E021` 在 `12-open-evidence-register.tsv` 登记。

## 2. Raw shape

- 21 个响应根部均为 JSON array。
- 合计 151,086 个 raw values，全部为长度 5 的 string；没有 number/object/null、空字符串、首尾空白或 raw duplicate。
- 所有响应均为 gzip JSON，`Cache-Control: max-age=30`。
- 只证明当前观测形状；未来新类型必须拒绝危险 normalization，不能无条件 `String()`。

## 3. Response body clusters

| Term | Open rows | Campuses with identical body |
|---|---:|---|
| Fall 2026 | 12,645 | `NB,NK,CM,B,CC,H,MC,L,AC,J,D` |
| Fall 2026 | 1,099 | `ONLINE_NB,ONLINE_NK,ONLINE_CM`；是 12,645 集合的 subset |
| Fall 2026 | 0 | `CU` |
| Summer 2026 | 2,069 | `NB,NK,CM` |
| Summer 2026 | 829 | `ONLINE_NB,ONLINE_NK,ONLINE_CM`；是 2,069 集合的 subset |

相同 body 不是可长期硬编码的 campus-family SLA；它只证明当前参数响应不能按“每个 body 只含该 campus”解释。

## 4. Same-scope exact join failure

| Scope | Open indexes | Same-scope match | Orphan | Orphan rate |
|---|---:|---:|---:|---:|
| 92026/NB | 12,645 | 9,055 | 3,590 | 28.39% |
| 92026/NK | 12,645 | 2,101 | 10,544 | 83.38% |
| 92026/CM | 12,645 | 1,389 | 11,256 | 89.02% |
| 92026/ONLINE_NB | 1,099 | 751 | 348 | 31.67% |
| 92026/B | 12,645 | 0 | 12,645 | 100% |
| 92026/D | 12,645 | 3 | 12,642 | 99.98% |
| 72026/NB | 2,069 | 1,338 | 731 | 35.33% |
| 72026/NK | 2,069 | 520 | 1,549 | 74.87% |

21 scope 中 20 个违反“每个返回 index 必须在同一请求 scope 恰好 join 一个 Section”；唯一未失败的是 Catalog 与 Open 都为空的 CU。跨 scope 非去重求和为 16,600 matches 与 134,486 orphans，仅用于说明规模，不能当 distinct index 数。

Fall 全 15 campus Catalog union 可以解释 12,645 个返回值中的 12,640 个，仍有 5 个观测时点 orphan；这些返回值中有 1,080 个 index 在同一 term 的多个 Catalog campus scope 出现。Summer 的 Catalog 只采 6 个结构 scope，因此 term-union orphan 不能外推为全校 orphan。

## 5. 当前官方 SOC 的实际 join 方向

当前官方 `soc_app.CourseDownloadService.js`：

1. 把用户选中的 comma-separated campus 拆开；
2. 对每个 campus 分别请求 courses 与 openSections；
3. 把多个 Open arrays 直接合并；
4. `CourseAvailabilityUtils.updateAvailabilityForCourse` 遍历 Catalog Section，并以 `openSections.includes(section.index)` 更新 openStatus。

也就是说，官方 UI 是“Catalog Section 向 merged Open set 做 membership lookup”，不是“每个 Open value 必须反向属于该请求 campus”。额外 Open values 被自然忽略。

这为修改 contract 提供了权威依据，但不能由 Codex 静默替换已批准的 P2 same-scope exact-join 硬门。

## 6. Empty、absence 与 errors

- `92026/CU` 自然观察到 HTTP 200 `[]`，同时其 Catalog 也为空；这不能证明任意非空 target 的 `[]` 都安全。
- 21 次没有自然观察到 malformed、HTML、timeout、429 或 5xx；它们仍是 `NOT_OBSERVED`，只能用本地 fixture/injection 验证。
- Catalog snapshot 与 Open 观测时间不同：same-scope intersection 中合计有 48 个“Open返回但Catalog openStatus=false”和44个“Catalog openStatus=true但Open缺席”；Catalog `openStatus` 不能替代实时源，也不能用来裁定哪一侧绝对正确。
- 在 contract Review 前，Open absence 不得产生 Closed transition，HTTP 200 empty 也不得批量关闭 last-known state。

## 7. Conclusion

Raw shape 可安全解析；旧orphan hard-failure contract明确失败。该发现触发停止并促成用户批准官网set-membership/intersection。随后`21`恢复第二轮、`22`冻结42/42完成证据、`23`冻结最终合同；本文件前述第一轮数字保留为历史诊断，不再代表当前阻塞。
