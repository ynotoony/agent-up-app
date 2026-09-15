<!-- Input: C2/C3 运行事实、docs/development-process.md §12.4 字段。 -->
<!-- Output: 运行记录目录导航；每个 run 一个 JSON 文件。 -->
<!-- Pos: Agent 运行记录索引；成员变化时同步 artifacts.yaml 与所属 README。 -->

# 运行记录

本目录保存 C2/C3、跨会话交接和中断恢复的 run record。文件名为 `<YYYY-MM-DD>-<hex>.json`。字段权威是 [`../../development-process.md`](../../development-process.md) §12.4，不在本目录另写 schema。

run JSON 只追加或按状态机更新；本 README 不展开每一条运行记录。

## 目录清单

| 名字 | 地位 | 功能 |
| --- | --- | --- |
| `README.md` | 索引 | 说明 run record 职责与字段权威。 |
