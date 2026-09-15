<!-- Input: approved SPEC-010。 -->
<!-- Output: publish/accept/reject/feedback/history 外部行为。 -->
<!-- Pos: M6 垂直切片。 -->

# 12 — M6-01 结果与历史切片

**Type:** feature

**Priority:** P1

**Complexity:** C2

**What to build:** 用户能发布不可变结果版本；接受/拒绝留下新版本和 decision，旧版本不动；反馈进讨论并可开 ready 任务；读历史不改当前目标或任务板。

**Blocked by:** W5.5 done；SPEC-010 approved。

**Status:** ready

- [ ] 两次 `publish_result`；`load_result_history` 两个版本；第一版 content 不变
- [ ] 接受后再拒绝后续版本；旧 accepted 仍 accepted；拒绝不删文件
- [ ] 接受后 `submit_feedback` 有讨论且 result content 不变
- [ ] drop Runtime 后历史仍从 `.agentup/` 事实恢复，不靠 SQLite
- [ ] `evidence_paths` 相对通过；绝对路径 `invalid_input`
