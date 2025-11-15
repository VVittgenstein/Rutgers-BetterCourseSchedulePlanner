# Rutgers SOC API 调用手册

> 基于 `docs/soc_api_map.md`（参数/字段）与 `docs/soc_api_test_report.md`（限流实测）整理。更新时间：2025-11-15 UTC。

## 1. 端点与 URL 模板

| 资源 | 推荐基础 URL | 必填参数 | 模板示例 | 说明 |
| --- | --- | --- | --- | --- |
| 课程全量 | `https://sis.rutgers.edu/soc/api/courses.json` | `year`（YYYY）· `term`（0/1/7/9）· `campus`（NB/NK/CM/ONLINE_* 等） | ``/courses.json?year={year}&term={term}&campus={campus}`` | 返回指定校区**所有课程+sections**，无分页。payload 单校区≈0.9 MB gzip / 21 MB 解压（NB Fall）。 |
| 空位索引 | `https://sis.rutgers.edu/soc/api/openSections.json` | 同上 | ``/openSections.json?year={year}&term={term}&campus={campus}`` | 返回开放 section 的 index 数组（字符串）。适合高频轮询。 |
| 维度元数据 | `https://classes.rutgers.edu/soc/`（HTML） | 无 | 抓 `<div id="initJsonData">…</div>` | 提供 subjects、units、buildings、coreCodes 等维度。需解析 HTML 中嵌入 JSON。 |

### 1.1 推荐示例（cURL）
```bash
# NB 2024 Fall 全量课程
curl --compressed \
  'https://sis.rutgers.edu/soc/api/courses.json?year=2024&term=9&campus=NB' \
  -H 'Accept: application/json' \
  -H 'User-Agent: BetterCourseSchedulePlanner/0.1' \
  -o cache/2024-9-NB-courses.json.gz

# Newark Spring 开放席位轮询
curl --compressed \
  'https://sis.rutgers.edu/soc/api/openSections.json?year=2025&term=1&campus=NK' \
  | jq '.[0:10]'
```

> ⚠️ `subject`、`level`、`keyword`、`school` 等 URL 参数会被服务器忽略，所有筛选需在本地完成。缺失任何一个必填参数将得到 400 + HTML 错误页。

## 2. 批量/分页策略
1. **按 term × campus 分片**：SOC 端点不支持分页；建议将每个学期拆成校园列表（主校区 NB/NK/CM + ONLINE_* + off-campus 代码），依次下载 `courses.json` 与 `openSections.json`。若需 subject 粒度数据，在本地（SQLite/Parquet）按 `subject` 字段过滤。
2. **缓存与增量**：利用响应头 `Cache-Control: max-age=900` 与 `ETag`。全量抓取后缓存 gzip 文件与解析结果，增量更新时读取缓存 + 合并 open sections。
3. **initJsonData 更新**：每日或版本更新前抓取一次 `https://classes.rutgers.edu/soc/` 并提取 `initJsonData`，生产 `subjects`, `units`, `coreCodes`, `buildings` 等维度表；与课程数据分离存储。
4. **跨学期同步**：脚本入参接受 `--terms "2024:9,2025:1"`，每个组合循环 campus 列表；通过 `max_workers=1`（单线程）或 campus 级并行（<=3）控制并发。

## 3. 安全节奏与退避

| 场景 | 推荐频率 | 并发建议 | 退避策略 | 说明 |
| --- | --- | --- | --- | --- |
| term × campus 全量 `courses.json` 抓取 | ≤1 次 / 15 分钟（遵守 `max-age=900`） | 顺序或单线程 | 若 ≥2s / >50 MB 或非 200，延迟 5 分钟并重试；连续失败 3 次则告警 | 单请求 21 MB 解压（NB），磁盘缓存后供所有筛选使用。 |
| `openSections.json` 空位轮询 | 15–30 秒一次（单校区） | ≤5 req/s（同校区）；跨校区并行总量 ≤10 req/s | 出现非 200/网络异常：按 30s、60s、120s 指数退避并记录 | R2/R3 实验（10 req/s）未触发限流，但建议留裕度。 |
| `initJsonData` 元数据刷新 | 1 次 / 日 或手动触发 | 单线程 | 失败时 5 分钟重试，3 次失败提醒 | 页面约 700 KB，解析成本低。 |

- **请求 headers**：统一 UA、`Accept-Encoding: gzip`；记录 `ETag` 与 `Content-Length` 方便缓存命中判断。
- **解压/解析指标**：对 `courses.json` 记录 `payload_size_bytes`、`parse_duration_ms` 方便监控 CPU/IO。

## 4. 常见错误 & 规避原则
| 问题 | 触发方式 | 影响 | 规避 & 处理 |
| --- | --- | --- | --- |
| term/campus 代码错误 | 过期学期（term=5）、拼写错误（campus=nb） | 返回 200 + `[]`，导致误判为暂无课程 | 在配置层枚举合法 term/campus；收到 200 且 `Content-Length<40` 时抛出配置错误。 |
| 缺少必填参数 | 漏掉 `term` 或 `campus` | HTTP 400 + HTML，解析失败 | 发送前校验必填参数；对 `Content-Type!=application/json` 的响应直接标记为客户端错误并停止重试。 |
| 过度并发 | 大于 10 req/s 持续命中 | 当前未触发限流，但存在未来封禁风险 | 统一调度器限制全局 RPS；实现指数退避并允许切换 campus 序列。 |
| subject/level 过滤误解 | 期望 API 端过滤 | 实际仍返回全量，浪费带宽 | 永远全量抓取后在数据库中过滤；脚本通过日志警告任何附加参数。 |
| openSections 不匹配 | 缓存课程 JSON 过期 | index 无法映射课程 | 轮询到新 index 时先检查 `courses_cache_age`，超过 15 min 则刷新对应 term+campus。 |

## 5. 必需日志字段与监控指标
- **请求维度**：`term`, `year`, `campus`, `endpoint`, `request_id`, `attempt`.
- **响应维度**：`status_code`, `content_type`, `content_length`, `etag`, `cache_control`, `payload_size_bytes`（解压后）.
- **性能指标**：`connect_duration_ms`, `transfer_duration_ms`, `inflate_duration_ms`, `parse_duration_ms`.
- **业务指标**：`courses_count`, `sections_count`, `open_section_count`.
- **错误分类**：`client_config_error`（200+空数组）、`missing_required_param`（400）、`transport_error`、`rate_limit`（若未来出现 429/503）。

所有日志需上传到 observability 管道（CloudWatch / ELK），并提供以下报警：
1. 15 分钟内同一 `term+campus` 全量抓取失败 ≥3 次。
2. openSections 轮询 5 分钟内连续失败 ≥10 次。
3. 解压或解析耗时超过基线（>5s）且持续 3 次，提示磁盘/CPU 异常。

---

**落地建议**
1. 以 `scripts/fetch_soc.py`（示例）承载调用：输入 term/campus 队列 → 顺序执行 → 写入 `data/raw/{term}-{campus}-{endpoint}.json.gz`。
2. 将本文作为抓取与通知轮询脚本的唯一约束来源；修改策略（频率/参数）需 PR 更新此文档并在记录中关联验收证据。
