<!-- Input: approved SPEC-006。 -->
<!-- Output: post_discussion / add_attachment / load_request_thread 外部行为。 -->
<!-- Pos: M3 垂直切片。 -->

# 08 — M3-01 讨论与附件切片

**Type:** feature

**Priority:** P1

**Complexity:** C3

**What to build:** 用户能对需求发文字并附多图；关进程后评论和附件文件仍在；超大附件被拒绝。

**Blocked by:** W2.5 done；SPEC-006 approved。

**Status:** done

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


## Independent Review Checkpoint

- **2026-09-15 / review_pass**
- **结论**：pass。对照 approved SPEC-006 与本票三条核对，实现提供 `post_discussion` / `add_attachment` / `load_request_thread`；文字+两附件经 drop + 新 Runtime + `load_project` 后 `load_request_thread` 可恢复；附件字节写入 `.agentup/attachments/<id>`，fact content 只有元数据；单文件超 10MiB 返回 `invalid_input` 且目录 listing 不变；renderer capability 未授予 fs/shell。无足以单独否决本票的 P0 机器合同互斥。
- **审查范围**：`docs/issues/08-m3-discussion.md`（本 Review 只改 Status 并追加本段）、`docs/specs/006-m3-discussion.md`（approved）、commit `c0ee903`（`src-tauri/src/{lib,runtime,discussion}.rs`、permissions/capabilities、schemas、`src-tauri/tests/m3_discussion.rs`、`tests/m0_renderer_capabilities.py`、票/进度/索引）。对照未改的 `src/App.tsx`、`src-tauri/src/typed_facts.rs`。未改 src-tauri 实现、未代修、未 commit、未改 progress。
- **独立性**：新 Review 执行体；本会话首条指令即为独立审查。未参与 M3 Implementation（`c0ee903`），未共享该实现上下文。身份：Review 执行体；模型：grok-4.6；时间：2026-09-15 15:16 CST。依据 `R-RR-002`，此路径可独立。
- **Diff / 证据快照**：`HEAD=c0ee903209c78e8b3480cb94b285da3461d48963`（分支 `codex/w3-discussion`）。审查前工作区仅未跟踪 `src-tauri/gen/`；本 Review 只追加本 Checkpoint 并将 Status 改为 `review_pass`。

### P0

无未关闭、足以单独否决本票的 P0 机器合同互斥。

### 逐项验收（对照票核对清单与 approved SPEC-006）

| 必须核对 | 判断 | 证据 |
| --- | --- | --- |
| 1. 三条 command 与 capability allowlist | 成立 | 规格：`docs/specs/006-m3-discussion.md:12-20`。Tauri handler：`src-tauri/src/lib.rs:156-193,205-218`。Runtime 入口：`src-tauri/src/discussion.rs:2-73`（`post_discussion`）、`75-165`（`add_attachment`）、`167-244`（`load_request_thread`）。allowlist：`src-tauri/permissions/m0.toml:4-16`、`src-tauri/capabilities/m0-default.json:1-9`、`src-tauri/src/runtime.rs:11-26`。静态检查要求这三条：`tests/m0_renderer_capabilities.py:40-62`。 |
| 2. 文字+两附件，新 Runtime `load_request_thread` 可恢复 | 成立 | 规格验收 1：`docs/specs/006-m3-discussion.md:26`。测试：两附件 + `post_discussion`，`drop` 后 `AppRuntime::new` + `load_project` + `load_request_thread`，discussion body 与两个 id/sha256/relative_path/on-disk 字节一致：`src-tauri/tests/m3_discussion.rs:73-163`。实现：讨论走 `write_fact` 并产出 `discussion.posted`：`discussion.rs:49-72`、`typed_facts.rs:385`；加载按时间排序且 attachments 只映射元数据：`discussion.rs:185-243`。 |
| 3. 字节只在 `.agentup/attachments/`；fact 无 bytes | 成立 | 规格：`docs/specs/006-m3-discussion.md:18,22`。写入路径 `agentup/attachments/<id>`：`discussion.rs:136-140,310-348`，`agentup_dir`=`root/.agentup`：`src-tauri/src/runtime.rs:852-853`。fact content 仅 `relative_path`/`media_type`/`byte_length`/`sha256`：`discussion.rs:123-129`；`parse_attachment` 拒绝 `bytes`/`data`/`content` 且 `deny_unknown_fields`：`typed_facts.rs:842-877`。测试：响应与 thread 项无 `bytes`，磁盘文件在 `.agentup/attachments/<id>`：`m3_discussion.rs:104-107,119-122,147-165`。 |
| 4. 超限拒绝且零写入 | 成立（单文件 10MiB；合计 100MiB 有实现、无 M3 用例） | 规格：`docs/specs/006-m3-discussion.md:18,27`。单文件 `len > 10MiB` 在任何 mkdir/write 之前 `invalid_input`：`discussion.rs:107-113`。测试 `10MiB+1`：`invalid_input`、listing 不变、`.agentup/attachments` 不存在：`m3_discussion.rs:169-197`。合计上限在写字节前调用：`discussion.rs:131-135`、`typed_facts.rs:5-6,467-513`。 |
| 5. 无 fs/shell；无 Agent、不写项目源码 | 成立 | 规格：`docs/specs/006-m3-discussion.md:14,28`。capability 仅 event listen/unlisten + `allow-m0-commands`：`m0-default.json:5-8`。renderer 禁止 fs/shell/http 等：`tests/m0_renderer_capabilities.py:9-22,34-38`；`src/App.tsx:1-3` 只用 `invoke`/`listen`。`c0ee903` 不改 `src/` 生产源码；测试只在 TempDir 写 fixture：`m3_discussion.rs:32-38`。COMMANDS/handler 无 fs/shell。 |

### 非 P0 残留（不挡本门）

- 合计 100MiB 的零写入没有独立 M3 测试；门禁靠 `reject_attachment_limits` 代码路径，不是本票 `m3_discussion.rs` 的红绿证据。`reject_attachment_limits` 累加目录内全部 attachment JSON，不是规格字面「最新 revision」。
- `parse_attachment` 只要求 `relative_path` 以 `attachments/` 开头并拒绝绝对路径/`..`（`typed_facts.rs:561-568,850-868`），没有把路径钉死为 `attachments/<fact_id>`。`add_attachment` 命令路径是钉死的（`discussion.rs:119-121`）。
- `load_request_thread` 不重读磁盘附件、不重算 sha256；与磁盘一致是 persist 后的存储值，不是加载时完整性校验。
- `write_attachment_bytes` 先 `exists()` 再 `rename`（`discussion.rs:310-348`），不是 `create_new` 到最终路径；M3 用随机 `att-<hex>`，碰撞不是本门 P0。
- renderer `src/App.tsx` 未接这三条 command。本票验收是 command/capability/落盘，不是 UI。

### 验证命令与环境

- 仓库：`/Users/bic/Projects/agent-up`；日期 2026-09-15；macOS arm64；cwd 同上；rustc 1.94.0；cargo 1.94.0；Python 3.14.6。
- `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests` → pass：lib/main 0 tests；`m0_runtime.rs` 7 passed；`m1_facts.rs` 4 passed；`m1_projection.rs` 3 passed；`m2_5_external.rs` 5 passed；`m2_project.rs` 4 passed；`m3_discussion.rs` 2 passed。`--offline` 一次通过，未联网。
- `python3 tests/m0_renderer_capabilities.py` → pass：`renderer capability static check ok`；scanned 3 renderer files。
- `git diff --check` → 审查写入前无错误；审查只改本票。

### 下一步

Coordinator：本门 Independent Review pass。本 Review 不提交、不改进度。Commit 阶段另启。
