<!-- Input: approved SPEC-007。 -->
<!-- Output: put_scope/diff_scope/put_task/set_task_state/load_board 外部行为。 -->
<!-- Pos: M4 垂直切片。 -->

# 09 — M4-01 范围与任务切片

**Type:** feature

**Priority:** P1

**Complexity:** C2

**What to build:** 用户能发布范围版本并看到与上一版的差异；能建主任务和子任务；不能直接写 task_state，只能经 set_task_state 并留下事件。

**Blocked by:** W3 done；SPEC-007 approved。

**Status:** review_pass

- [x] 五条 command 与 capability
- [x] diff_scope 覆盖增删改
- [x] put_task 拒绝 content.task_state；set_task_state 有事件
- [x] 子任务重启后仍在；CAS 冲突

## Implementation Checkpoint

**2026-09-15 / in_progress**

**2026-09-15 / review_ready**：Implementation 交付五条 command：`put_scope`、`diff_scope`、`put_task`、`set_task_state`、`load_board`。`put_task` 若 content 含 `task_state` → `invalid_input`；新建固定 `ready`。`set_task_state` 经 `write_fact` 发 `task.state_changed`。`diff_scope` 按 `relative_path` 给出 added/removed/changed。错误 envelope `{ok,command,error}`。capability 只扩这五条。未 push、不自审。

- 范围：`src-tauri/src/{lib,runtime,scope_task}.rs`、`src-tauri/permissions/m0.toml`、`src-tauri/capabilities/m0-default.json`、`schemas/command-*`、`src-tauri/tests/m4_scope_task.rs`、`tests/m0_renderer_capabilities.py`、本票。未改 `docs/progress.md`。
- 验证（2026-09-15，cwd `/private/tmp/codex-w4-scope-task`）：
  - `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests`：ok。lib/main 0 tests；m0_runtime 7；m1_facts 4；m1_projection 3；m2_5_external 5；m2_project 4；m3_discussion 2；m4_scope_task 4。共 29 passed。
  - `python3 tests/m0_renderer_capabilities.py`：renderer capability static check ok。
  - `git diff --check`：clean。

## Independent Review Checkpoint

- **2026-09-15 / review_pass**
- **结论**：pass。对照 approved SPEC-007 与本票核对清单，五条 command 已挂 handler / COMMANDS / permission / capability / registry；两次 `put_scope` 后 `diff_scope` 按 `relative_path` 给出 added/removed/changed；`put_task` 拒绝 content.`task_state`，新建 `ready`；`set_task_state` 经 `write_fact` 落盘 `task.state_changed` 且 `load_board` 显示新状态；非法 TaskState → `invalid_input`；子任务 `parent_id` 在 drop Runtime + `load_project` 后仍在；旧 `expected_revision` → `revision_conflict` 且 `error.details.revision` 为当前值。无足以单独否决本票的 P0 机器合同互斥。Coordinator 可派发 Commit/merge 执行体。不 git commit / 不 push。
- **审查范围**：`docs/issues/09-m4-scope-task.md`（本 Review 只改 Status 并追加本段）、`docs/specs/007-m4-scope-task.md`（approved）、`docs/specs/003-m1-fact-layer.md` task/scope、commit `5a8e19d`（`src-tauri/src/{lib,runtime,scope_task}.rs`、permissions/capabilities、`schemas/command-*`、`src-tauri/tests/m4_scope_task.rs`、`tests/m0_renderer_capabilities.py`、本票）。对照 `src-tauri/src/typed_facts.rs`（本 commit 未改）。未改 src-tauri 实现、未代修、未 commit、未改 progress。
- **独立性**：新 Review 执行体；本会话首条指令即为独立审查。未参与 M4 Implementation（`5a8e19d`），未共享该实现上下文。身份：Review 执行体；模型：grok-4.6；时间：2026-09-15 15:38 CST。工作树 `/private/tmp/codex-w4-scope-task` 分支 `codex/w4-scope-task`。依据独立 Review 门禁，此路径可独立。不以 Implementation 自述为证据。
- **Diff / 证据快照**：`HEAD=5a8e19d`（`feat(m4): scope versions task state events`）。审查前工作区仅未跟踪 `src-tauri/gen/`；本 Review 只追加本 Checkpoint 并将 Status 改为 `review_pass`。

### 验证（本 Review 复跑，2026-09-15，cwd `/private/tmp/codex-w4-scope-task`）

| 检查 | 结果 | 环境 |
| --- | --- | --- |
| `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests` | pass：29 passed / 0 failed。lib.rs 0；main.rs 0；m0_runtime 7；m1_facts 4；m1_projection 3；m2_5_external 5；m2_project 4；m3_discussion 2；m4_scope_task 4 | rustc/cargo；`--offline` |
| `python3 tests/m0_renderer_capabilities.py` | pass：`renderer capability static check ok`；scanned 3 renderer files | python3 |
| `git diff --check` | pass：clean | git；相对工作区 |

### P0

无未关闭、足以单独否决本票的 P0。`write_fact` 不是 Tauri command（不在 `generate_handler!` / `COMMANDS` / permissions）。`load_board` 读 `.agentup/facts/`，不读 SQLite。

### 逐项验收（对照 SPEC-007 验收与审查合同 P0）

| 必须核对 | 判断 | 测试证据 | 实现证据 |
| --- | --- | --- | --- |
| 1. 五条 command 已存在且 allowlist | 成立 | capability 静态检查要求这五条：`tests/m0_renderer_capabilities.py:52-56` | handler：`src-tauri/src/lib.rs:199-288,312-316`。COMMANDS：`src-tauri/src/runtime.rs:11-29,1464`。permissions：`src-tauri/permissions/m0.toml:18-22`。capability 仅 event listen/unlisten + `allow-m0-commands`：`src-tauri/capabilities/m0-default.json:1-10`。registry：`schemas/command-registry.json:341-477` |
| 2. 两次 `put_scope` 后 `diff_scope` 按 relative_path 报 added/removed/changed；事件 `scope.updated` | 成立 | `src-tauri/tests/m4_scope_task.rs:60-106`：r1→r2 added `src/c.rs`、changed `src/b.rs`、removed `[]`；第三次 put 后再 diff 覆盖 removed `src/a.rs`/`src/b.rs`；`second["event"]["event_type"]=="scope.updated"`（:85） | `put_scope` → `write_fact` type=scope：`scope_task.rs:2-94`。diff 按 path 分 added/removed/changed 后排序：`scope_task.rs:96-183`。持久事件：`typed_facts.rs:416-429,328-377` |
| 3. `put_task` 拒绝调用方 `task_state`（invalid_input）；新建 `task_state=ready`；用户 command 不是 `write_fact` 状态机 | 成立 | `m4_scope_task.rs:109-128`：content 带 `task_state=done` → `invalid_input`；成功创建 `fact.content.task_state=="ready"` | 拒绝：`scope_task.rs:198-206`。新建 `current_revision==0` 固定 `"ready"`，更新时复制旧 state：`scope_task.rs:285-298`。handler 无裸 `write_fact`：`lib.rs:237-262,298-318`。`COMMANDS` 无 `write_fact`：`runtime.rs:11-29` |
| 4. `set_task_state` 新 revision + 持久 `task.state_changed`；`load_board` 显示新状态；非法 TaskState → `invalid_input` | 成立 | `m4_scope_task.rs:129-141`：事件类型/payload、`load_board` `tasks[0].task_state=="in_progress"`；`"not-a-state"` → `invalid_input`。响应含 event 仅在 `persist_typed_fact` 成功之后 | 枚举校验：`scope_task.rs:334-338,480-490`。`write_fact`：`scope_task.rs:368-381`。CAS 新 revision：`typed_facts.rs:104-111,145-194`。事件 `delivery=persisted`：`typed_facts.rs:400-414`。board：`scope_task.rs:393-477`，最新 fact：`discussion.rs:280-320` |
| 5. 子任务 `parent_id` 指向父任务；drop Runtime、新 AppRuntime、`load_project`+`load_board` 仍在（磁盘事实，非 SQLite） | 成立 | `m4_scope_task.rs:144-189`：`drop(runtime)` → `AppRuntime::new` → `load_project` → `load_board`；child `parent_id=="task-main"`，`depends_on`/`task_state` 仍在；`tasks.len()==2` | `put_task` 写入 `parent_id`：`scope_task.rs:299-315`。`load_board` 只扫 `facts/tasks`+`facts/scopes`：`scope_task.rs:413-417` + `discussion.rs:280-320`。无 SQLite 查询 |
| 6. 旧 `expected_revision` → `revision_conflict` 且 `error.details.revision` 为当前值；无静默覆盖 | 成立 | `m4_scope_task.rs:192-220`：第二次 `put_scope(..., 0)` 与 `set_task_state(..., 0)` 均为 `revision_conflict`，`details.revision==1` | CAS：`typed_facts.rs:104-111`。`fail` 只透传 `relative_path|revision|field`：`runtime.rs:785-806`。`remap_command` 只改 `command` 名：`discussion.rs:374-378` |
| 7. 路径项目相对、无穿越、不写项目源码、无 Agent runner | 成立 | 本票无单独穿越用例；capability 扫描禁止 renderer fs/shell：`tests/m0_renderer_capabilities.py:9-38,52-56`。`5a8e19d` 不改 `src/` 生产源码 | `put_scope` 校验 `valid_relative_path`：`scope_task.rs:61-77`。规则拒绝绝对路径、`\`、`..`：`typed_facts.rs:561-569`。事实写在 `.agentup/facts/`（`persist_typed_fact`），entries 只是 JSON 字段。`scope_task.rs` 无 `std::process` / 无项目源写入。capability 无 fs/shell |

测试名与断言对齐：`two_put_scope_diff_covers_added_removed_changed` 虽第三次 put 额外覆盖 removed，主断言仍是增删改路径，未测错合同。

### 非 P0 残留（不挡本门）

- `put_task` 经 `write_fact` 对每次成功写入都构建 `task.state_changed`（`typed_facts.rs:400-414`），包括仅改 title。registry `emits: []`（`command-registry.json:397-424`），result schema 把 event 标成可选。与 SPEC-007「状态变更走 `set_task_state`」不互斥，但监听方可能把非状态写入当成状态变更。
- `put_task` 多了一个 registry 未列的可选 `content`，只用来拒绝 `task_state`（`lib.rs:247` / `scope_task.rs:198-206`）。
- 重启测试未删除 SQLite；门禁靠 `load_board` 代码路径读事实文件，不是「删库后再 load」。
- `put_task` 的 stale CAS、scope 路径穿越、不存在的 `parent_id` 均无本票红绿用例。

### 后续

移交 Commit。本 pass 只关闭 M4-01 Independent Review 门禁；不 git commit / 不 push。并列缺口不阻塞本票 Commit，除非 Commit 执行体发现实现在审查之后被改动。Coordinator 可以派发 merge/commit 执行体。
