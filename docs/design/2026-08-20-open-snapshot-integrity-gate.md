# 设计方案：Open 快照完整性安全门（Snapshot Integrity Gate）

状态：v5 —— **已批准**（Codex 终审 2026-08-20）。进入实现阶段；遗留
细节按 `2026-08-20-pr-acceptance-checklist.md` 在 PR 中验收。
日期：2026-08-20
作者：Claude；评审：Codex（v1：5 项阻断；v2 复审：5 项遗留，本版逐项闭合）

## 1. 背景与证据

2026-08-19，`openSections.json`（NB）返回语义残缺快照：13 个连续样本
8,146 个 index，异常前基线 11,423，对异常前快照严格满足 A ⊂ P
（P∖A = 3,277，A∖P = 0，独立验真）。HTTP 200、gzip 完整、JSON 合法、
Index 合规——现有全部校验拦不住。严格证明持续 ≥41s；外包络 ~105s。
当前代码将其判为 `ValidApplied`，缺席 index 全写 CLOSED，恢复后翻回
OPEN：假 CLOSED→假 OPEN→假警报 + 烧 maxAudible 额度 + 污染本地历史。

- 原始抓包：`data/open-sections-repro/20260819T2117Z-original-capture/`
- 独立验真：`.../20260819T2117Z-original-capture-verification/`

## 2. 历史约束（硬边界，不变）

1. P7.1-007：空集/零交集/失败/race 保留 LKG，"no mass-close failure path"。
2. RC2（`b4a0a8b`，同期实机验收记录）：真实空目标 `92025/H` 在
   `UnsafeEmpty` 规则下无限重试卡死就绪门 → 已验证空数组改判有效。
3. RC3 §7.1（ACCEPTED）：「经过完整性和关联规则判定为有效的空集合可以
   构成成功结果；不能把所有空响应无条件视为成功。」

## 3. 安全门的作用域（v3 关键澄清：只包裹本可应用的快照）

**Gate 只对"按现有规则本将判为 `ValidApplied` / `ValidEmptyNoRows` 的
快照"生效。** 现有硬拒绝（`StaleCatalogRace`、`UnsafeZeroIntersection`、
格式/传输失败）**永远优先、完全绕过 Gate**，且：

- 硬拒绝**不构成 Gate 样本**：不进入候选计数、不触发转移、不改变隔离态
  （与传输失败同权处理）；
- 因此 `QuarantineConfirm` **不可能应用一份零交集/race 快照**——它根本
  进不了候选池。新增钉死测试：**全 orphan 响应稳定持续 >300s，永不应用，
  分类始终 `UnsafeZeroIntersection`**。

## 4. 状态机

每个 target 一台，状态含义与 v2 相同，定义补全如下：

```text
GateRuntime {                       // v4：epoch 提升到两态之上、恒存在
    epoch: u64,                     // 每次状态转移 +1（含 Healthy 内窗口推进）
    state: GateState,
}
GateState::Healthy {
    baseline: Option<BaselineEpoch>,
    // None = 无基线（新 target / 新目录身份且无可迁移 LKG）
    //        → Gate 不判定，一切照旧（Pass）
}
BaselineEpoch {
    catalog_identity,               // v4：= 目录 section-set 语义哈希，
                                    // 非 content_version——不同未发布
                                    // candidate 可同为 serving+1，版本号
                                    // 不能唯一标识目录身份
    window: VecDeque<u64>,          // 最近 W=16 个 applied 快照的
                                    // observed_intersection_count
    value: u64,                     // = max(lower_median(window), lkg_count)
}
GateState::Quarantined {
    reference_baseline: u64,        // 入隔离时冻结
    catalog_identity,               // 同上
    episode: CandidateEpisode {
        anchor_set: BTreeSet<SectionIndex>,
        anchor_set_sha256,          // = canonical_set_sha256
        first_seen, last_seen,
        consistent_count: u32,
    },
}
```

**serving/candidate 双 runtime（v5 窄修）**：v4 的"candidate 一律无基线
Pass"构成**绕过**——目录更新窗口内，与 candidate 目录配对的残缺 Open 会
被原子发布并 fanout（open.rs:1129 → refresh_coordinator.rs:1357），原始
漏洞在每次目录更新时重现（评审方构造，成立）。改为：

- candidate 拥有**独立 GateRuntime**，按其 section-set 语义身份键控；
- **播种**：提交前以 |serving LKG open 集 ∩ candidate 目录| 为基线
  （启用条件与取整规则同 §4；无 LKG → 无基线 Pass，属既有 residual）；
- 同身份 candidate 的 **Hold 跨重试累计**（隔离/确认窗口语义与 serving
  相同，探测节奏跟随所在工作流的重试序列）；
- candidate **成功发布时其 GateRuntime 原子提升为 serving runtime**
  （在发布事务所在的串行锁临界区内完成替换，旧 candidate 项清除）；
- 身份互异的 serving/candidate 互不读写——v4 的隔离目标保留，绕过消除。

钉死测试：serving=A（LKG≈11,423）→ candidate=B 首拉配对 8,146 残缺快照
→ **必须 Hold、candidate 不发布、零 fanout**；candidate 重试恢复正常
规模 → 正常发布且 runtime 提升。

### 判定（仅 Healthy 且 baseline=Some 时执行；单侧，只看骤降）

```text
missing = baseline.value.saturating_sub(observed_intersection_count)
suspect = missing >= max(ceil(0.10 * baseline.value), 25)
// 取整：ceil；比较全整数域。增长（missing=0 侧）永不 suspect。
// baseline.value == 0 时恒 Pass（覆盖 92025/H 与 baseline=0 边角）。
```

空集是 `observed=0` 的特例，同一公式覆盖：小基线目标照旧
`ValidApplied`/`ValidEmptyNoRows`（RC2 语义保留）；大基线目标骤空 → 隔离。

### Healthy 态其余转移（v3 补全）

| 事件 | 转移 |
|---|---|
| applied 快照提交成功 | window 推入 observed 计数（满 16 挤最旧）；value 重算 |
| catalog 世代变更 | window 清空；`value` 以 \|LKG open 集 ∩ 新 catalog\| 迁移播种；LKG 不可用 → baseline=None（Gate 暂停至首个 applied 播种） |
| 硬拒绝/传输失败 | 状态不变 |

### Quarantined 态转移（与 v2 相同，仅列差异；恢复条件见 v5.1 修正）

**v5.1（实现期修正，回放测试发现）——恢复出口加迟滞**：恢复条件从
"缺口 < 入口阈值"收紧为 **`missing < max(ceil(reference/20), 25)`**（出口
5%，入口 10% 的一半）。原因：入口与出口共用阈值时，评审 v1 反例
`11423 → 8146 → 10300` 中的 10300（缺 9.83%）会被字面公式判为"恢复"并
应用——v2 文档的工作示例文字与公式互相矛盾，五轮评审均未察觉，真实
抓包回放测试第一次运行即暴露。缺口在 5%–10% 迟滞带内的样本继续扣住，
直至完全恢复或满足确认条件。

- **隔离专用探测节奏（v3 新增，替代 Transient 复用）**：隔离期间该 target
  的 Open 重试固定为 `QUARANTINE_PROBE_SECONDS = min(30, 正常 watch 间隔)`
  秒，**全校区统一**（v2 复用 Transient 的问题：NK/CM 序列 15/30/60/120/300
  + 正抖动，120s 档即超 MAX_GAP，确认条件永不可达）。30s ≪ MAX_GAP=120s，
  三校区确认路径均可达。429/origin-pause 仍最高优先（照常冻结一切）。
- 候选一致性：对称差 ≤ 1%（ceil 取整）of anchor 大小。
- 确认：`consistent_count ≥ 3 && last_seen − first_seen ≥ 300s &&
  每对相邻候选样本间隔 ≤ 120s`。
- 与 anchor 与 reference 均不合 → **完全重锚**：anchor/first_seen/
  last_seen/count 全部重置（v3 澄清：不保留旧 first_seen，杜绝异集拼接跨度）。
- 硬拒绝/传输失败：非样本，状态不变；相邻**候选样本**间隔以候选为准，
  失败插入导致间隔 >120s 时按超 MAX_GAP 处理（重锚）。

### 重启重建（v3 收紧）

启动时读该 target 最近 attempts（`(target_id, attempt_sequence DESC)` 索引）：

- **仅当**最近一条是 `SUSPECT_PARTIAL_SNAPSHOT` **且**自该条起向前是
  **不间断**的 suspect 连续段（无 FAILED/INTERRUPTED/其他分类插入），
  才重入 Quarantined（reference 由 LKG 派生）；
- episode 续接**仅当同时满足**：(a) 重启后第一个可疑样本的
  canonical_set_sha256 与持久化的最近 suspect attempt 哈希**完全相等**；
  (b) **跨重启间隔同样受 MAX_GAP 约束**（v4 补）：now − 持久化最近
  suspect 的 `completed_at` ≤ 120s——停机十分钟后即使哈希相同也不继承
  旧 count/span。任一不满足 → 全新 episode（count=1、时间重置）；
- 其余一切情况 → Healthy（baseline 照 §4 播种规则）。

## 5. Suspect 语义（同 v2，两处明确）

- 走 `finalize_non_applied_attempt`，LKG 不动、零 fanout、
  `error_code = SUSPECT_PARTIAL_SNAPSHOT`、freshness 立即 Stale、
  新增 `OpenUncertaintyReason::SuspectPartialUpstream`；
- **投影新增独立 `OpenFailureClass::SuspectPartial`（不复用
  SchemaViolation，评审方确认）**，接入 `map_failure_code`
  （projection.rs:665 起）及全部 code→class 映射。

## 6. 数据流与提交协议（v4 重写：串行锁取代后置 CAS）

**流水线（v4，消除 v3 的循环依赖——硬分类原在 reconcile 内计算，却又
要求先于 Gate）**：

```text
① PreGateAssessment（纯函数，不依赖 Gate 状态）
   输入 = 响应 + 当前目录快照
   输出 = HardReject(race | zero-intersection | malformed …)
        | ValidContext { observed_intersection_count,
                         response_canonical_set_sha256, catalog_identity }
② HardReject → 现有拒绝路径（Gate 全程不参与、非样本）
③ ValidContext → 在 per-target Gate 串行锁内计算
   GateDecision { disposition: Apply | Hold,      // v4：显式处置字段
                  kind: Pass | Suspect | QuarantineRecover
                        | QuarantineConfirm,
                  catalog_identity, response_canonical_set_sha256,
                  observed_intersection_count, gate_epoch,
                  next_state }
④ reconcile_open_set / 存储 classify / candidate commit 接收
   PreGateAssessment 结果 + GateDecision，仅验证一致、不得重算
```

**提交协议（v4：per-target 强串行，后置 CAS 降级为不变式断言）**：

- v3 的"事务成功后比较 epoch"存在安全反例：A(Confirm+Apply) 与
  B(Reanchor+Hold) 同从 epoch=e 计算，B 先提交 SUSPECT 后 A 依串行语义
  已丧失确认资格，但 A 的 `VALID_APPLIED` 事务照样落库并替换 LKG——
  后置 CAS 只能丢弃 A 的内存 next_state，**无法撤销已提交的写入**。
- 修正：**per-target Gate 串行锁横跨 ③→④→状态推进全程**——决策、数据库
  事务、状态 advance 在同一锁内完成，primary / candidate / concurrent
  三条路径全部纳入。锁序固定为 Gate 锁 → 存储锁（全库互斥本就串行化
  事务，本锁只是把"决策与提交"绑为一个临界区，额外争用可忽略）。
- `gate_epoch` 保留为**不变式断言**：锁内提交前断言捕获 epoch == 当前
  epoch，不等即 panic（说明锁被绕过，属实现缺陷，宁可炸不可默）。
- 提交失败 / `StaleCatalogRace` → 状态不推进（锁内直接放弃 advance）。

## 7. 迁移 0005（v3 重写：先改 runner，再谈 12 步）

**现状阻断**：runner 先启用 FK，随后在 `TransactionBehavior::Immediate`
事务内 `execute_batch(migration.sql)`（migration.rs:92 区段）——事务内
`PRAGMA foreign_keys=OFF` 无效；直接 DROP 父表会触发两个
`ON DELETE CASCADE` 子表清空，且清空后 `foreign_key_check` 照样干净。

**方案（v4 按复审意见补全 runner 规格——现行 runner 将全部 pending
迁移放在一个事务里，必须先改分段）**：

1. **runner 分段提交**：迁移逐个独立事务执行（v0→v5 依次五段各自
   commit + 各自 ledger 行；任一段失败，之前已 commit 的段保持有效）。
2. 迁移描述新增 `requires_foreign_keys_off: bool`。对此类迁移：
   `PRAGMA foreign_keys=OFF`（**事务外**）→ **readback 断言**
   `PRAGMA foreign_keys` 返回 0 → 开 IMMEDIATE 事务 → 标准十二步
   （建新父表→拷贝→DROP 旧→改名→重建索引；FK 关闭期间 CASCADE 不触发）
   → 事务内运行 `PRAGMA foreign_key_check` 并**实际遍历结果行**断言为空
   → commit → `PRAGMA foreign_keys=ON` + readback 断言 1。
3. **失败路径**：任一步失败 → rollback 当前事务；随后**要么**成功恢复
   `foreign_keys=ON`（readback 确认）**要么**直接丢弃该连接（绝不把
   FK=OFF 的连接归还使用方）。
4. **可重试测试**：0005 中途注入失败 → 断言 ledger 无 0005 行、数据
   完好（父子行数不变）、重开连接后重跑 0005 成功。
5. 既有三件套保留：(a) 带父子数据 v4→v5 行数/索引/0004 列/检查零行；
   (b) 级联保全；(c) 人为孤儿子行必须被检出（阴性）。
6. 账本（迁移计数 4→5）与 bundle parity 同步。

## 8. 测试计划（v2 基础上新增/修订）

新增：
1. 全 orphan 稳定 >300s → 永不应用（§3）；
2. NK/CM 校区在隔离探测节奏下确认路径可达（§4）；
3. suspect→FAILED→restart → 不重入隔离/全新 episode；
4. 重启后哈希不等 → episode 完全重置；哈希相等 → 续接跨度；
5. 重锚不保留旧 first_seen（异集不拼接）；
6. baseline=None/=0/catalog 世代切换的播种与 Pass 行为；
7. **串行锁并发测试（v5 措辞修正——强串行下"A 已持 Confirm 而 B 先
   提交"不可能发生）**：A 阻塞在取锁前、B 先完成 Suspect 重锚提交，
   随后 A 入锁重新裁决为 Hold，断言 A 的 VALID_APPLIED 不落库、LKG 不被
   替换；并断言锁内互斥（B 持锁期间 A 确实阻塞）；
8. candidate 双 runtime：A→B candidate 首拉残缺 → Hold/不发布/零
   fanout；同身份重试 Hold 累计；恢复后发布且原子提升；身份互异
   互不读写；
9. 跨重启 MAX_GAP：停机 >120s 后哈希相同也不继承 count/span；
10. §7 迁移全套（分段提交、readback、失败恢复/弃连接、遍历检查行、
    可重试、三件套）。

v2 既有测试（回放、漏洞序列、确认边界、fork 共享、提交失败不推进）保留。

## 9. Residual risks（不变 + 一条）

1. 全新 target 首拉即残缺：无基线，接受；
2. 稳定 >5 分钟的一致残缺产物会被确认为真实（三重巧合）；
3. 重启丢 anchor 集合本体，跨重启一致性退化为哈希全等判定；
4. （新）catalog 世代切换且 LKG 不可用的空窗期 Gate 暂停（首个 applied 恢复）。

## 10. 明确不做

不恢复 `UnsafeEmpty` 无条件拒绝；不做 per-section 启发式；
不动 429/circuit/origin_pause；不做增量写 `open_section_current`。

---

### v5 → v5.1 变更记录（2026-08-21 实现期修正，待评审追认）

- 恢复出口加迟滞（出口阈值 = 入口的一半，新常量
  `GATE_RECOVERY_THRESHOLD_DENOMINATOR = 20`）：修复"入口阈值兼作出口"
  使 v1 反例中 9.83% 缺口样本被误判恢复的公式漏洞；由真实抓包回放测试
  `v1_design_loophole_sequence_never_applies_the_drifting_partial` 发现并
  钉死。文档工作示例与公式自此一致。

### v4 → v5 变更记录（2026-08-20 窄修，回应 Codex 四审设计阻断 1）

- candidate 由"一律无基线 Pass"（构成绕过）改为**独立 GateRuntime**：
  按 section-set 身份键控、以 serving LKG ∩ candidate 目录播种、Hold 跨
  重试累计、发布时在串行锁内原子提升为 serving；新增 A→B 残缺首拉钉死
  测试；§8.7 并发测试措辞按强串行语义修正。

### v3 → v4 变更记录（2026-08-20，回应 Codex 三审 Gate 侧 3 项阻断）

1. 后置 epoch CAS 存在"陈旧 Confirm 已落库不可撤销"反例 → 改为
   per-target 串行锁横跨决策/事务/状态推进全程（三条提交路径全纳入），
   epoch 降级为锁内不变式断言；`GateRuntime{epoch,state}` 两态恒存在；
2. 目录身份改绑 section-set 语义哈希（candidate 版本号不唯一）；
   serving/candidate 状态隔离；重启续接补跨重启 MAX_GAP；
3. 流水线显式化 PreGateAssessment → GateDecision{**Apply/Hold**}，消除
   硬分类与 Gate 的循环依赖；迁移 runner 规格补全（分段提交、OFF/ON
   readback、失败恢复或弃连接、遍历 foreign_key_check、0005 可重试测试）。

### v2 → v3 变更记录（2026-08-20，回应 Codex 复审 5 项遗留）

1. Gate 作用域收窄为"仅包裹本可应用快照"，硬拒绝绕过且非样本；补全
   orphan >300s 钉死测试（遗留 1）；
2. 隔离专用探测节奏（全校区 min(30s, watch 间隔)），替代 Transient 复用
   （遗留 2）；
3. 重启续接仅认哈希全等 + 不间断 suspect 段；重锚完全重置（遗留 3）；
4. 迁移改为"runner 支持事务前关 FK"+ 级联保全/阴性测试（遗留 4）；
5. 补全无基线表示、catalog 世代转移、W/取整/单侧判定/baseline=0；
   GateDecision 五元组绑定 + 乐观 epoch 提交协议（遗留 5）；
6. 投影确定采用独立 SuspectPartial 类。
