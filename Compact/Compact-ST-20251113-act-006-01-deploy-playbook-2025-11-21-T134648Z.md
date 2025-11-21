# Compact — ST-20251113-act-006-01-deploy-playbook

## 已落实事实
- 新增部署手册 `docs/deployment_playbook.md`（时间戳 2025-11-22），面向本地一键运行：涵盖先决条件、配置拷贝、带示例命令的启动顺序、验证与排障。
- 手册内容要点：
  - 先决条件：Node.js v22、npm、SQLite 可选、Python 可选、外网访问 SOC/SendGrid/Discord、SENDGRID_API_KEY/SMTP/Discord Bot Token 准备。
  - 配置准备：复制 `configs/fetch_pipeline.example.json`/`mail_sender.example.json`/`discord_bot.example.json` 到本地版本并调整；可选 `.env.local` 示例（API/前端 Vite 代理）。
  - 启动顺序：`npm install`（根+frontend）、`npm run db:migrate -- --db data/local.db --verbose`、`npm run data:fetch -- --config configs/fetch_pipeline.local.json --mode full-init --terms 12024 --campuses NB`、API `APP_PORT=3333 SQLITE_FILE=data/local.db npm run api:start`、前端 `VITE_API_PROXY_TARGET=http://localhost:3333 npm run dev -- --host 0.0.0.0 --port 5174`、openSections poller（含 checkpoint/metrics）、mail_dispatcher（需 SENDGRID_API_KEY）、discord_dispatcher（需 token + allow-channel）。
  - 验证流：SQLite 计数 + fetch summary，API `/api/ready` + `/api/courses` 取样，前端加载无 fallback；订阅→通知：用 `sqlite3` 查 index，`curl /api/subscribe` 创建，强制关闭->轮询 `--once` 触发事件，检查 `open_event_notifications` pending，然后运行 dispatchers 直至清空；支持 mail/Discord 干跑工具（`scripts/mail_e2e_sim.ts`、`npm run discord:test-send -- --dryRun`）。
  - 排障表：覆盖 SOC 429、API 503/schema 缺失、前端 fallback、未入队通知、邮件/Discord dry-run 或 allowlist 问题等。
- `record.json` 中子任务 ST-20251113-act-006-01-deploy-playbook 状态改为 `done`，updated_at 更新为 2025-11-22T02:05:00Z；任务总节点 updated_at 同步更新。

## 接口/行为变更
- 无代码接口变更；新增文档与任务状态更新，带来部署操作指引及默认端口/路径约定（API 3333、Vite 5174、SQLite `data/local.db`）。

## 自测情况
- 未运行自动化测试与实际部署流程（文档变更）。

## 风险/限制/TODO
- 手册中的命令依赖真实凭据（SendGrid/SMTP、Discord），未验证在全新环境中的实际可行性。
- 强制触发订阅通知使用直接修改 SQLite 并在 poller `--once` 下跑一次，需注意恢复真实状态（手册建议重新跑 incremental fetch）；可能与现有 checkpoint 状态冲突。
- Vite/API 端口及代理假设可能与现有环境差异，需按实际调整。***
