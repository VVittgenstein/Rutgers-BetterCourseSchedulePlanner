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
