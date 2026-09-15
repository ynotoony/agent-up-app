<!-- Input: 已批准 EXEC-001 W1 与 proposed SPEC-003。 -->
<!-- Output: 八类用户事实 type-specific schema 与 writer/reader 外部行为测试。 -->
<!-- Pos: M1-01 任务票；不实现 SQLite，不接 Agent。 -->

# 04 — M1-01 事实类型合同落地

**Type:** feature

**Priority:** P1

**Complexity:** C2（公共事实接口）

**What to build:** 不打开 UI，也能把讨论、计划、任务、范围、运行、决定、结果和附件元数据写成事实并读回来；旧 revision 和重复事件被拒绝。

**Blocked by:** SPEC-003 批准（当前 proposed）。M0-03 done。

**Status:** blocked

- [ ] fact-record schema 以 type-specific content 取代八类 generic object。
- [ ] 上列类型各有至少一条 fixture 写入/读回测试。
- [ ] CAS 与 event_replay 行为与 SPEC-001/003 一致。
- [ ] 不新增 renderer 系统 capability。

## Implementation Checkpoint

待 SPEC-003 批准后领取。
