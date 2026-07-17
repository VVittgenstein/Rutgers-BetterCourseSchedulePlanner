# P5 共享核心、变体边界与最终验证门

## 1. 最终结论

- 阶段：`P5`
- 记录状态：`P5_PASS`
- Gate：`P5_PASS`
- P4 输入门：`P4_PASS`
- P6 Review存储修正：`P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001`（2026-07-13）
- P6：`ELIGIBLE`
- P7：`NOT STARTED / NOT AUTHORIZED`
- Rutgers 新请求、产品源码变更、构建、打包、发布和生产变更：均为 `0`

本门确认的是实现架构与验证闭包，不是产品实现。Candidate 验证已经以 0 failures 通过，`07/07a` 已按唯一允许的提升范围进入最终 `P5_PASS`；Final 验证必须再次返回 0 failures，否则本门立即失效并回到 Review。本门使 P6 eligible，但不授权 P7、包构建、GitHub Release 或真实生产部署。

`P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001`在P6 Review中修正了P5的物理存储与archive数据边界：Windows local使用唯一package-root `data/rbcsp.sqlite`，Linux public保持`/var/lib/bcsp` operational-only，两个archive都不预装数据库或真实课程数据。该修正不改变能力分类和行数，也不构成P7授权。

## 2. P4 输入与阶段边界

P5 只消费当前主线 P2–P4 已冻结合同。P4 的最终状态必须保持 `P4_PASS`，其 76 行公网 baseline/delta、22 个筛选字段、共享 Open 合同、public session/runtime/package/operations 边界和 `PUBLIC_ZERO_SURFACE` 结论均不得在 P5 被重新解释。

P5 只完成以下设计：

1. 把每项能力唯一分类为 `SHARED / LOCAL_ONLY / PUBLIC_ONLY / EXCLUDED`；
2. 给每项能力指定语义 owner、模块边界、两个产品消费者、build edge、验收测试和 P7 任务；
3. 冻结“一个共享业务实现 + 两个薄 entrypoint/adapter”的单主线架构；
4. 证明每个共享能力由两端复用，每个变体能力在 owner 产品中被验证、在另一产品制品中为零表面；
5. 关闭语义、代码 fork、source/route/API/storage/i18n/bundle/package 泄漏与状态、Open policy、部署授权冲突；
6. 把所有分类、测试与冲突防线追踪到 P7 和 P6 handoff。

P5 不修改 Rust、TypeScript、React、CSS、SQL、依赖、lockfile、构建、package、服务器、DNS、证书、Release 或生产状态，不发出 Rutgers 请求，也不读取 chat log、旧 P1 或 deprecated 产物。

## 3. ALL：能力分类闭包

`01-shared-local-public-capability-matrix.tsv` 对 P4 的 `P4-B001..P4-B076` 一一映射，共 76 行且每行只能有一种 classification：

| classification | 行数 | 语义 |
|---|---:|---|
| `SHARED` | 46 | 两个产品必须调用同一个业务实现和版本化合同 |
| `LOCAL_ONLY` | 9 | 只由本地 adapter/entrypoint 拥有，public build 不可达 |
| `PUBLIC_ONLY` | 19 | 只由公网 adapter/entrypoint 拥有，local build 不可达 |
| `EXCLUDED` | 2 | 两个最终产品都不可达、不可打包 |
| **合计** | **76** | 与 P4 baseline/delta 完全闭合 |

其中 22 个 `FLT-C01..FLT-C10 / FLT-S01..FLT-S11`（含 `FLT-S04a` 和 `FLT-S04b`）逐行保持 `SHARED`，没有用概括性“支持筛选”替代字段级合同。

唯一架构公式为：

```text
shared domain/catalog/query/open/watch/ui/i18n
             |                    |
             v                    v
       local adapter         public adapter
             |                    |
        local entry            public entry
             |                    |
  Windows 本地一键包          Linux 公网包
```

共享业务逻辑实现目标计数为 `1`；长期 local/public 代码 fork 目标计数为 `0`。adapter 只能提供配置、状态所有权、入口、平台和运维差异，不得复制或改写共享业务语义。

## 4. 共享 Open 与筛选合同不分叉

以下全部属于共享业务核心，而不是 local/public 各一套：

- `SectionKey=(term,campus,index)`，`OpenBatchKey=(term,campus)`；不跨 campus union，不用 orphan 创造 Section；
- 五位字符串集合、同 batch Catalog intersection、duplicate/orphan audit、Catalog race 与 unsafe empty/zero-intersection 保留 LKG，绝不 mass-close；
- per-target single-flight、Catalog/Open 共用 Rutgers origin concurrency `1`、EDF 无饥饿、missed tick 不追赶、backoff/Retry-After/circuit/timeout/size limit；
- attempt、target refresh observation、section observation 和 episode/action 分层；ETag 只审计；fresh/stale/UNKNOWN 与 `FLT-S03` 的 `UNCERTAIN` 规则；
- watch 不放大上游请求，ONE_SHOT、Max audible、CONTINUOUS confirmation、Closed→Open re-arm 和浏览器音频前置条件；
- 三值代数、same-section/same-variant witness、course-centered 结果和 independent section search/detail。

变体只输入 policy：

- local：Catalog 默认 600 秒且 1–1440 分钟可配；Open 默认 30 秒且 3–3600 秒可配；active-watch batch 目标 10 秒；local run + Rutgers-day counters；
- public：Catalog 固定 600 秒；普通 Open 固定 30 秒；active-watch batch 目标 10 秒；普通用户不可配置或强制上游刷新；service-wide Rutgers-day counters；
- 两端都不承诺真实 seat 变化到通知的严格 30 秒上界；必须显示 requested/effective/actual cadence、lag、stale 与 circuit 状态。

## 5. 状态、UI、入口和制品边界

- shared：22 筛选、course/section 导航、watch、toast、ONE_SHOT、CONTINUOUS、Max audible、响应式 WebUI、`en-US/zh-CN` 完整键和 typed API facts；
- local：persistent user state、Saved views、persistent history、reset local user data、可配 refresh、local run counters、Windows entry/package；
- public：每次 top-level load 新 ephemeral session、系统/浏览器语言初始化、固定 refresh policy、service operations state、Linux Rust entry、health/readiness、systemd、Caddy/HTTPS、backup/upgrade/restore/rollback 计划；
- excluded：Node/Fastify/npm/Vite-dev 生产 runtime 与 evidence/raw/secrets/runtime residue。

storage ownership固定为：

- Windows local由已解析的`RBCSP.exe`目录锚定`data/rbcsp.sqlite`，只有一个物理数据库；operational与`LOCAL_ONLY` personal tables/migrations使用分离manifest、namespace、allowlist与schema snapshot；
- local路径与process CWD无关，解析或写入失败必须明确停止，不得回退到Known Folder、LocalAppData、临时目录或第二个数据库；
- Linux public只在`/var/lib/bcsp`创建service-owned operational state，public source/dependency/link/migration/schema/table/package均不得含personal定义；
- local Reset只清personal逻辑域，不删除物理数据库，也不清Catalog/Open/diagnostics。

独立 local/public UI entrypoint 使用正向 import allowlist。仅依赖 tree-shaking 不算隔离；P7 必须对 source、DOM/route、API、storage key/schema、i18n key、bundle symbol/chunk 和 package manifest 做负向扫描。稳定 direct section URL 属于共享导航，不得被误删为 Share links；current-page selection/watch/audio 属于共享能力，不得被误删为 persistent subscription。

最终仍只有两个包：Windows 本地一键包与 Linux 公网包。两者的archive都必须拒绝`*.sqlite`、`*.sqlite3`、`*.db`、WAL/SHM等SQLite sidecar、seed/snapshot数据库及真实Rutgers Catalog/Open数据；schema/migration可随代码交付，runtime state必须在首次运行后才创建。部署说明、SBOM 或备份不构成第三个包；真实生产部署仍需独立授权。

## 6. 测试复用与变体验证

`04-test-reuse-and-variant-matrix.tsv` 共 106 行：

| test_kind | 行数 |
|---|---:|
| `SHARED_CONTRACT` | 46 |
| `PUBLIC_VARIANT` | 19 |
| `LOCAL_VARIANT` | 9 |
| `LOCAL_NEGATIVE_BUNDLE` | 21 |
| `PUBLIC_NEGATIVE_BUNDLE` | 11 |
| **合计** | **106** |

验证乘数规则：

- 每个 `SHARED` 能力恰好一个跨两产品的共享合同测试；
- 每个 `PUBLIC_ONLY` 能力恰好一个 public variant 测试和一个 local negative-bundle 测试；
- 每个 `LOCAL_ONLY` 能力恰好一个 local variant 测试和一个 public negative-bundle 测试；
- 每个 `EXCLUDED` 能力恰好两个 negative-bundle 测试，分别证明两个制品都不存在该能力。

负向测试不是“UI 不显示”单点断言，而是验证错误产品的 source/build edge、route、API、storage、i18n、bundle 和 package 均不可达或不存在。

存储与package变体测试还必须证明：多种CWD启动只命中同一exe-root `data/rbcsp.sqlite`；local仅有一个物理库且两个逻辑域不越权；public不链接personal migrations/tables；两个archive均无DB/WAL/SHM/seed/真实Catalog/Open payload，首次运行再创建状态。

## 7. ONLY：冲突与泄漏闭包

`05-conflict-and-leakage-ledger.tsv` 共 12 行，覆盖：

1. `SEMANTIC_CONFLICT`
2. `CODE_FORK`
3. `SOURCE_LEAKAGE`
4. `ROUTE_LEAKAGE`
5. `API_LEAKAGE`
6. `STORAGE_LEAKAGE`
7. `I18N_LEAKAGE`
8. `BUNDLE_LEAKAGE`
9. `PACKAGE_LEAKAGE`
10. `STATE_OWNERSHIP_CONFLICT`
11. `OPEN_POLICY_CONFLICT`
12. `DEPLOYMENT_AUTHORITY_CONFLICT`

全部状态为 `RESOLVED`，未解决项为 `0`。任一项重新打开、任一 shared capability 出现第二份业务实现、任一 long-lived fork、任一变体能力进入错误制品，P5 必须失败并回到 Review。

## 8. 追踪闭包

`06-p5-traceability-matrix.tsv` 共 194 行：

- 76 行 `CAPABILITY_CLASSIFICATION`；
- 106 行 `TEST_CONTRACT`；
- 12 行 `RESOLVED_CONFLICT`。

每行都有非空 P7 task、P6 handoff、验证 ID 与 `MAPPED_TO_P7` 状态。P7 任务只实现共享 core、local adapter/package、public adapter/runtime/ops/package 与 build guard；不得建立第二套业务核心。

## 9. Candidate 结果与 Final 完整性验证

`tools/validate-p5.ps1 -ValidationMode Candidate` 已以 0 failures 验证：

- P4 gate 为 `P4_PASS`，P5 输入和文件闭包存在；
- 76 个 P4 delta 一一映射且 classification 唯一，计数为 46/9/19/2；
- 22 个筛选字段均为 `SHARED`；
- 每项能力 owner、module、consumer、build edge、测试和 P7 task 非空且符合 classification；
- 106 个测试满足逐能力 multiplicity 和 variant/negative 配对；
- 12 个冲突全部 `RESOLVED`，unresolved 为 0；
- 194 个 trace 全部映射到 P7；
- `P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001`已贯穿charter、capability/test/conflict/trace、adapter和本门，且storage/package专项断言闭合；
- candidate gate 下 P6/P7 均为 false，副作用计数均为 0。

Candidate 成功后只提升了 `07/07a` 到 `P5_PASS`。`-ValidationMode Final` 必须重算相同闭包并确认 `p6_eligible=TRUE`、`p7_authorized=FALSE` 与全部零副作用字段；Final 失败会撤销 P6 eligibility，P7 始终不得由本门授权。

## 10. 零副作用声明

P5 设计阶段计数：

- Rutgers / 其他新证据网络请求：`0`
- 产品源码、依赖、lockfile、数据库、build 配置变更：`0`
- package build / release publish：`0`
- 服务器、DNS、Cloudflare、证书、凭据或生产变更：`0`
- 共享业务逻辑的设计目标实现数：`1`
- 长期 local/public fork 数：`0`

## 11. Machine-readable final state

```text
phase=P5
record_status=P5_PASS
p5_gate=P5_PASS
p4_input_gate=P4_PASS
review_storage_amendment=P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001
capability_rows=76
shared_rows=46
local_only_rows=9
public_only_rows=19
excluded_rows=2
filter_rows=22
test_rows=106
shared_contract_tests=46
public_variant_tests=19
local_variant_tests=9
local_negative_bundle_tests=21
public_negative_bundle_tests=11
conflict_rows=12
unresolved_conflicts=0
trace_rows=194
shared_business_logic_implementation_count=1
long_lived_fork_count=0
local_physical_database_count=1
local_database_relative_path=data/rbcsp.sqlite
local_storage_path_anchor=RESOLVED_EXECUTABLE_ROOT
local_storage_cwd_independent=TRUE
local_storage_fallback_allowed=FALSE
local_operational_personal_logical_domains=2
public_state_root=/var/lib/bcsp
public_personal_migrations_linked=FALSE
public_personal_tables_linked=FALSE
release_archive_database_files=0
release_archive_seed_or_real_data_files=0
rutgers_requests=0
product_source_mutations=0
package_builds=0
production_mutations=0
p6_eligible=TRUE
p6_reason=P5_FINAL_GATE_PASSED
p7_authorized=FALSE
```
