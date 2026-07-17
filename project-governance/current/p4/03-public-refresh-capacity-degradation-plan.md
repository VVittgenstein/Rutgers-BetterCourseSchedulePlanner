# P4 公网Refresh、Capacity与Degradation计划

## 1. 冻结结论

公网只运行一套集中Catalog/Open poller。浏览器load、reload、query、tab、selected section、WS连接和用户数量不直接创建Rutgers请求；active watch只把其所属共享`OpenBatchKey=(term,campus)`的目标cadence从30秒提升到10秒。

| 时钟/边界 | 公网值 |
|---|---:|
| Catalog target interval | 固定`600s` |
| Open general interval | 固定`30s` |
| 有active watch的Open target | 目标`10s` |
| Open effective公式 | watched=`min(30,10)=10s`；否则`30s` |
| 真实Rutgers origin并发 | `1`，Catalog/Open共享 |
| 同target并发 | single-flight |
| 排序 | earliest absolute due first；仅同deadline时watch优先 |
| missed tick | coalesce/skip；不catch-up burst |
| 普通用户配置 | 无 |

10/30/600秒都是scheduler requested cadence，不是Rutgers发布SLA或绝对完成周期。系统必须显示实际start-to-start interval、scheduler lag、last attempt/valid/change、LKG age与circuit，不能把requested值冒充achieved值。

## 2. Target与数据流

```text
published term/campus service scope
          |
          +-> CatalogTarget(term,campus) -- 600s --+
          |                                      |
          +-> OpenBatch(term,campus) -- 30/10s --+--> shared EDF origin limiter (1)
                                                     |
                                                     v
                                              Rutgers official origin
                                                     |
                                                     v
                                     validate -> reconcile -> SQLite/LKG
                                                     |
                                                     v
                                            WebSocket fanout/audio
```

- Catalog与Open是两种独立上游刷新，但共享唯一真实origin limiter。
- `OpenBatchKey=(term,campus)`。当前每batch恰好一个匹配key的官方URI；未来有多required arrays时也只能在同batch内全部验证后union/deduplicate。
- 不同campus从不为状态变更动态union；一个scheduler cycle允许安全成功的batch分别原子提交。
- Section外部key始终为`(term,campus,index)`；orphan仅审计/忽略，绝不创建Section。
- 每个Open attempt捕获Catalog `content_version`。提交时版本漂移为`STALE_CATALOG_RACE`，不更新LKG、不生成Closed，只按正常due coalesce。

## 3. Central scheduler

### 3.1 Due与single-flight

每个target保存：

- `requested_interval_sec`、`effective_interval_sec`；
- `last_scheduled_due`、`next_absolute_due`、`actual_started_at/completed_at`；
- `scheduler_lag_ms=max(0, actual_started_at-next_absolute_due)`；
- in-flight flag、failure streak、circuit deadline；
- last attempt、last valid、last body/state change与LKG age。

Timer、服务启动恢复、watch lane promotion和内部maintenance wakeup都进入同一个per-target single-flight状态机。多个事件同时到达只保留一个最早合法due；执行中到来的tick只coalesce为后续正常due，不在完成后连发补齐。

### 3.2 EDF与无饥饿

所有Catalog/Open工作按绝对due升序取出；active-watch只在due完全相同时作为tie-break。一次请求完成后，该target的next due按合同推进，不把“当前仍逾期”解释为立即无限重入。这样普通Open与Catalog即使在watch压力下也有有界进展机会。

P7必须用fake clock验证：

- Catalog/Open混合队列中每一lane最终获得执行；
- watch promote/demote不产生双飞或catch-up burst；
- 同target连接/watch/section数变化不改变attempt数；
- overdue target以真实lag排序并显示，不被静默丢弃；
- 系统时间跳变、America/New_York DST与进程恢复不产生负lag或请求风暴。

提高真实origin并发、采用每浏览器poller、按section poll或用cache-busting规避上游缓存都不在本合同内，必须回Review。

## 4. Open response分类与LKG

| 输入 | 分类 | 状态动作 |
|---|---|---|
| 2xx合法非空五位字符串set，有安全intersection | `VALID_APPLIED` | 原子commit target observation、Section state与LKG |
| 2xx空数组且该Catalog target也空 | `VALID_EMPTY_NO_ROWS` | commit observation，无Section state row |
| 2xx空数组且Catalog target非空 | `UNSAFE_EMPTY` | 保留LKG；绝不mass-close |
| 非空合法set、非空Catalog但intersection为0 | `UNSAFE_ZERO_INTERSECTION` | 保留LKG；绝不two-miss/mass-close |
| HTTP/transport/schema/size/off-origin失败 | `FAILED`或fatal分类 | 保留LKG；不得用absence转Closed |
| Catalog version drift | `STALE_CATALOG_RACE` | 不更新LKG/Closed；新Section保持UNKNOWN |
| 尚无成功观察 | `UNKNOWN` | Open筛选结果为UNCERTAIN |

每个实际请求开始创建`OpenPullAttempt`。每个valid applied response，即使body不变，也创建target `OpenRefreshObservation`；随后给该batch每个watched Section创建section `OpenObservation`。ETag只作审计，不判定body/state变化，不触发episode或声音。

Freshness沿用P3：

```text
fresh_until = last_valid_completed + 2 * requested_effective_interval + 15s
```

任何之后的FAILED、UNSAFE或STALE_CATALOG_RACE使LKG立即stale。只有fresh known OPEN/CLOSED给确定筛选结果；stale/UNKNOWN为UNCERTAIN，同时展示last-known、age与原因。

## 5. Transport、retry与origin circuit

- HTTPS GET至allowlisted官方origin；不带cookie/auth/cache-busting；拒绝off-origin redirect。
- Connect timeout `5s`；total timeout `15s`；decoded body上限`10 MiB`。
- 每attempt立即自动重试`0`次。
- network/408/5xx、`UNSAFE_EMPTY`、`UNSAFE_ZERO_INTERSECTION`退避step：`30,60,120,240,480,600s`。
- `retry_delay=max(requested_effective_interval, backoff_step)+deterministic_jitter`，jitter固定在所选delay的`0–10%`；失败不能让target比请求cadence更快。
- 429优先遵守合法`Retry-After`，否则打开origin-wide `15m` circuit。
- 403、off-origin redirect、schema/value/size fatal打开origin-wide fail-closed circuit；至少冷却`60s`后只允许显式诊断recheck，不自动循环。
- 任一origin-wide circuit同时阻断Catalog/Open；用户、watch、reload或管理员页面不能绕过。
- 普通full GET；P3未验证conditional GET/304，公网不得自行启用。`Cache-Control: max-age=30`与ETag是观测证据，不是BCSP跳过安全验证的许可。

## 6. Capacity模型

令：

- `N_c`为进入服务scope的Catalog targets数；
- `N_10`为至少有一个active watch的distinct Open batches数；
- `N_30`为其余Open batches数；
- `S`为单个串行上游request的实际服务时间。

则请求需求为：

```text
Q_requested = N_c/600 + N_10/10 + N_30/30
Q_serial_capacity ~= 1/S
```

P3两轮42次串行小样本观测p95约`1.501s`，只给出约`0.666 QPS`的审计参考，**不是**长期Rutgers容量SLA。以P3列举的当前Fall 15 targets作静态例子：

| 场景 | Open需求 | Catalog需求 | 总请求需求 | 与0.666审计参考关系 |
|---|---:|---:|---:|---|
| 15个general targets | `0.500 QPS` | `0.025 QPS` | `0.525 QPS` | 名义低于，但无SLA |
| 9个distinct watched + 6 general | `1.100 QPS` | `0.025 QPS` | `1.125 QPS` | 高于，预期lag |
| 15个全部watched | `1.500 QPS` | `0.025 QPS` | `1.525 QPS` | 高于，预期明显lag |

若9个watched sections位于同一term/campus，它们只提升1个batch；若分属9个batch，则提升9个。连接或用户数不进入QPS公式。

这个模型证明：active target 10秒是优先调度目标，不是所有负载下都能达到的墙钟保证。P4不通过并发放大、丢弃target或浏览器直连来掩盖饱和。

## 7. Degradation状态机

公网按事实暴露以下正交状态：

1. `SCHEDULER_LAGGING`：target在due后才开始；显示exact lag与actual interval。
2. `FRESH`：存在valid observation且未超freshness窗口、之后无unsafe/failure/race。
3. `STALE_LKG`：仍显示last-known但筛选为UNCERTAIN；显示age和stale原因。
4. `UNKNOWN`：无LKG；不得冒充Closed。
5. `BACKING_OFF`：显示next eligible time、failure category，不提供绕过按钮。
6. `CIRCUIT_OPEN`：显示origin级原因与安全恢复条件。
7. `SERVICE_UNAVAILABLE`：operational DB或scheduler不安全，readiness失败。

降级顺序：

- 保持concurrency=1与single-flight；
- EDF继续处理最早due，watch只同deadline优先；
- missed ticks coalesce，不补发；
- 保留LKG并诚实标stale/lag；
- 继续target-request counters与故障审计；
- 绝不自动缩减campus、mass-close、伪造30/10秒达成或跳过安全validation。

持续饱和时，P7 release evidence必须报告可支持target/active-batch envelope；若实现只能靠提高真实origin并发或改变P3 cadence才能通过，停止Review。Fake-upstream可做负载/失败测试；不得向Rutgers压力测试。

## 8. Service-wide counters与retention

计数grain始终是upstream target request，不是Section、浏览器、WebSocket或fanout：

- `attempted`：实际request start；
- `succeeded`：valid applied response；
- `failed`：transport/HTTP/validation/unsafe application failure；
- `empty`：任何2xx空数组，作为与success/failure正交的维度。

公网展示当前`America/New_York` Rutgers自然日的service-wide attempted/succeeded/failed/empty。明细按P3每target保留“当前Rutgers日或最近256条，取覆盖更多者”；更旧明细滚入daily aggregate，不保存raw body。

进程重启从SQLite明细/aggregate恢复当天计数并继续；不能归零或重复计数。它们只按target和时间聚合，不记录session、IP、选中section、watch、audio或notification history。跨日按Rutgers timezone切换，UI明确标注时区。

## 9. Fanout与通知延迟边界

P3延迟模型保持：

```text
D_true = U + C + P + B + F
```

Rutgers发布延迟`U`与上游cache/representation延迟`C`不受BCSP控制；scheduler/queue `P`、batch `B`与fanout `F`可观测。公网只冻结：valid OpenObservation完成后到server WebSocket fanout的工程目标`<=1s`。

这不是硬SLA，也不意味着真实seat变化后30秒内通知。Browser audio只有页面仍连接、前台能力正常且AudioContext已由手势unlock时才进入目标；autoplay/suspend失败必须明确显示。UI文案使用“BCSP首次观察到Open”及真实时间戳。

## 10. P7 fake-upstream与runtime验收

| ID | 必测内容 |
|---|---|
| P4-REFRESH-001 | Catalog 600/Open 30/watched 10固定配置与普通用户不可修改 |
| P4-REFRESH-002 | per-target single-flight、reload/query/watch同target不放大 |
| P4-REFRESH-003 | Catalog/Open共享concurrency=1，EDF无饥饿、无catch-up |
| P4-REFRESH-004 | 15-target general/9-watch/all-watch模型下actual interval与lag真实显示 |
| P4-REFRESH-005 | valid/empty/unsafe-zero/failure/catalog-race/LKG/UNKNOWN原子状态机 |
| P4-REFRESH-006 | backoff、429 Retry-After、origin circuit与恢复 |
| P4-REFRESH-007 | America/New_York跨日/DST、restart counter恢复与retention rollup |
| P4-REFRESH-008 | body/state hash、ETag audit-only、每valid observation基数 |
| P4-REFRESH-009 | observation→server fanout目标测量；browser audio blocked可见 |
| P4-REFRESH-010 | 公网浏览器network trace只有BCSP HTTPS/WSS，无Rutgers请求 |

对应P7任务：`P7-SHARED-CATALOG/OPEN/WATCH`实现共享语义；`P7-PUBLIC-RUNTIME`固定配置与服务状态；`P7-PUBLIC-SESSION`验证页面事件不放大；`P7-PUBLIC-OPS`验证监控/重启；`P7-PUBLIC-PACKAGE`保存测试证据。

## 11. Machine-readable state

```text
status=P4_PUBLIC_REFRESH_CONTRACT_FROZEN
catalog_interval_sec=600
open_general_interval_sec=30
open_active_target_sec=10
open_batch_key=term_campus
per_target_single_flight=TRUE
shared_catalog_open_origin_limiter=TRUE
real_origin_concurrency=1
scheduler=EDF_NO_STARVATION
missed_ticks=COALESCE_OR_SKIP_NO_CATCH_UP
user_or_watch_count_amplifies_requests=FALSE
browser_events_trigger_upstream_requests=FALSE
counter_scope=SERVICE_WIDE
counter_timezone=AMERICA_NEW_YORK
counters_survive_service_restart=TRUE
valid_observation_to_server_fanout_target_sec=1
hard_fanout_sla=FALSE
hard_real_change_to_notification_30s=FALSE
rutgers_pressure_test=FALSE
```
