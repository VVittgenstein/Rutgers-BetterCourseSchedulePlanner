# P7.1-002 dependency lock、license 与初始 SBOM 基线

## 1. 决议与入口

- 记录：`P7-1-002-DEPENDENCY-BASELINE-2026-07-13-001`
- 任务：`P7.1-002 Dependency lock license and SBOM baseline`
- 唯一前置提交：`f39b033b491b7f22429348df11e3fd6191ef1615`（`P7.1-001`）
- 分支：`codex/p7-implementation`
- 项目许可证：`ISC`
- 版权行：`Copyright (c) 2026 VVittgenstein`

用户已批准本记录中的 toolchain、Jiff、build/CI 工具与图边界决议。P7.1–P7.4 的实施权限继续有效；本任务不授权 P7.5 真实世界测试、真实 Rutgers 请求、Vultr 变更、GitHub Release、DNS/Cloudflare/证书或生产流量变更。

## 2. 本任务的实现边界

P7.1-002 只建立可机械解析的 dependency-resolution scaffold：精确 manifests、精确 lockfiles、工具链 pin、license/official-metadata/advisory 证据和一份初始 CycloneDX SBOM。`crates/dependency-baseline` 只能引用依赖以使 Cargo 解析并检查其 feature/target 闭包，不得实现产品行为、API、storage、Rutgers adapter、watch、UI 或打包逻辑。

`P7.1-003` 仍独占最终 Rust workspace/module graph、shared/adapters/entries 与 capability build guards。本任务的单 member scaffold 不是最终架构，也不得被解释为批准第二套实现。

## 3. 固定工具链与 build-only 工具

| 项目 | 固定值 | 分发属性 |
|---|---:|---|
| Rust | `1.97.0` | build-only，不进入最终包 |
| Cargo profile | `minimal` + `rustfmt` + `clippy` | build-only |
| Node.js | `24.18.0` | build-only，不进入最终包 |
| npm | `11.16.0` | build-only，不进入最终包 |
| cargo-deny | `0.20.2` | build/CI-only |
| cargo-about | `0.9.1` | build/CI-only |
| cargo-cyclonedx | `0.5.9` | build/CI-only |

`time` 固定为 `0.3.53`。新增批准的 `jiff` 固定为 `0.2.32`，仅用于封装后的 `America/New_York` IANA/DST 适配；Jiff 类型不得进入 shared domain 或公开 API model。嵌入资产候选 `include_dir` 与 `rust-embed` 必须以真实 registry metadata、locked closure、license 和 feature 图机械比较，active graph 最终只能保留一个。

## 4. dependency graph 的可证明范围

本任务中的“zero consumer”严格定义为：

`zeroConsumerScope = ACTIVE_P7_TARGET_GRAPH_ONLY`

active P7 graph 仅由 `Cargo.toml`、`crates/dependency-baseline/Cargo.toml`、`frontend/package.json` 及对应的 `Cargo.lock`、`frontend/package-lock.json` 组成。根目录既有 Node manifest/lock 和旧 source graph 不属于 active P7 graph；它们冻结为 `FROZEN_EXCLUDED_PENDING_OWNER_TASK`。

因此：

- 可以证明 rejected dependency 在 active P7 manifests、locks 与解析图中为零消费者；
- 不得声称全仓库为零消费者；`repositoryWideZeroConsumerClaim = false`；
- legacy graph 中已知或可能存在的消费者不得在本任务偷删；必须由 `rejected-dependency-consumers.tsv` 中绑定的后续 owner task 完成 parity、迁移或删除；
- `react-window` 与 `@types/react-window` 从 active frontend manifest/lock 移除，但 legacy UI source 的替换/删除归 `P7.2-001`；
- root Fastify、better-sqlite3、backend Zod、tsx/TypeScript 等旧闭包保持冻结排除，不能被 active Rust/frontend lock 或最终包继承。

## 5. 必须生成并相互闭合的证据

本提交必须精确包含任务合同 JSON、graph scope、rejected consumer owner 表、两套 lock、locked component inventory、official metadata source、license allowlist、license-file hash、advisory scan、toolchain fingerprint、初始 CycloneDX SBOM、builder、validator 与 completion 记录。所有版本和 license 判断必须从实际 lock 与官方 registry/repository metadata机械解析，不能以计划值代替运行证据。

最低通过条件：

1. `Cargo.lock` 与 `frontend/package-lock.json` 能在固定工具链下以 locked/offline 方式重解析；不存在 floating Git dependency。
2. active graph 中 rejected dependency/closure 为零；候选 embed crate 恰好一个。
3. `time=0.3.53`、`jiff=0.2.32`、Rust/Node/npm 和三项 Cargo 工具 pin 与指纹一致。
4. 所有 locked component 均有 source/checksum 或可解释的平台来源、SPDX/license decision 与 official metadata；未知或拒绝 license 为零。
   仅锁文件存在、但从两个批准目标图均不可达的组件必须以精确组件/版本/license 记录为 `PLATFORM_EXCLUDED`，并附目标图不可达证据；它不因此进入 `deny.toml` 的 active Rust allowlist。
5. advisory scan 实际执行并通过；不能访问 registry/advisory source 时必须 STOP，不得伪造 PASS。
6. 初始 SBOM 是机器可读 CycloneDX，覆盖当前 Rust 与 frontend locked closure；它是实现期基线，不替代 P7.4 的两个最终包各自 SBOM。
7. P7.1-001 冻结的 167 项既有工作树内容逐项保持，`.secrets/` 不枚举、不读取、不追踪。

## 6. Git 与公开元数据边界

本任务只允许 `02a` 中列出的 26 个相对路径进入专用提交。不得 stage、恢复、stash、reset、clean 或改写任何既有用户工作树内容。公开内容可以包含仓库相对路径、Git 状态、文件大小与非敏感 hash；不得包含文件正文转储、`.secrets/`、凭据、个人信息、绝对用户路径、真实 raw Catalog/Open payload 或私有基础设施 inventory。

验证顺序固定为：

1. `PreCommit`：HEAD 与 remote tracking 均仍是 `f39b033...`，index 精确等于 26 路径 allowlist；
2. 专用 commit；
3. `PostCommit`：parent 与 committed path set 精确匹配，index 为空；
4. push；
5. `PostPush`：`origin/codex/p7-implementation` 精确等于 HEAD。

任一 license、version、official metadata、advisory、SBOM、秘密审计、167 项保护或 path allowlist 无法机械闭合时，任务状态必须为 FAIL/STOP，不能进入 `P7.1-003`。
