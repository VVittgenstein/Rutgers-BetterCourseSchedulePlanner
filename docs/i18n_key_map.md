# i18n key map

This note inventories every user-facing copy that currently exists in the Vite + React frontend, defines how keys will be structured, and tracks the default English (`en`) / Simplified Chinese (`zh`) text that now lives inside `frontend/i18n/messages.json`.

## Key naming rules

1. **Namespace format** – every key follows `area.section.slot` (e.g. `filters.header.title`). Use three segments when possible; reserve a fourth segment for variants (`filters.sections.basic.term.placeholder`).
2. **Areas** – `app` (shell alerts), `filters` (left panel), `courseList` (results header/footer), `courseCard` (per-row details), `schedule` (calendar preview), `tagChip`, `errors`, `document`, plus shared `common` helpers.
3. **Shared helpers** – anything reused in multiple places (actions, statuses, day names, time strings) lives under `common.*` so that the translator only touches one string when changes ripple through multiple components.
4. **Interpolation** – use `{{name}}` placeholders only for dynamic values that come from runtime data; quote the placeholder in this map so translators know what each token means (e.g. `{{count}} courses`).
5. **Docs parity** – whenever you add, rename, or remove a key, update both `frontend/i18n/messages.json` and this file in the same commit so reviewers can audit intent alongside implementation.

## Copy inventory

### App shell (`frontend/src/App.tsx`)

| Key | English default | zh-CN default | Usage / context |
| --- | --- | --- | --- |
| `app.shell.loadingDictionary` | Loading filter dictionary… | 正在加载筛选字典... | Placeholder shown while the dictionary fetch is pending. |
| `app.shell.fallbackAlert` | Filters API is unavailable, falling back to the offline dictionary. | Filters API 不可用，已切换到离线字典。 | Warning banner rendered when the remote dictionary API fails. |
| `app.shell.waitlistNotice` | Waitlist filtering is not wired to the API yet. Results are for reference only. | 等候名单筛选尚未接入 API，当前结果仅供参考。 | Informational banner when the user chooses “has waitlist.” |
| `app.shell.empty.ready` | No results matched your filters. Try adjusting them. | 暂无匹配结果，请调整筛选条件。 | Empty-state message once the user has picked enough filters to run a query. |
| `app.shell.empty.missingFilters` | Select a term and campus to load courses. | 请先选择学期与校区以加载课程列表。 | Empty-state message before the initial term/campus selection. |

### Filter panel (`frontend/src/components/FilterPanel.tsx`)

#### Header + global actions

| Key | English default | zh-CN default | Usage |
| --- | --- | --- | --- |
| `filters.header.eyebrow` | Filters | Filters | Section eyebrow atop the sidebar. |
| `filters.header.title` | Build your schedule | Build your schedule | Panel headline. |
| `filters.header.subtitle` | Select a term and refine results with multi-select chips. | 请选择学期并用多选标签细化结果。 | Supporting description under the title. |
| `filters.header.reset` | Reset filters | 清空筛选 | Button that re-initializes the filter state. |
| `filters.status.loadingBadge` | Loading… | 加载中… | Small badge rendered near “基础信息” while the dictionary is loading. |

#### Basic info section

| Key | English default | zh-CN default | Usage |
| --- | --- | --- | --- |
| `filters.sections.basic.title` | Basic info | 基础信息 | Section heading. |
| `filters.sections.basic.term.label` | Term | Term | `<label>` text next to the term `<select>`. |
| `filters.sections.basic.term.placeholder` | Choose a term | 请选择学期 | Disabled placeholder option. |
| `filters.sections.basic.campus.label` | Campus | Campus | Label for campus `<select>`. |
| `filters.sections.basic.campus.all` | All campuses | 全部校区 | First option in the campus dropdown. |
| `filters.sections.basic.keyword.label` | Search keyword | Search keyword | Label for free-text search. |
| `filters.sections.basic.keyword.placeholder` | Course title, code, or instructor | 课程名、编号或教师 | Input placeholder. |
| `filters.sections.basic.openStatus.label` | Show sections | Show sections | Label for the open-status pill toggle. |
| `filters.sections.basic.openStatus.all` | All | 全部 | Pill copy for “all sections.” |
| `filters.sections.basic.openStatus.openOnly` | Open seats | 有空位 | Pill copy for “only show open seats.” |
| `filters.sections.basic.openStatus.waitlist` | Has waitlist | 有候补 | Pill copy for the waitlist variant. |

#### Subject picker

| Key | English default | zh-CN default | Usage |
| --- | --- | --- | --- |
| `filters.sections.subjects.title` | Subject | 科目 Subject | Section heading (mixed language today). |
| `filters.sections.subjects.clear` | Clear | 清除 | Clear button for the selected subjects. |
| `filters.sections.subjects.search.label` | Search subjects | Search subjects | Label above the search box. |
| `filters.sections.subjects.search.placeholder` | e.g. Computer Science | 例如：Computer Science | Placeholder text. |
| `filters.sections.subjects.truncatedHint` | Showing {{current}} / {{total}} subjects. Continue typing to narrow the list. | 已截断 {{current}} / {{total}}，请继续搜索以缩小范围。 | Helper text after the checkbox list truncates to 12 items. |

#### Meeting-time picker

| Key | English default | zh-CN default | Usage |
| --- | --- | --- | --- |
| `filters.sections.meeting.title` | Meeting time | 上课时间 | Section heading. |
| `filters.sections.meeting.clear` | Clear | 清除 | Button that clears the entire meeting filter. |
| `common.days.short.mon` – `common.days.short.sun` | Mon / Tue / ... | 周一 / 周二 / ... | Shared weekday labels consumed by both the meeting-day chips and the schedule preview columns. |
| `filters.sections.meeting.start` | Start time | 开始时间 | Label above the first `<input type="time">`. |
| `filters.sections.meeting.end` | End time | 结束时间 | Label above the second `<input type="time">`. |

#### Delivery, level, quick tags

| Key | English default | zh-CN default | Usage |
| --- | --- | --- | --- |
| `filters.sections.meta.title` | Delivery & Level | 授课方式 & 年级 | Section heading covering both groups. |
| `filters.sections.meta.clear` | Clear | 清除 | Button clearing both delivery + level selections. |
| `filters.sections.meta.delivery` | Delivery | Delivery | Label above the delivery checkbox group. |
| `filters.sections.meta.level` | Level | Level | Label above the level checkbox group. |
| `filters.sections.tags.title` | Quick tags | 快速标签 | Section heading for the TagChip presets. |
| `filters.sections.tags.clear` | Clear | 清除 | Button clearing all tag presets. |

#### Active-filter chips + meeting fallback labels

| Key | English default | zh-CN default | Usage |
| --- | --- | --- | --- |
| `filters.chips.keyword` | Keyword | 关键词 | Chip label when the search query is active. |
| `filters.chips.subject` | Subject | 科目 | Chip label for each selected subject. |
| `filters.chips.level` | Level | 年级 | Chip label for the level filter. |
| `filters.chips.delivery` | Delivery | 授课方式 | Chip label for delivery methods. |
| `filters.chips.tag` | Tag | 标签 | Chip label for quick tags. |
| `filters.chips.core` | Core | 核心课程 | Chip label for core codes. |
| `filters.chips.meeting` | Meeting | 上课时间 | Chip label when any meeting constraint is active. |
| `filters.chips.meetingFallback` | Meeting filter | 上课时间筛选 | Text shown when the chip needs a fallback label. |
| `filters.chips.openOnly` | Open seats only | 只看有空位 | Chip label mirroring the open-only pill. |
| `filters.chips.hasWaitlist` | Has waitlist | 有候补 | Chip label mirroring the waitlist pill. |
| `filters.chips.timePlaceholder.start` | Start | 开始 | Placeholder text injected into meeting chips when no explicit value is set. |
| `filters.chips.timePlaceholder.end` | End | 结束 | Same for the end time. |

### Course list & cards (`frontend/src/components/CourseList.tsx`)

| Key | English default | zh-CN default | Usage |
| --- | --- | --- | --- |
| `courseList.header.eyebrow` | Results | 结果 | Section eyebrow above the results list. |
| `courseList.header.loading` | Loading courses… | 加载课程中… | Title while the query is running. |
| `courseList.header.count` | {{count}} courses | {{count}} 门课程 | Title once results are available. |
| `courseList.header.range` | Showing {{start}} - {{end}} · Page {{page}} | 显示 {{start}} - {{end}} · 第 {{page}} 页 | Subtext under the header when we know totals. |
| `courseList.header.refreshing` | Refreshing… | 刷新中… | Badge when `isFetching` is true. |
| `courseList.empty.default` | No courses match your filters. Try adjusting them. | 没有符合条件的课程，请尝试修改筛选条件。 | Default empty-state copy (also reused by `App` for ready state). |
| `courseList.footer.pagination` | {{pages}} pages · {{pageSize}} per page | 共 {{pages}} 页 · 每页 {{pageSize}} 条 | Pagination meta in the footer. |
| `courseList.footer.none` | No pagination available | 无可分页信息 | Footer fallback when totals are unknown. |
| `courseList.footer.pageLabel` | Page {{page}} / {{pages}} | 第 {{page}} / {{pages}} 页 | Middle text in the pagination bar. |
| `courseList.pagination.prev` | Previous | 上一页 | Left pagination button. |
| `courseList.pagination.next` | Next | 下一页 | Right pagination button. |
| `courseCard.badges.open` | Open | 有空位 | Badge shown when any section is open. |
| `courseCard.badges.levelFallback` | N/A | 暂无 | Badge placeholder when level is unknown. |
| `courseCard.meta.credits` | Credits | 学分 | First meta tile label. |
| `courseCard.meta.sections` | Sections | 班次 | Second meta tile label. |
| `courseCard.meta.subject` | Subject | 科目 | Third meta tile label. |
| `courseCard.tags.deliveryFallback` | Delivery TBD | 授课方式待定 | Text shown when no delivery labels exist. |
| `courseCard.details.prerequisites` | Prerequisites: | 先修课： | Prefix before the prerequisites field. |
| `courseCard.details.updated` | Updated {{time}} | 更新于 {{time}} | Footer text when a relative timestamp exists. |
| `courseCard.details.updatedRecent` | Updated recently | 最近更新 | Footer text when we cannot compute the relative timestamp. |
| `courseCard.details.term` | Term {{term}} | 学期 {{term}} | Footer text showing the term. |
| `courseList.error.traceId` | Trace ID: {{id}} | Trace ID：{{id}} | Helper text shown in the error state when `traceId` exists. |
| `courseList.error.retry` | Retry | 重试 | Retry button next to the error state. |

Relative time helpers (`frontend/src/components/CourseList.tsx`)

| Key | English default | zh-CN default | Usage |
| --- | --- | --- | --- |
| `common.time.relative.justNow` | just now | 刚刚 | When the result was updated within 1 minute. |
| `common.time.relative.minutes` | {{count}}m ago | {{count}} 分钟前 | 1 minute – 1 hour range. |
| `common.time.relative.hours` | {{count}}h ago | {{count}} 小时前 | 1 hour – 1 day range. |
| `common.time.relative.days` | {{count}}d ago | {{count}} 天前 | Beyond 1 day. |

### Schedule preview (`frontend/src/components/SchedulePreview.tsx`)

| Key | English default | zh-CN default | Usage |
| --- | --- | --- | --- |
| `schedule.header.eyebrow` | Preview · Weekly grid | 预览 · 每周视图 | Eyebrow text above the calendar. |
| `schedule.header.title` | Schedule snapshot | 课程概览 | H3 title. |
| `schedule.header.count` | {{count}} sections | {{count}} 个班次 | Count bubble on the right. |
| `common.days.short.mon` – `...sun` | Mon / Tue / ... | 周一 / 周二 / ... | Column headers and meeting-day toggles (shared with the filter panel). |
| `common.time.am` / `common.time.pm` | AM / PM | 上午 / 下午 | Used by the hourly scale and meeting chips. |

### TagChip & misc utilities

| Key | English default | zh-CN default | Usage |
| --- | --- | --- | --- |
| `tagChip.remove` | Remove {{label}} | 移除 {{label}} | `aria-label` for the “×” button in `TagChip`. |
| `document.title` | BCSP Filter Playground | BCSP 课程筛选实验场 | `<title>` tag inside `frontend/index.html`. |
| `errors.runtime.missingRoot` | Missing #root element | 缺少 #root 根节点 | Runtime guard inside `frontend/src/main.tsx`. |
| `errors.network.requestFailed` | Request failed: {{status}} | 请求失败：{{status}} | Default message produced by `frontend/src/api/client.ts` when HTTP status is not OK. |

### Common helpers

| Key | English default | zh-CN default | Usage |
| --- | --- | --- | --- |
| `common.actions.clear` | Clear | 清除 | Shared button label for smaller “Clear” affordances. |
| `common.actions.resetFilters` | Reset filters | 重置筛选 | Used by the sidebar reset button. |
| `common.actions.retry` | Retry | 重试 | Shared action for API retries. |
| `common.actions.prev` | Previous | 上一页 | Pagination button. |
| `common.actions.next` | Next | 下一页 | Pagination button. |
| `common.status.loading` | Loading… | 加载中… | Generic loading badge text. |
| `common.status.refreshing` | Refreshing… | 刷新中… | Used by the course list badge. |
| `common.messages.subjectsTruncated` | Showing {{current}} / {{total}} entries. Continue typing to narrow the list. | 已截断 {{current}} / {{total}}，请继续搜索以缩小范围。 | Helper text for truncated subject lists (mirrors the sentence above). |
| `common.time.range.start` | Start | 开始 | Placeholder text when meeting chips have no explicit start time. |
| `common.time.range.end` | End | 结束 | Placeholder text when meeting chips have no explicit end time. |

## Translation source (`frontend/i18n/messages.json`)

All keys above ship with English + Simplified Chinese scaffolding inside `frontend/i18n/messages.json`. The JSON uses two top-level locales (`en`, `zh`) that mirror the same nested object graph, so TypeScript can later assert parity when an i18n helper loads the bundle. Keep translations human-friendly (full sentences, consistent punctuation) because these values are rendered directly inside React components.

## Contribution workflow

1. **Adding a new key**
   - Decide where the copy belongs (try to re-use an existing `common.*` slot before minting a new one).
   - Update `frontend/i18n/messages.json` by adding the key under both `en` and `zh`. Use a short, reviewer-friendly placeholder (e.g. `"TODO"` or copy-pasted English) if the actual translation is pending.
   - Document the new key in `docs/i18n_key_map.md` so designers / PMs can audit wording outside the diff viewer.

2. **Checking for missing translations**
   - Run `jq '.[\"en\"] | keys' frontend/i18n/messages.json` and compare with the `zh` block to ensure the same structure (a lightweight script will be added later; for now this manual `jq` diff is the stopgap).
   - If one locale is missing a key, either provide the translation immediately or insert a `TODO` marker and open a follow-up issue before merging.

3. **Syncing translations when copy changes**
   - When updating phrasing, change both locales in the JSON file and adjust the description in this markdown map.
   - Re-run `pnpm -C frontend build` to ensure no type errors were introduced while wiring the new key into React components.
   - Notify translators (or leave a note in the PR) if additional locales must be updated; the mirrored JSON structure allows translation bots to diff and highlight the exact slots that changed.
