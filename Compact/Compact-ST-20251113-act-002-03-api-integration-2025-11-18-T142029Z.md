## 已落实事实
- 课程列表页面改为通过新入口 `frontend/src/App.tsx` 将 `FilterPanel`、`CourseList` 与 `useCourseQuery`/`useFiltersDictionary` 联动：加载 filters 字典后自动填充默认 term/campus，实时触发分页与虚拟化渲染（`frontend/src/components/CourseList.tsx` + `CourseList.css`）。
- `useCourseQuery`（`frontend/src/hooks/useCourseQuery.ts`）负责将 `CourseFilterState` 映射为 `/api/courses` 查询，内建缓存/去抖、AbortSignal 取消与 `refetch`，并将后端 payload 规整为 `CourseResultItem` + `meta`。
- `useFiltersDictionary`（`frontend/src/hooks/useFiltersDictionary.ts`）通过新 API 客户端（`frontend/src/api/client.ts` + `filters.ts` + `types.ts`）请求 `/api/filters`，失败时落到本地 `fallbackDictionary` 并暴露重试提示；`frontend/src/App.tsx` 现已在 fallback 成功后解除 FilterPanel loading/禁用状态。
- `CourseFilterState` 到 API 参数映射更新（`frontend/src/state/courseFilters.ts`），与后端契约一致（term/campus/subject/meeting... + include=sectionsSummary,subjects）。FilterPanel 变更确保筛选动作重置分页并支持 loading/dirty 状态。
- Vite dev server 追加 `/api` 代理配置（`frontend/vite.config.ts`），前端依赖新增 `react-window` 及类型（`frontend/package.json`, `package-lock.json`）。

## 接口/行为变更
- 前端默认向 `/api/courses` 发送请求，`CourseList` 渲染虚拟化列表并提供分页/重试按钮；空态信息依据 term/campus 是否齐全而变化。
- `/filters` 字典不可用时会切换至 fallback 并在 UI 中弹出警示，允许手动 “重试”。
- `FilterPanel` 触发查询时自动重置页码，并在等候名单筛选被选中时给出 `course-app__alert` 提示该条件尚未接入 API。

## 风险 / 限制 / TODO
- 后端暂未支持 `hasWaitlist`/`tag`/`keyword` 等部分筛选参数，前端仅提示风险未真正过滤；需要后续接口补齐或在 UI 禁用。
- fallback filters 仅包含示例数据，真实部署需确保 `/api/filters` 可用否则选项将失真。
- 未实现错误 Toast，全局错误以列表空态展示；若需悬浮通知需补充组件。

## 自测
- `cd frontend && npm run build`（TS 编译 + 产线打包）通过。

## Code Review - ST-20251113-act-002-03-api-integration - 2025-11-18T14:51:19Z

frontend/src/App.tsx
Comment on lines +50 to +54


              state={filters}
              dictionary={dictionaryState.dictionary}
              onStateChange={setFilters}
              onReset={setFilters}
              loading={dictionaryState.status !== 'success'}


@chatgpt-codex-connector
chatgpt-codex-connector bot

P2 Badge Fallback filters stuck in loading state after API error

When the filters API fails, useFiltersDictionary still populates the fallback dictionary but sets status to 'error', yet App treats any non-success status as loading when rendering FilterPanel. This leaves the panel showing a loading badge and disables the reset button even though filter data is already available from the fallback, preventing users from clearing filters while on the offline dictionary path. Consider basing loading on the presence of dictionary or the usingFallback flag so the panel becomes usable once fallback data is loaded.

## Code Review - ST-20251113-act-002-03-api-integration - 2025-11-18T14:51:47Z

Codex Review: Didn't find any major issues. Breezy!
