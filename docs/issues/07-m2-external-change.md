<!-- Input: approved SPEC-005。 -->
<!-- Output: 五类外部变化的外部行为测试与必要 runtime 加固。 -->
<!-- Pos: W2.5 门禁票。 -->

# 07 — M2.5 外部变化门

**Type:** feature

**Priority:** P1

**Complexity:** C3

**What to build:** 外部改文件、未提交工作区文件、符号链接穿越、并发 revision、目录失效都不能静默丢掉用户改动。

**Blocked by:** M2-01 done；SPEC-005 approved。

**Status:** review_fail

- [ ] 五类变化均有失败/拒绝测试，且不写意外文件
- [ ] 不削弱已有 M0/M1/M2 测试

## Implementation Checkpoint

**2026-09-15 / review_ready**：五类外部变化测试在 `src-tauri/tests/m2_5_external.rs`，cargo test 5 passed。未削弱既有套件。

## Independent Review Checkpoint

- **2026-09-15 / review_fail**
- **结论**：fail。对照 approved SPEC-005 五条规则与本票「五类变化均有失败/拒绝测试，且不写意外文件」，W2.5 交付不能过门。`cargo test` 23 passed 只证明现有断言为绿，不能把测错合同的用例洗成验收。不得进入 Commit，不准开始 W3。本 Review 不改 `src-tauri`、不代修、不 commit、不改 `docs/progress.md`。
- **审查范围**：`docs/issues/07-m2-external-change.md`（本 Review 只改 Status 并追加本段）、`docs/specs/005-m2-external-change.md`（approved）、commit `8bcd3c5`（仅新增 `src-tauri/tests/m2_5_external.rs` 并改票/进度；无 runtime 加固）。对照既有 `src-tauri/src/runtime.rs`、`src-tauri/tests/m0_runtime.rs`、`src-tauri/tests/m2_project.rs`。未改受审实现。
- **独立性**：新 Review 执行体；本会话首条指令即为独立审查。未参与 W2.5 Implementation（`8bcd3c5`），未共享该实现上下文。身份：Review 执行体；模型：grok-4.6；时间：2026-09-15。依据 `R-RR-002`，此路径可独立。
- **Diff / 证据快照**：`HEAD=8bcd3c5e8ac7db6c00f144387db32a2d9c7fd678`（分支 `codex/w2.5-external`）。审查前工作区仅未跟踪 `src-tauri/gen/`；本 Review 只追加本 Checkpoint 并将 Status 改为 `review_fail`。

### P0

1. **并发门测错合同（SPEC-005 规则 4）**
   规格要求：同一 request 的旧 `expected_revision` 不得覆盖；错误码 `revision_conflict`；`error.details.revision` 为当前值；不产生第二事件（`docs/specs/005-m2-external-change.md:16`）。
   W2.5 用例 `concurrent_stale_revision_does_not_drop_new_fact`（`src-tauri/tests/m2_5_external.rs:84-128`）两次 `create_request` 都传 `expected_revision=0`（`112,125`），断言的是 `already_exists`（`118,127`），从不读 `details.revision`，不数 events，不读回 fact 内容。这是重复 id 的顺序拒绝，不是旧 revision CAS。M0 已有更强用例：`src-tauri/tests/m0_runtime.rs:147-181` 同时覆盖 `already_exists`、`revision_conflict` 和 `details.revision==1`。本门新增测试严格弱于既有套件，且与规则 4 的错误码互斥。
   实现侧 `create_request` 在文件已存在且 `expected_revision==0` 时走 `already_exists`（`src-tauri/src/runtime.rs:458-465`）；真正的 `revision_conflict` + `details.revision` 在 `466-472`。`persist_request` 先 `exists()` 再 `write_json_atomic`（`1123-1158`），目标文件 `rename` 可覆盖，两个 `expected_revision=0` 的并发创建可以静默丢掉先写入的 fact。本票 Output 写「必要 runtime 加固」，`8bcd3c5` 没有改 runtime。

2. **目录失效未证明「不创建 .agentup」**
   规格要求：canonical 路径不存在或不是目录 → `path_not_found` / `path_state=missing`，不在失效路径上创建 `.agentup/`（`docs/specs/005-m2-external-change.md:17`）。
   用例 `missing_directory_is_not_initialized`（`src-tauri/tests/m2_5_external.rs:130-140`）只对 `scan_project` 断言 `path_not_found`（`139`）。不调用 `initialize_project`，不断言 `path_state`，不断言失效路径未被 mkdir、未出现 `.agentup`。若 `scan_project` 把消失目录重建并写入，该测试仍绿。M2 已有更强用例：`src-tauri/tests/m2_project.rs:151-192`（`path_state=missing`、scan/load `path_not_found`、原 path 不被重建）。

### 逐项验收（对照 SPEC-005 与用户五类核对）

| 必须核对 | 判断 | 证据 |
| --- | --- | --- |
| 1. 外部改文件过期 token | 成立 | 规格：`005-m2-external-change.md:13`。测试：preview 后改 `README.md`，`initialize_project` → `confirmation_expired` 且无 `.agentup`：`m2_5_external.rs:44-57`。实现：fingerprint 与 token/入参不一致则删 token 并拒绝：`runtime.rs:306-328`。 |
| 2. 未提交工作区文件计入 fp-v1，`.git` 不计 | 成立（残余：不是真 Git 仓库） | 规格：`005-m2-external-change.md:14`。测试：改 `.git/HEAD` fp 不变，新增 `src.txt` fp 变：`m2_5_external.rs:59-70`。实现：walk 跳过名为 `.git` / `.agentup` 的目录：`runtime.rs:888-890`；普通文件进入 `fingerprint_v1`：`977-1003`。未跑 `git add`/`git status`；假 `.git` 目录足以证明排除规则，不能证明 Git index 策略。 |
| 3. 符号链接不穿越 | 成立 | 规格：`005-m2-external-change.md:15`。测试：项目内 symlink 指向根外 → `scan_project` `path_outside_project` 且无 `.agentup`：`m2_5_external.rs:72-82`。实现：`symlink_metadata` + `read_link` + 词法 `symlink_escapes`，不 follow：`runtime.rs:907-914,951-959`。与 M0 `m0_runtime.rs:210-220` 几乎同构。 |
| 4. 并发旧 revision / 重复 id 不覆盖 | **不成立（P0）** | 见上。规格要 `revision_conflict`；本门测试要 `already_exists`。 |
| 5. 目录失效不创建 `.agentup` | **不成立（P0）** | 见上。规格要 `path_not_found`/`path_state=missing` 且不写失效路径；本门测试只覆盖 scan 的错误码。 |
| 不削弱已有 M0/M1/M2 测试 | 成立 | 本 Review 复跑 `--lib --tests`：m0 7、m1_facts 4、m1_projection 3、m2_project 4 均 passed。`8bcd3c5` 未改这些文件。 |

### 并列缺口（即使修好 P0 也不等于过门，除非重审）

- 票核对清单两框仍未勾选（`docs/issues/07-m2-external-change.md:16-17`），Implementation 已标 `review_ready`。
- `require_canonical_dir` 对「存在但不是目录」返回 `invalid_input`（`runtime.rs:809-815`），不是规格字面的 `path_not_found`；`classify_path` 对此是 `not_dir`（`project_index.rs:457-476`），不是 `missing`。本轮未单独立项为 P0，因为本门测试根本没覆盖「不是目录」。
- 子 symlink 出现在 preview 之后、initialize 之前，以及 `.agentup` 自身为 symlink，没有 W2.5 新测试；后者走 `reject_agentup_symlink`（`runtime.rs:823-836`）。

### 验证命令与环境

- 仓库：`/Users/bic/Projects/agent-up`；日期 2026-09-15；macOS arm64；cwd 同上。
- `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests` → pass：lib/main 0 tests；`m0_runtime.rs` 7 passed；`m1_facts.rs` 4 passed；`m1_projection.rs` 3 passed；`m2_5_external.rs` 5 passed；`m2_project.rs` 4 passed。`--offline` 一次通过，未联网。绿测试不能覆盖 P0。
- `python3 tests/m0_renderer_capabilities.py` → N/A + reason：本票 `8bcd3c5` 未改 renderer/capabilities。
- `git diff --check` → 审查写入前无错误；审查只改本票。

### 下一步

交回 Implementation。必须补上与 SPEC-005 规则 4/5 同构的外部行为测试：旧 `expected_revision` → `revision_conflict` + `details.revision` + 无第二事件 + fact 内容未被覆盖；重复 id 不覆盖；失效路径上 `initialize_project`/`scan_project` 均不创建 `.agentup`，并处理 `path_state=missing`。若并发 `create_request` 仍能 `rename` 覆盖，必须加固 runtime。修复后保持由**另一个**未参与该修复的执行体重审。本 Review 不代修。
