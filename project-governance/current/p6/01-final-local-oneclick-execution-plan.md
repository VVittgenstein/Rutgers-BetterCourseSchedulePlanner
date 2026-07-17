# P6 最终 Windows 本地一键包执行计划

## 1. 交付合同

- **状态**：`P6_LOCAL_EXECUTION_PLAN_AMENDED_FOR_REVIEW`
- **最终制品**：`WINDOWS_LOCAL_RELEASE_ARCHIVE`
- **必需目标**：64-bit Windows（`x86_64-pc-windows-msvc`）；ARM64 不属于本轮必交付范围，若增加必须独立 Review
- **运行时**：单个 Rust 本地服务进程 + 嵌入式 React WebUI + bundled SQLite/FTS5
- **入口**：`RBCSP.exe` 与薄 `Start-RBCSP.bat`
- **用户前置依赖**：无 Node、npm、Python、Git、SQLite 工具、管理员权限或系统 OpenSSL
- **本地可写状态**：以正在运行的`RBCSP.exe`所在目录为`<package-root>`，唯一主库固定为`<package-root>/data/rbcsp.sqlite`
- **当前执行权限**：无；必须等 P6 Review 批准进入 P7

本计划完整消费 P3 本地计划与 P5 单一共享基线。它不建立 local 业务 fork；local composition root 只能装配共享 core、local state/store、Windows host 与 local UI entry。

## 2. 最终包 allowlist 与 denylist

Windows archive 至少且仅按正向 allowlist包含：

| 路径/成员 | 用途 |
|---|---|
| `RBCSP.exe` | self-contained release binary；嵌入共享与 local WebUI assets |
| `Start-RBCSP.bat` | 只定位 archive 根目录、启动 exe、转发退出码并给出可读错误 |
| `README.md` / `QUICKSTART.md` | 解压到可写目录、启动、退出、升级、Reset、完整数据库备份、卸载与排障 |
| `VERSION` / `SHA256SUMS` | release 身份与完整性 |
| `SBOM` / `THIRD-PARTY-NOTICES` / license | 依赖与许可披露 |

release archive不得包含：public systemd/Caddy/ops、Node/npm/Vite dev runtime、源码、source map、测试 fixture/raw evidence、`.git`、私有 inventory、secret、token、真实路径、日志、任何`.sqlite`/`.db`/`-wal`/`-shm`、schema seed、预装Catalog/Open课程数据、checkpoint、backup、active watch/session、旧 release、macOS launcher，或 mail/Discord/Web Push/Calendar/waitlist/Share-link 表面。`data/`及其唯一主库只允许在首次运行时创建，不是release archive的预置成员。

## 3. 可直接执行的 P7 顺序

每个工作包必须在独立 task 中完成其输出、验证、记录和实质 commit；下一个工作包只能消费已通过的前置输出。失败时回滚当前 task 的新增实现，不修改冻结合同，也不把失败修补混入后续 task。

下表的`L-*`是便于按本地交付阅读的执行视图，不是另一套task ID。其与`04-p7-task-and-commit-matrix.tsv`中32个canonical task的精确映射见`03-shared-implementation-dependency-dag.md`第6节；依赖、skill、commit boundary或stop gate若有歧义，以`04`为准，禁止据此建立重复实现。

| ID / P7阶段 | 输入 | 执行动作 | 必需输出 | 验收门 | 回滚点 |
|---|---|---|---|---|---|
| `L-00 / P7.1` | P6 Review 批准；P3/P4/P5 hash一致 | 建立共享 workspace、locked toolchain、两个 binary composition roots 和正向 dependency boundary；此 task 不实现业务 | 可复算 workspace graph、toolchain/lock baseline、local/public entry skeleton | graph 无环；shared 不依赖 target；local 不依赖 public；无第二业务实现 | 删除本 task 新增 skeleton/lock 变更并回到批准基线 |
| `L-01 / P7.1` | `L-00` | 实现共享 typed DTO、SectionKey/Course/variant/occurrence、FilterSchema 与错误/版本 envelope | 单一 shared domain/protocol crate 与 shared contract tests | 22行filter逐字段映射；`(term,campus,index)`稳定；en-US/zh-CN key schema可版本化 | 回退本 task commit；不得在adapter复制模型 |
| `L-02 / P7.1` | `L-01` | 实现共享 operational SQLite schema、migration、transaction、FTS5与repository ports；物理路径由target adapter注入 | operational table/migration domain与in-memory/temp tests | migration checksum/rollback、FTS parity、table ownership allowlist、无旧subscription/contact/token | 回退新migration与代码；测试库重建，绝不改用户真实库 |
| `L-03 / P7.1` | `L-02` | 实现 discovery/Catalog client、safe staging、normalizer、target replacement、provenance/checkpoint | 共享 Catalog ingest/query core | 固定fixtures覆盖empty/duplicate/TBA/Delivery/variant；target失败不破坏LKG | 回退当前 client/normalizer commit；删除临时测试库 |
| `L-04 / P7.1` | `L-03` | 实现 query、22筛选、course-centered结果、independent section search/detail、direct URL | shared query/API contract | same-section/same-variant witness、三值结果、FTS和分页均通过 | 回退query/API task；保留已验证schema与ingest |
| `L-05 / P7.1` | `L-02`,`L-04` | 实现 LOCAL_ONLY personal table/migration domain：filters、selected、prefs、Saved views、history、Reset；与operational tables物理共存于唯一主库 | local state API/store、CAS、table ownership allowlist与组合migration | 恰好一个主SQLite；跨重启持久；active watch不恢复；filter Reset/library delete-all/user-data Reset三种scope不混淆 | 回退local adapter task；完整测试主库从snapshot恢复 |
| `L-06 / P7.1` | `L-03`,`L-04` | 实现共享 Catalog/Open scheduler、single-flight、EDF、backoff/circuit、observation/counter/freshness | shared refresh runtime 与 fake-upstream harness | 3/10/30/3600场景、origin并发1、无catch-up、unsafe不mass-close、stale/UNKNOWN正确 | 停止harness并回退scheduler task；不得用调高并发修复 |
| `L-07 / P7.1` | `L-05`,`L-06` | 实现 typed HTTP/WS、loopback/nonce/Origin安全、selection/watch、ONE_SHOT/CONTINUOUS状态机 | local runtime composition、WS protocol、episode/action store | 最多9 section、disconnect清理、cap只静音不取消、Closed→Open re-arm、A/C/D确认语义 | 停止测试进程、清除临时nonce/DB、回退runtime task |
| `L-08 / P7.1` | `L-04`–`L-07` | 建立 shared React功能壳、local entry、完整两语 catalog；只完成可用结构，不宣称正式视觉完成 | 功能完整的local WebUI baseline | route/API/state/i18n contract通过；Saved views只由local graph可达 | 回退UI baseline task；不得临时将local能力放入shared |
| `L-09 / P7.2` | `L-08`已集成且功能门通过 | 独立 task 同时使用 `$industrial-brutalist-ui` 与 `$design-taste-frontend` 完成正式桌面/移动响应式 UI | 正式视觉系统、组件、responsive states、视觉验证记录 | 搜索/筛选/course/section/watch/audio/Saved views/Reset全状态截图与交互验证；无功能回归 | 回退P7.2独立commit，恢复`L-08`功能壳 |
| `L-10 / P7.3` | `L-09`已实现、集成并视觉验证 | 新建独立 task；先输出Before/After/Why，再仅使用 `$emil-design-eng` 审计与打磨 | 独立审计记录、polish实现、重新视觉验证 | findings一一闭合；与P7.2 task/record/commit不同；视觉与功能回归通过 | 失败只回退P7.3独立commit，保留已验证P7.2 |
| `L-11 / P7.4` | `L-10`及shared/public integration已通过 | 实现executable-anchored package-root解析、可写性fail-fast、单实例、随机loopback、launcher、graceful shutdown、嵌入assets与release build | 不含数据库/真实数据的Windows local archive候选 | CWD-independent、可写目录、allowlist/denylist、reproducible build、SBOM/license/secret/path/data scan、archive hash | 删除候选archive；回退package task，不触碰已有`data/` |
| `L-12 / P7.4` | `L-11` | 在干净Windows账户执行deterministic、fake-upstream、upgrade/rollback candidate验收并冻结hash | 候选资格记录、不可变hash、known-issues与P7.5入口 | 第7节非live部分全部通过；失败不得进入P7.5 | 保留失败证据，撤销候选资格，回到最早失败task |
| `L-13 / P7.5` | `L-12`与live预算/权限门通过 | 在干净Windows环境解压真实候选包并执行有界真实Rutgers E2E | 去敏请求ledger、真实Catalog/Open/浏览器WS/watch/toast/audio证据 | 第7节live部分全部通过；candidate hash不变；失败不得现场修包 | 回到最早owner task修复，重建两个新candidate并从头重跑全部P7.5 |

## 4. 本地运行与安全组合

1. `RBCSP.exe` 只监听随机 loopback port，启动时用 CSPRNG 生成 session nonce；不启用任意 CORS。
2. state-changing HTTP 与 WebSocket 同时校验 Origin 与 nonce；外部网页不能触发 Reset、watch 或设置变更。
3. 浏览器 current page 可关闭；只有显式退出或进程结束才停止全部 active watch。当前版本不增加托盘、隐藏常驻、自启动或系统通知。
4. `<package-root>`只由当前`RBCSP.exe`的已解析位置确定，不依赖进程CWD，也不回退到`LocalAppData`、`TEMP`、管理员目录或其它位置。若包根不可写，必须在创建网络client或发送任何Rutgers请求前明确失败；因此用户必须解压到可写目录。
5. 首次启动先原子创建`data/`与schema-only、零课程的`data/rbcsp.sqlite`；SQLite相邻WAL/SHM只是sidecar，不构成第二主库。operational与LOCAL_ONLY personal table/migration domain逻辑隔离但物理共存。
6. “重置本地用户数据”只按事务性allowlist清空personal tables；Catalog、Open operational state、checkpoint、observation与诊断计数不删除，也不删除主数据库文件。
7. launcher 不下载依赖、不提权、不修改PATH/注册表/防火墙，也不以 PowerShell execution-policy 旁路作为启动条件。

## 5. 数据、刷新与通知验收

### 5.1 Catalog 与 Open

- Catalog 默认600秒，允许1–1440分钟；Open普通默认30秒，允许3–3600秒；active-watch相关batch的requested effective目标为10秒。
- 同target用户/section/watch数不增加上游请求；Catalog/Open共用real-origin max concurrency `1`。
- 所有 attempt 产生attempt记录；所有valid response即使无变化也产生target refresh observation，并为watched section产生section observation。
- UI同时显示requested/effective/actual interval、last attempt/last valid、lag、fresh/stale/UNKNOWN、circuit和local run/Rutgers-day attempted/succeeded/failed/empty。
- valid observation到server fanout的工程目标为`<=1s`；浏览器音频还要求已连接、解锁且可播放，不能把该目标写成真实seat变化到声音的硬30秒SLA。

### 5.2 ONE_SHOT 与 CONTINUOUS

- ONE_SHOT对每个不同且valid的Open section observation发出cue，直到每section Max audible；默认3、正整数、无产品上限。
- 达cap后仅静音该section，不停止watch、toast或history；显式restart/resume/reset counter才归零。
- CONTINUOUS默认10分钟并支持Unlimited；按OpenEpisode启动，确认/timeout后同episode不重响，必须可靠Closed→Open才re-arm。
- 多section共用bounded mixer；A/C已Open时分别可确认，未确认D后来Open必须立即重新响；failure/unsafe/stale不制造Closed也不re-arm。

## 6. Upgrade 与回滚合同

Windows archive升级必须保留包根内同一`data/`，不得把用户状态静默迁移到包外：

1. 退出当前进程并完成WAL checkpoint；
2. 备份完整`data/rbcsp.sqlite`及其schema/version manifest；备份对象不能只含personal tables；
3. 在同一稳定包根之外暂存并离线验证新archive的`SHA256SUMS`、SBOM与allowlist，然后在应用关闭时替换程序/文档成员，明确保留原`data/`；若采用新包根，则必须显式复制并核验完整`data/`，不得产生两个同时活动的数据根；
4. 首启先对完整数据库副本做migration dry-run，再对`data/rbcsp.sqlite`事务迁移；
5. 验证startup、query、Saved views、history、settings、Open freshness与watch不自动恢复；
6. 观察通过后保留旧archive至少一个版本。

若migration可逆，关闭新版本后切回旧exe并执行明确定义的down migration；若不可逆，则必须恢复升级前一致性完整`rbcsp.sqlite`备份并使用对应旧binary。任何回滚不得只删除personal tables或整个包根代替，也不得伪造today counters或Open连续性。

## 7. Clean Windows最终验收清单

在无开发工具的全新标准用户账户与clean VM各执行一次：

- archive解压路径分别含空格、中文/Unicode和长路径；
- 从包根外不同CWD双击bat与直接exe均命中同一`data/rbcsp.sqlite`，浏览器打开，端口/nonce不固定且只监听loopback；只读包根在任何网络请求前明确失败且不回退；
- archive在首次运行前没有任何DB/WAL/SHM、seed或真实课程/Open数据；阻断网络首次启动后只创建schema-only主库且Catalog/Open/observation行数为0；
- 放行fake upstream后验证首次Catalog初始化、搜索、22筛选、course展开、section独立搜索/直接URL、mobile/desktop responsive；
- SQLite FTS5可用；重启保留prefs/selection/Saved views/history，不恢复active watch；
- 3/10/30/3600 fake-upstream用例、offline/timeout/429/5xx/malformed/unsafe empty/zero-intersection/catalog race；
- WS重连、disconnect清理、9 section、ONE_SHOT cap、CONTINUOUS A/C/D与Closed→Open；
- en-US/zh-CN parity、系统语言首次检测、Reset后重新检测；
- filter Reset、library delete-all、user-data Reset作用域；
- 升级、不可逆migration配套restore、旧version rollback；
- 无Node/Python/npm/Git/OpenSSL/管理员权限；Windows Defender/常见安全扫描无高风险；
- archive解压后hash一致，重复构建满足第05文档的reproducibility门；
- 完整删除解压目录会同时删除`data/rbcsp.sqlite`与用户数据；QUICKSTART必须在卸载步骤前要求用户按需备份完整主库，不得继续声称“删包不删数据”。

P7.5还必须在一台干净Windows环境对P7.4冻结的同一archive hash执行一次真实世界E2E：release仍无预装数据库；首次创建唯一主库；连接真实Rutgers Catalog/Open；动态选择而非硬编码课程；搜索并组合至少三个有结果筛选条件；打开真实Course与`(term,campus,index)` Section详情；证明Open集合只与同批Catalog Section正确关联；UI freshness、lag、attempt/success/failed/empty计数与去敏ledger一致；建立真实浏览器WebSocket；对当前真实Open section启动watch并验证状态与toast。声音只在浏览器已通过用户手势解锁且端点可用时作为live hard evidence；若不可用必须显示明确unavailable，不能伪报成功。若有界发现后没有任何真实Open section，则记为`LIVE_PRECONDITION_NOT_MET`并停止，不伪造、不无限等待。

## 8. 完成与停止条件

只有`L-00..L-12`对应的candidate gate通过，本地包才可成为P7.4冻结候选；它还必须由`L-13/P7.5`以完全相同hash通过真实世界E2E，才可进入最终P7完成记录。P7.5只产生证据，不构建第三个包。若必须引入管理员安装器、第三个进程、云账号、active订阅持久化、系统通知、ARM64必交付或第二套共享逻辑，停止回Review。

```text
phase=P6
plan=LOCAL_ONECLICK
status=P6_LOCAL_EXECUTION_PLAN_AMENDED_FOR_REVIEW
package=WINDOWS_LOCAL_RELEASE_ARCHIVE
required_architecture=X86_64_WINDOWS_MSVC
runtime_process_count=1
administrator_required=FALSE
node_or_python_required=FALSE
saved_views=LOCAL_ONLY_PERSISTENT
active_watch_persistent=FALSE
local_storage_root=PACKAGE_ROOT
local_storage_anchor=EXECUTABLE_PACKAGE_ROOT
local_primary_sqlite_relative_path=data/rbcsp.sqlite
local_primary_sqlite_count=1
local_operational_personal_tables_co_resident=TRUE
local_storage_cwd_dependent=FALSE
local_storage_fallback=NONE
local_first_start_creates_database=TRUE
local_release_contains_database=FALSE
local_release_contains_real_catalog_open_data=FALSE
origin_max_concurrency=1
p7_authorized=FALSE
package_builds=0
```
