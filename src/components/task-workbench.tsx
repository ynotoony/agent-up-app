// Input: Project-backed task snapshot, URL filters and explicit user handoff.
// Output: Multi-project board/list, task inspection and a real interactive Agent entry.
// Pos: Primary product surface; existing goals and legacy records live outside this workflow.
import { useEffect, useMemo, useRef, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { ArrowDown, ArrowRight, ArrowUpRight, Check, CheckCircle2, Circle, CircleDot, CircleHelp, Clock3, Columns3, FilePlus2, FileText, FolderOpen, GitBranch, Inbox, List, Loader2, RefreshCw, Search, Sparkles, Terminal, X } from 'lucide-react';
import { api, errorMessage, type RuntimeInfo } from '@/lib/api';
import { LANES, shortDate, taskLane, taskStatus, textItems, useWorkbench, type ProjectSource, type TaskLane, type WorkTicket } from '@/lib/workbench';
import type { GovernanceTicket, Project, TicketDraft, TicketHandoff } from '../../shared/types';
import { cn } from '@/lib/utils';
import { Button } from './ui/button';

const laneIcon = { ready: Circle, active: CircleDot, attention: CircleHelp, done: CheckCircle2 };
const FORMAL_TICKET_INDEX = 'facts/requirements/tickets/index.json';
const MAX_TICKETS = 4000;
type CreatedHandoff = { key: string; receipt: TicketHandoff | null; error: string; creationWarning: string | null };

function ticketCreationReason(source?: ProjectSource): string | null {
  if (!source) return '尚未读取项目任务索引，请先同步任务。';
  if (source.error) return `项目索引读取失败：${source.error}`;
  const result = source.result;
  if (!result) return '尚未读取项目任务索引，请先同步任务。';
  if (result.index_path !== FORMAL_TICKET_INDEX) {
    return `当前使用旧索引 ${result.index_path}，仅支持读取；安全创建要求 ${FORMAL_TICKET_INDEX}。`;
  }
  if (result.total + result.skipped >= MAX_TICKETS) {
    return `正式任务索引已达到 ${MAX_TICKETS} 条上限。`;
  }
  if (!result.ticket_creation_supported) {
    return result.ticket_creation_reason || '后端尚未确认开票所需文件、脚本和 schema 均可用；当前已禁用写入。';
  }
  return null;
}

export function TaskWorkbench({ projectId, attention = false }: { projectId?: string; attention?: boolean }) {
  const { projects, tickets, sources, loading, refreshing, error, refreshedAt, refresh } = useWorkbench();
  const [params, setParams] = useSearchParams();
  const [createdHandoff, setCreatedHandoff] = useState<CreatedHandoff | null>(null);
  const query = params.get('q') ?? '';
  const view = params.get('view') === 'list' ? 'list' : 'board';
  const laneFilter = attention ? 'attention' : params.get('status') ?? 'all';
  const showArchived = params.get('archived') === '1';
  const selectedProject = projects.find(project => project.id === projectId);
  const sourceForProject = (id: string) => sources.find(source => source.project.id === id);
  const creationReasonForProject = (id: string) => ticketCreationReason(sourceForProject(id));
  const creatableProjects = projects.filter(project => creationReasonForProject(project.id) === null);
  const unavailableProjects = projects.flatMap(project => {
    const reason = creationReasonForProject(project.id);
    return reason ? [{ project, reason }] : [];
  });
  const selectedCreationReason = projectId ? creationReasonForProject(projectId) : null;
  const scoped = tickets.filter(ticket => (!projectId || ticket.project.id === projectId) && (showArchived || ticket.governance_status !== 'superseded'));
  const filtered = useMemo(() => scoped.filter(ticket => (laneFilter === 'all' || taskLane(ticket) === laneFilter) && `${ticket.title} ${ticket.summary ?? ''} ${ticket.id} ${ticket.project.name} ${textItems(ticket.body?.goal).join(' ')}`.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())), [scoped, query, laneFilter]);
  const failures = sources.filter(source => (!projectId || source.project.id === projectId) && source.error);
  const successfulSources = sources.filter(source => (!projectId || source.project.id === projectId) && source.result);
  const unknown = tickets.filter(ticket => (!projectId || ticket.project.id === projectId) && ticket.lane !== 'superseded' && taskLane(ticket) === null);
  const archived = filtered.filter(ticket => ticket.governance_status === 'superseded');
  const omissions = sources.filter(source => (!projectId || source.project.id === projectId) && (source.result?.skipped ?? 0) > 0);
  const selectedKey = params.get('task');
  const selected = tickets.find(ticket => ticket.key === selectedKey);
  const updateQuery = (key: string, value: string | null) => setParams(previous => { const next = new URLSearchParams(previous); if (value) next.set(key, value); else next.delete(key); return next; }, { replace: key === 'q' });
  const heading = attention ? '待我处理' : selectedProject?.name ?? (projectId ? '项目任务' : '工作台');

  return <main id="main-content" tabIndex={-1} className="workbench-page">
    <header className="workbench-header">
      <div className="min-w-0">
        <div className="page-eyebrow"><span className="brand-dash" />{projectId ? 'PROJECT' : attention ? 'YOUR ATTENTION' : 'YOUR WORKSPACE'}</div>
        <h1>{heading}</h1>
        <p className="page-subtitle">{projectId ? selectedProject?.path : attention ? '把注意力留给需要你判断的事。' : '所有项目，正在发生的工作。'}</p>
      </div>
      <div className="page-actions">
        <button className="sync-button" onClick={() => void refresh()} disabled={refreshing} title={refreshedAt ? `上次读取：${refreshedAt.toLocaleTimeString('zh-CN')}` : undefined}>
          <span className={cn('sync-dot', refreshing && 'animate-pulse')} />{refreshing ? '读取中…' : '同步任务'}
        </button>
        {projectId ? <>{selectedProject && <button className="action-link compact-action" disabled={Boolean(selectedCreationReason)} title={selectedCreationReason ?? undefined} aria-describedby={selectedCreationReason ? 'ticket-create-unavailable' : undefined} onClick={() => updateQuery('new', '1')}><FilePlus2 size={15} aria-hidden="true" />新建任务</button>}<Link className="subtle-link" to={`/project/${projectId}/records`}>项目记录 <ArrowUpRight size={14} aria-hidden="true" /></Link></> : <>{!attention && projects.length > 0 && <button className="action-link compact-action" disabled={creatableProjects.length === 0} title={creatableProjects.length === 0 ? '当前没有已确认支持安全开票的项目' : undefined} onClick={() => updateQuery('new', '1')}><FilePlus2 size={15} aria-hidden="true" />新建任务</button>}<Link className={projects.length && !attention ? 'subtle-link' : 'action-link'} to="/connect"><FolderOpen size={15} aria-hidden="true" />接入项目</Link></>}
      </div>
    </header>

    {projectId && selectedCreationReason && selectedProject && <div id="ticket-create-unavailable" role="status" className="creation-notice"><CircleHelp size={16} aria-hidden="true" /><div><strong>此项目暂不支持安全创建任务</strong><p>{selectedCreationReason}</p></div><button type="button" className="creation-refresh" disabled={refreshing} onClick={() => void refresh()}><RefreshCw size={13} aria-hidden="true" />重新检查</button></div>}
    {!projectId && !attention && projects.length > 0 && creatableProjects.length === 0 && <div role="status" className="creation-notice"><CircleHelp size={16} aria-hidden="true" /><div><strong>当前没有已确认支持安全创建任务的项目</strong><ul>{unavailableProjects.map(({ project, reason }) => <li key={project.id}><span>{project.name}</span><span>{reason}</span></li>)}</ul></div><button type="button" className="creation-refresh" disabled={refreshing} onClick={() => void refresh()}><RefreshCw size={13} aria-hidden="true" />重新检查</button></div>}

    {error && <div role="alert" className="source-notice"><CircleHelp size={17} aria-hidden="true" /><div><strong>无法读取项目</strong><p>{error}</p></div><Button variant="secondary" onClick={() => void refresh()}>重试</Button></div>}
    {!loading && projectId && !selectedProject && !error && <div className="workbench-empty"><FolderOpen size={30} /><h2>项目不在工作区中</h2><p>它可能已被移除。重新接入目录即可读取项目任务。</p><Link className="action-link" to="/connect">接入项目</Link></div>}
    {loading ? <div className="board-loading" role="status"><Loader2 size={20} className="animate-spin" aria-hidden="true" />正在读取项目里的真实任务…</div> : !error && (!projectId || selectedProject) && <>
      {failures.length > 0 && successfulSources.length > 0 && <p className="partial-source-note" role="status">部分项目未能读取；以下统计仅覆盖成功读取的 {successfulSources.length} / {sources.filter(source => !projectId || source.project.id === projectId).length} 个项目。</p>}
      {unknown.length > 0 && <div className="source-notice" role="status"><CircleHelp size={17} aria-hidden="true" /><div><strong>{unknown.length} 项任务状态无法识别</strong><p>不支持的状态不会计入待处理或已完成。任务仍保留在“未识别状态”中，可打开原始记录核实状态值。</p></div></div>}
      <section className="work-overview" aria-label="任务状态概览">
        {LANES.map(lane => {
          const Icon = laneIcon[lane.id];
          const count = scoped.filter(ticket => taskLane(ticket) === lane.id).length;
          return <button key={lane.id} onClick={() => !attention && updateQuery('status', laneFilter === lane.id ? null : lane.id)} aria-pressed={laneFilter === lane.id} disabled={attention} className={cn('overview-stat', `tone-${lane.id}`, laneFilter === lane.id && 'is-filtered')}>
            <span className="stat-label"><Icon size={15} aria-hidden="true" />{lane.label}</span><span className="stat-number">{String(count).padStart(2, '0')}</span>
            <span className="stat-caption">{lane.hint}</span>
          </button>;
        })}
      </section>

      {failures.map(source => <div key={source.project.id} role="alert" className="source-notice"><CircleHelp size={17} aria-hidden="true" /><div><strong>{source.project.name} · 任务来源未就绪</strong><p>{source.error}</p></div><Link to={`/connect?path=${encodeURIComponent(source.project.path ?? '')}`} className="subtle-link">检查接入 <ArrowRight size={14} /></Link></div>)}
      {omissions.map(source => <div className="source-notice" role="alert" key={source.project.id}><CircleHelp size={16} /><p>{source.project.name} 的索引有 {source.result?.skipped} 条无效记录，未显示在看板中。请检查 {source.result?.index_path}。</p></div>)}

      <div className="board-toolbar">
        <div className="view-switch" aria-label="任务视图"><button onClick={() => updateQuery('view', null)} aria-pressed={view === 'board'}><Columns3 size={15} aria-hidden="true" />看板</button><button onClick={() => updateQuery('view', 'list')} aria-pressed={view === 'list'}><List size={15} aria-hidden="true" />列表</button></div>
        <span className="toolbar-count">{filtered.length} 项任务</span>
        {laneFilter !== 'all' && !attention && <button className="filter-chip" onClick={() => updateQuery('status', null)}>{LANES.find(lane => lane.id === laneFilter)?.label ?? laneFilter}<X size={12} aria-hidden="true" /><span className="sr-only">清除状态筛选</span></button>}
        <div className="toolbar-spacer" />
        <label className="archive-toggle"><input type="checkbox" checked={showArchived} onChange={event => updateQuery('archived', event.target.checked ? '1' : null)} />含归档</label>
        <label className="task-search"><Search size={15} aria-hidden="true" /><span className="sr-only">搜索任务</span><input type="search" name="task-search" autoComplete="off" placeholder="搜索任务、项目…" value={query} onChange={event => updateQuery('q', event.target.value || null)} /></label>
      </div>

      {projects.length === 0 && !error ? <div className="workbench-empty"><FolderOpen size={32} aria-hidden="true" /><h2>把第一个项目带进来</h2><p>接入本地目录，已有任务会直接出现在这里。<br />新项目也可以从一个空目录开始。</p><Link className="action-link" to="/connect">接入项目 <ArrowRight size={15} aria-hidden="true" /></Link></div>
      : projects.length === 0 && error ? null
      : scoped.length === 0 && failures.length === 0 ? <div className="workbench-empty"><Inbox size={32} aria-hidden="true" /><h2>项目已接入，等待第一项任务</h2><p>这里直接读取项目的任务文件。添加真实任务后，点击“同步任务”。</p><p className="source-path">{sources.find(source => !projectId || source.project.id === projectId)?.result?.index_path}</p></div>
      : failures.length > 0 && scoped.length === 0 ? <div className="workbench-empty compact"><CircleHelp size={28} aria-hidden="true" /><h2>{successfulSources.length === 0 ? '没有可完整读取任务索引的项目' : '当前项目任务来源不可读'}</h2><p>不能据此判断项目没有任务。检查目录和索引文件后重新同步。</p><Button variant="secondary" loading={refreshing} onClick={() => void refresh()}>重新读取</Button></div>
      : filtered.length === 0 ? <div className="workbench-empty compact"><Search size={26} aria-hidden="true" /><h2>{attention ? failures.length ? '已读取项目中没有待处理事项' : '当前没有待处理事项' : failures.length ? '已读取项目中没有匹配任务' : '没有匹配的任务'}</h2><p>{attention ? failures.length ? `部分项目不可读取，不能确认全部状态。成功读取 ${successfulSources.length} 个项目。` : '有任务等待审查、收口或解除阻塞时，会集中出现在这里。' : failures.length ? `部分项目不可读取；当前结果只覆盖已成功读取的来源，另有 ${failures.length} 个项目需检查。` : '尝试其他关键词，或清除当前筛选。'}</p>{!attention && <Button variant="secondary" onClick={() => { setParams({ view }); }}>清除筛选</Button>}</div>
      : view === 'board' ? <div className={cn('task-board', (attention || laneFilter !== 'all') && 'is-focused')}>
        {LANES.filter(lane => laneFilter === 'all' || lane.id === laneFilter).map(lane => <section key={lane.id} className={cn('task-lane', `tone-${lane.id}`)} aria-label={`${lane.label}任务`}>
          <div className="lane-header"><span className="lane-dot" /><h2>{lane.label}</h2><span className="lane-count">{filtered.filter(ticket => taskLane(ticket) === lane.id).length}</span></div>
          <div className="lane-body">{filtered.filter(ticket => taskLane(ticket) === lane.id).map(ticket => <TaskCard key={ticket.key} ticket={ticket} onOpen={() => updateQuery('task', ticket.key)} />)}{!filtered.some(ticket => taskLane(ticket) === lane.id) && <div className="lane-empty"><span />{lane.id === 'done' ? '完成的工作会留在这里' : lane.id === 'attention' ? '暂时没有需要处理的事项' : lane.id === 'active' ? '还没有进行中的任务' : '暂时没有待开始任务'}</div>}</div>
        </section>)}
      </div> : <div className="task-table"><div className="task-table-header"><span>任务</span><span>项目</span><span>状态</span><span>更新</span></div>{filtered.map(ticket => <button key={ticket.key} className="task-table-row" onClick={() => updateQuery('task', ticket.key)}><span className="table-title"><span className="ticket-id">{ticket.id}</span><strong>{ticket.title}</strong></span><span>{ticket.project.name}</span><span className={`tone-${taskLane(ticket) ?? 'unknown'}`}><span className="status-text">{taskStatus(ticket)}</span></span><span>{shortDate(ticket.updated_at)}<ArrowUpRight size={14} aria-hidden="true" /></span></button>)}</div>}
      {view === 'board' && unknown.length > 0 && <section className="exception-list"><h2>未识别状态 · {unknown.length}</h2><p>这些真实任务保留在索引中，需检查状态后才能参与派工。</p><div>{unknown.filter(ticket => !projectId || ticket.project.id === projectId).map(ticket => <TaskCard key={ticket.key} ticket={ticket} onOpen={() => updateQuery('task', ticket.key)} />)}</div></section>}
      {archived.length > 0 && <section className="archive-list"><h2>已归档 · {archived.length}</h2><ul>{archived.map(ticket => <li key={ticket.key}><span className="ticket-id">{ticket.id}</span><strong>{ticket.title}</strong><span>{ticket.project.name}</span><span>已归档</span></li>)}</ul></section>}
      <footer className="board-footer"><GitBranch size={13} aria-hidden="true" /><span>任务来自项目文件 · 每 15 秒读取 · 派工不会改写票据状态</span><span>{refreshedAt && `上次读取 ${new Intl.DateTimeFormat('zh-CN', { hour: '2-digit', minute: '2-digit' }).format(refreshedAt)}`}</span></footer>
    </>}
    {selectedKey && <TaskInspector key={selectedKey} ticket={selected} initialHandoff={createdHandoff?.key === selectedKey ? createdHandoff : null} projectTickets={tickets.filter(ticket => ticket.project.id === selected?.project.id)} refreshing={refreshing} sourceError={error || sources.find(source => source.project.id === selected?.project.id || source.project.id === selectedKey.split(':', 1)[0])?.error || ''} onClose={() => updateQuery('task', null)} onOpenDependency={key => updateQuery('task', key)} onRefresh={refresh} />}
    {params.get('new') === '1' && <NewTicketDialog projectId={projectId} projects={creatableProjects} unavailableProjects={unavailableProjects} projectCreationReason={selectedCreationReason} onClose={() => updateQuery('new', null)} onCreated={async (createdProjectId, ticketId, receipt, handoffError, creationWarning) => {
      await refresh();
      const key = `${createdProjectId}:${ticketId}`;
      setCreatedHandoff({ key, receipt, error: handoffError, creationWarning });
      setParams(previous => {
        const next = new URLSearchParams(previous);
        next.delete('new');
        next.set('task', key);
        return next;
      });
    }} />}
  </main>;
}

function TaskCard({ ticket, onOpen }: { ticket: WorkTicket; onOpen: () => void }) {
  const summary = ticket.summary || textItems(ticket.body?.goal).join(' ');
  const status = taskStatus(ticket);
  const lane = taskLane(ticket) ?? 'unknown';
  return <button className={cn('task-card', `tone-${lane}`)} onClick={onOpen} aria-label={`${ticket.project.name}，${ticket.title}，${status}`}>
    <span className="card-meta"><span className="project-mark">{ticket.project.name.slice(0, 1).toUpperCase()}</span><span>{ticket.project.name}</span><ArrowUpRight size={14} className="card-arrow" aria-hidden="true" /></span>
    <span className="ticket-id">{ticket.id}</span>
    <h3>{ticket.title}</h3>
    {summary && summary !== ticket.title && <p className="card-description">{summary}</p>}
    <span className="card-footer"><span className="status-text">{status}</span><span>{shortDate(ticket.updated_at)}</span></span>
    {ticket.body_error && <span className="card-warning">任务正文读取失败</span>}
    {!ticket.body_error && ticket.unresolved_dependencies.length > 0 && <span className="card-warning">等待 {ticket.unresolved_dependencies.length} 项前置任务</span>}
  </button>;
}

function TaskInspector({ ticket, initialHandoff, projectTickets, refreshing, sourceError, onClose, onOpenDependency, onRefresh }: { ticket?: WorkTicket; initialHandoff: CreatedHandoff | null; projectTickets: WorkTicket[]; refreshing: boolean; sourceError: string; onClose: () => void; onOpenDependency: (key: string) => void; onRefresh: () => Promise<void> }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const initialFocus = useRef<HTMLButtonElement>(null);
  const [runtimes, setRuntimes] = useState<RuntimeInfo[]>([]);
  const [runtimeId, setRuntimeId] = useState('');
  const [loadingAgents, setLoadingAgents] = useState(true);
  const [detail, setDetail] = useState<GovernanceTicket | null>(null);
  const [loadingDetail, setLoadingDetail] = useState(Boolean(ticket));
  const [detailError, setDetailError] = useState('');
  const [agentError, setAgentError] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(initialHandoff?.error ?? '');
  const [receipt, setReceipt] = useState<TicketHandoff | null>(initialHandoff?.receipt ?? null);
  const [tab, setTab] = useState<'overview' | 'source'>('overview');
  const lock = useRef(false);
  const alive = useRef(true);
  const ticketProjectId = ticket?.project.id;
  const ticketId = ticket?.id;
  const creationWarning = initialHandoff?.creationWarning ?? ticket?.creation_warning ?? detail?.creation_warning ?? null;
  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialog.current?.showModal(); initialFocus.current?.focus();
    return () => { previous?.focus(); };
  }, []);
  useEffect(() => {
    alive.current = true;
    Promise.all([api.runtimes.list(), api.runtimes.getSelected()]).then(([items, preferred]) => {
      if (!alive.current) return;
      setRuntimes(items);
      setRuntimeId(items.find(item => item.available && item.id === preferred)?.id ?? items.find(item => item.available)?.id ?? '');
    }).catch(e => { if (alive.current) setAgentError(errorMessage(e)); }).finally(() => { if (alive.current) setLoadingAgents(false); });
    return () => { alive.current = false; };
  }, []);
  useEffect(() => {
    let active = true;
    if (!ticketProjectId || !ticketId) {
      setDetail(null); setDetailError(''); setLoadingDetail(false);
      return () => { active = false; };
    }
    setLoadingDetail(true); setDetail(null); setDetailError('');
    api.projects.ticketDetail(ticketProjectId, ticketId).then(response => {
      if (active && alive.current) setDetail(response.ticket);
    }).catch(failure => { if (active && alive.current) setDetailError(errorMessage(failure)); }).finally(() => { if (active && alive.current) setLoadingDetail(false); });
    return () => { active = false; };
  }, [ticketProjectId, ticketId]);
  const launch = async () => {
    if (creationWarning || !ticket?.ready || !runtimeId || lock.current || sourceError) return;
    lock.current = true; setBusy(true); setError('');
    try {
      const result = await api.projects.ticketLaunch(ticket.project.id, ticket.id, runtimeId);
      if (alive.current) setReceipt(result);
      void onRefresh();
    } catch (e) { if (alive.current) setError(errorMessage(e)); }
    finally { lock.current = false; if (alive.current) setBusy(false); }
  };
  const fullTicket = detail ?? ticket;
  const blockedBy = fullTicket?.blocked_by ?? ticket?.blocked_by ?? [];
  const unresolvedDependencies = fullTicket?.unresolved_dependencies ?? ticket?.unresolved_dependencies ?? [];
  const ticketReady = Boolean(fullTicket?.ready && fullTicket.body_loaded && !detailError);
  const available = runtimes.filter(runtime => runtime.available);
  return <dialog ref={dialog} className="task-inspector" onCancel={event => { event.preventDefault(); if (!busy) onClose(); }} onClick={event => { if (event.target === event.currentTarget && !busy) onClose(); }} aria-labelledby="task-detail-heading">
    <div className="inspector-shell">
      <header className="inspector-top"><span><FileText size={15} aria-hidden="true" />任务详情</span><button ref={initialFocus} className="icon-control" aria-label="关闭任务详情" disabled={busy} onClick={onClose}><X size={18} /></button></header>
      {creationWarning && <div role="alert" className="inspector-body"><p className="inline-error"><strong>任务已写入，但配套记录需要恢复</strong><br />{creationWarning}</p></div>}
      {!ticket ? <div className="inspector-body"><h2 id="task-detail-heading">当前无法读取这项任务</h2><p>任务可能已从索引移除，或项目来源暂时不可用。关闭后同步任务重试。</p></div> : <>
        <div className="inspector-title"><p className="ticket-id">{ticket.project.name} / {ticket.id}</p><h2 id="task-detail-heading">{ticket.title}</h2><div className={cn('inspector-meta', `tone-${taskLane(ticket) ?? 'unknown'}`)}><span className="status-text">{taskStatus(ticket)}</span><span><Clock3 size={13} />{shortDate(ticket.updated_at)} 更新</span>{ticket.complexity && <span>{ticket.complexity}</span>}</div></div>
        <div className="inspector-tabs" role="tablist" aria-label="任务信息"><button role="tab" id="overview-tab" aria-selected={tab === 'overview'} aria-controls="overview-panel" onClick={() => setTab('overview')} onKeyDown={e => { if (e.key === 'ArrowRight') { setTab('source'); document.getElementById('source-tab')?.focus(); } }}>概览</button><button role="tab" id="source-tab" aria-selected={tab === 'source'} aria-controls="source-panel" onClick={() => setTab('source')} onKeyDown={e => { if (e.key === 'ArrowLeft') { setTab('overview'); document.getElementById('overview-tab')?.focus(); } }}>来源记录</button></div>
        <div className="inspector-body" role="tabpanel" id={tab === 'overview' ? 'overview-panel' : 'source-panel'} aria-labelledby={tab === 'overview' ? 'overview-tab' : 'source-tab'}>
          {sourceError && <p role="alert" className="inline-error">{sourceError}</p>}
          {detailError && <p role="alert" className="inline-error">{detailError}</p>}
          {loadingDetail && <p role="status" className="inspector-help">正在读取任务原文…</p>}
          {!loadingDetail && fullTicket?.body_error && <p role="alert" className="inline-error">{fullTicket.body_error}</p>}
          {tab === 'source' ? <><h3>项目内的任务文件</h3><p className="source-path">{fullTicket?.source_path ?? ticket.source_path}</p><p className="inspector-help">任务状态以索引为准，正文保留项目中的原始内容。</p><pre className="source-code">{JSON.stringify({ governance_status: fullTicket?.governance_status ?? ticket.governance_status, ...fullTicket?.body }, null, 2)}</pre></> : <>
            {fullTicket?.body_loaded && <><DetailSection title="要完成什么" items={textItems(fullTicket.body?.goal)} empty="任务未提供目标描述。" />
            <DetailSection title="改动范围" items={textItems(fullTicket.body?.scope)} />
            <DetailSection title="验收条件" items={textItems(fullTicket.body?.acceptance)} numbered /></>}
            {blockedBy.length > 0 && <section className="detail-section"><h3>前置任务</h3><ul className="dependency-list">{blockedBy.map(id => {
              const dependency = projectTickets.find(item => item.id === id);
              const resolved = dependency?.governance_status === 'done' && !unresolvedDependencies.includes(id);
              return <li key={id}>
                {resolved ? <Check size={14} aria-hidden="true" /> : dependency ? <Circle size={14} aria-hidden="true" /> : <CircleHelp size={14} aria-hidden="true" />}
                <span className="dependency-id">{id}</span>
                {dependency ? <>
                  <button type="button" className="dependency-open" onClick={() => onOpenDependency(dependency.key)}>{dependency.title}</button>
                  <span className="dependency-status">{taskStatus(dependency)}</span>
                </> : <>
                  <span className="dependency-missing">未在已同步索引中找到</span>
                  <button type="button" className="dependency-sync" disabled={refreshing} onClick={() => void onRefresh()}><RefreshCw size={13} aria-hidden="true" className={refreshing ? 'animate-spin' : undefined} />{refreshing ? '同步中…' : '重新同步'}</button>
                </>}
              </li>;
            })}</ul></section>}
            {fullTicket?.body?.checkpoint && <DetailSection title="最近记录" items={textItems(fullTicket.body.checkpoint)} />}
            {!ticketReady && <div className="readiness-note"><CircleHelp size={16} aria-hidden="true" /><p>{fullTicket?.readiness_reason || '先读取并确认完整任务正文与验收条件，之后才能派工。'}</p></div>}
          </>}
        </div>
        <footer className="inspector-action">
          {receipt ? <div className="handoff-receipt" role="status"><CheckCircle2 size={20} aria-hidden="true" /><div><strong>{receipt.needs_paste ? '终端已打开，上下文已复制' : '命令已交给终端'}</strong><p>{receipt.needs_paste ? '在 ZCode 中按 ⌘V 粘贴，再确认开始。' : '终端已接受命令。请确认 Agent 已进入交互会话；返回工作台后会重新读取任务状态。'}</p><span className="ticket-id">{receipt.runtime_id} · {receipt.session_id}</span></div></div> : <>
            <div className="agent-picker"><label htmlFor="handoff-agent"><Terminal size={15} aria-hidden="true" />执行 Agent</label><select id="handoff-agent" value={runtimeId} onChange={event => setRuntimeId(event.target.value)} disabled={busy || loadingAgents || !available.length}>{!available.length && <option value="">{loadingAgents ? '检测中…' : '没有可用 Agent'}</option>}{available.map(runtime => <option key={runtime.id} value={runtime.id}>{runtime.name}</option>)}</select></div>
            {agentError && <p role="alert" className="inline-error">{agentError}</p>}
            {!loadingAgents && !available.length && <p className="inspector-help">先在<Link to="/settings" onClick={onClose}> Agent 设置</Link>中检查本机 CLI。</p>}
            {error && <p role="alert" className="inline-error">{error}</p>}
            <Button className="handoff-button" loading={busy} disabled={Boolean(creationWarning) || !ticketReady || loadingDetail || !runtimeId || loadingAgents || Boolean(sourceError)} onClick={() => void launch()}><Terminal size={16} aria-hidden="true" />{runtimeId === 'zcode-app' ? '复制上下文并打开 ZCode' : '交给 Agent'}<ArrowUpRight size={15} aria-hidden="true" /></Button>
            <p className="inspector-help">{ticketReady ? '打开交互式会话，携带项目目录与任务上下文。' : fullTicket?.readiness_reason || '当前任务不可派工。'}</p>
          </>}
        </footer>
      </>}
    </div>
  </dialog>;
}

function DetailSection({ title, items, numbered = false, empty }: { title: string; items: string[]; numbered?: boolean; empty?: string }) {
  if (!items.length && !empty) return null;
  return <section className="detail-section"><h3>{title}</h3>{items.length ? <ul className={numbered ? 'acceptance-list' : 'detail-prose'}>{items.map((item, index) => <li key={index}>{numbered && <span>{String(index + 1).padStart(2, '0')}</span>}<p>{item}</p></li>)}</ul> : <p className="inspector-help">{empty}</p>}</section>;
}

function NewTicketDialog({ projectId, projects, unavailableProjects, projectCreationReason, onClose, onCreated }: { projectId?: string; projects: Project[]; unavailableProjects: { project: Project; reason: string }[]; projectCreationReason: string | null; onClose: () => void; onCreated: (createdProjectId: string, ticketId: string, receipt: TicketHandoff | null, handoffError: string, creationWarning: string | null) => Promise<void> }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const descriptionInput = useRef<HTMLTextAreaElement>(null);
  const errorSummary = useRef<HTMLDivElement>(null);
  const [targetProjectId, setTargetProjectId] = useState(projectId ?? '');
  const [description, setDescription] = useState('');
  const [draft, setDraft] = useState<TicketDraft | null>(null);
  const [runtimeId, setRuntimeId] = useState('');
  const [generating, setGenerating] = useState(false);
  const [writing, setWriting] = useState(false);
  const [error, setError] = useState('');
  const [draftLock, setDraftLock] = useState(false);
  const actionLock = useRef(false);
  const targetProjectSupported = projects.some(project => project.id === targetProjectId);
  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const node = dialog.current;
    node?.showModal();
    if (projectId) descriptionInput.current?.focus();
    return () => {
      if (node?.open) node.close();
      if (previous?.isConnected) previous.focus();
    };
  }, [projectId]);
  useEffect(() => {
    if (error) errorSummary.current?.focus();
  }, [error]);
  const generateDraft = async () => {
    if (!targetProjectId || !targetProjectSupported || description.trim().length < 4 || actionLock.current) return;
    actionLock.current = true;
    setDraftLock(true);
    setGenerating(true);
    setError('');
    setDraft(null);
    try {
      const result = await api.projects.ticketDraft(targetProjectId, description.trim());
      setDraft(result.draft);
      setRuntimeId(result.runtime_id);
    } catch (failure) {
      setError(errorMessage(failure));
    } finally {
      actionLock.current = false;
      setDraftLock(false);
      setGenerating(false);
    }
  };
  const submit = async () => {
    if (!targetProjectId || !targetProjectSupported || !draft || actionLock.current) return;
    actionLock.current = true;
    setDraftLock(true);
    setWriting(true);
    setError('');
    try {
      const created = await api.projects.ticketCreate(targetProjectId, draft);
      let receipt: TicketHandoff | null = null;
      let handoffError = '';
      const creationWarning = created.ticket.creation_warning ?? null;
      if (!creationWarning && created.ticket.ready && created.ticket.body_loaded) {
        try {
          const [runtimes, preferred] = await Promise.all([api.runtimes.list(), api.runtimes.getSelected()]);
          const selectedRuntime = runtimes.find(runtime => runtime.available && runtime.id === preferred) ?? runtimes.find(runtime => runtime.available);
          if (selectedRuntime) receipt = await api.projects.ticketLaunch(created.project_id, created.ticket.id, selectedRuntime.id);
          else handoffError = '任务已写入，但当前没有可用 Agent。你可以在详情中检查设置后再交接。';
        } catch (failure) {
          handoffError = `任务已写入，但 Agent 交接未完成：${errorMessage(failure)}`;
        }
      }
      await onCreated(created.project_id, created.ticket.id, receipt, handoffError, creationWarning);
    } catch (failure) {
      setError(errorMessage(failure));
    } finally {
      actionLock.current = false;
      setDraftLock(false);
      setWriting(false);
    }
  };
  const updateDescription = (value: string) => {
    setDescription(value);
    setDraft(null);
    setError('');
    const input = descriptionInput.current;
    if (input) {
      input.style.height = 'auto';
      input.style.height = `${Math.min(input.scrollHeight, window.innerHeight * 0.34)}px`;
    }
  };
  const previewItems = (items: string[]) => items.length
    ? <ul>{items.map((item, index) => <li key={`${index}:${item}`}>{item}</li>)}</ul>
    : <p className="draft-empty">无</p>;
  return <dialog ref={dialog} className="task-inspector new-ticket-dialog" onCancel={event => { event.preventDefault(); if (!draftLock) onClose(); }} onClick={event => { if (event.target === event.currentTarget && !draftLock) onClose(); }} aria-labelledby="new-ticket-heading">
    <div className="inspector-shell">
      <header className="inspector-top"><span><FilePlus2 size={15} aria-hidden="true" />新建任务草案</span><button className="icon-control" aria-label="关闭新建任务" disabled={draftLock} onClick={onClose}><X size={18} aria-hidden="true" /></button></header>
      <div className="inspector-body draft-body">
        <h2 id="new-ticket-heading">描述要完成的结果</h2>
        <label className="form-field draft-description" htmlFor="ticket-description"><span>任务描述</span><textarea id="ticket-description" ref={descriptionInput} autoFocus={Boolean(projectId)} value={description} onChange={event => updateDescription(event.target.value)} rows={1} maxLength={12000} disabled={draftLock} aria-invalid={Boolean(error)} aria-describedby={error ? 'ticket-description-help ticket-draft-error' : 'ticket-description-help'} placeholder="例如：让登录页的所有操作都能通过键盘完成，并保留现有视觉和行为。" /></label>
        {!projectId && <label className="form-field draft-project" htmlFor="ticket-project"><span>目标项目</span><select id="ticket-project" autoFocus disabled={draftLock} value={targetProjectSupported ? targetProjectId : ''} onChange={event => { setTargetProjectId(event.target.value); setDraft(null); setError(''); }}><option value="">选择可安全创建任务的项目</option>{projects.map(project => <option key={project.id} value={project.id}>{project.name}</option>)}</select></label>}
        {projectId && projectCreationReason && <p className="inline-error" role="status">{projectCreationReason}</p>}
        {!projectId && unavailableProjects.length > 0 && <details className="unavailable-projects"><summary>不可新建的项目（{unavailableProjects.length}）</summary><ul>{unavailableProjects.map(({ project, reason }) => <li key={project.id}><strong>{project.name}</strong><span>{reason}</span></li>)}</ul></details>}
        <p id="ticket-description-help" className="inspector-help">Agent 会读取相关项目上下文并生成任务字段。草案确认前不会写入。</p>
        {error && <div ref={errorSummary} id="ticket-draft-error" tabIndex={-1} role="alert" className="inline-error">{error}</div>}
        {!draft && <Button className="handoff-button draft-generate" loading={generating} disabled={!targetProjectId || !targetProjectSupported || Array.from(description.trim()).length < 4 || draftLock} onClick={() => void generateDraft()}><Sparkles size={16} aria-hidden="true" />生成任务草案</Button>}
        {draft && <section className="draft-preview" aria-label="任务草案预览">
          <header><span className="ticket-id">Agent 草案 · {draft.complexity}</span><button type="button" className="draft-regenerate" disabled={draftLock} onClick={() => void generateDraft()}><RefreshCw size={14} aria-hidden="true" />重新生成</button></header>
          <dl>
            <div><dt>标题</dt><dd>{draft.title}</dd></div>
            <div><dt>目标</dt><dd>{draft.goal}</dd></div>
            <div><dt>范围</dt><dd>{previewItems(draft.scope)}</dd></div>
            <div><dt>验收条件</dt><dd>{previewItems(draft.acceptance)}</dd></div>
            {draft.blocked_by.length > 0 && <div><dt>前置任务</dt><dd>{previewItems(draft.blocked_by)}</dd></div>}
          </dl>
          <details className="draft-diagnostics">
            <summary>查看验证口径与评估依据</summary>
            <dl>
              <div><dt>验证口径</dt><dd>{draft.verification}</dd></div>
              <div><dt>复杂度依据</dt><dd>{draft.complexity_reason}</dd></div>
              <div><dt>优先级依据</dt><dd>{draft.priority_reason}</dd></div>
              <div><dt>Profile</dt><dd>{draft.profile}</dd></div>
            </dl>
          </details>
          <p className="draft-runtime">草案由 {runtimeId || 'Agent'} 生成。确认后才会写入所选项目。</p>
        </section>}
      </div>
      {draft && <footer className="inspector-action draft-action"><p className="inspector-help">请核对范围、复杂度和验收条件。确认后写入任务；若任务已就绪，将使用当前 Agent 打开交互会话。前置条件未满足时会先打开详情。</p><Button className="handoff-button" loading={writing} disabled={draftLock} onClick={() => void submit()}><Check size={16} aria-hidden="true" />确认并交给 Agent</Button></footer>}
    </div>
  </dialog>;
}
