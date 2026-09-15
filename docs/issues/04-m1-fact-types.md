<!-- Input: 已批准 EXEC-001 W1 与 proposed SPEC-003。 -->
<!-- Output: 八类用户事实 type-specific schema 与 writer/reader 外部行为测试。 -->
<!-- Pos: M1-01 任务票；不实现 SQLite，不接 Agent。 -->

# 04 — M1-01 事实类型合同落地

**Type:** feature

**Priority:** P1

**Complexity:** C2（公共事实接口）

**What to build:** 不打开 UI，也能把讨论、计划、任务、范围、运行、决定、结果和附件元数据写成事实并读回来；旧 revision 和重复事件被拒绝。

**Blocked by:** SPEC-003 批准（当前 proposed）。M0-03 done。

**Status:** review_ready

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
