# P3 Catalog Request Amendment 002 — Current Campus D

## Status

- `FROZEN_SCOPE_CORRECTION`
- Frozen at: `2026-07-13T10:55:57+08:00`
- Base manifest SHA-256: `6F6A71F5F8E2D492882FD364DB1B8510A46E2895AA484F0EDA9A19E37D353D90`
- Prior amendment SHA-256: `95E1D2C1EE875EBB45907E00C1B6AF21A4E1C21AE2EE7F1D1A698E2285479FA6`

## Reason

P3 初次 discovery 对当前官方 `AppConstants.CAMPUSES` 发生人工转录遗漏：当前 `soc_utils.js?v=2026-04-07` 第 53 行还启用了：

```text
{ code:"D", name:"Mercer County Community College", type:3 }
```

该官方脚本的 SHA-256 仍是 `35CB04BD2D8AB83B1609B5AC0979EF3EA8FDDBA306980BB566E4B341DD02605E`，因此这不是 Rutgers 在两次观察间改变 scope，而是 P3 的 scope transcription defect。

## Exact correction

只增加一次请求：

```text
CAT-C021  courses.json?year=2026&term=9&campus=D
```

不把 Summer 2026 扩成 15 campuses。P3 的证据设计仍是：

- Fall 2026：当前官方 selector 的全部 15 campuses；
- Summer 2026：用于相邻 term 结构比较的 6 个 main/online campuses。

这会把累计 network attempts 硬上限从 21 增至 22：原始 20 + gzip 客户端失败后的 1 次冻结 replacement + 本次 1 次 scope correction。automatic retry 仍为 0，串行间隔与全部 stop conditions 不变。

本 amendment 必须先冻结，才能执行 `CAT-C021`。不得借此增加其他 term/campus 请求。
