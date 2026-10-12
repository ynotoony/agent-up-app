// Input: One registered project, file-backed goals, receipts and legacy requirement rows.
// Output: Read-only project history; the primary action is returning to real task tickets.
// Pos: /project/:id/records; task creation and Agent dispatch remain on the workbench.
import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { ArrowLeft, CheckCircle2, CircleHelp, Clock3, GitCommitHorizontal, Loader2 } from 'lucide-react';
import type { NativeDelivery, NativeGoal, Project, Requirement } from '../../shared/types';
import { api, errorMessage } from '@/lib/api';

type RecordGroup = { goal: NativeGoal; deliveries: NativeDelivery[] };
export default function ProjectRecordsPage() {
  const { id = '' } = useParams();
  const [project, setProject] = useState<Project | null>(null);
  const [groups, setGroups] = useState<RecordGroup[]>([]);
  const [legacyRequirements, setLegacyRequirements] = useState<Requirement[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  useEffect(() => {
    let active = true;
    Promise.all([api.projects.get(id), api.goals.list(id)]).then(async ([dashboard, goals]) => {
      const items = await Promise.all(goals.map(async goal => ({ goal, deliveries: await api.deliveries.list(id, goal.id).catch(() => []) })));
      if (active) {
        setProject(dashboard.project);
        setLegacyRequirements(dashboard.requirements);
        setGroups(items.filter(item => item.goal.status === 'completed' || item.deliveries.length > 0 || item.goal.status === 'failed'));
      }
    }).catch(failure => { if (active) setError(errorMessage(failure)); }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [id]);
  return <main id="main-content" tabIndex={-1} className="workbench-page">
    <header className="workbench-header"><div><div className="page-eyebrow">PROJECT / HISTORY</div><h1>{project?.name || '项目记录'}</h1><p className="page-subtitle">历史需求、目标和交付凭证</p></div><Link className="subtle-link" to={'/project/' + id}><ArrowLeft size={15} aria-hidden="true" />返回任务看板</Link></header>
    {error && <div role="alert" className="source-notice"><CircleHelp size={16} aria-hidden="true" /><p>{error}</p></div>}
    {loading ? <div className="board-loading" role="status"><Loader2 className="animate-spin" aria-hidden="true" />正在读取项目记录…</div> : <>
      {groups.length === 0 && legacyRequirements.length === 0 && <div className="workbench-empty"><Clock3 size={28} aria-hidden="true" /><h2>暂无历史记录</h2><p>项目真实任务和当前状态显示在任务看板。</p></div>}
      {legacyRequirements.length > 0 && <section className="archive-list"><h2>旧需求记录 · {legacyRequirements.length}</h2><p className="archive-description">兼容旧流程，保留历史原文和执行记录；新任务请使用项目内票据文件。</p><ul>{legacyRequirements.map(requirement => <li key={requirement.id}><span className="ticket-id">{requirement.id.slice(0, 8)}</span><Link to={'/requirement/' + requirement.id}>{requirement.goal_summary || requirement.content_preview}</Link><span>{requirement.status}</span><time>{new Date(requirement.updated_at).toLocaleDateString('zh-CN')}</time></li>)}</ul></section>}
      {groups.length > 0 && <div className="record-timeline">{groups.map(({ goal, deliveries }) => <article className="record-group" key={goal.id}><header><div><p className="ticket-id">目标记录 · 修订版 {goal.revision}</p><h2>{goal.summary || goal.content}</h2></div><span className={'record-status ' + goal.status}>{goal.status === 'completed' ? '目标已完成' : goal.status === 'failed' ? '未完成' : goal.status}</span></header><div className="record-receipts">{deliveries.map(delivery => <div className="record-receipt" key={delivery.id}><CheckCircle2 size={15} aria-hidden="true" /><div className="min-w-0"><strong>{delivery.task_title}</strong><p>{delivery.status === 'accepted' ? '已验收并合入本地' : delivery.status === 'failed' ? '未完成，隔离目录与证据已保留' : delivery.status}</p></div>{delivery.commit && <span className="receipt-commit"><GitCommitHorizontal size={14} aria-hidden="true" />{delivery.commit.slice(0, 9)}</span>}<time>{new Date(delivery.updated_at).toLocaleString('zh-CN')}</time></div>)}</div></article>)}</div>}
    </>}
  </main>;
}
