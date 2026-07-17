# P3 Rutgers Catalog 只读请求 Manifest

> **Post-run correction（2026-07-13）**：本冻结 manifest 的 discovery 人工转录漏掉 active campus `D`。`01d/01e-catalog-request-amendment-002` 只增加 `CAT-C021`（Fall 2026 / D）并已成功完成。有效证据范围现为 Fall 2026 全部当前 15 campuses，加上 Summer 2026 六个 main/online 结构样本；本次纠正没有、也不需要扩成两个 term × 15 campuses。

## 1. 冻结状态

- **状态**：`FROZEN BEFORE FIRST COURSES.JSON REQUEST`
- **冻结时间**：2026-07-13T02:47:33+08:00
- **机器版本**：`01a-catalog-request-manifest.json`
- **机器版本SHA-256**：`6F6A71F5F8E2D492882FD364DB1B8510A46E2895AA484F0EDA9A19E37D353D90`
- **执行工具**：`tools/acquire_catalog_evidence.py`
- **技术amendment**：`01b/01c-catalog-request-amendment-001`（首个gzip客户端失败；一个同scope replacement；成功payload上限不变）
- **完整raw目录**：`data/staging/p3-rutgers-evidence-20260713/catalog/`（Git ignored）
- **请求类型**：官方公开、无认证、只读HTTP GET

本manifest一旦发生第一条`courses.json`请求即不可原地扩大或改写。失败重试、添加term/campus、增加optional query或提升上限都必须先停止并回Review；不得用“补样本”绕过预算。

## 2. 官方发现证据

发现请求只用于冻结官方当前选择器，不属于下面的Catalog payload预算：

| Evidence | 官方URL | 观察结果 | 响应事实 |
|---|---|---|---|
| DISC-001 | `https://classes.rutgers.edu/soc/` | `currentTermDate={date:2026-03-30, year:2026, term:9, campus:AC}`；页面引用JS版本`2026-04-07` | 200；94,803 UTF-8重编码bytes；SHA-256 `433BF4010812C1502FF89E46745C6A1524F1A886B91783F490411C4B9E076D32`；observed 2026-07-13T02:47:30+08:00 |
| DISC-002 | `https://classes.rutgers.edu/soc/js/soc_utils.js?v=2026-04-07` | 官方`AppConstants.CAMPUSES`列出15个启用值；初次人工转录漏掉`D`，已由`01d/01e` amendment-002纠正；`SemesterUtils`从Fall 2026依次生成已发布term | 200；43,936 bytes；SHA-256 `35CB04BD2D8AB83B1609B5AC0979EF3EA8FDDBA306980BB566E4B341DD02605E`；Last-Modified 2026-04-08 |

官方来源：[Rutgers Schedule of Classes](https://classes.rutgers.edu/soc/)。上述hash用于证明本次发现输入，不表示Rutgers承诺API稳定或为RBCSP提供SLA。

## 3. 冻结term与campus范围

Term：

- 当期：Fall 2026，wire `year=2026&term=9`，内部候选ID `92026`。
- 相邻已发布term：Summer 2026，wire `year=2026&term=7`，内部候选ID `72026`。
- Fall通过原 manifest 的14个值加 amendment-002 的`D`覆盖全部15个当前有效campus；Summer只用三个主campus及其三个Online/Remote聚合值进行跨term结构检查。产品交付范围仍是全部当前有效campus，不把未在Summer复采的off-campus值解释为不支持，也不把证据矩阵扩成两个term×15 campus。

Campus（以DISC-002启用项为准）：

| 类别 | Codes |
|---|---|
| Main | `NB`, `NK`, `CM` |
| Online/Remote | `ONLINE_NB`, `ONLINE_NK`, `ONLINE_CM` |
| Off-campus | `B`, `CC`, `H`, `CU`, `MC`, `L`, `AC`, `J`, `D` |

被官方JS注释掉的`WM`与`RV`不进入本次“当前有效”范围；只能标`NOT_CURRENTLY_EXPOSED`，不能声称永久不存在。

## 4. 精确请求矩阵

Endpoint固定为：

```text
https://classes.rutgers.edu/soc/api/courses.json?year={year}&term={term}&campus={campus}
```

不发送`subject`、`level`或无效参数；历史材料关于这些参数是否被忽略不作为当前事实。一个term/campus payload只下载一次，后续字段/subject/level分析全部复用本地raw。

| IDs | Term | Campuses | Planned GETs |
|---|---|---|---:|
| `CAT-C001`–`CAT-C014` | Fall 2026 (`2026/9`) | `NB,NK,CM,ONLINE_NB,ONLINE_NK,ONLINE_CM,B,CC,H,CU,MC,L,AC,J` | 14 |
| `CAT-C021`（amendment-002） | Fall 2026 (`2026/9`) | `D` | 1 |
| `CAT-C015`–`CAT-C020` | Summer 2026 (`2026/7`) | `NB,NK,CM,ONLINE_NB,ONLINE_NK,ONLINE_CM` | 6 |
| **总计成功 scope** |  |  | **21** |

完整逐项ID和参数由`01a`锁定。

## 5. 请求预算与停止规则

- 并发：严格`1`。
- 任意两次attempt开始之间：至少5秒。
- 每请求timeout：45秒。
- 自动retry上限：`0`；任一attempt无论成功或失败都不得由工具再次请求相同ID。
- 原Catalog HTTP attempt硬上限：20；`01b/01c`登记一个非证据client failure及唯一同scope replacement；`01d/01e`再冻结一个只补Fall campus D的scope correction。因此累计effective硬上限为22，最多产生21份成功payload。Discovery请求不计入。
- 单响应硬上限：100 MiB；超出即停止且不把截断内容当JSON证据。
- Header：`User-Agent: RBCSP-Evidence/1.0`、`Accept: application/json`、`Accept-Encoding: identity`；不发送cookie、authorization或referer。
- 立即停止：403、429、非2xx、content-type不含JSON、JSON parse失败、根部不是array、响应超限、ledger/raw不一致。
- 5xx/timeout/network同样记录后停止；本manifest不授权自动重试。

## 6. 每次保存的事实

每个attempt写入`02-catalog-request-ledger.tsv`并保存：

- request ID、term/campus、规范化URL、UTC开始/完成时间；
- HTTP status、RTT、bytes、content type；
- 响应`Date/ETag/Last-Modified/Cache-Control/Age`（存在时）；
- body SHA-256、top-level row count、outcome和error class；
- ignored raw逻辑相对路径。

不得保存`Set-Cookie`、完整header dump或访问token。

## 7. 数据质量检查与使用限制

Catalog profile至少检查：

- grain、top-level course数量、section/meeting/instructor数量；
- course/section/index composite key唯一性与跨term/campus碰撞；
- 字段名称、类型、null/empty/sentinel比例及scope漂移；
- Delivery raw code/description与modality/synchronicity候选映射；
- day/time tokens、TBA/invalid、optional/exam/session dates/requiredness；
- structured instructor、location、exam、permission、eligibility、core、prerequisite、credits与FTS来源；
- 22行P2筛选contract的数据可达性。

本样本可以证明“在这些scope和时间观察到什么”，不能证明长期稳定、全历史term一致、API SLA或持续刷新容量。所有结论必须使用`OBSERVED_ONCE / OBSERVED_REPEATED / OBSERVED_MULTI_SCOPE / INFERRED / NOT_OBSERVED`。
