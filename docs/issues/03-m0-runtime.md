<!-- Input: 已批准的 SPEC-001、SPEC-002，以及 M0-02 review_pass。 -->
<!-- Output: M0 runtime 垂直切片任务：五命令、最小 capabilities、首条恢复验收与五类威胁的外部行为测试。 -->
<!-- Pos: M0-03 任务票；不接入真实 Agent、SQLite、附件或项目源文件写入。 -->

# 03 — M0-03 Runtime 恢复切片

**Type:** feature

**Priority:** P1

**Complexity:** C3（路径安全边界、原子写入、capabilities 默认拒绝）

**What to build:** 用户选择一个项目目录后，可以扫描、预览初始化、确认后得到最小事实目录、创建一条草稿需求，关闭并重开应用后仍能从事实恢复；越权路径、未确认写入、旧 revision 和重复事件会被拒绝并返回可读错误。

**Blocked by:** 02 — M0-02 Runtime 合同（done）；`SPEC-002` approved。

**Status:** done

- [ ] `scan_project`、`preview_initialize`、`initialize_project`、`create_request`、`load_project` 通过 typed invoke 可用，renderer 无 filesystem/shell/process/network/secret capability。
- [ ] 预览不写磁盘；无 confirmation token 或 fingerprint 变化时初始化被拒绝。
- [ ] 确认后只在项目根创建最小 `.agentup/`，使用临时目录原子提交；失败保留旧状态。
- [ ] 可创建 `RequestLifecycle: draft` 需求；重复 `request_id` 或旧 revision 不产生第二次状态变化。
- [ ] 关闭重开后 `load_project` 不依赖 SQLite 或内存恢复项目与需求。
- [ ] 路径逃逸、符号链接穿越、事件重放返回稳定错误码，不回显根外路径或文件内容。

## Implementation Checkpoint

**2026-09-15 / in_progress**：Coordinator 派 C3 Implementation。权威：`SPEC-002`、`EXEC-001` W0。不得另写第二套 fingerprint 或错误 envelope。

**2026-09-15 / review_ready**：Implementation 交付 M0 runtime 垂直切片。范围：仓库根 Tauri 2 + React/Vite renderer；五条 typed command（`scan_project` / `preview_initialize` / `initialize_project` / `create_request` / `load_project`）；deny-by-default capability（仅五 command + 四 event）；fp-v1；错误 envelope `{ok:false, command, error:{code,message,details?}}`。未接入真实 Agent、SQLite 事实源、附件或项目源码写入。
验证（2026-09-15，macOS aarch64，cwd `/Users/bic/Projects/agent-up`）：
- `python3 tests/m0_schema_parse.py` → 6 个 schema JSON parse 通过。Draft 2020-12 validator：N/A + reason: 环境无 jsonschema/ajv。
- `python3 tests/m0_renderer_capabilities.py` → renderer 无 fs/shell/process/network/secret 插件引用；capability 未授予 fs/shell/http/process。
- `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests` → `tests/m0_runtime.rs` 7 passed（零写入、原子临时目录、重复 request_id/旧 revision、关闭重开 load、path_outside_project、symlink、event_replay）。
- `npx tsc --noEmit` → 通过。
- `git diff --check` → 无错误。
下一步：独立 Review（不得由本 Implementation 执行体自审）。不 git commit / 不 push。


- **2026-09-15 / done**：独立 Review pass 后在 `codex/m0-03-runtime` 提交实现。未 push。

## Independent Review Checkpoint

- **2026-09-15 / review_pass**
- **结论**：pass。六条票验收与 SPEC-002/registry 的命令合同在实现中对齐；未发现未关闭 P0，也未发现会否定验收路径的 prose/代码互斥。不得由本 Review 代修或 commit。
- **审查范围**：`docs/issues/03-m0-runtime.md` 任务合同与 Implementation Checkpoint（只读）、`docs/specs/001-m0-foundation.md`、`docs/specs/002-m0-runtime-contract.md`、`docs/execution-plan.md` W0、`schemas/*`、`src-tauri/src/{lib,runtime,main}.rs`、`src-tauri/capabilities/m0-default.json`、`src-tauri/permissions/m0.toml`、`src-tauri/tauri.conf.json`、`src/App.tsx`、`tests/*`、`src-tauri/tests/m0_runtime.rs`。未改 schemas/src/src-tauri/tests/SPEC，未改写 Implementation Checkpoint。
- **独立性**：新 Review 执行体；本会话首条指令即为独立审查。未参与 M0-03 Implementation，未共享该实现上下文。身份：Review 执行体；模型：grok-4.6；时间：2026-09-15 13:14 CST。依据 `R-RR-002`，此路径可独立。
- **Diff / 证据快照**：`HEAD=70b06f4bab3b35889b3247e8a296d1a1fd2bcabd`；受审 runtime/renderer 主要为工作区未跟踪文件。审查前后未修改受审实现。

### P0

无未关闭 P0。不得用「测试绿了」代替 SPEC/票对照；下列验收均有文件:行号与本 Review 复跑命令。

### 逐项验收

| 必须核对 | 判断 | 证据 |
| --- | --- | --- |
| 1. 五 command 与 registry/SPEC-002 输入输出副作用错误权限一致；错误 envelope `{ok,command,error}` | 成立 | 注册与 handler 仅五命令：`schemas/command-registry.json:4-147`、`src-tauri/src/lib.rs:17-107`、`src-tauri/src/runtime.rs:10-16`。成功 `ok/command/data`：`runtime.rs:704-706`；错误顶层仅 `ok/command/error`，details 白名单三键：`runtime.rs:708-729`，对照 `schemas/command-error.schema.json:6-24,83-119`。`revision_conflict` 带 `details.revision`：`runtime.rs:427-433`。副作用：scan/preview/load 无写入；initialize/create_request 只写项目根 `.agentup/`：`runtime.rs:1012-1101`。 |
| 2. fp-v1 唯一算法，无第二套 hash | 成立 | 算法在 `runtime.rs:933-960`（相对路径 UTF-8 排序、regular file `rel\0len\0sha256hex\n`、symlink `rel\0SYMLINK\0target\n`、拼接后再 SHA-256，输出 `fp-v1:`+16 小写 hex）。全文件无 md5/blake/sha1；`sha256_hex` 仅此指纹与稳定 `project_id` 派生，不是第二套 root fingerprint。形状 `^fp-v1:[0-9a-f]{16}$`：`runtime.rs:1301-1305`。 |
| 3. 预览零写入；无 token / fingerprint 变化拒绝 initialize | 成立 | preview 只写内存 token：`runtime.rs:182-193`。测试断言预览前后 listing 不变：`src-tauri/tests/m0_runtime.rs:89-113`。空 token → `confirmation_required`（`runtime.rs:222-228`）；未知 token → `confirmation_expired`（`250-256`）；fingerprint 变化消费 token 并拒绝（`282-291`）。 |
| 4. 原子临时目录 `.agentup.tmp.*` rename；失败保留旧状态 | 成立 | `commit_initialize`：sibling `.agentup.tmp.<random>`，写 manifest/事件并 flush/fsync 后单次 `rename` 为 `.agentup/`：`runtime.rs:1012-1065`。目标已存在 → `already_initialized` 且不覆盖：`242-248,1017-1021,1053-1058`；测试保留旧 manifest 且无残留 tmp：`src-tauri/tests/m0_runtime.rs:116-142`。未确认初始化不创建 `.agentup`：`tests/m0_runtime.rs:101-113`。 |
| 5. create_request draft；重复 id / 旧 revision 不二次变更 | 成立 | lifecycle 固定/强制 `draft`：`runtime.rs:454-465,495-498`。重复 `expected_revision=0` → `already_exists`：`419-425`。旧 revision → `revision_conflict` + `details.revision`：`427-433`。测试目录 listing 不变：`src-tauri/tests/m0_runtime.rs:145-179`。 |
| 6. load_project 不依赖 SQLite/内存 | 成立 | `runtime.rs` 无 sqlite。load 从 `.agentup/manifest.json`、`events/`、`facts/requests/` 读取：`536-646,1168-1284`。测试 `drop(runtime)` 后新 `AppRuntime` load 恢复 draft：`src-tauri/tests/m0_runtime.rs:181-206`。 |
| 7. 路径逃逸、symlink 穿越、event_replay 稳定错误码，不回显根外路径 | 成立 | 绑定路径不一致 → `path_outside_project`：`runtime.rs:649-656`。walk 遇逃逸 symlink 拒绝且不跟随：`865-871,906-915`。`.agentup` 自身为 symlink 拒绝：`784-797`。重复 event id → `event_replay`：`1192-1230`。错误 message 含 `/` `\\` 时替换为安全句：`732-738`。测试：`src-tauri/tests/m0_runtime.rs:208-270`。 |
| 8. renderer 与 capability：无 fs/shell/process/network/secret；只允许五 command | 成立 | renderer 仅 `@tauri-apps/api` 的 `invoke`/`listen`：`src/App.tsx:1-3,58-60`；命令名白名单五行：`src/App.tsx:5-10,82-136`。capability 仅 event listen/unlisten + `allow-m0-commands`：`src-tauri/capabilities/m0-default.json:1-12`；`src-tauri/permissions/m0.toml:1-12`。生成 ACL 与之一致：`src-tauri/gen/schemas/capabilities.json`。静态检查通过。 |

### 并列缺口（residual，不构成本轮 fail）

- `commit_initialize` 在文件写入成功后若 `fsync_dir` 失败（`runtime.rs:1051-1052`）会 `?` 返回且不走 `cleanup`；SPEC 写「任一步失败时删除临时目录」。本机 cargo 测试中 initialize 成功，说明该路径未命中；属罕见 I/O 清理缺口，不是验收路径互斥。
- `AppError::from_io` 把 `NotFound` 映射为 `path_not_found`（`runtime.rs:689-696`）。`create_request` 的 registry/SPEC 错误表不含该码；仅 TOCTOU/竞态才可能冒出。不是五命令主路径形状互斥。
- `persist_request` 是逐文件 `write_json_atomic` + 失败删 fact（`runtime.rs:1096-1100`），不是 `.agentup.tmp.*` 目录 rename。票第 3 条的临时目录合同针对 initialize；create_request 的「原子操作」按文件 CAS 实现。
- `bind_existing` 在未知 `project_id` 时直接绑定调用方路径（`runtime.rs:657-661`）。已扫描的 id 换根会 `path_outside_project`（测试覆盖）；未先 scan 的伪造 id 不是本票六条验收的阻塞项。
- `initialize_uses_atomic_temp_directory_and_keeps_old_state_on_failure` 覆盖的是 `already_initialized` 保留旧 manifest，而不是注入半途 rename 失败。实现代码仍有 tmp+rename。

### 验证命令与环境

- 仓库：`/Users/bic/Projects/agent-up`；日期 2026-09-15；macOS arm64；cwd 同上。`python3` 3.14.6；`rustc` 1.94.0。
- `python3 tests/m0_schema_parse.py` → pass：6 个 schema JSON parse。
- `python3 tests/m0_renderer_capabilities.py` → pass：scanned 3 renderer files。
- `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests` → pass：`m0_runtime.rs` 7 passed；lib/main unit tests 0。
- Draft 2020-12 validator：**N/A + reason**：环境无 jsonschema/ajv；JSON.parse 不能证明 schema 语义。不因此 fail。
- `npx tsc --noEmit`：**N/A + reason**：本 Review 合同指定的复跑命令不含 tsc；未作为阻塞项运行。Implementation Checkpoint 声称通过，本 Review 不以该声明代替复跑。
- `git diff --check`：已跟踪文件无 diff/无空白错误。受审实现多为未跟踪文件。

### 后续

移交 Commit。本 pass 只关闭 M0-03 Independent Review 门禁；不 git commit / 不 push。并列缺口不阻塞本票 Commit，除非 Commit 执行体发现实现在审查之后被改动。
