<!-- Input: approved SPEC-009。 -->
<!-- Output: 取消、超时、重试、崩溃恢复与无子进程的外部行为。 -->
<!-- Pos: M5.5 运行可靠性垂直切片。 -->

# 11 — M5.5 运行可靠性切片

**Type:** feature

**Priority:** P1

**Complexity:** C3

**What to build:** 用户能取消正在跑的 fake run；15 min 必须停；provider 超时最多自动重试一次；越权写入不重试；崩溃后能 load 再 cancel；关 App 不等于后台继续跑；本切片不拉起 OS 子进程。

**Blocked by:** M5-01 done；SPEC-009 approved。

**Status:** ready

- [ ] `cancel_run` 后 `apply_fake_script` 失败；取消后文件不变
- [ ] `advance_run_clock` 超过 15 min → `run_state=interrupted`，restart 后 `load_run` 仍在
- [ ] 一次模拟 provider timeout 会重试；连续第二次 timeout 中断
- [ ] 越权写入仍不重试
- [ ] 无子进程被拉起
