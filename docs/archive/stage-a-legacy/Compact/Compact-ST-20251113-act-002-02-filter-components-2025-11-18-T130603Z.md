Subtask `ST-20251113-act-002-02-filter-components` – Compact (UTC 2025-11-18T13:06:03Z)

## Confirmed Facts
- 建立独立的 React + Vite 前端工作区（`frontend/package.json`, `frontend/tsconfig*.json`, `frontend/vite.config.ts`, `frontend/index.html`），并在 `frontend/README.md` 说明 `npm run dev/build` 工作流，用于本地调试筛选/预览组件。
- `frontend/src/components/FilterPanel.tsx` 基于 `CourseFilterState` 与 `FiltersDictionary` 渲染 term/campus 下拉、搜索框、开放状态 pill、科目多选（带查询与 12 项截断提示）、授课方式/年级复选组、meeting day/time 控件及“清空”操作；所有活跃条件会生成 Removable TagChip，`清空筛选` 会保留 term/campus 其它字段重置。
- `frontend/src/components/TagChip.tsx` + `TagChip.css` 提供可点击/可移除的胶囊样式，支持 tone/compact/icon，供 FilterPanel 和其它位置重复使用。
- `frontend/src/components/SchedulePreview.tsx` + `SchedulePreview.css` 新增周视图布局：将 `ScheduleSection` 的 meetings 展平为定位块（含课程编号、时间段、地点），自动推导活跃天数及颜色图例，覆盖 8:00–22:00 时间窗并适配移动端。
- `frontend/src/dev/ComponentPlayground.tsx` / `mockData.ts` / `src/index.css` 构建 mock 驱动的演示页：使用静态字典与示例课程连接 FilterPanel 与 SchedulePreview，以验证多选/清空/meeting 过滤对日程预览的影响。

## Interface / Behavior Changes
- `FilterPanel` 公开 `FiltersDictionary`、`FilterOption` 等接口，并要求由外部提供 `CourseFilterState` + `onStateChange`/`onReset`，因此未来全局 store/URL 同步层需按照该 contract 提供字典和状态。
- `TagChip` 作为独立组件，可在 Filters header 或其它页面复用，可选 `onClick`/`onRemove` 与色调，统一了活跃条件的展示方式。
- `SchedulePreview` 输出 `ScheduleSection`/`ScheduleMeeting` 数据 contract，日历视图或其它容器可直接重用该组件，以一致的每周布局展示课程块。

## Risks / TODO
- 目前仅在 Vite playground 中使用 mock 数据；尚未接入真实 `/api/filters` 字典或全局 filter store，URL 同步和 API 请求逻辑仍待后续任务完成。
- `SchedulePreview` 尚未处理同一时间段的冲突排布、拖拽交互或可选时间窗等高级需求，后续引入真实数据时需要验证碰撞与可视化策略。
- 暂无自动化测试或 Storybook 快照；所有验证依赖本地 `npm run dev` 及 `npm run build`，未来应补充组件级测试或视觉回归。

## Testing
- `frontend: npm run build`
