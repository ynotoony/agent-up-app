<!-- Input: EXEC-001 W7/W7.5、用户默认批准。 -->
<!-- Output: 三场景 E2E、打包、诊断脱敏、无静默更新。 -->
<!-- Pos: M7 产品化合同；approved。 -->

# SPEC-011 — M7 三场景与发布门

**ApprovalState:** approved

**依据**：EXEC-001 W7 / W7.5；用户默认批准。

## 目标

本机可走通 S1–S3 的 command 路径；macOS 可打包；无静默自动更新；诊断包脱敏。Linux 不进首版。Windows 安装包与签名方案记录但不在本切片强制出包。

## Commands

| Command | 副作用 | 要点 |
| --- | --- | --- |
| `export_diagnostics` | 写 `.agentup/diagnostics/<id>.json` | 输入 `project_id`。输出脱敏包：无文件正文、无绝对路径、无密钥。事实只保留 id/type/revision 与已消毒的 content（路径改成相对；匹配 `(?i)(api_key|secret|token|password)` 的字段改为 `[redacted]`）。 |

错误 envelope `{ok,command,error}`。renderer 仍无 fs/shell/network/secret。

## 打包 / 更新

- `bundle.active=true`。不得加入 updater / 静默自动更新。
- 更新：`docs/manual-update.md` 手动下载。卸载 App 不删除各项目 `.agentup/`。
- 隐私：`docs/privacy.md`。诊断默认不上送。

## 场景（command E2E，网络断开）

公共失败：未确认不写盘；错误不回显根外路径；杀 Runtime 后从 `.agentup/` 恢复。

**S1 界面改造**：init → draft 需求 → 讨论+附件 → 范围确认 → fake Implement 写范围内文件 → Review fail 不能 commit → Review pass → commit_changes → publish/accept → 反馈开新一轮。

**S2 Bug 修复**：init 含可复现文件 → 范围最小 → Implement 只改范围内 → 越权写失败 → 结果可 load。

**S3 模糊评估**：无 included 范围 → 禁止写源码 → 只发布评估 result → 接受。

## 验收

1. S1–S3 三条测试全绿。
2. `export_diagnostics` 无绝对路径、无密钥、无附件字节。
3. 仓库无 updater 依赖；`bundle.active` 为 true。
4. `properties/` 不在构建输入（已 gitignore）。
5. renderer 主路径控件可键盘聚焦（button/input 无正 tabindex 陷阱）。

## 范围外

真实模型、Linux 包、代码签名稳定版宣称、真实关 App 的 OS 级测试。
