# P6 验证、容量、双包与 Release 计划

## 1. 冻结结论

- **状态**：`P6_VERIFICATION_RELEASE_PLAN_AMENDED_FOR_REVIEW`
- **测试上游**：deterministic fake upstream用于故障/容量/状态转换；P7.5另有有界真实Rutgers E2E；禁止对Rutgers做压力/故障测试
- **关键cadence**：`3 / 10 / 30 / 3600`秒
- **真实Rutgers origin并发合同**：`1`
- **valid Open observation到server fanout工程目标**：`<=1s`，不是硬SLA
- **最终制品数**：严格`2`
- **GitHub Release**：条件满足后再Review，不由P6自动授权

本文件覆盖P7.4候选门与独立P7.5真实世界E2E，但不会把所有验证推迟到后段：每个P7.1–P7.3 task必须先通过其局部测试，P7.4从固定source revision重跑全套、构建并冻结恰好两个candidate，P7.5只消费其不变hash进行三环境真实验证。

## 2. 验证层级与失败规则

| 层级 | 运行时机 | 失败结果 |
|---|---|---|
| unit/property | 每个实现task | 当前task不可完成，不得进入依赖节点 |
| shared contract | shared core每次变更 | local/public两个entry都阻塞 |
| adapter/variant | local/public adapter变更 | 对应entry阻塞；shared语义不得用adapter patch分叉 |
| negative reachability | UI/build/package graph每次变更 | 发现错误产品表面即阻塞，不接受“运行时隐藏” |
| integration | P7.1、P7.2、P7.3阶段门 | 下一阶段不得启动 |
| fake-upstream/capacity | P7.1局部 + P7.4全量 | 不得通过提高真实origin并发或降低真实性修复 |
| artifact/deterministic clean-machine | P7.4 | candidate资格撤销，不得进入P7.5 |
| real-world Windows | P7.5-002 | 不得现场修包；撤销两个candidate并回到最早owner task |
| real-world Linux Actions | P7.5-003 | `ENVIRONMENT_BLOCKED`也阻塞；不得用Vultr结果替代 |
| real-world Vultr staging + restore | P7.5-004 | 立即停止测试并执行已批准恢复；恢复失败阻塞P7 |
| cross-environment final | P7.5-005 | 任一mandatory FAIL/BLOCKED/SKIPPED/无法解释的INCONCLUSIVE均不得完成P7或申请Release |

失败证据保留reason、seed、source revision、toolchain、环境与最小复现；不得包含raw Rutgers body、secret、private inventory或个人状态。

## 3. Deterministic fake-upstream规范

### 3.1 Harness能力

Fake upstream必须可脚本化控制：virtual monotonic/wall clock、term/campus target、response body/status/header、连接/总耗时、cache age/ETag、disconnect、429 `Retry-After`、乱序完成、Catalog content version、body/state hash与调用计数。它必须证明请求只打到本机/隔离网络，并在测试结束断言Rutgers真实origin request count为0。

固定seed与scenario manifest纳入测试证据；长cadence使用virtual time，不能用一小时sleep拖延CI，也不能通过缩短常量改变被测语义。

### 3.2 3/10/30/3600 cadence矩阵

| ID | 配置/场景 | 必须断言 |
|---|---|---|
| `CAP-003` | local general Open=`3s`，无watch | 每target按requested 3s进入EDF；single-flight；服务时间>3s时coalesce且actual/lag上升；不catch-up burst |
| `CAP-010` | local或public target有active watch | 该`(term,campus)`batch requested effective=`10s`；同batch 1或9个watch请求数相同；demote后回general due，不双飞 |
| `CAP-030L` | local默认general Open=`30s` | 无watch时30s；watch提升10s；显式restart不生成额外上游attempt |
| `CAP-030P` | public固定general Open=`30s` | 普通用户/API/UI均不能更改或立即refresh；reload/tab/query/WS/用户数不放大 |
| `CAP-3600` | local general Open=`3600s` | 失败retry delay不小于requested effective；virtual hour内不因600s backoff cap而加速；watch期间10s，demote后恢复合法3600s next due |
| `CAP-MIX` | 同时存在3/10/30/3600与Catalog600 targets | absolute EDF、同deadline才watch tie-break；每lane最终进展；没有负lag、饥饿或catch-up |

Catalog固定/可配边界另测：public固定600秒；local默认600秒、范围60–86400秒。所有边界值、非法值、配置migration与Reset恢复默认必须测试。

## 4. Scheduler、容量与lag验证

### 4.1 不变量

每个scenario同时采集：`requested_interval`、`effective_interval`、scheduled due、actual start/end、start-to-start actual interval、scheduler lag、in-flight数量、attempt/valid/state-change、queue depth、circuit/backoff。验收为：

- per-target in-flight峰值`<=1`；Catalog/Open合计fake-real-origin并发峰值`<=1`；
- multiple users/connections/watchers/sections不改变同target attempt序列；
- EDF按绝对due，无普通lane饥饿；watch只用于同deadline tie-break；
- missed tick只coalesce/skip，单次完成后没有补偿burst；
- overload时actual interval/lag如实增加，系统不删target、不跨campus union、不伪造cadence达成；
- wall-clock回拨/前跳、DST、restart不导致负lag或请求风暴。

### 4.2 容量包络

按冻结公式计算并用fake service time重放：

```text
Q_requested = N_catalog / 600 + N_watched / 10 + N_general / 30
serial_capacity ~= 1 / request_service_time
```

至少覆盖：15 general targets、9 watched+6 general、15 watched，以及local 15 targets全3秒的过载例。对service time `50ms / 1.5s / 5s / 15s timeout`分别运行virtual 24小时；记录p50/p95/p99 actual interval与lag、最大queue、lane progress、backoff/circuit。P3的约1.501秒p95只作为历史审计参考，不能当Rutgers SLA或测试通过阈值。

本阶段不预设“所有负载仍达到10/30秒”。通过条件是安全不变量、无饥饿、真实性与可观测性成立，并给出每种service time/target mix的实测支持包络。若产品需求只能靠将真实origin并发提高到大于1满足，停止回Review。

## 5. Open正确性与`<=1s`分段延迟

### 5.1 状态机用例

完整覆盖：valid nonempty、duplicate、orphan、empty+empty Catalog、unsafe empty+nonempty Catalog、unsafe zero-intersection、timeout/408/429/5xx、malformed/HTML/oversize/off-origin redirect、Catalog version race、ETag变而body不变、body变而intersection不变、UNKNOWN、LKG stale/recovery。任何failure/unsafe/race都不能mass-close或re-arm episode。

每个valid attempt验证四层基数与关联：`OpenPullAttempt -> OpenRefreshObservation -> OpenObservation -> OpenEpisode/action`。unchanged valid response仍有refresh/section observation；ETag只审计；freshness、UNCERTAIN与today counters逐项断言。

### 5.2 延迟测量

从`valid OpenObservation committed`的monotonic timestamp开始，到server把对应WS frame交给每个合格连接的send queue完成，计算fanout latency。至少运行：

- 1、100、1000个fake连接；
- 每连接1与9个selected sections；
- 1与15个target同轮变化；
- ONE_SHOT、CONTINUOUS A/C/D与大量已ack episode；
- 慢消费者、断连、重连与backpressure。

合格连接的p95与max目标均为`<=1s`；慢/阻塞连接必须被有界queue隔离或断开，不能拖慢其它连接。该测试明确排除上游`U+C`、scheduler等待和浏览器autoplay；失败时优化fanout/DB/serialization，不得宣称真实seat变化到声音有30秒硬保证。

## 6. 功能、UI、i18n与可访问性

- 22个筛选逐字段truth table；same-section/same-variant witness、TBA/unknown三值、course-centered结果、独立section搜索/detail/direct URL。
- local Saved views CRUD/duplicate/apply/dirty/incompatible/quota/CAS，三种Reset scope与跨启动history；public八层Saved零表面。
- local/public desktop和mobile主流浏览器矩阵；keyboard-only、focus order/visible focus、labels、live region、contrast、reduced motion、zoom 200%与touch target。
- `en-US`/`zh-CN`所有key parity、locale格式、系统语言检测、Rutgers raw原文不误翻译。
- P7.2视觉验证与P7.3重新视觉验证是两套独立证据；P7.3必须附`Before | After | Why`，并重跑核心功能与accessibility。

## 7. 依赖、secret、license与SBOM门

### 7.1 Dependency closure

从locked Rust与frontend dependency graph生成：直接/传递依赖、版本、source/checksum、feature、target reachability、license、known advisory、owner/用途。禁止floating git dependency、unreviewed binary download、install-time network fetch、copyleft/notice义务未闭合或来源不明的asset/font/sound。

每次依赖变更必须重新执行vulnerability/advisory与license review。无法确认license或公开分发权的依赖/asset在Release candidate中计数必须为0。

### 7.2 Secret/privacy scan

扫描tracked source、Git diff、build logs、binary strings、source maps、archive、SBOM/provenance与docs：token/key/password/cookie/真实domain/IP/SSH/Cloudflare/Vultr inventory、absolute user path、raw payload、session/watch/personally identifying logs。测试secret必须可识别、无权限且只在fixture allowlist；任何真实/疑似真实secret立即阻塞并按泄漏响应处理。

### 7.3 SBOM与provenance

每个包各有一份机器可读SBOM（例如CycloneDX或SPDX）与human-readable notices；列出binary/UI/assets直接及传递组件、版本、hash、license。Provenance至少记录source revision、dirty flag必须false、toolchain/target、lock hash、build image/VM identity、commands、`SOURCE_DATE_EPOCH`、artifact/SBOM hash和测试gate结果，不包含secret或私有inventory。

## 8. Reproducible build门

从同一公开source revision、locked toolchain/lock、固定依赖缓存内容和声明的clean builder运行两次独立build：

1. 清空target/dist与临时目录；
2. 固定locale、timezone、archive排序、permissions、line endings和`SOURCE_DATE_EPOCH`；
3. 禁止build时访问未声明网络；
4. 分别只构建`bcsp-local`与`bcsp-server`target graph；
5. strip/debug/path remap规则一致；
6. 规范化archive后比较所有成员、size、mode与SHA-256。

通过条件：同target两次规范化archive SHA-256完全一致，内部binary/assets/SBOM/notices hash也一致。若差异，必须生成diffoscope或等价差异报告并修复；仅“功能相同”不算可重复构建。Windows与Linux不同target之间当然不要求hash相同。

## 9. 双包与错误能力零表面审计

最终artifact set cardinality必须恰好为2：

1. `Windows local release archive`
2. `Linux public deployment package`

逐包解压并进行source/dependency、route/API/schema、storage、i18n、DOM/assets、binary/bundle symbols、manifest/archive allowlist扫描：

- Windows包必须没有PUBLIC_ONLY systemd/Caddy/ops/service-wide public session能力；
- Linux包必须没有LOCAL_ONLY Saved views、persistent prefs/history/reset/interval editor/Windows launcher的任何八层表面；
- 两包都没有EXCLUDED Node dev runtime、raw evidence、secret、private inventory、旧能力与运行残留；
- shared版本/protocol/schema/hash完全一致，不能各自编译出不同业务实现；
- 部署runbook/SBOM/backup说明是Linux包内容，不增加artifact cardinality。
- P7.5截图、去敏request ledger、测试摘要与Vultr恢复证据是验证记录，不是第三个package；P7.5前后两个candidate的SHA-256必须逐字节不变。

两包还必须通过统一的无预装数据证明：

1. 解压后、首次启动前不存在任何`.sqlite`、`.db`、`-wal`、`-shm`、seed、Catalog/Open snapshot或真实课程数据；
2. 阻断上游首次启动后只创建schema-only DB，Catalog/Open/observation行数为0；
3. 放行fake或获批real upstream后，所有行的provenance/`observedAt`晚于本次启动，不能来自archive；
4. Windows从不同CWD、batch与直接exe均只使用`<package-root>/data/rbcsp.sqlite`；Linux release目录只读且只写`/var/lib/bcsp`。

## 10. Clean-machine门

### 10.1 Windows

至少在一个新建标准用户账户和一台全新Windows VM分别验证：无开发工具/管理员权限、空格/Unicode路径、包外CWD、首次创建唯一主库、不可写包根pre-network fail-fast、重启持久、active不恢复、全部功能/声音/Reset、upgrade/rollback、offline/unsafe、整目录删除会删除数据的卸载说明与Defender/安全扫描。运行期间不得从网络下载runtime或依赖。

### 10.2 Linux

P7.4至少在一台全新Ubuntu 24.04 VM验证：archive无DB/真实数据、空`/var/lib/bcsp`首次建schema-only operational DB、无LOCAL_ONLY personal migrations/tables、非root service、loopback bind、systemd hardening、Caddy test HTTPS/WS、DB restart、backup/restore、upgrade/rollback、disk full/Caddy down/upstream circuit和日志隐私。只允许隔离test domain/CA；P7.4不使用真实域名、DNS、Cloudflare或Vultr。

## 11. 独立 P7.5 Real-World E2E

### 11.1 共同安全合同与请求预算

P7.5不替代fake upstream。timeout、429/5xx、malformed、unsafe empty、Catalog race、Closed→Open re-arm、toast/audio完整状态转换、容量与lag仍由deterministic fake upstream作为hard gate；live测试只证明真实候选包、真实Rutgers合同、真实浏览器与真实运维边界互通。

令一次受控discovery得到的当前有效campus数为`N`，实现不得硬编码`N=15`。每个环境的上限为：

```text
discovery attempts <= 2
Catalog first-initialization attempts <= N
Open attempts <= N + 3
total Rutgers attempts <= 2N + 5
```

若当次`N=15`，每环境最多35次，Windows + Actions + Vultr合计最多105次/candidate hash。三个环境必须严格串行，环境之间至少15分钟；每环境live window最长480秒并在第二轮600秒Catalog refresh前退出。继续强制Rutgers origin concurrency=`1`与per-target single-flight=`1`；禁止自动retry、Actions matrix、cache bust、故障注入、非法参数、压力测试和手动refresh。遇到403、429、off-origin redirect、schema anomaly或预算越界立即停止，尊重`Retry-After`且不得换机器绕过。每candidate每环境只允许一次live run；重跑需新的人工批准与追加预算。证据只保存时间、状态码、bytes、分类、hash和计数，不保存raw body。

### 11.2 干净 Windows

`P7.5-002`必须解压P7.4真实候选archive而非开发构建；核对hash和无预装数据；首次创建`./data/rbcsp.sqlite`；动态从真实Catalog选择课程；搜索并组合至少三个有结果筛选条件；打开真实Course与由`(term,campus,index)`标识的Section；将真实Open集合只与同批Catalog Section相交；核对freshness、lag、attempt/succeeded/failed/empty与ledger；建立真实浏览器WS；动态选择当前真实Open section并验证initial-already-open watch状态和toast。音频端点可用时必须以用户手势解锁并验证cue调用/输出链；不可用时必须明确显示unavailable。所有scope均无当前真实Open section时记`LIVE_PRECONDITION_NOT_MET`并停止，不等待自然变化或伪造证据。

### 11.3 GitHub Actions Linux 层

`P7.5-003`只允许人工`workflow_dispatch`、单一Ubuntu job与`contents: read`；不得push/PR/schedule自动触发live测试，不得自动retry或matrix并发，不得使用OIDC、deployment、Release、Vultr、Cloudflare或真实secret。job安装冻结Linux candidate bytes，不从源码重建；启动真实`bcsp-server`、systemd与真实Caddy进程，以受信测试CA完成HTTPS/WSS；用desktop/mobile Playwright完成与Windows等价的真实Catalog/Open/search/filter/detail/WS/watch流程，证明多个浏览器只访问BCSP/Caddy且不各自直连Rutgers、reload为新session、Saved views/history/Reset为零表面。hosted runner预装开发工具不用于证明“系统无Node/Rust”；该主机级证明由Vultr层承担。

### 11.4 Vultr staging Linux 层

`P7.5-004`除P7授权外还必须取得针对命名实例与精确diff的`VULTR_STAGING_MUTATION_AUTHORIZATION`。即时preflight必须确认实例/凭据/OS/arch/资源/费用、无未知服务、systemd健康、恢复能力和测试后目标基线；任何当前`degraded`状态（包括已发现的`fwupd`版本不一致）须先在独立授权下修复并重新核验。创建且验证恢复点后，实例身份标为`staging/non-production`，安装与Actions完全相同hash，使用真实Caddy和测试专属内部CA/客户端hosts映射完成desktop/mobile、集中poller、fresh reload与public零表面验证。不得修改真实DNS、Cloudflare、ACME、公网证书或生产流量。测试后必须恢复snapshot或按批准方案重装，验证端口、service、package、DB、user、firewall与残留回到批准非生产基线；不得把测试安装直接“提升”为生产。

### 11.5 失败回流与最终门

```text
P7.5发现产品缺陷
  -> 回到最早owner task修复
  -> 重跑受影响的P7.1/P7.2/P7.3 gate
  -> P7.4重新构建恰好两个新candidate并产生新hash
  -> Windows、Actions、Vultr三个P7.5环境全部从头重跑
```

`P7.5-005`只有在三环境同hash、全部mandatory assertion为PASS、预算合规、证据去敏、Vultr已恢复且package cardinality仍为2时，才能输出唯一`P7_REAL_WORLD_E2E_PASS`与最终P7 completion record。此记录只允许申请后续GitHub Release授权；生产仍未授权。

## 12. GitHub Release条件

GitHub Release是可选的P7后步骤。只有全部满足才可向用户申请Release授权：

1. P6 Review已批准且P7.1–P7.5最终gate全部PASS，`P7.5-005=P7_REAL_WORLD_E2E_PASS`；
2. 恰好两个final package、SHA256、SBOM、notices、provenance和release notes齐全；
3. shared/variant/negative、fake-upstream、capacity、`<=1s`fanout、UI/accessibility、clean-machine全部PASS；
4. secret/privacy、dependency advisory、license、allowlist/denylist和reproducibility全部PASS；
5. Git branch/commit适合公开，worktree clean，不含内部/私有状态；
6. 用户明确批准发布对应tag/version与两个asset hash。

Release失败或条件未满足时不影响已验证本地artifact的存在，但不得把candidate称为已发布。GitHub Release也不授权生产部署、DNS或服务器变更。

## 13. Machine-readable state

```text
phase=P6
status=P6_VERIFICATION_RELEASE_PLAN_AMENDED_FOR_REVIEW
fake_upstream_required=TRUE
real_world_e2e_required=TRUE
real_world_e2e_subphase=P7.5
real_world_environments=WINDOWS_CLEAN,GITHUB_ACTIONS_UBUNTU,VULTR_STAGING
live_total_attempt_formula_per_environment=2N+5
live_environment_window_seconds=480
live_environment_minimum_gap_minutes=15
live_environment_runs_per_candidate=1
rutgers_pressure_test=FALSE
fake_upstream_cadences_seconds=3,10,30,3600
origin_max_concurrency=1
scheduler=EDF_NO_STARVATION
missed_ticks=COALESCE_OR_SKIP_NO_CATCH_UP
lag_must_be_visible=TRUE
valid_observation_to_fanout_target_seconds=1
hard_real_change_to_notification_30s=FALSE
final_package_count=2
secret_scan_required=TRUE
license_audit_required=TRUE
sbom_per_package_required=TRUE
reproducible_build_required=TRUE
windows_clean_account_required=TRUE
windows_clean_vm_required=TRUE
linux_clean_vm_required=TRUE
candidate_hash_immutable_during_p7_5=TRUE
vultr_staging_requires_separate_mutation_authorization=TRUE
vultr_staging_must_be_restored=TRUE
github_release_conditional=TRUE
github_release_authorized=FALSE
production_deployment_authorized=FALSE
```
