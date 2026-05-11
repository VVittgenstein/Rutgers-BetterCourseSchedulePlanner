Subtask `ST-20251113-act-002-01-ui-architecture` – Compact (UTC 2025-11-18T12:37:42Z)

## Confirmed Facts
- `docs/ui_flow_course_list.md` 记录课程浏览页的最终布局、状态流和交互草图，覆盖过滤列/结果区结构、URL 同步策略以及组件拆分顺序，满足验收中“信息架构 + 状态定义 + 组件清单”的要求。
- 新增 `frontend/src/state/courseFilters.ts`，集中定义 `CourseFilterState`、`MeetingFilter`、默认初始状态生成器与 `buildCourseQueryParams`/`serializeCourseFilters`/`parseCourseFiltersFromSearch` 等工具，直接贴合 `/api/courses` 契约。
- `parseMeetingDays` 处理 `M,T,W,TH,F,SA,SU` 与逗号/空格组合，为后续 URL -> 状态恢复提供一致性。

## Interface / Behavior Changes
- 任何计划接入过滤状态的 store 或 hook 需从 `frontend/src/state/courseFilters.ts` 导入统一的类型、默认值和 URL 序列化逻辑，以保证分享链接、分页和排序字段与 API 参数一致。
- `buildCourseQueryParams` 现在会在缺少 `term` 时抛错；调用方必须在触发查询前完成 term/campus 初始化。

## Risks / TODO
- 仅提供信息架构与 state contract，尚未实现实际的状态管理容器、URL 同步 hook 或 UI 组件；后续任务需按文档拆解执行。
- 未覆盖 `CalendarView`/`SectionDrawer` 等组件级别的具体接口，仍需要与 `/api/sections` 协作定义。

## Testing
- 未运行自动化测试（新增文档与 TypeScript 类型/工具，尚无引用点）。
