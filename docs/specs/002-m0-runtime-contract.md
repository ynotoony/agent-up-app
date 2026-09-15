<!-- Input: docs/specs/001-m0-foundation.md, M0-02 task contract, and the two M0 JSON Schemas. -->
<!-- Output: M0 runtime command/event, capability, and threat-model contract. -->
<!-- Pos: M0 runtime boundary; approved contract for M0 command/event/capability/threat-model implementation. -->

# SPEC-002 — M0 Runtime 合同

**ApprovalState:** approved

**依据**：`SPEC-001` 已批准；M0-02 独立 Review 于 2026-09-15 pass。用户在被告知「下一步只剩批准 SPEC-002」后于 2026-09-15 回复「继续」，作为可追溯的批准指令。本合同驱动后续 M0 runtime 实现。

## 目标与边界

M0 runtime 只负责选择项目目录、读取目录、预览初始化、在用户确认后初始化 `.agentup/`、创建需求事实，以及从事实恢复项目。所有 command 都通过 Tauri typed invoke 调用；renderer 不直接访问文件系统、shell、进程、网络或秘密。

本合同中的 capabilities 是**权限设计草案**，不是已生成的 Tauri capability 文件。默认拒绝；未列出的 capability 不存在。M0 不实现真实 Agent、shell、项目源文件写入、SQLite projection、附件或网络 provider。

## 通用输入与结果

机器合同文件：事实记录使用 [`schemas/fact-record.schema.json`](../../schemas/fact-record.schema.json)，事件使用 [`schemas/event.schema.json`](../../schemas/event.schema.json)，command 注册表使用 [`schemas/command-registry.schema.json`](../../schemas/command-registry.schema.json) 和其实例 [`schemas/command-registry.json`](../../schemas/command-registry.json)，成功和失败结果分别使用 [`schemas/command-result.schema.json`](../../schemas/command-result.schema.json) 与 [`schemas/command-error.schema.json`](../../schemas/command-error.schema.json)。Prose 表格必须与这些 schema 同步；实现不得只依赖 prose。

`SPEC-001` 的用户事实类型清单保持不变；本合同中的 `project` 与 `manifest` 分支只表达 M0 初始化所需的 runtime 记录形状，其中 manifest 是初始化事实文件的 envelope，不代表新增用户领域对象。

`schema_version: 1` 的 request 事实 `content.lifecycle` 只能是 `draft`。其他 `RequestLifecycle` 值不属于本 schema 版本；引入它们必须批准新规格并升高 `schema_version`。`discussion` / `plan` / `task` / `scope` / `run` / `decision` / `result` / `attachment` 在本版本只约束 envelope，不校验 type-specific content；M0 command 不得创建这些类型。

### 路径与身份

- `project_path` 是用户选择的目录路径；runtime 必须先 canonicalize，再确认它是目录。
- runtime 为项目分配稳定 `project_id`，并将项目根绑定到本次 command context。调用者提供的路径必须始终落在该根目录内。
- 返回给 renderer 的项目文件路径使用相对项目根的路径；错误不得回显项目根外路径或文件内容。
- `confirmation_token` 是由 `preview_initialize` 生成、绑定到 `project_id`、根目录 fingerprint 和预览内容的一次性不透明值。它只允许 `initialize_project` 使用，不是通用权限令牌。

### `root_fingerprint`（`fp-v1`）

runtime 对 canonical project root 递归收集 regular files，按 UTF-8 相对路径的字节序排序；排除 `.agentup/` 和 `.git/` 目录，保留其他目录和文件名。每个文件记录为 `<relative-path>\0<byte-length>\0<SHA-256(file-bytes)>\n`，其中 relative path 使用 `/` 分隔且不以 `/` 开头。符号链接不跟随：记录为 `<relative-path>\0SYMLINK\0<readlink-target>\n`；若解析过程中试图穿越符号链接，command 失败并返回 `path_outside_project`。将所有记录按上述顺序拼接后取 SHA-256，输出 `fp-v1:` 加前 16 个小写十六进制字符。该 fingerprint 是并发变化检测值，不是安全签名。

### Manifest revision 来源

初始化创建的 manifest 是 `type: manifest` 的事实记录，`content.revision` 固定为 `1`，来源是“首次成功初始化提交”，不是文件 mtime、事件数量或 renderer 输入。manifest envelope 的 `revision` 同样为 `1`；`project.initialized` 的 `aggregate_revision` 和 payload `manifest_revision` 必须等于该 manifest revision。未来 manifest 更新必须通过批准规格定义新的 CAS revision，不能复用 M0 的初始化值。

### 成功与错误

每个 command 返回 typed success 或 typed error，不以异常文本作为协议。成功结果的顶层字段仅为 `ok: true`、`command` 和 command-specific `data`。错误结果的顶层字段仅为 `ok: false`、`command` 和 `error` 对象，与成功结果对称。`error` 必填 `error.code` 与 `error.message`，可选 `error.details`。`error.details` 若存在，只能包含 `relative_path`、`revision`、`field` 三个可选键，且至少一项；禁止绝对路径、`..` 段、反斜杠、项目根外路径、文件内容和任意附加键。`revision_conflict` 必须在 `error.details.revision` 报告当前事实 revision。

公共错误码：`invalid_input`、`path_not_found`、`path_outside_project`、`permission_denied`、`not_initialized`、`already_initialized`、`confirmation_required`、`confirmation_expired`、`revision_conflict`、`already_exists`、`malformed_fact`、`malformed_event`、`event_replay`、`io_error`。

## Command API

### `scan_project`

| 项目 | 合同 |
| --- | --- |
| 输入 | `project_path`；不接受项目根以外的路径或任意 shell 参数。 |
| 输出 | `project_id`、`project_name`、`project_type`、`documents`（相对路径及识别出的文档类型）、`agentup_state`（`absent` / `present` / `invalid`）、`root_fingerprint`。 |
| 副作用 | 无磁盘写入、无进程启动、无网络；可发出 `project.scan_completed` runtime notification。 |
| 错误 | `invalid_input`、`path_not_found`、`permission_denied`、`path_outside_project`、`io_error`。 |
| 最小权限 | 只读访问用户选择的项目根及其受限后代；不授予写、shell、进程、网络或秘密读取权限。 |

扫描只返回完成识别所需的元数据和相对路径，不把整个项目文件内容发送到 renderer。`root_fingerprint` 必须使用本合同 `fp-v1` 算法；不得另写实现规格或第二套 hash。该值只做并发变化检测，不是安全签名。

### `preview_initialize`

| 项目 | 合同 |
| --- | --- |
| 输入 | `project_id`、`project_path`、`expected_root_fingerprint`。路径必须与扫描绑定的项目根一致。 |
| 输出 | `project_id`、`planned_paths`（最小初始化路径清单）、`root_fingerprint`、`confirmation_token`、`requires_confirmation: true`。 |
| 副作用 | 无磁盘写入；confirmation token 可保存在 runtime 的短期内存中，不能成为持久事实。 |
| 错误 | `invalid_input`、`path_not_found`、`permission_denied`、`path_outside_project`、`already_initialized`、`io_error`。 |
| 最小权限 | 只读访问项目根；不授予写、shell、进程、网络或秘密读取权限。 |

预览结果必须明确列出将创建的 `.agentup/manifest.json`、事件目录和初始化事件目标；预览前后项目目录不得新增文件。

### `initialize_project`

| 项目 | 合同 |
| --- | --- |
| 输入 | `project_id`、`project_path`、`confirmation_token`、`expected_root_fingerprint`。token 必须一次性、未过期、绑定同一项目和同一预览 fingerprint。 |
| 输出 | `project_id`、manifest 摘要、`initialized_at`、`initialization_event_id`。不返回项目文件内容。 |
| 副作用 | 仅在项目根下创建最小 `.agentup/manifest.json`、事件目录和 `project.initialized` 事件；使用临时文件和原子替换；不得写项目源文件。 |
| 错误 | `invalid_input`、`path_not_found`、`confirmation_required`、`confirmation_expired`、`path_outside_project`、`already_initialized`、`permission_denied`、`io_error`、`malformed_event`。 |
| 最小权限 | 仅允许在绑定项目根的 `.agentup/` 后代创建或更新初始化文件；拒绝符号链接穿越、shell、进程、网络和秘密读取。 |

输入缺字段或类型错误返回 `invalid_input`；绑定路径不存在返回 `path_not_found`。若 fingerprint 在预览后改变，runtime 必须拒绝初始化并返回 `confirmation_expired`，要求重新扫描和预览。初始化使用项目根下的 sibling 临时目录 `.agentup.tmp.<random>`：先在临时目录写 manifest 和事件文件，逐文件 flush，并尽可能 fsync 文件与临时目录；全部校验成功后以单次 rename 将临时目录提交为 `.agentup/`。目标已存在时拒绝并返回 `already_initialized`，不覆盖任何现有文件。任一步失败时删除临时目录；若清理失败仍保留旧状态并返回 `io_error`，错误只回显安全错误码，不回显完整临时路径。

### `create_request`

| 项目 | 合同 |
| --- | --- |
| 输入 | `project_id`、`request_id`、`content`、`metadata`、`expected_revision`。M0 新建需求要求 `expected_revision: 0`，并将 RequestLifecycle 固定为 `draft`；content 和 metadata 必须为对象。 |
| 输出 | 新建 fact record（`type: request`、`revision: 1`、`source: user`）、`request.created` 事件及其 ID。 |
| 副作用 | 只写项目根 `.agentup/` 内的需求事实和不可变事件；以 compare-and-swap 校验 revision；写事实与事件时使用原子操作。 |
| 错误 | `invalid_input`、`not_initialized`、`path_outside_project`、`already_exists`、`revision_conflict`、`malformed_fact`、`malformed_event`、`permission_denied`、`io_error`。 |
| 最小权限 | 只读写绑定项目根 `.agentup/` 后代；不允许写项目源文件、shell、进程、网络或秘密。 |

相同 `request_id` 的第二次新建必须返回 `already_exists`，不能产生第二个状态变化或第二个同义事实。后续 revision 更新不属于 M0 command API。

### `load_project`

| 项目 | 合同 |
| --- | --- |
| 输入 | `project_path`，可选 `project_id` 用于校验路径绑定。 |
| 输出 | `manifest`、按 revision 排序的 `facts`、可重放的持久 `events`、当前需求摘要和 `project_id`。不存在 SQLite 时结果必须相同。可发出 `request.rehydrated` runtime notification。 |
| 副作用 | 无磁盘写入、无事件追加、无进程启动、无网络；读取失败不得修复或覆盖事实。 |
| 错误 | `invalid_input`、`path_not_found`、`path_outside_project`、`not_initialized`、`malformed_fact`、`malformed_event`、`event_replay`、`permission_denied`、`io_error`。 |
| 最小权限 | 只读访问绑定项目根 `.agentup/` 后代；不授予写、shell、进程、网络或秘密读取权限。 |

加载以 `.agentup/` 为事实源；SQLite（如未来存在）只能作为派生投影，损坏或缺失不得阻止事实恢复。

## Event API

所有事件使用 [`schemas/event.schema.json`](../../schemas/event.schema.json) 的 Draft 2020-12 envelope。`event_id` 全局唯一；重复 ID 必须被识别为 `event_replay`，不能再次改变状态。schema 的 `oneOf` 同时约束 event_type、aggregate_type、source、delivery 和 payload shape；实现不得接受未列出的组合。`aggregate_revision` 为正整数；对持久事件，它必须与关联事实的 revision 保持一致；只读 notification 使用本次读取观察到的 aggregate revision，不产生新的事实 revision。事件 payload 只能包含本事件所需的结构化对象。

| Event | aggregate | 产生时机 | 持久性与副作用 |
| --- | --- | --- | --- |
| `project.scan_completed` | `project` | `scan_project` 成功 | runtime notification；不写磁盘、不产生事实。若项目尚未初始化，使用观察版本 `1`；该 notification 不进入持久事件流。 |
| `project.initialized` | `project` | `initialize_project` 成功 | 持久追加；`aggregate_revision = payload.manifest_revision = manifest.content.revision = 1`。 |
| `request.created` | `request` | `create_request` 成功 | 持久追加；与 request fact revision `1` 一致。 |
| `request.rehydrated` | `request` | `load_project` 从事实恢复需求后 | runtime notification；使用恢复出的 request revision；不写磁盘、不产生新的事实 revision。 |

只读 notification 仍使用不可变的 event envelope 传递，消费端不得把它们当作持久事实。持久事件重放必须先检查事件 ID、aggregate revision 和事件类型，发现重复或跳号即停止恢复并返回可读错误。

## Capabilities 清单

下表是未来 Tauri capabilities 的最小逻辑清单，默认 deny。真实 Tauri capability 名称和配置文件属于后续 runtime 实现票；本文件不声称它们已存在。

| 主体/操作 | 允许 | 明确拒绝 |
| --- | --- | --- |
| renderer | 调用五个 allowlisted typed commands；接收四类 runtime event | filesystem、shell、process、network、secret、任意路径访问和任意 Tauri command。 |
| `scan_project` | 绑定项目根内的目录元数据只读 | 写入、执行、网络、读取根外路径和读取秘密文件内容。 |
| `preview_initialize` | 绑定项目根内的预览所需元数据只读；生成短期 token | 任何磁盘写入、执行、网络和持久 token 存储。 |
| `initialize_project` | 绑定项目根下 `.agentup/` 的最小创建/原子替换 | 项目源文件写入、根外路径、符号链接穿越、shell、进程、网络和秘密。 |
| `create_request` | 绑定项目根下 `.agentup/` 的事实与事件写入 | `.agentup/` 外写入、源文件写入、shell、进程、网络和秘密。 |
| `load_project` | 绑定项目根下 `.agentup/` 的事实只读 | 修复性写入、事件追加、shell、进程、网络和秘密。 |

路径授权必须以 runtime canonical path 为准，而不是 renderer 提供的字符串前缀；`.agentup/` 必须是项目根的直接子目录。capability 不因 command 参数而扩大，command 也不能把 renderer 输入转成 shell 命令。

## 威胁模型

M0 的攻击面是 renderer 输入、项目根目录及其文件系统边界、事实/事件文件和 Tauri invoke 通道。以下控制在 runtime 实现前必须保持为验收条件。

| 威胁 | 例子与影响 | 必须控制 | 检测与恢复 |
| --- | --- | --- | --- |
| **路径逃逸** | `..`、绝对路径、编码变体或符号链接把读写带出项目根，导致越权读取或写入。 | canonicalize 后做祖先关系检查；拒绝根外路径和符号链接穿越；只允许项目根下 `.agentup/` 写入。 | 记录安全错误但不回显敏感路径；返回 `path_outside_project`，保留旧事实。 |
| **未确认写入** | 恶意或误操作直接调用初始化，未经过用户看到的预览。 | `initialize_project` 强制一次性 confirmation token；token 绑定项目、fingerprint 和预览；变化后失效。 | 返回 `confirmation_required`/`confirmation_expired`；初始化前后断言项目目录无意外新增文件。 |
| **revision 冲突** | 两个窗口或重试调用基于旧 revision 覆盖新事实，造成事实丢失。 | 新建要求 revision `0`，更新使用 compare-and-swap；旧 revision 永不覆盖；原子替换。 | 返回 `revision_conflict`，并在 `error.details.revision` 报告当前 revision，要求重新 load；不产生事件。 |
| **事件重放** | 重试或恶意注入相同 event ID，重复创建需求或重复推进状态。 | 事件 ID 唯一性、aggregate revision 校验、允许事件类型白名单；重复 ID 不得二次应用。 | 返回 `event_replay` 或 `malformed_event`；恢复停止在可识别边界，不删除历史。 |
| **敏感数据泄露** | scan、错误、日志或事件 payload 暴露秘密文件、绝对路径、环境变量或项目内容。 | 只返回相对路径和必要元数据；默认拒绝秘密读取、网络和 shell；错误与日志做内容脱敏。 | 静态检查返回字段；发现泄露时撤销输出、保留事实并记录安全事件。 |

额外的完整性要求：事实和持久事件必须通过 schema 校验；写入失败必须原子回退；load 遇到无法验证的事实或事件时返回错误，不自动“修复”或静默跳过。

M0 限制：一次性 confirmation token 能证明预览 fingerprint 与初始化请求绑定，但不能在受攻陷的 renderer 中证明用户确实看过预览。后续涉及更高权限的 runtime 实现应增加原生确认或等价的受信用户手势；M0 不扩大权限来解决该问题。

## 验收与实现前置

实现 M0 runtime 前必须证明：两个 schema 可由 Draft 2020-12 validator 解析；五个 command 的输入、输出、副作用、错误和权限与本合同一致；五类威胁均有测试；renderer 没有 filesystem/shell capability；初始化前零写入、旧 revision 拒绝、重复事件拒绝和关闭重开恢复均有外部行为测试。

本规格为 `approved`。后续任务按本合同生成 Tauri capabilities 与 runtime 实现；本文件仍不包含那些实现。
