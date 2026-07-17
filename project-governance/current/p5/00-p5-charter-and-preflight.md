# P5 Charter 与 Preflight — 单一共享基线、双交付适配边界

## 1. 阶段结论与权限边界

- **阶段**：P5
- **状态**：`P5_CORE_BOUNDARY_FROZEN`
- **输入门**：P3 `P3_PASS`；P4 `P4_PASS`
- **目标**：把已冻结的本地一键包完整计划与公网delta归并为一套可实施、可证明没有能力泄漏的共享架构
- **共享产品实现**：一套Rust业务核心、一套React产品UI、一份FilterSchema、一组API/schema版本
- **长期local/public产品fork**：`0`
- **本阶段Rutgers请求预算**：`0`
- **产品源码、依赖、数据库、构建与包变更预算**：`0`
- **生产服务器、DNS、Cloudflare、证书、GitHub Release及其他外部变更预算**：`0`
- **P6 Review存储修正**：`P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001`（2026-07-13）
- **下一阶段门**：P5全部矩阵与验证通过后，P6才可合并最终执行计划；P7仍未授权

P5只冻结“共享什么、在哪里适配、怎样在构建图中证明差异”。它不实现Rust/React代码，不选择或升级依赖，不运行产品构建，不产生两个包，也不触碰生产环境。P3冻结的Catalog、Open、filter、section identity、watch、episode与声音语义，以及P4冻结的公网session、固定时钟、Linux package/operations语义，不得在P5重新解释。

`P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001`只修正此前本地物理数据库拆分与Windows Known Folder路径假设：Windows本地一键包改为由已解析的`RBCSP.exe`所在目录锚定唯一物理库`./data/rbcsp.sqlite`，并在该库内保持operational与`LOCAL_ONLY` personal两套逻辑table/migration所有权域。该决定不改变任何Catalog/Open/filter/watch语义，不授权P7；与本修正冲突的旧物理路径措辞由本决定取代。

## 2. 权威输入与固定SHA-256

旧chat log、旧P1执行产物和deprecated产物不构成本阶段输入。本阶段只消费当前主线P2–P4及主工作流，并将下列直接设计输入按原始文件字节固定。任一hash不符，P5必须停止并解释差异；不得静默重算、改写上游合同或用P5文档替换冻结输入。

### 2.1 P3共享合同与本地完整计划

| P3输入 | SHA-256 | P5用途 |
|---|---|---|
| `p3/07-local-oneclick-implementation-plan.md` | `26B6805284E50639F91FD6523365FD8C0A9607353A0EF6B99D20B6E8910AA634` | 本地完整ALL/ONLY、Windows入口、持久状态与打包边界 |
| `p3/09-p3-validation-and-freeze-gate.md` | `7CCB3CB067781961A17E7910D150325CC34B9390CEFEAE18E4F25A4E48CD9851` | P3总门与P4/P5资格 |
| `p3/23a-shared-open-final-contract.md` | `73031DD718A346D659214D6BE4C571D505836098DE42879E8861349CBD73887E` | 人类可审查的共享Open/scheduler/episode合同 |
| `p3/23b-shared-open-final-contract.json` | `9BFE6712E959C39B6719E7A47F7F96E33AAC5BD20A8CFEF51320C0E2A0C77804` | 机器可读共享Open冻结状态 |

### 2.2 P4公网设计闭包（`00`至`07a`）

| P4输入 | SHA-256 |
|---|---|
| `p4/00-p4-charter-and-preflight.md` | `B4E178CF1EBAE078250CEA1DAFA480169BD0A309BF72DFDFC75A2AD2B6A93647` |
| `p4/01-public-baseline-delta-matrix.tsv` | `70DE2D625C04E955E1947FFD2F6438E1B996340CCBA4994ADA2D633316CB5EB0` |
| `p4/02-public-runtime-session-capability-contract.md` | `EAA63A7E4A58AB8C34BAE22215B3E1088ACABCD0065D8508B8B5607DF97DCB1B` |
| `p4/03-public-refresh-capacity-degradation-plan.md` | `9E63493058F53ECF0009B66E59714EE1C9E28DFCEAA7C3FF5CB4899185425DCD` |
| `p4/04-linux-package-and-operations-plan.md` | `71744604BA7C4CC7EC193A2CB8429BF7494412B9ED1613BA04E9C4AF2816CB90` |
| `p4/05-public-capability-deny-audit.tsv` | `AB9F58D1B67A42924E09413120C67911BF0A5B6F75595FFB70A00C17947CA622` |
| `p4/06-p4-traceability-matrix.tsv` | `F5E5BC153D5E1FF0F39FCE0BFAC373970D2960CE6352A939D5EBDC0B7EBA3AD1` |
| `p4/07-p4-validation-gate.md` | `3DF1C43EB15CDBDA4970A4D61E2B48AA474C4EC31E875001923B36D6B18BE650` |
| `p4/07a-p4-validation-gate.json` | `A879B1988A2121344585C8B1D83992FDF5ADE805A0C3816722055A77D8EFEADD` |

若P2中的历史数值与后续经真实证据及用户Review冻结的P3/P4数值不同，以本节固定的P3/P4输入为准；P5不得恢复已被取代的1秒Open轮询等旧候选值。

## 3. 单一共享基线不变量

下列实现计数在P7完成后也必须保持为`1`，不是只要求“行为大致相同”：

| 共享能力 | 实现计数 | 唯一权威 |
|---|---:|---|
| SectionKey、Course/Section、三值代数与错误码 | 1 | shared Rust domain |
| 22行FilterSchema、normalization、same-section witness、query与理由 | 1 | shared filter/query |
| Catalog client、normalization、target transaction与freshness | 1 | shared Catalog domain/runtime |
| OpenBatch、membership/reconcile、LKG、observation、counter与scheduler | 1 | shared Open domain/runtime |
| OpenEpisode、ONE_SHOT、CONTINUOUS及通知前置语义 | 1 | shared episode/watch domain |
| HTTP/WS DTO、错误envelope、schema版本与生成类型 | 1 | shared contract package |
| course-centered/section-independent React产品UI | 1 | shared React application |
| `en-US`/`zh-CN`共享产品message catalog | 1 | shared i18n base catalog |

本地与公网只能在以下接缝处分化：

1. binary entrypoint与操作系统host；
2. 明确的adapter/provider实现；
3. target-specific capability manifest；
4. 已验证的config policy与storage provider；
5. target-specific前端entry/import graph；
6. package graph与运行/运维附件。

禁止复制、条件分支重写或长期维护第二套domain/filter/Catalog/Open/episode/query/UI核心。平台差异不能渗入共享真值；adapter不得修改Open join、empty/error、freshness、episode或筛选结果。

## 4. 两个交付target的固定差异

| 维度 | Windows本地一键包 | Linux公网包 | 共享要求 |
|---|---|---|---|
| binary入口 | `bcsp-local` / 最终`RBCSP.exe`，loopback、本地browser/launcher生命周期 | `bcsp-server`，loopback behind Caddy、systemd生命周期 | 都调用同一shared application service |
| 用户状态 | 唯一物理库`./data/rbcsp.sqlite`中的`LOCAL_ONLY` personal逻辑table/migration域持久prefs、current filters、selected、Saved views、history；active不持久 | 每个top-level document全新ephemeral状态；无个人持久层，也不链接personal migration/table定义 | 页面内普通filter/watch/audio模型复用 |
| Saved views | 完整LOCAL_ONLY CRUD/CAS/migration | source/import/DOM/route/API/storage/i18n/bundle/package均为零 | 只共享FilterSchema，不共享Saved能力 |
| Reset | 带确认的“重置本地用户数据”；保留Catalog与诊断 | 无Reset产品能力；新document自然回默认 | 不得把public reload命名为Reset |
| Catalog cadence | 默认`600s`；用户范围`1–1440`分钟 | 固定`600s`；普通用户不可配置 | 同一scheduler与single-flight语义 |
| Open general cadence | 默认`30s`；用户范围`3–3600s` | 固定`30s`；普通用户不可配置 | watched target均为`10s`目标；同一EDF/origin limiter |
| counters | 当前local run + Rutgers day | service-wide Rutgers day，跨服务重启 | 同一attempt分类与America/New_York日界 |
| runtime storage | 已解析的exe/package root锚定`./data/rbcsp.sqlite`；一个物理库内分离operational与`LOCAL_ONLY` personal逻辑域；不读取CWD且无路径fallback | `/var/lib/bcsp`下的service-owned operational store；不含、不链接personal migration/table | 同一operational schema/API版本；物理同库不等于所有权混合 |
| package | Windows release archive、`RBCSP.exe`与BAT薄入口；首次运行才创建`data/rbcsp.sqlite` | Ubuntu 24.04 versioned archive、systemd/Caddy/ops；运行时才在`/var/lib/bcsp`创建operational state | 最终严格两个包；两个archive均不含DB/SQLite/WAL/SHM、seed或真实Catalog/Open数据；部署不是第三包 |

公网固定时钟是`Catalog=600s / Open=30s / active-watch target=10s`。本地是`Catalog默认600s、范围1–1440分钟 / Open默认30s、范围3–3600s / active-watch target=10s`；本地Open设为3秒时不得被10秒静默clamp。所有target继续共享真实Rutgers origin concurrency=`1`、per-target single-flight、EDF无饥饿与no catch-up，并诚实显示actual interval/lag。

## 5. P5必须完成的ALL

P5设计闭包必须证明：

1. Rust共享crate与两个独立binary crate的依赖方向单向，public graph无法到达LOCAL_ONLY crate；
2. local/public只通过entrypoint、adapter、capability、config/storage provider及package graph分化；
3. Cargo feature unification或workspace全量构建不会把LOCAL_ONLY符号链接进`bcsp-server`；
4. React只维护一套产品UI与FilterSchema，target-specific entry/import graph在构建前形成可审计闭包；
5. 公网LOCAL_ONLY的source graph、DOM、route、API、storage key、i18n、bundle与package八个表面都为零；
6. 排除不能只依靠CSS隐藏、运行时`if`、路由不展示、minifier或tree-shaking侥幸；
7. 本地完整保留prefs/history/Saved views/Reset与可配置interval，公网完整保留共享搜索、22筛选、course/section、watch、toast与声音；
8. 两个target的HTTP/WS schema与protocol version保持一致；差异通过显式capability contract表达，而不是漂移DTO；
9. 共享测试只写一次；adapter、build graph、zero-surface与package差异增加target-specific门；
10. Windows路径只由已解析的可执行文件目录派生为`data/rbcsp.sqlite`，从任意CWD启动都得到同一路径；不可写或解析失败时明确失败，不回退到CWD、Known Folder、临时目录或其他位置；
11. local personal table/migration域只能由local entry链接；public只使用`/var/lib/bcsp`下operational域且其source/link/schema/package均无personal定义；
12. 两个最终archive的allowlist/denylist都拒绝预建数据库、SQLite sidecar（WAL/SHM）、seed以及真实Catalog/Open数据；
13. 每个边界都可追踪到P3/P4输入、`P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001`与未来P7任务，但P5不实施这些任务。

## 6. P5明确ONLY与零副作用

P5不得：

- 发出Rutgers或其他新证据请求，生成request manifest/ledger/raw body，或压力测试任何真实origin；
- 修改Rust、TypeScript、React、CSS、SQL、Cargo/npm配置、依赖、lockfile、产品fixture、数据库、构建产物或最终包；
- 登录服务器、执行systemd/Caddy、修改DNS/Cloudflare/证书、发布GitHub Release或变更生产数据；
- 创建第二套Open/filter/query/episode/UI实现、第二套API版本或长期local/public源码fork；
- 把local-only源码编入public后仅靠运行时capability关闭；
- 把public的ephemeral状态错误实现为本地user store的空账号、匿名行或no-op Saved repository；
- 把本地数据库路径绑定到process CWD，或在exe-root不可用时静默回退到Known Folder、LocalAppData、临时目录或第二个数据库；
- 在任一最终archive内预装数据库、WAL/SHM、seed或真实Catalog/Open数据；
- 把运维文档、部署结果或SBOM计为第三个最终包。

## 7. 停止条件

以下任一情况要求立即停止并回到Review：

- 固定输入hash漂移，P3/P4 gate不再为PASS，或必须改写P3/P4语义才能完成边界设计；
- public binary或public frontend build graph必须依赖LOCAL_ONLY实现才能编译；
- local/public需要不同FilterSchema、Open reconcile、episode状态机、API/schema版本或React产品树；
- Cargo feature unification无法通过独立crate/entry/package graph消除泄漏；
- zero-surface只能靠CSS、运行时隐藏、未引用假设或tree-shaking证明；
- 需要产品实现、真实构建、网络或生产变更才能让P5文档成立。

## 8. 向P6/P7的交接

P5通过后，P6只能把本阶段冻结的共享core、两个adapter/build graph和测试门合并进最终local/public执行计划与P7 DAG；不得再发明第三个variant。P6完成后必须停在P6 Review，只有用户批准才可进入P7。

P7实施时应先建立共享contract/domain与生成schema，再建立两个binary entrypoint和两条前端build entry，最后分别验证local persistence与public zero-surface/package。真实公网部署仍需P7之后的独立授权。

## 9. Machine-readable state

```text
phase=P5
status=P5_CORE_BOUNDARY_FROZEN
p3_input_gate=P3_PASS
p4_input_gate=P4_PASS
p3_key_inputs_pinned=4
p4_inputs_00_through_07a_pinned=9
review_storage_amendment=P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001
shared_domain_implementation_count=1
shared_filter_schema_implementation_count=1
shared_open_implementation_count=1
shared_catalog_implementation_count=1
shared_episode_implementation_count=1
shared_react_ui_implementation_count=1
long_lived_local_public_forks=0
final_package_count=2
local_physical_database_count=1
local_database_relative_path=data/rbcsp.sqlite
local_storage_path_anchor=RESOLVED_EXECUTABLE_ROOT
local_storage_cwd_independent=TRUE
local_storage_fallback_allowed=FALSE
public_state_root=/var/lib/bcsp
public_personal_schema_linked=FALSE
release_archive_database_files=0
release_archive_seed_or_real_data_files=0
rutgers_requests_authorized=0
product_source_changes_authorized=0
product_source_mutations=0
production_changes_authorized=0
production_mutations=0
p6_entry_requires_p5_validation=TRUE
p7_authorized=FALSE
```
