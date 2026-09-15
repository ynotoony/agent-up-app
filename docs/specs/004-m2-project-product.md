<!-- Input: EXEC-001 W2、SPEC-002/003、用户默认批准。 -->
<!-- Output: 项目作为产品对象：登记、失效、重绑定、删除 .agentup 备份。 -->
<!-- Pos: M2 项目产品合同；approved（用户默认批准 EXEC-001 默认建议）。 -->

# SPEC-004 — M2 项目产品合同

**ApprovalState:** approved

**依据**：EXEC-001 W2；用户 2026-09-15「默认批准。我只看最后结果。」

## 目标

用户可以把多个项目根目录记在 App 里，目录消失时得到可读错误和下一步，删除只动 `.agentup/` 并可备份，不删项目源码。跨根目录写入仍禁止。

## 登记

App 私有索引（应用数据目录内的 sqlite/json，**不是**项目事实源）保存：`project_id`、canonical `project_path`、`registered_at`、`last_seen_at`、`path_state`（`ok|missing|not_dir|unreadable`）。

新 command（typed，错误 envelope 仍 `{ok,command,error}`）：

| Command | 副作用 | 要点 |
| --- | --- | --- |
| `list_projects` | 无 | 返回登记项；对每项做 canonicalize，更新 `path_state`；不写项目目录 |
| `register_project` | 只写 App 索引 | 输入已 scan 的 `project_id`+canonical path；拒绝根外/文件 |
| `rebind_project` | 只写 App 索引 | 旧 path missing 时绑到新 canonical 目录；校验 project_id 与 manifest 一致 |
| `preview_remove_agentup` | 无 | 只读校验后签发一次性 confirmation_token；列出将备份/删除的 `.agentup` |
| `remove_agentup` | 写项目根 `.agentup/` | 必须 confirmation_token；先把 `.agentup/` 复制到 sibling `.agentup.backup.<utc>`，再删除 `.agentup/`；**不得**删其他文件 |

`scan_project` 成功后可登记。renderer 仍无 fs/shell。新 command 加入 allowlist capability。

## 删除与备份

- 备份目录不得覆盖已存在路径，否则 `already_exists`。
- 删除失败必须保留备份并返回 `io_error`，不得留下半删 `.agentup/` 又无备份。
- 项目源码、`.git/`、用户文件禁止触碰。

## 验收

1. 两个项目登记后 `list_projects` 隔离。
2. 拔掉目录 → `path_state=missing`，load/scan 返回 `path_not_found`，不写盘。
3. rebind 到新位置后 load 恢复同一 `project_id`。
4. `remove_agentup` 后源码仍在，`.agentup/` 不在，backup 存在且含 manifest。
5. 无 token 拒绝删除。

## 范围外

多用户、远程同步、Linux 打包、Agent。
