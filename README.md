# 课程筛选与订阅工具 / Course Filter & Subscription Tool

本工具用于自动爬取课程数据，支持多条件筛选，并提供邮件提醒或本地声音提醒功能，帮助你更方便地抢课和订阅感兴趣的课程。

This tool crawls course data, supports flexible filtering, and provides email or local sound notifications to help you monitor and subscribe to desired courses more easily.

---

## 使用指南（零基础一步一步） / Beginner-Friendly Guide

（下面的步骤默认你几乎没有电脑/代码操作经验：只需要“下载 → 解压 → 双击启动 → 打开网页 → 点击按钮”。）

### 0）你需要准备什么？

- 一台电脑（Windows 10/11 或 macOS）。
- 能联网（第一次启动会自动下载一些依赖，时间可能较久）。
- 一个浏览器（Chrome / Edge / Safari 均可）。
- 可选：如果想用邮件提醒，需要一个 SendGrid 账号；如果不想折腾，直接用「本地声音提醒」最简单。

---

### 1）下载（只做一次）

1. 打开项目的 Releases（发布）页面：`https://github.com/VVittgenstein/BetterCourseSchedulePlanner/releases`
2. 下载最新版本的压缩包（通常是 `bcsp-xxxxxx.zip`）。
3. 如果别人已经把压缩包发给你了：跳过这一步，直接去第 2 步。

---

### 2）解压（把压缩包“拆开”）

> 建议解压到一个路径简单的文件夹，避免中文、空格、网盘同步目录（例如 OneDrive/Google Drive）。

**Windows**

1. 打开“下载”文件夹，找到刚下载的 `bcsp-xxxxxx.zip`。
2. 右键它 → 选择“全部解压…”。
3. 选择一个位置，例如：`C:\\BCSP\\` 或者桌面上的 `BCSP` 文件夹 → 点击“解压”。
4. 解压完成后，进入新出现的文件夹。

**macOS**

1. 打开“下载(Downloads)”文件夹，找到 `bcsp-xxxxxx.zip`。
2. 双击它，系统会自动解压成一个同名文件夹。
3. 打开这个解压后的文件夹。

解压后的文件夹里，你应该能看到：`Start-WebUI.bat`（Windows 用）和 `Start-WebUI.command`（macOS 用）。

---

### 3）启动程序（只需要双击）

> 启动后会弹出一个黑色窗口（Windows）或终端窗口（macOS）。请不要关闭它：关闭就等于停止运行。

**Windows**

1. 在解压后的文件夹里，找到 `Start-WebUI.bat`。
2. 双击运行。
3. 如果弹出“Windows 已保护你的电脑”：点击“更多信息”→“仍要运行”。

**macOS**

1. 在解压后的文件夹里，找到 `Start-WebUI.command`。
2. 双击运行。
3. 如果提示“无法打开/来自未知开发者”：右键该文件 → 选择“打开” → 再点一次“打开”。（必要时去 系统设置 → 隐私与安全性 → 允许打开。）

---

### 4）第一次可能会提示安装 Node.js（按提示装就行）

如果你双击启动脚本后，系统自动打开了 Node.js 官网，或者窗口里提示 “Node 22+ required / Node.js 未安装”：

1. 在打开的官网页面下载 **LTS** 版本（建议 v22 或更高）。
2. 运行安装程序，基本一路“下一步/继续”即可（保持默认选项）。
3. 安装完成后：把刚才那个黑色/终端窗口关掉，再回到第 3 步重新双击启动脚本。
4. 如果仍提示未安装：重启电脑后再双击启动脚本。

---

### 5）打开网页界面（你真正操作的地方）

1. 启动脚本运行后，通常会自动打开浏览器。
2. 如果没有自动打开：手动打开浏览器，在地址栏输入 `http://localhost:5174` 然后回车。

> `localhost` 的意思是“这台电脑自己”，不会把网页公开到互联网。

---

### 6）拉取课程数据（必须做，否则右侧列表是空的）

在网页左侧最上方，找到 **「拉取数据」(Term data fetch)** 这一块：

1. 在 **学年(Year)** 输入/选择年份（例如 2026）。
2. 在 **学期(Semester)** 选择：春(Spring) / 夏(Summer) / 秋(Fall) / 冬(Winter)。
3. 在 **校区代码(Campus code)** 输入：
   - `NB` = New Brunswick（不确定就选这个）
   - `NK` = Newark
   - `CM` = Camden
4. 点击 **「开始」(Run fetch)**。
5. 等待状态变成 **「完成…」/ “Done …”**（第一次可能需要几分钟）。

完成后，工具会自动切换到刚刚拉取的学期/校区，并刷新筛选字典；一般不需要手动刷新网页。如果你仍然看不到课程列表，按一次浏览器刷新（F5）。

---

### 7）筛选课程（找到你要的课）

1. 左侧找到 **“Build your schedule”**（筛选器区）。你可以只用最常用的几个：
   - **Search keyword**：输入课程名或课程编号（例如 “CS”/“198”）。
   - **Show sections → 有空位(Open only)**：只看还有空位的节次。
   - **Subject / Day of week / Time / Credits**：按科目、上课日、时间、学分等筛选。
2. 右侧会显示课程列表。每门课下面都有若干个 **Index（节次索引，一般是 5 位数字）** 和状态（有空位/已满）。

---

### 8）订阅空位提醒（可选：邮件或本地声音）

左侧找到 **「订阅入口」(Subscription center)**：

1. 先确认顶部小徽标里显示的是你的 **校区 + 学期**（不要是“未选校区/未选学期”）。如果没选好，请回到第 6 步先拉取数据。
2. 在右侧课程列表里找到你想监控的节次，把它的 **Index（5 位数字）** 记下来。
3. 把这个数字填到 **「节次索引」**。
4. 选择提醒方式：
   - **邮箱(Email)**：在下面输入你的邮箱地址。
   - **声音(Sound)**：保持这个网页打开；如果提示“点击开启声音”，点一下并允许浏览器播放声音。
5. 点击 **「提交订阅」**。看到“订阅已保存”就成功了。

**取消订阅**

- 在页面底部/右下角的 **「当前监控的课程」(Active subscriptions)** 里，点击 **「取消订阅」** 即可停止监控。

---

### 9）想用邮件提醒？（可选：SendGrid 一次性设置）

> 如果你只想要提醒、不想注册服务：建议直接用「声音」最省事。

**A. 在 SendGrid 做两件事：验证发件邮箱 + 生成 API Key**

1. 打开 `https://sendgrid.com/` 注册账号并完成邮箱验证。
2. 在 SendGrid 后台找到：**Settings → Sender Authentication → Single Sender Verification**，按提示验证一个发件邮箱。
3. 在 SendGrid 后台找到：**Settings → API Keys → Create API Key**，创建一个 API Key 并复制保存（只会显示一次）。

**B. 回到本工具填写邮件设置**

1. 在网页底部找到 **「邮件设置」(Mail settings)**。
2. 填写：
   - **From 邮箱**：填你在 SendGrid 验证过的那个发件邮箱。
   - **SendGrid API Key**：粘贴刚复制的 Key。
   - **Dry-run**：这是“测试模式”。想要真的发邮件，就把 Dry-run 关闭；不确定就先保持开启，确认流程没问题再关。
3. 点击 **「保存邮件设置」**。
4. 重要：保存后需要 **重启** 才生效——把启动脚本的黑色/终端窗口关掉，再回到第 3 步重新启动。

**提示**

- Gmail 可能会把提醒邮件放进垃圾邮件/推广邮件，第一次请去那两个目录找一下。

---

### 10）如何关闭/停止？

- 直接关闭启动脚本打开的黑色窗口（Windows）或终端窗口（macOS）即可停止。
- 关闭后，爬虫/监控/提醒都会停止；想继续监控就保持窗口开着。

---

### 11）常见问题（卡住就看这里）

- **双击启动脚本后“一闪就没了”**：通常是 Node.js 未安装或版本过低；按第 4 步安装/升级 Node.js 后再试。
- **浏览器打不开页面 / 页面显示无法访问**：确认启动脚本窗口还在运行；然后手动打开 `http://localhost:5174`；仍不行就把窗口关掉重新启动一次。
- **一直停在“执行中/Fetching…”**：第一次可能较久；请耐心等 5–10 分钟。网络较慢也会变久。
- **Windows 报错提到 `better-sqlite3` / “Microsoft C++ Build Tools”**：按提示安装 “Visual Studio Build Tools（Desktop development with C++）”，然后重新启动脚本。
- **没有声音**：浏览器可能禁止自动播放；在「声音提醒」区域点击“点击开启声音/Enable sound”，并检查浏览器标签页是否静音。
- **邮件不来**：确认你已关闭 Dry-run、From 邮箱已在 SendGrid 验证、保存后已重启 Start-WebUI；并检查垃圾邮件。

---

## English Guide (Step-by-step, no coding)

1) Download the latest release ZIP from `https://github.com/VVittgenstein/BetterCourseSchedulePlanner/releases`, then unzip it.
2) Start the app:
   - Windows: double-click `Start-WebUI.bat`
   - macOS: double-click `Start-WebUI.command` (right-click → Open if blocked)
3) First run may ask you to install Node.js. Install the **LTS** version (v22+), then rerun the starter script.
4) Open the web UI:
   - It usually opens automatically, or go to `http://localhost:5174`
5) Fetch course data (required):
   - In **Term data fetch**, choose Year + Semester + Campus code (`NB`/`NK`/`CM`), then click **Run fetch** and wait for **Done**.
6) Filter courses:
   - Use **Build your schedule** filters on the left; the course list (with section indexes) is on the right.
7) Subscribe (optional):
   - In **Subscription center**, paste a 5-digit section **Index**, choose **Email** or **Sound**, then click **Subscribe**.
   - Cancel in **Active subscriptions**.
8) Email delivery (optional, SendGrid):
   - Verify a sender + create an API key in SendGrid, fill **Mail settings**, then restart the starter script to apply changes.
