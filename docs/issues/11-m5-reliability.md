<!-- Input: approved SPEC-009。 -->
<!-- Output: 取消、超时、重试、崩溃恢复与无子进程的外部行为。 -->
<!-- Pos: M5.5 运行可靠性垂直切片。 -->

# 11 — M5.5 运行可靠性切片

**Type:** feature

**Priority:** P1

**Complexity:** C3

**What to build:** 用户能取消正在跑的 fake run；15 min 必须停；provider 超时最多自动重试一次；越权写入不重试；崩溃后能 load 再 cancel；关 App 不等于后台继续跑；本切片不拉起 OS 子进程。

**Blocked by:** M5-01 done；SPEC-009 approved。

**Status:** review_pass

- [x] `cancel_run` 后 `apply_fake_script` 失败；取消后文件不变
- [x] `advance_run_clock` 超过 15 min → `run_state=interrupted`，restart 后 `load_run` 仍在
- [x] 一次模拟 provider timeout 会重试；连续第二次 timeout 中断
- [x] 越权写入仍不重试
- [x] 无子进程被拉起


## Implementation Checkpoint

**2026-09-15 / review_ready**：`advance_run_clock` 15 min → `interrupted`/`error_code=timeout`。`{tool:timeout}` 第一次重试仍 active，第二次中断。`cancel_run` 后 apply 失败。越权写不增加 timeout count。`agent_run.rs` 无 process spawn。未 push、不自审。

## Independent Review Checkpoint

- **2026-09-15 / review_pass**
- **结论**：pass。对照 approved SPEC-009 与本票 P0：`cancel_run` 后 `apply_fake_script` 为 `invalid_input` 且取消后文件字节不变；`advance_run_clock` `elapsed_ms>=900000` → `run_state=interrupted` 且 `error_code=timeout`，drop Runtime 后 `load_run` 仍读到该事实；第一次 `{tool:"timeout"}` 仍 `active` 并标记重试，连续第二次中断；越权 write → `invalid_input`、不增加 timeout 计数、不中断、文件不变；`agent_run.rs` 无 `std::process` / `Command::` / spawn；`advance_run_clock` 已进 COMMANDS / handler / permission / registry / capability 链；M1 七测仍绿，新 run 字段为可选。无足以单独否决本票的 P0。Coordinator 可派发 Commit。不 git commit / 不 push。
- **审查范围**：`docs/issues/11-m5-reliability.md`（本 Review 只改 Status 并追加本段）、`docs/specs/009-m5-reliability.md`（approved）、commit `3b8ed1e`（`src-tauri/src/{lib,runtime,agent_run,typed_facts}.rs`、permissions/capabilities、schemas、`src-tauri/tests/m5_5_reliability.rs`、`tests/m0_renderer_capabilities.py`）。对照 `src-tauri/tests/m1_facts.rs`、`src-tauri/tests/m1_projection.rs`（本切片未改 fixture 形状）。未改 src-tauri 实现、未代修、未 commit、未改 `docs/progress.md`、未写 `gen/`。
- **独立性**：新 Review 执行体；本会话首条指令即为独立审查。未参与 M5.5 Implementation（`3b8ed1e` / `98713e0`），未共享该实现上下文。身份：Review 执行体；模型：grok-4.6；时间：2026-09-15 CST。工作树 `/private/tmp/codex-w5.5-reliability` 分支 `codex/w5.5-reliability`。依据独立 Review 门禁，此路径可独立。不以 Implementation 自述为证据。用户禁止 spawn sub-agent，本审查在同一 Review 会话内完成双轴核对。
- **Diff / 证据快照**：`HEAD=3b8ed1eaf86e421f036dfdd0e6d174039a972de8`（`feat(m5.5): cancel timeout retry without subprocesses`）。审查前工作区仅未跟踪 `src-tauri/gen/`；本 Review 只追加本 Checkpoint 并将 Status 改为 `review_pass`。

### 验证（本 Review 复跑，2026-09-15，cwd `/private/tmp/codex-w5.5-reliability`）

| 检查 | 结果 | 环境 |
| --- | --- | --- |
| `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests` | pass：40 passed / 0 failed。lib.rs 0；main.rs 0；m0_runtime 7；m1_facts 4；m1_projection 3；m2_5_external 5；m2_project 4；m3_discussion 2；m4_scope_task 4；m5_5_reliability 5；m5_fake_agent 6 | rustc/cargo；`--offline`；`CARGO_TARGET_DIR=/tmp/codex-w5.5-reliability-target` |
| `python3 tests/m0_renderer_capabilities.py` | pass：`renderer capability static check ok`；scanned 3 renderer files | python3 |
| `git diff --check` | pass：clean | git；相对工作区 |

### P0

无未关闭、足以单独否决本票的 P0。七行验收均有对口测试且断言锁的是合同而非空壳。`load_run` 读 `.agentup/facts/runs`，不读 SQLite。`agent_run.rs` 无 process spawn。关 App 的进程内替身是 `cancel_run`（规格允许不测真实关 App）。

### 逐项验收（对照 SPEC-009 验收与审查合同 P0）

| 必须核对 | 判断 | 测试证据 | 实现证据 |
| --- | --- | --- | --- |
| 1. `cancel_run` 后 `apply_fake_script` 失败；取消后文件不变 | 成立 | `src-tauri/tests/m5_5_reliability.rs:81-105`（`cancel_run_blocks_later_writes`：先写 `src/in.rs`=`IN\n`，`cancel_run` 后写 `NOPE\n` → `invalid_input`，文件仍 `IN\n`） | 非 `active` 立即 `invalid_input`：`agent_run.rs:131-132`。写文件只在该检查之后：`agent_run.rs:221-230`。`cancel_run` → `close_run(..., "interrupted", ...)`：`agent_run.rs:358-376,512-542` |
| 2. `advance_run_clock` ≥ 900000 → `interrupted` + `error_code=timeout`；drop Runtime 后 `load_run` 仍在 | 成立 | `src-tauri/tests/m5_5_reliability.rs:110-126`（`clock_timeout_persists_after_restart`：`elapsed_ms=900_000` 后 `run_state`/`error_code`；`drop(runtime)` → `load_project` + `load_run` 同值） | 阈值：`agent_run.rs:304-318`。`error_code` 写入事实：`agent_run.rs:538-542`。`load_run` → `latest_run_fact`：`agent_run.rs:384-399,547-549` |
| 3. 第一次 `{tool:timeout}` 重试仍 `active`；连续第二次中断 | 成立 | `src-tauri/tests/m5_5_reliability.rs:130-144`（第一次 `ok`、`run_state=active`、`retried=true`；第二次 `interrupted` + `error_code=timeout`） | 计数 0→1 且不关 run：`agent_run.rs:186-219`。`prior>=1` → `close_run` interrupted/timeout：`agent_run.rs:188-204` |
| 4. 越权 write → `invalid_input`，不增加 retry、不中断 | 成立 | `src-tauri/tests/m5_5_reliability.rs:148-165`（`src/out.rs` → `invalid_input`；`run_state=active`；`provider_timeout_count` 缺或 0；字节仍 `OUT\n`） | 越权在 timeout 计数之前返回：`agent_run.rs:164-171`，`186-207` 不可达。无 `write_fact`。磁盘写更晚：`agent_run.rs:226-230` |
| 5. `agent_run.rs` 无 `std::process` / `Command` spawn | 成立 | `src-tauri/tests/m5_5_reliability.rs:169-174`（源码不含 `std::process` / `Command::` / `std::os::unix::process`）。本 Review 复检：亦无 `.spawn(` / `tokio::process` | `run_command` 直接 `invalid_input`：`agent_run.rs:178-179`。clock/cancel/apply 无进程 API |
| 6. `advance_run_clock` 已 allowlist | 成立 | `tests/m0_renderer_capabilities.py:57-66` 要求 permissions 含该名；python 检查绿 | COMMANDS：`src-tauri/src/runtime.rs:11-36`。handler + invoke：`src-tauri/src/lib.rs:400-409,446`。permissions：`src-tauri/permissions/m0.toml:29`。capability 经 `allow-m0-commands`。registry：`schemas/command-registry.json:625-645` |
| 7. M1 测试仍绿（新 run 字段可选） | 成立 | cargo 中 `m1_facts` 4 / `m1_projection` 3 全绿。fixture 无 `elapsed_ms`/`provider_timeout_count`/`error_code` | `RunContent` 新字段 `Option`：`typed_facts.rs:726-741`。`parse_run` 仅在出现时校验：`typed_facts.rs:789-791`。schema `run-content` required 仍五字段：`schemas/fact-record.schema.json` `$defs.run-content.required` |

测试名与断言对齐：五测锁的就是对应合同，无「绿测但断言错对象」行。

### 非 P0 残留（不挡本门）

- `provider_timeout_count` 在成功 write 后不归零；timeout → write → timeout 会中断，规格字面是「连续第二次」。P0 用例是连续两次 timeout，已锁住。
- `apply_fake_script` registry `output_required` 仍含 `results`；timeout 重试路径返回 `retried`、无 `results`。测试不跑 JSON Schema。
- 无红绿用例：`elapsed_ms<900000` 仍 active；已关闭后再 `advance_run_clock`；崩溃时仍 active 再 `cancel_run`（M5 已覆盖 drop 后 load active）。
- 子进程清理规格还写了「fake timeout 后 `cancel_run`」；本票静态源检 + 取消写测分开覆盖，无同测串联。
- 同一 `apply` 里 timeout+合法 write 会走 timeout 分支、不执行 prepared writes。
- permissions 描述仍写「M0, M2, M3 and M4」。

### 后续

移交 Commit。本 pass 只关闭 M5.5 Independent Review 门禁；不 git commit / 不 push。并列缺口不阻塞本票 Commit，除非 Commit 执行体发现实现在审查之后被改动。Coordinator 可以派发 merge/commit 执行体。
