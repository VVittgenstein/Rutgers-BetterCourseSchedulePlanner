# RBCSP 渐进式架构审计

日期：2026-09-07。源码基线：`7168ab2793e94723065a9872d3f785f9d5474456`（main）。
状态：审计及第一阶段 ARCH-001 的本地实现、验证完成，实施记录见文末。
R01–R12 的引用与行号记录的是上述审计基线；后续阶段仍为建议。

## 判断

优先处理的是**旧路径没有退场、测试支架进入生产接口、架构规则重复记录实现细节**。
现有 local/public 产品边界、纯查询引擎、Open 完整性门和持久化意图都有实际价值。
保留这些边界，在现有 crate 内逐步收口，比重新搭建一套框架更合适。

本轮枚举了全部 tracked 文件，并对 359 个 Rust、TS/TSX、MJS、Shell/PowerShell 文件做
规模、符号引用和重复片段扫描，沿下列热点核对真实调用及测试。合计 199,818 个物理行，
包括空行、注释、测试和工具；不是业务代码量。没有逐行人工审阅全部约 20 万行。
未跟踪的对话归档、旧 worktree、构建输出和运行数据库不属于清理对象。

| 覆盖面 | 检查内容与结论 |
|---|---|
| 15 个 workspace member：13 个库、2 个程序 | Cargo 依赖、各 lib 导出、入口及低引用 API；确认依赖脚手架残留 |
| contracts / domain / query | 序列化、别名、schema/golden、三值逻辑、同一课节证据；核心行为测试应保留 |
| Rutgers / catalog / operational-storage | 上游适配、投影、迁移和查询准备；确认 SQL 镜像、存储职责集中 |
| open / watch / application | Gate、监看准入、调度、准备快照、产品路由；确认旧状态投影与构造链 |
| local-user-state / local-runtime | CAS、批量意图、个人状态、重启与本地装配；部分测试走旧可选装配路径 |
| public-operations / public-runtime | 公网存储所有权、票据、容量、宿主与准入；薄层不等于无效层 |
| frontend | 全部入口的文件图、低引用导出、状态所有权、测试夹具及 UI 断言 |
| architecture / S3 / packaging / deploy / CI | 架构规则与实际 CI 接线；S3 是离线工具，未混入生产调度 |

## 有证据的清理项

### R01：V1 服务状态投影已退出生产调用链

- `crates/bcsp-application/src/service_status.rs:215` 的 `project_service_status` 无调用，
  通过 `#[allow(dead_code)]` 留存。实际 HTTP 路由在 `product_routes.rs:328` 调用 V2。
- `service_status.rs:781` 起的 8 个辅助函数只被这条旧链或其单测使用；也分别压制 dead-code lint。
- `catalog_ready_while_open_pending_is_partially_ready`、`level_contract_covers_initializing_ready_and_error`、
  `lkg_with_degraded_issue_remains_searchable_and_degraded`、
  `latest_open_failure_with_lkg_is_degraded_but_an_old_failure_is_not` 四个测试仍只验证旧辅助链。
- **处理：删除旧投影及专用辅助链，逐项将仍适用的场景绑定到实际 V2 路径，再删除旧测试。**
  约 400 行旧生产实现可以退场；这不是整个文件或全部 V1 合同都可删除。
  `unavailable_service_status` 仍被当前路由使用，实际错误响应与兼容性类型必须保留。

### R02：依赖脚手架制造了虚假的“使用”

- 13 个库共有 12 个 `boundary_marker()`、13 个 `PACKAGE_BOUNDARY`、
  13 个 `dependency_contract` 和 6 个 `dev_dependency_contract`。
- `boundary_marker()` 在仓库源码、测试和工具中没有调用；常量只在脚手架内部交叉引用。
  示例：`crates/bcsp-application/src/lib.rs:104`。
- 匿名导入 `use dependency as _;` 不能证明业务需要依赖。去掉这些模块后的文本扫描发现
  17 个 normal dependency 声明在所属 `src` 内没有引用。例如 `bcsp-watch` 的
  serde/serde_json/tokio/tracing/uuid，`bcsp-domain` 的 serde/uuid，
  `bcsp-public-operations` 的 rusqlite/tracing。
- **处理：删脚手架，逐个核对 src、tests、build 和 feature 合并后删依赖，运行受影响 crate 检查。**
  “17 个”是候选声明数，不是已经验证可移除的独立包数。真实依赖必须以编译和目标闭包复验。

### R03：有些包装器和导出确实没有消费者

| 位置 | 核验结果 | 处理 |
|---|---|---|
| `bcsp-watch/src/facade.rs:261` `start_with_current` | 只有定义；实际准入走 `start_with_admission` | 删除此便利入口 |
| `bcsp-open/src/service.rs:752` `execute_official` | 只有定义 | 删除便利入口及随之失去用途的 import |
| `bcsp-local-user-state/src/store.rs:228` `remove_selected_section` | 只有定义 | 删除未使用的单项写入 API；保留实际批量持久化路径 |
| `bcsp-rutgers-client/src/discovery.rs:300` `try_from_bootstrap` | 只有定义 | 删除便利构造入口，保留有调用的构造和校验 |
| `bcsp-application/src/host.rs:433` `spawn_loopback_server` | 只有定义与 lib 转导出 | 删除该无 socket 便利入口；保留实际 socket 宿主 |
| `frontend/.../local/product/LocalDesiredWatchContext.tsx:24` | `useMemo(() => watchIntent, [watchIntent])` 返回相同引用 | 直接传入 `watchIntent`；Context 本身保留 |
| `frontend/.../shared/design-system/primitives.tsx:133,158` | `FieldFrame`、`VisuallyHidden` 只有定义与 barrel 转导出 | 删除未用组件及专用 props；不清空设计系统 |
| `frontend/.../shared/i18n/presenter.ts:33,39,79` | 三个 message-key 映射只有定义 | 删除 prerequisite/permission/matchOutcome 映射；不要连带删有用翻译 |
| `frontend/.../shared/watch/readiness.ts:25` | `WATCH_READINESS_RINGS` 只有定义 | 删除常量，保留仍使用的 ring 类型 |
| `frontend/.../local/personal/contracts.ts:236` | `PrepareUserDataResetRequest` 只有定义 | 删除未用 TS interface；保留实际 reset 协议 |

上述短路径中的 `frontend/.../` 对应 `frontend/src/ui/`；Rust 短路径均位于 `crates/`。
这是当前 workspace 内的引用结论；实施时需重新搜索，不能把一次快照变成永久 deny list。

额外实测：现有 TypeScript 检查通过，但开启 `noUnusedLocals` / `noUnusedParameters` 后，
shared 检查报 `LiveWatchProvider.tsx:33` 的 `WATCH_CONTACT_STALE_MILLISECONDS` 未用 import。
local/public/build 三个 project 的额外检查通过。这两个开关不负责发现未使用的公开导出。

### R04：刷新运行时保留了过时的 pool 构造链

`crates/bcsp-application/src/refresh_runtime.rs:171` 起有六层 spawn 入口，逐层补默认值、
转换参数。最深一层在 `:273` 对 `Vec<Coordinator>` 只执行 `into_iter().next()`，
再按 target `fork_empty()`。生产在 `official_refresh_runtime.rs:496` 构造单元素 Vec。

**处理：用一个明确的 coordinator template 参数，加一个承载真实选项的启动配置。**
合并旧 pool 便利入口，测试也调用该正式入口。保留全局网络预算、三并发、target 优先级、
取消和 shutdown；不能因为清理构造层而重新做调度器。
`refresh_runtime.rs:147` 的 `run_wal_maintenance` 只丢弃一次 `state.run` 的返回值，也可就地内联。

## 降低持续修改成本

### R05：架构门重复维护了 Cargo，并限制正常源码组织

`tools/architecture/verify-rust-graph.mjs:51` 手写外部依赖版本/features；`:73` 手写各 crate 的
精确依赖集合；`:478` 比较集合相等。Cargo.toml、Cargo.lock 之外形成另一份需同步的依赖模型。
CURRENT.md 已记录 v0.1.4 曾仅因新增 dev dependency 未同步清单而构建失败。

`frontend/tools/verify-import-graph.mjs:126` 还固定 npm 脚本片段，`:28` 起排除 CSS 等源码组织方式；
模块可达检查不能识别“通过 barrel 可达但无人使用的导出”。

**处理：从 Cargo metadata/真实构建图读取事实，把规则收窄为有理由的边界约束。**
保留 public→local 禁入、共享层方向、原生打开能力归属、normal cache/dev hooks 等必要限制。
发布工具链和依赖锁定继续留在 release contract 与 lockfile。模块移动、删无用依赖不应要求复写完整图。
规则改变时，用越界依赖、local 泄漏、异常 feature 等反例验证检查器仍能拒绝错误。

### R06：迁移 SQL 被手工镜像到 Rust

`crates/bcsp-operational-storage/src/migration_bundle.rs` 共 1,056 行，手写了 8 个 SQL 文件的副本。
本轮逐份文本比较均一致；`migration.rs:502` 的测试也在保证镜像和源文件一致。
同时，Rust source gate 在 `verify-rust-graph.mjs:900` 普遍拒绝 `include_str!` 等嵌入，
仅为特定前端资产开例外。这种规则使消除重复需要连同边界检查一起设计。

**处理：迁移 `.sql` 作为唯一人工维护来源，编译期嵌入；检查器递归审计允许的字面量路径及内容。**
限定到受跟踪的迁移文件，拒绝路径逃逸、symlink 和非预期输入。编译期嵌入仍能生成独立运行的二进制。
精确保持已有 SQL 字节、migration ID/hash、外键控制及事务行为，保留真实升级/回滚测试。
镜像一致性测试在双份维护仍存在时有价值；解决双份来源后再改成嵌入清单及历史 hash 检查。

### R07：LiveWatchProvider 承担了过多独立生命周期

`frontend/src/ui/shared/watch/LiveWatchProvider.tsx` 3,084 行。调用点扫描计得 35 个 useState、
49 个 useRef、31 个 useEffect、63 个 useCallback；这是定位线索，不是质量评分。
同一组件负责 selection 保存、desired CAS 队列、遥测请求排序、socket 事件、重连计划、声音、通知和 readiness。

建议按状态所有权拆分：

- selection persistence：confirmed/desired/in-flight 和失败回滚；
- intent authority：generation/revision、queue、uncertain/disproved、cutoff 必须由同一对象持有；
- telemetry：batch 分组、debounce、abort、请求序号和 freshness 到期；
- connection lifecycle：显式断开、重连计划和 transport 事件；
- alert delivery：声音、通知与 episode 去重；已有 audio/readiness 纯模块继续复用。

先独立抽 telemetry，再迁 intent authority；每步让 Provider 使用新模块，并移除对应旧逻辑。
不要只是把相同的一大包 refs 传给更多 hooks。React Provider 最终负责订阅和组合。
local 能力仍由 local composition 注入，不能通过 shared import local 来完成拆分。

### R08：准备快照、查询参考实现与测试装配没有完全分清

`prepared_serving.rs` 4,236 行，其中末尾测试前约 2,953 行；同文件包含 publication barrier、
worker 生命周期、registry、不可变 snapshot、动态字典和 FTS SQLite。
`query_service.rs` 同时包含直接读 storage 的 `SharedQueryService` 与 `PreparedQueryService`。
实际查询路由已用 prepared；`local-runtime/src/personal.rs:48` 却仍通过可选 prepared 字段，
在 `:208` 等位置保留两种运行路径。其旧路径还被测试及 reference parity 使用，不能整块误判为死代码。

**处理：先明确生产装配所需字段，再把 reference 查询保留在测试支持或明确命名的模块。**
将 prepared 内部拆为 publication/registry、snapshot、dictionary/FTS、worker；第一步不改 crate 图。
保持一个已绑定的快照完成一次请求、dirty/commit 屏障和 Open overlay 的不可用掩码。
测试优先用临时文件数据库覆盖正式装配，避免为省夹具而让生产构造接受缺失关键服务的状态。

### R09：手写合同有多份来源，版本别名进一步增加定位成本

Rust DTO、3,002 行 `contracts/src/schema.rs`、TS contracts、goldens 都表达部分相同结构。
`contracts/src/query.rs:1721` 起将 V2/V3 别名指向 V1 名称，两个 schema 函数又转调 V1；
当前实际 `contractVersion` 已是 3。保留兼容 wire 是必要的，内部命名不必永久追随旧版本。

**处理：先清理无消费者的别名，选择一个小合同试点从权威定义生成 TS/结构元数据。**
逐个覆盖 enum、nullable、可选字段、未知字段策略和 JSON 数字范围。保留手写输入校验、迁移、
旧数据解码及代表性 wire goldens；结构生成不能替代这些行为验证。避免一开始就建设通用 schema 平台。

### R10：测试需要按判别力整理

| 对象 | 证据 | 建议 |
|---|---|---|
| R01 的四个 V1 测试 | 只调用已退出生产链的旧辅助函数 | 将适用场景覆盖到 V2 后移除 |
| `frontend/tests/ui-shell.test.tsx:368` | 对比度断言使用硬编码颜色，CSS 只检查字符串存在 | 验证实际 token/计算样式；颜色与背景相同应失败 |
| `filter-panel.test.tsx:143`、`course-workspace.test.tsx:157`、`watch-workspace.test.tsx:1220` | 固定 CSS 字符串、尺寸和声明顺序，容易阻碍等价改写 | 合并少量确有产品意义的规则；布局/焦点/滚动用相应浏览器行为检查 |
| `product-contracts.test.ts:26,123` | `JSON.parse(...) as T` 后仅检查几项 fixture 内容 | 不能作为 TS/Rust 完整绑定证据；生成类型检查或真实 codec 测试更有判别力 |
| 多份 FakeWatch/FakeAudio | desired-watch 两个文件有 49 个重叠的相同 15 行窗口；共 4 个文件声明 FakeWatch | 共享可控 transport/clock fixture，保留各场景断言；重叠窗口数不是可删行数 |
| schema binding / wire / malformed tests | 分别保护结构、字节与拒绝非法输入 | 合并重复 fixture，保留不同故障模型的断言 |

本轮做了一个无文件修改的反例：给设计 CSS 追加覆盖规则，使 `--bcsp-ink` 与 `--bcsp-paper`
同为 `#F6F6F4`，在 Node VM 中执行 ui-shell 那个测试的原断言体，以 Node assert 适配 expect。
**原来的 30 次断言全部通过。**这是该断言体判别力不足的证据；没有声称它是整套 Vitest 的变异测试，
也没有声称已经做过浏览器对比度测量。

三值真值表、同一 section witness、CAS/receipt/tombstone、撤绿、断线恢复、Gate、
事务回滚、启动迁移、容量背压与 local/public 隔离测试应保留。测试数量本身不构成删除理由。

### R11：常规 CI 缺产品回归反馈

唯一 workflow `.github/workflows/public-ops.yml:54` 的 push/PR job 运行打包/ops 合同、
Rust 检查器 self-test、前端 import guard/self-test；没有运行 `cargo test`、Clippy、
frontend Vitest 或 TypeScript。手动 Linux 包构建也不等于这些行为测试。

**处理：增加普通 PR 产品验证入口。** Rust tests/Clippy/fmt、前端 tests/typecheck 与真实边界门
应该随源码变化运行；Windows 特有行为由 Windows job 覆盖。两套前端构建负责验证目标资产。
S3 改动触发其离线 suite；部署 soak 和正式 release gates 仍在对应发布/部署阶段运行。
本地迭代只跑相关测试，阶段结束再跑完整组合，避免每次改 helper 都等全套门。

### R12：重复准入与导航成本应在第二轮收口

`local-runtime/src/watch.rs:52` 与 `public-runtime/src/watch.rs:26` 的 target 分组、
投影及错误映射高度相似。共享批量 admission 算法可以进入已有 application 模块，
两端只提供 storage/policy/clock；历史保存、local desired authority、公网配额仍各自持有。

仓库此前没有根 AGENTS.md；CURRENT.md 有八百余行，混合当前规则、已结束阶段及历史数字。
本轮新增简短 [AGENTS.md](../../AGENTS.md) 作为按任务找代码和测试的入口，不改变既定实现/审查分工。
后续把稳定不变量与历史日志分开维护，避免让普通代码修改先读整段聊天史。

## 目标形态与推进顺序

保留 13 个库，先在原库中建立清晰的模块所有权。目标是一次修改能定位到一个主实现、
一个直接验证入口和必要的集成边界；以下是建议结构，不是现有目录的宣称。

```text
apps + local/public composition
  -> application: product routes / prepared query / refresh / watch transport
       -> catalog + open + watch + query: 各自业务规则
       -> storage + Rutgers client: 持久化和外部输入

frontend local/public composition
  -> feature controller: selection / intent / telemetry / connection / delivery
  -> shared React UI + 现有 product/intent ports

authoritative contract / SQL / manifests
  -> 编译或生成产物
  -> 真实行为测试 + 必要的产品边界检查
```

| 阶段 | 完整交付 | 完成判据 |
|---|---|---|
| A：清理与反馈 | R01–R03；普通产品 CI；依赖删除只同步现有 gate，不同时重写 gate | 死链退场、行为门执行、wire/SQL/产品边界不变 |
| B：消除重复事实 | R04–R06；合并正式构造入口、语义边界规则、SQL 单一来源 | 不再维护 pool 空包装或 SQL 双份文本；历史迁移 hash 保持一致；越界反例仍失败 |
| C：监看纵向切片 | R07；先 telemetry，再 authority，其余按需要迁移 | 每次抽取都接回产品；取消/乱序/重连/批量/撤绿场景通过 |
| D：查询与合同收口 | R08–R10、R12；prepared 分模块、测试夹具、单合同生成试点 | 生产必需依赖明确；reference 角色可见；不新增第二套查询/监看实现 |

每阶段都可独立合入，全部阶段不强制合成一次大改。迁移字节不变时可通过代码 revert 回退；
若某阶段需要数据迁移或 wire 改动，应拆出独立产品变更及升级方案。
不以统一文件行数上限、crate 数减少或新增抽象数量作为验收标准。

## 第一阶段任务包（原始范围，现已批准）

- 名称：ARCH-001，清理失效入口并建立产品回归反馈。
- 起点：上述 main 基线；工作分支 `codex/architecture-foundation`。
- 范围：R01–R03 涉及源码/测试、相应 Cargo manifest 与既有 graph fixture、TS unused 检查、
  新的普通 CI job/workflow，以及必要文档。
- 不包含：R04 以后的运行时拆分、架构门语义重写、迁移改写、版本升级、发布或生产主机操作。
- 行为：已有两款产品行为保持；状态 V2 及当前错误回包、9/255 各自容量、CAS、断开屏障不变。
- 测试：四个旧状态测试逐一登记适用的 V2 对应场景；删除纯死代码不用新增镜像测试。
  对依赖调整验证 normal/dev feature；对 CI 入口给出实际执行的测试命令。
- 提交：CI、Rust 清理、frontend 清理可各一个逻辑 commit；保留用户未跟踪归档，不夹带构建资产。
- 停止条件：查出未列出的有效消费者时保留该项并记录；需要改变 wire/数据/产品语义时提出具体差异，
  继续完成不依赖该决定的清理。普通低风险债务不扩大本阶段。
- 回报：base/head、变更文件、删去入口与调用搜索、测试结果、保留项和 git status。
  实现完成声明仍待独立验收。

### 可直接交给实现代理的提示

```text
实现 RBCSP 的 ARCH-001：清理已失效的源码入口，并让普通 PR 执行产品回归测试。
先读 AGENTS.md、docs/orchestration/CURRENT.md 和
docs/design/2026-09-07-agent-friendly-architecture-audit.md 的 R01–R03、R11。
核对 git status、HEAD 与基线 7168ab2793e94723065a9872d3f785f9d5474456；不要覆盖用户文件。
在 codex/architecture-foundation 工作；若分支已存在，核对其内容，不要重置。

范围只有 R01–R03、产品 CI、相关 manifest/graph fixture 和文档。
删除无调用的 V1 service-status 投影及专用辅助链，检查四个旧单测：仍有效的场景应覆盖实际 V2，
随后移除旧链测试。保留当前路由仍调用的 unavailable_service_status、错误回包和 wire 类型。
删除 boundary_marker/PACKAGE_BOUNDARY/匿名依赖脚手架；从 src/tests/build/feature 角度逐个审查依赖，
证实无用再删，并同步现有架构 gate 的事实清单。本阶段不重写 gate 规则。
重新搜索 R03 的所有消费者，清理确认未用的 Rust/TS 入口、映射和 import，以及无效 identity useMemo。
保留 Context、ProductApi、时钟/传输 seam 和 local/public 真实能力边界。
启用 noUnusedLocals/noUnusedParameters 后检查所有 TS project；不要为删代码新增无行为价值的测试。

增加普通 push/PR 的 Rust tests/Clippy/fmt 和 frontend tests/typecheck/build/实际边界检查；
不要破坏现有 packaging-contracts job 或把正式发布/部署变成 PR 步骤。Windows 专有用例用 Windows。
源码迭代先跑受影响测试；阶段结束运行：
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
npm --prefix frontend run verify
cargo test --workspace --locked
node tools/architecture/verify-rust-graph.mjs
node tools/architecture/verify-public-rust-zero-surface.mjs
node --test tools/architecture/verify-rust-graph.test.mjs tools/architecture/verify-public-rust-zero-surface.test.mjs
先完成前端构建，再验证与嵌入资产相关的 Rust 用例。不要把全量门放在每个小提交后。

禁止改既有 SQL 字节/ID/hash、wire version、local 255/public 9 上限、CAS/receipt/tombstone、
显式断开、准备快照和 Gate 语义。禁止趁机拆分 Provider、重做调度器或引入通用框架。
不升级依赖版本，不打包发布，不部署，不提交构建输出或用户未跟踪归档。
发现有效消费者就保留该项并报告；需要产品决定时说明精确影响，继续独立可完成的清理。
可按 CI/Rust/frontend 分逻辑提交，无需逐 helper 等待；不要添加外部实现工具归因 trailer。
完成后回报：用户可见结果、base/head/commits、文件、合同是否改变、精确测试结果、
旧链/无调用证据、保留项和 git status。完成声明需 Codex 独立验收。
```

## 本轮实际验证

验证在 Windows 工作树运行，Node 为 `24.19.0`（发布固定版为 `24.18.0`），Rust 为 `1.97.0`。
这不是正式可复现发布验收。

- 前端：31 文件、476 测试通过；import guard + 92 guard 测试通过；4 个 TS project 常规检查通过。
- 两套前端构建与目标验证通过；Vite 仍有超过 500 kB 的 chunk 提示。
- Rust：workspace Clippy `--all-targets --all-features -D warnings`、`cargo fmt --all -- --check` 通过。
- Rust 两个实际 architecture gate 和两个 self-test 文件通过。
- 额外 TS unused 检查：shared 发现 1 个未用 import；local/public/build 通过。
- CSS 断言反例：原断言体 30 次检查仍通过，说明它不能证明实际颜色对比度。
- `cargo test --workspace --locked`：873 passed、0 failed、1 ignored；另有 harness-free
  `desired_watch_fixture` 正常退出。跳过项是既有的真实浏览器嵌入 UI 测试。
- 未运行 Linux 部署/soak、完整 S3 suite 或真实浏览器产品验收；未修改产品源码、SQL 或 CI。

临时扫描明细与命令日志位于本机会话的 `%TEMP%/rbcsp-architecture-audit-20260907/`，
不作为仓库构建输入。本文保留了独立定位问题需要的源码路径、调用证据和验证口径。

## ARCH-001 实施记录

用户在审计后明确批准执行；本次由 Codex 直接实施第一阶段，后续阶段与发布/部署不包含在此次交付中。
实施分支：`codex/architecture-foundation`；基线为 `7168ab2`。随后按用户授权快进合入 main，
`1536cd9` 已推送远端；实现分支已清理。

实现提交：`4fdbecd`（CI）、`6303c6b`（Rust/依赖/路由测试）、`157baac`（frontend）。
源码交付范围为 `7168ab2..157baac`；后续文档提交记录导航与本次证据。

- 删除 R01 的 V1 投影、8 个专用辅助函数和 4 个旧测试；实际 V2 投影、错误映射及仍在使用的
  V1 fallback 经文本比较保持不变（只归一化换行）。
- 清除全部 boundary marker / dependency contract 脚手架。删除 23 条 crate 依赖声明
  （7 条内部、16 条外部），另将 public-operations 的 rusqlite 改为 dev dependency。
  根 workspace 移除无主的 tower-http 声明；现有 graph 规则仅同步事实，没有放宽边界。
- Cargo.lock 清除 http-range-header、mime_guess、unicase 三个孤立条目；保留包的版本和 checksum
  全部不变。既有 migration SQL 字节、wire 定义与产品容量未改。
- 删除 R03 列出的无调用 API、未用前端组件/映射/类型和 identity useMemo；删除 execute_official
  后同时收掉其唯一使用的 SystemOpenPullClock，保留被实际使用的 OpenPullClock trait。
- 启用所有 TS project 的 noUnusedLocals/noUnusedParameters。辅助错误文字和无障碍 CSS 仍被界面
  使用，保留相关样式；未扩大为 UI 重构。
- 新增 product-verify.yml：源码变化触发 Linux/Windows 的产品测试、类型检查、双前端构建、
  Rust fmt/Clippy 和实际架构门。S3 变化走独立 s3-verify.yml；原有打包/部署 workflow 未改。

旧测试的行为去向：

| 旧测试 | 当前有效覆盖 |
|---|---|
| catalog_ready_while_open_pending_is_partially_ready | 新 service_status_v2_requires_complete_snapshots_before_reporting_ready：按当前合同，只有 Catalog 不算可用；完成 Open 后才进入就绪统计 |
| level_contract_covers_initializing_ready_and_error | 上述真实路由生命周期测试覆盖 Initializing/PartiallyReady/Ready；新 service_status_keeps_the_v1_error_payload_when_storage_is_unavailable 覆盖 Error fallback |
| lkg_with_degraded_issue_remains_searchable_and_degraded | 保留 ready_target_in_retry_wait_remains_queryable_from_its_last_complete_snapshot；新生命周期测试覆盖全量 READY 后 RetryWait → DEGRADED 且快照仍可用 |
| latest_open_failure_with_lkg_is_degraded_but_an_old_failure_is_not | 新 service_status_v2_reports_the_latest_open_failure_and_clears_it_after_success：真实 SQLite 失败/成功写入后读取 HTTP 状态，确认历史错误不残留 |

阶段验证：前端 verify 的 476 项行为测试、92 项 guard 测试、类型检查和双目标构建通过；
产品路由定向测试 24 项通过；S3 168 项通过；两个实际 Rust 架构门及 self-test 通过。
完整 Rust workspace 为 872 passed / 0 failed / 1 ignored，另有 harness-free fixture 正常退出。
测试净减 1 项来自删除 4 个旧辅助函数测试、增加 3 个真实路由测试；fmt 与最终 workspace
Clippy（all-targets/all-features、locked、-D warnings）通过。

新 workflow 的 YAML、触发路径、权限、矩阵、步骤顺序及 Bash 语法已在本机检查。
本机没有可用 Linux/WSL 环境。用户授权推送 main 后，实际 CI 暴露了三个窄问题：
`653f265` 将一个跨四页面的完整测试流程总时限从 5 秒设为 15 秒，保留全部断言和单步等待限制；
`76896aa` 把 PathBuf 引用限定到实际使用的 Windows 分支，保持运行行为。定向测试分别为 12/12、10/10。
`9d06cea` 修正 Pong 背压测试对内核默认缓冲大小的假设，只在测试连接两端设置缓冲预算；
23 项宿主测试通过。临时移除生产 Pong timeout 时原断言失败，恢复后通过，证明拒绝能力仍在。
packaging contract 和 S3 CI 已通过，最终 Product verification
[Linux/Windows 均通过](https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/actions/runs/34106149718)。
该 run 验证源码 `9d06cea`：Windows Rust 872 passed/1 ignored，Linux Rust 871 passed/1 ignored，
两端均无失败；frontend 476、类型检查、双目标构建、Clippy 和实际架构门均通过。
后续提交仅更新记录文档，与此验证源码相比没有产品或测试代码变化。
远端非 main 分支已清理；未发布新安装包或部署。用户原有未跟踪对话归档保持不变，
实施日志位于 `%TEMP%/rbcsp-arch-001/`。
