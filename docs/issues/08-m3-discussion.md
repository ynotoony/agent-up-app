<!-- Input: approved SPEC-006。 -->
<!-- Output: post_discussion / add_attachment / load_request_thread 外部行为。 -->
<!-- Pos: M3 垂直切片。 -->

# 08 — M3-01 讨论与附件切片

**Type:** feature

**Priority:** P1

**Complexity:** C3

**What to build:** 用户能对需求发文字并附多图；关进程后评论和附件文件仍在；超大附件被拒绝。

**Blocked by:** W2.5 done；SPEC-006 approved。

**Status:** review_ready

- [x] 三条 command 与 capability allowlist
- [x] 文字+两附件重启后可恢复；字节只在 `.agentup/attachments/`
- [x] 超限拒绝且零写入

## Implementation Checkpoint

**2026-09-15 / in_progress**

**2026-09-15 / review_ready**：Implementation 交付三条 command：`post_discussion`、`add_attachment`、`load_request_thread`。附件字节只写 `.agentup/attachments/<id>`，fact content 仅元数据（`relative_path=attachments/<id>`、`byte_length`、`sha256`、`media_type`）。超 10MiB 或合计 100MiB → `invalid_input` 且不落盘。capability 只扩这三条，无 fs/shell。关进程用 drop + 新 Runtime + `load_project` 后 `load_request_thread` 恢复。

- 范围：`src-tauri/src/{lib,runtime,discussion}.rs`、`src-tauri/permissions/m0.toml`、`src-tauri/capabilities/m0-default.json`、`schemas/command-*`、`src-tauri/tests/m3_discussion.rs`、`tests/m0_renderer_capabilities.py`。未接 Agent、不写项目源码、不 push、不自审。
- 验证（2026-09-15，cwd `/Users/bic/Projects/agent-up`）：
  - `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests`：ok；m3_discussion 2 passed；既有 m0/m1/m2/m2.5 套件全绿。
  - `python3 tests/m0_renderer_capabilities.py`：renderer capability static check ok。
  - `git diff --check`：clean。
