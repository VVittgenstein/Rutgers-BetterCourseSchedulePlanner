# P4 公网Runtime、Session与Capability合同

## 1. 结论

公网WebUI保持P3冻结的共享课程搜索、22项筛选、course/section信息、Open状态、最多9个selected sections、显式live watch、toast与浏览器声音提醒；它与本地一键包不是两套普通用户产品。

公网delta是：**用户状态只活在当前top-level document对应的页面session中；服务运行状态集中共享并可跨进程重启。** 公网不提供Saved views或任何等价持久化表面。浏览器永不直接访问Rutgers。

## 2. 三层状态所有权

| 层 | 允许内容 | 生命周期 | 存储位置 |
|---|---|---|---|
| 页面session状态 | 当前filters、sort/page、selected sections、页面内language选择、volume、sound mode/duration、Max audible设置/已用计数、toast/alert、当前active watch UI | 当前top-level document；reload/new tab/new window后重建 | React memory；不得写browser durable/session storage |
| 服务进程ephemeral状态 | `connection -> SectionKey`、`SectionKey -> connections`、WS heartbeat、当前alarm fanout bookkeeping | 当前连接/进程；断连或重启即失效 | `bcsp-server` memory |
| 服务共享operational状态 | Catalog、canonical Section、Open LKG/checkpoint、attempt/target observation、日聚合、scheduler/circuit、服务级Rutgers-day counters | 可跨进程与服务器正常重启；按P3 retention | 服务SQLite与运维checkpoint；不含个人状态 |

严禁把页面session或连接映射写入共享SQLite。服务operation记录可以包含`(term,campus)` target、时间、结果、lag、hash、计数与错误分类，但不得包含用户session ID、浏览器指纹、IP、选中的sections、watch映射、声音设置、确认动作或个人notification history。

## 3. Top-level document session语义

### 3.1 新session边界

以下每次都创建全新、互不继承的页面session：

- 首次打开站点；
- reload / hard reload；
- 在新tab或新window打开任意BCSP route；
- 关闭后再次打开；
- 直接访问course或section详情URL。

同一已加载SPA内的客户端route navigation仍属于当前页面session，不重置状态。Direct course/section route是共享信息访问能力，不是Share link，也不得在URL中恢复filters、selected、audio、watch或Saved state。

### 3.2 初始化次序

每个新document必须按以下顺序初始化：

1. 服务返回不可缓存的HTML/bootstrap，生成只适用于该document的随机nonce；
2. 语言根据`navigator.languages`、浏览器`Accept-Language`与产品支持集选择`en-US`或`zh-CN`，无法匹配时使用`en-US`；
3. filters恢复FilterSchema默认，selected为空，active为空；
4. volume、ONE_SHOT/CONTINUOUS、duration与Max audible恢复产品默认；
5. 从服务共享checkpoint读取Catalog/Open freshness、状态和service-wide counters；
6. 用户必须再次显式点击“开始订阅”才能建立active watch并解锁AudioContext。

语言可在当前页面临时切换，但reload后重新按系统/浏览器协商。除language初始值外，其余所有用户设置按产品默认初始化，不能从以前页面、cookie、URL或服务数据库恢复。

### 3.3 浏览器存储与缓存

- 禁止用`localStorage`、`sessionStorage`、IndexedDB、Cache Storage、cookie、Service Worker data、URL query/hash或history state跨top-level load保存用户产品状态。
- Cookie只可用于不承载用户偏好的短期安全机制；本设计优先使用document bootstrap nonce与same-origin header。任何安全token都不能恢复用户状态或active watch。
- HTML/bootstrap使用`Cache-Control: no-store`；内容寻址的静态assets可以长期immutable缓存，但不得把用户状态烘焙进asset或Service Worker cache。
- 浏览器back/forward cache若恢复旧document，客户端必须通过page lifecycle和server session有效性检查；不能复活已失效的watch。若无法可靠证明，pageshow persisted时清空active并要求用户重新开始。

## 4. Public capability surface

| 能力 | 公网合同 | 本地差异 |
|---|---|---|
| Search/filter/course/section/direct detail | 共享、完整保留 | 同一共享实现 |
| Filter current state | 当前页面内 | 本地跨启动保存 |
| Selected sections | 当前页面内，最多9 | 本地可持久保存selection但不恢复active |
| Language | 每个document按系统/浏览器；页面内临时切换 | 本地override可持久 |
| Audio settings/Max audible | 每个document恢复默认 | 本地配置值可持久 |
| Active watch | 显式开始；连接/页面结束即失效 | 同样不跨启动恢复 |
| Current-page alert/history | 当前页面内可见 | 本地episode/watch history持久 |
| Catalog refresh | 固定600秒，不给普通用户设置 | 本地1–1440分钟 |
| Open refresh | 固定30秒，active batch目标10秒，不给普通用户设置 | 本地默认30秒、3–3600秒 |
| Saved views | **ABSENT** | `LOCAL_ONLY`完整CRUD/CAS/migration |
| Reset local user data | **ABSENT**；新document天然回默认 | 本地带确认的安装级Reset |

“当前页面内可见history”只是React对本页面收到的Open episode/alert进行临时汇总；它不是持久notification history，不提供跨页面查询API。

## 5. Saved views零表面合同

公网制品必须同时满足：

1. 无Save/Apply/Rename/Update/Duplicate/Delete/Delete-all入口、组件和文案；
2. 无SavedViewDefinition、codec、migration、repository、CAS handler或storage key；
3. 无`/saved-views` route、REST/GraphQL/WebSocket method或bootstrap capability；
4. 无浏览器storage、server table、seed、telemetry event或日志字段；
5. 无Saved views专属i18n key与chunk；
6. 无URL filters、Share links、账号/cloud sync、default view或自动恢复替代物；
7. 公网编译产物静态扫描不得出现P5 denylist中冻结的symbols/routes/keys。

公网仍消费共享FilterSchema进行normalization、query、chips和i18n；不得因为排除local Saved codec而复制一套FilterSchema。

## 6. Browser、API与Rutgers隔离

- 浏览器HTTP与WS只连接部署域名；CSP `connect-src`只允许same-origin HTTPS/WSS（开发环境另行显式配置，不进入release）。
- Rutgers URI、client、retry、ETag、cache与Open reconcile只存在于服务端shared poller。
- 页面load、API query、filter变化、route变化、WS连接、selected/watch数量都只读取已有服务状态或改变fanout映射，不能直接排入一个新的Rutgers请求。
- 公网不暴露“强制上游刷新”给普通用户。页面“刷新结果”若存在，只重新查询共享数据库并返回最新checkpoint，不绕过scheduler、backoff或circuit。
- CORS默认关闭；只接受部署origin。状态变更HTTP验证Origin与document nonce；WS升级验证Origin、协议版本与nonce。
- Rutgers raw payload、绝对文件路径、SQL、secret、session nonce和连接映射不得返回给页面。

## 7. WebSocket与active watch生命周期

1. 页面可选择最多9个复合`SectionKey=(term,campus,index)`；第10个明确拒绝。
2. 用户明确开始watch；服务验证每个key并建立当前connection内存映射。
3. Active watch只把相关共享`OpenBatchKey=(term,campus)`提升为10秒目标；同batch任意用户/section数量都不增加上游请求。
4. 服务只fanout由有效、安全reconcile生成的section-level `OpenObservation`，并带observation ID、target refresh ID、状态、freshness和时间。
5. WS transport短暂重连可以恢复当前页面的连接，但不得自动重新发送已经由server判定失效的watch；断线/heartbeat超时/服务重启后UI明确显示“订阅已停止”，要求用户重新开始。
6. Reload、新document、关闭tab、进程重启均清理active映射、audible计数和未确认alarm；任何operational checkpoint不得复活它们。
7. 多连接共享同一个server Open observation；fanout规模与Rutgers请求规模解耦。

## 8. 服务重启与共享状态

正常重启后：

- 读取Catalog/Open SQLite、LKG、target checkpoint、failure/circuit与当前Rutgers-day aggregates；
- 恢复scheduler的安全due状态，但不catch-up burst；
- `attempted/succeeded/failed/empty`按`America/New_York`当天持久事实继续累计，不归零；
- 清空全部connection/watch/session/audio/alert内存；
- 新页面只看到service-wide target counters、freshness、lag与故障，不看到任何过去用户活动。

如果operational DB不可用，服务readiness失败并保持安全降级；不能用Catalog raw status伪造live Open，也不能为了“补计数”突发请求Rutgers。

## 9. 安全与隐私最小化

- document nonce使用CSPRNG，短生命周期，只在内存；不可写日志。
- Session日志只使用瞬时request trace ID；不得建立可跨页面跟踪的稳定用户ID。
- Watch/fanout诊断默认只记录聚合连接数、target和错误类别，不记录逐用户section列表。
- API错误使用stable code与安全details；不泄露SQLite、Rutgers raw、server path或部署secret。
- CSP、frame-ancestors、nosniff、Referrer-Policy与same-site安全头由P7公网runtime/ops测试冻结。

## 10. P7验收场景

| ID | 场景 | 必须结果 |
|---|---|---|
| P4-SESSION-001 | 设置filters/audio/selected后reload | 新session；全部恢复默认/空；语言重新协商 |
| P4-SESSION-002 | 同route新tab | 与旧tab完全隔离，且不产生Rutgers pull |
| P4-SESSION-003 | SPA内导航后返回 | 同document状态可继续；active语义不变 |
| P4-SESSION-004 | WS断线、heartbeat超时、服务重启 | watch停止且不自动恢复；service counters继续 |
| P4-SESSION-005 | 多用户watch同batch | 一个共享poll target；fanout到各connection |
| P4-SESSION-006 | 扫描public source graph与compiled artifact | Saved symbols/API/storage/i18n/chunk全部不存在 |
| P4-SESSION-007 | DevTools检查storage/cookies/cache/URL | 不存在可恢复个人产品状态的数据 |
| P4-SESSION-008 | public API/WS fuzz Origin/nonce/key | fail closed且不泄露内部信息 |
| P4-SESSION-009 | direct section URL reload | 详情可访问，但filters/selected/watch/audio不恢复 |
| P4-SESSION-010 | 页面load/query/reload并发 | 上游attempt数不因页面事件增加 |

对应P7任务：共享部分进入`P7-SHARED-QUERY/WATCH/UI/I18N`；公网delta进入`P7-PUBLIC-SESSION/RUNTIME/ZERO-SURFACE`。

## 11. Machine-readable state

```text
status=P4_PUBLIC_RUNTIME_CONTRACT_FROZEN
public_session=NEW_ON_EVERY_TOP_LEVEL_DOCUMENT_LOAD
user_state=EPHEMERAL
public_personal_persistence=NONE
language_init=SYSTEM_OR_BROWSER_EACH_LOAD
saved_views=ABSENT
saved_views_zero_surface=TRUE
browser_direct_rutgers=FALSE
upstream_architecture=CENTRAL_POLLER_WEBSOCKET
new_document_upstream_amplification=FALSE
public_operational_state=SERVICE_WIDE_NON_PERSONAL
operational_state_survives_restart=TRUE
active_watch_survives_document_or_restart=FALSE
public_user_refresh_configuration=FALSE
```
