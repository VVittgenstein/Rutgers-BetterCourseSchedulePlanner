# P1 启动前基线

## 1. 捕获信息

- **捕获日期**：2026-07-12
- **工作区**：`Z:\Project\Rutgers-BetterCourseSchedulePlanner`
- **当前分支**：`dev`
- **当前 HEAD**：`a4b035a586a4b14fc3a75698caf99badce869fd5`
- **权威工作流 SHA-256**：`EF199DAC55CD9D919E2CC3DABEFD80709B626A0B24DF8CA0A3AB369465E52B56`
- **远端**：`origin = https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner`

## 2. 工作树状态

P1 开始前工作树已经是有意义的脏状态：

- tracked deletion：24
- tracked modification：1
- untracked path：20
- porcelain entry 总数：45

这些既存状态属于用户历史现场。P1 不得清理、恢复、覆盖、暂存、提交或借机重写它们。

主要类别包括：

- `.orchestrator/` 历史文件的既存删除；
- `AGENTS.md` 的既存删除；
- 旧 P1 主文档的既存修改；
- 根目录会话日志和旧 `docs/` 产物的未跟踪状态；
- 当前新建的 `project-governance/`。

路径元数据只用于保护现场，不授权读取旧 P1 文件内容。

## 3. 来源可达性预检

以下来源根在 P1 启动时存在：

- 当前权威工作流
- `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner`
- `Z:\.claude`
- `Z:\.codex`
- `C:\Users\Administrator\.codex`
- 本地 `origin` 远端配置
- 本地 `feature/task-015` 分支

存在只表示可供后续定向调查，不表示可以无差别扫描个人历史。原始会话必须严格限定为 RBCSP 项目相关记录。

## 4. Git 污染隔离

当前 `dev` HEAD 位于旧 P1 执行后的历史区域，因此：

- 不得从当前 HEAD 直接读取旧 P1 文档或旧 P1 commit 内容。
- Git 调查必须先仅用 commit metadata、parent、path name 和时间建立旧 P1 禁区与旧项目安全截止线。
- 产品源码和旧历史的读取必须显式排除 charter 第 5 节中的路径和旧 P1 commit 范围。
- `feature/task-015` 是旧 RBCSP 产品历史来源，但不得通过旧 P1 的后续整理产物间接读取它。

## 5. 写入隔离

本次 P1 唯一允许写入的目录：

`project-governance/current/p1/`

P1 不使用旧 `docs/p1-a-recovery/`，不修改废弃流程文件，不向 `.ngagent/` 写入状态。

## 6. 启动结论

用户已经明确批准正式进入 P1。预检确认必要来源根和 Git 历史入口存在；P1 可以在上述禁区、工作树保护和单主线规则下继续。
