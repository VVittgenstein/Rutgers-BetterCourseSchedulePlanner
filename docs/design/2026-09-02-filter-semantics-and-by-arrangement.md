# 筛选条件全集、判定逻辑，与「时间另行约定」取值的新增

状态：ACTIVE（2026-09-02 全部裁定并实施）。日期：2026-09-02。
现行派生规则版本：`CATALOG_DERIVATION_VERSION = 2`。
本提案要求：`CATALOG_DERIVATION_VERSION = 3` 与迁移 `0008_by_arrangement_synchronicity.sql`。

## 0. 这份文档回答两件事

1. **现状**：产品一共有哪些筛选条件，每一条到底怎么判，它们怎么组合。
2. **提案**：这次准备加的「上课时间另行约定」是什么，影响面多大，代价多少。

第 1 部分是对现有代码的如实描述，不是设想。每一条都标了源码位置。

## 1. 三值判定

每一条筛选条件对一个候选对象产出三种结果之一
（`crates/bcsp-query/src/evaluation.rs`）：

| 结果 | 含义 |
| --- | --- |
| `MATCH` | 证据充分且符合 |
| `NO_MATCH` | 证据充分且不符合 |
| `UNCERTAIN` | 证据不足以判断（缺字段、待定、冲突、数据过期） |

聚合有两个算子：

- `and_all`：任一 `NO_MATCH` → `NO_MATCH`；否则任一 `UNCERTAIN` → `UNCERTAIN`；
  否则 `MATCH`。**空集 = `MATCH`**（未启用的条件是恒真）。
- `or_active`：空集或任一 `MATCH` → `MATCH`；否则任一 `UNCERTAIN` → `UNCERTAIN`；
  否则 `NO_MATCH`。

## 2. 「判定结果」和「是否进入结果集」是两回事

`admit_filter_evaluation`（`crates/bcsp-query/src/predicates.rs:426`）决定一行
是否留在结果里，规则是：

- `NO_MATCH` → 一律剔除；
- `MATCH` → 一律保留；
- `UNCERTAIN` → **默认保留**，只有三条例外，它们在本条件启用时被开关控制：

| 条件 | 开关 |
| --- | --- |
| 先修要求（FLT-C09） | `includeIncomplete.prerequisite` |
| 授课方式（FLT-S04a） | `includeIncomplete.modality` |
| 同步方式（FLT-S04b） | `includeIncomplete.synchronicity` |

**这是理解全部筛选行为的关键**：其余 15 条的 `UNCERTAIN` 一律静默放行，
用户既看不到也关不掉。学分为 `BA`、学科缺失、上课时间待定（TBA）、
空位数据过期——这些都会让对应课程/课节照常出现在结果里。

## 3. 课程级条件（9 条，按课程 variant 判定）

来源：`course_filter_evaluations`（`predicates.rs:59`）。

| ID | 界面 | 判定规则 | `UNCERTAIN` 来源 |
| --- | --- | --- | --- |
| FLT-C01 | 学期 | `group.term == filters.term`，必填，恒启用 | 无 |
| FLT-C02 | 校区 | 空集=不启用；否则 `group.campus ∈ 集合` | 无 |
| FLT-C03 | 学科 | `variant.subjectCode ∈ 集合`（精确） | 字段缺失/未知 |
| FLT-C04 | 关键词 | 分词后 token-AND；命中课程标识符优先排序（`text.rs`） | 无（命中或不命中） |
| FLT-C05 | 课程号段 | `课程号 → 百位段 ∈ 集合` | 号码无法解析、字段缺失 |
| FLT-C06 | 课程层次 | `variant.level ∈ 集合`（ASCII 大写折叠） | 字段缺失/未知 |
| FLT-C07 | 学分 | **完全包含**：课程的 `[min,max]` 必须落在所选区间内 | 学分为 `BA`（按安排）、无法解析、缺失 |
| FLT-C08 | 核心课程体系 | `ANY` = `or_active`，`ALL` = `and_all`；代码两侧 ASCII 大写折叠后精确比对 | 课程未登记核心代码 |
| FLT-C09 | 先修要求 | `ANY` 恒真；否则与 `preReqNotes` 是否为空比较 | 先修状态未知 |

要点：

- **FLT-C07 是完全包含，不是相交**。一门 1–4 学分的课在「3–3」区间里
  会被判 `NO_MATCH`。
- **FLT-C09 的「无先修要求」= Rutgers 的 `preReqNotes` 字段为空字符串**。
  写在课程描述里的先修条件不在此列。
- **FLT-C08 的 `ALL` 在真实数据里几乎必然为空**（如 `NS ∧ QR` = 0 门）。

## 4. 课节级条件（9 条，按课节判定）

来源：`section_filter_evaluations`（`predicates.rs:118`）。

| ID | 界面 | 判定规则 | `UNCERTAIN` 来源 |
| --- | --- | --- | --- |
| FLT-S01 | Index 号 | `section.index ∈ 集合` | 无 |
| FLT-S03 | 开放状态 | 证据新鲜且状态确定 → `状态 ∈ 集合`；否则若集合含 `UNKNOWN` → `MATCH`，否则 `UNCERTAIN` | 数据过期、正在刷新、熔断 |
| FLT-S04a | 授课方式 | `线下 / 在线 / 线上线下混合` 之一 ∈ 集合 | `Other`、`Unknown`、模式与地点冲突 |
| FLT-S04b | 同步方式 | `同步 / 异步 / 同步异步混合` 之一 ∈ 集合 | `时间另行约定`、`Unspecified`、`Unknown` |
| FLT-S05 | 教师 | 教师名集合匹配（空白折叠后精确；取值来自动态字典） | 教师字段缺失 |
| FLT-S06 | 可上课时间 | 见 4.1 | 时间待定/缺失/非法；课节无时段记录且不是异步 |
| FLT-S07 | 上课地点 | 所有「需要到场」的时段都必须在所选集合内（跳过线上/远程与显式可选时段） | 无时段记录，或全部时段都不需到场 |
| FLT-S09 | 考试代码 | 代码或代码文本任一命中 | 字段缺失 |
| FLT-S10 | 许可要求 | `specialPermissionAddCode` 非空 = 需要许可 | 字段缺失 |

### 4.1 FLT-S06 可上课时间（`availability.rs`）

规则一句话：**该课节列出的每一次上课，都必须整段落在你添加的某一个时间段之内。**

逐条展开：

- 时段的「是否必修」在 Rutgers 数据里 100% 是未知
  （23,727/23,727），因此**未知按必修处理**；只有显式 `OPTIONAL` 的时段被跳过。
- 时段本身标为异步（`ExplicitAsync`）→ 直接通过，不看时间。
- 时间待定/缺失/非法 → `UNCERTAIN`（默认放行）。
- 课节完全没有时段记录 → 课节是异步则 `MATCH`，否则 `UNCERTAIN`。
- **相邻时间段不合并**：`周四 15:20–17:00` 与 `周四 17:00–19:30` 两段，
  不能容纳一节 `16:00–18:00` 的课。这是**故意的**，由
  `adjacent_windows_cannot_be_stitched_and_invalid_time_is_uncertain` 冻结。
- 时间窗**同样作用于有固定时间的在线课**（在线同步）。在线异步课没有时间，
  自动通过。
- 空集 = 不启用；添加第一个时间段是收窄，之后每加一个时间段只会放宽。

## 5. 组合规则

来源：`engine.rs:640-690`。

1. 课程级条件对每个 variant 求值。
2. 课节级条件对每个课节求值，**九条一起对同一个课节求值**
   （same-section 语义：不能课节 A 满足时间、课节 B 满足同步方式）。
3. variant 进入结果，当且仅当：课程级全部 `admitted` **且** 至少一个课节
   `admitted`（课节级条件全部未启用时该要求自动满足）。
4. variant 之下**只列出 `admitted` 的课节**，被筛掉的课节不出现。
5. 课节行上显示的判定 = `and_all(课程级判定, 该课节判定)`，所以课程级的
   不确定会重复出现在每一行上。

例外：**课程详情页（`/api/v1/query/course-detail`）不接收筛选条件**，
它列出该课程的全部课节并一律标 `MATCH`。已在前端加提示说明。

## 6. 本次提案：新增「时间另行约定」

### 6.1 依据

Rutgers 官方 Schedule of Classes 的显示逻辑（`soc_utils.js` / `soc_app.js`）：

```js
isByArrangementMeetingTime(t)  →  t.baClassHours === "B"
isOnlineOrRemoteMeetingTime(t) →  t.campusLocation === "O" || t.campusLocation === "T"

if (isByArrangement(t)) {
    if (isOnlineOrRemote(t))  显示 "Asynchronous content"
    else                      显示 "Hours by arrangement"
}
```

Rutgers 把「时间另定」分成两类，只有同时是线上/远程的那一类才叫
asynchronous。本产品现行的 `by_arrangement` 判据
（`delivery.rs:115`）与之一致，**不需要改**。

问题在另一头：非线上的那一类，Rutgers 有明确说法「Hours by arrangement」，
而本产品把它归入 `Synchronicity::Unknown`，界面显示「未知」。
「未知」比官方口径更没有信息量，而且对读者是误导——不是我们不知道，
是 Rutgers 明确说了「时间另行约定」。

### 6.2 影响面（92026/NB 实测；实施后的实际结果见 11）

现有「线下 · 未知」课节 4,575 个，其构成：

| 时段类型 | 数量 |
| --- | --- |
| `23 RSCH-MA` 研究 | 1,744 |
| `19 PROJ-IND` 独立项目 | 1,363 |
| `08 MUS-INDV` 个人音乐课 | 522 |
| `80 GRADUATE 800-LEVEL` | 377 |
| 其余（音乐小组、临床、实习、荣誉、海外、田野…） | ~500 |
| `02 LEC` / `05 LAB` | 62 |

其中 **4,570 个时段的 `baClassHours == "B"`**，只有 3 个是真坏数据
（时间字段非法）。全库（所有学期校区）范围内，`baClassHours == "B"` 且
无时间的时段共 8,516 个，现被判为 `UNKNOWN` 6,053、`UNSPECIFIED` 171、
`ASYNC` 2,292（后者是已正确处理的线上课）。

### 6.3 规则改动（唯一一处）

`normalize_occurrence`（`delivery.rs`）的同步性判定新增一条分支：

| 条件 | 现在 | 改后 | `normalization_reason` |
| --- | --- | --- | --- |
| `baClassHours == "B"`、无时间、**非**线上/远程、模式码可识别 | `Unknown` | `ByArrangement` | `OFFICIAL_BY_ARRANGEMENT_ONSITE` |

其余分支一律不动。线上的 `B`（`campusLocation` 为 `O`/`T`，或 mode 90）
继续判 `Asynchronous`，与 Rutgers 一致。

课节级聚合（`classify_delivery`）随之扩展：同质集合保留该值；
`ByArrangement` 与 `Synchronous` 混合的课节仍然按现行规则处理
（见 6.4 的待定项）。

### 6.4 两个设计选择（已裁定：A1 + B1）

**选择 A：新取值是否可被用户勾选？**

- **A1（采纳）不可勾选，只作显示**。同步方式仍是三个选项，
  `ByArrangement` 在筛选中按 `UNCERTAIN` 处理，与今天的 `Unknown` 完全一致，
  由「包含数据不完整的记录」开关控制。
  → **任何筛选结果都不改变**，纯粹是把「未知」换成「时间另行约定」。
- **A2 可勾选**，同步方式变成四个选项。
  → 对「同步+异步」这类现有选择结果不变（不在集合里 = 剔除，和今天一样）；
  但对**已勾「包含数据不完整的记录」**的用户是行为变化：那些课节不再由
  该开关放回，必须改勾新选项。

**选择 B：混合课节怎么算？**

一个课节若同时有「固定时段」和「时间另行约定的线下时段」，
今天是 `Unknown`。改后：

- **B1（采纳）** 仍是 `Unknown`——它既不是纯粹的固定时间，也不是纯粹的另行约定，
  且 `Mixed` 的既有定义严格是「同步 + 异步」，不应扩义。
- **B2** 扩展 `Mixed` 的定义。**不推荐**：会改变现有 `Mixed` 的语义，
  且与 Rutgers 无对应。

### 6.5 代价与可逆性

- `CATALOG_DERIVATION_VERSION` 2 → 3。
- **需要新迁移**（起草时判断错误，实测才发现）：四张目录表的
  `synchronicity` 列带 `CHECK (synchronicity IN ('SYNC','ASYNC','MIXED',
  'UNSPECIFIED','UNKNOWN'))`，写入新值直接触发约束失败。SQLite 不能就地改
  CHECK，因此新增 `0008_by_arrangement_synchronicity.sql`，按
  0005 的先例做标准十二步表重建（`catalog_sections`、`catalog_occurrences`
  及两张 staging 表；仅放宽该 CHECK，其余列、约束、键、索引原样重建）。
- 首次启动自动完成迁移 + 原地重算。用户那份 452 MB 库实测 **5.5 秒 / 6 个
  target**。
- **不可逆**（起草时判断错误）。迁移账本多一条 id=8 的记录后，0.1.2 打开该
  库会以 `UnknownMigration { migration_id: 8 }` 失败关闭。`rederive.rs` 那条
  「更高 stamp 也会被重新推导」只覆盖派生版本，不覆盖迁移账本。
  **升级前务必备份 `data` 目录。**
- 需要同步改动：`CatalogSynchronicity` 契约取值、前端 TS 类型与
  `presenter.ts` 映射、中英文案、相关测试（`delivery.rs` 单测、
  `normalization.rs` 集成测试、前端 i18n 契约测试）。
- 若选 A2，还需改 `UserSynchronicityV3`、请求契约与
  `schema.rs` 中冻结的 `["SYNC","ASYNC","MIXED"]` 描述符。

## 7. 已裁定（2026-09-02，产品所有者）

| # | 议题 | 裁定 | 理由 |
| --- | --- | --- | --- |
| 1 | 相邻/重叠时间段是否合并 | **不改** | 同一段空闲本来就应该一次输入；分两段输入是使用方式问题，不是程序缺陷。现行冻结测试保留。 |
| 2 | 上课时间待定（TBA）是否可排除 | **不改** | 系统标定为待定的，一律按通过处理。将来可能作为独立的筛选条件加入，本轮不做。 |
| 3 | 学分是否改为区间相交 | **不改** | 完全包含才是想要的语义：设 1–2 是为了找确实是 1.5 学分的课，不应把 1–4 学分的浮动课带进来。 |
| 4 | 上课地点的匹配语义 | **要改**，细则见 8 | 选了 College Avenue 就意味着排除还要去别的校区的课节。 |

## 8. 上课地点（FLT-S07）语义修订

### 8.1 现状与问题

默认模式 `ANY_MEETING`：只要**有一次**课在所选地点就算命中。选 College Avenue
会返回 1,592 个课节，其中 78 个还要求你去另一个实体校区上课。这与
「选了就等于排除其它」的意图相反。

### 8.2 实测构成（92026/NB，含 College Avenue 时段的课节）

| 该课节其余时段落在哪 | 课节数 |
| --- | --- |
| 仅 College Avenue | 1,208 |
| College Avenue + 线上（无其它实体校区） | 306 |
| College Avenue + 另一个实体校区 | 78 |

### 8.3 修订方案（已实施）

判定改为：**该课节所有「需要到场」的上课，其地点都必须在所选集合内。**
线上/远程时段（`campusLocation` 为 `O`/`T`）不产生通勤要求，因此被跳过，
这与 FLT-S06 跳过异步时段是同一条原则。

结果：选 College Avenue 得到 1,514 个课节 —— 排除了那 78 个跨校区的，
保留了 306 个「线下只在 College Avenue、另有线上时段」的。

现行 `ALL_REQUIRED_MEETINGS` 模式把线上时段也算进要求，得到 1,208 个，
过严；本方案取代它。

### 8.4 模式选择器的去留

`ANY_MEETING` 在「选了就等于排除其它」的模型下没有正当用途，界面上的
「子校区匹配方式」下拉已移除，两个模式值现在得到同一个判定。
`MeetingLocationFilterV2::mode` 字段在契约里保留，只为让已保存的筛选状态
继续反序列化；它不再改变任何结果。

## 9. 实施清单（Rust）

| 文件 | 改动 |
| --- | --- |
| `crates/bcsp-contracts/src/catalog.rs` | `CatalogSynchronicity` 新增 `ByArrangement`，wire 值 `BY_ARRANGEMENT` |
| `crates/bcsp-catalog/src/model.rs` | `Synchronicity` 新增 `ByArrangement` 及契约映射 |
| `crates/bcsp-catalog/src/delivery.rs` | `onsite_hours_by_arrangement` 判据、同步性新分支、`OFFICIAL_BY_ARRANGEMENT_ONSITE` reason、课节聚合注释 |
| `crates/bcsp-catalog/src/rederive.rs` | `CATALOG_DERIVATION_VERSION` 2 → 3 |
| `crates/bcsp-operational-storage/src/storage.rs` | 同步性 wire 白名单新增 `BY_ARRANGEMENT` |
| `crates/bcsp-operational-storage/migrations/0008_by_arrangement_synchronicity.sql` | 四张目录表重建，放宽 `synchronicity` 的 CHECK |
| `crates/bcsp-operational-storage/src/migration_bundle.rs` | 迁移 0008 的字节级镜像 |
| `crates/bcsp-contracts/tests/golden/contract-manifest-v1.json` | `bcsp.catalog.synchronicity.v1` 枚举新增 `BY_ARRANGEMENT` |
| `crates/bcsp-query/src/predicates.rs` | `evaluate_synchronicity` 把新值按 `UNCERTAIN` 处理；`evaluate_meeting_location` 改为单一规则并跳过线上时段 |

前端：`contracts/catalog.ts` 类型、`presenter.ts` 映射、
`filter.option.by_arrangement` 中英文案、`filter.meeting_location_helper`
说明、移除子校区匹配方式下拉。

## 10. 本次一并保留的前端修复（已完成，未提交）

| 改动 | 原因 |
| --- | --- |
| 可上课时间说明文案（中英） | 原文把规则说反了 |
| 核心课程体系条件摘要只显示代码，`ANY/ALL` 本地化 | `WCr · Writing and Communication, Revision` 的内嵌逗号被读成第四项 |
| 课程详情页加「本页不应用搜索条件」提示 | 详情接口不收筛选条件，会列出被筛掉的课节 |
| 中文 `混合` 拆成 `线上线下混合` / `同步异步混合` | 两个不同维度共用一个词 |
| `完整数据显示` → `包含数据不完整的记录` | 原标签字面含义与实际功能相反 |

## 11. 实施后的实测结果

在用户 452 MB 真实数据库的副本上运行新二进制：迁移 0008 + 派生 v3 重算共
**5.5 秒**，6 个 target 全部盖到 `derivation_version = 3`。

92026/NB 课节分布（重算后）：

| 授课方式 | 同步方式 | 课节数 |
| --- | --- | --- |
| 线下 | 同步 | 5,279 |
| 线下 | **时间另行约定** | **4,461** |
| 在线 | 异步 | 1,150 |
| 线上线下混合 | 同步异步混合 | 462 |
| 在线 | 同步 | 241 |
| 线下 | 未知 | 114 |
| 其余 | | 280 |

改动前「线下 · 未知」为 4,575；其中 4,461 个获得了 Rutgers 明确公布的说法，
剩下 114 个是真正判不出来的（既有固定时段又有另行约定时段，或时间字段非法）。
全库共 5,979 个时段带上新的 `OFFICIAL_BY_ARRANGEMENT_ONSITE` reason。
