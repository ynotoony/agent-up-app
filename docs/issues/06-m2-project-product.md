<!-- Input: approved SPEC-004。 -->
<!-- Output: 多项目登记、失效、重绑定、删除备份的外部行为。 -->
<!-- Pos: M2 垂直切片票。 -->

# 06 — M2-01 项目产品切片

**Type:** feature

**Priority:** P1

**Complexity:** C3

**What to build:** 用户能登记两个项目、看到目录失效、重绑定后恢复同一项目，并能在确认后备份并删除 `.agentup/` 而不动项目源码。

**Blocked by:** M1-02 done；SPEC-004 approved。

**Status:** review_ready

- [x] `list_projects` / `register_project` / `rebind_project` / `remove_agentup`
- [x] 两项目隔离；missing path 不写项目盘
- [x] remove 先备份再删 `.agentup/`，不动源码
- [x] capability 只扩这四条 command；仍无 fs/shell

## Implementation Checkpoint

**2026-09-15 / in_progress**：W1 done。按 SPEC-004 实施。

**2026-09-15 / review_ready**：Implementation 交付四条 command。App 索引写在应用数据目录 `app-index.sqlite`，不是项目事实源；`list_projects` 只更新索引里的 `path_state`，missing 时 scan/load 返回 `path_not_found` 且不重建项目盘。`register_project` 要求已 scan；`rebind_project` 仅在旧 path missing 时改索引并校验 manifest `project_id`。`remove_agentup` 空 token → `confirmation_required`；有效一次性 token 先 sibling 备份 `.agentup.backup.<utc>` 再删 `.agentup/`，源码与 `.git` 不动。capability 只把这四条加入 allowlist，仍无 fs/shell。测试用 `mint_remove_confirmation` 签发 remove token（非 Tauri command）。
验证（2026-09-15，macOS aarch64，cwd `/Users/bic/Projects/agent-up`）：
- `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests` → lib/main 0 tests；`m0_runtime` 7 passed；`m1_facts` 4 passed；`m1_projection` 3 passed；`m2_project` 4 passed（两项目隔离且 register 不写项目盘、missing 不写盘、rebind 同一 project_id、无 token 拒绝且 backup 含 manifest）。`--offline` 一次通过，未联网。
- `python3 tests/m0_renderer_capabilities.py` → renderer capability static check ok；scanned 3 renderer files。
- `git diff --check` → 无错误。
下一步：独立 Review（本 Implementation 不自审）。不 push。
