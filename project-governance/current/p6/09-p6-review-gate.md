# P6 修订后最终 Review Gate

## 1. 结论

P6 已按用户2026-07-13 Review回复与补充建议形成新的计划闭包，当前状态为 **`P6_REVIEW_READY`**。本次变更由`P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001`与`10-p6-review-decision-record.md`记录。

这不是 P7 批准，也不是生产部署批准：

- P3、P4、P5 的产品语义门仍为`P3_PASS`、`P4_PASS`、`P5_PASS`；只按已批准决定修订storage/package lifecycle及其hash；
- 原27个P7 task的语义与Git规则保留；新增独立P7.5五个task、总计32个及live请求预算已获计划层批准；
- P7尚未启动，产品源码、依赖、lockfile、数据库、build、package、Git远端与Release均未在P6执行；
- 用户要求的Vultr配置/密钥/机器状态检查已作为A0S只读preflight完成；独立授权的`fwupd`精确最小基线修复与随后独立授权的`unattended-upgrades.service` restart均已`COMPLETED_PASS`，当前systemd=`running`、failed units=0、needrestart service count=0；
- 只有用户另行明确批准启动P7后，才可从`P7.1-001`开始；P7.5 live run、Vultr staging mutation与生产仍有各自独立门。

## 2. P6 产物闭包

| 产物 | 当前结论 |
|---|---|
| `00-p6-charter-and-preflight.md` | P6修订范围、A0S初始只读事实、bounded remediation结果、P7.1→P7.5与停止条件一致 |
| `01-final-local-oneclick-execution-plan.md` | Windows唯一主库为exe锚定的`<package-root>/data/rbcsp.sqlite`；无fallback；首次创建；archive无DB/真实数据 |
| `02-final-public-package-and-deployment-execution-plan.md` | Linux使用archive外`/var/lib/bcsp` operational-only状态；Actions+Vultr两级真实E2E；生产另行授权 |
| `03-shared-implementation-dependency-dag.md` | 32节点、无环、`P7.1→P7.2→P7.3→P7.4→P7.5` |
| `04-p7-task-and-commit-matrix.tsv` | P7.1=15、P7.2=4、P7.3=3、P7.4=5、P7.5=5；每task独立输出/测试/record/commit/stop gate |
| `05-verification-capacity-packaging-release-plan.md` | fake upstream确定性门保留；P7.5三环境同hash、有界live Rutgers、Vultr恢复门 |
| `06-dependency-license-cleanup-risk.tsv` | 42行；29`APPROVED`、13`REJECTED`；`directories`因无消费者被拒绝；无UNKNOWN |
| `07-release-and-production-authorization-boundary.md` | A0S只读、已完成A0R、A1/A1E/A1S、Release与production权限分离 |
| `08-p6-traceability-matrix.tsv` | P3/P4/P5各9行全量闭包；相关行延伸到P7.5但仍为27条上游trace |
| `09/09a` | 机器可读`P6_REVIEW_READY`；P7.5计划/live预算已批准；P7、live run、staging mutation、Release与production均未授权 |
| `10/10a` | 用户已接受的计划、`fwupd`精确最小修复结果及仍待action-time授权的decision逐项可审计 |

## 3. 本地存储与无预装数据合同

Windows包的`<package-root>`只由正在运行的`RBCSP.exe`位置决定，不受CWD影响。唯一主数据库是`data/rbcsp.sqlite`；operational与LOCAL_ONLY personal tables逻辑分域、物理共存。包根不可写时必须在任何Rutgers请求前失败，不能回退到LocalAppData/TEMP。Reset只清personal table allowlist；升级与回滚备份完整主库；删除整个解压目录会删除数据，文档必须先提示备份。

两个release archive均不得含任何DB/WAL/SHM、seed、checkpoint、Catalog/Open snapshot或真实课程数据。Windows首次运行在包内相对`data/`建schema-only库；Linux首次service start在`/var/lib/bcsp`建operational-only库。阻断网络时课程/Open/observation必须为0。

## 4. P7任务、UI与Git边界

原27个task、Git规则及UI顺序保持用户已接受的语义：

- P7.2四个task使用`$industrial-brutalist-ui`与`$design-taste-frontend`完成UI编写、集成和视觉验证；
- P7.3另有三个task，只在P7.2通过后使用`$emil-design-eng`做独立`Before | After | Why`审计、打磨和重验证；
- 二者task、record和commit均不同；
- 每个实质task只有在依赖与mandatory tests通过后，才能形成自己的record和commit，并只push到本Review批准的P7分支；
- P7.4只构建、审计并冻结恰好两个candidate，不再宣告P7完成或Release资格。

新增P7.5五个task使总数变为32：

1. `P7.5-001`：live权限、candidate hash、预算、环境与Vultr即时preflight；
2. `P7.5-002`：干净Windows真实Rutgers候选包E2E；
3. `P7.5-003`：人工dispatch GitHub Actions Linux真实世界层；
4. `P7.5-004`：命名Vultr staging真实Caddy/HTTPS E2E与恢复/重装；
5. `P7.5-005`：三环境同hash汇总、唯一最终P7 completion record。

P7.5发现产品缺陷时不得现场修包；必须回到最早owner task，重新通过受影响的P7.1–P7.3门、由P7.4构建两个新hash，并从头重跑三个P7.5环境。

## 5. 严格两个包

整个P7最多且只能产生：

1. `WINDOWS_LOCAL_RELEASE_ARCHIVE`；
2. `LINUX_PUBLIC_DEPLOYMENT_PACKAGE`。

P7.5只消费这两个包；截图、ledger、SBOM、checksum、manifest、notices、runbook、GitHub Release页面与Vultr恢复记录都不是第三个产品包。P7.5前后candidate bytes与SHA-256必须不变。

## 6. Fake upstream、真实E2E与可判定性

Fake upstream继续承担容量、timeout/429/5xx/malformed、unsafe empty、Catalog race、Closed→Open、toast/audio完整状态转换等确定性hard gate。真实E2E承担实际候选包、实际Rutgers Catalog/Open、实际浏览器WS/watch与实际systemd/Caddy边界。

每环境live请求上限为`2N+5`（`N`为当次discovery得到的有效campus数），环境窗口最长480秒，三环境串行且间隔至少15分钟；禁止自动retry、matrix、cache bust、压力/故障测试和手动refresh。当前真实Open section存在时必须验证watch、toast及可用条件下的声音链路；若有界发现后没有真实Open section，记录`LIVE_PRECONDITION_NOT_MET`并停止，不无限等待或伪造。此时P7仍不能完成，重跑需新授权与追加预算。

上述P7.5结构与预算已获计划层批准；它只冻结未来A1E授权可采用的上限与stop rules，不授权当前发出Rutgers请求或执行任何P7.5 live run。

## 7. Vultr A0S只读Preflight结果

控制台、私有inventory、本地client key、live host key和SSH batch authentication相互匹配；私钥无宽泛read ACL。实例在线，Ubuntu 24.04 x86_64、1 vCPU、约1 GiB RAM、约25 GB NVMe，UFW与NTP正常；当前只有SSH监听，未安装/运行BCSP或Caddy，也没有`/opt/bcsp`、`/var/lib/bcsp`。控制台具备snapshot/restore/reinstall表面，automatic backups当前未启用。

原始A0S快照的阻塞项为：systemd=`degraded`，`fwupd.service`与`fwupd-refresh.service`失败，原因是daemon 1.9.33与libfwupd 1.9.34不一致。该历史事实保留为initial state；当时只读检查还确认`dpkg --audit`为空、无held package、无reboot-required标记，且模拟只计划安装`libfwupd3`、升级`fwupd`、移除0。

用户独立批准的精确最小修复已按边界完成：metadata refresh=`PASS`；刷新后模拟仍严格为安装`libfwupd3`、升级`fwupd`、移除0；实际安装成功；只重启`fwupd.service`与`fwupd-refresh.service`并reset-failed。最终systemd=`running`、failed units=0、`fwupd.service` active/result success、`fwupd-refresh` result success、dpkg audit=0、held=0、reboot-required=false、fwupd pending changes=0。未创建snapshot、未整机reboot、未改BCSP/Caddy，未执行P7/live/staging/Release/production。该结果计为1次bounded remediation event、3个transaction；P7.5仍须在`P7.5-001`即时复核。

补充只读health evidence发现，`needrestart -b`仍仅输出`NEEDRESTART-SVC: unattended-upgrades.service`与`NEEDRESTART-KSTA: 1`；verbose list诊断`needrestart -vrl`明确显示该service的`unattended-upgrade-shutdown`进程使用obsolete `/usr/bin/python3.12` binary，并建议`systemctl restart unattended-upgrades.service`。记录`diagnosticMode=NEEDRESTART_VERBOSE_LIST`与`reasonCode=UNATTENDED_UPGRADES_PROCESS_USES_OBSOLETE_PYTHON_BINARY`，不记录瞬时PID。`/var/run/reboot-required`不存在，故whole-machine reboot required=`false`；该service本身ActiveState=`active`、SubState=`running`、Result=`success`。

第一次获批package transaction仍严格只有安装`libfwupd3`与升级`fwupd`，未变更Python；因此不对该残留的因果来源作未经证实的归因。用户随后独立批准只重启`unattended-upgrades.service`。执行前安全门为PASS：`apt-daily`与`apt-daily-upgrade`均inactive、apt/dpkg locks none、dpkg audit=0、该service active/running/success。唯一执行动作为`systemctl restart unattended-upgrades.service`，结果PASS。

重启后systemd=`running`、failed units=0、`unattended-upgrades.service` active/running/success、`fwupd.service` active/result success、`fwupd-refresh` result success、needrestart service count=0、dpkg audit=0、held=0、reboot-required=false、fwupd pending changes=0。restart authorized/performed=`true`、waiver=`false`、residual cleared=`true`。Staging readiness现为`BASELINE_HEALTHY_REQUIRES_P7_5_001_RECHECK`；这不取消`P7.5-001`现场即时复核。

## 8. Release与生产边界

GitHub Release只能在`P7.5-005=P7_REAL_WORLD_E2E_PASS`后，针对具体repository/tag/version/source commit/两个asset hash另行授权。Vultr P7.5测试机始终是staging/non-production；创建snapshot、安装、修改systemd/Caddy/DB、restart以及restore/reinstall必须由独立`VULTR_STAGING_MUTATION_AUTHORIZATION`精确覆盖。

即使P7.5和Release通过，也不能把staging安装直接提升为生产。测试后先恢复或重装；随后重新取得A3 production discovery，再按届时真实DNS、Cloudflare、证书、备份、费用与机器状态申请A4 production change。使用同一台机器测试不等于批准生产部署。

## 9. 已批准项目与剩余授权门

既有27个task、Git规则、单一共享架构、UI拆分、依赖/license/final gate、能力非目标与严格两个包继续有效。用户本次已明确批准：

1. 新增五个P7.5 task及总数`32`，包括真实Open缺失时fail-closed；
2. 第05文档的live请求预算、Actions人工dispatch与最小权限边界；
3. 第7节所列`fwupd`精确最小修复范围。

仍未授权且必须后续独立批准的是：

1. 明确启动P7；
2. 在候选hash、环境与当次run确定后执行P7.5真实网络测试；
3. 在P7.4冻结candidate后，对命名Vultr实例执行snapshot/install/test/restore或reinstall；
4. P7结束后的GitHub Release与production变更。

## 10. 当前副作用与状态

P6修订没有产品源码/依赖/数据库/build/package/Git/Release/production mutation，也没有Rutgers请求。外部活动只有authenticated只读Vultr preflight、精确获批并通过的bounded `fwupd` remediation，以及随后单独获批并通过的`unattended-upgrades.service` restart；公开记录不保存地址、UUID、用户名、fingerprint、瞬时PID或key material。

```text
p3_gate=P3_PASS
p4_gate=P4_PASS
p5_gate=P5_PASS
p6_eligible=TRUE
p6_gate=P6_REVIEW_READY
p7_plan_approved=TRUE
p7_5_plan_approved=TRUE
live_request_budget_approved=TRUE
p7_status=NOT_STARTED_AWAITING_USER_APPROVAL
p7_authorized=FALSE
real_world_network_test_authorized=FALSE
vultr_baseline_remediation_authorized=TRUE
vultr_baseline_remediation_status=COMPLETED_PASS
vultr_initial_system_state=DEGRADED
vultr_initial_failed_units=2
vultr_initial_blocking_reason=FWUPD_DAEMON_LIBRARY_VERSION_MISMATCH
vultr_current_system_state=RUNNING
vultr_current_failed_units=0
vultr_residual_needrestart_initial_service_count=1
vultr_residual_needrestart_service=unattended-upgrades.service
vultr_residual_needrestart_kernel_status=1
vultr_residual_needrestart_diagnostic_mode=NEEDRESTART_VERBOSE_LIST
vultr_residual_needrestart_reason_code=UNATTENDED_UPGRADES_PROCESS_USES_OBSOLETE_PYTHON_BINARY
vultr_residual_needrestart_obsolete_binary=/usr/bin/python3.12
vultr_residual_causal_attribution=NOT_ASSERTED
vultr_unattended_restart_safety_gate=PASS
vultr_unattended_restart_authorized=TRUE
vultr_unattended_restart_performed=TRUE
vultr_unattended_restart_waiver=FALSE
vultr_residual_needrestart_cleared=TRUE
vultr_current_needrestart_service_count=0
vultr_python_changed_by_bounded_mutations=FALSE
vultr_whole_machine_reboot_required=FALSE
vultr_staging_readiness=BASELINE_HEALTHY_REQUIRES_P7_5_001_RECHECK
vultr_staging_mutation_authorized=FALSE
github_release_authorized=FALSE
production_status=NOT_AUTHORIZED
production_authorized=FALSE
task_count=32
p7_5_task_count=5
exact_package_count=2
rutgers_requests=0
external_read_only_preflight_performed=TRUE
product_source_mutations=0
dependency_mutations=0
database_mutations=0
product_builds=0
package_builds=0
release_publications=0
vultr_remediation_events=2
vultr_remediation_transactions=4
vultr_mutations=2
production_mutations=0
git_mutations=0
```
