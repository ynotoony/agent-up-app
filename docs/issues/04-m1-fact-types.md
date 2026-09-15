<!-- Input: 已批准 EXEC-001 W1 与 proposed SPEC-003。 -->
<!-- Output: 八类用户事实 type-specific schema 与 writer/reader 外部行为测试。 -->
<!-- Pos: M1-01 任务票；不实现 SQLite，不接 Agent。 -->

# 04 — M1-01 事实类型合同落地

**Type:** feature

**Priority:** P1

**Complexity:** C2（公共事实接口）

**What to build:** 不打开 UI，也能把讨论、计划、任务、范围、运行、决定、结果和附件元数据写成事实并读回来；旧 revision 和重复事件被拒绝。

**Blocked by:** SPEC-003 批准（当前 proposed）。M0-03 done。

**Status:** review_fail

- [x] fact-record schema 以 type-specific content 取代八类 generic object。
- [x] 上列类型各有至少一条 fixture 写入/读回测试。
- [x] CAS 与 event_replay 行为与 SPEC-001/003 一致。
- [x] 不新增 renderer 系统 capability。

## Implementation Checkpoint

**2026-09-15 / in_progress**：在 SPEC-003 仍为 proposed 时先落地 schema（与 M0-02 同期合同方式相同）。不把 SPEC-003 标成 approved。writer/reader 测试另提交。

**2026-09-15 / review_ready**：Implementation 交付 M1-01 writer/reader 外部行为。八类事实写入 `.agentup/facts/<type>s/<id>.r<revision>.json` 并可读回；CAS 旧 revision → `revision_conflict`；重复 `event_id` → `event_replay`。未实现 SQLite，未新增 Tauri command / renderer fs/shell capability，未把 SPEC-003 标为 approved，未接 Agent。附件只写元数据。
验证（2026-09-15，macOS aarch64，cwd `/Users/bic/Projects/agent-up`）：
- `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests` → `tests/m0_runtime.rs` 7 passed；`tests/m1_facts.rs` 4 passed（八类写入/重启读回、旧 revision、event_replay、附件字节/超限）。
- `python3 tests/m0_schema_parse.py` → schema JSON parse 通过。
- `git diff --check` → 无错误。
下一步：独立 Review（本 Implementation 不自审）。不 push。

## Independent Review Checkpoint

- **2026-09-15 / review_fail**
- **结论**：fail。未关闭 P0：SPEC-003 仍为 `proposed`，本票 `Blocked by` 未满足，C2 实现不能对未批准规格 `review_pass`。对照 SPEC-003 **文本** 与 schema/writer 的七条核对大体对齐，且本 Review 复跑测试为绿——这不能把 proposed 规格洗成 approved，也不能把未满足 blocker 的 C2 票洗成 pass。不得由本 Review 代修、批准 SPEC-003 或 commit。
- **审查范围**：`docs/issues/04-m1-fact-types.md` 任务合同与 Implementation Checkpoint（只读）、`docs/specs/001-m0-foundation.md`、`docs/specs/002-m0-runtime-contract.md`、`docs/specs/003-m1-fact-layer.md`（proposed）、`docs/specs/README.md`、`docs/execution-plan.md` W1 上限、`schemas/fact-record.schema.json`、`schemas/event.schema.json`、`src-tauri/src/{lib,runtime,typed_facts}.rs`、`src-tauri/tests/m1_facts.rs`、`src-tauri/permissions/m0.toml`、`src-tauri/capabilities/m0-default.json`、`src-tauri/src/lib.rs` invoke 列表。未改 schemas/src/src-tauri/SPEC，未改写 Implementation Checkpoint。
- **独立性**：新 Review 执行体；本会话首条指令即为独立审查。未参与 M1-01 Implementation（`ffa3f1b` / `3fd1124`），未共享该实现上下文。身份：Review 执行体；模型：grok-4.6；时间：2026-09-15 13:41 CST。依据 `R-RR-002`，此路径可独立。
- **Diff / 证据快照**：`HEAD=ffa3f1b8998c0d494b43b7fd5874e69b485737f8`（分支 `codex/m1-01-writer`）。审查前工作区仅未跟踪 `src-tauri/gen/`；审查只追加本 Checkpoint 并将 Status 改为 `review_fail`。

### P0

| P0 | 判断 | 证据 |
| --- | --- | --- |
| SPEC-003 未批准却驱动 C2 实施/审查通过 | **未关闭 → 本票 fail** | 票自身 `Blocked by: SPEC-003 批准（当前 proposed）`：`docs/issues/04-m1-fact-types.md:15`。规格 `ApprovalState: proposed`：`docs/specs/003-m1-fact-layer.md:7`；目录规则「只有 `approved` 规格可驱动实施」：`docs/specs/README.md:10,25`。三道门禁：实施/生效须有可追溯 `ApprovalState: approved`：`docs/development-process.md:324-327`（R-DP-011）。Review 输入是 **approved** 规格：`docs/agent/roles/review.md` §1。AGENTS.md / 流程：C2 blocker 未满足不得动手。Implementation 自己写明「SPEC-003 仍为 proposed 时先落地」：Implementation Checkpoint 2026-09-15 / in_progress（本 Review 不改写该段）。M0-02 类比不成立：M0-02 交付的是合同草案本身；本票是 C2 writer/reader **实现**。 |

无其他未关闭、足以单独否决八类 content 形状的 P0 机器合同互斥。不得用「测试绿了」代替上表。

### 逐项验收（对照票核对清单与 SPEC-003 文本）

下列核对在 **假设 SPEC-003 已被批准且文本不再变** 时成立；因 P0 未关闭，不能得出本票 pass。

| 必须核对 | 判断 | 证据 |
| --- | --- | --- |
| 1. 八类事实 type-specific content，不再接受 generic object | 对照文本成立 | `fact-record.schema.json:6-40` 的 `oneOf` 仅为 request/project/manifest + discussion/plan/task/scope/run/decision/result/attachment，无 generic object 分支。八类 `$defs/*-content` 均 `additionalProperties: false` 且 required 对齐 SPEC-003：discussion `:209-244`、plan `:246-289`、task `:291-337`、scope `:370-391`、run `:393-446`、decision `:448-511`、result `:513-548`、attachment `:550-584`。writer 按类型解析并 `deny_unknown_fields`：`typed_facts.rs:526-537,593-881`。 |
| 2. 写入 `.agentup/` 可读回；目录按需创建 | 对照文本成立 | 路径 `.agentup/facts/<type>s/<id>.r<revision>.json`：`typed_facts.rs:96,257-268,344`。`create_dir_all` 仅在 persist 时创建 facts 子目录与 `events/`：`typed_facts.rs:342-343`。fixture 八类落盘并 `drop` runtime 后 `load_project` + `read_fact` 字段一致：`src-tauri/tests/m1_facts.rs:197-257`（含 `:222-229` 路径断言）。SPEC-003 目录清单未写 `facts/` 前缀，但与已批准 M0 `facts/requests/` 布局连续，不单列为本票 P0。 |
| 3. 旧 revision → `revision_conflict` 且能表达当前 revision | 对照文本成立 | CAS：`typed_facts.rs:99-110`，`error.details.revision` 为当前 revision。测试写 r1→r2 后用 expected=1 被拒，details.revision=2，目录 listing 不变，读回 body 仍为 second：`m1_facts.rs:259-317`。符合 SPEC-001 CAS 与 SPEC-002 `revision_conflict` 必须报告当前 revision。 |
| 4. 重复 `event_id` → `event_replay`，不二次落盘 | 对照文本成立（有残留） | persist 在写新事实前若事件文件已存在则 `event_replay`：`typed_facts.rs:352-360`。测试用另一 `fact_id` 重放同一 event_id，listing 不变且 `disc-other` 读不到：`m1_facts.rs:319-355`。五类 M1 持久事件写入并在重启后出现在 `load_project` events：`typed_facts.rs:384-461`，`event.schema.json:20-32`，`m1_facts.rs:237-247`。残留：无持久事件的类型（plan/run/attachment/open decision）会丢弃调用方 `event_id`，不查全局唯一。 |
| 5. 附件无字节；超限拒绝 | 对照文本成立（有残留） | content 禁止 `bytes`/`data`/`content`：`typed_facts.rs:850-857`；schema `additionalProperties: false` 且无字节字段：`fact-record.schema.json:550-584`。单文件上限 10 MiB 与 EXEC-001 / schema `maximum: 10485760` 对齐：`typed_facts.rs:5,477-483`，`fact-record.schema.json:573-577`。单需求 100 MiB：`typed_facts.rs:6,504-512`。测试：带 bytes → `malformed_fact`；10MiB+1 → `invalid_input`；10×10MiB 后再 +1 拒绝且不落 overflow：`m1_facts.rs:358-433`。残留：总额把同一附件的历史 revision 文件累加，测试未覆盖更新路径。 |
| 6. 未新增 renderer fs/shell；未把 SQLite 当事实源 | 成立 | invoke 仍五命令：`src-tauri/src/lib.rs:101-107`。capability 无 fs/shell/network：`src-tauri/capabilities/m0-default.json:5-8`；权限白名单仍五 M0 command：`src-tauri/permissions/m0.toml:4-10`。`write_fact`/`read_fact` 仅 runtime 内部方法，未注册 Tauri command。全 `src-tauri` 无 sqlite/rusqlite/sqlx。`load_facts` 仍只扫 `facts/requests`（`runtime.rs:1257-1282`），不把 SQLite 当权威。 |
| 7. 本 Review 复跑验证 | 命令绿，不能导致 pass | 见下表。 |

### 非阻断残留（不单独构成 fail，批准后重审仍应看见）

- `discussion.body`：SPEC-003 为 100000 **字**，schema `maxLength: 100000`（字符）；writer 用 `String::len()` 字节：`typed_facts.rs:598`。ASCII fixture 遮住该缝。
- 附件总额扫描目录内全部 `.json` revision，而非每 id 最新 revision：`typed_facts.rs:486-503`。
- `load_project` 的 `facts` 数组仍只有 manifest+request，八类靠内部 `read_fact` 读回；若 SPEC-003 批准时把「读回」解释成 `load_project` 输出，当前实现不够。

### 验证（本 Review 复跑）

| 检查 | 结果 | 环境 |
| --- | --- | --- |
| `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests` | pass：lib 0；`tests/m0_runtime.rs` 7 passed；`tests/m1_facts.rs` 4 passed（`eight_typed_facts_persist_and_reload_after_restart`、`stale_revision_is_rejected_and_does_not_overwrite`、`duplicate_event_id_is_event_replay_and_does_not_apply_again`、`attachment_rejects_bytes_and_oversize`） | rustc/cargo 1.94.0；macOS aarch64；cwd `/Users/bic/Projects/agent-up`；2026-09-15 |
| `python3 tests/m0_schema_parse.py` | pass：6 个 schema JSON 可解析，`schema parse ok` | Python 3.14.6；同 cwd |
| `git diff --check` | pass：无空白错误 | git；相对 `HEAD=ffa3f1b` |

### 下一步

交回 Implementation/Coordinator：先由用户把 SPEC-003 标为 `approved`（或改合同后再批），blocker 满足后重新派独立 Review。本 Review 不代批规格、不改代码。
