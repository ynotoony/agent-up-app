<!-- Input: approved SPEC-009。 -->
<!-- Output: 取消、超时、重试、崩溃恢复与无子进程的外部行为。 -->
<!-- Pos: M5.5 运行可靠性垂直切片。 -->

# 11 — M5.5 运行可靠性切片

**Type:** feature

**Priority:** P1

**Complexity:** C3

**What to build:** 用户能取消正在跑的 fake run；15 min 必须停；provider 超时最多自动重试一次；越权写入不重试；崩溃后能 load 再 cancel；关 App 不等于后台继续跑；本切片不拉起 OS 子进程。

**Blocked by:** M5-01 done；SPEC-009 approved。

**Status:** review_ready

- [x] `cancel_run` 后 `apply_fake_script` 失败；取消后文件不变
- [x] `advance_run_clock` 超过 15 min → `run_state=interrupted`，restart 后 `load_run` 仍在
- [x] 一次模拟 provider timeout 会重试；连续第二次 timeout 中断
- [x] 越权写入仍不重试
- [x] 无子进程被拉起


## Implementation Checkpoint

**2026-09-15 / review_ready**：`advance_run_clock` 15 min → `interrupted`/`error_code=timeout`。`{tool:timeout}` 第一次重试仍 active，第二次中断。`cancel_run` 后 apply 失败。越权写不增加 timeout count。`agent_run.rs` 无 process spawn。未 push、不自审。
