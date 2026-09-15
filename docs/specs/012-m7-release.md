<!-- Input: EXEC-001 W7.5、用户默认批准。 -->
<!-- Output: 安装≠事实、升级失败不丢事实、备份恢复检查、未签名不得称稳定版。 -->
<!-- Pos: M7.5 发布门合同；approved。 -->

# SPEC-012 — M7.5 发布门

**ApprovalState:** approved

**依据**：EXEC-001 W7.5；用户默认批准。

## 目标

发布前必须证明：App 数据目录不是事实源；删 SQLite / 模拟升级失败后仍能从 `.agentup/` 读回；可导出事实并做恢复检查；诊断已脱敏；无自动更新。未签名不得称稳定版。本切片不强制产出已签名安装包。

## Commands

错误 envelope `{ok,command,error}`。

| Command | 副作用 | 要点 |
| --- | --- | --- |
| `export_facts` | 写 `.agentup/backups/<backup_id>/` | 输入 `project_id`。复制 `manifest.json`、`facts/`、`events/`、`attachments/`（若有）。不复制 SQLite。 |
| `verify_restore` | 可删投影 SQLite；不改事实文件 | 输入 `project_id`。删除 `cache.sqlite`（若存在）后从事实目录 load。输出 fact/event 计数。失败不得改写 facts。 |

`export_diagnostics` 沿用 SPEC-011。

## 规则

- App `app_data_dir` 被清空 ≠ 删除项目 `.agentup/`。
- `format_version` 当前为 1；load 不得改写 manifest。更高 version 本切片不生产。
- 签名方案见 `docs/signing.md`：无开发者证书的构建是 preview，不是稳定版。
- 仍无 updater。

## 验收

1. 清空 runtime app_data_dir 后 `load_project` 仍从项目 `.agentup/` 恢复。
2. 损坏或删除 SQLite 后 `verify_restore` / `load_project` 成功，facts 文件字节不变。
3. `export_facts` 备份不含 sqlite；目录可单独保存。
4. 诊断包仍无绝对路径/密钥/附件字节。
5. 无 updater；`docs/signing.md` 声明未签名 ≠ 稳定版。

## 范围外

真实代码签名、Windows 安装包出包、Linux、静默更新。
