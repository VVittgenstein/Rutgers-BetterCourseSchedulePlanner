# P3 Catalog Request Manifest Amendment 001 — gzip client correction

## 1. Incident

- **状态**：`FROZEN TECHNICAL CORRECTION`
- **时间**：2026-07-13T02:53:54+08:00
- **原manifest SHA-256**：`6F6A71F5F8E2D492882FD364DB1B8510A46E2895AA484F0EDA9A19E37D353D90`
- **受影响ID**：`CAT-C001`（Fall 2026 / NB）
- **结果**：`NON_EVIDENTIARY_CLIENT_FAILURE`

首个HTTP GET已经到达`urllib.urlopen`的成功响应分支，随后客户端直接把以gzip编码的wire body送入UTF-8 JSON decoder，触发`UnicodeDecodeError`。进程在写ledger/raw前退出；没有继续执行`CAT-C002`–`CAT-C004`，没有保存或使用部分payload。

这不是Rutgers schema、字段、scope、HTTP错误或产品合同冲突；它是采集工具没有处理`Content-Encoding: gzip`的确定性缺陷。该attempt仍必须登记，不能伪装为“没有请求”。精确开始/结束时间、wire bytes与hash在崩溃后无法恢复，ledger明确留空并记录13.5秒tool runtime。

## 2. 修正

- 工具现在记录`Content-Encoding`、wire bytes/hash与decoded bytes/hash。
- 只接受空/identity或gzip；未知encoding、解压失败、wire/decoded任一超限均终止。
- Git ignored raw `.json`保存解压后的精确JSON bytes；wire body不保存，但记录SHA-256。
- 原20个target、单并发、5秒间隔、45秒timeout与零自动retry不变。

## 3. 唯一替代请求

本amendment只增加一个人工审查后的replacement ID：

```text
CAT-C001-R1 = GET courses.json?year=2026&term=9&campus=NB
replaces      CAT-C001
```

- 网络attempt总硬上限从20变为21，其中`CAT-C001`是不可用失败，最多仍只有20份可用Catalog payload。
- `CAT-C001-R1`只能attempt一次，失败即停止，不得再建R2。
- 不增加term、campus、optional query、并发或成功payload数量。
- 其余原manifest内容保持不可变。

该技术修正处于用户已授权的P3只读取证scope内，并已在主线公开记录；不将其解释为放宽任何证据或产品停止门。

