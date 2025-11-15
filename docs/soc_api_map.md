# Rutgers SOC API 资源与参数映射

> 本文基于 2024 Fall（term=9, year=2024）与 2025 Spring（term=1, year=2025）两个学期、NB 与 NK 两个校区的实测结果整理。所有示例均通过 `https://sis.rutgers.edu/soc/api/*` （会 302 到 `https://classes.rutgers.edu`) 获得。

## 1. 资源列表
| 资源 | 说明 | 基础 URL | 必填参数 | 选填/忽略参数 | 返回格式 | 备注 |
| --- | --- | --- | --- | --- | --- | --- |
| `courses.json` | 按 `year`+`term`+`campus` 返回**整座校区的全部课程+全部 section**，包含课程、容量、授课、教室等字段 | `/soc/api/courses.json` | `year` (YYYY) · `term` (0/1/7/9) · `campus` (见下文) | `subject`/`level`/`keyword`/`school` **实测被忽略** | JSON 数组，单个元素为 course 对象（含 `sections` 列表） | 响应默认 `Content-Encoding:gzip`，NB Fall 2024 压缩包约 0.9 MB，解压 21 MB / 4 367 门课 |
| `openSections.json` | 给定 `year`/`term`/`campus`，返回**当前有空位的 section 索引号列表** | `/soc/api/openSections.json` | `year` · `term` · `campus` | 无 | JSON 数组（字符串形式的 section index，如 `"05972"`） | 与 `courses.json` 同参数，压缩后几十 KB；需与课程 JSON 交叉匹配 |
| `sections.json` | 早期材料中提到的单课 section 端点，现网 `https://classes.rutgers.edu/soc/api/sections.json?...` 返回 404 | —— | —— | —— | —— | 需要从 `courses.json` 的 `sections` 数组获取 section 详情 |
| `initJsonData` | SOC 页面内嵌的初始化元数据，含 `subjects`(520)、`units`、`coreCodes`、`buildings`、`currentTermDate` | `https://classes.rutgers.edu/soc/` HTML 中 `<div id="initJsonData">…</div>` | 无（直接抓页面并解析 JSON） | —— | JSON 对象 | 仅通过页面源码提供；无独立 API 端点 |

## 2. 参数空间
### 2.1 term / year / semester
- `term` 取值固定：`0`=Winter，`1`=Spring，`7`=Summer，`9`=Fall（来自 `AppConstants.TERM_NAMES`）。
- `year` 为 4 位公历年，例如 2025。
- SOC 前端页面使用 `semester=<term><year>`（如 `12025`），下载数据时需拆分为 `term=1&year=2025`。
- 缺失任何一个必填参数会返回 HTTP 400；非法取值（例如 `term=5` 或 `campus=ZZ`）会得到 200 + 空数组。

### 2.2 campus（校区代码）
主校区与线上/外点代码来自 `AppConstants.CAMPUSES`：
| 类型 | 代码 | 含义 |
| --- | --- | --- |
| 主校区 | `NB` | New Brunswick |
|  | `NK` | Newark |
|  | `CM` | Camden |
| 线上衍生 | `ONLINE_NB` / `ONLINE_NK` / `ONLINE_CM` | 对应校区的 Online & Remote 课 |
| Off-campus 节点 | `B`, `CC`, `H`, `CU`, `MC`, `WM`, `L`, `AC`, `J`, `D`, `RV` 等 | 面向 BCC、RVCC、Mays Landing、Joint Base 等合作中心 |

> ⚠️ API 区分大小写：`campus=nb` 返回空结果。UI 会在多选后拆分成多个请求，每个请求只携带一个 campus 代码。

### 2.3 level
- UI 允许 `U`/`G`/`U,G` 组合并在前端过滤，API 响应仍包含全部层级。
- `courses.json?...&level=G` 与无 `level` 时结果完全一致（4 367 门课，`level` 字段同时出现 `U`/`G`）。

### 2.4 subject / keyword / school / index
- `subject` 为 3 位字符串（`013`、`198` 等），官方例子建议传 `subject=198`，**实测服务器忽略**。
- `keyword`、`school`、`courseNumber`、`index` 等 URL 参数只在 Hash 片段里使用，数据仍一次性下发，由浏览器过滤。
- 需要 subject 列表时，可解析 `initJsonData.subjects`，结构示例：`{"code":"013","description":"BIBLICAL STUDIES","school":"01","schoolDescription":"School of Arts & Sciences"}`。

### 2.5 open section 判定
- `openSections.json` 返回所有有空余座位的 section index 字符串。
- `courses.json` 的 `sections[*].openStatus` / `openStatusText` 也反映开放状态，但该字段在下载瞬间即可与 `openSections` 对齐验证。

## 3. 字段速查
### 3.1 课程级字段（`courses.json` 元素）
| 字段 | 类型 | 必然存在 | 说明 |
| --- | --- | --- | --- |
| `subject`, `courseNumber`, `courseString` | string | 是 | `courseString` 形如 `01:013:111`，适合生成唯一键 |
| `title`, `expandedTitle`, `courseDescription` | string | `expandedTitle` 可能为空 | 官方简称、长标题和描述 |
| `credits`, `creditsObject` | number / object | 是 | `creditsObject={"code":"3_0","description":"3.0 credits"}`，部分课程为 `BA`（By Arrangement） |
| `level` | string | 是 | `U`/`G`/`UG` 等；API 不会过滤 |
| `offeringUnitCode` / `offeringUnitTitle` | string | 是 | 对应 `initJsonData.units` 中的学院/School |
| `campusCode`, `mainCampus`, `campusLocations` | string / array | 是 | `campusLocations` 为子校区数组（参照 `AppConstants.SUBCAMPUSES`），如 `{code:"1",description:"College Avenue"}` |
| `supplementCode` | string | 经常为空 | 常见于跨校区/College tag（例如 `NB`, `CM`） |
| `coreCodes` | array | 可能为空 | Rutgers Core / GenEd 代码对象：`{"code":"AH","description":"Arts and Humanities"}` |
| `preReqNotes`, `subjectNotes`, `courseNotes`, `unitNotes` | string | 可为 `null` | 备注（先修/学院要求等），包含 `*PREREQ`、`*COREQ` 标记 |
| `synopsisUrl` | string | 可能为空 | 指向课程大纲页面 |
| `openSections` | integer | 是 | 课程下开放 section 数量；与 `sections[*].openStatus`/`openSections.json` 对应 |
| `sections` | array<section> | 是 | Section 级详细信息，数量从 1 到几十 |

### 3.2 section 级字段（`course.sections[*]`）
| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `index` | string | WebReg/SOC 使用的 5 位索引号，唯一定位 section，亦是 `openSections.json` 中的元素 |
| `number` | string | Section 编号（如 `01`、`H1`） |
| `openStatus` / `openStatusText` | bool / string | 布尔值 + `OPEN`/`CLOSED`，与 `openSections.json` 同步 |
| `sectionCourseType` | string | 课程类型（Lecture/Lab/Recitation 等，值为空或 `LEC`/`LAB` etc） |
| `sessionDates`, `sessionDatePrintIndicator` | string | `sessionDates` 多为 `null`，`sessionDatePrintIndicator` 常见 `N` |
| `instructors`, `instructorsText` | array / string | `instructors` 为 `[{"name":"DOE, J"}, ...]`，`instructorsText` 为逗号连接字符串 |
| `meetingTimes` | array | 详见下节；若为 async 课程，可空数组并附 `sectionNotes` |
| `majors`, `minors`, `unitMajors` | array<string> | 限制专业/学院；为空表示开放 |
| `sectionEligibility`, `specialPermissionAddCode`/`DropCode`(+Description) | string | 是否需要特殊许可（`C`=college, `D`=department 等） |
| `courseFee` / `courseFeeDescr` | string | 额外费用信息 |
| `crossListedSections`, `crossListedSectionsText`, `crossListedSectionType` | array / string | 交叉列表的索引与类型 |
| `subtitle`, `subtopic`, `sectionNotes`, `comments`, `commentsText` | string | 备注文本（含 honors、同步/异步说明等） |
| `examCode`, `examCodeText` | string | 期末考试安排代码 |
| `legendKey`, `printed` | string/bool | UI 展示标识 |
| `sectionCampusLocations` | array | 与 `subject`级 `campusLocations` 相似，细化至具体校区 |

### 3.3 meetingTime 结构（`section.meetingTimes[*]`）
| 字段 | 说明 |
| --- | --- |
| `meetingDay` | `M/T/W/TH/F/S/U`，与 `pmCode` 组合判定时间段 |
| `startTime`/`endTime` (hh:mmp) + `startTimeMilitary`/`endTimeMilitary` | 字符串，方便可读和排序 |
| `meetingModeCode`/`meetingModeDesc` | 02=Lecture, 21=Hybrid, 80=Asynchronous 等；共 20+ 枚值 |
| `campusAbbrev`, `campusName`, `campusLocation` | 具体校区/楼宇（如 `CAC`, `College Avenue`） |
| `buildingCode`, `roomNumber` | 课堂位置，远程课为空；`baClassHours` 表示 by-arrangement 时数 |

### 3.4 `openSections.json` 结构
- JSON 数组，例如 `["23603","05972",...]`。
- 与 `courses.json` 中的 `sections[*].index` 直接对应。
- 数据量远小于课程列表（NB Fall 2024 仅 8 600 条索引，压缩包 ~21 KB），适合高频轮询。

## 4. 示例请求与响应
所有请求均需 `--compressed`（或手动 gunzip）。

### 4.1 2024 Fall · New Brunswick (`term=9`, `year=2024`, `campus=NB`)
```bash
curl --compressed 'https://sis.rutgers.edu/soc/api/courses.json?year=2024&term=9&campus=NB' \
  | jq '.[0] | {subject, courseNumber, title, credits, openSections, sections: [.sections[0] | {index, number, openStatusText, instructorsText, meetingTimes}]}'
```
输出摘要（首门课 `BIBLE IN ARAMAIC`）：
```json
{
  "subject": "013",
  "courseNumber": "111",
  "title": "BIBLE IN ARAMAIC",
  "credits": 1,
  "openSections": 0,
  "sections": [
    {
      "index": "05957",
      "number": "01",
      "openStatusText": "CLOSED",
      "instructorsText": "HABERL, CHARLES",
      "meetingTimes": [
        {
          "meetingDay": "H",
          "startTimeMilitary": "1400",
          "endTimeMilitary": "1520",
          "meetingModeDesc": "LEC",
          "campusName": "COLLEGE AVENUE",
          "buildingCode": "SC",
          "roomNumber": "120"
        },
        {
          "meetingModeDesc": "ONLINE INSTRUCTION(INTERNET)",
          "meetingModeCode": "90",
          "campusName": "** INVALID **",
          "baClassHours": "B"
        }
      ]
    }
  ]
}
```
`openSections.json` 请求：
```bash
curl --compressed 'https://sis.rutgers.edu/soc/api/openSections.json?year=2024&term=9&campus=NB' | jq '.[0:10]'
```
示例结果 `[
  "23603","05972","05974","05976","05980","06005","06011","06012","06013","06017"
]`。

### 4.2 2025 Spring · Newark (`term=1`, `year=2025`, `campus=NK`)
```bash
curl --compressed 'https://sis.rutgers.edu/soc/api/courses.json?year=2025&term=1&campus=NK' \
  | jq '.[0] | {subject, courseNumber, title, campusCode, openSections, sections: [.sections[0] | {index, openStatusText, meetingTimes}]}'
```
示例输出（首门课 `SOC WELF POL & SRV`）：
```json
{
  "subject": "910",
  "courseNumber": "504",
  "title": "SOC WELF POL & SRV",
  "campusCode": "NK",
  "openSections": 1,
  "sections": [
    {
      "index": "09058",
      "openStatusText": "OPEN",
      "meetingTimes": [
        {
          "meetingDay": "M",
          "startTimeMilitary": "1800",
          "endTimeMilitary": "2040",
          "meetingModeDesc": "LEC",
          "campusName": "NEWARK",
          "buildingCode": "HIL",
          "roomNumber": "106"
        }
      ]
    }
  ]
}
```
对应 `openSections.json` 的前 10 条：`["09058","05358","05359","09065","05360","05361","05362","05363","05364","05365"]`。

## 5. 官方示例 vs 实测行为
| 项目 | 官方/历史示例 | 实测情况 | 结论 |
| --- | --- | --- | --- |
| URL 参数 | `courses.json?subject=198&semester=12018&campus=NB&level=UG`（Rutgers Course API README） | 服务器仅识别 `year`,`term`,`campus`；其余被忽略。`semester` 参数不可用，需拆分。 | 工具需自行拆 `semester` 并在本地按 subject/level 过滤 |
| `sections.json` | README 中提及 `sections` 端点 | 访问 `/soc/api/sections.json?...` 得 404 | 不存在单独 section 端点；解析 `courses.json` |
| 级别过滤 | README 建议传 `level=UG` | `level` 参数无效，响应仍含 `U` 与 `G` | 级别过滤在本地完成 |
| keyword / subject 过滤 | UI 支持关键字/多 subject 查询 | 请求中追加 `subject` 或 `keyword` 无效果 | 需客户端过滤，API 只能全量拉回 |
| Rate limit | 无官方说明 | 本次以 1 req/s 连续抓取 NB/NK/CM 共 6 个请求，未触发限流或验证码 | 后续测速率/退避策略另行完成 |

## 6. 建议调用策略
1. **term+campus 全量拆分**：对每个学期按校区（NB/NK/CM/ONLINE_* 等）分别请求一次 `courses.json` 与一次 `openSections.json`，并缓存解压后的 JSON（单校区 ≈ 20 MB）。
2. **本地维度表**：抓取一次 `https://classes.rutgers.edu/soc/` 并解析 `initJsonData`，得到 subjects/core codes/units/buildings；随前端更新手动刷新。
3. **过滤逻辑前移**：把 subject/keyword/level/campusLocation 等筛选逻辑放入数据库或前端，以免重复网络请求。
4. **开放席位检测**：以 `openSections.json` 为轮询主数据，只有发现目标 index 出现在列表时才回查 `courses.json` 缓存，避免频繁下载大文件。
5. **错误处理**：对 HTTP 400（缺失参数）和 200+空数组（非法 term/campus）分别记录日志，便于监控每日学期代码变更。
