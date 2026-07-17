# P5 React UI Capability与Build Exclusion合同

## 1. 冻结结论

- **状态**：`P5_UI_BUILD_EXCLUSION_CONTRACT_FROZEN`
- **React产品UI实现**：`1`
- **FilterSchema实现**：`1`
- **public build-reachable LOCAL_ONLY source nodes**：`0`
- **public LOCAL_ONLY DOM/route/API/storage/i18n/bundle/package surface**：全部`0`
- **CSS隐藏是否满足排除**：`FALSE`
- **仅依赖tree-shaking是否满足排除**：`FALSE`
- **本阶段前端源码/构建变更**：`0`

“同一React UI”指local与public共同消费一套产品shell、course-centered搜索、22行筛选、course/variant/section、direct detail、Open状态、current-page selection/watch/toast/audio和i18n基础。它不意味着先把LOCAL_ONLY组件编入public再隐藏。Saved views、持久prefs/history、Reset与interval配置只存在于local target-specific source graph；public graph在bundler运行前已经无法到达它们。

## 2. 冻结的前端source graph

P7前端必须采用两个显式entry、一个shared application与严格单向import：

```text
                         shared FilterSchema
                                  |
shared API/WS client -> shared React application <- shared en-US/zh-CN catalog
                                  ^
                  +---------------+---------------+
                  |                               |
        entry.local.tsx                    entry.public.tsx
                  |                               |
     local route/API/i18n adapter        public session/runtime adapter
     Saved/history/settings/Reset         fixed policy/status shell
                  |                               |
           local asset graph                   public asset graph
```

允许的逻辑目录/包方向：

| 图节点 | 可导入 | 禁止导入 |
|---|---|---|
| `ui/shared/**` | shared contracts、FilterSchema、shared API/WS、shared i18n | `ui/local/**`、`ui/public/**`、target manifest |
| `ui/local/**` | `ui/shared/**`、local-only API/schema/catalog | `ui/public/**` |
| `ui/public/**` | `ui/shared/**`、public session/status adapter | `ui/local/**`、local-only schema/catalog |
| `entry.local` | shared + local root | public root |
| `entry.public` | shared + public root | local root、local glob/barrel/dynamic import |

不得创建一个同时export shared/local/public的顶层barrel。不得使用`import.meta.glob`、route filesystem discovery、i18n全目录扫描或动态字符串import把`ui/local/**`意外纳入public chunk graph。Shared代码不能通过target name、environment variable或运行时feature flag选择产品能力。

## 3. 同一产品UI与FilterSchema

### 3.1 Shared React application

两个target必须逐字节或从同一源构建以下UI逻辑：

- `/` course-centered搜索、course group/variant card和MATCH/UNCERTAIN/other sections；
- `/sections`独立section搜索；
- `/sections/:term/:campus/:index`可reload/direct访问详情；
- 22行FilterSchema控件、active chips、普通filter Reset、三值reason与same-section witness结果；
- loading、initializing、partial、valid empty、suspect empty、stale、failed、offline、validation error与ready；
- Open requested/effective/actual interval、lag、freshness、LKG、UNKNOWN/circuit与时间戳/计数显示；
- 当前页面最多9个selected sections、显式watch、toast、ONE_SHOT、CONTINUOUS和浏览器audio错误；
- shared `en-US`/`zh-CN`文案、日期/数字/plural/ARIA以及Rutgers原文保留。

Shared component不得出现`isPublic ? ... : ...`来维护两套业务树。允许通过类型化slot挂入target自有导航/页面，但shared shell不知道slot的LOCAL_ONLY名称或数据模型；public root传入的slot集合中根本没有local extension。

### 3.2 单一FilterSchema

- FilterSchema及22字段metadata、default、codec、canonicalization和reason key只定义一次；local/public搜索和shared UI从同一生成输出消费。
- Local SavedViewSnapshot只能单向引用shared FilterSchema的version/canonical snapshot；shared FilterSchema绝不引用Saved view定义、repository或route。
- Public不维护“去掉Saved字段的第二份schema”，因为Saved state本来就不属于FilterSchema字段；普通filter Reset语义两个target一致。
- 构建记录同一个FilterSchema content hash与API schema version。任一target单独改字段、enum、default或unknown语义都使全产品构建失败。

## 4. Target-specific composition

### 4.1 Local entry

Local root在shared application之外显式增加：

- Saved views manager与local-only route/side panel；
- persistent history surface；
- settings中的language override、Catalog/Open interval配置和Reset local user data；
- local user-state API client与revision/CAS conflict handling；
- local-only `en-US`/`zh-CN`catalog，且两个locale key parity；
- Windows/local help与data-location文案。

Local current filters、selected、settings、Saved views和history的持久权威是Rust local user store；不能另建浏览器`localStorage`复制数据库。Active watch、audible count与未确认alarm仍只在当前运行/页面状态，不跨启动恢复。

### 4.2 Public entry

Public root只增加ephemeral session lifecycle、service status/readiness、固定refresh policy展示和public ops安全反馈。每个top-level document：

- filters/selected/audio/settings/alerts/active从产品默认或空值初始化；
- language按系统/浏览器检测，页面内切换不持久；
- 不读写`localStorage`、`sessionStorage`、IndexedDB、Cache Storage、cookie、URL query/hash或Service Worker data来恢复产品状态；
- 不提供普通用户interval mutation或强制上游refresh；
- 页面reload/query/watch人数不触发一个新的Rutgers pull。

Public capability manifest只正向列出实际共享/public能力，不出现Saved views等LOCAL_ONLY key的`false`占位。Public bootstrap、API和DOM不得通过disabled flag泄漏能力名称。

## 5. Public零表面八层闭包

“source为零”指public build-reachable graph为零，不要求把同仓库中local交付所需源码删除。每项LOCAL_ONLY能力必须同时跨以下八层通过：

| Surface | Public必须为零 | 合格证明 | 不合格替代 |
|---|---|---|---|
| `SOURCE` | public entry可达的local component、codec、repository client、type、migration helper | TypeScript/bundler dependency graph denylist | 文件未被手工打开、未调用函数 |
| `DOM` | button、link、menu、panel、dialog、hidden node、ARIA label、data attribute | rendered DOM/accessibility tree scan | `display:none`、disabled、off-screen |
| `ROUTE` | Saved/history/settings persistence/Reset等local route与route name | generated route manifest + navigation/direct URL tests | 不展示导航但router仍注册 |
| `API` | local-only REST/WS method、client、schema、bootstrap capability | public OpenAPI/client/WS manifest diff与server route test | 404 handler占位、返回disabled |
| `STORAGE` | key、IndexedDB schema、cookie、Service Worker cache、server user table/client | source scan + browser DevTools/automation + DB schema audit | 写空值、匿名user、no-op repository |
| `I18N` | local-only message key、英文/中文文本或chunk | target i18n manifest/key scan | key存在但永不调用 |
| `BUNDLE` | symbol、module ID、source map、lazy chunk、CSS selector、literal | pre/post-minify asset scan + module manifest | 依赖minifier、tree-shaking、strip |
| `PACKAGE` | local assets/docs/schema/source map/manifest residue | Linux archive正向allowlist与unpack scan | 文件存在但server不serve |

该闭包至少覆盖P4 `05`冻结的18项排除能力及全部8个surface，尤其Saved views、persistent personal state/history、Reset、local interval config/run counters、Windows launcher和证据/secret residue。Direct section URL、current-page selection/watch/toast/audio、响应式密度、共享service diagnostics不得被zero-surface测试误删。

## 6. Build graph强制规则

### 6.1 TypeScript与module boundaries

1. Shared、local、public使用独立project references或等价静态边界；public project不include/reference `ui/local/**`。
2. Lint/import-boundary规则禁止shared导入target、public导入local；违规在bundle前失败。
3. Public alias表不定义`@local`；解析到local路径即hard error，不fallback到stub。
4. Route、API client与i18n manifest分别从明确allowlist生成；禁止从整个源码树自动发现。
5. Generated files记录target、shared schema hash、FilterSchema hash、locale key count和input list；不得记录私有绝对路径。

### 6.2 Bundler与assets

- Local build的唯一input是local entry；public build的唯一input是public entry。两者可以复用同一bundler配置函数，但必须传入类型化target manifest。
- Public bundler plugin在module-resolution阶段拒绝local path/deny symbol，而不是等待tree-shaking。
- Public build不复制一个包含local chunks的“完整dist”再删文件；只从public module graph产生新的content-addressed output目录。
- CSS与assets遵循相同graph。Local-only selector、icon、font subset、help text不能进入shared stylesheet或public chunk。
- Source map若P6允许生成，只能作为受控内部调试制品；两个最终包默认不含source map。任何map也必须接受同一zero-surface/secret/path扫描。
- Local assets只进入Windows archive/Rust embed；public assets只进入Linux公网包。package组装使用正向allowlist，不能用递归复制workspace dist。

### 6.3 为什么隐藏与tree-shaking不合格

CSS隐藏、`hidden`属性、disabled route或运行时capability check仍把DOM/API/i18n/bundle表面交付给公网用户；不满足P4。Tree-shaking受side effect、dynamic import、chunk split、source map和bundler版本影响，只能作为体积优化，不能作为安全/产品边界。

合格证明必须按顺序成立：

1. source graph在bundle前已无LOCAL_ONLY节点；
2. route/API/i18n/storage manifests无LOCAL_ONLY条目；
3. bundle前后module/symbol/literal/CSS扫描为零；
4. runtime DOM/network/storage行为测试为零；
5. 最终unpacked公网包再次为零。

任何一层失败都阻断public package，不可由另一层“看不到”抵消。

## 7. API、i18n与版本一致性

- 两target使用同一`API_PROTOCOL_VERSION`、共享DTO生成物、FilterSchema version、OpenObservation/WS frame和stable error codes。
- Shared route/client schema必须结构等价；local-only client在local graph单独生成，public client manifest没有对应方法或类型。
- Shared `en-US`/`zh-CN`catalog只包含共享能力key且保持parity；local catalog以local entry专属chunk扩展，也保持双语parity；public不生成local key的空翻译。
- UI build version与Rust package version来自同一release metadata；target字段只标识`local`或`public`制品，不能派生独立产品版本线。
- Public固定`Catalog=600s / Open=30s / watched target=10s`以只读状态展示；local展示`Catalog默认600s、1–1440分钟 / Open默认30s、3–3600s / watched target=10s`并通过local-only设置API修改。两者共享actual/lag/freshness组件。

## 8. P7验证矩阵

| ID | 必须结果 |
|---|---|
| `P5-UI-001` | local/public共享页面从同一React组件与同一FilterSchema生成，22字段/hash一致 |
| `P5-UI-002` | public TypeScript/bundler graph到`ui/local/**`的可达节点为0 |
| `P5-UI-003` | shared→target、public→local非法import在bundle前失败 |
| `P5-UI-004` | public route/API/i18n/storage manifests无LOCAL_ONLY条目或false占位 |
| `P5-UI-005` | 18能力×8surface的P4 zero-surface audit在源码、runtime与package重放通过 |
| `P5-UI-006` | CSS hidden/disabled/runtime flag/tree-shaking故意注入fixture均被门拒绝 |
| `P5-UI-007` | public reload/new tab/direct detail恢复默认且不持久、不产生Rutgers请求 |
| `P5-UI-008` | local Saved CRUD/CAS/migration/history/Reset/interval设置完整，且active不恢复 |
| `P5-UI-009` | shared route/DTO/WS/error、API version与双语base catalog在两个target一致 |
| `P5-UI-010` | unpacked Windows/Linux两个包只含各自asset graph，无第三包与交叉附件 |

P5只冻结这些验收门，不创建frontend entry、tsconfig、bundler config、route、catalog或产物。

## 9. 停止条件

以下任一情况必须停止Review：

- 共享UI必须知道local/public target并维护不同筛选、Open或episode业务树；
- public build必须先导入local module才能复用FilterSchema或shared components；
- local-only route/schema/i18n只能通过运行时关闭或tree-shaking排除；
- shared与local Saved views出现反向依赖，迫使public携带Saved codec/type；
- 两target的共享API/WS/FilterSchema version发生漂移；
- zero-surface检查会误删direct section URL、current-page watch/audio或service diagnostics；
- 需要修改P3/P4合同、产品源码、真实网络或生产环境才能完成本设计。

## 10. Machine-readable state

```text
status=P5_UI_BUILD_EXCLUSION_CONTRACT_FROZEN
react_ui_implementation_count=1
filter_schema_implementation_count=1
shared_open_ui_semantics_implementation_count=1
frontend_target_entries=2
public_local_only_source_graph_nodes=0
public_local_only_dom_nodes=0
public_local_only_routes=0
public_local_only_api_methods=0
public_local_only_storage_keys=0
public_local_only_i18n_keys=0
public_local_only_bundle_symbols=0
public_local_only_package_artifacts=0
css_hiding_satisfies_exclusion=FALSE
runtime_feature_flag_only_satisfies_exclusion=FALSE
tree_shaking_only_satisfies_exclusion=FALSE
public_build_exclusion_requires_graph_proof=TRUE
shared_api_schema_version_divergence=0
shared_filter_schema_hash_divergence=0
product_source_mutations=0
```
