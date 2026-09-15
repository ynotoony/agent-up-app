<!-- Input: approved SPEC-008。 -->
<!-- Output: fake Implement → 独立 fake Review → commit_changes 与范围写入外部行为。 -->
<!-- Pos: M5 垂直切片。 -->

# 10 — M5-01 fake Agent 切片

**Type:** feature

**Priority:** P1

**Complexity:** C3

**What to build:** 用户能跑 fake Implement，再跑独立 fake Review；Review fail 不能 commit；工具不能写批准范围之外；M0-C 字段记在 run 上。

**Blocked by:** W4 done；SPEC-008 approved。

**Status:** ready

- [ ] Fake implement 能写范围内文件；范围外 write 被拒绝，该文件不变
- [ ] Review `verdict=fail` → `commit_changes` 失败且文件保持原样；`pass` → `commit_changes` 成功
- [ ] 不可信 discussion 不能改 `prompt_version` 或绕过 scope
- [ ] Drop Runtime 后新 `load_run` 从 `.agentup/` 恢复 `run_state`
- [ ] 第三路并发 `start_run` 被拒绝
- [ ] 无真实 HTTP
