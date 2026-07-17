# P6 最终 Linux 公网包与独立部署执行计划

## 1. 交付与权限合同

- **状态**：`P6_PUBLIC_EXECUTION_PLAN_AMENDED_FOR_REVIEW`
- **最终制品**：`LINUX_PUBLIC_DEPLOYMENT_PACKAGE`
- **验证目标**：Ubuntu 24.04 LTS clean VM、`x86_64-unknown-linux-gnu`；增加其他发行版/架构须单独Review
- **运行时**：`bcsp-server` + 嵌入式 public React WebUI + SQLite + systemd；Caddy作为外部edge
- **public用户状态**：每个top-level document load均为新的ephemeral session；无持久个人状态、无Saved views能力
- **P7预发布验证**：经独立精确授权后，P7.5可在GitHub Actions与指定Vultr staging实例消费同一个冻结候选包；这不是生产部署
- **真实生产部署**：不是第三个包，且必须在P7.5完成后重新发现现场状态并另行授权
- **当前执行权限**：无；P7须经P6 Review批准，生产部署还须在P7之后另行批准

公网包由本地/共享产品基线增加public adapter、Linux entry与operations材料形成，不是第二套产品或长期fork。所有Catalog、query、Open、watch/episode和共享UI语义只实现一次。

## 2. 公网包 allowlist 与 denylist

| archive成员 | 用途 |
|---|---|
| `bin/bcsp-server` | locked release Rust binary；嵌入public WebUI；不含debug/source path |
| `systemd/bcsp.service` | 无真实host/secret的审计unit模板 |
| `caddy/Caddyfile.example` | 无真实域名、IP、token、证书路径的loopback reverse-proxy模板 |
| `config/bcsp.env.example` | 只列非secret示例、schema和必需key；不含真实值 |
| `ops/` | 幂等install/verify/backup/restore/upgrade/rollback脚本或等价runbook |
| `docs/` | operator quickstart、health/readiness、logging、backup、restore、rollback、troubleshooting |
| root metadata | `VERSION`、`SHA256SUMS`、build provenance、SBOM、third-party notices/license |

archive不得包含local user-state/Saved views/history/reset/interval配置源、personal table/migration或任何对应表面、Windows launcher、Node/npm/Vite dev runtime、source map、raw evidence、真实domain/IP、SSH/Cloudflare材料、`.env`真实值、Caddy ACME state、任何`.sqlite`/`.db`/`-wal`/`-shm`、schema seed、预装Catalog/Open课程数据、checkpoint、log/backup/session/watch、private inventory或内部执行状态。`/var/lib/bcsp`是安装后service-owned外置状态，不是archive成员。

## 3. 公网产品 P7 可执行顺序

公网工作与本地工作共享`P7.1-004..009`等基础task，不得复制。下表的`P-*`是便于按公网交付阅读的执行视图，不是另一套task ID；其与`04-p7-task-and-commit-matrix.tsv`中canonical task的精确映射见第03文档第6节。依赖、skill、commit boundary或stop gate若有歧义，以`04`为准；`P-*`只描述public delta的消费顺序。

| ID / P7阶段 | 输入 | 执行动作 | 必需输出 | 验收门 | 回滚点 |
|---|---|---|---|---|---|
| `P-00 / P7.1` | P6批准；`P7.1-003` workspace/build boundary | 建立public capability manifest、Linux composition root、public config/operational-store ports | `bcsp-server` composition与public typed config | dependency graph无local crate/source；共享合同未分叉 | 回退public skeleton task；不改shared core |
| `P-01 / P7.1` | `P-00` | 实现ephemeral document session、nonce/WS生命周期、connection-bound selection/watch/audio state | public session/runtime adapter | reload/new-tab恢复默认；断连清watch；普通用户不可强制上游refresh | 回退runtime adapter task；清理临时session |
| `P-02 / P7.1` | `P-00`及`P7.1-005..009`共享合同 | 注入public固定策略与service operational state；只装载operational migrations | 固定600/30/10 cadence、`/var/lib/bcsp`空状态目录首次启动建库、Rutgers-day service counters、health/readiness | pre-start state dir为空；pre-network schema-only且课程/Open为0；无LOCAL_ONLY personal table/migration；重启后service-day counter连续；浏览器事件不放大请求 | 回退policy/store task；恢复测试operational DB snapshot |
| `P-03 / P7.1` | `P-01`,`P-02` | 建立public router/API/schema/assets graph与shared HTTP/WS | public HTTP/WS runtime | Saved views/local state八层零表面；direct section URL与shared current-page watch仍存在 | 回退router task；不得用runtime hide代替编译排除 |
| `P-04 / P7.1` | `P-03` | 建立public React entry、shared product shell、system-language initialization | 功能完整public UI baseline | en-US/zh-CN parity；reload全部current state默认；无local route/API/storage/i18n/bundle symbol | 回退public entry task，保留sharedUI baseline |
| `P-05 / P7.2` | local/public功能壳均集成通过 | 与local同一正式UI task，同时使用`$industrial-brutalist-ui`与`$design-taste-frontend`完成desktop/mobile响应式界面 | 正式public UI及视觉验证 | 共享视觉一致；public固定/ephemeral/zero-surface状态准确；无local泄漏 | 回退P7.2独立commit到功能壳 |
| `P-06 / P7.3` | `P-05`已实现、集成、视觉验证 | 新的独立task先产出Before/After/Why，再只使用`$emil-design-eng`审计打磨 | 独立audit/polish记录与重新视觉验证 | findings一一闭合；与P7.2 task/record/commit不同；视觉与功能回归通过 | 回退P7.3独立commit，保留验证过的P7.2 |
| `P-07 / P7.4` | `P-06`与共享测试通过 | 完成Linux release build、embedded assets、package allowlist和ops模板 | Linux public package candidate | locked/reproducible build、SBOM/license/secret/path/source scan、public zero-surface | 删除candidate并回退package task |
| `P-08 / P7.4` | `P-07` | 在Ubuntu 24.04 clean VM安装到隔离测试环境，运行deterministic fake-upstream、systemd/Caddy/HTTPS test-CA、backup/restore/upgrade/rollback | clean-VM候选记录、不可变artifact hash与P7.5入口 | 第6节deterministic部分全部通过；不访问真实production | 保留失败证据，撤销candidate资格并回到最早失败task |
| `P-09 / P7.5` | `P-08`与live/staging授权门 | GitHub Actions手动workflow安装真实Linux候选并执行有界真实Rutgers E2E；随后在指定Vultr staging以相同hash执行真实Caddy HTTPS/WSS、desktop/mobile与恢复演练 | 两级Linux去敏证据、请求ledger、候选hash一致性、Vultr基线恢复证明 | 第6节live部分、public零表面、集中poller、fresh reload及恢复/残留扫描全部通过 | 不得现场修包；回到owner task并重建两个新candidate；Vultr恢复或按批准方案重装后停止 |

## 4. Public runtime冻结语义

### 4.1 Session与能力

- 每个top-level load分配新的document session；默认filters、selection、watch、audio、settings、history均重新初始化，语言每次按系统/浏览器选择`en-US`或`zh-CN`。
- active watch只在当前连接/文档生命周期内存在；disconnect、document unload或server restart清理，不持久化。
- public无Saved views source、DOM、route、API、storage、i18n、bundle或package surface；也无persistent prefs/history/reset-local-user-data或refresh interval编辑能力。
- current-page selection、watch、toast、ONE_SHOT/CONTINUOUS、volume和sound mode是共享临时功能，不得因“无持久状态”而删除。

### 4.2 Refresh、容量与降级

- Catalog固定600秒；普通Open固定30秒；含至少一个active watch的同`(term,campus)`batch目标10秒；普通用户不可改值或触发额外Rutgers请求。
- Catalog/Open共用real-origin concurrency `1`，EDF按absolute due调度，同deadline才watch优先；错过tick合并/跳过，不追赶。
- saturation时显式显示actual interval和lag，不通过并发、burst或跨campus union隐瞒；失败/unsafe/catalog-race保持LKG并立即stale。
- service-wide `America/New_York` today attempted/succeeded/failed/empty counters存入operational DB并跨进程restart；它们不是用户history，也不因新page重置。
- valid observation到server fanout目标`<=1s`；真实seat变化、上游cache、排队及浏览器audio不组成严格30秒SLA。

## 5. Linux运行与运维拓扑

```text
Internet
  -> Caddy :80/:443 (TLS, headers, WS reverse proxy)
  -> 127.0.0.1:<fixed-local-port>
  -> bcsp-server (systemd declared dedicated non-root `bcsp` user)
  -> /var/lib/bcsp (SQLite/state)
```

- `bcsp-server`只监听loopback；只有Caddy监听公网80/443。
- 专用`bcsp`用户无login、home、sudo；只能写`/var/lib/bcsp`与systemd runtime目录。
- versioned releases位于`/opt/bcsp/releases/<version>`，`/opt/bcsp/current`原子指向当前版本；`/etc/bcsp`由root控制，state、backup与release分离。
- fresh install在首次service start前保持`/var/lib/bcsp`空；首次启动先创建schema-only operational DB，再开始有界上游摄取。公网binary不得链接或创建LOCAL_ONLY personal migrations/tables。
- systemd必须设置working directory、read/write paths、restart/backoff、resource与权限hardening；readiness失败不得被当作成功上线。
- health只返回version/readiness和安全reason code，不泄漏raw payload、SQL/path、secret、session/watch或私有拓扑。
- logs结构化进journald；不记录raw body、cookie/header、IP、完整UA、watched sections、sound settings、absolute private path或secret。

## 6. Clean Ubuntu 24.04 验收

在全新VM且无Rust/Node/Python/Git开发环境下，使用candidate archive完成：

1. 离线验证SHA256、SBOM、provenance、license与archive allowlist，并证明archive没有DB/WAL/SHM、seed或真实课程/Open数据；
2. install到新的versioned release并创建最小权限用户/空state目录；阻断上游首启只创建schema-only operational DB且Catalog/Open/observation行为0；
3. 安装unit，验证`systemd-analyze security`或等价审计、restart loop与readiness；
4. Caddy配置验证，使用隔离test domain/本地CA或等价非生产TLS验证HTTPS、WS、headers与静态cache policy；
5. 3/10/30/3600 fake-upstream、EDF/lag、concurrency1、failure/429/circuit、stale/UNKNOWN与`<=1s`fanout目标；
6. desktop/mobile浏览器、course/section/22筛选、watch/audio、en-US/zh-CN；
7. reload/new-tab新session，断连/重启清active watch；service-day counters跨重启；
8. source/DOM/route/API/storage/i18n/bundle/package八层Saved views/local-state零表面；
9. SQLite online backup、manifest/hash/integrity、隔离restore drill；
10. forward migration、upgrade、不可逆migration配套DB snapshot rollback、damaged DB与disk-full降级；
11. logs/metrics不泄漏敏感数据，archive无private inventory/absolute build path；
12. 从同一source+lock+toolchain重复构建满足第05文档的reproducibility门。

P7.5的真实世界部分分两级且都消费P7.4冻结的同一Linux package hash：

1. GitHub Actions只允许人工`workflow_dispatch`、`contents: read`和单一Ubuntu job；安装真实候选包而非从源码重建，启动真实`bcsp-server`、systemd与真实Caddy HTTPS/WSS测试配置，以desktop/mobile浏览器连接真实Rutgers，验证搜索、多筛选、Course/Section、Open join、freshness/lag/counters、WS/watch、reload新session、多个浏览器不各自直连Rutgers以及Saved views/history/Reset零表面。不得使用OIDC、deployment、Release、Vultr或Cloudflare权限，不得push/PR定时自动触发live test。
2. 指定Vultr实例在P7期间只能标记为`staging/non-production`。`P7.5-004`开始前必须再次核验实例身份、凭据、费用、未知服务、精确变更allowlist并创建可验证恢复点；随后安装完全相同的候选hash，以真实Caddy进程和测试专属内部CA/客户端hosts映射完成同一desktop/mobile流程。测试后必须恢复snapshot或按批准方案重装，并通过基线/残留审计。任何真实DNS、Cloudflare、ACME公网证书或生产流量需求都必须硬停止并另行Review。

两级测试都不得把raw Rutgers body、数据库、private inventory或secret上传为Actions artifact或提交Git。若GitHub hosted runner无法访问Rutgers，记为`ENVIRONMENT_BLOCKED`并停止，不能用Vultr结果替代；任一mandatory assertion为FAIL/BLOCKED/SKIPPED/无法解释的INCONCLUSIVE都不能完成P7。

## 7. P7之后的独立生产部署顺序

以下`D-*`只是一份未来runbook。即使P7.5和GitHub Release完成也不得自动执行；必须先确认Vultr staging测试后的恢复/重装结果，再由用户针对届时真实外部状态重新发现并另行批准。测试同一台机器不等于批准生产提升。

| ID | 输入 | 未来操作 | 输出/验收 | 失败回滚 |
|---|---|---|---|---|
| `D-00 AUTHORIZE` | P7.5最终public hash、三环境E2E、Vultr恢复证据、SBOM/scan、rollback演练 | 申请明确生产discovery授权并列出预期只读检查 | 用户批准的服务器、域名、DNS/Cloudflare、Vultr范围 | 未批准则零变更停止 |
| `D-01 REDISCOVER` | `D-00` | 在staging恢复/重装后重新只读核验Vultr实例/OS/arch/资源/费用、DNS、Cloudflare proxy、80/443、Caddy/systemd、磁盘/backup、SSH key与现有服务 | 生产转换前现场preflight记录，无secret值 | 事实冲突则停止回Review；不得复用P7.5旧快照事实冒充当前状态 |
| `D-02 BACKUP` | preflight通过 | 创建并验证当前DB/config/manifest的一致性backup与restore可用性 | 带hash的加密/受限backup、restore checkpoint | backup/restore失败则不部署 |
| `D-03 STAGE` | backup通过 | 验证public package并解压到新version目录，写root-owned env引用 | 未切流的staged release | 删除staged目录，不触碰current |
| `D-04 MIGRATE` | stage通过 | maintenance窗口内停流/停服务，按版本执行migration并启动loopback canary | canary health/query/open/freshness通过 | binary+DB作为一组恢复到predeploy snapshot |
| `D-05 EDGE` | canary通过 | 原子切current，安装/reload systemd，验证并原子reload Caddy；按批准策略更新DNS/Cloudflare | HTTPS/WS/headers/health通过 | 恢复旧symlink/unit/config/DNS，恢复DB如需要 |
| `D-06 OBSERVE` | 已切流 | 在明确观察窗口核验5xx、resource、lag/circuit、SQLite/WAL、counters、desktop/mobile | 生产验收记录与最终状态 | 触发阈值即执行D-05回滚 |
| `D-07 CLOSE` | 观察通过 | 记录非secret部署结果、保留旧release、安排backup/restore drill | 可审计closeout | 不删除唯一可回退版本 |

部署中不得将secret写入CLI、shell history、Git、unit正文、archive或日志；使用root-owned file、systemd credential或已批准secret manager。Cloudflare token、SSH key、TLS/ACME data与真实inventory不进入公开证据。

## 8. Upgrade/rollback原子性

- 每次upgrade先verified backup，再解压新version；不原地覆盖current binary。
- binary、DB schema、embedded UI与config schema视为一个release unit。不可逆migration必须在切换前有可恢复snapshot。
- Caddy配置先`validate`再atomic reload；应用先loopback canary再切流。
- rollback同时恢复compatible binary与DB；回滚后验证health、query、counters、freshness、Caddy headers/WS。Open可标STALE，不伪造无中断连续性。
- 至少保留一个已验证旧release；删除旧release、rotation/retention或成本变更均在部署授权范围内执行。

## 9. 完成与停止条件

只有`P-00..P-08`全部通过，公网包才成为P7.4冻结候选；它还必须由`P-09/P7.5`在GitHub Actions与获批Vultr staging以同一hash完成真实世界E2E和恢复审计，才可进入最终P7完成记录。P7.5证据不是第三个包。若需要提高Rutgers并发、给用户refresh控制、持久个人状态、Saved views stub、第三个包、容器/Node第二runtime、真实DNS/Cloudflare/ACME或生产流量，停止回Review。

```text
phase=P6
plan=PUBLIC_PACKAGE_AND_DEPLOYMENT
status=P6_PUBLIC_EXECUTION_PLAN_AMENDED_FOR_REVIEW
package=LINUX_PUBLIC_DEPLOYMENT_PACKAGE
validation_os=UBUNTU_24_04_LTS
public_state_root=/var/lib/bcsp
public_first_start_creates_database=TRUE
public_release_contains_database=FALSE
public_release_contains_real_catalog_open_data=FALSE
public_local_personal_table_count=0
public_saved_views_surface=0
public_persistent_personal_state=FALSE
public_catalog_seconds=600
public_general_open_seconds=30
active_watch_target_seconds=10
origin_max_concurrency=1
deployment_is_third_package=FALSE
p7_authorized=FALSE
vultr_staging_mutation_authorized=FALSE
production_deployment_authorized=FALSE
production_mutations=0
```
