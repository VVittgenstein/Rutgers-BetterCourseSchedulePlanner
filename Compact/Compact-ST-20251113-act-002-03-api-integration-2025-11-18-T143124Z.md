## 已落实事实
- 课程列表页面仍通过 `frontend/src/App.tsx` 将 `FilterPanel`、`CourseList` 与 `useCourseQuery`/`useFiltersDictionary` 联动，但 App 现已在 fallback filters 生效时去除 FilterPanel loading 状态（`loading` 仅在无字典、请求未完成时为 true），避免离线字典下 reset 按钮被禁用。
- `useCourseQuery`、`useFiltersDictionary`、`CourseList` 逻辑与前次一致：实时映射 `CourseFilterState` 到 `/api/courses` 请求，缓存/去抖/取消、虚拟化渲染及错误/空态提示均已落地；`/filters` 失败时 fallback 字典与重试提示继续有效。
- 其余 API 客户端、过滤器状态映射、Vite 代理等基础设施保持与前一版相同。

## 接口/行为变更
- FilterPanel 的 loading badge / reset 禁用逻辑改为：仅当 filters 字典尚未加载完成时显示 loading。fallback 字典路径现在可即时操作，避免 offline 模式下的 UX 阻塞。
- 其他交互（虚拟化列表、分页、API 调用）无新增变更。

## 风险 / 限制 / TODO
- 后端仍未支持 `hasWaitlist`、`tag` 等筛选，前端仅以提示告知风险；fallback 字典仍是示例数据。
- 错误提示仍基于列表空态，无全局 Toast。

## 自测
- `cd frontend && npm run build`（TS + Vite build）通过。

## Code Review - ST-20251113-act-002-03-api-integration - 2025-11-18T14:50:43Z
Codex Review: Didn't find any major issues. Breezy!
