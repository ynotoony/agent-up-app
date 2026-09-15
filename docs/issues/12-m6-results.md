<!-- Input: approved SPEC-010。 -->
<!-- Output: publish/accept/reject/feedback/history 外部行为。 -->
<!-- Pos: M6 垂直切片。 -->

# 12 — M6-01 结果与历史切片

**Type:** feature

**Priority:** P1

**Complexity:** C2

**What to build:** 用户能发布不可变结果版本；接受/拒绝留下新版本和 decision，旧版本不动；反馈进讨论并可开 ready 任务；读历史不改当前目标或任务板。

**Blocked by:** W5.5 done；SPEC-010 approved。

**Status:** review_pass

- [x] 两次 `publish_result`；`load_result_history` 两个版本；第一版 content 不变
- [x] 接受后再拒绝后续版本；旧 accepted 仍 accepted；拒绝不删文件
- [x] 接受后 `submit_feedback` 有讨论且 result content 不变
- [x] drop Runtime 后历史仍从 `.agentup/` 事实恢复，不靠 SQLite
- [x] `evidence_paths` 相对通过；绝对路径 `invalid_input`

## Implementation Checkpoint

**2026-09-15 / review_ready**：五条 command。未 push、不自审。

## Independent Review Checkpoint

- **2026-09-15 / review_pass**
- **结论**：pass。对照 approved SPEC-010 验收 1–5 与本票核对：两次 `publish_result` 后 `load_result_history` 两个版本且第一版 summary 仍为原值；接受后再拒绝后续版本，旧 accepted 仍 `accepted`、源 pending 不动；接受后 `submit_feedback` 写入讨论且不改 result content；drop Runtime 后历史从 `.agentup/facts/results` 恢复，不读 SQLite；绝对 `evidence_paths` 为 `invalid_input`，相对路径由同一校验器放行。无足以单独否决本票的 P0。Coordinator 可派发 Commit。不 git commit / 不 push。
- **审查范围**：`docs/issues/12-m6-results.md`（本 Review 只改 Status 并追加本段）、`docs/specs/010-m6-results.md`（approved）、commit `1f4b363`（`src-tauri/src/{results,runtime,lib}.rs`、permissions/capabilities、schemas、`src-tauri/tests/m6_results.rs`、`tests/m0_renderer_capabilities.py`）。对照未改的 `src-tauri/src/typed_facts.rs` / `discussion.rs` 只读核验写入与读路径。未改 src-tauri 实现、未代修、未 commit、未改 `docs/progress.md`、未写 `gen/`。
- **独立性**：新 Review 执行体；本会话首条指令即为独立审查。未参与 M6 Implementation（`1f4b363`），未共享该实现上下文。身份：Review 执行体；模型：grok-4.6；时间：2026-09-15 CST。工作树 `/private/tmp/codex-w6-results` 分支 `codex/w6-results`。依据独立 Review 门禁，此路径可独立。不以 Implementation 自述为证据。用户禁止改 impl / 禁止 progress / 禁止 gen。
- **Diff / 证据快照**：`HEAD=1f4b363e8e4ed9a4ba5aa454899684bfea069b03`（`feat(m6): immutable result versions and feedback`）。审查前工作区仅未跟踪 `src-tauri/gen/`；本 Review 只追加本 Checkpoint 并将 Status 改为 `review_pass`。

### 验证（本 Review 复跑，2026-09-15，cwd `/private/tmp/codex-w6-results`）

| 检查 | 结果 | 环境 |
| --- | --- | --- |
| `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests` | pass：45 passed / 0 failed。lib.rs 0；main.rs 0；m0_runtime 7；m1_facts 4；m1_projection 3；m2_5_external 5；m2_project 4；m3_discussion 2；m4_scope_task 4；m5_5_reliability 5；m5_fake_agent 6；m6_results 5 | rustc/cargo；`--offline`；`CARGO_TARGET_DIR=/tmp/codex-w6-results-target` |
| `python3 tests/m0_renderer_capabilities.py` | pass：`renderer capability static check ok`；scanned 3 renderer files | python3 |
| `git diff --check` | pass：clean | git；相对工作区 |

### P0

无未关闭、足以单独否决本票的 P0。五条 SPEC-010 验收均有对口测试；相对路径通过锁在 `valid_relative_path`，测试只锁绝对拒绝。`load_result_history` 读 `.agentup/facts/results`，不读 SQLite。`reject_result` 只 `write_fact`，无删文件。

### 逐项验收（对照 SPEC-010 验收 1–5）

| 必须核对 | 判断 | 测试证据 | 实现证据 |
| --- | --- | --- | --- |
| 1. 两次 `publish_result` 后 `load_result_history` 含两个版本；第一版 content 不变 | 成立 | `src-tauri/tests/m6_results.rs:38-55`（`two_publish_versions_keep_first_content`：`res-1` version=1 summary=first；`res-2` version=2 summary=second；history len=2，`[0].summary` 仍 first） | 调用方不填 version；`next_result_version` = max+1：`results.rs:27-35,117-121`。新 id `write_fact(..., 0)`：`results.rs:40`。历史按 version 升序：`results.rs:89-100` |
| 2. 接受某一版本后再拒绝后续版本；旧 accepted 仍 `accepted` | 成立 | `src-tauri/tests/m6_results.rs:57-72`（accept `res-1`→`res-acc`，再 reject `res-acc`→`res-rej`；`res-acc.acceptance=accepted`，`res-1.acceptance=pending`） | `settle_result` 复制 source summary/paths、新 version、新 id：`results.rs:43-60,123-163`。无原地改 source。decision `status=chosen`：`results.rs:167-183`。无 `fs::remove` |
| 3. 接受后 `submit_feedback` 产生讨论，且不改任何 result content | 成立 | `src-tauri/tests/m6_results.rs:74-92`（accept 后 feedback body=`need contrast`；thread 含该 body；`res-acc` summary/acceptance 仍 first/accepted） | `post_discussion` 后可选 `put_task`：`results.rs:63-86`。不调用 result 写入。新 task `task_state=ready`（`put_task` 新建）：`scope_task.rs:285-299` |
| 4. drop Runtime 后历史仍在；来自 `.agentup/` 事实，不是 SQLite | 成立 | `src-tauri/tests/m6_results.rs:94-107`（`history_survives_restart`：drop 后 `load_project` + `load_result_history` 仍 `summary=first`） | `load_result_history` → `load_request_facts(agentup, "result", …)`：`results.rs:89-100`。读 `facts/<dir>/*.rN.json`：`discussion.rs:280-321`。函数体无 sqlite |
| 5. `evidence_paths` 相对路径可通过；绝对路径拒绝 | 成立 | `src-tauri/tests/m6_results.rs:109-116`（`/tmp/x` → `publish_result` `invalid_input`）。相对通过无独立测，见残留 | 发布前校验：`results.rs:17-21`。`valid_relative_path` 拒绝 `/`、`\`、`..`：`typed_facts.rs:561-568`。空路径不写入字段：`results.rs:17,37-39` |

测试名与断言对齐：五测锁的就是对应合同。验收 5 的相对通过未单独断言，不构成 P0 否决。

### 非 P0 残留（不挡本门）

- 重复 `result_id` 规格要 `already_exists`；`publish_result` 走 `write_fact(..., expected_revision=0)`，已存在时先返回 `revision_conflict`（`typed_facts.rs:103-109`），不可达后面的 result 不可变分支（`typed_facts.rs:111-117`）。旧 content 仍不改。
- 验收 5 测试只覆盖绝对路径；`..` / `\` 与相对成功路径无测。
- 验收 2 测的是 reject 已 accepted 的 source，不是「再 publish 一版再 reject」；不删 evidence 文件无直接断言（实现无 delete）。
- 验收 3 带 `title` 应建 ready task，测试未断言 task / `task_state`。
- 验收 4 未删 sqlite 再读；实现读文件，测试只 drop Runtime。
- `settle_result` 若 decision 写入失败，result 新版本已落盘。`submit_feedback` 若 `put_task` 失败，discussion 已写入。
- permissions 描述仍写「M0, M2, M3 and M4」。

### 后续

移交 Commit。本 pass 只关闭 M6 Independent Review 门禁；不 git commit / 不 push。并列缺口不阻塞本票 Commit，除非 Commit 执行体发现实现在审查之后被改动。Coordinator 可以派发 merge/commit 执行体。
