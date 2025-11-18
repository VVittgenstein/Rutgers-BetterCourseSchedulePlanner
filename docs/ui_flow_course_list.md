# Course List UI Architecture

规划基于本地 Query API 的课程浏览/筛选体验，聚焦 MVP 要求：用户在选择 term + campus 后即可快速组合筛选条件、分页浏览课程并随时切换日历/列表视图。

## 1. 页面结构与交互

```
+-------------------------------------------------------------------------------------------------------------+
| Global App Bar                                                                                              |
| - brand    - environment badge    - Sync indicator (API latency + DB version)                               |
+-------------------------------------------------------------------------------------------------------------+
| Filters Column (fixed width, sticky) | Results Workspace                                                     |
|                                       | - List header: term/campus pills, result count, sort dropdown        |
| Primary actions row:                  | - Tab group: `List` / `Calendar` / `Compact`                         |
|   [Term picker] [Campus picker]       |                                                                       |
| Secondary filters (accordion groups)  |  List View                                                            |
| - Search keyword                      |  ------------------------------------------------------------------  |
| - Course metadata (subject, level,    |  | CourseCard (collapsible)                                        | |
|   credits, core codes, prereq tags)   |  | - summary row (code, title, open chips, credits)                 | |
| - Section filters (meeting day/time,  |  | - expand: instructors, prereqs, tags, CTA to view sections       | |
|   instructor, delivery, campus/site)  |  |                                                                ↑ | |
| - State chips: each active filter is  |  |                                                                | |  Virtualized
|   mirrored as removable chips in the  |  |                                                                | |  scroller
|   Filters header + Results header     |  ------------------------------------------------------------------  |
| - Reset / Save view / Share link      |                                                                       |
|                                       |  Calendar View                                                        |
|                                       |  - Occupies entire workspace area                                    |
|                                       |  - Weekly grid overlay with color-coded sections                      |
|                                       |  - Hover reveals tooltip + CTA to open section drawer                 |
|                                       |                                                                       |
|                                       | Pinned drawer / side panel                                            |
|                                       | - Section detail + subscription button                               |
+-------------------------------------------------------------------------------------------------------------+
```

- `Filters Column` 在桌面 >1200px 时保持 320px 固定宽度，移动端则折叠为顶部抽屉。
- `Results Workspace` 高度自适应，滚动区域与过滤面板独立，以避免筛选时页面跳动。
- Tabs 与排序控件共享状态，当 API 返回分页数据时保持 `List` 与 `Calendar` 视图同步使用缓存数据。

## 2. 状态流 / 数据流

```
Browser URL  <--->  GlobalFilterStore  <--->  Query Builder  -->  /api/filters (bootstrap)
                                             (computes params)        |
                                                                      v
                                                             /api/courses (paginated)
                                                       (async cache keyed by params)
                                                                      |
                                                                      v
                                ┌──────────────┬─────────────────────┬───────────────┐
                                | ListView VM  | Calendar VM         | SectionPanel  |
                                | derives sort | derives meetings    | fetches /sections on demand
                                | + grouping   | + layout slots      | + memoized by sectionId
```

1. **Bootstrap**：页面首次加载调用 `/api/filters` 以填充 term/campus/subjects/字典，并将默认 term/campus（来自部署配置或 URL）写入 `GlobalFilterStore`。
2. **URL 同步**：`useUrlSync` hook 侦听 store 变化并更新查询串（见下一节）。刷新时解析 URL -> 初始化 store，实现分享链接。
3. **查询构建**：Store 变化时，通过 `buildCourseQuery(storeState)` 生成稳定 key，并传给 `usePaginatedCourses` hook。Hook 负责：
   - 取消上一请求（AbortController）
   - 将 filters 映射到 `/api/courses` 参数
   - 在 `hasNext` 与分页页码变化时合并/替换结果
4. **视图模型**：List/Calendar 视图观察统一的数据缓存，仅实现渲染层的排序/格式化逻辑，避免重复请求。
5. **错误与降级**：当 API 不可用时，`GlobalFilterStore.uiStatus` 标记为 `error`，Filters header 展示 toast + “重试”按钮。

## 3. 全局过滤状态 shape 与 URL 策略

### TypeScript shape

```ts
type MeetingFilter = {
  days: Array<'M' | 'T' | 'W' | 'TH' | 'F' | 'SA' | 'SU'>;
  startMinutes?: number;
  endMinutes?: number;
};

export type CourseFilterState = {
  term?: string;          // required before querying
  campus?: string;
  subjects: string[];     // "school:subject" normalized
  queryText: string;
  level: Array<'UG' | 'GR' | 'N/A'>;
  credits: { min?: number; max?: number };
  coreCodes: string[];
  keywords: string[];     // derived from chips such as "hasWaitlist"
  tags: string[];         // UI-only quick filters (e.g., "writing intensive")
  meeting: MeetingFilter;
  instructors: string[];
  delivery: Array<'in_person' | 'online' | 'hybrid'>;
  openStatus: 'all' | 'openOnly' | 'hasWaitlist';
  pagination: { page: number; pageSize: number };
  sort: { field: 'relevance' | 'courseNumber' | 'title' | 'updated'; dir: 'asc' | 'desc' };
  uiStatus: 'idle' | 'loading' | 'error';
  dirtyFields: Set<string>; // used to highlight unsaved preset changes
};
```

### URL 同步策略
- 使用 `?term=20241&campus=NB&subject=01:198&delivery=online&meetingDays=MWF&meetingStart=600`.
- 除 `queryText` 外，所有数组参数使用多值编码：`subject=01:198&subject=01:640`。移动端复制链接保持人类可读。
- `pagination.page` 总是同步；`pageSize` 仅在偏离默认 25 时写入。
- `sort` 组合编码为 `sort=courseNumber:asc`。
- `meeting`：`meetingDays=MWF`、`meetingStart=600`、`meetingEnd=900`。
- `tags`（UI 快捷开关）写入 `tag=writing_intensive` 等 slug。
- 通过 `URLSearchParams` 比较增量变化，只 pushState 当查询 key 发生变化（避免刷历史记录）。

> 目前 `/api/courses` 仅支持 meeting day/time 过滤，会议校区/地点关键字将在 API 增强后再开启。

## 4. 核心组件清单与依赖

| Component/Module | 责任 | 上游依赖 | 下游/输出 |
| --- | --- | --- | --- |
| `GlobalFilterStore` (zustand/recoil/自研) | 保存 `CourseFilterState` 并暴露 actions | Bootstrap config, URL params | Query hooks, FilterPanel, chips |
| `useUrlSync` hook | 双向同步 store <-> location.search | GlobalFilterStore | Router history |
| `FilterPanel` | 渲染 term/campus/secondary filters + chips | `GlobalFilterStore`, `FiltersDictionary` | dispatch actions + analytics events |
| `FiltersDictionaryProvider` | 缓存 `/api/filters` 结果 | Query API | Term/Campus pickers, SubjectSelect |
| `usePaginatedCourses` | 调用 `/api/courses` 并缓存 | `buildCourseQuery`, Fetch client | `CourseList`, `CalendarView` |
| `CourseList` | 虚拟化课程卡片，触发展开/详情 | `usePaginatedCourses` | Section drawer |
| `CalendarView` | 将结果映射为 meeting blocks | `usePaginatedCourses` | Collisions hints / tooltips |
| `SectionDrawer` | 延迟请求 `/api/sections`，展示 CTA | Course selection, `/api/sections` | Subscribe modal |
| `PresetManager` | 保存/加载本地 filter preset | GlobalFilterStore | FilterPanel (apply preset) |

### 后续实现拆分建议
1. **组件基础设施**：实现 `GlobalFilterStore` + `useUrlSync` + `FiltersDictionaryProvider`，保证 term/campus 选择和 URL 分享可用。
2. **筛选面板与 chips**：分支任务开发 SearchField、SubjectPicker、AdvancedFilters（meeting/instructor/delivery 等）。
3. **结果视图**：先行搭建 `CourseList`（含虚拟滚动 + loading skeleton），随后并行制作 `CalendarView` 布局。
4. **Section 细节与订阅 Drawer**：在 API `/api/sections` ready 后接入，复用 Drawer 容器。

## 5. 可观察性与降级
- 每次查询完成记录 `traceId` 并传递给错误 toast，方便 support 定位 API 日志。
- FiltersPanel 内展示“上次同步时间 + term/campus 数据版本”，若 `/api/filters` 失败则提供“重试”按钮并禁用 dependent controls。
- 在移动端启用 `prefers-reduced-motion` 检测，简化列表/日历切换动画，保证 0.5s 以内响应。

以上信息将指导 `ST-20251113-act-002-02-filter-components` 及后续前端实现。
