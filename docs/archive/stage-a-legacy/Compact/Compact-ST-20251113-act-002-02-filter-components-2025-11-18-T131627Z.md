Subtask `ST-20251113-act-002-02-filter-components` – Compact (UTC 2025-11-18T13:16:27Z)

## Confirmed Facts
- React/Vite playground continues to host FilterPanel, TagChip, and SchedulePreview components with mock dictionaries/sections (`frontend/src/dev`).
- Meeting时间过滤逻辑已按 `/api/courses` 契约更新：当设定 `meeting.startMinutes`/`meeting.endMinutes` 时，只有全部 meeting 满足“起始 >= startMinutes 且结束 <= endMinutes”的 section 才会保留，不再接受简单重叠（`frontend/src/dev/ComponentPlayground.tsx:28-38`）。
- SchedulePreview 在绘制块时会忽略完全落在可视窗口之外的 meeting，并在 clamp 后检测高度为零的情形，避免自定义 `startHour`/`endHour` 时出现负高度或越界（`frontend/src/components/SchedulePreview.tsx:77-95`, `174-193`）。

## Interface / Behavior Changes
- Playground 过滤结果与 API 行为保持一致，使演示页的 meeting 滤窗验证具备参考价值；任何依赖 `SchedulePreview` 的消费端现在可以安全传入更窄的时间窗口而不会渲染反向块。

## Risks / TODO
- 仍未接通真实 `/api/filters`/`/api/courses` 数据源，组件只在 mock 环境验证；冲突布局、URL 同步与 store 集成需在后续任务实现。
- SchedulePreview 仍未实现冲突提示/堆叠策略，真实数据可能导致覆盖。

## Testing
- `frontend: npm run build`

## Code Review - ST-20251113-act-002-02-filter-components - 2025-11-18T13:58:34Z
请根据以下两条code review进行修改：

#1
frontend/src/dev/ComponentPlayground.tsx
Comment on lines +29 to +33


        const start = filters.meeting.startMinutes ?? 0;
        const end = filters.meeting.endMinutes ?? 24 * 60;
        const overlaps = section.meetings.some(
          (meeting) => meeting.endMinutes > start && meeting.startMinutes < end,
        );


@chatgpt-codex-connector
chatgpt-codex-connector bot

P2 Badge Enforce meeting time bounds instead of simple overlap

When a start/end window is set, sections are kept if they merely overlap that window (meeting.endMinutes > start && meeting.startMinutes < end). A class from 7:00–12:00 still appears when the filter is 9:00–11:00 even though the contract (docs/query_api_contract.md, lines 60–62) expects meetings to start after meetingStart and finish before meetingEnd. This misaligns the playground preview with the API semantics and shows users sections outside their requested time range.

#2
frontend/src/components/SchedulePreview.tsx
Comment on lines +177 to +181


  const windowStart = startHour * 60;
  const clampStart = Math.max(meeting.startMinutes, windowStart);
  const clampEnd = Math.min(meeting.endMinutes, windowStart + totalMinutes);
  const top = ((clampStart - windowStart) / totalMinutes) * 100;
  const height = ((clampEnd - clampStart) / totalMinutes) * 100;


@chatgpt-codex-connector
chatgpt-codex-connector bot

P2 Badge Guard schedule blocks outside the displayed window

Positioning clamps meeting times to the viewport without checking order, so meetings entirely before startHour or after endHour produce a negative height and top > 100%. Such blocks render inverted or overlap the grid instead of being skipped or clamped to zero when a consumer customizes the window (e.g., startHour=10 with an 8–9am meeting).

## Code Review - ST-20251113-act-002-02-filter-components - 2025-11-18T13:59:12Z
Codex Review: Didn't find any major issues. Breezy!
