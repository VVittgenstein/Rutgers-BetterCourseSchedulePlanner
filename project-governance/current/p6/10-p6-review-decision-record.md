# P6 Review Decision Record — 2026-07-13

## 1. Record identity

```text
record_id=P6-REVIEW-DECISION-2026-07-13-001
storage_amendment_id=P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001
record_scope=P6_REVIEW_ONLY
p7_plan_approved=TRUE
live_request_budget_approved=TRUE
p7_authorized=FALSE
real_world_network_test_authorized=FALSE
vultr_baseline_remediation_authorized=TRUE
vultr_baseline_remediation_status=COMPLETED_PASS
vultr_unattended_restart_authorized=TRUE
vultr_unattended_restart_status=COMPLETED_PASS
vultr_staging_readiness=BASELINE_HEALTHY_REQUIRES_P7_5_001_RECHECK
github_release_authorized=FALSE
vultr_staging_mutation_authorized=FALSE
production_authorized=FALSE
```

本记录规范化用户对P6 Review的逐项回复与补充建议。新增P7.5五项、32项总计划与live请求预算已获计划层批准；`fwupd`基线修复已在下述精确最小范围内获执行授权并`COMPLETED_PASS`。该完成不等于启动P7、执行P7.5真实网络测试、安装Vultr staging、发布GitHub Release或变更生产。

## 2. Decisions

| decision_id | decision | status | consequence |
|---|---|---|---|
| `P6-RD-001` | Windows本地一键包使用包根相对路径 | `ACCEPTED` | `<package-root>`由运行exe定位；禁止CWD/LocalAppData/TEMP fallback |
| `P6-RD-002` | Windows只有一个物理主库 | `ACCEPTED` | `data/rbcsp.sqlite`内operational/personal逻辑分域；WAL/SHM只是sidecar |
| `P6-RD-003` | 两个最终包不预装真实课程/Open数据 | `ACCEPTED` | archive拒绝任何DB/WAL/SHM/seed/checkpoint/真实或缓存Catalog/Open；首次运行建schema-only库 |
| `P6-RD-004` | 单一共享Rust/React架构 | `ACCEPTED` | 一个shared core、两个窄adapter/entry、零长期fork |
| `P6-RD-005` | 原27个P7 task与Git规则 | `ACCEPTED_BASE_RETAINED` | 既有task ID/语义保留；每task独立验证、record、commit和受控push |
| `P6-RD-006` | UI编写与UI打磨分离 | `ACCEPTED` | P7.2用两个指定skills；P7.3之后用一个指定skill；task/record/commit不同 |
| `P6-RD-007` | 依赖、许可证、SBOM与最终验证门 | `ACCEPTED` | exact lockfile/official metadata/advisory/license仍为P7.1-002 hard gate |
| `P6-RD-008` | 当前能力范围、非目标及严格两个包 | `ACCEPTED` | P7.5证据、Release页面和部署行为不是第三个包 |
| `P6-RD-009` | fake upstream仍用于测试 | `ACCEPTED` | 容量、失败、边界与确定性状态转换不打真实Rutgers |
| `P6-RD-010` | 必须新增独立real-world E2E subphase | `ACCEPTED_REQUIREMENT` | 新增P7.5；P7.4只冻结candidate，P7.5才持有最终P7 gate |
| `P6-RD-011` | P7.5新增5个task，总数32 | `APPROVED` | 原27项继续有效；新增五行、DAG与stop gates成为正式P7计划，但尚未授权启动P7 |
| `P6-RD-012` | 干净Windows真实候选包E2E | `ACCEPTED_REQUIREMENT` | 真实Catalog/Open/search/filter/detail/freshness/counters/WS/watch/toast及可用时audio |
| `P6-RD-013` | Linux两级测试：Actions后Vultr staging | `ACCEPTED_IN_PRINCIPLE` | 两层消费同一Linux hash；Actions不能替代Vultr主机/恢复证明 |
| `P6-RD-014` | 现有Vultr在P7期间是test/preproduction，不是production | `ACCEPTED` | 测试同一台机器不授权生产；测试后恢复/重装，生产前重新discovery |
| `P6-RD-015` | 开始前确认Vultr配置、密钥与机器状态 | `AUTHORIZED_AND_COMPLETED_READ_ONLY` | A0S完成；零mutation；发现`FWUPD_DAEMON_LIBRARY_VERSION_MISMATCH`阻塞healthy baseline |
| `P6-RD-016` | live请求预算与Actions权限边界 | `APPROVED` | 每环境`<=2N+5`、480秒、串行、15分钟间隔、一次run、人工dispatch/contents-read；预算批准不授权实际live run |
| `P6-RD-017` | Vultr snapshot/install/Caddy/systemd/DB/restart/restore | `NOT_AUTHORIZED_YET` | 仅可在P7.4后由命名实例exact diff的`VULTR_STAGING_MUTATION_AUTHORIZATION`覆盖 |
| `P6-RD-018` | 修复当前fwupd degraded基线 | `COMPLETED_PASS` | 精确allowlist内完成；current systemd=`running`、failed units=0；未扩展到snapshot/reboot/其他package/BCSP/Caddy/P7 |
| `P6-RD-019` | GitHub Release | `POST_P7_SEPARATE_AUTHORIZATION` | 仅`P7.5-005 PASS`后针对repo/tag/version/two hashes申请 |
| `P6-RD-020` | 生产Vultr、DNS、Cloudflare、证书与流量 | `POST_P7_SEPARATE_AUTHORIZATION` | staging恢复后重新A3 discovery，再申请A4 change；当前为0 |
| `P6-RD-021` | 启动P7 | `NOT_AUTHORIZED` | 机械validator PASS或本记录都不能越过人工P6 Review gate |
| `P6-RD-022` | `unattended-upgrades.service` needrestart残留 | `AUTHORIZED_AND_COMPLETED_PASS` | 安全门通过后仅重启该service；needrestart service count归零，residual cleared；仍须P7.5-001即时复核 |

## 3. Live可判定性

真实环境不能安全地强制seat状态变化。P7.5 live hard gate要求真实Catalog、真实Open join、freshness/lag/counters、真实WS、watch注册与当前真实Open section的initial-already-open状态/toast。声音在浏览器已解锁且输出端点可用时必须验证；端点不可用必须明确显示unavailable。若有界发现后没有真实Open section，结果是`LIVE_PRECONDITION_NOT_MET`并阻塞，不通过无限等待、压力、故障注入或伪造数据解决。Closed→Open、完整toast/audio状态转换仍由fake upstream确定性证明。

## 4. A0S finding summary

只读preflight确认控制台身份、私有inventory、client key、live host key与SSH authentication相互一致，实例在线、UFW/NTP正常、snapshot/restore表面存在，且尚无BCSP/Caddy/80/443或BCSP state目录。自动备份未启用。原始快照systemd=`degraded`、failed units=2：`fwupd.service`和`fwupd-refresh.service`因daemon/library版本不一致失败；`dpkg --audit`为空、无held package、无reboot-required标记，模拟仅计划安装`libfwupd3`并升级`fwupd`、移除0。上述initial state及原因作为历史保留，不再代表当前机器状态。

## 5. `fwupd`最小修复执行结果

授权只覆盖：

1. 刷新repository/package metadata；
2. 重新执行只读模拟，并确认实际计划仍严格为安装`libfwupd3`、升级`fwupd`、移除0个package；
3. 仅执行上述两个package变化；
4. 仅重启相关`fwupd`服务、reset-failed并复核package状态、failed units与systemd健康状态。

明确禁止创建snapshot、整机reboot、升级其他package、移除任何package、安装其他package或借此执行BCSP/Caddy/staging/production变更。若metadata刷新后的diff新增任何package、升级对象、移除项、reboot要求或其他副作用，必须在mutation前停止并重新请求授权。该授权不是`VULTR_STAGING_MUTATION_AUTHORIZATION`，也不授权P7或P7.5 live网络执行。

执行结果为`COMPLETED_PASS`：metadata refresh=`PASS`；刷新后重新模拟仍严格为安装`libfwupd3`、升级`fwupd`、移除0；实际安装成功；仅重启`fwupd.service`与`fwupd-refresh.service`并reset-failed。最终systemd=`running`、failed units=0、`fwupd.service` active/result success、`fwupd-refresh` result success、dpkg audit=0、held=0、reboot-required=false、fwupd pending changes=0。计数为1次bounded remediation event、3个transaction（metadata、package、services）。未创建snapshot、未整机reboot、未修改BCSP/Caddy，其他package变化、P7/live/staging/Release/production mutation均为0。

Post-remediation只读复核同时发现一个范围外残留：`needrestart -b`仅输出`unattended-upgrades.service`，service count=1；`NEEDRESTART-KSTA=1`，但`/var/run/reboot-required=false`。verbose list模式`needrestart -vrl`进一步明确该service的`unattended-upgrade-shutdown`进程使用obsolete binary `/usr/bin/python3.12`，并建议`systemctl restart unattended-upgrades.service`。记录`diagnosticMode=NEEDRESTART_VERBOSE_LIST`与`reasonCode=UNATTENDED_UPGRADES_PROCESS_USES_OBSOLETE_PYTHON_BINARY`，不保存瞬时PID。该service当前ActiveState=`active`、SubState=`running`、Result=`success`。

第一次授权并执行的package transaction仍严格只有安装`libfwupd3`与升级`fwupd`，没有Python变化；残留因果来源标记为`NOT_ASSERTED`，不作擅自归因。用户随后独立批准只重启`unattended-upgrades.service`。执行前安全门全部PASS：`apt-daily` inactive、`apt-daily-upgrade` inactive、apt/dpkg locks none、dpkg audit=0，且该service为active/running/success。仅执行`systemctl restart unattended-upgrades.service`，结果PASS。

重启后systemd=`running`、failed units=0、`unattended-upgrades.service` active/running/success、`fwupd.service` active/result success、`fwupd-refresh` result success、needrestart service count=0、dpkg audit=0、held=0、reboot-required=false、fwupd pending changes=0。restart authorized/performed=`true`、waiver=`false`、residual cleared=`true`。整体Vultr staging readiness为`BASELINE_HEALTHY_REQUIRES_P7_5_001_RECHECK`。

## 6. Supersession and next gate

本记录仅supersede此前P6中关于Windows Known Folder/双库、P7只有四个subphase、P7.4直接完成P7以及“P7绝不接触任何真实Vultr”的条款。P3/P4/P5的Catalog/Open证据、产品能力、共享语义、原27个task与Git规则继续有效。

新增P7.5五项/32总数、live预算、`fwupd`最小修复与`unattended-upgrades.service`单服务restart均已按各自授权完成。下一门是用户另行明确启动P7；P7.5-001仍必须现场复核机器健康。P7.5真实网络执行仍须在候选hash、环境与当次run确定后取得A1E授权；P7.4之后的Vultr staging mutation、GitHub Release与production继续分别受独立授权门约束。
