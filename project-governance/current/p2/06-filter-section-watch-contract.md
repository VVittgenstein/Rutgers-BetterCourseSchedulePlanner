# P2 本地基线 Contract — 筛选、Section、Watch、Toast 与声音

## 1. 状态与解释优先级

- **状态**：P2最终批准合同；2026-07-13用户回复与“批准P2”均已纳入
- **适用对象**：本地一键包的完整产品基线；除明确标注为`LOCAL_ONLY`或`PUBLIC_DELTA`的能力外，普通用户语义属于`BASELINE_SHARED`
- **目的**：把 P1 已固定但仍缺精确语义的筛选、section、subscription management、toast、Max audible notifications 和声音行为闭合为可测试 contract
- **非目标**：不在 P2 决定 Rust 模块、UI视觉、最终 endpoint 命名、Rutgers 实际QPS、生产容量或真实部署；未取得真实 Rutgers 数据前，不把推测写成字段事实

若本文件与旧代码、旧 docs、旧 release 或旧 Compact 声称冲突，以本文件的 P2 裁决为准；旧材料只用于解释复用和清理。

## 2. 结果代数：确定匹配、不确定候选、不匹配

所有可筛选对象都使用三值结论：

| 结果 | 含义 | 默认展示 |
|---|---|---|
| `MATCH` | 所有激活条件都有足够数据证明满足 | 主结果 |
| `UNCERTAIN` | 没有已知反例，但至少一个激活条件因 TBA/unknown/缺失可靠数据无法证明 | 独立“不确定候选”区域，不混入确定结果 |
| `NO_MATCH` | 至少一个条件有已知反例 | 不进入默认结果；查看其他 sections 时显示原因 |

规则：

1. 缺失数据不得被当作确定匹配。
2. fallback 字典不得伪装成真实当前数据；数据不可用时必须显示 unavailable/error/freshness 状态。
3. 当某字段未激活为筛选条件时，该字段缺失不影响匹配，但仍在详情中标注 unknown。
4. `NO_MATCH`只在`AND`聚合中具有否决优先级；同维度`OR/ANY`必须使用第3.1节真值表，不能把`NO_MATCH + UNCERTAIN`误算成`NO_MATCH`。

## 3. 组合语义与同一个 Section 见证

### 3.1 布尔组合

- 不同维度：`AND`
- 同一维度多选：默认 `OR`
- `core codes`：提供显式 `ANY / ALL`
- 除获得明确 contract 的维度外，不增加隐藏的 ANY/ALL 或模糊聚合模式

三值`AND`：

| AND | MATCH | UNCERTAIN | NO_MATCH |
|---|---|---|---|
| MATCH | MATCH | UNCERTAIN | NO_MATCH |
| UNCERTAIN | UNCERTAIN | UNCERTAIN | NO_MATCH |
| NO_MATCH | NO_MATCH | NO_MATCH | NO_MATCH |

三值`OR`：

| OR | MATCH | UNCERTAIN | NO_MATCH |
|---|---|---|---|
| MATCH | MATCH | MATCH | MATCH |
| UNCERTAIN | MATCH | UNCERTAIN | UNCERTAIN |
| NO_MATCH | MATCH | UNCERTAIN | NO_MATCH |

- `ANY`严格等于对所选值做三值`OR`；`ALL`严格等于三值`AND`。
- 空维度表示该维度未激活，作为`MATCH`恒等元，不用“空OR=NO_MATCH”排除全部结果。

### 3.2 Course 命中

激活 section 条件时：

```text
courseMatch = coursePredicates(course)
              AND EXISTS one same section s
                  WHERE every active sectionPredicate(s) is MATCH
```

三值实现不得只计算boolean：每个section先对其激活section predicates做`AND`；同一course的sections再做`OR`；最后与course predicates的`AND`合并。于是“一个section为NO_MATCH、另一个为UNCERTAIN”的course结果是`UNCERTAIN`，而任一section为`MATCH`即可满足section层。

禁止由不同 sections 分别满足不同条件。例如，Section 01 只满足时间、Section 02 只满足 open，不能让 course 成为确定匹配。

若 course 条件没有反例，但不存在 `MATCH` section、只存在至少一个 `UNCERTAIN` section，则 course 进入 `UNCERTAIN`，而不是主结果。

### 3.3 展示

- Course card 默认只展开 `MATCH` sections。
- `UNCERTAIN` sections 必须单列并解释不确定字段。
- 用户可以显式“查看其他 sections”；每个 section 给出逐维度 mismatch/unknown reasons。
- 未激活任何 section 条件时，该 course 的所有 sections 都是候选展示对象，并仍按状态/标识稳定排序。
- 服务端统一计算筛选、总数、分页和理由；禁止在服务端分页后由前端再次删除 sections/courses，造成 total、页数或空页漂移。

## 4. 当前版本筛选字段 ALL 集合

### 4.1 Context 与 course 级

| ID | 字段 | 级别 | 数据与可靠性 | 操作符 | 同维度 | unknown 规则 | 当前裁决 |
|---|---|---|---|---|---|---|---|
| FLT-C01 | Term | context | Rutgers term dictionary；必须带source/version | exact，单选必需 | n/a | dictionary不可用则不查询 | `REQUIRED / PORT` |
| FLT-C02 | Campus | context/course offering | 当前catalog实际 campus | exact | 多选 OR；空=全部可用campus | 记录缺失→UNCERTAIN | `REQUIRED / PORT` |
| FLT-C03 | Subject | course | 当前catalog subject code/description | exact | 多选 OR；空=全部 | unknown→UNCERTAIN | `REQUIRED / PORT` |
| FLT-C04 | Free-text search | course discovery | course code/string、subject、number、title、expanded title | case-insensitive token AND；exact course identifier优先排序 | token间AND | FTS未构建/数据不可用是错误，不回退假结果 | `REQUIRED / PORT` |
| FLT-C05 | Course number | course | normalized course number | exact；可输入多个 | OR | unknown→UNCERTAIN | `REQUIRED / REWRITE`；历史scalar exact predicate可PORT，当前rewrite后缺独立字段，多值/unknown/跨层contract须补齐 |
| FLT-C06 | Level | course | explicit level；若推导必须标来源 | exact | OR | unknown→UNCERTAIN | `REQUIRED / PORT` |
| FLT-C07 | Credits | course | min/max/display | inclusive numeric range | min AND max | unknown→UNCERTAIN | `REQUIRED / PORT` |
| FLT-C08 | Core code | course | structured core attributes | exact | 用户显式ANY/ALL | unknown→UNCERTAIN | `REQUIRED / PORT` |
| FLT-C09 | Prerequisite presence | course | 必须区分empty与unknown | any / has / none | n/a | 选has/none而来源unknown→UNCERTAIN | `REQUIRED / PORT` |
| FLT-C10 | Course campus location | course | structured campus location code/desc | exact | OR | unknown→UNCERTAIN | `REQUIRED / PORT` |

`campus` 与 `subject` 不再是为了保护旧SQL而强制“至少选一个”；用户可在一个 term 内通过精确 course identifier 或其他条件定位课程。性能由索引、分页和查询限制保证，不通过破坏用户搜索语义保证。

### 4.2 Section 级

| ID | 字段 | 数据与可靠性 | 操作符 | 同维度 | unknown 规则 | 当前裁决 |
|---|---|---|---|---|---|---|
| FLT-S01 | Section index | `(term,campus,index)`；UI可输入index但必须结合context | exact | OR | 无法解析→NO_MATCH/明确错误 | `REQUIRED / REWRITE` |
| FLT-S02 | Section number | structured section number | exact | OR | unknown→UNCERTAIN | `REQUIRED / REWRITE` |
| FLT-S03 | Open status | `OPEN / CLOSED / UNKNOWN`及时间戳 | exact | 多选 OR | UNKNOWN单列 | `REQUIRED / PORT` |
| FLT-S04a | Delivery modality | 必须保留 Rutgers raw code/description 与映射依据；不得把“非online”自动当作on-campus | 暂定 canonical：`ON_CAMPUS_OR_IN_PERSON / ONLINE / HYBRID / OTHER / UNKNOWN` | OR | 可靠已识别但不属三主类→OTHER（选择OTHER可MATCH）；未识别/缺失/malformed/conflict→UNKNOWN（具体modality筛选为UNCERTAIN）；不得静默丢弃 | `REQUIRED / PORT / DATA_VALIDATION_REQUIRED` |
| FLT-S04b | Delivery synchronicity | 与modality正交；不得仅凭“有/无day/time”猜测 | 暂定 canonical：`SYNC / ASYNC / MIXED / UNSPECIFIED / UNKNOWN` | OR | generic Online可为`ONLINE + UNSPECIFIED`；选择SYNC/ASYNC时为UNCERTAIN | `REQUIRED / REWRITE / DATA_VALIDATION_REQUIRED` |
| FLT-S05 | Instructor | 优先structured instructor ID+display name；无structured时标低可靠性 | dictionary exact only | OR | unknown→UNCERTAIN | `REQUIRED / PORT`；当前不增加contains/fuzzy独立行为 |
| FLT-S06 | Weekly/occurrence availability | 见第5节 | interval containment | 全部必修occurrences AND | TBA/unknown→UNCERTAIN | `REQUIRED / REWRITE` |
| FLT-S07 | Meeting campus/location | location/campus code；必须来自meeting | exact | OR | unknown→UNCERTAIN | `REQUIRED / PORT` |
| FLT-S08 | Building/room | structured building/room；普通用户可选精确值或输入精确code | exact | OR | unknown→UNCERTAIN | `REQUIRED / PORT` |
| FLT-S09 | Exam code | structured exam code/text；code不是meeting occurrence本身 | exact | OR | unknown→UNCERTAIN | `REQUIRED / PORT` |
| FLT-S10 | Special permission | add permission structured code | any / required / not-required | n/a | unknown→UNCERTAIN | `REQUIRED / PORT` |
| FLT-S11 | Eligibility/major/minor/honors | 只有structured且已由ingest证明的数据才能确定匹配 | 每维度exact | 同维度OR，维度间AND | 缺失或无法解释→UNCERTAIN | `REQUIRED / PORT` |

Delivery暂定enum的边界现在固定，raw值到enum的具体映射留给真实数据门：

- modality `OTHER`：来源明确给出一种已知、可解释但不属于On Campus/In Person、Online或Hybrid的modality。
- modality `UNKNOWN`：raw缺失、malformed、互相冲突或证据不足，无法确定modality。
- synchronicity `UNSPECIFIED`：已知modality是Online，但来源没有给出可判定的Sync/Async subtype。
- synchronicity `UNKNOWN`：连该属性是否存在、是否适用或raw能否可靠解释都无法确定。
- synchronicity `MIXED`：同一section有可靠证据包含同步与异步的required组成；不能用多条互相冲突/低可靠raw自动合成。

### 4.3 Detail-only，不作为当前普通用户筛选维度

以下必须在 course/section 详情中展示，但当前不增加独立筛选控件：

- course/section notes、comments、subtitle、unit/subject notes；
- synopsis/source URL；
- cross-listings；
- source freshness、updated timestamp；
- permission drop information；
- raw/unknown labels和可靠性说明。

这样保留决策信息，但不把低可靠、难解释或没有明确用户价值的字段伪装成精确筛选。

### 4.4 当前不进入筛选面的旧候选

| 旧候选 | 裁决 | 原因/去向 |
|---|---|---|
| Waitlist filter/alert | `EXCLUDED` | 用户明确不做；Rutgers统一SOC只提供可依赖的Open/Closed等状态，当前没有可靠、全校统一的waitlist筛选字段；不得用CLOSED或其他raw hint冒充WAITLIST |
| `updatedSince` 普通用户控件 | `INTERNAL_ONLY` | 可用于cache/invalidation，不是用户找课主维度 |
| Named Compact view | `EXCLUDED` | 当前正式响应式UI自行处理信息密度，不提供无证据的旧命名开关 |
| Share links | `EXCLUDED` | 用户明确不做；可复制direct URL只属于导航/直接访问基础能力，不命名为分享产品功能 |
| Legacy `keywords / tags` preset fields | `EXCLUDED / REMOVE` | 旧UI/state曾出现但未进入实际query，是no-op表面；关键词价值由FLT-C04承担，结构化标签须映射到已有可靠字段，不能保留假筛选 |
| Calendar | `FUTURE / DEFER` | P1已明确不进当前核心 |

### 4.5 历史覆盖结论与真实数据门

对“当前筛选字段是否已经完全满足旧代码历史中承诺、提供的能力”的回答是：**目前不能证明完全满足**。

- 第4.1与4.2节已经覆盖旧代码/文档中可识别的大多数找课维度：term、campus、subject、文本、course number、level、credits、core、prerequisite、section index/number、Open/Closed、delivery、instructor、星期/时间、地点、exam、permission、eligibility。
- “字段列在schema、type、stub或旧文档中”不等于该字段曾经端到端可用；旧实现存在空section route、未摄取字段、前端静默丢值、normalizer与API枚举不一致等断链。
- Delivery 已拆为modality与synchronicity两轴。历史样本证明generic Online可能无法可靠细分同步/异步，因此可表示为`ONLINE + UNSPECIFIED`；但 `online / hybrid / on-campus / in-person / sync / async` 的当前raw取值、缺失率和跨层映射尚未用届时真实 Rutgers 数据证明，不能宣称历史承诺已经被完整兑现。
- 用户已经明确排除Waitlist、Share links与Named Compact view；即使旧历史出现相关名称，也不再属于当前ALL集合。
- 用户于2026-07-13后续Review明确要求做Saved views，并最终裁决只在本地包提供。旧设计中的`PresetManager`、Save view和`dirtyFields`只能作为交互/复用证据：允许Git历史中未发现真正的PresetManager持久化实现，不能把设计文档或dead state冒充已交付功能。当前精确contract见第7.4节。

允许语料中的raw→normalize→DB→query→UI核对结果如下。`E2E_BASIC`只表示旧基础exact链可达，不表示已满足本contract的三值、可靠性或真实数据门：

| Field | 旧/当前链路状态 | 主要断点 |
|---|---|---|
| C01 Term | `PARTIAL` | scope可写可查；dictionary source/version未闭合 |
| C02 Campus | `PARTIAL` | context可写可查；display/source/freshness多为回填 |
| C03 Subject | `E2E_BASIC` | 基础exact可达；仍须真实dictionary/profile |
| C04 Free text | `STUB / TEST_ONLY` | ingest未维护实际FTS表；测试手工seed不能证明真实课程可查 |
| C05 Course number | `DATA_ONLY` | raw已摄取；当前course route缺独立exact参数；历史SQL曾实现 |
| C06 Level | `E2E_BASIC` | 基础exact可达；推导来源仍须标注 |
| C07 Credits | `PARTIAL` | range可查；variable/arranged与缺失可靠性未闭合 |
| C08 Core | `PARTIAL` | join/query存在；只有OR，缺显式ANY/ALL完整链 |
| C09 Prerequisite | `PARTIAL` | text/boolean可查；NULL、empty、unknown被混淆 |
| C10 Course campus location | `E2E_BASIC` | 基础exact可达；仍须source/unknown验证 |
| S01 Section index | `DATA_ONLY / STUB` | raw已摄取；section handler固定空结果，旧unique key不足 |
| S02 Section number | `DATA_ONLY / STUB` | raw已摄取；独立section query固定空结果 |
| S03 Open status | `PARTIAL` | 状态可写；UNKNOWN与false混淆，empty可能破坏性误关 |
| S04a/S04b Delivery | `PARTIAL / DRIFT` | normalizer、DB、API与UI枚举不一致；async/remote/hybrid可能误归或静默丢弃 |
| S05 Instructor | `DATA_ONLY / STUB` | text/raw存在；structured关联/字典未实际填充，section handler为空 |
| S06 Availability | `PARTIAL / INCORRECT` | meeting存在；旧query只需一条row命中，缺occurrence/requiredness/TBA完整语义 |
| S07 Meeting campus/location | `PARTIAL` | raw/query部分可达；location层次与unknown三值未闭合 |
| S08 Building/room | `DATA_ONLY` | raw入库；当前course/section有效query参数缺失 |
| S09 Exam code | `E2E_BASIC / PARTIAL` | course基础query可达；独立section和unknown语义未闭合 |
| S10 Special permission | `DATA_ONLY / STUB` | raw入库；handler为空，add/drop code语义未验证 |
| S11 Eligibility/major/minor/honors | `DATA_ONLY / PARTIAL` | JSON/text部分入库；关联表/部分字段未populate，handler为空 |
| Legacy keywords/tags | `NO_OP` | 旧state/URL表面未进入实际query；已明确排除假筛选 |

当前实现的结构性Delivery证据尤其明确：`scripts/soc_normalizer.ts`可能生成`async`，而course/section route与`frontend/src/state/courseFilters.ts`只接受三类，`frontend/src/api/filters.ts`还会静默丢弃未知枚举；历史真实样本曾出现raw mode `90/91/92/93`（Online/Hybrid/Remote Sync/Remote Async），旧classifier会误分其中部分值。这些历史样本只证明风险存在，不代表2026-07-13之后的当前分布，所以仍必须执行下面的live证据门。

在P3冻结本地数据/筛选计划前，必须执行一次受控、只读、低请求量的真实 Rutgers 课程数据核验；在P4冻结Open轮询计划前，必须核验同scope的`openSections`。核验至少应：

1. 选择当时真实有效的term与**已批准交付范围内**的campus，不永久硬编码当前term ID；开始前冻结request manifest（endpoint、term×campus矩阵、请求硬上限、串行间隔、重试上限、停止条件），扩大scope必须回Review；记录采样时间、参数、HTTP结果、payload hash与schema/profile摘要。
2. 独立盘点raw Delivery code/description、meeting day/time、location、instructor、exam、permission、eligibility及其缺失/未知分布；产品现有classifier不得作为独立oracle。
3. 固定modality/synchronicity两轴的“用户显示名 ↔ URL/API wire value ↔ DB canonical ↔ Rutgers raw”映射；generic Online必须保留已知online事实而把同步性标`UNSPECIFIED`，未观察到的值只能标`NOT_OBSERVED`，不能写成“不存在”。
4. 验证`H / TH`等星期别名、非法时间、TBA、无meeting、online/on-campus混合meeting与多meeting section。
5. P3课程数据门计算裸index、`(term,index)`、`(term,campus,index)`碰撞并冻结join contract；P4复用同一catalog证据，再验证每个`openSections` index在同一scope中恰好join到一个section。样本无碰撞也不得反推裸index全局唯一。
6. 区分空数组、malformed、HTML error、timeout、429/5xx与真实零Open；错误只允许来自自然观察或本地fixture/injection，禁止为覆盖分支而发送无效、突发或压力请求。未自然观察到则标`NOT_OBSERVED`。
7. 保存raw provenance；只将最小、去敏回归fixture与hash/manifest纳入仓库，完整响应进入ignored evidence区。
8. 任一真实数据证据与当前contract冲突时立即停止冻结，回到共同Review；不得静默改字段、合并类别或降低严格规则。

## 5. Meeting availability 精确语义

### 5.1 用户输入模型

用户设置零个或多个 availability windows：

```text
AvailabilityWindow = { weekday, startMinutes, endMinutes }
```

- 同一星期可有多个窗口。
- 不同星期可有不同窗口。
- `startMinutes < endMinutes`；边界按完整包含处理。
- 未设置 availability filter 时，不因meeting unknown排除section。

### 5.2 occurrence 规范化

每条上游 meeting 必须先规范为一个或多个 occurrence，并保留：

- day/date；
- start/end；
- delivery/mode；
- `required / optional / unknown-requiredness`；
- `recurring / exam / special / other`；
- `known / partial / TBA / invalid`；
- 原始上游字段和normalization reason。

必须兼容 Rutgers 历史中出现过的 Thursday `H` 与 `TH`，且不能用贪婪单字符解析误读 `MTH/TTH`。非法小时/分钟不得进入确定匹配。

### 5.3 判定

对每个明确 required 且具有已知 day/start/end 的 occurrence `m`：

```text
fits(m) = EXISTS availability window w
          WHERE w.weekday = m.weekday
            AND w.startMinutes <= m.startMinutes
            AND m.endMinutes <= w.endMinutes
```

- 任一明确`required`且day/time已知的occurrence不满足：`NO_MATCH`。
- `requiredness-unknown`且day/time已知的occurrence若不满足：`UNCERTAIN`，不是`NO_MATCH`；它可能最终被证明optional。若同时已有明确required反例，仍由AND表得到`NO_MATCH`。
- 所有明确required或requiredness-unknown且day/time已知的potential-required occurrences都满足，并且不存在这两类的TBA/partial/invalid时间：`MATCH`。requiredness未知但已知时间落窗不会制造不确定，因为无论最终是required或optional都不构成反例。
- 无明确required反例，但存在required或requiredness-unknown的TBA/partial/invalid occurrence：`UNCERTAIN`。
- 明确 optional occurrence 不参与排除，但必须在详情显示。
- 明确asynchronous且没有同步的required或requiredness-unknown occurrence：availability为`MATCH`；若存在potential-required同步occurrence，则仍按上面规则判定，不能因async标签跳过。
- “没有meeting数据”不能自动推导async；若无明确async标志则为`UNCERTAIN`。
- Hybrid 的全部同步required occurrences按同一规则逐条检查。
- Exam code本身不是occurrence；若上游提供required exam/special meeting day/time，则照常检查。只有code、没有occurrence时不凭code臆造时间。

旧后端“存在一条meeting符合时间”与旧前端“先丢unknown再every”的组合均不合格。

## 6. Course/Section 信息与直接访问

### 6.1 Course card

至少显示：

- term、campus、subject/course code和number；
- title/expanded title；
- credits、level、core、prerequisite摘要；
- open sections摘要与数据新鲜度；
- `MATCH` section数、`UNCERTAIN` section数；
- 默认展开的匹配sections。

### 6.2 Section summary与完整详情

summary至少显示：复合标识、section number/index、open状态、delivery、教师、meeting摘要、地点、主要限制和watch状态。

直接可访问详情至少显示：

- 所属course与稳定复合section key；
- open状态及更新时间；
- 所有meeting occurrences及其known/optional/exam等分类；
- instructor、delivery、location/building/room；
- exam、permission、eligibility、major/minor/honors；
- notes/comments/subtitle/cross-listing；
- 数据来源、新鲜度、unknown/TBA说明；
- 加入/移出selected、开始/停止watch操作。

必须同时存在：

1. course-centered展开路径；
2. 独立section搜索；
3. 可刷新/可复制的section直接URL；
4. 单section详情查询contract。

旧 `/api/sections` 空数组stub必须删除，不能保留为兼容假表面。

## 7. Section identity 与浏览器选择

- 当前安全外部key：`(term, campus, indexNumber)`。
- 内部可以使用surrogate ID，但不得向用户/API暗示裸index全局唯一。
- selected section上限：每个浏览器session最多9个；第10个必须明确拒绝，不静默替换。
- `selected`与`active`必须分层：selected是用户选择；active只存在于当前页面的显式live watch生命周期。
- 浏览器不得持久保存或自动恢复：`active=true`、server connection ID、未完成watch session、当前audible计数或未确认alarm。
- 旧 `bcsp:subscriptionContact`、`bcsp:subscriptionContactType`、`bcsp:localSoundDeviceId`、`bcsp:localSoundEnabled` 必须迁移/清除，不能让旧值恢复active状态。

### 7.1 公网包与本地一键包的用户状态差异

| 状态 | 公网包以及部署 `PUBLIC_DELTA` | 本地一键包 `LOCAL_ONLY` |
|---|---|---|
| 新页面 | 每次top-level document load（首次打开、新tab、reload）都建立全新用户session | 从本地用户数据目录读取已保存的非active状态 |
| language | 每次按浏览器/系统语言初始化；只允许当前页面内切换，不持久 | 首次按系统初始化；用户选择可跨启动保存 |
| filters / selected sections | 每次新页面恢复默认/空选择，不写入持久browser storage | 可跨启动保存 |
| volume / sound mode / duration / Max audible | 每次新页面恢复产品默认 | 可跨启动保存配置值；已用计数不跨watch session保存 |
| refresh settings | 固定产品值，不提供普通用户修改 | 保存本地两种refresh interval |
| active watch | 页面/连接结束即失效，不自动恢复 | 同样不持久、不自动恢复 |
| notification history | 不持久；只存在于当前页面 | 必须跨本地启动持久存在，直到用户删除或Reset |
| saved view definitions | **不提供该功能**；无入口、definition、storage key、API或隐式URL替代 | 保存在本地用户数据目录，可跨本地启动/browser session使用 |

公网中的WebSocket transport在同一页面内重连，不把页面偏好重置为默认，但也不得自动重新开始已经失效的watch。

### 7.2 本地 Persistent history

本地history是**本地Open episode/watch历史**，不是旧personal subscription/contact/token/DB fanout模型，也不是每秒无限追加一行raw poll：

- 每个section episode至少保存`sectionKey`、首次/最后一次观察为Open的时间、Open观察次数、声音模式、成功audible次数、确认/超时状态、Closed结束时间与所属local run标识。
- 同一episode内每次新的Open observation必须更新`lastObservedAt`和`observationCount`，但可以在一个可审计summary中聚合；Closed→Open才创建下一episode。
- watch start/stop、恢复声音/重置计数与用户确认必须作为有时间戳的有意义动作进入history。
- history不保存contact、账号、token、server connection ID，也绝不能据此恢复active watch。
- history属于该本地安装的用户数据目录；必须支持schema migration。不得未经Review用静默TTL删除；容量/归档UI由P3设计，但Reset前须保持可查询。

### 7.3 Reset 本地用户数据

这里必须区分三个不同作用域，UI文案、入口和确认层级不得混用：

- FilterPanel普通“清除当前筛选/重置筛选”只把`currentFilterState`恢复为产品默认并回到page 1；在本地包中还要解除`appliedViewId/appliedRevision`关联。它不得删除任何本地Saved view definition，也不得清除selected sections、history、语言/声音/refresh设置。公网没有Saved views状态可供操作。
- Saved views manager中的“清空全部Saved views”只删除definition library；当前已经应用的筛选值保持不变并转为无关联custom state，其他用户状态不变。该操作必须有明确作用域和防误触处理。
- 只有下述带确认的“重置本地用户数据”才执行本地安装级用户数据清除，包括Saved view library。

本地WebUI必须提供带确认的“重置本地用户数据”：

1. 停止当前全部active watches并关闭其live映射，避免Reset后仍有不可见订阅。
2. 清除filters、selected sections、saved view library、language override、音量/模式/时长、Max audible设置与计数、refresh设置、episode/watch history和acknowledgement。
3. 恢复与公网新页面相同的默认**current state**；本地Saved view library按Reset定义明确清空。语言重新按系统/浏览器检测，本地refresh恢复10分钟/1秒默认。
4. 不删除课程catalog、应用二进制、服务运行所需配置或诊断日志；这些不是“本地用户数据”，如未来需要维护级清理必须另设明确操作。

### 7.4 本地 Saved views contract

Saved views是当前`REQUIRED / LOCAL_ONLY`能力，用于在本地一键包中把一组精确筛选条件保存为命名preset；它不是Share link、账号同步、结果快照或公网功能。

```text
SavedViewDefinition = {
  id,
  name,
  schemaVersion,
  revision,
  filterSnapshot: { stableFieldId -> canonicalValue },
  createdAt,
  updatedAt
}
```

- `revision`是每个definition的opaque/monotonic concurrency token。Create/duplicate生成新`id`和初始revision；rename/update/delete必须携带expected revision，成功后rename/update递增revision；冲突返回明确状态，不得静默覆盖。
- 必需操作：保存当前筛选为新view、列出、显式应用、重命名、显式更新/覆盖、复制、删除单项、清空全部。Rename只改名称不改snapshot；Update/overwrite必须显式。删除当前关联view或清空library都不清空已应用filters，只解除关联并转为custom state。覆盖与删除必须避免误操作；具体dialog/undo由P7.2决定。
- snapshot只包含term、campus及所有已激活course/section筛选；当前Saved views不保存sort。也不包含pagination page、结果/cache、UI loading/error、`dirtyFields`、selected sections、active watches、audio/refresh/language设置、history或acknowledgement。应用后从page 1重新查询。
- view name经trim后不能为空；同一library中大小写不敏感重名必须要求显式覆盖或改名。当前不设产品数量上限；storage/quota失败必须明确显示，禁止静默淘汰旧view。
- 当前筛选与最近显式应用/保存的view经canonical normalization比较后显示`clean / modified`；不能依赖手工维护的`dirtyFields`列表，否则新增筛选字段时会漏标。Apply建立`appliedViewId + appliedRevision`关联；save/update后为clean。Definition仅rename导致revision变化时，snapshot相等仍是clean，但association revision须刷新。
- 所有筛选字段、默认值、序列化、比较和migration必须由同一个versioned `FilterSchema`/stable field registry驱动。每项至少登记stable ID、type、neutral/default、normalize、validate、query encoder、saved-view codec/migration、chip/i18n metadata和tests；以后新增筛选维度不得在PresetManager再维护第二份字段清单。
- 旧schema view加载时先迁移；旧view缺少未来新增字段时按该字段当时的neutral/default迁移，不应自动标incompatible。新版本unknown/removed field必须保留raw definition、禁止送入query并标`INCOMPATIBLE / NEEDS_REPAIR`。term/campus/dictionary值已失效时同样禁止发出被放宽的假查询，并允许用户修复后另存。
- 公网包不提供Saved views：不渲染入口/文案，不创建definitions或browser storage，不暴露Saved views API，也不以URL filters、Share或自动恢复机制替代。公网仍使用共享`FilterSchema`完成普通筛选、query编码与chips，但不得把本地saved-view codec/manager编入公网制品。
- 本地definitions存入本地用户数据store并参与migration/backup；Reset本地用户数据会删除它们。既然当前filters跨启动恢复，必须同时保存其`appliedViewId/appliedRevision`关联或明确无关联的custom状态，以正确计算dirty；绝不能恢复active watch。
- 本地多页面/并发修改不得last-write-wins静默覆盖；P3必须为definition revision设计CAS/冲突反馈或等价机制。
- 当前明确不做：Share links、URL恢复filter state、账号/cloud sync、多人共享、自动应用default saved view、导入/导出文件。direct section URL仍只是独立访问基础能力。

## 8. WebSocket live watch contract

### 8.1 生命周期

1. 用户选择最多9个sections。
2. 用户以明确手势点击“开始订阅”；同一动作可解锁 AudioContext。
3. 浏览器建立/使用 WebSocket，发送带复合key的start frame；Max audible是浏览器声音策略，不得让服务端据此停止fanout。
4. 服务端验证、逐项返回active/rejected状态，并建立：
   - `connection -> watched sections`
   - `section -> live connections`
5. Open scheduler按已批准catalog/service scope持续运行，不由watch数量决定。第一个watch只建立该section的live fanout/episode consumer；不得额外创建重复Rutgers poll。若scope仍在初始化，UI明确显示尚无成功Open observation，不能用旧状态冒充。
6. stop、页面关闭、socket关闭、heartbeat超时、服务停止都会清理active映射。
7. 客户端可以重连transport，但不得自动重新发送旧active watches；UI显示“连接已断开，订阅已停止”，用户必须再次明确开始。
8. 每次用户明确开始某section watch时，该section的ONE_SHOT audible计数归零；本地history只记录这次动作，不把旧session复活。

### 8.2 消息

每个Open信息**attempt**都产生第11节的`RefreshObservation`/`refresh.status`，但只有通过schema与empty/error安全检查、成功reconcile的响应才产生per-section `OpenObservation`。失败保留last-known status，只更新时间截点/失败状态和计数，不产生`UNKNOWN`冒充新状态，也不触发episode转换。有效状态即使未变化，也不得用旧45秒cache把这次结果隐藏。

发给live watcher的section observation至少包含：

```text
{ protocolVersion, observationId, sectionKey, status, observedAt,
  sourceFreshness, pullSequence,
  counterSnapshot: {
    runCounts?: { attempted, succeeded, failed, empty },
    todayCounts: { attempted, succeeded, failed, empty }
  }
}
```

- `status`必须是`OPEN / CLOSED / UNKNOWN`；`UNKNOWN`只表达一个有效观察中的真实未知状态或尚无可判定状态，不能用来包装HTTP/parse/timeout失败。
- 公网可以省略`runCounts`，但必须提供`todayCounts`；本地必须提供两者。也可以用同protocol version的独立`refresh.status` frame传递counter snapshot，但UI语义和字段不得缩水为一个无分类总数。
- `OpenObservation`是每次有效且安全reconcile的pull产生的section结果；`OpenEpisode`是声音确认所需的状态段；`AudibleNotification`是由声音模式决定的输出。三者不得混为一个event。
- 每个Open observation都更新UI时间戳/当前结果与本地history summary；是否发声由第9、10节决定。
- 同一网络frame的精确重放可按相同`observationId`做transport幂等；这不是产品防抖，也不能吞掉下一次真实pull。
- 上游在后续poll仍返回Open时，必须产生新的`observationId`；ONE_SHOT可据此发下一声，CONTINUOUS不得因此创建同一section的新episode。
- 多个浏览器watch同一section时，Rutgers只由集中poller获取一次，再广播给所有live watchers。

### 8.3 Open episode 状态机

每个active section在一次显式watch session中独立维护episode：

```text
initial/closed --OPEN--> OPEN_EPISODE(unacknowledged)
OPEN_EPISODE --OPEN--> same episode; update lastObservedAt/count
OPEN_EPISODE(unacknowledged) --confirm--> acknowledged
OPEN_EPISODE(unacknowledged) --duration_elapsed--> timed_out
acknowledged/timed_out --OPEN--> same episode; do not retrigger CONTINUOUS
timed_out --explicit resume while still OPEN--> unacknowledged/ringing
any OPEN_EPISODE state --CLOSED--> episode closed
closed --OPEN--> new episode; CONTINUOUS may ring again
```

- watch开始后的第一次已观察`OPEN`可以创建初始episode。
- `acknowledged`表示用户确认，`timed_out`表示有限duration耗尽，history不得把两者混为同一原因。两者都不会因同episode后续Open自动重响；只有`timed_out`可由显式resume恢复当前episode，或先观察到`CLOSED`、以后再`OPEN`创建新episode。`UNKNOWN`不能冒充`CLOSED`。
- `CLOSED`、`UNKNOWN`同样进入当前结果和时间戳；错误/无数据不是合法`CLOSED`转换。
- 新section D形成未确认episode时，即使A的旧episode已确认，D也必须立即进入可发声集合。

## 9. Toast 与 Max audible notifications

### 9.1 Max audible notifications

- 配置粒度：每个selected section。
- 作用域：仅`ONE_SHOT`模式中、一次明确开始的active watch session。
- 默认：3。
- 允许值：任意正整数；产品不得设置1–10之类的上限。`0`不是Max值，静音使用音量0。
- 计数对象：该section成功开始播放的ONE_SHOT cue。音量0、autoplay blocked或开始播放前失败不消耗计数；一旦cue已成功排程/开始，即使随后被用户中断也计一次。
- 第N声仍完整播放；达到N后只停止该section后续ONE_SHOT声音，watch、poll/fanout、toast、UI状态、时间戳和本地history全部继续，该section显示“静默提醒（已达Max audible）”。
- 新的显式watch start将该section计数归零。用户也可在watch不停的情况下执行“恢复声音/重置计数”，把计数归零并立即解除静默状态。
- 多sections分别计数，一个section达到上限不影响其他sections；从ONE_SHOT切到CONTINUOUS不消耗计数，同一active session切回ONE_SHOT时保留原计数。

### 9.2 Toast

- 新Open episode必须产生可感知toast/alert，明确显示section，使用户无需从声音中辨认课程。
- 同一section同一episode内的后续Open observations更新同一alert的`lastObservedAt`与`observationCount`，不得每秒无限堆叠toast；这属于聚合展示，不是丢弃观察结果。
- ONE_SHOT下可同时显示当前`audibleCount / Max audible`与静默原因；CONTINUOUS下显示episode是否待确认/已确认/已超时。
- toast手动关闭只关闭视觉浮层，不停止watch、不改变ONE_SHOT audible计数；只有明确的episode“确认/关闭声音”动作才确认CONTINUOUS episode。
- UI可为了布局限制同时可见数量并提供alert/history列表，但不得静默丢失episode或把“最多可见toast数”混称为Max audible notifications。
- 是否自动消失、动画和视觉堆栈由P7.2/P7.3决定，但必须满足键盘、screen reader和reduced-motion。

旧固定 `MAX_TOASTS=5` 是显示实现，不是Max audible notifications，不能继承为产品语义。

## 10. 声音 contract

### 10.1 共同规则

- WebUI只播放通用提醒声；不朗读课程名、不依赖email、系统通知或原生App。用户在UI的alert/section状态中查看是哪门课Open。
- 音量范围0–100；0等同静音，但Open observation、toast、状态和history仍继续。是否跨页面保存遵循第7.1节的包差异。
- 模式：`ONE_SHOT / CONTINUOUS`；保存模式不等于保存active状态。
- WebUI打开且浏览器正常运行时保证；页面关闭、锁屏、系统挂起不保证。
- 不允许为多个sections叠加无界并行音轨；由一个browser audio mixer统一调度。

### 10.2 一声提醒

- 每条不同的Open observation尝试播放一次有限时长cue，直到该section达到Max audible。
- 达到Max后仍接收每次Open observation，但只做静默提醒；不得停止watch。
- burst中的N条可发声Open observations应产生N次有序trigger；实现可以安全排队，不能把整个batch合并为一次。若用户静音或音频失败，按第9.1节不消耗成功audible计数。

### 10.3 持续提醒

- CONTINUOUS使用“闹钟确认”语义，只由第8.3节的**新Open episode**启动，不受Max audible限制。
- 默认持续时长为10分钟；用户可配置有限正时长或`UNLIMITED`。P3可以设计输入控件和安全音频实现，但不得静默把`UNLIMITED`改成有限上限。
- 每个section episode有独立的`unacknowledged / acknowledged / timed_out / closed`状态；共享mixer在至少一个可发声、未确认episode存在时持续播放。
- 用户可以逐section“确认/关闭当前提醒”，也可以“全部确认”。确认A后，若C仍未确认则声音继续；若没有未确认episode则声音停止，但active watches都继续。
- 同一episode持续Open时，后续每秒观察只更新时间/次数，不重新启动已确认或已超时的声音。A必须先被可靠观察为Closed，以后再次Open才创建新episode并重新响。
- 若D在A已确认后首次变Open，D的新episode必须立即启动/恢复共享mixer。
- 有限持续时长到期时，该episode进入`timed_out`并停止为本episode发声；它不会因下一次仍Open的观察重新响，直到Closed→Open或用户显式“恢复当前episode声音”。
- 停止某一section watch会结束该section当前episode；若其他section仍有未确认episode，alarm继续。切换到ONE_SHOT、静音、连接结束或页面卸载会停止当前loop并在UI中显示相应原因。

### 10.4 Audio failure

- Start watch/Test sound手势用于解锁AudioContext。
- autoplay blocked、AudioContext失败或播放异常必须显示明确状态；toast、watch、观察结果和history继续，失败前未开始的cue不消耗Max audible。
- 不因旧HTTP claim的“先标sent”语义丢失整个用户反馈链。

## 11. 双刷新、时间截点与拉取计数

### 11.1 两种独立上游刷新

| 刷新 | 本地一键包 | 公网包以及部署 |
|---|---|---|
| 课程信息（catalog） | 默认10分钟；用户可配`1–1440`分钟 | 固定10分钟；普通用户不可修改 |
| Open信息（openSections） | 默认1秒；用户可配`1–3600`秒 | 固定1秒；普通用户不可修改 |

- Open scheduler的target是已批准并已初始化的catalog/service `term×campus` scope；即使没有active watch也持续刷新，供course card、filter、section detail和首次watch读取。Active watch只决定哪些section observation经WS fanout并触发浏览器提醒，不决定上游是否poll。
- 真实数据/限流证据若证明固定值不可安全实现，必须停止并回到共同Review；不得在实现中静默改默认、范围或公网固定值。
- 两种scheduler、状态、错误与时间戳彼此独立。browser query revalidation是catalog成功后的消费行为，不是第三种上游刷新。
- 本地WebUI可以请求立即执行一次目标scope的刷新/重试，但必须受per-target single-flight、coalescing与cooldown保护，连续点击不得放大为并行Rutgers请求；公网普通用户刷新页面或查询不得直接放大为一次Rutgers上游请求。

### 11.2 每次刷新都是新的结果

每个上游target请求都产生一条新的可观察记录，即使payload hash、课程字段或Open/Closed状态完全未变：

```text
RefreshObservation = {
  kind, targetScope, sequence, startedAt, completedAt,
  outcome: SUCCESS | EMPTY_VALID | FAILURE | PARTIAL,
  sourceObservedAt?, payloadHash?, changedCount?, errorClass?
}
```

- 课程信息刷新必须按target staging/transaction处理，不得因一个term/campus失败或刷新而truncate其他scope；多target批次允许`PARTIAL`，但必须逐target显示结论。
- 首次没有可用catalog时，UI必须显示初始化/配置/进度/错误，不显示fallback假课程。
- 成功课程刷新发布新的refresh checkpoint；只有数据真的变化时才需要变更内容version，但即使无变化也必须更新“课程信息时间截点”。相关browser query随后revalidate。
- Open attempt每次完成都更新attempt时间/结果与计数；只有成功且安全reconcile后才更新“Open信息时间截点”和per-section当前结果。合法重复Open仍产生新observation；failure或尚未通过empty guard的空响应保留last-known status，不得伪造UNKNOWN/CLOSED。
- UI主时间截点使用最近一次**成功完成**时间；若最新一次尝试失败，必须同时显示失败尝试时间/状态，不能用失败时间冒充数据新鲜度。存在可靠上游时间时可另显示`sourceObservedAt`。

### 11.3 Open pull 计数

- 计数单位是一次已发起的Rutgers `openSections` target请求，不是section数、Open数、WebSocket消息数或浏览器数。
- UI至少显示`attempted / succeeded / failed`，避免“拉取次数”掩盖失败；`EMPTY_VALID`计为attempted+succeeded并另标empty。
- 公网显示服务端在当前Rutgers自然日内的计数，并跨服务重启保持。
- 本地显示`当前运行`计数（进程启动时归零）与`今天`计数（跨本地重启保持）。
- “今天”统一按`America/New_York`日界线计算，并在EN/zh-CN UI中明确标注时区；不得使用无标签的服务器本地日期。
- 多浏览器watch同一target仍只增加一次上游pull计数；集中poll后fanout不重复计数。

旧15–120秒普通用户auto-refresh toggle和无界45秒Map cache继续排除，不能覆盖上述新的双刷新contract。

## 12. I18N contract

- 当前产品至少完整支持`en-US`与`zh-CN`；`en-US`是英文canonical locale，`zh-CN`是简体中文canonical locale，fallback为`en-US`。
- 公网每个新页面按浏览器首选语言/系统语言初始化，不持久用户选择；本地可持久语言选择，Reset后重新按系统检测。
- `html lang`、日期/时间/数字格式、pluralization、loading/empty/error/disabled、filters、toast、history、refresh、audio与Reset状态必须与当前locale一致并保持message-key parity；本地包还必须覆盖Saved views全部状态，用户自定义view名称保持原文；公网catalog/bundle不得包含可达Saved views产品表面。
- Rutgers原始课程标题、教师、地点、代码与raw provenance保持原文，不做未经来源支持的机器翻译；产品壳、解释、错误与操作标签双语。
- 本地quickstart/troubleshooting至少提供EN与zh-CN，或提供一份内容等价、可明确切换的双语文档；公网普通用户流程同样不得只翻译happy path。

## 13. 当前订阅相关非目标

以下明确不进入当前runtime/package：

- email、SMTP、SendGrid、mail config/UI/worker/templates/tests/docs；
- Discord、Web Push、原生App、系统通知；
- 服务端持久个人active subscription/contact/token；
- quiet hours、snooze、paused/suppressed个人状态机；
- 旧personal notification queue/history schema；本地第7.2节的无contact episode/watch history明确保留，公网history不持久；
- waitlist filter/alert、Share links与Named Compact view；
- 页面关闭/系统挂起后的声音保证。

邮件只在GitHub future feature中记录，不以死代码、隐藏route、模板或配置样例保留在当前产品树/包中。

## 14. 必需验证映射

| Contract | 必需测试 |
|---|---|
| 跨维度AND、同维度OR、core ANY/ALL | query golden + property tests |
| 同一个section见证 | 两个不同sections分别满足不同条件的反例 |
| availability | 多星期/多窗口、边界相等、一个required窗内一个窗外、H/TH、invalid time、三值AND/OR真值表 |
| unknown | TBA、partial、无meeting但非explicit async、requiredness-unknown已知且窗外→UNCERTAIN、已知且窗内不制造反例 |
| delivery | raw tuple、modality/synchronicity两轴、generic Online unspecified、on-campus、sync、async、hybrid、other/unknown、不静默丢值 |
| section路径 | independent search、direct URL reload、detail字段、course expand一致性 |
| results | MATCH/UNCERTAIN/NO_MATCH、other section reasons、server total/page一致 |
| identity | 相同index跨term/campus collision fixture |
| package state | 公网每次document load为新session且无Saved library；本地prefs/history跨启动；active从不恢复；本地普通筛选Reset不删library，只有带确认的本地用户数据Reset停止watch并清用户数据 |
| saved views | 仅本地：CRUD/duplicate/apply/page1、normalized dirty state、revision/CAS conflict UI与tests、普通筛选Reset不删definitions、delete-all只删library且保留当前filters、local persistence/用户数据Reset、schema migration/incompatible、quota error、无URL/share/cloud sync；公网入口/API/storage/key/bundle string零存在 |
| watch | 第10项拒绝、start/stop、disconnect/heartbeat cleanup、reload不active、显式restart清audible计数 |
| poll/fanout | 无watch仍按scope poll、多连接不增加上游请求、每attempt新refresh记录、valid/safe pull新observation、重复frame幂等、失败/unsafe empty不转状态 |
| max | ONE_SHOT默认3、任意正整数、无产品上限、第N声后仅静默、watch/toast/history继续、恢复声音/重置计数 |
| toast | 新episode可感知、同episode聚合更新时间/次数、关闭不改watch/count、history不丢episode |
| continuous | initial/Closed→Open新episode、同episode确认后不重响、A/C/D多section、10m/Unlimited、timeout/resume、confirm all |
| audio | 通用WebUI声音、N可发声observations=N one-shot triggers、volume0/autoplay failure不消耗成功audible计数 |
| refresh | local 10m(1–1440m)/1s(1–3600s)、public fixed、每次新结果、time checkpoints、run/today计数、时区、partial/error |
| i18n | `en-US`/`zh-CN` key parity、system detection、public不持久/local持久+Reset、locale格式与raw原文 |
| real-data gate | raw profile、Delivery mapping、H/TH、collision/join、FTS/instructor/eligibility/permission、empty/error、请求预算与provenance |

## 15. P2 Review 批准结论

| P1 unresolved | P2 结论 |
|---|---|
| 最终筛选字段全集 | 第4节给出候选ALL集合与明确非目标；历史概念覆盖已审计，但端到端完整性和当前raw取值仍须真实数据门证明 |
| async/TBA/hybrid/optional/exam | 第5节给出三值、requiredness与occurrence规则 |
| Compact/saved/share/waitlist | Compact、Share、Waitlist明确排除；Saved views为本地一键包`REQUIRED / LOCAL_ONLY`并遵循第7.4节，公网不提供；URL只作direct section导航基础能力，不恢复filters |
| browser auto-refresh/cache | 第11节双刷新、时间戳和计数取代旧toggle/45s cache；公网固定，本地可配 |
| toast/Max audible notifications | 第9节给出仅ONE_SHOT的成功audible计数、静默不停止watch、恢复声音语义 |
| persistence/history/quiet/snooze | quiet/snooze排除；公网新页面current state无持久且无Saved views；本地prefs/definitions/episode-watch history持久且可Reset；active从不持久 |
| composite key | 当前采用 `(term,campus,index)` |
| continuous sound | 第8.3与10.3节给出episode、确认、Closed→Open重触发、A/C/D多section、10分钟/Unlimited语义 |
| i18n | 至少`en-US`和`zh-CN`；公网按系统且不持久，本地可持久并由Reset清除 |

本文件已随P2于2026-07-13获得用户最终批准。第4.5节的真实数据门仍是P3/P4冻结设计前的必经输入；真实限流、容量与端到端延迟仍按权威流程在P4/P7验证。Saved views为`REQUIRED / LOCAL_ONLY`，公网明确不提供。P2批准使P3具备启动资格，但不自动启动P3，也不把真实数据假设视为已经证明。
