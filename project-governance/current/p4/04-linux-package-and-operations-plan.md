# P4 Linux公网包与Operations计划

## 1. 交付与授权结论

- **制品名称**：`公网包`
- **目标平台**：Ubuntu 24.04，预期`x86_64`；P7/部署前必须以真实主机preflight确认
- **应用入口**：`bcsp-server`
- **服务管理**：systemd
- **公网边缘与HTTPS**：Caddy
- **应用监听**：loopback only；只有Caddy监听公网80/443
- **本阶段状态**：设计冻结，不构建、不安装、不部署
- **真实生产部署**：未授权；P7完成后仍需独立授权
- **存储修订来源**：`P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001`（2026-07-13）
- **P7状态**：未授权；本修订只把已批准的包内相对存储决定传递到公网delta

“公网包以及部署”是一条交付任务，但只有公网包是最终包之一。安装说明、备份、SBOM、release记录与部署活动都不是第三个包。

两个最终archive共享同一条data-empty边界：不得预装数据库、WAL、SHM、seed、fixture或真实/缓存的Catalog与Open数据。Windows首次运行在包根目录下创建`data/rbcsp.sqlite`；公网包必须排除该Windows路径合同，并在Linux首次启动时于archive外创建`/var/lib/bcsp/rbcsp.sqlite`。两种首次启动都必须先形成schema-only operational DB，真实Catalog/Open只能在随后由runtime获取。

## 2. Runtime拓扑

```text
Internet
   |
 TCP 80/443
   v
Caddy (TLS termination, security headers, reverse proxy, WS)
   |
 127.0.0.1:<private-port>
   v
bcsp-server (non-root systemd service)
   |                 |
   |                 +-> central Rutgers Catalog/Open poller
   v
/var/lib/bcsp/rbcsp.sqlite + runtime-only WAL/SHM/checkpoints
```

- Caddy与`bcsp-server`在同一主机时，应用不监听公网地址。
- 浏览器只访问部署域名；CSP与server routes都不允许browser direct Rutgers。
- Public user session/WS映射只在内存；Catalog/Open/LKG/counters是共享operational state。
- 可选Cloudflare只能在真实部署授权后经现场核验作为DNS/proxy层；它不是P4/P7主计算平台，也不得改变trusted proxy、TLS或缓存语义而不留证据。

## 3. 公网包allowlist

P7生成的单个versioned archive只允许包含：

| 路径/类别 | 内容 |
|---|---|
| `bin/bcsp-server` | release Rust binary；无debug symbols或开发路径泄漏 |
| `share/bcsp/` | 若未嵌入binary的content-hashed React静态assets |
| `share/bcsp/schema/`（仅在未嵌入binary时） | 仅public-compatible schema/migration与manifest；无LOCAL_ONLY个人表/migration、seed、fixture或数据 |
| `systemd/bcsp.service` | 审计过的unit模板 |
| `caddy/Caddyfile.example` | 无域名/凭据的最小reverse-proxy模板 |
| `config/bcsp.env.example` | 仅非secret示例与必需key说明 |
| `ops/` | 幂等install/verify/backup/restore/upgrade/rollback脚本或明确runbook |
| `docs/` | operator quickstart、health、backup/restore/rollback和故障说明 |
| root metadata | VERSION、SHA256SUMS、build provenance、SBOM、third-party notices/license |

公网包禁止包含：

- 私钥、token、真实domain/IP、SSH材料、Cloudflare凭据、`.env`实际值或Caddy account data；
- 源码、`.git`、Node/npm/node_modules、Rust toolchain/target cache、测试、coverage、raw P3 evidence、开发fixture、notebook/report；
- 任意数据库或数据库样本（包括`*.sqlite`、`*.sqlite3`、`*.db`）、WAL、SHM、snapshot、seed、fixture、真实或缓存的Catalog/Open内容、用户session、watch/history、logs、backup、checkpoint或生产config；
- Windows本地一键包文件、`RBCSP.exe`、BAT、包根目录`data/rbcsp.sqlite`路径行为、LOCAL_ONLY Saved views/personal table migration/API/i18n/chunk；
- 旧release、邮件/Discord/Web Push/Calendar/waitlist/Share link或macOS surface。

Archive内所有路径必须是相对路径且防止traversal/symlink escape。positive allowlist、数据库扩展名/sidecar denylist及内容扫描必须共同证明archive中的数据库文件数、seed/fixture数、真实Catalog/Open数据数均为0。P7必须在clean Ubuntu 24.04 VM从archive安装，不能依赖源码checkout、Node、Cargo或Git；P7当前未授权。

## 4. Filesystem与身份

推荐FHS布局：

| 路径 | owner/mode意图 | 内容 |
|---|---|---|
| `/opt/bcsp/releases/<version>/` | `root:root`, read-only | 解压后的versioned release |
| `/opt/bcsp/current` | root管理symlink | 当前release |
| `/etc/bcsp/bcsp.env` | `root:bcsp 0640` | runtime config/secret引用；不来自archive实值 |
| `/var/lib/bcsp/` | `bcsp:bcsp 0750` | 首次启动创建的`rbcsp.sqlite`及其runtime-only WAL/SHM、scheduler checkpoint；不来自archive |
| `/var/backups/bcsp/` | `root:root 0700` | operator backups；不由HTTP服务 |
| `/run/bcsp/` | systemd RuntimeDirectory | PID/socket/短期运行文件（如需要） |

创建专用`bcsp`系统用户与group：无login shell、无home、无sudo。它只可写`/var/lib/bcsp`和systemd授予的runtime目录；不能写binary、Caddy config、`/etc/bcsp`或backup目录。

静态assets与public-compatible schema/migration优先嵌入binary；若外置，则只从当前release只读目录提供。`/var/lib/bcsp`由安装流程创建，但`rbcsp.sqlite`只由首次服务启动创建；应用绝不从current working directory或release目录搜寻/创建状态文件。

## 5. systemd合同

Unit至少包含以下意图，并由P7在Ubuntu 24.04真实systemd环境验证：

- `User=bcsp`、`Group=bcsp`、`UMask=0027`；
- `WorkingDirectory=/var/lib/bcsp`，`EnvironmentFile=/etc/bcsp/bcsp.env`；
- `ExecStart=/opt/bcsp/current/bin/bcsp-server`，显式config/state path；
- `Restart=on-failure`与有界退避，禁止restart tight loop；
- `NoNewPrivileges=true`、`PrivateTmp=true`、`ProtectSystem=strict`、`ProtectHome=true`；
- `ProtectKernelTunables=true`、`ProtectKernelModules=true`、`ProtectControlGroups=true`；
- 空`CapabilityBoundingSet`、`LockPersonality=true`、`RestrictSUIDSGID=true`；
- `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6`；
- 仅`ReadWritePaths=/var/lib/bcsp`和必要runtime目录；
- stdout/stderr结构化写入journald，不写用户可控路径；
- readiness失败不能被systemd误认为成功部署。

若某hardening指令与最终SQLite/TLS/runtime冲突，P7必须给出最小例外、威胁解释与测试；不得直接移除整个hardening组。

应用启动时：验证config/schema、绑定loopback；若`/var/lib/bcsp/rbcsp.sqlite`不存在，则创建空数据库并只应用public-compatible migration。该首次创建边界必须是schema-only：在任何Rutgers运行时获取前，Catalog/Open及个人数据行均为0。随后才打开/恢复safe shared scheduler state并按readiness合同启动真实运行时填充。进程退出前停止接收新请求、关闭WS、checkpoint并有界drain；active watches绝不跨重启恢复。

## 6. Caddy与HTTPS合同

Caddy配置模板不得硬编码真实domain。真实部署时由operator填入已核验域名并执行`caddy validate`。合同要求：

- 自动HTTPS或等价受管证书；HTTP重定向HTTPS；
- `reverse_proxy`只指向应用loopback；保留WebSocket upgrade；
- HTML/bootstrap `Cache-Control: no-store`；content-hashed assets可`public, immutable`；API/WS/open checkpoint不被edge缓存；
- 设置CSP（至少default-src self、connect-src self对应HTTPS/WSS）、`frame-ancestors 'none'`、`X-Content-Type-Options: nosniff`、合适Referrer-Policy；
- HSTS只在真实HTTPS、域名和回滚路径验证后启用，不能在P4文档阶段假装已生效；
- 不盲信`X-Forwarded-For`。若以后启用Cloudflare proxy，trusted proxy范围必须用部署时官方来源与测试单独冻结；
- 不通过Caddy暴露`/var/lib/bcsp`、backup、env、metrics原始标签、debug或admin endpoint；
- TLS private key、ACME state与Caddy data不进入公网包或Git。

Public health响应只给版本/ready状态与安全原因码，不返回upstream raw body、path、SQL、secret、session/watch或私有拓扑。

## 7. Configuration与secrets

配置分三类：

1. **公网固定产品值**：Catalog `600s`、Open general `30s`、active target `10s`、origin concurrency`1`；不得通过普通用户UI修改。
2. **非secret部署值**：loopback listen address、public origin、固定外部state root `/var/lib/bcsp`、log level；有schema、明确默认与fail-closed validation。
3. **secret/受限值**：若部署现场确实需要DNS provider token、管理凭据或其他secret，只通过root-owned file、systemd credential或获批secret manager注入；不得出现在CLI、unit正文、日志、archive或Git。

当前Rutgers read-only endpoints不需要浏览器凭据。不得为了session persistence引入账号、用户token或长期cookie secret；document nonce使用进程CSPRNG。

P7 package gate执行secret scan、absolute-path scan、private inventory scan和archive allowlist。真实部署前再次扫描最终config与shell history风险。

## 8. SQLite与durability

Public只有一个service-owned operational database：`/var/lib/bcsp/rbcsp.sqlite`。同一物理库按逻辑table families承载Catalog、Open LKG/checkpoint、attempt/observation、daily aggregates与service Rutgers-day counters；public build-reachable migration graph不得包含Windows LOCAL_ONLY filters/selected/settings/Saved views/history/acknowledgements/Reset表或migration。

- 最终公网archive内数据库、WAL、SHM、seed、fixture和真实/缓存Catalog/Open数据必须全部为0；不得通过“空DB模板”绕过禁止。
- 首次启动在外部state root创建新数据库、应用public-compatible schema/migration，并在任何upstream ingest前证明它是schema-only operational DB；真实数据只可由启动后的Rutgers runtime获取产生。
- 使用事务与WAL（若目标filesystem验证支持）；`rbcsp.sqlite`、WAL与SHM统一置于`/var/lib/bcsp`，不得回落到current working directory或release tree。
- migration按版本单向记录；启动前检查可用磁盘、schema与integrity preconditions。
- Catalog target replacement、Open reconcile与checkpoint按P3事务边界原子提交。
- 进程异常退出后从committed state恢复，不重放成重复counter，不catch-up burst。
- Raw Rutgers bodies不进入产品DB或backup；只存canonical/审计字段与hash。
- Public personal state为零；备份因此也不得意外成为个人watch/history档案。

## 9. Log与monitoring

### 9.1 应用日志

`bcsp-server`输出结构化日志到journald，允许字段：timestamp、level、release version、trace ID、target key、attempt/observation ID、classification、duration、lag、circuit、计数delta与安全错误码。

禁止记录：raw payload、query全文、document nonce、cookie/header、IP、完整User-Agent、逐连接watched sections、声音设置、secret、SQL、绝对私有path。错误details必须redact。

### 9.2 边缘日志

Caddy access log如启用，应JSON化、限制访问、配置rotation/retention，并最小化query、header和cookie。生产retention在部署Review按磁盘与隐私确认；默认不把访问日志加入backup或release evidence。

### 9.3 健康与告警指标

至少监测：

- systemd active/restart loop与process资源；
- `/health/live`、`/health/ready`；
- disk/inode、SQLite/WAL增长、backup age与integrity结果；
- per-target last attempt/valid、actual interval、scheduler lag、LKG age、failure/circuit；
- service Rutgers-day attempted/succeeded/failed/empty；
- WS当前连接数与fanout错误的聚合值，不导出个人watch labels；
- Caddy证书续期与5xx。

P4不选择外部监控vendor或发送真实alert；P6/P7只需锁定最小operator可见性与无高基数个人标签。

## 10. Backup计划

### 10.1 Scope与频率

Backup至少包含：

- SQLite一致性snapshot与schema/version manifest；
- 非secret配置模板和当前release version/checksum；
- 如恢复确实需要，受单独加密/访问控制的secret/config备份；不得混入普通archive。

不备份logs、raw payload、session/watch memory、Caddy临时cache或release二进制副本。Release binary由已验证公网包重取；Caddy ACME state可按部署策略单独保护或由受控重新签发恢复。

默认operations计划：每天一次一致性backup、每次upgrade/migration前一次；至少保留最近7个daily与4个weekly。真实部署可采用更强保留，但任何更改必须记录磁盘、加密、异地与恢复责任。

### 10.2 一致性与验证

- 使用SQLite online backup API或在受控checkpoint/停服窗口生成一致性snapshot；禁止直接复制活跃DB/WAL后宣称可恢复。
- Backup写入临时文件，完成integrity check与SHA-256后原子rename。
- Manifest记录时间（UTC与America/New_York日）、release/schema、files、sizes、hash与工具版本；不含secret值。
- Backup目录root-only；若传至异地，必须加密、验证目标访问控制并在生产授权范围内执行。
- 定期在隔离staging执行真实restore drill；“文件存在”不等于backup通过。

## 11. Restore与灾难恢复

标准restore runbook：

1. 宣告maintenance并停止Caddy对新流量或返回维护页；
2. `systemctl stop bcsp`，记录当前release/schema与故障证据；
3. 对当前损坏状态做隔离副本，绝不覆盖最后已知可用backup；
4. 验证backup manifest/hash、解密权限与SQLite integrity；
5. 还原到临时目录，设置`bcsp:bcsp`与安全mode；
6. 检查backup schema与目标binary兼容，必要时使用匹配version先启动；
7. 原子替换state，启动应用loopback，验证live/ready、Catalog/Open checkpoint与today counters；
8. 恢复Caddy流量，观察lag/circuit/5xx；
9. 记录RPO/RTO实际结果与任何counter gap；不得伪造缺失attempt。

Restore不会恢复用户session、selected、watch、audio或history；新页面仍全新初始化。若备份太旧导致Open freshness过期，UI必须STALE/UNKNOWN，scheduler按正常due恢复，不burst。

## 12. Install、upgrade与rollback

### 12.1 安装preflight

真实执行前重新核验：Ubuntu exact version、`uname -m`、磁盘/内存、systemd/Caddy版本、NTP时钟、DNS、80/443可达性、TLS路径、firewall、当前服务冲突、backup目录、运行用户、域名与服务器授权。P3所记Vultr EWR主机只是目标基线，不是当前现场事实。

### 12.2 安装（未来授权后）

1. 从批准release获取公网包、SHA256SUMS/SBOM；离线验证hash与provenance；
2. 创建system user与FHS目录，但不复制、生成或下载任何数据库/seed；
3. 解压至新的versioned release，验证allowlist/permissions；
4. 写入root-owned runtime config，安装systemd unit，`daemon-reload`；
5. 先启动loopback应用，证明首次启动在`/var/lib/bcsp/rbcsp.sqlite`创建schema-only operational DB且无LOCAL_ONLY个人表，再检查migration/live/ready；
6. 写入/验证Caddy配置，再原子reload；
7. 验证HTTPS、security headers、WS、direct section URL、Open freshness/counters与browser无Rutgers直连；
8. 保存部署记录，但不把secret或inventory提交Git。

### 12.3 Upgrade

- Upgrade前必须完成verified backup与当前release/DB schema记录。
- 新包解压到新version目录，验证hash/SBOM/allowlist；不原地覆盖current binary。
- 停流/有界drain后切换symlink，运行显式migration/preflight，再启动与smoke test。
- 验证Catalog/Open scheduler没有double instance、counter重复、watch恢复或catch-up burst。
- 观察窗口内验证Caddy 5xx、readiness、SQLite/WAL、lag/circuit；成功后才清理旧release，至少保留一个可回退版本。

### 12.4 Rollback

- Binary与DB schema必须作为一组回退。若migration不可逆，不能只切旧binary；必须停服并恢复upgrade前一致性DB snapshot。
- 回退后验证release/schema、health、counters、freshness与security headers；Open数据可能STALE，不能伪造连续性。
- Rollback脚本/步骤必须幂等、有明确失败退出，并在staging演练。
- 生产rollback权限属于单独部署授权，不因本计划存在而自动获得。

## 13. Production authorization gate

P7可以在隔离Ubuntu VM构建、安装和验证公网包，但以下动作始终需要P7之后的单独用户批准：

- 使用真实SSH/云平台/Cloudflare/GitHub生产凭据；
- 修改真实服务器packages、users、filesystem、systemd、Caddy或firewall；
- 修改DNS、申请/切换真实证书、开放流量；
- 迁移/替换真实DB、启用定时backup、删除旧release；
- 发布GitHub Release或把生产域名对外宣布可用（除非另有明确授权）。

申请生产授权时必须附：P7两个包的hash、clean-machine结果、SBOM/secret scan、备份与restore drill、部署/rollback命令、预期变更diff、停机窗口、域名/DNS/服务器现状核验与明确观察窗口。

## 14. P7验收

| ID | 验收证据 |
|---|---|
| P4-OPS-001 | Ubuntu 24.04 clean VM无Node/Cargo/Git，从data-empty archive安装；首次启动创建`/var/lib/bcsp/rbcsp.sqlite` schema-only operational DB |
| P4-OPS-002 | archive allowlist/denylist、hash、SBOM、license、secret/path/data scan；DB/WAL/SHM/seed/fixture/真实Catalog/Open计数均为0 |
| P4-OPS-003 | non-root loopback bind与systemd hardening audit |
| P4-OPS-004 | Caddy validate、HTTPS test domain、WS、no-store/immutable与security headers |
| P4-OPS-005 | journald/log redaction、rotation与无个人高基数标签 |
| P4-OPS-006 | crash/restart后service counters继续，active watch为零，无catch-up |
| P4-OPS-007 | online backup、manifest/hash/integrity与隔离restore drill |
| P4-OPS-008 | forward upgrade与不可逆migration配套DB rollback演练 |
| P4-OPS-009 | damaged DB/disk full/Caddy down/upstream circuit降级与readiness |
| P4-OPS-010 | package与部署步骤分离，production mutation计数为零 |

对应P7任务为`P7-PUBLIC-RUNTIME/OPS/PACKAGE`；共享应用行为仍由`P7-SHARED-*`提供；`P7-PUBLIC-ZERO-SURFACE`证明制品中无LOCAL_ONLY能力。

## 15. Machine-readable state

```text
status=P4_PUBLIC_PACKAGE_OPERATIONS_PLAN_FROZEN
storage_amendment=P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001
storage_amendment_date=2026-07-13
target_os=UBUNTU_24_04
expected_arch=X86_64_PREFLIGHT_REQUIRED
service_binary=BCSP_SERVER
service_user=NON_ROOT_BCSP
service_manager=SYSTEMD
application_public_bind=FALSE
edge_proxy=CADDY
https=REQUIRED
backup_plan=REQUIRED
restore_plan=REQUIRED
upgrade_plan=REQUIRED
rollback_plan=REQUIRED
linux_state_root=/var/lib/bcsp
public_operational_database=/var/lib/bcsp/rbcsp.sqlite
first_start_database=SCHEMA_ONLY_OPERATIONAL
both_package_first_start_databases=SCHEMA_ONLY_OPERATIONAL
archive_database_files=0
archive_wal_shm_files=0
archive_seed_fixture_files=0
archive_real_catalog_open_data=0
local_personal_migrations_in_public=FALSE
secrets_in_package=FALSE
personal_state_in_operational_backup=FALSE
production_deployment_authorized=FALSE
production_mutations=0
p7_authorized=FALSE
deployment_is_third_package=FALSE
final_package_count=2
```
