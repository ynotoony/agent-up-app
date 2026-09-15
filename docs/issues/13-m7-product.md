<!-- Input: approved SPEC-011。 -->
<!-- Output: S1–S3 E2E、诊断脱敏、打包门。 -->
<!-- Pos: M7 垂直切片。 -->

# 13 — M7-01 三场景与发布门

**Type:** feature

**Priority:** P1

**Complexity:** C3

**What to build:** 三条场景 command 路径可重复跑绿；诊断包脱敏；打包开启且无自动更新。

**Blocked by:** W6 done；SPEC-011 approved。

**Status:** review_ready

- [x] S1–S3 E2E
- [x] export_diagnostics 脱敏
- [x] bundle.active true 且无 updater
- [x] 隐私与手动更新说明

## Implementation Checkpoint

**2026-09-15 / review_ready**：S1–S3 E2E、export_diagnostics、bundle.active、无 updater。未 push、不自审。
