# P6 合并执行计划章程与 Preflight

## 1. 阶段结论

- **阶段**：`P6`
- **状态**：`P6_EXECUTION_PLAN_AMENDED_FOR_REVIEW`
- **输入门**：P3 `P3_PASS`；P4 `P4_PASS`；P5 `P5_PASS`
- **本阶段性质**：把已冻结的本地完整计划、公网 delta 与单一共享基线合并为可直接执行、可验证、可回滚的 P7 计划
- **阶段停止点**：`P6 Review`
- **P7**：`NOT STARTED / NOT AUTHORIZED`
- **GitHub Release**：`NOT AUTHORIZED`
- **真实生产部署**：`NOT AUTHORIZED`

P6 只形成计划与停止门，不实现 Rust/React/CSS/SQL，不选择或升级产品依赖，不运行 product build，不生成包，不发送 Rutgers 请求，不修改数据库、Git 远端、Release、域名、DNS、Cloudflare、Caddy、证书或生产配置。按用户2026-07-13明确要求，P6 Review期间先完成一次去敏authenticated只读Vultr test-readiness preflight；随后在两次独立精确授权下完成bounded `fwupd` baseline remediation和单一`unattended-upgrades.service` restart。总计恰好2个bounded Vultr mutation events、4个transactions（metadata、package、fwupd services、unattended service），未扩展为P7、staging install或production变更。只有用户另行明确批准启动P7，当前主线才可进入`P7.1-001`。

## 2. 权威输入与固定 SHA-256

本阶段只消费当前主线 P2–P5 与主工作流。下列直接设计输入按文件原始字节固定；任一 hash 漂移、任一 gate 不再为 PASS，P6 立即停止，不得通过重算或改写上游合同继续。

### 2.1 P3：本地完整计划与共享 Open 合同

| 输入 | SHA-256 |
|---|---|
| `p3/07-local-oneclick-implementation-plan.md` | `26B6805284E50639F91FD6523365FD8C0A9607353A0EF6B99D20B6E8910AA634` |
| `p3/09-p3-validation-and-freeze-gate.md` | `7CCB3CB067781961A17E7910D150325CC34B9390CEFEAE18E4F25A4E48CD9851` |
| `p3/23a-shared-open-final-contract.md` | `73031DD718A346D659214D6BE4C571D505836098DE42879E8861349CBD73887E` |
| `p3/23b-shared-open-final-contract.json` | `9BFE6712E959C39B6719E7A47F7F96E33AAC5BD20A8CFEF51320C0E2A0C77804` |

### 2.2 P4：公网 delta、运行时与运维合同

| 输入 | SHA-256 |
|---|---|
| `p4/00-p4-charter-and-preflight.md` | `B4E178CF1EBAE078250CEA1DAFA480169BD0A309BF72DFDFC75A2AD2B6A93647` |
| `p4/02-public-runtime-session-capability-contract.md` | `EAA63A7E4A58AB8C34BAE22215B3E1088ACABCD0065D8508B8B5607DF97DCB1B` |
| `p4/03-public-refresh-capacity-degradation-plan.md` | `9E63493058F53ECF0009B66E59714EE1C9E28DFCEAA7C3FF5CB4899185425DCD` |
| `p4/04-linux-package-and-operations-plan.md` | `71744604BA7C4CC7EC193A2CB8429BF7494412B9ED1613BA04E9C4AF2816CB90` |
| `p4/07-p4-validation-gate.md` | `3DF1C43EB15CDBDA4970A4D61E2B48AA474C4EC31E875001923B36D6B18BE650` |
| `p4/07a-p4-validation-gate.json` | `A879B1988A2121344585C8B1D83992FDF5ADE805A0C3816722055A77D8EFEADD` |

### 2.3 P5：单一共享基线、两个 adapter 与零泄漏合同

| 输入 | SHA-256 |
|---|---|
| `p5/00-p5-charter-and-preflight.md` | `93137019459770307398C786841CD12F14C14D1116C732BA265C2A9EF98CE867` |
| `p5/01-shared-local-public-capability-matrix.tsv` | `4CDCC23CA05E87B19EF5776750CF8FAF995E8C894B595900875851B535F00509` |
| `p5/02-adapter-entrypoint-config-boundaries.md` | `87865721D81DD2D71DE319E92322302B755E2D9154693486EC89EC289B2DC21C` |
| `p5/03-ui-capability-and-build-exclusion-contract.md` | `3AB98A0EC8D9B89B250548EBD8DB724DA819CAD5AF42ADE4BA6EF12CD9B7F4FE` |
| `p5/04-test-reuse-and-variant-matrix.tsv` | `FB09CBA8F17AAE45DF9303ED7B7EFC202044B1B356C2DBD96A706658D0CEE3DD` |
| `p5/05-conflict-and-leakage-ledger.tsv` | `21E61F4CD7317C2873D7C19BDFDEF262561130358CB06818B16CA697F8CDF2D6` |
| `p5/06-p5-traceability-matrix.tsv` | `5128DE0B7E0F4C25F5A4CCCAAF011E7C4C3E7C3E2143B88FFD90BA3A7938C991` |
| `p5/07-p5-validation-gate.md` | `BB5814D75011D9D6897F0540995CAF432B582A469F6212002264D384FA325985` |
| `p5/07a-p5-validation-gate.json` | `E5669F65601D68F2BD9B500D1888A922F2D875CE874DDCE7C8CE8C7C664D43CA` |

P5 的冻结计数是 76 项唯一能力归属（46 `SHARED`、9 `LOCAL_ONLY`、19 `PUBLIC_ONLY`、2 `EXCLUDED`）、106 项测试、12 项全部已解决的冲突与 194 行 trace；共享业务实现目标为 1，长期 fork 目标为 0。

## 3. 两个包与一次独立部署

P7.4 只能构建并冻结：

1. **本地一键包**：Windows local release archive；
2. **公网包**：Linux public deployment package。

公网包中的 systemd/Caddy 模板、运维脚本、SBOM、license notices 和部署说明是同一个 Linux 公网包的内容，不构成第三个包。P7.5以不变hash在干净Windows、GitHub Actions Ubuntu和获批Vultr staging验证这两个candidate，只产出证据，不构建第三个包。将公网包安装为生产服务是P7.5之后的独立变更活动，也不构成第三个包。

## 4. 不可重写的不变量

P6 与未来 P7 必须保留：

- 唯一共享 domain、Catalog、FilterSchema、query、Open、watch/episode、HTTP/WS contract 与共享 React product shell；
- 两个薄 composition root：Windows local entry 与 Linux public entry；adapter 只能注入平台、配置、状态所有权和运维差异；
- `SectionKey=(term,campus,index)`、`OpenBatchKey=(term,campus)`，不跨 campus union；
- Rutgers Catalog/Open 共用真实 origin concurrency `1`、per-target single-flight、EDF 无饥饿、无 catch-up burst；
- public Catalog 固定 600 秒，普通 Open 固定 30 秒，active-watch batch 目标 10 秒；local Catalog 默认 600 秒且 1–1440 分钟，Open 默认 30 秒且 3–3600 秒，active-watch batch 目标 10 秒；
- public 每个 top-level document 是全新 ephemeral session、Saved views 零表面；local 才有持久 prefs/history/Saved views/reset；
- Windows local唯一主库是以运行exe定位的`<package-root>/data/rbcsp.sqlite`，不依赖CWD且无fallback；Linux public状态位于archive外`/var/lib/bcsp`；两个release archive均不预装DB或真实Catalog/Open数据；
- `en-US` 与 `zh-CN` 完整 parity；
- 真实状态变化到通知不承诺严格 30 秒；只把 valid Open observation 到 server fanout 冻结为 `<=1s` 工程目标，并显式显示 actual cadence、lag、stale 与 circuit。

## 5. P6 产物与消费关系

| P6产物 | 唯一责任 | 未来消费者 |
|---|---|---|
| `01-final-local-oneclick-execution-plan.md` | Windows 本地包逐步实现、候选与真实E2E | P7.1、P7.2、P7.3、P7.4、P7.5 |
| `02-final-public-package-and-deployment-execution-plan.md` | Linux 公网包、两级Linux E2E与未来独立部署 runbook | P7.1、P7.2、P7.3、P7.4、P7.5、生产部署 Review |
| `03-shared-implementation-dependency-dag.md` | 无环实现依赖与 P7.1→P7.5 硬序 | 所有 P7 task |
| `05-verification-capacity-packaging-release-plan.md` | fake-upstream、容量、双包、real-world E2E与 release 验证门 | P7.4、P7.5、Release Review |
| `07-release-and-production-authorization-boundary.md` | P7、GitHub Release 与生产变更授权边界 | P6 Review、Release/部署 Review |

这些文档描述执行顺序，但自身不授权执行。

## 6. P7 总体硬序

```text
P7.1 功能实现
  -> P7.2 正式 UI 设计与实现
  -> P7.2 集成并完成视觉验证
  -> P7.3 独立 UI 审计与打磨
  -> P7.3 重新集成并完成视觉验证
  -> P7.4 集成、容量、deterministic clean-machine与候选双包冻结
  -> P7.5 独立 real-world E2E：clean Windows -> GitHub Actions Linux -> Vultr staging + restore
  -> P7 完成
  -> 可选 GitHub Release Review
  -> 独立生产部署 Review
```

P7.2 必须同时使用 `$industrial-brutalist-ui` 与 `$design-taste-frontend`。P7.3 只有在 P7.2 已实现、集成并视觉验证后才可使用 `$emil-design-eng`，且必须是不同 task、不同完成记录、不同 commit；不得并行、合并或倒序。

## 7. Preflight 与停止条件

进入 P7 前必须同时满足：

1. P6 Review 获得用户明确批准；
2. 本节全部固定 hash 重算一致，P3/P4/P5 仍为 PASS；
3. worktree 与待建分支的既有用户修改已记录并保护；
4. P7 task、验证 ID、commit 边界与回滚点已一一对应；
5. 不需要修改 P3 Open、P4 public 或 P5 adapter/zero-surface 语义；
6. P7 的依赖下载、构建和 fake-upstream 权限只在获批范围内，不包含 Rutgers 压测；
7. secrets、真实基础设施 inventory 与生产凭据不进入源码、日志、测试 fixture、commit 或制品。
8. 进入`P7.5-001`前Vultr仍须即时复核；原始`fwupd`阻塞已修复，随后发现的`unattended-upgrades.service` obsolete-binary needrestart残留也已在第二次独立授权下仅通过service restart清除。当前systemd=`running`、failed units=0、needrestart service count=0，staging readiness为`BASELINE_HEALTHY_REQUIRES_P7_5_001_RECHECK`；历史诊断不记录瞬时PID，且不对残留因果来源作未经证实的断言；
9. P7.5真实请求预算、GitHub Actions人工触发边界与命名Vultr staging exact mutation allowlist均已获得相应授权。

任一固定输入漂移、出现第二套共享业务实现、需要第三个包、需要在 P7.2 前使用 P7.3 skill、需要真实生产/DNS/Cloudflare/ACME才能完成P7.5，或 evidence 与冻结合同冲突时，立即停止并回到 Review。

## 8. P6 有界副作用记录

```text
phase=P6
status=P6_EXECUTION_PLAN_AMENDED_FOR_REVIEW
p3_input_gate=P3_PASS
p4_input_gate=P4_PASS
p5_input_gate=P5_PASS
final_package_count=2
deployment_is_third_package=FALSE
shared_business_logic_implementation_count=1
long_lived_fork_count=0
rutgers_requests=0
external_read_only_preflight_performed=TRUE
vultr_initial_system_state=DEGRADED
vultr_initial_failed_units=2
vultr_initial_blocking_reason=FWUPD_DAEMON_LIBRARY_VERSION_MISMATCH
vultr_baseline_remediation_authorized=TRUE
vultr_baseline_remediation_status=COMPLETED_PASS
vultr_current_system_state=RUNNING
vultr_current_failed_units=0
vultr_residual_needrestart_initial_service_count=1
vultr_residual_needrestart_service=unattended-upgrades.service
vultr_residual_needrestart_diagnostic_mode=NEEDRESTART_VERBOSE_LIST
vultr_residual_needrestart_reason_code=UNATTENDED_UPGRADES_PROCESS_USES_OBSOLETE_PYTHON_BINARY
vultr_residual_needrestart_obsolete_binary=/usr/bin/python3.12
vultr_residual_needrestart_kernel_status=1
vultr_residual_causal_attribution=NOT_ASSERTED
vultr_unattended_restart_safety_apt_daily=INACTIVE
vultr_unattended_restart_safety_apt_daily_upgrade=INACTIVE
vultr_unattended_restart_safety_package_locks=NONE
vultr_unattended_restart_safety_dpkg_audit_lines=0
vultr_unattended_restart_before_state=ACTIVE_RUNNING_SUCCESS
vultr_unattended_restart_authorized=TRUE
vultr_unattended_restart_performed=TRUE
vultr_unattended_restart_waiver=FALSE
vultr_residual_needrestart_cleared=TRUE
vultr_current_needrestart_service_count=0
vultr_current_unattended_upgrades_state=ACTIVE_RUNNING_SUCCESS
vultr_python_changed_by_bounded_mutations=FALSE
vultr_whole_machine_reboot_required=FALSE
vultr_staging_readiness=BASELINE_HEALTHY_REQUIRES_P7_5_001_RECHECK
vultr_remediation_events=2
vultr_remediation_transactions=4
product_source_mutations=0
dependency_mutations=0
database_mutations=0
product_builds=0
package_builds=0
release_publications=0
production_mutations=0
vultr_mutations=2
git_mutations=0
p7_authorized=FALSE
real_world_network_test_authorized=FALSE
vultr_staging_mutation_authorized=FALSE
github_release_authorized=FALSE
production_deployment_authorized=FALSE
next_gate=P6_REVIEW
```
