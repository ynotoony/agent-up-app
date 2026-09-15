<!-- Input: approved SPEC-005。 -->
<!-- Output: 五类外部变化的外部行为测试与必要 runtime 加固。 -->
<!-- Pos: W2.5 门禁票。 -->

# 07 — M2.5 外部变化门

**Type:** feature

**Priority:** P1

**Complexity:** C3

**What to build:** 外部改文件、未提交工作区文件、符号链接穿越、并发 revision、目录失效都不能静默丢掉用户改动。

**Blocked by:** M2-01 done；SPEC-005 approved。

**Status:** review_ready

- [ ] 五类变化均有失败/拒绝测试，且不写意外文件
- [ ] 不削弱已有 M0/M1/M2 测试

## Implementation Checkpoint

**2026-09-15 / review_ready**：五类外部变化测试在 `src-tauri/tests/m2_5_external.rs`，cargo test 5 passed。未削弱既有套件。
