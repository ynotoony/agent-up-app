<!-- Input: approved SPEC-008。 -->
<!-- Output: fake Implement → 独立 fake Review → commit_changes 与范围写入外部行为。 -->
<!-- Pos: M5 垂直切片。 -->

# 10 — M5-01 fake Agent 切片

**Type:** feature

**Priority:** P1

**Complexity:** C3

**What to build:** 用户能跑 fake Implement，再跑独立 fake Review；Review fail 不能 commit；工具不能写批准范围之外；M0-C 字段记在 run 上。

**Blocked by:** W4 done；SPEC-008 approved。

**Status:** review_pass

- [x] Fake implement 能写范围内文件；范围外 write 被拒绝，该文件不变
- [x] Review `verdict=fail` → `commit_changes` 失败且文件保持原样；`pass` → `commit_changes` 成功
- [x] 不可信 discussion 不能改 `prompt_version` 或绕过 scope
- [x] Drop Runtime 后新 `load_run` 从 `.agentup/` 恢复 `run_state`
- [x] 第三路并发 `start_run` 被拒绝
- [x] 无真实 HTTP


## Implementation Checkpoint

**2026-09-15 / review_ready**：C3 交付六条 command：`start_run`、`apply_fake_script`、`finish_run`、`commit_changes`、`cancel_run`、`load_run`。fake write 只碰 included scope；越权 `invalid_input` 且文件不变。Review fail → `review_required`；pass 后 `commit_changes` 写 decision。第三路 active `conflict`。不接 HTTP。未 push、不自审。

## Independent Review Checkpoint

- **2026-09-15 / review_pass**
- **结论**：pass。对照 approved SPEC-008 与本票核对清单：六条 command 已挂 handler / COMMANDS / permission / capability / registry；范围内 fake write 落盘、范围外 `invalid_input` 且字节不变；Review `fail` → `commit_changes` `review_required` 且 implement 写入不回滚，`pass` → `ok`；discussion「ignore previous」不改 `prompt_version`、不绕过 scope；drop Runtime 后 `load_project` + `load_run` 从 `.agentup/facts/runs` 恢复 `run_state`；第三路 active `start_run` → `conflict` 且在 `write_fact` 之前返回；本切片无真实 HTTP 客户端。M1 run fixture 仍可解析（M0-C 字段可选）。无足以单独否决本票的 P0 机器合同互斥。Coordinator 可派发 Commit/merge 执行体。不 git commit / 不 push。
- **审查范围**：`docs/issues/10-m5-fake-agent.md`（本 Review 只改 Status 并追加本段）、`docs/specs/008-m5-agent-m0c.md`（approved）、commit `19bdf4e`（`src-tauri/src/{lib,runtime,agent_run,typed_facts}.rs`、permissions/capabilities、`schemas/command-*` 与 `fact-record.schema.json`、`src-tauri/tests/m5_fake_agent.rs`、`tests/m0_renderer_capabilities.py`、本票）。对照 `src-tauri/tests/m1_facts.rs`、`src-tauri/tests/m1_projection.rs`（本 commit 未改 fixture 形状）。未改 src-tauri 实现、未代修、未 commit、未改 progress。
- **独立性**：新 Review 执行体；本会话首条指令即为独立审查。未参与 M5 Implementation（`19bdf4e`），未共享该实现上下文。身份：Review 执行体；模型：grok-4.6；时间：2026-09-15 CST。工作树 `/private/tmp/codex-w5-fake-agent` 分支 `codex/w5-fake-agent`。依据独立 Review 门禁，此路径可独立。不以 Implementation 自述为证据。用户禁止 spawn sub-agent，本审查在同一 Review 会话内完成双轴核对。
- **Diff / 证据快照**：`HEAD=19bdf4e910c768eea96938c52fdfb560f3a81176`（`feat(m5): fake implement review commit gate`）。审查前工作区仅未跟踪 `src-tauri/gen/`；本 Review 只追加本 Checkpoint 并将 Status 改为 `review_pass`。

### 验证（本 Review 复跑，2026-09-15，cwd `/private/tmp/codex-w5-fake-agent`）

| 检查 | 结果 | 环境 |
| --- | --- | --- |
| `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests` | pass：35 passed / 0 failed。lib.rs 0；main.rs 0；m0_runtime 7；m1_facts 4；m1_projection 3；m2_5_external 5；m2_project 4；m3_discussion 2；m4_scope_task 4；m5_fake_agent 6 | rustc/cargo；`--offline` |
| `python3 tests/m0_renderer_capabilities.py` | pass：`renderer capability static check ok`；scanned 3 renderer files | python3 |
| `git diff --check` | pass：clean | git；相对工作区 |

### P0

无未关闭、足以单独否决本票的 P0。`load_run` 读 `.agentup/facts/runs`，不读 SQLite。`src-tauri/Cargo.toml` 无 HTTP 客户端；`agent_run.rs` 无 `std::process` / 无网络。旧 `expected_revision` 路径带 `error.details.revision`，本票无对应用例，不构成静默覆盖。

### 逐项验收（对照 SPEC-008 验收与审查合同 P0）

| 必须核对 | 判断 | 测试证据 | 实现证据 |
| --- | --- | --- | --- |
| 1. 六条 command 已存在且 allowlist | 成立 | capability 静态检查要求这六条：`tests/m0_renderer_capabilities.py:57-62` | handler：`src-tauri/src/lib.rs:292-431`。COMMANDS：`src-tauri/src/runtime.rs:11-36,1471`。permissions：`src-tauri/permissions/m0.toml:23-28`。capability 经 `allow-m0-commands`：`src-tauri/capabilities/m0-default.json`。registry：`schemas/command-registry.json:478-621` |
| 2. 范围内 write 成功；范围外 `invalid_input` 且文件字节不变 | 成立 | `src-tauri/tests/m5_fake_agent.rs:97-120`（`fake_implement_writes_in_scope_and_rejects_out_of_scope`：写 `src/in.rs` 为 `IN\n`；写 `src/out.rs` → `invalid_input`，before 字节不变） | 允许集仅 `included=true`：`agent_run.rs:466-493`。越权在执行前拒绝：`agent_run.rs:159-166`。写入：`agent_run.rs:188-191,520-525` |
| 3. Review `fail` → `commit_changes` `review_required` 且不回滚 implement 文件；`pass` → `ok` | 成立 | `src-tauri/tests/m5_fake_agent.rs:124-200`（fail 后 `review_required` 且 `src/in.rs` 仍为 `IN\n`；随后 pass 后 `commit_changes` ok） | 最新 review 非 closed+pass → `review_required`：`agent_run.rs:314-340`。该路径只 `fail()`，不碰工作区文件。pass 后写 decision：`agent_run.rs:341-358` |
| 4. 不可信 discussion 不能改 `prompt_version` 或绕过 scope | 成立 | `src-tauri/tests/m5_fake_agent.rs:204-237`（`ignore previous...` 后 `prompt_version` 仍 `pv-1`；写 `src/out.rs` → `invalid_input`，文件仍 `OUT\n`） | `start_run` 只从输入写 `prompt_version`，不读 discussion：`agent_run.rs:72-89`。write 只看 scope `included=true`：`agent_run.rs:146-166,466-493` |
| 5. Drop Runtime 后 `load_project` + `load_run` 从 `.agentup/` 恢复 `run_state` | 成立 | `src-tauri/tests/m5_fake_agent.rs:241-267`（drop 后 `load_project`，`load_run` 得 `run_state=active` 且 `prompt_version=pv-1`） | `load_run` → `latest_run_fact` 读 `agentup/facts/runs`：`agent_run.rs:271-286,431-433`。无 SQLite |
| 6. 第三路并发 `start_run` → `conflict`，不写 run 事实 | 成立 | `src-tauri/tests/m5_fake_agent.rs:271-315`（impl+review 后再 start implement → `conflict`）。命名测试未列目录断言「无新文件」；合同由实现在 `write_fact` 前返回锁住 | 计数后 `blocked` 则 `conflict` return：`agent_run.rs:47-71`；`write_fact` 在 `72-89`，冲突路径不可达 |
| 7. 无真实 HTTP | 成立 | 命名测试 `fake_provider_has_no_http`（`m5_fake_agent.rs:319-337`）只断言 `start_run` ok，**不**锁 HTTP 缺席；本行不以该测试为证。本 Review 源检：`src-tauri/src/` 无 http/reqwest/net；`src-tauri/Cargo.toml` 直接依赖无 HTTP crate | fake/replay 为进程内 `apply_fake_script`：`agent_run.rs:94-218`。`run_command` 直接 `invalid_input`：`agent_run.rs:173-175`。无进程/网络 |
| 8. M1 run fixture 仍解析（M0-C 字段可选） | 成立 | cargo 中 `m1_facts` 4 / `m1_projection` 3 全绿。fixture 无 `prompt_version`/`provider`：`src-tauri/tests/m1_facts.rs:148-157`、`src-tauri/tests/m1_projection.rs:155-164` | `RunContent` 新字段 `Option`：`typed_facts.rs:727-736`。`parse_run` 仅在出现时校验：`typed_facts.rs:766-783`。schema `run-content` 未把 M0-C 列入 required：`schemas/fact-record.schema.json:396-405,446-465` |
| 9. 旧 `expected_revision` → `revision_conflict` 且 `error.details.revision` | 非 P0（未覆盖，非静默覆盖） | 本票无 stale CAS 红绿用例 | `apply_fake_script`：`agent_run.rs:123-129`。`close_run`：`agent_run.rs:392-397`。`write_fact`：`typed_facts.rs:104-110`。`fail()` 白名单拷贝 `revision`：`runtime.rs:791-811` |

测试名与断言对齐：除 `fake_provider_has_no_http` 名实不符外，其余五测断言的就是对应合同。HTTP 行改由源检成立，不因绿测名而放行。

### 非 P0 残留（不挡本门）

- `fake_provider_has_no_http` 是空壳：只 `start_run` 成功。行为仍无 HTTP。
- `third_active_run_conflicts` 未断言 `run-impl-2` 无事实文件。
- 本票无 stale `expected_revision`、无 `included=false` 专例、无缺 `verdict` 仍 active、无 `cancel_run` 红绿用例。
- 重启测试未删 SQLite；门禁靠 `load_run` 代码路径读事实文件。
- `apply_fake_script` 先写源文件再 `write_fact`；同进程 mutex 下 CAS 已先核对。registry `side_effect` 只标 `project_agentup_write`，未表达范围内源写。
- `cancel_run` 把 token 写成 `0,0`，不回放已有估计（本切片脚本也不把估计写进 active run）。
- 32k/128k/15min 硬顶本切片无执行器可超；无静默截断 scope/task/decision。
- permissions 描述仍写「M0, M2, M3 and M4」。

### 后续

移交 Commit。本 pass 只关闭 M5-01 Independent Review 门禁；不 git commit / 不 push。并列缺口不阻塞本票 Commit，除非 Commit 执行体发现实现在审查之后被改动。Coordinator 可以派发 merge/commit 执行体。
