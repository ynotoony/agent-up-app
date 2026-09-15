<!-- Input: 任务票、依赖边与领域语言。 -->
<!-- Output: 票层索引：切片规则、领取规则与票模板。 -->
<!-- Pos: 任务票权威层索引；一旦我被更新，务必更新我的开头注释，以及所属文件夹的 README.md。 -->
<!-- read_when: conditional-issues —— 命中触发矩阵"需要 ≥2 张票；存在 Blocked by 依赖；跨会话交接"才创建；未命中不创建（触发矩阵见 docs/development-process.md）。 -->

# 任务票（issues）

## 架构

- 一张票一个文件：`<NN>-<slug>.md`，按依赖顺序编号（blocker 在前）。
- 票是 **agent 的任务**，只活在文件里；GitHub Issues 等人向追踪器若启用，只承接人向工作项（bug 报告、外部请求、协作讨论），两边不互为副本、不双写状态。
- 领取规则：**blocker 全部 `done` 的票才可领取**（frontier）；状态流转 `ready / in_progress / blocked / review_ready / review_pass / review_fail / done` 记录在票内与 `../progress.md`。

## 切片与依赖规则

- **垂直切片**：一刀穿透所有层（数据、业务接口、页面、测试），不是单层横切；完成的票可独立演示或验证。
- **体量 = 一个新鲜上下文窗口**：装不下一张票就再拆。
- **Blocked by 依赖边**：只列真正门禁这张票的票，不列"顺手相关"。
- **宽重构例外（expand–contract）**：机械的大范围变更不硬塞垂直切片——先扩展（新旧形态并存），分批迁移（每批一票，blocked by 扩展票），最后收缩（删除旧形态，blocked by 全部迁移批）；批次无法独立保绿时共享集成分支，绿只在最终集成票承诺。

## 票模板

```markdown
# <NN> — <标题>

**What to build:** 这张票让什么端到端行为生效，从用户视角写——不是逐层实现清单。

**Blocked by:** 依赖票的编号/标题，或"无——可立即开始"。

**Status:** ready

- [ ] 验收条件 1
- [ ] 验收条件 2
```

不写具体文件路径和代码片段（它们过期最快；原型片段例外规则见 [`../specs/README.md`](../specs/README.md)）。票内 Checkpoint 区追加记录：最后检查点、已修改范围、验证命令与结果（失败也记录）、下一步。

## 目录清单

| 名字 | 地位 | 功能 |
| --- | --- | --- |
| `README.md` | 层索引 | 说明任务票规则、切片和依赖。 |
| `01-m0-foundation.md` | 任务票 | M0-01 状态与事实合同；已完成。 |
| `02-m0-runtime-contract.md` | 任务票 | M0-02 Schema、API、capabilities 和威胁模型；已完成。 |
| `03-m0-runtime.md` | 任务票 | M0-03 五命令恢复切片；done。 |
| `04-m1-fact-types.md` | 任务票 | M1-01 八类事实 schema 与读写测试；blocked on SPEC-003。 |
| `05-m1-sqlite-projection.md` | 任务票 | M1-02 SQLite 投影重建；done。 |
| `06-m2-project-product.md` | 任务票 | M2 项目产品切片；review_ready。 |

| `07-m2-external-change.md` | 任务票 | W2.5 外部变化门；in_progress。 |
| `08-m3-discussion.md` | 任务票 | M3 讨论与附件；in_progress。 |
