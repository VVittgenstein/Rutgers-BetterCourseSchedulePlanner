Subtask `ST-20251113-act-002-01-ui-architecture` – Compact (UTC 2025-11-18T12:48:36Z)

## Confirmed Facts
- `frontend/src/state/courseFilters.ts` 现仅暴露 `/api/courses` 支持的 meeting 过滤字段（meetingDays / meetingStart / meetingEnd），已移除 `meetingCampus`、`meetingLocation` 相关 state 与序列化逻辑，避免发送无效查询参数。
- `docs/ui_flow_course_list.md` 的状态定义与 URL 同步章节同步删去 meeting 校区 / 地点字段，并加注说明：待 API 增强后再重新开放该过滤。

## Interface / Behavior Changes
- 使用 `buildCourseQueryParams`、`serializeCourseFilters`、`parseCourseFiltersFromSearch` 的调用方不再获得 meeting campus/location 字段，因而无需处理对应 URL 参数；share links 也不会生成无效的 `meetingCampus`/`meetingLocation`。

## Risks / TODO
- meeting 校区 / 教室筛选仍在需求列表中，但当前 API 不支持；日后扩展需要同步更新 state shape 与文档（已在 docs 中提示）。

## Testing
- 未运行自动化测试（仅 TypeScript contract + 文档更新，暂无使用者依赖）。 

## Code Review - ST-20251113-act-002-01-ui-architecture - 2025-11-18T12:48:55Z

---review-start---

frontend/src/state/courseFilters.ts
Comment on lines +123 to +128


  if (state.meeting.campusCodes.length) {
    params.meetingCampus = [...state.meeting.campusCodes];
  }
  if (state.meeting.locationKeywords.length) {
    params.meetingLocation = [...state.meeting.locationKeywords];
  }


@chatgpt-codex-connector
chatgpt-codex-connector bot

P2 Badge Meeting location filters not recognized by courses API

The query builder emits meetingCampus and meetingLocation when meeting location filters are set, but the /api/courses route only understands meetingDays, meetingStart, and meetingEnd (see api/src/routes/courses.ts), so these keys are ignored. Users attempting to filter by meeting campus or room keywords will see no change in results. Align the parameters with supported course filters or drop these fields until the API supports them.

---review-end---
