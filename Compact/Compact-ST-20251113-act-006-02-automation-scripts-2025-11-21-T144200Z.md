# Compact — ST-20251113-act-006-02-automation-scripts

## 已落实事实
- 新增 `scripts/setup_local_env.sh`：校验 node/npm，生成缺失的本地配置（fetch/mail/Discord）、`.env.local` 示例，安装根/前端依赖，跑迁移到 `data/local.db`，并按指定 term/campus 触发抓取（默认 full-init，可 `--skip-fetch`/`--skip-frontend-install`/`--subjects`/`--mode incremental`）。
- 新增 `scripts/run_stack.sh`：一键后台启动 API + 前端 + openSections poller，日志写入 `logs/run_stack/*.log`，监控任一子进程退出即整体退出；可选 `--with-mail`/`--with-discord`（要求 SENDGRID_API_KEY / DISCORD_BOT_TOKEN 与对应 config），支持自定义 ports/DB/interval/checkpoint、允许频道列表，默认链接基于 `http://localhost:5174`。
- 两个脚本均已设置可执行权限（`chmod +x`），与文档中 `./scripts/...` 的调用方式一致。
- 部署手册补充“Automation shortcuts”段落，给出上述脚本的典型命令与凭据要求，方便从零起步。

## 接口/行为变更
- 无 API 变更；新增两个可执行脚本用于本地自动化，并约定默认日志目录 `logs/run_stack/`。

## 自测情况
- `bash -n scripts/setup_local_env.sh`、`bash -n scripts/run_stack.sh` 通过；未实际跑安装/抓取/启动流程。

## 风险/限制/TODO
- mail/Discord dispatcher 启动仍依赖真实凭据与模板；脚本只在显式 `--with-*` 时检查 env/config。
- 默认生成的 fetch config 只覆盖首个 term/campus，更多组合需手动编辑 `configs/fetch_pipeline.local.json` 或通过 CLI 覆盖。***

## Code Review - ST-20251113-act-006-02-automation-scripts - 2025-11-21T14:49:44Z

Codex Review: Didn't find any major issues. 🚀

## Code Review - ST-20251113-act-006-02-automation-scripts - 2025-11-21T15:53:09Z

- P1: `scripts/run_stack.sh` / `scripts/setup_local_env.sh` 提交时未带可执行位，导致按文档直接运行遇到 permission denied（已通过 `git update-index --chmod=+x` 修复）。***

## Code Review - ST-20251113-act-006-02-automation-scripts - 2025-11-21T16:09:26Z

#1
scripts/run_stack.sh



@@ -0,0 +1,325 @@
#!/usr/bin/env bash



@chatgpt-codex-connector
chatgpt-codex-connector bot

P1 Badge Add execute bit to run_stack.sh

On POSIX systems ./scripts/run_stack.sh currently exits with “Permission denied” because the file was added with mode 100644 (not executable) even though the docs now instruct running it directly; the shebang at the top implies it should be invoked as a program. Set the executable bit (e.g., 755) or adjust the usage so the one-step stack start command actually works.

#2
scripts/setup_local_env.sh



@@ -0,0 +1,292 @@
#!/usr/bin/env bash



@chatgpt-codex-connector
chatgpt-codex-connector bot

P1 Badge Add execute bit to setup_local_env.sh

Similarly, scripts/setup_local_env.sh was committed as 100644 instead of executable, so the documented command ./scripts/setup_local_env.sh ... fails with a permission error unless the user prefixes bash or manually chmods it; the shebang indicates it should be runnable. Mark it executable so bootstrapping works as advertised.

## Code Review - ST-20251113-act-006-02-automation-scripts - 2025-11-21T16:17:13Z

Codex Review: Didn't find any major issues. Keep them coming!
