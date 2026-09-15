<!-- Input: approved SPEC-006。 -->
<!-- Output: post_discussion / add_attachment / load_request_thread 外部行为。 -->
<!-- Pos: M3 垂直切片。 -->

# 08 — M3-01 讨论与附件切片

**Type:** feature

**Priority:** P1

**Complexity:** C3

**What to build:** 用户能对需求发文字并附多图；关进程后评论和附件文件仍在；超大附件被拒绝。

**Blocked by:** W2.5 done；SPEC-006 approved。

**Status:** in_progress

- [ ] 三条 command 与 capability allowlist
- [ ] 文字+两附件重启后可恢复；字节只在 `.agentup/attachments/`
- [ ] 超限拒绝且零写入

## Implementation Checkpoint

**2026-09-15 / in_progress**
