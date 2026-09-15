<!-- Input: SPEC-003、M1-01。 -->
<!-- Output: SQLite 仅投影、可从事实重建的外部行为。 -->
<!-- Pos: M1-02 任务票；SQLite 不得成为事实源。 -->

# 05 — M1-02 SQLite 投影重建

**Type:** feature

**Priority:** P1

**Complexity:** C3（数据重建、不可把投影当权威）

**What to build:** 删除 SQLite 之后仍能从 `.agentup/` 重建项目、需求、讨论、任务、范围、运行、决定和结果索引，且与删除前一致。

**Blocked by:** 04 — M1-01 事实类型合同落地。

**Status:** review_ready

- [x] 投影缺失不阻止 load。
- [x] 损坏投影不改写事实，返回可读错误或触发只读重建。
- [x] 重建后索引与事实 revision 对齐。
- [x] 测试删除 SQLite 再重建。

## Implementation Checkpoint

**2026-09-15 / in_progress**：M1-01 done。领取 C3 Implementation。SQLite 不得成为事实源。

**2026-09-15 / review_ready**：Implementation 交付 SQLite 投影重建。`rebuild_projection(project_id)` 扫描 `.agentup` 事实与持久事件，按 revision 物化 `.agentup/cache.sqlite`；不改写事实文件，不把 SQLite 当 CAS/事实源。缺失或损坏的 sqlite 不阻止 `load_project` / `read_fact`。畸形事实返回 `malformed_fact`，不写入“修好”的投影，也不改原文件。未新增 Tauri command / renderer capability，未接 Agent。`rusqlite 0.32`（`bundled`）从本地 cargo cache 解析，`--offline` 一次通过，未联网下载。
验证（2026-09-15，macOS aarch64，cwd `/Users/bic/Projects/agent-up`）：
- `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests` → lib 0；`tests/m0_runtime.rs` 7 passed；`tests/m1_facts.rs` 4 passed；`tests/m1_projection.rs` 3 passed（八类写入后重建、删 sqlite 再重建索引一致、畸形事实不被投影修好、损坏/缺失 sqlite 不阻断 load）。
- `git diff --check` → 无错误。
下一步：独立 Review（本 Implementation 不自审）。不 push。
