# Sessions / 会话日志

> Detailed log per experiment / work session.
> One file per attempt — preserve **all**, even failures.
> Failed sessions are valuable: they document what doesn't work.

## Naming convention / 命名约定

`YYYY-MM-DD-<short-kebab-name>.md`

Example: `2026-05-11-pm-init-setup.md`, `2026-05-12-stage-b-poller-refactor.md`

## Workflow / 工作流

1. **Start a new experiment:** Create `docs/sessions/YYYY-MM-DD-<name>.md` from the template below.
2. **During work:** Update the session file with progress, commands, key conversations.
3. **On success ✅:**
   - Mark session as ✅.
   - Update `CLAUDE.md` (Next Steps section) — never write history here.
   - Append a ✅ entry to `docs/TIMELINE.md`.
4. **On failure ❌:**
   - Mark session as ❌ and record what was tried + why it failed + what was learned.
   - **Do NOT touch `CLAUDE.md`** — current state dashboard stays at the last successful state.
   - Append a ❌ entry to `docs/TIMELINE.md` with what was tried and what was learned.

## Status legend

- ✅ Success / 成功
- ❌ Failed / 失败
- ⚠️ Partial / Pivoted / 部分成功或转向
- 🚧 In Progress / 进行中

## Session template / 模板

```markdown
# [session_topic 中文标题 / English title]

**状态 / Status**: 🚧 In Progress
**日期 / Date**: YYYY-MM-DD

## 目标 / Goal

[本次工作目标, 2-5 句话, 包含背景和动机.]

## 做了什么 / What was done

### [子主题 1]

**操作 / Actions**:
- [具体操作, 带文件路径/命令/参数]

**关键对话原文 / Key dialog**:
> [保留最关键的对话片段]

**产出 / Outputs**:
- `[文件路径]` ([行数] 行) — [内容简述]

## 决策记录 / Decision log

| 决策 | 上下文 | 结论 | 提出者 | 状态 |
|------|--------|------|--------|------|
| [Decision] | [Why needed] | [Conclusion] | [Who proposed] | 已确认/未验证/待验证 |

## 关键发现 / Key findings

1. **[Finding name]**: [详细描述, 包含具体数据/指标/原文引用]
   - 影响 / Impact: [对后续工作的影响]

## 失败与教训 / Failures & lessons

- **[Failure description]**: [Tried X] → [Failed because Y] → [Learned Z]

## 结果 / Result

- ✅ [Success parts with data/metrics]
- ❌ [Failed parts with reasons]
- ⚠️ [Surprises or caveats]

## 结论 / Conclusion

[3-5 句总结: 整体判断 + 决策 + 对项目方向的影响]

## 下一步 / Next steps

- [ ] [Concrete TODO]

## 相关产出 / Related outputs

- [File path list with brief descriptions]

## 相关 / Related

- **前序 / Predecessor**: [Previous session file link]
- **全程记录 / Timeline**: `docs/TIMELINE.md`
```
