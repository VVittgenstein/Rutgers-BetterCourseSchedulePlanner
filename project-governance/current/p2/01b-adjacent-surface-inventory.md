# P2 邻接表面清单

## 1. 目的

`01-file-universe.tsv` 的 N/N 分母只包含 158 个 tracked、产品拥有、允许读取的文件。本文件登记其余会影响发现、运行、Git状态或宽泛打包的物理表面，避免把它们漏掉或错误算作产品源码。

## 2. Generated、vendor 与 runtime

| Surface | 启动快照 | Git状态 | 内容读取 | 三轴与package去向 |
|---|---:|---|---|---|
| root `node_modules/**` | 3,646 files / 62,690,047 bytes | ignored | 未作语义审计 | `INTERNAL_ONLY / REMOVE / INTERNAL_TOOLING`；vendor不得进包 |
| `frontend/node_modules/**` | 2,843 files / 66,671,252 bytes | ignored | 未作语义审计 | `INTERNAL_ONLY / REMOVE / INTERNAL_TOOLING`；由lock重建，不进包 |
| `frontend/dist/**` | 3 files / 298,599 bytes | ignored | 只作静态制品字符串/可达性审计 | `INTERNAL_ONLY / REMOVE / INTERNAL_TOOLING`；含mail/claim/Playground旧表面，必须从获批source重建 |
| `frontend/tsconfig.tsbuildinfo` | 1 file / 1,010 bytes | ignored | metadata/hash only | `INTERNAL_ONLY / REMOVE / INTERNAL_TOOLING` |
| `data/runtime/**` | 1 runtime JSON / 1,887 bytes | ignored | 正文未作为产品证据 | `INTERNAL_ONLY / REMOVE / INTERNAL_TOOLING`；不进包 |
| `data/*.db-*`, `*.sqlite-*`, migration log, poller checkpoint | 6 physical runtime/sidecar/log paths | ignored | 不读取DB或私有运行内容 | `INTERNAL_ONLY / REMOVE / INTERNAL_TOOLING`；运行目录隔离 |
| `scripts/poller_checkpoint.json` | 1 ignored checkpoint | ignored | 仅旧运行态证据 | `INTERNAL_ONLY / REMOVE / HISTORICAL_EVIDENCE`；不进包 |
| ignored local/user configs | 2 physical paths | ignored | 不读取正文；其中mail user config属于ONLY闭包 | mail配置`EXCLUDED/REMOVE`；公网local config `CARRY_TO_P4` metadata only |

`frontend/dist` 启动快照：

- `index.html`：482 bytes，SHA-256 `BF0E67B2B557...`
- CSS：24,765 bytes，SHA-256 `BC49922D3828...`
- JS：273,352 bytes，SHA-256 `297643B79246...`

这些是陈旧制品证据，不是当前交付输入。

## 3. 旧 archive

| Archive | Bytes | SHA-256 | Metadata count | P2去向 |
|---|---:|---|---:|---|
| `bcsp-20260122.zip` | 245,913 | `48E976EF9B2EFCBFB692F6CC119C790BF7D4D3E9A361C464E38F61A89A5AFAD1` | 129 entries / 125 files | `HISTORICAL_EVIDENCE / OUT_OF_SCOPE` |
| `release/bcsp-20260121.zip` | 273,969 | `F62F14D2CEE0DE4BD90931E37808141FB45DF970E39CFF7BAAB78E9A999A9A50` | 136 entries / 136 files | `HISTORICAL_EVIDENCE / OUT_OF_SCOPE` |
| `release/bcsp-20260121.tar.gz` | 222,193 | `827D2EF1F59357780AC70A92F489A92246D5EC4BC1EDE2BEF2EC1CBFA63951AD` | 166 entries / 136 files | `HISTORICAL_EVIDENCE / OUT_OF_SCOPE` |

只读取容器hash和目录metadata，没有解压写盘。三个旧包均不是自包含Windows产品；含源码、旧启动器、mail/tests，REL21还含runtime checkpoint。不得把它们嵌入、重发或当canonical build。

## 4. 历史证据表面

| Surface | 启动快照 | 使用边界 | 三轴去向 |
|---|---:|---|---|
| `docs/archive/stage-a-legacy/**` | 88 files / 761,751 bytes | 只作历史发现；不作为当前权威 | `INTERNAL_ONLY / OUT_OF_SCOPE / HISTORICAL_EVIDENCE` |
| `docs/archive/stage-a-legacy/Compact/**` | 74 files / 200,822 bytes；path+hash tree digest `307CB90739F2EFA8CE41693F1451010111B286669A00277AB71DC1D7760C3A78` | 用户指出旧RBCSP的Compact重要；获批P1已优先恢复，本P2用于交叉发现而不继承旧分类 | `INTERNAL_ONLY / OUT_OF_SCOPE / HISTORICAL_EVIDENCE` |
| root `read_only.md` | tracked pointer | 仅指向旧archive且声明自身非权威 | `INTERNAL_ONLY / OUT_OF_SCOPE / HISTORICAL_EVIDENCE` |
| `project-governance/current/p1/**` | 8 files / 159,639 bytes | 获批P1，属于P2正式输入；不是runtime/package | `INTERNAL_ONLY / REUSE_AS_IS / HISTORICAL_EVIDENCE` |
| reports | 7 files，已在158矩阵逐文件裁决 | 方法/fixture来源；历史运行不能升级为当前通过 | `INTERNAL_ONLY`或mail `EXCLUDED`；全部不进包 |

## 5. Untracked、deleted 与流程表面

| Surface | 状态 | 内容使用 | 去向 |
|---|---|---|---|
| current `project-governance/**` | untracked | 当前权威workflow、获批P1和本P2治理材料 | `INTERNAL_ONLY / REUSE_AS_IS / INTERNAL_TOOLING`；不进用户包 |
| `CLAUDE.md`、`.claude/**` | untracked/ignored internal tooling | 不作为产品requirement | `INTERNAL_ONLY / OUT_OF_SCOPE / INTERNAL_TOOLING`；宽泛归档拒绝 |
| chat-log、session logs、DIGEST/TIMELINE/registry | untracked history/index | 不代替权威P1/P2，也不进产品包 | `INTERNAL_ONLY / OUT_OF_SCOPE / HISTORICAL_EVIDENCE` |
| 明确废弃workflow文档 | untracked | 未用作规范或阶段证明 | `INTERNAL_ONLY / OUT_OF_SCOPE / HISTORICAL_EVIDENCE` |
| `.orchestrator/**`与`AGENTS.md` | tracked但启动时已由用户删除 | P2不恢复、不读取missing内容 | dirty baseline only；不进入当前产品矩阵 |
| `.worktrees/**` | ignored/internal | 不进入审计语义或包 | `INTERNAL_ONLY / OUT_OF_SCOPE / INTERNAL_TOOLING` |

## 6. 禁读与私有metadata-only表面

| Surface | 处理 |
|---|---|
| 禁读旧P1文件/目录 | 只登记隔离规则；未打开、未搜索正文、未引用结论；不列子项 |
| `.ngagent/**`及旧NGAT/Organ派生物 | 整体隔离；不读取内容、不计入语义矩阵、不列子项 |
| `.secrets/**`与private inventory | 已确认根路径被ignore；不读取正文、不记录子文件名、IP、UUID、fingerprint或key material |
| ignored secret-bearing config | 只登记已知源码引用的配置路径和ignore状态；不hash、不显示size/正文 |

## 7. Package风险闭合

即使某表面不在158个产品源码分母，它仍必须被P7.4正向allowlist拒绝。尤其是：

- vendor/generated/runtime；
- untracked internal/history/chat；
- private/secret；
- old release；
- governance/P1/P2；
- Compact/archive/reports；
- deleted/ignored worktree或agent状态。

因此“158/158”不是“磁盘上只有158个文件”，而是“158个产品拥有文件逐文件裁决，所有其他物理表面另有明确类别和package去向”。
