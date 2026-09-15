<!-- Input: approved SPEC-005。 -->
<!-- Output: 五类外部变化的外部行为测试与必要 runtime 加固。 -->
<!-- Pos: W2.5 门禁票。 -->

# 07 — M2.5 外部变化门

**Type:** feature

**Priority:** P1

**Complexity:** C3

**What to build:** 外部改文件、未提交工作区文件、符号链接穿越、并发 revision、目录失效都不能静默丢掉用户改动。

**Blocked by:** M2-01 done；SPEC-005 approved。

**Status:** done

- [ ] 五类变化均有失败/拒绝测试，且不写意外文件
- [ ] 不削弱已有 M0/M1/M2 测试

## Implementation Checkpoint

**2026-09-15 / review_ready**：五类外部变化测试在 `src-tauri/tests/m2_5_external.rs`，cargo test 5 passed。未削弱既有套件。

- **2026-09-15 / review_ready**：补 `write_json_exclusive`（hard_link 不覆盖）；测试打到 `revision_conflict`+`details.revision`、事实未改、无第二事件；失效路径上 `list_projects.path_state=missing`、`initialize_project`/`scan_project` 为 `path_not_found` 且不创建 `.agentup`。

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

## Independent Review Checkpoint

- **2026-09-15 / review_pass**
- **结论**：pass。对照 approved SPEC-005 五条规则、本票「五类变化均有失败/拒绝测试，且不写意外文件」，以及上一轮 fail 写下的 P0（规则 4 打到 `revision_conflict` + `details.revision` + 无第二事件 + fact 仍在；规则 5 的 initialize/scan 在失效路径上 `path_not_found`、`path_state=missing`、不创建 `.agentup`；exclusive persist 不得 `rename` 覆盖），`2f5b5ab` 补上了与合同同构的外部行为测试和 request 持久化加固。`cargo test` 23 passed 只是必要条件；本轮按源码与测试断言逐项核对，不因「修过了」自动过门。上一轮 `review_fail` 记录全文保留。不得把本 pass 理解成 Review 代 commit；本 Review 不改 `src-tauri`、不代修、不 commit、不改 `docs/progress.md`。
- **审查范围**：`docs/issues/07-m2-external-change.md`（本 Review 只改 Status 并追加本段）、`docs/specs/005-m2-external-change.md`（approved）、commit `2f5b5ab`（`src-tauri/src/runtime.rs` exclusive persist + `src-tauri/tests/m2_5_external.rs` 规则 4/5 测试；票 Implementation Checkpoint）。对照 `src-tauri/src/runtime.rs`、`src-tauri/src/project_index.rs`、`src-tauri/tests/m0_runtime.rs`、`src-tauri/tests/m2_project.rs`。未改受审实现。
- **独立性**：新 Review 执行体；本会话首条指令即为独立审查。未参与 W2.5 Implementation（`8bcd3c5`）也未参与修复（`2f5b5ab`），未共享该实现上下文。身份：Review 执行体；模型：grok-4.6；时间：2026-09-15。依据流程独立 Review 要求，此路径可独立。
- **Diff / 证据快照**：`HEAD=2f5b5ab5e64a7dd78753f3d57003558a20e98aee`（分支 `codex/w2.5-external`）。审查前工作区仅未跟踪 `src-tauri/gen/`；本 Review 只追加本 Checkpoint 并将 Status 改为 `review_pass`。

### 上次 P0 复核（必须打到的合同，不是「有测试」）

1. **规则 4：`revision_conflict` + `details.revision` + 无第二事件 + fact 仍在**
   规格：`docs/specs/005-m2-external-change.md:16`。
   测试 `concurrent_stale_revision_does_not_drop_new_fact`（`src-tauri/tests/m2_5_external.rs:99-143`）：先 `expected_revision=0` 写入 title=`one`（`103-110`）；第二 runtime 对同一 id 再 `expected_revision=0` → `already_exists`（`116-123`）；再 `expected_revision=1` → `revision_conflict` 且 `error.details.revision == 1`（`124-132`）；回读 fact `content.title` 仍为 `one`（`134-138`）；`events.len()` 等于冲突前（`115,139-140`），且仍含第一次 `event_id`（`141`）。
   实现：存在且 `expected_revision==0` → `already_exists`（`src-tauri/src/runtime.rs:458-465`）；`expected_revision != current_revision` → `revision_conflict` + `details.revision`（`466-472`）；`expected_revision != 0` 的 create 拒绝更新，同样 `revision_conflict` + `details.revision`（`474-480`）。本测试走后一分支（current=1 且 expected=1），与 M0 `src-tauri/tests/m0_runtime.rs:147-181` 同构，错误码不再与规则 4 互斥。persist 失败则不 push 通知（`560-564`）。

2. **规则 5：initialize/scan 失效路径 `path_not_found`，`path_state=missing`，不创建 `.agentup`**
   规格：`docs/specs/005-m2-external-change.md:17`。
   测试 `missing_directory_is_not_initialized`（`src-tauri/tests/m2_5_external.rs:145-175`）：登记后 drop 目录；`list_projects` 项 `path_state=missing`（`163-169`）；`scan_project` → `path_not_found`（`170-171`）；`initialize_project` → `path_not_found`（`172-173`）；`gone/.agentup` 不存在（`174`）。
   实现：`scan_project`/`initialize_project` 均先 `require_canonical_dir`/`bind_existing`（`86-90`、`269-271`、`688-689`）；路径不存在返回 `path_not_found` 且不 mkdir（`797-801`），因此到不了 `commit_initialize`（`1055-1063`）。`list_projects` 用 `classify_path` 的 NotFound → `missing`（`src-tauri/src/project_index.rs:6-36`、`457-459`）。

3. **exclusive persist 不得 `rename` 覆盖**
   `persist_request` 对 fact/event 调用 `write_json_exclusive`（`src-tauri/src/runtime.rs:1135-1136`）。`write_json_exclusive` 用 `create_new` 写临时文件后 `hard_link` 到目标（`1143-1172`）；目标已存在则拒绝 `already_exists`，**没有** `fs::rename`。`write_json_atomic` 仍 `rename`（`1175-1193`），但 `persist_request` 不再调用它。Unix `link(2)` 在目标存在时失败，不能替换已有 inode。

### 逐项验收（对照 SPEC-005）

| 必须核对 | 判断 | 证据 |
| --- | --- | --- |
| 1. 外部改文件过期 token | 成立 | 规格：`005-m2-external-change.md:13`。测试：preview 后改 `README.md`，`initialize_project` → `confirmation_expired` 且无 `.agentup`：`m2_5_external.rs:44-57`。实现：fingerprint 与 token/入参不一致则删 token 并拒绝：`runtime.rs:321-330`。 |
| 2. 未提交工作区文件计入 fp-v1，`.git` 不计 | 成立 | 规格：`005-m2-external-change.md:14`。测试：改 `.git/HEAD` fingerprint 不变，新增 `src.txt` 变化：`m2_5_external.rs:60-71`。实现：walk 跳过 `.git`/`.agentup`：`runtime.rs:888-889`。 |
| 3. 符号链接不穿越项目根 | 成立 | 规格：`005-m2-external-change.md:15`。测试：子路径 symlink 指向项目外 → `path_outside_project` 且无 `.agentup`：`m2_5_external.rs:73-82`。实现：`symlink_escapes` 拒绝：`runtime.rs:904-910`。 |
| 4. 旧 revision 不覆盖 + 无第二事件 | 成立 | 见上次 P0 复核 1。 |
| 5. 目录失效不建 `.agentup` | 成立（缺失路径） | 见上次 P0 复核 2。 |
| 不削弱 M0/M1/M2 | 成立 | `2f5b5ab` 未改 `m0_runtime.rs`/`m1_facts.rs`/`m1_projection.rs`/`m2_project.rs`；本轮 cargo 下这些套件仍全绿。 |

### 非 P0 残留（不挡本门）

- 规则 4 测试是双 runtime **顺序**调用，不是线程竞态。竞态不覆盖的担保在 `hard_link`，没有单独的 threaded race 测试。
- `expected_revision=0` 撞现有 request 仍是 `already_exists`（与 SPEC-002/M0 一致），规则 4 的 `revision_conflict` 由非 0 expected 打到。
- SPEC-005 字面「不是目录」→ `path_not_found` / `path_state=missing`；实现仍是 `invalid_input`（`runtime.rs:809-814`）与 `path_state=not_dir`（`project_index.rs:466,472`），与 SPEC-004 的 `ok|missing|not_dir|unreadable` 一致。本门测试未覆盖「路径存在但不是目录」。上一轮已标明非本轮 P0，本轮不移动门柱。
- `typed_facts.rs` 的 revisioned writer 仍走 `write_json_atomic`/`rename`；不是 `create_request` 路径，不构成本门 P0。

### 验证命令与环境

- 仓库：`/Users/bic/Projects/agent-up`；日期 2026-09-15；macOS arm64；cwd 同上；rustc 1.94.0；cargo 1.94.0。
- `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests` → pass：lib/main 0 tests；`m0_runtime.rs` 7 passed；`m1_facts.rs` 4 passed；`m1_projection.rs` 3 passed；`m2_5_external.rs` 5 passed；`m2_project.rs` 4 passed。`--offline` 一次通过，未联网。
- `python3 tests/m0_renderer_capabilities.py` → N/A + reason：本票 `2f5b5ab` 未改 renderer/capabilities。
- `git diff --check` → 审查写入前无错误；审查只改本票。

### 下一步

Coordinator：本门 Independent Review pass。上一轮 fail 仍在票内可对照。本 Review 不提交、不改进度、不启动 W3。W3 讨论/附件实现按 SPEC-005 验收句，须由用户/Coordinator 在本 pass 之后另启。
