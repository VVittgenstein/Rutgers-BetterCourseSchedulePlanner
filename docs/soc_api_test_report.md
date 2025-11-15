# Rutgers SOC API 调用实验报告

> 2025-11-15（UTC）在 `classes.rutgers.edu` 生产端点手动验证；脚本通过 `requests` 库顺序执行，UA=`BetterCourseSchedulePlanner/0.1`，所有请求均启用 `Accept-Encoding: gzip`。

## 1. 调用矩阵
### 1.1 常见组合（term+campus 主路径）
| ID | Endpoint | Query | 状态/耗时 | 响应体 | 备注 |
| --- | --- | --- | --- | --- | --- |
| A1 | `courses.json` | `year=2024&term=9&campus=NB` | 200 / 2.65s | ~21.0 MB（解压）/ 4,367 课程 | 单校区全量；`Cache-Control: max-age=900`，`ETag: 1285791231` |
| A2 | `courses.json` | `year=2024&term=9&campus=NB&subject=198` | 200 / 0.46s | ~21.0 MB / 4,367 课程 | 服务器忽略 `subject`，payload 与 A1 完全一致 |
| A3 | `courses.json` | `year=2024&term=9&campus=NB&level=G` | 200 / 1.78s | ~21.0 MB / 4,367 课程 | `level` 被忽略；可在本地过滤 |
| A4 | `courses.json` | `year=2024&term=9&campus=NB&keyword=data` | 200 / 0.73s | ~21.0 MB / 4,367 课程 | `keyword` 同样被忽略 |
| A5 | `openSections.json` | `year=2024&term=9&campus=NB` | 200 / 1.72s | ~70 KB / 8,614 index | 推荐用作轮询源；返回字符串数组 |
| A6 | `courses.json` | `year=2025&term=1&campus=NK` | 200 / 1.21s | ~4.67 MB / 1,290 课程 | Newark Spring 基线 |
| A7 | `courses.json` | `year=2025&term=1&campus=NK&subject=910` | 200 / 0.48s | ~4.67 MB / 1,290 课程 | `subject` 同样无效 |

### 1.2 异常组合（错误码与空集）
| ID | Endpoint | Query | 状态/耗时 | 响应体 | 备注 |
| --- | --- | --- | --- | --- | --- |
| B1 | `openSections.json` | `year=2024&term=9&campus=ZZ` | 200 / 0.50s | `[]` | 非法 campus 返回空数组、不报错 |
| B2 | `courses.json` | `year=2024&term=5&campus=NB` | 200 / 0.14s | `[]` | term=5（不存在学期）返回空数组 |
| B3 | `courses.json` | `year=2024&term=9`（缺 campus） | 400 / 0.16s | HTML 错误页（Tomcat） | 缺失必填参数时直接 400，无 JSON |
| B4 | `courses.json` | `year=2025&campus=NK`（缺 term） | 400 / 0.78s | HTML 错误页（Tomcat） | 同上 |

**结论**：当前 SOC API 仅识别 `year`/`term`/`campus` 三个参数。`subject`、`level`、`keyword` 等均忽略，需在本地做过滤；非法组合不会报错，只返回空数组并仍会消耗一次完整请求。

## 2. 速率限制实验
以 `openSections.json?year=2024&term=9&campus=NB` 为目标，顺序发送 8+20+30 次请求并逐步缩短间隔：

| 回合 | 节奏 | 请求数 | 状态集 | 平均耗时 | 最大耗时 | 观察 |
| --- | --- | --- | --- | --- | --- | --- |
| R1 | 1 req/s | 8 | 200 | 0.64s | 1.38s | `Cache-Control: max-age=900`，未见限流 header |
| R2 | 4 req/s | 20 | 200 | 0.65s | 1.27s | 个别请求耗时 >1s，但未返回错误 |
| R3 | 10 req/s | 30 | 200 | 0.50s | 1.73s | 仍全为 200，无 `Retry-After` 或验证码 |

**建议**：
- `openSections` 轮询可安全地控制在 <=5 req/s（单进程），若需要更高频率建议在 429/非 200 时退避 30-60 s 并切换备用校区。
- `courses.json` payload 大（单校区 ~21 MB 解压）；即便未触发限流，也应将抓取频率限制在单 term+campus 15 分钟一次（遵守响应中的 `Cache-Control: max-age=900`）。

## 3. 成功/失败响应样例
### 3.1 成功：`courses.json?year=2024&term=9&campus=NB`
```bash
curl --compressed -sSL -D - 'https://sis.rutgers.edu/soc/api/courses.json?year=2024&term=9&campus=NB'
```
```
HTTP/1.1 200 OK
Server: Apache/2.4.56 (Unix) mod_jk/1.2.48
Content-Encoding: gzip
Cache-Control: max-age=900
ETag: 1285791231
Content-Type: application/json

[{
  "subject": "013",
  "courseNumber": "111",
  "title": "BIBLE IN ARAMAIC",
  "credits": 3,
  "sections": [{
    "index": "05957",
    "openStatusText": "CLOSED",
    "meetingTimes": [{
      "meetingDay": "H",
      "campusName": "COLLEGE AVENUE",
      "buildingCode": "SC",
      "meetingModeDesc": "LEC"
    }, {
      "meetingModeDesc": "ONLINE INSTRUCTION(INTERNET)",
      "campusName": "** INVALID **"
    }]
  }]
} ...]
```

### 3.2 失败：缺失 `term`
```bash
curl --compressed -sSL -D - 'https://sis.rutgers.edu/soc/api/courses.json?year=2025&campus=NK'
```
```
HTTP/1.1 400 Bad Request
Server: Apache/2.4.56 (Unix) mod_jk/1.2.48
Content-Type: text/html;charset=utf-8

<!DOCTYPE html><html>...<h1>HTTP Status 400 - </h1>...
```
> 错误响应为 Tomcat HTML，无法解析为 JSON。监控逻辑需将 `Content-Type` 用于兜底检测。

### 3.3 `openSections` 片段
```bash
curl --compressed 'https://sis.rutgers.edu/soc/api/openSections.json?year=2024&term=9&campus=NB' | jq '.[0:10]'
```
```
[
  "23603",
  "05972",
  "05974",
  "05976",
  "05980",
  "06005",
  "06011",
  "06012",
  "06013",
  "06017"
]
```

## 4. 调用与监控建议
- **全量抓取**：按 term+campus 拆分 `courses.json`，遵循 15 分钟缓存窗口；下载后立即在本地缓存/数据库中做 subject、keyword、level 等筛选，避免重复调用。
- **空位轮询**：使用 `openSections.json` 作为主数据，每 15-30 秒轮询一次指定校区即可覆盖 8,614 index；搭配缓存的课程 JSON 进行 index -> 课程映射。
- **错误处理**：
  - 对 400 错误记录 URL/必填参数缺失，并跳过重试（属于客户端错误）。
  - 对 200+空数组（非法 term/campus）在日志中分类为配置错误。
  - 若未来出现 429/503，可按照指数退避 30 s、60 s、120 s 并切换到其他校区以分散压力。
- **监控字段**：记录 `ETag`、`Cache-Control`、`Content-Length` 与 `X-Server-Name` 便于排查缓存漂移；在客户端暴露 `payload_length` 与解压时长监控消耗。

## 5. TODO / 后续衔接
- 已满足 ST-02 的调用矩阵、速率试验与样例文档需求；可将本文作为 `soc_api_handbook.md` 的输入，固化调用频率与退避策略。
- 若需更深入的限流上限，可在 staging 环境引入并发请求与 If-Modified-Since / If-None-Match 试验，观察缓存行为。
