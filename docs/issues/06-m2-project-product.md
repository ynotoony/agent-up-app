<!-- Input: approved SPEC-004。 -->
<!-- Output: 多项目登记、失效、重绑定、删除备份的外部行为。 -->
<!-- Pos: M2 垂直切片票。 -->

# 06 — M2-01 项目产品切片

**Type:** feature

**Priority:** P1

**Complexity:** C3

**What to build:** 用户能登记两个项目、看到目录失效、重绑定后恢复同一项目，并能在确认后备份并删除 `.agentup/` 而不动项目源码。

**Blocked by:** M1-02 done；SPEC-004 approved。

**Status:** in_progress

- [ ] `list_projects` / `register_project` / `rebind_project` / `remove_agentup`
- [ ] 两项目隔离；missing path 不写项目盘
- [ ] remove 先备份再删 `.agentup/`，不动源码
- [ ] capability 只扩这四条 command；仍无 fs/shell

## Implementation Checkpoint

**2026-09-15 / in_progress**：W1 done。按 SPEC-004 实施。
