# P5 Adapter、Entrypoint与Config/Storage边界合同

## 1. 冻结结论

- **状态**：`P5_ADAPTER_ENTRYPOINT_CONFIG_BOUNDARIES_FROZEN`
- **共享Rust业务实现**：`1`
- **独立binary crate**：`2`（Windows local与Linux public各一个）
- **长期产品fork**：`0`
- **API/WS协议版本分歧**：`0`
- **公网LOCAL_ONLY可达crate或符号**：`0`
- **P6 Review存储修正**：`P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001`（2026-07-13）
- **本阶段产品源码/构建变更**：`0`

本合同把P3本地完整计划与P4公网delta落实为依赖方向，不重新定义产品行为。两个binary使用同一套domain、FilterSchema、Catalog、Open、query、watch/episode和shared HTTP/WS contracts；它们只能通过entrypoint、host adapter、capability manifest、config/storage provider及package graph分化。

本合同按`P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001`修正物理存储拓扑：Windows local只有一个由已解析可执行文件目录锚定的`data/rbcsp.sqlite`，operational与personal仍是不同的逻辑table/migration所有权域；Linux public只在`/var/lib/bcsp`承载operational state，且不得链接personal migrations/tables。P7仍未授权。

## 2. 冻结的Rust workspace依赖图

P7建立的workspace必须表达下列逻辑package边界；最终目录名可在P6 dependency表中做不改变依赖语义的机械调整，但不得合并成带`local/public`互斥feature的单体crate。

```text
shared-contracts  <--------------------------+
       |                                      |
shared-domain -> shared-filter-query          |
       |                 |                    |
       +-> shared-catalog-runtime             |
       +-> shared-open-runtime -> shared-watch-episode
                         |                    |
                         +-> shared-application
                                      ^
                  +-------------------+-------------------+
                  |                                       |
        local-runtime-adapter                    public-runtime-adapter
                  |                                       |
        local-user-state-store                  public-operations-store
                  |                                       |
           bin: bcsp-local                         bin: bcsp-server
```

### 2.1 Shared packages

| 逻辑package | 唯一责任 | 禁止内容 |
|---|---|---|
| `shared-contracts` | SectionKey、DTO、stable error code、HTTP/WS protocol version、生成schema输入 | target-specific route、Windows/Linux host、Saved repository |
| `shared-domain` | Course/Section、三值代数、Refresh/Open observations、freshness、counter与episode值对象 | IO、用户持久化、页面session |
| `shared-filter-query` | 单一22行FilterSchema、canonicalization、same-section witness、排序/分页与reason | local/public条件predicate或复制schema |
| `shared-rutgers-client` | allowlisted HTTP、timeout/size/redirect规则、raw DTO与provenance | target cadence、浏览器行为、个人状态 |
| `shared-catalog-runtime` | discovery、normalize、target transaction、Catalog scheduler与FTS/index协调 | Windows/public入口分支 |
| `shared-open-runtime` | OpenBatch membership、LKG、safe empty/error、observations、EDF/single-flight/backoff/circuit | 变体专属Open算法或按用户poller |
| `shared-watch-episode` | connection/watch fanout、OpenEpisode、ONE_SHOT/CONTINUOUS前置语义 | 本地history repository或public session persistence |
| `shared-operational-storage` | Catalog/Open逻辑table/migration域、transaction、checkpoint、bounded diagnostic retention；可被local单库与public service路径分别托管 | Saved views、prefs、selected、个人history；不得决定target物理路径 |
| `shared-application` | 组合共享query/API/WS服务与ports；提供共享router和asset接口 | 导入local-user-state或public ops实现 |

这些package是逻辑边界而非允许复制的模板：每项产品语义只有一份实现和一组共享测试。`shared-application`依赖抽象的调度策略与operational store ports，但不得以`if target == public`改变domain结果。

### 2.2 Local-only packages与入口

`local-user-state-store`只存在于local依赖图，负责同一物理`data/rbcsp.sqlite`内的`LOCAL_ONLY` personal逻辑域：

- 与operational域分离命名、分离版本序列、分离allowlist的LOCAL_ONLY personal tables与migrations；
- current filters/association、最多9个selected SectionKeys、language override、volume/sound/Max audible设置；
- Saved views definition、CRUD、CAS、migration、quota/incompatible状态；
- episode/action history；
- 带确认的本地用户数据Reset。

`local-runtime-adapter`先解析正在运行的`RBCSP.exe`绝对路径，以其父目录作为唯一package root，再机械派生`<package-root>/data/rbcsp.sqlite`；它不得读取process CWD参与路径选择，也不得回退到Known Folder、LocalAppData、临时目录、环境变量指定目录或第二个数据库。路径解析、目录创建、文件创建或写入失败必须返回明确可诊断错误并停止启动。该adapter还负责loopback origin/session nonce、单实例、浏览器打开/退出、本地配置读写和local router extension。`bcsp-local`是独立binary crate，只负责composition root；不得实现第二套业务规则。发布名为`RBCSP.exe`，`Start-RBCSP.bat`只是package层薄入口。

local composition root只打开一个SQLite connection/pool和一个物理文件。它先注册共享operational migration manifest，再由仅local可达的`local-user-state-store`注册personal migration manifest；两个manifest必须使用互不重叠的migration ID namespace、table allowlist和schema snapshot。共享crate只看operational ports，personal crate只看personal ports，物理同库不允许跨域SQL所有权。Reset只清除personal allowlist内的rows/owned tables或执行等价域内重建，不删除`rbcsp.sqlite`，不重置operational schema/version，也不清除Catalog/Open/diagnostics。

Active watch、connection、当前audible count与未确认alarm从不由local user store恢复。Reset只通过local router extension触达，先停止active/watch/mixer，再清user data；Catalog/Open operational store、诊断、日志与应用文件保持不变。

### 2.3 Public-only packages与入口

`public-runtime-adapter`负责：

- 每个top-level document新建ephemeral页面session的服务边界；
- public固定refresh policy与只读status surface；
- loopback bind、trusted reverse-proxy/Origin/nonce策略、readiness与graceful shutdown；
- service-wide Rutgers-day counters与非个人operational state恢复；
- public静态asset manifest及无LOCAL_ONLY的router composition。

`public-operations-store`只对共享operational schema提供`/var/lib/bcsp` service-owned路径、backup/restore与migration host glue；它不实现用户、Saved views、prefs、selected、watch或history repository。public dependency/link graph、migration registry、schema snapshot及runtime database都不得出现personal table/migration定义。严禁提供`NoOpSavedViewsRepository`、匿名user row、空personal schema或占位migration来伪装零表面。

`bcsp-server`是独立binary crate，只依赖shared packages与public adapters。它不依赖、编译或链接`local-user-state-store`、`local-runtime-adapter`、Windows launcher或任何local route/schema/i18n/assets。systemd/Caddy/ops属于公网package graph，不进入Rust共享业务实现。

## 3. Port与provider合同

| Port/provider | Shared消费者 | Local实现 | Public实现 | 不得改变的语义 |
|---|---|---|---|---|
| `RefreshPolicyProvider` | Catalog/Open scheduler | 从已验证local settings读取范围内值 | 返回编译/部署冻结常量；无用户mutation port | target key、EDF、single-flight、origin concurrency、backoff |
| `OperationalStore` | Catalog/Open/query/status | 已解析exe root下`data/rbcsp.sqlite`中的operational逻辑域 | `/var/lib/bcsp`下service-owned operational DB；不链接personal域 | operational schema/version、transaction、retention、counter grain |
| `PersonalLocalStore` | **仅local router/UI extension** | 同一`data/rbcsp.sqlite`中的独立personal table/migration域 | **不存在；crate、migration与table定义均不链接** | 物理同库不合并逻辑所有权；Reset不得越过personal allowlist |
| `Clock/Timezone` | scheduler/counters | 注入真实或fake clock | 注入真实或fake clock | `America/New_York`日界与lag公式 |
| `AssetProvider` | shared HTTP shell | local target build manifest | public target build manifest | shared route与shared UI schema |
| `RuntimeHost` | application lifecycle | Windows loopback/browser/single-instance | Linux loopback/systemd/Caddy readiness | active不跨进程恢复 |
| `UserStateService` | **仅local router/UI extension** | local user-state store | **不存在，不提供public实现** | public不能出现no-op或nullable替代物 |

`UserStateService`与`PersonalLocalStore`不属于`shared-application`启动所需port，因此public composition root无需、也不能构造它们。共享业务依赖必须始终从entrypoint指向shared packages；shared packages不得反向依赖任一adapter。local把两个逻辑域放入同一物理SQLite文件是composition/storage-host决定，不得把personal schema提升为shared依赖。

### 3.1 固定物理路径与archive空数据合同

Windows运行时拓扑唯一为：

```text
<resolved-executable-root>/
  RBCSP.exe
  Start-RBCSP.bat
  data/                 # 可在首次成功启动时创建
    rbcsp.sqlite        # 仅运行时创建；不在release archive内
```

从package root、其子目录、快捷方式或任意其他CWD启动，解析结果都必须是同一个`<resolved-executable-root>/data/rbcsp.sqlite`。没有“便携模式/安装模式”自动探测，也没有失败后路径fallback。

Windows与Linux两个release archive都必须通过denylist证明不存在：`*.sqlite`、`*.sqlite3`、`*.db`、`*-wal`、`*-shm`及其他SQLite sidecar、seed/snapshot/fixture数据库，以及真实Rutgers Catalog/Open payload。空目录占位不得携带数据文件；schema与migration作为代码/资源定义随binary交付，数据库仅在首次运行时创建。Linux运行时状态只在安装/启动后写入`/var/lib/bcsp`，不由archive预装。

## 4. Capability manifest

每个target在构建前生成一份闭合的、不可由普通用户修改的capability manifest，用来约束router、UI import与package audit，而不是在运行时隐藏已经编入的能力。

Local manifest明确包含：

- persistent current filters/selected/settings；
- Saved views；
- durable episode/action history；
- Reset local user data；
- configurable Catalog/Open intervals；
- local run counters；
- Windows launcher/lifecycle。

Public manifest只列共享搜索、筛选、course/section、Open/status、current-page selection/watch/toast/audio、i18n与service operations能力；它不包含Saved views等LOCAL_ONLY键，即使值为`false`也不能把相关symbol、文案或schema带进public graph。Manifest不是服务端动态feature toggle，不能被request、env或数据库切换成local能力。

## 5. 固定Config policy

| 配置 | Local policy | Public policy |
|---|---|---|
| Catalog refresh | 默认`600s`；用户可设`1–1440`分钟；越界拒绝、不clamp | 固定`600s`；无普通用户设置/API mutation |
| Open general refresh | 默认`30s`；用户可设`3–3600s`；越界拒绝、不clamp | 固定`30s`；无普通用户设置/API mutation |
| Active-watch target | 同target存在active时`10s`，但local general=`3s`时effective仍为`3s` | 同target存在active时`10s` | 都使用`min(general,10)` |
| Rutgers origin concurrency | 固定`1`，Catalog/Open共享 | 固定`1`，Catalog/Open共享 | adapter不得提高 |
| Retry/backoff/circuit | P3共享合同 | P3共享合同 | 不允许target override |
| Locale | 首次系统检测，local override持久 | 每个document按系统/浏览器，页面内临时选择 | canonical `en-US`/`zh-CN` |

Config按三层拆分：

1. `SharedSafetyConfig`：timeout、body size、allowlisted origin、concurrency与backoff合同；只能由共享代码定义，两个target一致；
2. `RefreshPolicy`：local为范围验证后的user setting，public为固定常量；两者输出相同typed结构；
3. `HostConfig`：listen/state/log/public origin等部署值，只影响adapter/host，不得改变产品domain。

Public runtime config schema不得接受Catalog/Open cadence的普通用户或部署覆盖。Local配置失败必须返回stable validation error；不得用默认值悄悄吞掉无效输入。

## 6. Cargo feature与feature-unification防泄漏

LOCAL_ONLY能力不得实现为shared crate上的`local` feature，也不得用`--no-default-features`的偶然命令保证安全。冻结规则如下：

1. local/private能力放在独立crate，public binary的normal、build和dev release依赖闭包均不得出现这些crate；
2. `bcsp-local`与`bcsp-server`是两个独立binary package，各自拥有明确直接依赖，不使用一个binary里的`cfg!(target_os)`或`cfg(feature)`切换产品；
3. workspace显式使用Cargo resolver v2或经P6批准的更强等价resolver；shared crates的default features为空或仅含两个target都需要的能力；
4. `workspace.dependencies`不得全局启用LOCAL_ONLY依赖feature；共享第三方依赖的feature集合必须以两个release graph的union审查，不能改变产品capability；
5. 公网release只以package-scoped、locked命令构建`bcsp-server`，禁止`--all-features`；本地同理构建`bcsp-local`；
6. 即使CI执行`cargo build --workspace`同时构建两个binary，也必须分别审计最终link/dependency graph；同一workspace中“local crate也被构建”不等于可以进入public artifact；
7. P7用`cargo metadata`、`cargo tree`/等价图、binary symbols、archive allowlist与denylist共同证明public闭包；只看最终UI行为不合格；
8. feature unification一旦导致public reachable graph出现local-only crate/symbol，构建立即失败，不能靠strip、LTO或dead-code elimination补救。

允许的Cargo features只用于单一target内部的纯技术选择，且不得改变domain/API/capability。例如可由P6审查TLS/backend编译选项，但不得出现`saved-views`、`persistent-history`或`public-mode`之类共享业务feature。

## 7. Router、API与schema版本

- `shared-contracts`只生成一次共享HTTP/WS DTO、stable error envelope和`API_PROTOCOL_VERSION`。
- 共享routes在两个binary中使用同一handler/service和同一schema snapshot；contract tests对共享路径逐项做结构等价验证。
- Local-only routes由`local-runtime-adapter`单独composition，包括Saved views、history、settings persistence与Reset；它们仍使用同一个protocol version与error envelope，但不进入public router或public schema artifact。
- Public没有对应的404占位handler、disabled method、GraphQL field、WS method或capability flag；请求local-only path应由通用未知路由处理，不能泄漏能力名。
- Public固定refresh/status API不接受interval mutation字段；local extension接受typed interval update。共享status DTO字段与含义不漂移，public/local仅按合同暴露各自counter scope。
- WS SectionKey、OpenObservation、episode/fanout frame与protocol version完全共享，不允许target-specific frame fork。

新增或变更共享字段必须一次更新共享schema并同时通过local/public contract tests。Local extension演进不得提升共享API version之外的私有版本线；若确需破坏性变更，必须在同一全产品version中处理。

## 8. P7构建与验证门

| ID | 必须证明 |
|---|---|
| `P5-RUST-001` | shared domain/filter/Catalog/Open/episode各只有一个实现crate与一组authoritative tests |
| `P5-RUST-002` | `bcsp-server` cargo metadata/tree无local adapter/user-state crate |
| `P5-RUST-003` | workspace/all-target构建不通过feature unification污染public link graph |
| `P5-RUST-004` | local/public shared route、DTO、error和WS schema版本一致 |
| `P5-RUST-005` | local `600s,1–1440m / 30s,3–3600s / watch10s`边界与invalid rejection |
| `P5-RUST-006` | public `600/30/10`固定，config/API/env不能修改 |
| `P5-RUST-007` | local单一`data/rbcsp.sqlite`内personal CRUD/history/Reset完整且不越过逻辑域；public无personal repository/migration/table/no-op替代物 |
| `P5-RUST-008` | 两binary只通过composition root与ports分化，adapter不能改变共享结果 |
| `P5-RUST-009` | public symbol/schema/archive/package扫描无LOCAL_ONLY personal migration/table；LTO/strip前后都验证 |
| `P5-RUST-010` | P3 Open fake-upstream suite在两个entrypoint复用且结果一致 |
| `P5-RUST-011` | Windows从多个CWD启动均解析同一exe-root `data/rbcsp.sqlite`，且所有fallback注入都明确失败 |
| `P5-RUST-012` | 两个archive均无DB/SQLite/WAL/SHM、seed或真实Catalog/Open；首次运行才创建各自runtime state |

这些是P7验收规范，不授权P5创建crate、Cargo.toml、schema或测试代码。

## 9. Machine-readable state

```text
status=P5_ADAPTER_ENTRYPOINT_CONFIG_BOUNDARIES_FROZEN
review_storage_amendment=P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001
rust_shared_domain_implementation_count=1
shared_open_implementation_count=1
rust_binary_entrypoints=2
local_binary=BCSP_LOCAL
public_binary=BCSP_SERVER
long_lived_product_forks=0
public_local_only_dependency_nodes=0
public_local_only_runtime_symbols=0
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
public_build_uses_local_only_feature=FALSE
cargo_feature_unification_leakage_allowed=FALSE
cargo_all_features_public_release_allowed=FALSE
api_schema_version_divergence=0
ws_protocol_version_divergence=0
public_catalog_interval_sec=600
public_open_general_interval_sec=30
public_open_active_interval_sec=10
local_catalog_default_sec=600
local_catalog_min_minutes=1
local_catalog_max_minutes=1440
local_open_default_sec=30
local_open_min_sec=3
local_open_max_sec=3600
local_open_active_interval_sec=10
real_origin_concurrency=1
product_source_mutations=0
```
