# 公共版（Linux）上线前加固 Checklist

状态：v2 —— **已批准**（随 Codex 终审 2026-08-20 通过）。
定位：修改**现在做进仓库**（配置与代码即刻可改），真机验证待重新部署——
Vultr 实例已由产品负责人主动停机。H9 与公网组装集成测试为**重新上线硬门**。
日期：2026-08-20
来源：2026-08-20 公共版 watch/WebSocket 三路审计（代码 / 部署配置 / 线上探测）。

## 0. 审计结论摘要

- 线上探测：`140.82.9.50` 全端口静默（22/80/443/ICMP，IPv4+IPv6）——实例
  已停机（产品负责人确认为主动关闭）。配置快照显示 `public_domain: null`、
  Caddyfile 仍为 `planner.invalid` 占位、UFW 仅曾开放 22 端口：**公共 HTTPS
  边缘从未真正上线过**。
- 代码骨架健康：WS 路由/快车道激活与退场/断连清理/前端完整性均验证无误；
  CI 含真 Caddy + 真浏览器的 `bcsp.v1` 握手验证。
- 风险集中在运维配置与滥用面，见下。

## 1. 必修项（重新部署前）

每项含：现状 → 后果 → 修法 → 验证。

### H1. systemd 文件描述符上限（严重度：中，一行修复）
- 现状：`deploy/public/systemd/bcsp.service` 无 `LimitNOFILE`；二进制不自调
  rlimit。Ubuntu 24.04 systemd 服务默认软限 1024。
- 后果：每个 WS/HTTP keep-alive/SQLite 句柄各占一个 FD；选课日约千级并发
  即 EMFILE——新连接（含 `upgrade.sh` 的 `/health/live` 探针）全部被拒。
- 修法：`[Service]` 加 `LimitNOFILE=65536`。
- 验证：部署后 `systemctl show bcsp -p LimitNOFILE`；压测千连接不报 EMFILE。

### H2. Caddy reload 切断所有监控长连接（严重度：中）
- 现状：`Caddyfile.example:18` 无 `stream_close_delay`；Caddy ≥2.7 reload 时
  立即关闭全部已升级连接；runbook:48 将"validate and reload"列为日常操作。
- 后果：运维每 reload 一次，全部在线用户的监控被切断（提醒必达设计落地后
  可自动恢复，但仍有秒级窗口 + 服务器端 watch 状态重建）。
- 修法：`reverse_proxy { stream_close_delay 4h }`；runbook 加显著警告与
  低峰窗口建议。
- 验证：挂一条 WS，执行 `caddy reload`，断言连接存活。

### H3. 部署脚本无条件重启（严重度：中）
- 现状：`deploy/public/ops/upgrade.sh:26-37` 即使 release id 未变化也执行
  `bcsp_restart_service`。
- 后果：无变化的例行升级也清空全部 watch（`seal_and_stop` + watch 不持久化）。
- 修法：release id 相同时跳过重启；runbook 注明部署=断线事件、建议低峰执行。
- 验证：同 id 二次运行 upgrade.sh，断言服务 PID 不变。

### H4. WS 连接数无上限（严重度：中，滥用面）
- 现状：`handle_watch_socket`（bcsp-public-runtime/src/host.rs:508-530）无
  per-session/全局连接上限；axum::serve 无并发层；一个页面 nonce 可开无限
  socket；每 socket = tokio task + unbounded sender + CoreConnection。
- 后果：单客户端可廉价耗尽 1 GB 内存/任务额度（DoS）。
- 修法（v2 补全）：
  a) 全局 WS 连接上限（如 2048，超出 503）+ 每 session 上限（如 3）；
  b) **出站背压**：每连接 outbound 由 unbounded channel（host.rs:418）改为
     **有界队列**（如 256 帧），写满即判慢消费者、主动断开该连接——连接数
     封顶约束不了单 socket 的队列膨胀，两者缺一不可；
  c) 每 IP 限制：Caddy 标准 directive **不含** rate limit（评审指出），
     指明采用 `mholt/caddy-ratelimit` 第三方模块（xcaddy 构建，部署方式
     写入 runbook），或退而在应用层按 remote addr 限流——二选一在实施时
     定，不留"配一下 Caddy"的模糊表述。
- 验证：并发开 N+1 连接断言第 N+1 被拒；灌满出站队列断言慢消费者被断开；
  metrics `bcsp_websocket_connections` 可观测。

### H5. 匿名首页请求驱逐真实会话（严重度：中，滥用面）
- 现状：每次 document GET 无条件签发 nonce；registry 容量 4096，满时 LRU
  驱逐最旧（session.rs:126-139）。
- 后果：bot 刷 ~4096 次首页即可吊销全部在线用户的会话（现有 WS 存活但无法
  re-arm/换证）。
- 修法（v2 重写，评审指出原案"无过期即拒发"会反转成对新用户的 DoS——
  4096 个未过期但无连接的 nonce 即可封站两小时）：
  a) registry 为每个 nonce 跟踪 `active_ws_count`（**仅计公网 watch WS**
     ——经 reserve_ws 租约 +1、lease drop -1；presence 为 local-only，
     与公网 registry 无关）；
  b) 驱逐顺序：已过期 → 最旧且 `active_ws_count == 0` 的 inactive →
     **仅当全部 nonce 都有活跃连接**（真容量耗尽）才对新请求 **503**
     （与 validate 合同统一：容量耗尽 503，429 专用于限流）；
  c) validate 换证签发新 nonce 时**废弃旧 nonce**，防错误重试自灌满；
  d) Caddy 侧对 `/` 加速率限制（模块要求同 H4c）。
- 验证：4096+ 次匿名 GET 后既有活跃会话仍有效；4096 个无连接 nonce 场景
  下新用户仍可获得会话；全活跃场景才 503。

### H6. 崩溃循环后服务永久趴下（严重度：低）
- 现状：`Restart=on-failure` + `StartLimitBurst=5`/`60s`；无 MemoryHigh；
  ~955 MiB 主机。
- 后果：OOM 循环 5 次后 unit 进入 failed，直到人工 `reset-failed`。
- 修法：`StartLimitIntervalSec=0`（或大幅放宽）+ `MemoryHigh=700M` 软压 +
  runbook 加告警建议（journalctl 监控）。
- 验证：kill -9 六连，断言服务仍自动恢复。

### H7. Origin/Host 大小写严格比较（严重度：低，配置陷阱）
- 现状：`host.rs:516,791-793` 对 `BCSP_PUBLIC_ORIGIN` 做区分大小写全等比较；
  env schema 接受大写主机名；ops 探针用同一 env 串做 Host 头所以照常通过。
- 后果：运维写 `https://Planner.Example` → 浏览器全部 403/421，而健康检查
  全绿——静默全站不可用。
- 修法：入库时对 authority 做小写规范化（或比较时 case-insensitive）。
- 验证：大写 origin 配置下浏览器握手成功。

### H8. 上线配置本身（严重度：前置条件）
- 申请/绑定真实域名 → `Caddyfile` 替换 `planner.invalid`、
  `BCSP_PUBLIC_ORIGIN` 写入真实 origin；
- UFW 开放 80/443（快照显示从未开过）；
- 快照遗留的安全项：关闭 root SSH 密码登录（capture notes 已标记）；
- `fwupd` failed units 清理（外观问题）。

### H9. WS 长连接 soak 测试（**重新上线硬门**，与公网组装集成测试并列）
- 现状：CI 最长 WS 生命周期 ~54 秒（real-world-browser.mjs:561-585）；从未
  测试过跨 60s 心跳窗口或多分钟连接穿越真 Caddy。
- 修法：加一个 10 分钟 soak（真 Caddy + 真浏览器，静默挂机），断言心跳
  往返、连接不掉、内存平稳。
- 验证：CI 通过即验证。

## 2. 记录在案、不排期（低优先杂项）

- 快车道在目标 primary workflow 进行中不激活（refresh_runtime.rs:515-555；
  受限于 workflow 时长，watch 已在时有并行通道兜住）；
- catalog 期间 watch 数归零时的 250ms 空转 fork（refresh_coordinator.rs:752-754，
  纯 CPU 浪费）；
- 公共 bootstrap 死载荷 + volume 100/70 不一致（host.rs:628-641 vs
  LiveWatchProvider.tsx:273）——若做 bootstrap 水合再一并清理；
- 零窗口 TCP 对端可将 socket 任务钉到 OS 超时（受 H4 上限约束后影响有界）。

## 3. 依赖关系

- H2/H3 的用户侧影响由《提醒必达》设计（自动重连）兜底，但兜底≠豁免：
  加固项独立成立；
- H5 的最优修法依赖《提醒必达》的"WS 活动续期 nonce"（同一处代码改动）。
