<!-- Input: SPEC-003、M1-01。 -->
<!-- Output: SQLite 仅投影、可从事实重建的外部行为。 -->
<!-- Pos: M1-02 任务票；SQLite 不得成为事实源。 -->

# 05 — M1-02 SQLite 投影重建

**Type:** feature

**Priority:** P1

**Complexity:** C3（数据重建、不可把投影当权威）

**What to build:** 删除 SQLite 之后仍能从 `.agentup/` 重建项目、需求、讨论、任务、范围、运行、决定和结果索引，且与删除前一致。

**Blocked by:** 04 — M1-01 事实类型合同落地。

**Status:** blocked

- [ ] 投影缺失不阻止 load。
- [ ] 损坏投影不改写事实，返回可读错误或触发只读重建。
- [ ] 重建后索引与事实 revision 对齐。
- [ ] 测试删除 SQLite 再重建。

## Implementation Checkpoint

待 M1-01 done 后领取。
