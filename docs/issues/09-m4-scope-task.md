<!-- Input: approved SPEC-007。 -->
<!-- Output: put_scope/diff_scope/put_task/set_task_state/load_board 外部行为。 -->
<!-- Pos: M4 垂直切片。 -->

# 09 — M4-01 范围与任务切片

**Type:** feature

**Priority:** P1

**Complexity:** C2

**What to build:** 用户能发布范围版本并看到与上一版的差异；能建主任务和子任务；不能直接写 task_state，只能经 set_task_state 并留下事件。

**Blocked by:** W3 done；SPEC-007 approved。

**Status:** review_ready

- [x] 五条 command 与 capability
- [x] diff_scope 覆盖增删改
- [x] put_task 拒绝 content.task_state；set_task_state 有事件
- [x] 子任务重启后仍在；CAS 冲突

## Implementation Checkpoint

**2026-09-15 / in_progress**

**2026-09-15 / review_ready**：Implementation 交付五条 command：`put_scope`、`diff_scope`、`put_task`、`set_task_state`、`load_board`。`put_task` 若 content 含 `task_state` → `invalid_input`；新建固定 `ready`。`set_task_state` 经 `write_fact` 发 `task.state_changed`。`diff_scope` 按 `relative_path` 给出 added/removed/changed。错误 envelope `{ok,command,error}`。capability 只扩这五条。未 push、不自审。

- 范围：`src-tauri/src/{lib,runtime,scope_task}.rs`、`src-tauri/permissions/m0.toml`、`src-tauri/capabilities/m0-default.json`、`schemas/command-*`、`src-tauri/tests/m4_scope_task.rs`、`tests/m0_renderer_capabilities.py`、本票。未改 `docs/progress.md`。
- 验证（2026-09-15，cwd `/private/tmp/codex-w4-scope-task`）：
  - `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests`：ok。lib/main 0 tests；m0_runtime 7；m1_facts 4；m1_projection 3；m2_5_external 5；m2_project 4；m3_discussion 2；m4_scope_task 4。共 29 passed。
  - `python3 tests/m0_renderer_capabilities.py`：renderer capability static check ok。
  - `git diff --check`：clean。
