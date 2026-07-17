# P6 Release 与真实生产授权边界

## 1. 当前停止状态

- **状态**：`P6_AUTHORIZATION_BOUNDARY_AMENDED_FOR_REVIEW`
- **当前允许**：完成P6文档与validator；已完成一次去敏只读Vultr preflight、一次独立授权的bounded `fwupd` baseline remediation及一次独立授权的`unattended-upgrades.service` restart；停在P6 Review
- **P7产品实现**：`NOT AUTHORIZED`
- **Git commit/push for P7**：`NOT AUTHORIZED`
- **package build**：`NOT AUTHORIZED`
- **GitHub Release**：`NOT AUTHORIZED`
- **真实生产部署**：`NOT AUTHORIZED`
- **Vultr只读test-readiness discovery**：`COMPLETED_INITIAL_BASELINE_BLOCKER_FOUND`
- **Vultr bounded `fwupd` baseline remediation**：`COMPLETED_PASS`
- **Vultr `unattended-upgrades.service` residual restart**：`AUTHORIZED_AND_COMPLETED_PASS`
- **Vultr current staging readiness**：`BASELINE_HEALTHY_REQUIRES_P7_5_001_RECHECK`
- **除已完成`fwupd` remediation与已完成`unattended-upgrades.service` restart外的Vultr snapshot/install/package/firewall/systemd/Caddy/DB/restart/restore等变更**：`NOT AUTHORIZED`
- **域名/DNS/Cloudflare/证书/生产流量变更**：`NOT AUTHORIZED`

已有“依序执行P3–P6并停在P6 Review”的授权不延伸到P7、Release或生产。P6文档中的命令、task、runbook和回滚步骤都是未来合同，不构成当前执行许可。

## 2. 授权分层

| 层级 | 需要的明确批准 | 批准后允许 | 仍然禁止 |
|---|---|---|---|
| `A0 P6 REVIEW` | 用户批准本次修订后的P6最终计划 | 进入约定P7分支，按DAG实现、测试、形成task记录和commit/push | GitHub Release、Vultr mutation、生产/域名/DNS/Cloudflare变更 |
| `A0S VULTR TEST READINESS READ-ONLY` | 用户指定现有实例并要求开始前确认配置、密钥与状态 | authenticated只读控制台/SSH discovery、去敏结论与阻塞项 | snapshot、upgrade、reboot、clear-failed、install、restart或任意配置写入；本层已执行一次但P7.5-001仍须即时复核 |
| `A0R BOUNDED VULTR BASELINE REMEDIATION` | 用户针对A0S发现的`fwupd`版本不一致批准精确最小diff | metadata刷新与重新模拟；仅安装`libfwupd3`、升级`fwupd`、移除0；仅恢复相关服务并健康复核；本层已`COMPLETED_PASS` | snapshot、整机reboot、其他package变化、BCSP/Caddy/staging/production变更；diff扩大必须事前停止 |
| `A0N BOUNDED NEEDRESTART SERVICE RECOVERY` | 用户在verbose诊断与安全门明确后，仅批准重启`unattended-upgrades.service` | 验证timers inactive、locks none、dpkg clean与service健康后，执行一次service restart并复核；本层已`COMPLETED_PASS` | package/Python变更、其他service restart、snapshot、整机reboot、BCSP/Caddy/P7/staging/production变更 |
| `A1 P7.1-P7.4 IMPLEMENTATION` | A0 + hash/worktree/preflight一致 | 源码、依赖、fake-upstream、build、deterministic clean Windows/Linux验证；冻结恰好两个candidate | 真实Rutgers run、Vultr mutation、生产部署、真实DNS/Cloudflare/证书 |
| `A1E P7.5 REAL-WORLD` | A1候选门 + 用户批准真实Rutgers endpoint/budget/环境 | 干净Windows与人工dispatch GitHub Actions有界live run；指定Vultr的只读即时复核 | Rutgers压力/故障测试、自动重跑、任何未列Vultr写入、生产流量 |
| `A1S P7.5 VULTR STAGING CHANGE` | A1E + 命名实例、exact diff、费用、restore point、回滚和测试后基线的独立批准 | 只在指定staging实例创建恢复点、安装同hash候选、配置测试Caddy/CA、运行E2E并恢复snapshot或按批准方案重装 | 真实DNS/Cloudflare/ACME、生产证书/流量、直接提升为生产、范围外资源 |
| `A2 GITHUB RELEASE` | `P7.5-005 P7_REAL_WORLD_E2E_PASS` + 第05文档全部Release条件 + 用户批准tag/version/assets hash | 创建指定GitHub Release并上传恰好两个批准asset | 生产安装、DNS/Cloudflare/Vultr变更；额外第三asset包（普通metadata由Release页面/两包内携带） |
| `A3 PRODUCTION REDISCOVERY` | P7.5后且staging恢复/重装完成，用户批准指定账号/资源的只读preflight | 重新核验指定Vultr实例、OS/arch/费用、域名、DNS/Cloudflare模式、端口、备份与现有服务 | 复用P7.5旧事实冒充当前状态；任意写入、install、restart、DNS切换、secret导出 |
| `A4 PRODUCTION CHANGE` | A3事实一致 + 用户批准变更diff、窗口、backup、rollback和观察阈值 | 仅在批准资源上执行指定加固/安装/migration/systemd/Caddy/DNS步骤和预先批准的回滚 | 范围外主机、账号、domain、token、额外服务或不可审计变更 |
| `A5 CLOSEOUT` | A4验证通过 | 保存去敏记录、保持backup/旧release、确认运营责任 | 将secret/inventory提交Git或Release；静默扩大日志/retention/费用 |

每层批准只授权该层；后层不得从前层自动推断。GitHub Release与生产部署彼此独立：Release可以不做，部署也只能使用已通过审计的公网包；两者都不是第三个包。

## 3. P6 Review需要用户明确批准的内容

P7启动语句必须能明确关联：

1. 认可`P7.1 -> P7.2 -> P7.3 -> P7.4 -> P7.5`硬序与32个task；
2. 认可一个共享业务实现、两个adapter/entry、零长期fork；
3. 认可最终只有Windows local archive与Linux public deployment package；
4. 认可P7.2使用`$industrial-brutalist-ui + $design-taste-frontend`，P7.3在其集成和视觉验证后独立使用`$emil-design-eng`；
5. 认可fake-upstream、3/10/30/3600、origin concurrency1、EDF/lag与`<=1s`fanout验证，以及P7.5三环境有界真实Rutgers E2E；
6. 认可每个实质task的独立验证/commit边界，以及不把0A–P6补写成P7 commit；
7. 明确P7不触碰真实生产、域名、DNS或Cloudflare；只有A1S精确批准的命名Vultr staging实例可在P7.5有限变更并必须恢复，使用同一实例不授权生产。

如果用户只对文档提出修订而未明确批准P7，则状态仍为`P7 NOT AUTHORIZED`。

## 4. P7获批后的边界

P7仅允许在workspace、获批开发分支、隔离test environment与clean VM中：

- 实现Rust/React/CSS/SQL、locked依赖与build config；
- 使用固定fixtures和本地fake upstream完成确定性故障/容量门；A1E后才可按第05文档预算向真实Rutgers做串行功能E2E，不得主动制造压力、错误或绕cache流量；
- 构建、扫描并验证两个candidate package；
- 在P7.4使用无真实域名/凭据的systemd/Caddy模板；在P7.5 Actions/Vultr staging使用真实Caddy进程与测试内部CA/hosts映射，不触碰真实DNS/Cloudflare/ACME；
- 按DAG形成task记录、实质commit，并只推到P6 Review批准的远端分支；
- 在P7.4冻结两个candidate hash、SBOM与provenance；在P7.5结束时交付三环境同hash、去敏ledger与Vultr恢复证据。

一般P7批准仍不允许：登录Cloudflare/registrar，购买/调整资源，修改任何未命名VM，申请/变更真实证书，改DNS/proxy，上传生产DB/config，发布GitHub Release，或启用生产服务。命名Vultr实例上的snapshot/install/firewall/users/packages/files/systemd/Caddy/DB/restart/restore只能由A1S的精确allowlist授权，且身份始终为测试/预发布。

## 5. GitHub Release独立门

只有第05文档的全部条件通过并获得用户对具体`repository + tag + version + source commit + 两个asset SHA-256`的批准，才允许发布。执行前重新确认：

- release页面不包含内部路径、private inventory、secret或未公开证据；
- source commit与provenance一致，worktree clean，tag指向获批commit；
- asset恰好为Windows local archive与Linux public deployment package；SBOM/notices已在各自包内，部署说明不是第三包；
- 上传后下载重算hash，与批准值一致；失败/partial release应撤为draft或删除未完成release，不留真假混杂资产。

GitHub Release不证明生产可用，也不允许自动deploy webhook、Cloudflare action、Vultr API或SSH步骤。

## 6. P7之后的生产Preflight独立门

必须先取得`A3 PRODUCTION REDISCOVERY`，才可把P7.5测试机作为未来生产候选重新核验。申请时应列出：

- Vultr账号与具体instance ID/region（不在文档中记录secret）；
- 预期OS/arch、资源、费用与现有服务；
- 具体domain/subdomain、registrar/DNS authority与Cloudflare proxy/TLS策略；
- 计划访问的端口、systemd/Caddy/DB/backup位置；
- 只读命令/API与会产生的审计日志。

公开DNS查询不能替代账号内真实状态；历史inventory也不能冒充当前事实。发现实例、费用、OS、domain ownership、现有服务、备份或凭据与计划冲突时停止，不进入A4。

## 7. Production Change独立门

申请`A4`必须附：

1. P7最终Linux包、SBOM、provenance与SHA-256；
2. clean Ubuntu、backup/restore、upgrade/rollback和容量结果；
3. 现场A3事实与预期最终拓扑；
4. 逐命令/逐文件/逐DNS记录的变更diff；
5. maintenance与观察窗口、负责人、停止阈值；
6. verified pre-change backup和restore point；
7. binary+DB schema+Caddy/DNS的一致回滚方案；
8. secret注入方式与日志/shell-history保护；
9. 明确批准的自动/人工回滚权限。

执行只限批准diff。出现未列出的现有服务、backup失败、migration不可恢复、hash不符、health/readiness失败、5xx/lag/circuit超过阈值、DNS/TLS事实冲突或需要扩大权限时立即停止；只有已预先批准的安全回滚可以执行，其他新动作需再次授权。

## 8. 域名、DNS、Cloudflare与Vultr边界

除已完成且留痕的A0R baseline remediation、A0N single-service restart与A1S未来可能精确批准的staging mutation外，在P7.5完成并获得A3/A4之前，以下生产计数必须为0：

- domain购买、续费、nameserver或registrar变更；
- DNS A/AAAA/CNAME/TXT/CAA记录创建、修改、删除；
- Cloudflare zone、proxy、SSL/TLS、WAF、cache、API token或Origin设置变更；
- Vultr instance规格、network、SSH key或billing变更；
- 任何未列入A1S allowlist的snapshot、firewall、user、package、Caddy/systemd/DB/backup/install/restart/restore；
- ACME issuance、production certificate或流量切换。

P4/P6中的示例domain、Caddyfile、env和目录只能是无secret模板。真实值仅在A4现场以root-owned file、systemd credential或批准secret manager注入，不进入Git、package、日志或对话产物。

## 9. 紧急情况与回滚授权

计划中的rollback不是无限授权。生产变更前必须约定可自动触发的指标、最大观察时间、具体回滚命令与数据恢复点。只有防止正在发生的数据损坏或服务中断且已在A4中预先批准的回滚可直接执行；其后立即报告事实。扩容、换domain、提高Rutgers并发、关闭安全校验或删除backup都不是“回滚”。

## 10. 当前有界副作用与下一步

原始A0S只读快照确认控制台与SSH身份/密钥匹配，实例在线且具备snapshot/restore表面；同时发现systemd因`fwupd` daemon/library版本不一致而`degraded`、两个单元失败，自动备份未启用。用户随后精确授权A0R：metadata刷新通过；刷新后模拟仍严格为安装`libfwupd3`、升级`fwupd`、移除0；实际安装成功；仅重启`fwupd.service`与`fwupd-refresh.service`并reset-failed。最终systemd=`running`、failed units=0、`fwupd.service` active/result success、`fwupd-refresh` result success、dpkg audit=0、held=0、reboot-required=false、fwupd pending changes=0。

该事件计为1次bounded Vultr remediation、3个transaction（metadata、package、services）。没有创建snapshot、没有整机reboot、没有修改BCSP/Caddy，也没有执行P7、Rutgers live run、staging、Release或production。

补充只读health evidence显示，batch模式仍仅输出一个service残留：`unattended-upgrades.service`；verbose list模式`needrestart -vrl`进一步明确其`unattended-upgrade-shutdown`进程使用obsolete binary `/usr/bin/python3.12`，并建议`systemctl restart unattended-upgrades.service`。诊断reason code为`UNATTENDED_UPGRADES_PROCESS_USES_OBSOLETE_PYTHON_BINARY`、mode为`NEEDRESTART_VERBOSE_LIST`；不记录瞬时PID。`needrestart`同时输出kernel status `1`，但`/var/run/reboot-required`不存在，因此whole-machine reboot required=`false`。该service当前ActiveState=`active`、SubState=`running`、Result=`success`。

第一次package transaction仍严格只有安装`libfwupd3`与升级`fwupd`，没有Python package/binary变更；因此记录只陈述诊断事实，不擅自把残留因果归于该remediation。用户随后独立批准仅重启`unattended-upgrades.service`。执行前安全门全部通过：`apt-daily`与`apt-daily-upgrade`均inactive，apt/dpkg locks为none，dpkg audit=0，该service为active/running/success。唯一执行动作为`systemctl restart unattended-upgrades.service`，结果PASS。

重启后systemd=`running`、failed units=0、`unattended-upgrades.service` active/running/success、`fwupd.service` active/result success、`fwupd-refresh` result success、needrestart service count=0、dpkg audit=0、held=0、reboot-required=false、fwupd pending changes=0。waiver=`false`，residual cleared=`true`。整体staging readiness改为`BASELINE_HEALTHY_REQUIRES_P7_5_001_RECHECK`；这仍不是P7.5现场健康事实，必须在`P7.5-001`重新核验。

没有用户明确批准P7时，不创建P7实现task、分支/commit、产品依赖变更、build、Rutgers live run或新的Vultr写操作。

```text
phase=P6
status=P6_AUTHORIZATION_BOUNDARY_AMENDED_FOR_REVIEW
current_authority=P6_DOCUMENTATION_AND_COMPLETED_BOUNDED_VULTR_READINESS_RECOVERY
p7_authorized=FALSE
p7_git_commit_authorized=FALSE
p7_git_push_authorized=FALSE
package_build_authorized=FALSE
github_release_authorized=FALSE
real_world_network_test_authorized=FALSE
vultr_read_only_preflight_authorized=TRUE
vultr_read_only_preflight_completed=TRUE
vultr_initial_system_state=DEGRADED
vultr_initial_failed_units=2
vultr_initial_blocking_reason=FWUPD_DAEMON_LIBRARY_VERSION_MISMATCH
vultr_baseline_remediation_authorized=TRUE
vultr_baseline_remediation_status=COMPLETED_PASS
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
production_discovery_authorized=FALSE
production_deployment_authorized=FALSE
domain_changes_authorized=FALSE
dns_changes_authorized=FALSE
cloudflare_changes_authorized=FALSE
vultr_production_changes_authorized=FALSE
release_is_production_authorization=FALSE
deployment_is_third_package=FALSE
final_package_count=2
rutgers_requests=0
external_read_only_preflight_performed=TRUE
git_mutations=0
release_publications=0
vultr_remediation_events=2
vultr_remediation_transactions=4
vultr_mutations=2
production_mutations=0
next_required_user_gate=EXPLICIT_P7_START_APPROVAL
```
