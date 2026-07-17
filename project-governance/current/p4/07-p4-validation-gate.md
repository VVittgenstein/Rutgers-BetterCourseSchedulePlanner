# P4 验证与冻结总门

## 1. 最终结论

- 阶段：`P4`
- 记录状态：`P4_PASS`
- Gate：`P4_PASS`
- P3 输入门：`P3_PASS`
- P5：`ELIGIBLE`
- P6：`NOT STARTED — REQUIRES P5 PASS`
- P7：`NOT STARTED / NOT AUTHORIZED`
- 存储修订：`P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001`（2026-07-13）
- P4 Rutgers 网络请求或请求产物：`0`
- 产品源码、依赖、数据库、构建或包变更：`0`
- 生产服务器、DNS、Cloudflare、证书、Release 或其他外部变更：`0`

`tools/validate-p4.ps1 -ValidationMode Candidate`已验证冻结输入、全部矩阵、22筛选、零表面笛卡尔积、trace与零副作用字段；本文件与`07a`随后提升为最终`P4_PASS`并再次接受Final验证。2026-07-13的`P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001`只修订存储与data-empty package合同并要求hash级联复核，不授权实现、构建、发布、生产部署或P7。

## 2. 冻结输入与阶段完整性

P4只消费当前主线P2/P3与权威工作流。`00`固定的8个P3关键输入必须逐字节保持SHA-256一致，其中P3总门为`P3_PASS`、共享Open合同为`FROZEN_P3_SHARED_OPEN_CONTRACT`、本地完整计划为`P3_LOCAL_ONECLICK_PLAN_FROZEN`，P3额外Rutgers请求授权为0。

P4没有重新解释raw证据、重新访问Rutgers或建立第二套Open合同。P3的以下事实继续是不可降级前置条件：

- SectionKey为`(term,campus,index)`，OpenBatchKey为`(term,campus)`；
- 不同campus不得为状态变更动态union；orphan不创建Section；unsafe empty、zero-intersection、失败和Catalog race均保留LKG且不得mass-close；
- Catalog/Open共享真实origin concurrency=1、每target single-flight、EDF无饥饿、missed tick不追赶；
- Public普通Open固定30秒、active-watch batch目标10秒；不承诺真实seat变化到通知严格30秒；
- 每attempt、target observation和section observation分层，stale/UNKNOWN在Open筛选中返回UNCERTAIN；
- ONE_SHOT与CONTINUOUS声音合同、最多9个selected sections及浏览器音频边界不因公网部署而改变。

## 3. P4产物闭包

| 产物 | 冻结内容 | 候选状态 |
|---|---|---|
| `00-p4-charter-and-preflight.md` | 阶段权限、P3 hash、两个包、零网络/零生产边界 | `P4_CORE_DESIGN_FROZEN` |
| `01-public-baseline-delta-matrix.tsv` | P2/P3本地基线逐项分类；76行，三类disposition | `FROZEN DESIGN INPUT` |
| `02-public-runtime-session-capability-contract.md` | 公网ephemeral session、共享能力、状态所有权与安全边界 | `P4_PUBLIC_RUNTIME_CONTRACT_FROZEN` |
| `03-public-refresh-capacity-degradation-plan.md` | 固定双时钟、容量、排队、降级、观测与fake-upstream门 | `P4_PUBLIC_REFRESH_CONTRACT_FROZEN` |
| `04-linux-package-and-operations-plan.md` | Linux外部单库、data-empty包、首次启动schema-only建库、systemd、Caddy/HTTPS、日志、备份、升级/回滚与部署授权边界 | `P4_PUBLIC_PACKAGE_OPERATIONS_PLAN_FROZEN` |
| `05-public-capability-deny-audit.tsv` | 18项禁用能力 × 8个artifact surface = 144行 | `PUBLIC_ZERO_SURFACE` |
| `06-p4-traceability-matrix.tsv` | 6个冻结输入、78个baseline/delta测试、144个zero-surface测试、30个核心合同验收，共258行 | `MAPPED_TO_P7` |
| `07a-p4-validation-gate.json` | 本门的机器可读最终状态 | `P4_PASS` |

`01`的76行包括46项`INHERIT_SHARED`、19项`PUBLIC_MODIFY`与11项`PUBLIC_EXCLUDE`。它逐项列出全部22个FilterSchema字段，且明确覆盖Catalog/Open双刷新、Section identity、course-centered与independent section访问、active watch、toast、ONE_SHOT、CONTINUOUS、Max audible、WebUI音频、i18n、ephemeral state、Linux runtime/ops及LOCAL_ONLY排除项。

## 4. 已冻结的用户裁决

以下不是P4自行扩张，而是对已批准P2/P3裁决的公网delta闭包：

1. 最终产品仍严格只有两个包：Linux公网包与Windows本地一键包；真实部署不是第三个包。
2. 公网每次top-level document load都是全新匿名页面session；filters、selected、volume、声音模式/时长、Max audible、设置、alert/history和active均不跨load持久。
3. 公网语言每个新页面按浏览器/系统初始化，只允许当前页面临时切换；至少完整支持`en-US`与`zh-CN`，fallback为`en-US`。
4. 公网不提供Saved views；本地仍完整提供。公网无入口、DOM、route、API、storage、i18n、bundle或package表面，也不得用URL filter恢复、Share或default view替代。
5. 公网保留共享22筛选、三值与same-section witness、course-centered结果、独立section搜索和direct detail。
6. 公网保留当前页面内最多9个selected sections、显式live watch、toast、ONE_SHOT Max audible与CONTINUOUS闹钟确认；active永不持久。
7. 公网Catalog固定600秒；普通Open固定30秒；有active watch的同一batch目标10秒。普通用户不能配置或强制上游刷新；reload、query、tab、section和watch人数不线性增加Rutgers请求。
8. 公网服务只在archive外的`/var/lib/bcsp/rbcsp.sqlite`持久共享Catalog/Open LKG、checkpoint、有限诊断与service-wide Rutgers-day counters；公网migration graph不得包含Windows LOCAL_ONLY个人表或migration，也不得把共享状态变成个人prefs、selection、watch或history。
9. 公网不提供email、Discord、Web Push、native/system notifications、Waitlist、Share links、Named Compact、macOS、Calendar、quiet/snooze、persistent personal subscription或本地Reset。
10. systemd、Caddy、HTTPS、备份、升级和回滚在P4只形成计划；生产服务器、域名、DNS、Cloudflare、证书、凭据与GitHub Release没有获得变更授权。
11. 两个最终archive都不得包含数据库、WAL、SHM、seed、fixture或真实/缓存Catalog/Open数据；Windows首次运行在包根目录创建schema-only `data/rbcsp.sqlite`，公网首次启动在外部state root创建schema-only `/var/lib/bcsp/rbcsp.sqlite`，真实数据都只可随后由runtime获取。P4只承接公网delta，不把Windows路径行为带入Linux。

## 5. ALL检查

候选验证必须同时证明：

- `01`每一行都有输入、唯一ID、三类disposition之一、完整公网合同、理由、消费者、验证ID与P7任务；
- 22个`FLT-*`标识在`01`中各自出现并进入`06`，不是用“支持筛选”概括；
- Open join、safe-empty/LKG、双时钟、single-flight、共享origin limiter、EDF、backoff/circuit、observation、freshness、counter、latency wording和通知前置条件全部有验证消费者；
- 当前页面selection/watch/audio仍被保留，只有其持久化或自动恢复表面被排除；
- service operational state与personal state分离，服务重启不复活active watch；
- `01`的指定storage/package delta逐项排除Windows包根`data/rbcsp.sqlite`路径、LOCAL_ONLY个人migration/table以及所有预装数据库或真实数据；
- 公网archive通过positive allowlist与database/WAL/SHM/seed/fixture/真实Catalog/Open denylist证明data-empty，首次启动才在`/var/lib/bcsp/rbcsp.sqlite`创建schema-only operational DB；
- Public Linux入口、数据库、readiness、日志、systemd、Caddy、TLS、备份、升级、恢复、包manifest和生产授权门均落到P7任务。

## 6. ONLY与PUBLIC_ZERO_SURFACE检查

`05`对18项能力逐一横跨以下8个表面：`SOURCE / DOM / ROUTE / API / STORAGE / I18N / BUNDLE / PACKAGE`。每行状态必须为`PUBLIC_ZERO_SURFACE`，并具有唯一测试ID及允许例外，避免错误删除共享能力。

关键边界：

- Saved views的“source zero”指不得进入public build-reachable source graph；LOCAL_ONLY实现仍可在同一仓库隔离存在。
- Persistent selection/watch的“zero”只禁止持久化、恢复和稳定身份；当前页面selection、connection-bound active映射与共享Open fanout明确保留。
- Share links的“zero”不删除稳定direct section URL；direct URL只携带SectionKey，不能恢复filters或自称Share。
- Compact的“zero”不删除响应式信息密度；只删除Named Compact模式、持久开关与产品宣称。
- Web Push/system/native的“zero”不删除当前页面WebSocket、toast或WebUI通用声音。
- Persistent history的“zero”不删除服务级匿名Open诊断与Rutgers-day counters。

## 7. 追踪与P7任务

`06`的258行必须全部有非空P7任务与验证门：

- 6个`FROZEN_INPUT`保护P3/P2/workflow解释边界；
- 78个`BASELINE_DELTA_TEST`来自`01`的全部验证ID；
- 144个`PUBLIC_ZERO_SURFACE_TEST`与`05`一一对应；
- 30个`CORE_CONTRACT_TEST`逐一承接`02`的10个session、`03`的10个refresh和`04`的10个ops验收ID；
- P7任务只允许落到`P7-SHARED-*`、`P7-PUBLIC-SESSION/RUNTIME/OPS/PACKAGE/ZERO-SURFACE`，不得创建第二套本地/公网业务核心。
- P7 public package验证必须从data-empty archive开始，并证明schema-only first start、external state root以及LOCAL_ONLY migration/table为零；本映射本身不授权P7。

P4只完成设计映射。P5仍须分析共享core、local adapter、public adapter、构建条件能力与测试复用；P6仍须合并最终执行DAG并停止在P6 Review。因此P4通过只使P5 eligible，不使P6或P7 eligible。

## 8. 零副作用与工作区保护

本阶段只新增`project-governance/current/p4/**`治理文件。没有：

- 发出Rutgers或其他新证据网络请求；
- 生成P4 request manifest、ledger、raw body或新增网络证据；
- 修改Rust、TypeScript、React、CSS、SQL、依赖、lockfile、数据库、runtime state、build、package或release；
- 登录服务器、SSH、执行systemctl/caddy、修改DNS/Cloudflare、申请证书、迁移生产数据或发布GitHub Release；
- 读取chat log、旧P1正文或deprecated产物作为权威输入。

## 9. 冻结与停止条件

候选验证成功后，只把`07/07a`提升为`P4_PASS`；这不是产品源码变更。Final验证再次复算输入hash、文件状态、矩阵行数/唯一性、22筛选、zero-surface笛卡尔覆盖、trace一一映射与零副作用字段。未来若修改任何P4合同，必须重新打开本门并重跑两阶段验证。

任一输入hash漂移、漏掉字段/表面、PUBLIC_EXCLUDE误删共享功能、public source graph必须携带LOCAL_ONLY能力、需要重写P3共享Open合同，或需要真实外部变更才能证明设计时，P4不得通过，必须停止回Review。

## 10. 最终机器状态

```text
phase=P4
record_status=P4_PASS
p4_gate=P4_PASS
p3_input_gate=P3_PASS
baseline_delta_rows=76
inherit_shared_rows=46
public_modify_rows=19
public_exclude_rows=11
deny_capabilities=18
deny_surfaces=8
public_zero_surface_rows=144
trace_rows=258
trace_frozen_inputs=6
trace_baseline_delta_tests=78
trace_public_zero_surface_tests=144
trace_core_contract_tests=30
storage_amendment=P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001
storage_amendment_date=2026-07-13
public_operational_database=/var/lib/bcsp/rbcsp.sqlite
both_package_first_start_databases=SCHEMA_ONLY_OPERATIONAL
final_archive_database_files=0
final_archive_wal_shm_files=0
final_archive_seed_fixture_files=0
final_archive_real_catalog_open_data=0
public_local_personal_migrations=0
p4_rutgers_request_artifacts=0
product_source_mutations=0
production_mutations=0
p5_eligible=TRUE
p6_started=FALSE
p7_authorized=FALSE
```
