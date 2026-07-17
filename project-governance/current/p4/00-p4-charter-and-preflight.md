# P4 Charter 与 Preflight — 公网包以及部署设计

## 1. 阶段结论与边界

- **阶段**：P4
- **状态**：`P4_CORE_DESIGN_FROZEN`
- **阶段输入门**：P3 `P3_PASS`
- **任务线**：`公网包以及部署`
- **本阶段产物性质**：在P3共享/本地完整基线上冻结公网delta与Linux部署计划
- **本阶段Rutgers请求预算**：`0`
- **产品源码、依赖、数据库与用户包变更预算**：`0`
- **生产服务器、DNS、Cloudflare、证书与GitHub Release变更预算**：`0`
- **下一阶段依赖**：P4验证通过后才能进入P5共享/变体边界设计

P4只回答“共享产品如何形成公网包，以及该包将来如何被部署”。它不实现Rust/React代码，不构建包，不触碰真实生产环境，也不重新取Rutgers证据。共享Catalog、Open、筛选、watch与通知语义全部继承P3冻结合同；P4只能增加公网运行约束或删除明确的`LOCAL_ONLY`能力，不能产生第二套核心产品。

## 2. 权威输入与固定SHA-256

本阶段读取当前主线P2/P3与权威工作流；旧chat log、旧P1执行产物和deprecated产物继续不构成输入。以下8个P3关键输入按原始文件字节固定；任一hash不符都使本P4设计失去输入资格，必须停止并解释差异，不能静默重算或用新文件覆盖。

| P3输入 | SHA-256 |
|---|---|
| `p3/07-local-oneclick-implementation-plan.md` | `26B6805284E50639F91FD6523365FD8C0A9607353A0EF6B99D20B6E8910AA634` |
| `p3/08-p3-traceability-matrix.tsv` | `234D2C8091B126CC594D6EA406AB9214339FD62FDC0348CDF60D360F434971EE` |
| `p3/09-p3-validation-and-freeze-gate.md` | `7CCB3CB067781961A17E7910D150325CC34B9390CEFEAE18E4F25A4E48CD9851` |
| `p3/11-open-request-ledger.tsv` | `DC7A44570C727AA3561A2468A361AB27191807E87A781E41EF81EFF730B7977E` |
| `p3/22a-open-round2-completion.md` | `20BC18B5B2ABAC82F97C559C02F0D6B7F5EE08792F710637E3790B62D85C9E6D` |
| `p3/22b-open-round2-completion.json` | `7014E38B6A142CD653CA387AD062D82176A7F372767ADADE2E7A47E123E88F94` |
| `p3/23a-shared-open-final-contract.md` | `73031DD718A346D659214D6BE4C571D505836098DE42879E8861349CBD73887E` |
| `p3/23b-shared-open-final-contract.json` | `9BFE6712E959C39B6719E7A47F7F96E33AAC5BD20A8CFEF51320C0E2A0C77804` |

`09`冻结P3总门；`22a/22b`与`11`冻结两轮Open证据及完整ledger；`23a/23b`冻结共享Open合同；`07/08`冻结本地完整基线与P2→P7追踪。P4不得直接解释raw evidence来改写这些结论，也不得发送新请求去“再确认”。

## 3. 两个包与一次独立部署

最终产品严格只有两个包：

1. `公网包`：面向Ubuntu 24.04 Linux服务器的版本化部署制品；
2. `本地一键包`：面向Windows普通用户的release archive。

真实生产部署是把已经验证的公网包安装到获批服务器上的独立变更活动，不是第三个包。P4可以冻结部署步骤、回滚策略和授权门，但不得执行这些步骤。P7构建/验证两个包后，真实部署仍需单独获得授权并重新核验域名、DNS、服务器、备份和凭据现场。

## 4. P4要完成的ALL

P4必须完整冻结以下公网delta：

1. **会话与状态**：每次top-level document load建立全新用户session；页面级状态不跨load持久化；语言每次按浏览器/系统初始化。
2. **能力边界**：公网无Saved views，无本地Persistent history，无本地Reset用户数据能力，无refresh配置；共享搜索、筛选、course/section、watch、toast与声音提醒保留。
3. **上游架构**：浏览器只访问BCSP；集中Catalog/Open poller持有Rutgers连接；WebSocket只做服务端已验证OpenObservation的live fanout。
4. **固定时钟**：Catalog固定600秒；普通Open固定30秒；有active watch的同一`(term,campus)` batch目标10秒。
5. **调度与安全**：term/campus固定batch、per-target single-flight、Catalog/Open共享Rutgers origin concurrency=1、EDF无饥饿、LKG/退避/circuit/catalog-race与真实lag展示。
6. **运维状态**：服务级Catalog/Open checkpoint、attempt与Rutgers-day counters可跨进程重启；它们不得含个人选择、watch映射或通知历史。
7. **Linux与边缘**：Ubuntu 24.04、非root systemd服务、Caddy反代与HTTPS、日志、备份/恢复、升级/回滚、secret边界和生产授权门。
8. **验证与追踪**：P4矩阵把每项delta映射到P7共享任务或`P7-PUBLIC-*`任务，并证明公网零Saved surface与页面reload零上游放大。

## 5. P4明确ONLY

以下内容不得在P4出现为实现或副产品：

- 新Rutgers网络请求、新raw证据、压力测试、cache busting或提高真实origin concurrency；
- Rust、TypeScript、CSS、SQL migration、package、lockfile或产品测试代码变更；
- SSH、服务器登录、systemctl/caddy真实执行、DNS/Cloudflare修改、证书申请或生产数据迁移；
- 公网用户账户、cloud sync、个人数据库、Saved views、Share links、URL filter恢复、邮件、Discord、Web Push、系统通知、waitlist或Calendar；
- 浏览器直连Rutgers，或让reload、用户数、tab数、section数、watch数线性放大上游请求；
- 把公网delta复制为第二套筛选、Open reconcile、WebSocket或React产品；
- 把部署说明、SBOM、运维备份、GitHub Release或报告计作第三个包。

## 6. 架构不变量

```text
browser page (ephemeral user state)
        |
        | HTTPS / same-origin API / WebSocket
        v
Caddy -> bcsp-server -> shared query + Open domain
                         |             |
                         |             +-> in-memory connection/watch fanout
                         v
                  shared service SQLite
                  (Catalog/Open/operational counters only)
                         |
                         v
                  central Rutgers poller
                  (one shared origin limiter)
```

- 页面从服务共享checkpoint查询；新页面不能触发一个新的Rutgers pull。
- 公网“无持久个人状态”不等于“无服务运行状态”。Catalog、Open LKG、attempt/aggregate、scheduler checkpoint和服务Rutgers-day counters是共享运维事实，可跨重启；用户filters、selected、audio设置、watch和history不是。
- 同一套共享domain contract服务本地和公网；公网adapter负责固定配置、ephemeral session与LOCAL_ONLY能力编译排除。
- 生产入口只监听loopback，由Caddy暴露HTTPS；浏览器不获得Rutgers endpoint访问路径。

## 7. 阶段停止条件

出现以下任一情况时，P4必须停止并回到Review：

- P3固定输入hash不一致，或需要修改P3共享合同才能完成公网设计；
- 设计要求浏览器直连Rutgers、真实origin concurrency大于1、动态跨campus union或另建Open状态源；
- 设计无法同时满足“每个top-level load新session”和“服务计数跨重启但不含个人状态”；
- 公网制品必须包含Saved views/local persistence源码、API、route、storage key、i18n或bundle surface；
- 需要真实服务器、DNS、凭据或生产变更才能证明P4设计；
- 将部署活动或文档包装成第三个最终包；
- 发现无法通过fake-upstream验证的容量、LKG、退避、无饥饿或lag诚实显示冲突。

## 8. P4到P5/P6/P7的交接

- P5必须把本文件与P4 delta矩阵转成共享core、local adapter、public adapter和编译边界，不得形成长期代码fork。
- P6必须把公网包、Windows本地包、共享实现DAG、P7任务/测试/发布顺序和真实部署授权边界合并为最终计划，并在P6 Review停止。
- P7才可实现`P7-SHARED-DOMAIN/CATALOG/QUERY/OPEN/WATCH/UI/I18N`与`P7-PUBLIC-SESSION/RUNTIME/OPS/PACKAGE/ZERO-SURFACE`；实际生产部署仍不在P7自动授权内。

## 9. Machine-readable state

```text
phase=P4
status=P4_CORE_DESIGN_FROZEN
p3_input_gate=P3_PASS
p3_key_inputs_pinned=8
p4_scope=PUBLIC_PACKAGE_AND_DEPLOYMENT_PLAN
rutgers_requests_authorized=0
p4_rutgers_request_artifacts=0
product_source_changes_authorized=0
production_changes_authorized=0
production_mutations=0
deployment_execution_authorized=FALSE
deployment_is_third_package=FALSE
final_package_count=2
p5_entry_requires_p4_validation=TRUE
p7_authorized=FALSE
```
