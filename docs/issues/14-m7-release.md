<!-- Input: approved SPEC-012。 -->
<!-- Output: 安装≠事实、升级失败回滚、备份恢复检查。 -->
<!-- Pos: M7.5 发布门切片。 -->

# 14 — M7.5 发布门

**Type:** feature

**Priority:** P1

**Complexity:** C3

**What to build:** 证明卸载 App 不删项目事实；SQLite 损坏不丢事实；可导出事实并做恢复检查；未签名不得称稳定版。

**Blocked by:** W7 done；SPEC-012 approved。

**Status:** ready

- [ ] 清空 app_data_dir 后项目 `.agentup/` 仍可 load
- [ ] 删/坏 SQLite 后 facts 不变且可恢复
- [ ] export_facts 不含 sqlite
- [ ] 诊断脱敏仍成立
- [ ] signing.md + 无 updater
