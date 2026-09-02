# 在线 Section 同步性推导 v2 与既有数据库的原地重推导

状态：ACTIVE。日期：2026-09-01。
派生规则版本：`CATALOG_DERIVATION_VERSION = 2`（`crates/bcsp-catalog/src/rederive.rs`）。
迁移：`crates/bcsp-operational-storage/migrations/0007_catalog_derivation_state.sql`。

## 0. 问题（P1）

NB Fall 2026 的 1,438 个 ONLINE section 里有 1,291 个存成
`synchronicity = UNSPECIFIED`，`ONLINE + ASYNC` 过滤在
`includeIncomplete.synchronicity = false` 时 `total = 0`。

根因在 `crates/bcsp-catalog/src/delivery.rs` 的 `normalize_occurrence`：
`meetingModeCode == "90"`（ONLINE INSTRUCTION(INTERNET)，Rutgers 几乎所有在线
meeting 都用它）被**无条件**映射为 `Synchronicity::Unspecified`，在时间规则
之前就命中；只有疫情期的 92/93 才产出 SYNC/ASYNC。真实数据是干净的二分：
2,294 条无时间的 mode-90 行全部带 `baClassHours == "B"`，258 条有时间的
mode-90 行全部 `baClassHours == ""`。

## 1. 规则 v2

### 1.1 Occurrence（`normalize_occurrence`）

mode-90 的同步性由时间事实决定，而不是由 mode code 决定：

| 条件 | synchronicity | normalization_reason |
| --- | --- | --- |
| 时间 `Known` | `Synchronous` | `ONLINE_SCHEDULED_SYNCHRONOUS` |
| 时间 `Empty / Missing / Null` 且 `baClassHours == "B"` | `Asynchronous` | `ONLINE_BY_ARRANGEMENT_ASYNCHRONOUS` |
| 其他（`Partial / Invalid`，或无 `"B"`） | `Unspecified` | `GENERIC_ONLINE_UNSPECIFIED` |

裁定：

- 异步推断**要求** `"B"`：今天零成本（2,294/2,294），并防御未来某个漏掉时间
  的排期在线 meeting 被误判为异步；
- 谓词 `online_hours_by_arrangement` **不看 campusLocation**：真实 mode-90
  行带物理校区码 `6`/`7` 或空串，那是"在哪"的冲突不是"何时"的冲突。
  这些行**继续**保留 `MODE_LOCATION_CONFLICT` reason 与 `UnknownConflict`
  modality（reason 精度顺序 UNRECOGNIZED_MODE_TUPLE > MODE_LOCATION_CONFLICT
  > 新的两个 reason > GENERIC_ONLINE_UNSPECIFIED），但同步性照常推导；
- 92/93 不变；`OccurrenceKind`、evidence、modality 的冻结 oracle**一字不改**
  （由 `mode_90_derives_synchronicity_from_time_and_by_arrangement` 的
  时间 × baClassHours × campusLocation 表驱动测试钉死）。

### 1.2 Section（`classify_delivery`）

- 同质集合保留其值（不变）；
- 每个 occurrence 都可靠（仅 SYNC / ASYNC）且两者都出现 → **`Mixed`**（新）；
- 任一 occurrence 为 UNKNOWN / UNSPECIFIED → `Unknown`（不变）。

真实形状：排期 LEC（mode 02，SYNC）+ 在线 by-arrangement 组件（mode 90 `"B"`，
ASYNC）→ MIXED。真实数据里 517 个 section 受影响（462 NB HYBRID、29 NB
ONLINE、14 NK HYBRID 等），此前它们都是 UNKNOWN，`UserSynchronicityV3::Mixed`
过滤永远匹配不到任何东西。

NB 92026 ONLINE section 的模拟结果：`{UNSPECIFIED 1291, SYNC 117, UNKNOWN 30}`
→ `{ASYNC 1152, SYNC 238, UNKNOWN 31, UNSPECIFIED 17}`。

## 2. 为什么只改规则是危险的

三件事叠加：

1. 派生列（section 的 modality/synchronicity；occurrence 的 kind/modality/
   synchronicity/evidence/normalization_reason）持久化在 serving 表里；
2. 发布对 raw 语义 hash 短路：hash 只覆盖 canonical facts，派生列不进
   hash，上游 payload 不变 → `AppliedUnchanged` → 旧派生列永远留着；
3. 读侧投影 `to_normalized_catalog_v1` 从 canonical facts **重算**派生并
   **拒绝**任何不一致的行。

于是二进制升级后，prepared-serving foundation 在第一个 stale target 上失败，
产品面 `SnapshotUnavailable`，直到 Rutgers 恰好改 payload（对低流失的 02027
target 来说等于永远）。因此规则变更必须配套**启动期原地重推导**。

被否决的替代方案：

- 把派生版本折进 `SECTION_SEMANTIC_PREIMAGE_V1`：作废所有存量
  `canonical_sha256`，投影同样 fail-closed，且逼出一次伪 content_version 跳变；
- 启动时强制 catalog refresh：需要网络，且仍然 `AppliedUnchanged`；
- 删 serving 行：丢掉离线可用性；
- 让投影容忍派生漂移（只记 warning）：无需迁移，但 SQL 读者与外部工具会看到
  与投影不一致的存储行。原地重写是保持"存储 == 投影"不变量的方案，代价是
  一张 stamp 表与一次启动扫描。

## 3. 派生版本戳（migration 0007）

```sql
CREATE TABLE catalog_derivation_state (
    target_id TEXT PRIMARY KEY REFERENCES catalog_targets(target_id) ON DELETE CASCADE,
    derivation_version INTEGER NOT NULL CHECK (derivation_version >= 1),
    stamped_at TEXT NOT NULL,
    stamped_observation_id TEXT
) STRICT;
```

- 缺行 == 版本 1（v0.1.1 及更早的规则，`LEGACY_CATALOG_DERIVATION_VERSION`）；
- 每 target 一行：部分完成的启动扫描可以续做，各 target 的发布独立盖戳；
- 与 `migration_bundle.rs` 逐字节镜像（既有测试守卫）；七处迁移计数断言均为 7。

**不变量：戳描述的永远是 serving 表里真正存在的行。**
`publish_staged` 只在替换了 serving 行的分支盖戳（`AppliedChanged`、
`InitialValidEmpty`），`AppliedUnchanged` 分支**不**盖戳。`derivation_version`
是 `apply_catalog_refresh` / `publish_staged_refresh` /
`finish_open_pull_success_with_catalog` 的参数，生产调用方传
`bcsp_catalog::CATALOG_DERIVATION_VERSION`；staging 不跨进程存活
（`recover_interrupted_refreshes` 在每次 open 时清掉 STAGED），所以不把版本
写进 staging 元数据。

## 4. 存储原语（`bcsp-operational-storage`）

- `catalog_target_versions()`：枚举**所有** `catalog_targets` 行（含 selector
  之外的 target 与 `current_content_version == 0` 的 target）。不能用
  `discovered_targets()`（selector 派生）——prepared foundation 按 term 窗口 ×
  产品校区枚举 target，与 selector 无关，任何漏掉的已发布 target 都会让 build
  失败；
- `catalog_derivation_versions()`：`BTreeMap<TermCampusKey, u32>`，缺行即
  legacy；
- `rewrite_catalog_delivery(target, CatalogDeliveryRewrite)`：**单个
  IMMEDIATE 事务**内
  `UPDATE catalog_sections` / `UPDATE catalog_occurrences` /
  `UPDATE catalog_provenance SET detail_json = json_set(...)`，然后 upsert
  戳。每条 UPDATE 断言 `changes() == 1`（否则
  `InvalidStoredProjection`，整体回滚）。provenance 的 `entity_key` 是
  SectionKey 的 Display 形式 `"{term}/{campus}/{index}"` 再拼 `"/" +
  occurrence_key`（与 `mapping.rs` 一致，评审指出的 spec 错误已修正）。
  **绝不**触碰 content_version、canonical_sha256、accepted_semantic_hash、
  checkpoints、counts、FTS、staging 与 Open 表，所以
  `published_catalog_snapshot` 的校验与 Open gate 的 section-set identity
  都保持有效。所有 wire token 先经 `validate_wire_token` /
  `validate_safe_code`。

## 5. 启动重推导（`bcsp_catalog::rederive_stored_delivery`）

对 `catalog_target_versions()` 的每个 target：戳 `!=`
`CATALOG_DERIVATION_VERSION` 才处理（用 `!=` 而非 `<`：被更新的二进制盖过更高
版本戳的数据库在降级后同样被重推导，而不是投影失败）。`content_version > 0`
的 target：读 `published_catalog_snapshot`，按 `projection.rs` 同一套解码把
canonical facts 还原成 `RawCatalogSection`，逐 meeting `normalize_occurrence`，
`classify_delivery`，用与 `mapping.rs` **共享的** `pub(crate)` wire 转换
helper（`section_delivery_wire` / `occurrence_delivery_wire` /
`occurrence_key`）算出应存值，只把有差异的行放进 rewrite，然后**总是**调用
`rewrite_catalog_delivery`（零差异也盖戳）。`content_version == 0` 的 target
只盖戳。每 target 一条 `CATALOG_DERIVATION_REPROJECTED` info 日志。幂等：
第二次运行是 no-op。

挂载点（都在任何 prepared-serving build 与 refresh runtime 之前）：

- 本地：`crates/bcsp-local-runtime/src/bootstrap.rs` `OperationalGate::open`，
  紧接 `OperationalStorage::open` 之后、第二个连接打开之前；失败映射为
  `LocalBootstrapError::CatalogDerivation`，`StartupFailureReport` 有专门分支
  （不再误导用户"把包移到可写目录"）；
- 公网：`crates/bcsp-public-runtime/src/host.rs` `build_production_runtime`，
  `serving_storage` 打开后立即执行；失败为
  `PublicRuntimeError::CatalogDerivation`（code
  `PUBLIC_CATALOG_DERIVATION_FAILED`）。

多进程打开同一公网库时，IMMEDIATE 事务把重推导串行化，操作幂等，后来者会
发现戳已是当前版本。

## 6. 不支持降级

一旦行携带 v2 派生（ASYNC/SYNC/MIXED 与新 reason），v0.1.1 的投影会拒绝它们并
fail-closed。**降级到 v0.1.1 需要删除 `data/`**（或让新二进制先跑一次把戳
写回——v0.1.1 没有这个能力）。发布说明必须写明。首次启动会对每个 target 一次
性重写约 12k section + 17k occurrence + 17k provenance 行（NB），秒级；WAL
会临时增长。

## 7. 测试清单

- `delivery.rs`：表驱动 mode-90 测试；section 聚合断言（sync+async → Mixed，
  LEC 02 + 90 `"B"` → Mixed，含 Unspecified/Unknown → Unknown）；
- `tests/normalization.rs`：10003 加 `"B"` → Async、10011 排期 mode-90 →
  Sync、10007 无 `"B"` → Unspecified；真实形状回归（NB `"B"`、Newark loc 7
  定时、HYBRID 混合、空 location）；
- `tests/projection.rs`：mode-90 `"B"` 经 normalize → 命令 → 存储 → 投影
  往返为 Async / MIXED，发布盖戳；
- `tests/rederive.rs`：legacy 行投影失败 → 重推导后成功、计数、幂等、更高戳
  也重推导、空 target 只盖戳、空时间戳 fail-closed；
- `operational_storage.rs`：`catalog_target_versions` 覆盖 selector 之外与
  version-0 target；发布仅在替换行时盖戳（AppliedChanged / InitialValidEmpty，
  AppliedUnchanged 不盖）；`rewrite_catalog_delivery` 精确改列、provenance
  entity_key 往返、其余字节不变、拒绝伪造 token / 未知行并整体回滚；
- `bcsp-local-runtime`：legacy 库重启后 gate 已修复、投影成功；
- `bcsp-application` `query_service`：P1 复现——O section + mode-90
  `{baClassHours:"B", campusLocation:"O", 空 day/times}` 在
  `includeIncomplete.synchronicity = false` 下命中 ONLINE + ASYNC，且不出现在
  ONLINE + SYNC。

## 8. 附带决定：`UNKNOWN_REQUIREDNESS` 按 REQUIRED 处理（FLT-S06 / FLT-S07）

同一轮引擎级审计（2026-09-01，92026/NB：4,391 门课 / 11,976 section）发现两个
过滤器从未排除过任何东西：可用时段（FLT-S06）任意窗口都返回全集，分校区
（FLT-S07）的 `ALL_REQUIRED_MEETINGS` 模式对任何地点也返回全集。原因相同：
`bcsp-query` 只对 `requiredness == REQUIRED` 的 occurrence 给出 `no_match`，
对 `UNKNOWN_REQUIREDNESS` 一律给 `uncertain`，而 `admit_filter_evaluation`
会放行 uncertain；真实数据里所有 occurrence（NB 17,376/17,376）的 requiredness
都是 `UNKNOWN_REQUIREDNESS`——Rutgers 的 feed 根本不带这个字段。

决定：在 `crates/bcsp-query/src/availability.rs`（`evaluate_facts`）和
`crates/bcsp-query/src/predicates.rs`（`evaluate_meeting_location` 的
`ALL_REQUIRED_MEETINGS` 分支）里，把 `UNKNOWN_REQUIREDNESS` 当作 `REQUIRED`
评估：

- 有排期的 meeting 落不进任何窗口 → `no_match`（不再是 uncertain）；
- `ALL_REQUIRED_MEETINGS` 对未知 requiredness 的 occurrence 按地点逐个评估，
  而不是整体退化为 uncertain；
- 显式 `OPTIONAL` 仍然不参与；
- 没有可用时间的 occurrence（TBA / 缺失 / 无效）仍然是 uncertain，因为没有
  可以据以排除的事实。

理由：Rutgers 只列出学生要去上的 meeting；可跳过的 meeting 不会出现在 feed
里，所以"未声明 requiredness"在语义上就是"必须出席"。把它当作未知而放行，
等于让这两个过滤器永远失效。相关单元测试与 `bcsp-query/tests/query_engine.rs`
已按新规则更新。

顺带修的第二个审计缺陷（FLT-C08）：核心课程代码在 feed 里是混合大小写
（`AHo`、`AHp`、`AHq`、`AHr`、`WCd`、`WCr`），请求侧的 `try_new` 会把代码统一为
大写，而 prepared-serving 的动态字典与查找都按原样保存，导致这些代码无论以何种
大小写提交都得到 `INVALID_FILTER_OPTION`。现在 `normalize_dynamic_value`
（以及 `query_service` 的旧路径校验）把核心代码统一为 ASCII 大写；discovery
的 `coreCodeDictionaries` 仍保留 feed 原始拼写供展示。
