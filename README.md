# 课程筛选与订阅工具 / Course Filter & Subscription Tool

本工具用于自动爬取课程数据，支持多条件筛选，并提供邮件提醒或本地声音提醒功能，帮助你更方便地抢课和订阅感兴趣的课程。

This tool crawls course data, supports flexible filtering, and provides email or local sound notifications to help you monitor and subscribe to desired courses more easily.

---

## 安装与启动 / Installation & Startup

1. **下载 Release**
   - 从发布页下载最新的 `release` 压缩包。
   - 将压缩包解压到任意一个文件夹（路径中尽量避免中文和空格）。

2. **一键运行**
   - 在解压后的文件夹中找到一键启动脚本（如 `start.bat`）并双击运行。
   - 如果你的电脑 **没有安装 Node.js**：
     - 脚本会自动打开 Node.js 官网。
     - 根据你的操作系统（Windows / macOS / Linux）下载并安装 Node.js。
   - 安装 Node.js 完成后：
     - 关闭刚才运行的 `.bat` 窗口。
     - 再次双击运行该 `.bat` 脚本。

3. **首次启动**
   - 启动成功后，会打开界面（或浏览器页面）。
   - 请按照下方「使用说明」开始爬取数据和筛选课程。

---

## 使用说明 / How to Use

1. **选择学科 / Choose Subjects**
   - 在界面左上角，选择你想要 **筛选/订阅** 的学科（Subject）。
   - 选择完成后，点击 **「开始」**（Start）。

2. **开始爬取数据 / Start Crawling**
   - 点击「开始」后，程序会自动开始爬取所选学科的课程数据。
   - 期间请保持程序运行，不要关闭窗口。
   - 当界面提示 **「完成」** 时，说明所有课程数据已经爬取完成。

3. **使用「Build your schedule」筛选课程 / Use “Build your schedule”**
   - 爬取完成后，进入 **“Build your schedule”** 功能页面。
   - 可以根据多种条件对课程进行筛选（支持多条件组合）：
     - 是否 open / close
     - 科目（Subject）
     - 校区与上课地点（Campus / Location）
     - 上课星期（Day of week）
     - 上课时间（Time）
     - 课程学分（Credits）
     - 核心代码（Core code）
     - 考试代码（Exam code）
     - 是否需要先修课程（Prerequisites）
     - 授课方式（Teaching mode）
     - 年级（Year / Level）

---

## 邮件提醒配置（SendGrid） / Email Notification (SendGrid)

若希望通过 **邮件提醒** 来接收课程变化或订阅结果，请按以下步骤配置 SendGrid。

1. **注册 SendGrid**
   - 前往 SendGrid 官网注册一个账户。
   - 完成邮箱验证。
   - 在 SendGrid 后台创建并获取一个 **API Key**（请妥善保存）。

2. **配置邮件设置 / Configure Email Settings**
   - 打开本工具界面中的 **「邮件设置」**（Email Settings）区域。
   - 填写以下信息：
     - **From 邮箱 (From Email)**：  
       必须是已经在 SendGrid 中完成验证的邮箱地址。
     - **API Key**：  
       填入你在 SendGrid 后台拿到的 `api-key`。
     - 其他选项可暂时忽略（如无特别需求）。
   - 点击 **「保存邮件设置」**（Save Email Settings）。
   - 保存成功后，**重启 `.bat` 启动脚本** 让设置生效。

3. **订阅入口设置 / Subscription Settings**
   - 在订阅入口中，将你想要订阅的课程的 **index（索引）** 填入对应位置。
   - 建议在接收通知的邮箱中，**至少留一个非 Gmail 的邮箱**，因为：
     - Gmail 有可能会把正常通知邮件误判为垃圾邮件。
   - 确认联系方式设置为 **邮箱（Email）**，即可通过邮件收到提醒。

---

## 本地声音提醒 / Local Sound Notification

如果你不想使用邮件提醒，可以使用 **本地声音提醒**：

1. 打开订阅设置页面。
2. 在「联系方式」或「通知方式」一栏，将方式切换为 **声音（Sound）**。
3. 之后当有订阅课程变化时，程序会通过本地声音进行提醒，而不是发送邮件。

---

## 使用建议 / Tips

- 若长时间未收到邮件，请检查：
  - SendGrid API Key 是否正确；
  - 「From 邮箱」是否为已在 SendGrid 验证过的邮箱；
  - 垃圾邮件/广告邮件目录（特别是 Gmail）。
- 爬取数据期间不要频繁关闭程序，避免数据不完整。
- 若修改了配置（例如邮件设置），请记得 **重启 `.bat` 脚本** 使配置生效。

---

## English Version

### 1. Installation & Startup

1. **Download Release**
   - Download the latest `release` archive from the release page.
   - Extract it to any folder (preferably without Chinese characters or spaces in the path).

2. **One-click Start**
   - In the extracted folder, locate the one-click script (e.g., `start.bat`) and double-click it.
   - If **Node.js is not installed** on your machine:
     - The script will automatically open the Node.js official website.
     - Download and install Node.js according to your OS (Windows / macOS / Linux).
   - After Node.js is installed:
     - Close the `.bat` window you just ran.
     - Double-click the `.bat` script again to restart.

3. **First Launch**
   - After starting successfully, the app UI (or browser page) will open.
   - Follow the steps below (“How to Use”) to fetch data and filter courses.

---

### 2. How to Use

1. **Choose Subjects**
   - In the top-left corner, select the subject(s) you want to **filter/subscribe** to.
   - Click **“Start”** after selection.

2. **Start Crawling Data**
   - After clicking “Start”, the tool will crawl course data for the selected subjects automatically.
   - Keep the program running; do not close the window.
   - When you see a **“Completed”** message, all course data has been fetched.

3. **Use “Build your schedule”**
   - Once crawling is done, go to the **“Build your schedule”** page.
   - You can filter courses by multiple conditions:
     - Open / Closed status
     - Subject
     - Campus / Location
     - Day of week
     - Time
     - Credits
     - Core code
     - Exam code
     - Prerequisites requirement
     - Teaching mode
     - Year / Level

---

### 3. Email Notification with SendGrid

If you want to receive **email notifications** for course changes or subscriptions, configure SendGrid as follows.

1. **Sign Up for SendGrid**
   - Register an account on SendGrid.
   - Verify your email.
   - Create and obtain an **API Key** from the SendGrid dashboard.

2. **Configure Email Settings**
   - In this tool’s UI, open the **Email Settings** section.
   - Fill in:
     - **From Email**:  
       Must be an email address that has been verified in SendGrid.
     - **API Key**:  
       Use the `api-key` you generated in SendGrid.
     - Other options can be left as default if not needed.
   - Click **“Save Email Settings”**.
   - After saving, **restart the `.bat` script** so the configuration takes effect.

3. **Subscription Settings**
   - In the subscription area, enter the **index** of the courses you want to subscribe to.
   - It is recommended to use **at least one non-Gmail email address** for receiving notifications because:
     - Gmail may occasionally classify normal notifications as spam.
   - Make sure your contact method is set to **Email**, then you will receive email notifications.

---

### 4. Local Sound Notification

If you prefer not to use email notifications, you can enable **local sound alerts** instead:

1. Open the subscription settings.
2. In the “Contact method” or “Notification method” field, switch to **Sound**.
3. The program will then use local audio alerts instead of sending emails when subscribed courses change.

---

### 5. Tips

- If you don’t receive email notifications:
  - Check whether the SendGrid API Key is correct.
  - Ensure the “From Email” is a verified sender in SendGrid.
  - Check your spam/junk folders (especially for Gmail).
- Avoid closing the program while data is being crawled to ensure data integrity.
- After changing configurations (e.g., email settings), always **restart the `.bat` script**.
