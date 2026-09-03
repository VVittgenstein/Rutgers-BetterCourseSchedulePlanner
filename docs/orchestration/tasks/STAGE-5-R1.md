# Stage 5 R1：让 evidence analyzer 的 GO 门不能被空证据绕过

状态：**READY — 返回原 Stage 5 worktree 集中窄修**
Prompt 版本：`STAGE-5-R1/v1`
基线：`codex/parallel-wave1-stage5-s3-evidence@95827285205de68be85fc9711821d25f66f3f1d5`

## A. 修复任务包

现有 NB 报告的 `NO_PRODUCTION_CHANGE / DATA_REQUIRED` 结论正确，58 项测试通过；但未来 GO 门有四个
可构造假阳性，必须修复后 analyzer 才可作为决策工具：

1. A4-2 只数 window metadata：一个峰值时段的孤立/零变化样本也能补齐 peak 条件；
2. A4-6 只 leave-one `(target,window)` group，既未整 target 留出，也未做承诺的 outlier sensitivity；
3. 只要任意无关 sample 有 `serverDate`，全局 clock status 就可让 client-clock comparison/safe offset 通过
   A4-5；
4. A4-1 只信 campus label：同一 capture/QA DB 复制三份、改 run.json 为 NB/NK/CM 可冒充独立证据。

修复合同：

- A4-1 只把独立 data provenance 算作独立 target evidence。把 metadata/target identity 与 observation-data
  fingerprint 分开；重复文件、重复 SQLite 内容或等价 observation/hash series 不得因重命名而增加覆盖；
- A4-2 的 peak 与 off-peak 必须各有真正 qualifying informative brackets，不得由空/单样本 window 满足；
- 生产 GO/safe offset 必须基于足够且覆盖 qualifying groups 的 server-clock evidence。comparison fallback 到
  client clock时必须 NO-GO；一个无关 Date 不能解除；
- A4-6 至少证明 winner 在整 target 留出、target/window group 留出和确定性少量 outlier sensitivity 下保持；
  采用高效算法还是 bounded rerun 由实现代理决定，但报告必须分别列出证据，不能继续用 group 代称 target；
- 增加四条可打挂 `9582728` 的 end-to-end counterexample；全部 A4 门真实满足的 GO fixture 仍可达；
- 重新生成 JSON/MD，现有本地 NB 数据仍为 NO_PRODUCTION_CHANGE；
- 原始 capture 实际位于 gitignored `data/`，不是 committed。修正文案；测试优先找当前 repo root 的
  `data/open-sections-repro`，worktree 再 fallback 到共享主 repo/env。数据存在时不得因 checkout 布局 skip，
  CI 真无数据时可诚实 skip。

不改 production scheduler/policy/storage，不联网、不采样，不处理重复 CLI 入参、裸 NUL、一般字段类型等
deferred，除非修改同一行自然消除且不扩展。只跑 analyzer tests、离线 current-data command、diff-check。

## B. 实现任务提示（请用户从下一行开始原样复制）

ultracode: 请继续使用外部实现工具的官方 dynamic workflow，只在
`Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\parallel-wave-1\stage5-s3-evidence` 的
`codex/parallel-wave1-stage5-s3-evidence@95827285205de68be85fc9711821d25f66f3f1d5` 上完成
`STAGE-5-R1/v1`。不要写主 checkout，不 reset/rebase/merge，不修改 `docs/orchestration/**`，严格离线。

先阅读主 checkout 的 `docs/orchestration/tasks/STAGE-5-R1.md` A 部分。只关闭四个 false-GO 根因：独立
provenance 不能靠重标 NB/NK/CM 伪造；peak/offpeak 必须各有 informative evidence；client-clock fallback/
一个无关 serverDate 不能授权 GO/safe offset；A4-6 必须分别做整 target、group 和小量 outlier stability。
再修正 local gitignored data 的定位/措辞，重生成仍为 NO_PRODUCTION_CHANGE 的 evidence。

内部算法与测试组织由你自主决定，但必须有四条能打挂 `9582728` 的 end-to-end counterexample 和仍可达的
真实 GO fixture。不要改任何 production crate、不要联网采样、不要顺手修 deferred。只跑 analyzer
focused tests与 diff-check，最后一次性回报 commits、gate 语义、negative proof、current-data 结果和
`9582728..<head>`。
