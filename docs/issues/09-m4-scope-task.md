<!-- Input: approved SPEC-007。 -->
<!-- Output: put_scope/diff_scope/put_task/set_task_state/load_board 外部行为。 -->
<!-- Pos: M4 垂直切片。 -->

# 09 — M4-01 范围与任务切片

**Type:** feature

**Priority:** P1

**Complexity:** C2

**What to build:** 用户能发布范围版本并看到与上一版的差异；能建主任务和子任务；不能直接写 task_state，只能经 set_task_state 并留下事件。

**Blocked by:** W3 done；SPEC-007 approved。

**Status:** in_progress

- [ ] 五条 command 与 capability
- [ ] diff_scope 覆盖增删改
- [ ] put_task 拒绝 content.task_state；set_task_state 有事件
- [ ] 子任务重启后仍在；CAS 冲突

## Implementation Checkpoint

**2026-09-15 / in_progress**
