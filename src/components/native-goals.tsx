// Input: 已接入项目 ID、用户目标、真实规划结果与用户确认。
// Output: App 原生目标的创建、规划进度、任务编辑与确认；不启动实施。
// Pos: 项目工作区的目标面板，沿用既有组件与设计 tokens；src/ 目录登记豁免。
import { useCallback, useEffect, useRef, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { AlertCircle, CheckCircle2, ChevronDown, ChevronRight, Loader2, Plus, RefreshCw, Sparkles, Trash2 } from 'lucide-react';
import type { NativeGoal, NativeTask } from '../../shared/types';
import { api, errorMessage } from '@/lib/api';
import { filterNativeGoals } from '@/lib/native-goal-search';
import { cn } from '@/lib/utils';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { AutoTextarea } from '@/components/ui/auto-textarea';
import { NativeDeliveryPanel } from './native-delivery';

const inputClass = 'w-full rounded-lg border border-input bg-background px-3 py-2 text-sm leading-relaxed text-foreground outline-none focus-visible:border-primary focus-visible:ring-2 focus-visible:ring-primary/30 disabled:opacity-60';
const statusLabels: Record<NativeGoal['status'], string> = {
  draft: '待规划', planning: '方案制定中', awaiting_confirmation: '待确认', ready: '计划已确认', completed: '目标已完成', failed: '规划失败',
};

function compactGoalTitle(goal: NativeGoal) {
  const source = (goal.summary || goal.content).trim();
  const beforeColon = source.split(/[：:]/, 1)[0]?.trim();
  const title = beforeColon && beforeColon.length >= 4 && beforeColon.length <= 36 ? beforeColon : source.split(/[。！？!?\n]/, 1)[0]?.trim() || source;
  return title.length > 56 ? `${title.slice(0, 56)}…` : title;
}

function goalLead(goal: NativeGoal) {
  const source = (goal.summary || goal.content).trim();
  const title = compactGoalTitle(goal);
  return source === title ? '' : source;
}

type PendingAction = { goalId: string; kind: 'plan' | 'confirm'; startedAt: number };

function GoalStatus({ goal, pending }: { goal: NativeGoal; pending?: PendingAction | null }) {
  const active = pending?.goalId === goal.id ? pending : null;
  const processing = Boolean(active) || goal.status === 'planning';
  const label = active?.kind === 'confirm' ? '保存中' : active?.kind === 'plan' && goal.status !== 'planning' ? '请求规划中' : statusLabels[goal.status];
  return <Badge className={cn('shrink-0', processing ? 'bg-primary/10 text-primary' : goal.status === 'failed' ? 'bg-destructive/10 text-destructive' : goal.status === 'draft' ? 'bg-muted text-muted-foreground' : 'bg-primary/10 text-primary')}>
    {processing && <Loader2 aria-hidden="true" className="mr-1 h-3 w-3 animate-spin" />}
    {label}
  </Badge>;
}

function GoalStages({ goal }: { goal: NativeGoal }) {
  const plan = goal.status === 'awaiting_confirmation' || goal.status === 'ready' ? 'done' : goal.status === 'planning' ? 'active' : goal.status === 'failed' ? 'failed' : 'pending';
  const delivery = goal.status === 'completed' ? 'done' : goal.status === 'ready' ? 'active' : 'pending';
  const acceptance = goal.status === 'completed' ? 'done' : 'pending';
  const items = [
    ['目标', 'done'],
    ['计划', plan],
    ['执行与验证', delivery],
    ['用户验收', acceptance],
  ] as const;
  return <ol aria-label="目标交付阶段" className="flex min-w-0 flex-wrap items-center gap-y-2 text-xs">
    {items.map(([label, state], index) => <li key={label} className="flex items-center">
      <span className={cn('inline-flex items-center gap-1.5 whitespace-nowrap', state === 'done' ? 'text-primary' : state === 'active' ? 'font-medium text-primary' : state === 'failed' ? 'text-destructive' : 'text-muted-foreground')}>
        <span className={cn('grid h-5 w-5 place-items-center rounded-full border text-[10px]', state === 'done' ? 'border-primary bg-primary text-primary-foreground' : state === 'active' ? 'border-primary text-primary' : state === 'failed' ? 'border-destructive text-destructive' : 'border-border')}>
          {state === 'done' ? <CheckCircle2 aria-hidden="true" className="h-3.5 w-3.5" /> : state === 'active' ? <span className="h-1.5 w-1.5 rounded-full bg-current" /> : index + 1}
        </span>
        {label}
      </span>
      {index < items.length - 1 && <span aria-hidden="true" className="mx-2 h-px w-6 bg-border sm:w-10" />}
    </li>)}
  </ol>;
}

function PlanningElapsed({ startedAt }: { startedAt: number }) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);
  const seconds = Math.max(0, Math.floor((now - startedAt) / 1000));
  return <span className="tabular-nums">已等待 {seconds < 60 ? `${seconds} 秒` : `${Math.floor(seconds / 60)} 分 ${seconds % 60} 秒`}</span>;
}

export function NativeGoals({ projectId }: { projectId: string }) {
  const [searchParams, setSearchParams] = useSearchParams();
  const [goals, setGoals] = useState<NativeGoal[]>([]);
  const [query, setQuery] = useState('');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [composing, setComposing] = useState(searchParams.get('newGoal') === '1');
  const [content, setContent] = useState('');
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [pendingAction, setPendingAction] = useState<PendingAction | null>(null);
  const [error, setError] = useState('');
  const alive = useRef(true);
  const readVersion = useRef(0);
  const mutation = useRef(false);
  const createLock = useRef(false);

  useEffect(() => {
    if (searchParams.get('newGoal') !== '1') return;
    setComposing(true);
    const next = new URLSearchParams(searchParams);
    next.delete('newGoal');
    setSearchParams(next, { replace: true });
  }, [searchParams, setSearchParams]);

  const refresh = useCallback(async () => {
    const version = ++readVersion.current;
    try {
      const result = await api.goals.list(projectId);
      if (!alive.current || version !== readVersion.current) return;
      setGoals((previous) => result.map((goal) => {
        const current = previous.find((item) => item.id === goal.id);
        return current && current.revision > goal.revision ? current : goal;
      }));
      setSelectedId((id) => result.some((goal) => goal.id === id) ? id : result[0]?.id ?? null);
    } catch (err) {
      if (alive.current && version === readVersion.current) setError(errorMessage(err));
    } finally {
      if (alive.current && version === readVersion.current) setLoading(false);
    }
  }, [projectId]);

  useEffect(() => {
    alive.current = true;
    void refresh();
    return () => { alive.current = false; readVersion.current += 1; };
  }, [refresh]);

  const polling = pendingAction !== null || goals.some((goal) => goal.status === 'planning');
  useEffect(() => {
    if (!polling) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const next = () => { timer = setTimeout(async () => { await refresh(); if (!stopped) next(); }, 3000); };
    next();
    return () => { stopped = true; clearTimeout(timer); };
  }, [polling, refresh]);

  const updateGoal = (goal: NativeGoal) => {
    readVersion.current += 1;
    setGoals((previous) => previous.some((item) => item.id === goal.id)
      ? previous.map((item) => item.id === goal.id ? goal : item)
      : [goal, ...previous]);
  };

  const create = async () => {
    if (Array.from(content.trim()).length < 4 || createLock.current) return;
    createLock.current = true;
    setCreating(true);
    setError('');
    try {
      const goal = await api.goals.create(projectId, content.trim());
      if (!alive.current) return;
      updateGoal(goal);
      setSelectedId(goal.id);
      setContent('');
      setComposing(false);
    } catch (err) {
      if (alive.current) setError(errorMessage(err));
    } finally {
      createLock.current = false;
      if (alive.current) setCreating(false);
    }
  };

  const act = async (goal: NativeGoal, kind: PendingAction['kind'], action: () => Promise<NativeGoal>) => {
    if (mutation.current) return;
    mutation.current = true;
    readVersion.current += 1;
    setPendingAction({ goalId: goal.id, kind, startedAt: Date.now() });
    setError('');
    try {
      const result = await action();
      if (alive.current) updateGoal(result);
    } catch (err) {
      if (alive.current) {
        setError(errorMessage(err));
        await refresh();
      }
    } finally {
      mutation.current = false;
      if (alive.current) setPendingAction(null);
    }
  };

  const selected = goals.find((goal) => goal.id === selectedId);
  const filteredGoals = filterNativeGoals(goals, query);
  const showComposer = composing || (!loading && goals.length === 0);

  return <section className="space-y-4" aria-label="项目目标">
    <div className="flex items-center justify-between gap-3">
      <div>
        <h2 className="font-medium">目标与任务 <span className="ml-1 text-sm text-muted-foreground">{goals.length}</span></h2>
        <p className="mt-1 text-xs text-muted-foreground">写下目标 → 生成任务计划 → 由你确认</p>
      </div>
      {!showComposer && <Button variant="secondary" onClick={() => { setComposing(true); setError(''); }}><Plus aria-hidden="true" className="h-4 w-4" />新建目标</Button>}
    </div>

    {error && <div role="alert" className="flex items-start gap-2 rounded-lg border border-destructive/20 bg-destructive/10 p-3 text-sm text-destructive">
      <AlertCircle aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0" />
      <div className="min-w-0 flex-1 break-words">{error}</div>
      <Button size="sm" variant="ghost" onClick={() => { setError(''); void refresh(); }}>重新读取</Button>
    </div>}

    {loading ? <div role="status" className="flex items-center gap-2 rounded-xl border border-border bg-card p-5 text-sm text-muted-foreground"><Loader2 aria-hidden="true" className="h-4 w-4 animate-spin" />正在读取目标…</div> : <>
      {showComposer && <form onSubmit={(event) => { event.preventDefault(); void create(); }} className="space-y-4 border-y border-border py-5">
        <div><h3 className="font-medium">新建目标</h3><p id="goal-create-help" className="mt-1 text-sm text-muted-foreground">描述你希望完成什么。先保存目标，再让 Agent 只读分析项目、拆分任务。</p></div>
        <label htmlFor="native-goal-content" className="sr-only">目标描述</label>
        <AutoTextarea id="native-goal-content" autoFocus rows={3} maxLength={12000} value={content} onChange={(event) => setContent(event.target.value)} disabled={creating} aria-describedby="goal-create-help" placeholder="例如：让工作台显示每个项目的目标进度" maxAutoHeight={220} className={inputClass} />
        {content.trim() && Array.from(content.trim()).length < 4 && <p className="text-xs text-muted-foreground">请至少输入 4 个字，描述你想完成的事。</p>}
        <div className="flex items-center justify-end gap-2">
          {goals.length > 0 && <Button type="button" variant="ghost" disabled={creating} onClick={() => setComposing(false)}>返回目标</Button>}
          <Button type="submit" loading={creating} disabled={Array.from(content.trim()).length < 4}><Plus aria-hidden="true" className="h-4 w-4" />保存目标</Button>
        </div>
      </form>}

      {!showComposer && goals.length > 0 && <div className="grid grid-cols-1 items-start gap-5 lg:grid-cols-3">
        <nav aria-label="目标列表" className="space-y-2 lg:sticky lg:top-0">
          <div className="flex items-center gap-2">
            <label htmlFor="native-goal-search" className="sr-only">搜索目标或计划概述</label>
            <input id="native-goal-search" type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索目标或计划概述" className={cn(inputClass, 'min-w-0')} />
            <Button type="button" size="sm" variant="ghost" className="shrink-0" disabled={!query} onClick={() => setQuery('')} aria-label="清空搜索">清空</Button>
          </div>
          {filteredGoals.length === 0 && <p role="status" className="border-y border-border px-3 py-4 text-sm text-muted-foreground">没有匹配的目标，请尝试其他关键词或清空搜索。</p>}
          <div className="border-y border-border">
          {filteredGoals.map((goal) => <button key={goal.id} type="button" aria-current={selectedId === goal.id ? 'true' : undefined} onClick={() => { setSelectedId(goal.id); setError(''); }} className={cn('w-full border-b border-border px-3 py-4 text-left outline-none transition-colors last:border-b-0 focus-visible:ring-2 focus-visible:ring-primary/30', selectedId === goal.id ? 'bg-primary/5' : 'hover:bg-accent')}>
            <div className="mb-2 flex items-center justify-between gap-2"><GoalStatus goal={goal} pending={pendingAction} /><ChevronRight aria-hidden="true" className="h-4 w-4 shrink-0 text-muted-foreground" /></div>
            <p className="break-words text-sm font-medium leading-relaxed">{compactGoalTitle(goal)}</p>
            {goalLead(goal) && <p className="mt-1 line-clamp-2 break-words text-xs leading-5 text-muted-foreground">{goalLead(goal)}</p>}
            <p className="mt-2 text-xs text-muted-foreground">{goal.tasks.length > 0 ? `${goal.tasks.length} 个任务 · ` : ''}{new Date(goal.updated_at).toLocaleDateString('zh-CN')}</p>
          </button>)}
          </div>
        </nav>
        {selected && <GoalDetail key={selected.id} projectId={projectId} goal={selected} pending={pendingAction?.goalId === selected.id ? pendingAction : null} busy={pendingAction !== null || selected.status === 'planning'} onPlan={(feedback) => void act(selected, 'plan', () => api.goals.plan(projectId, selected.id, feedback))} onConfirm={(tasks) => void act(selected, 'confirm', () => api.goals.confirm(projectId, selected.id, selected.revision, tasks))} onChanged={refresh} />}
      </div>}
    </>}
  </section>;
}

function GoalDetail({ projectId, goal, pending, busy, onPlan, onConfirm, onChanged }: { projectId: string; goal: NativeGoal; pending: PendingAction | null; busy: boolean; onPlan: (feedback: string) => void; onConfirm: (tasks: NativeTask[]) => void; onChanged: () => Promise<void> }) {
  const [feedback, setFeedback] = useState('');
  const [tasks, setTasks] = useState<NativeTask[]>(() => goal.tasks.map((task) => ({ ...task })));
  const [planRevision, setPlanRevision] = useState(goal.revision);
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(goal.tasks[0]?.id ?? null);
  // Only a new completed plan replaces edits; polling, startup and failure keep the draft.
  useEffect(() => {
    if ((goal.status === 'awaiting_confirmation' || goal.status === 'ready') && goal.revision !== planRevision) {
      setPlanRevision(goal.revision);
      setTasks(goal.tasks.map((task) => ({ ...task })));
      setSelectedTaskId(goal.tasks[0]?.id ?? null);
      setFeedback('');
    }
  }, [goal.status, goal.revision, goal.tasks, planRevision]);
  useEffect(() => {
    if (!tasks.some((task) => task.id === selectedTaskId)) setSelectedTaskId(tasks[0]?.id ?? null);
  }, [selectedTaskId, tasks]);
  const planning = pending?.kind === 'plan' || goal.status === 'planning';
  const confirming = pending?.kind === 'confirm';
  const startedAt = goal.status === 'planning' && goal.run ? Date.parse(goal.run.started_at) : pending?.startedAt;
  const ready = goal.status === 'ready' || goal.status === 'completed';
  const canConfirm = goal.status === 'awaiting_confirmation' && goal.questions.length === 0 && tasks.length > 0;
  const invalidTasks = tasks.some((task) => !task.title.trim() || !task.description.trim() || !task.acceptance.some((item) => item.trim()));
  const selectedTask = tasks.find((task) => task.id === selectedTaskId) ?? tasks[0] ?? null;
  const editTask = (id: string, update: Partial<NativeTask>) => setTasks((items) => items.map((task) => task.id === id ? { ...task, ...update } : task));
  const removeTask = (id: string) => setTasks((items) => items.filter((item) => item.id !== id).map((item) => ({ ...item, depends_on: item.depends_on.filter((dependency) => dependency !== id) })));
  const statusText = goal.status === 'completed' ? '已完成' : goal.status === 'ready' ? '等待执行' : goal.status === 'awaiting_confirmation' ? '等待你确认计划' : goal.status === 'planning' ? 'Agent 正在分析项目' : goal.status === 'failed' ? '需要重新规划' : '下一步：生成计划';

  return <div className="min-w-0 space-y-6 lg:col-span-2">
    <section className="space-y-5 border-b border-border pb-6">
      <div className="flex flex-wrap items-start justify-between gap-4 border-l-4 border-primary pl-4">
        <div className="min-w-0 max-w-3xl">
          <p className="text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground">当前目标</p>
          <h3 className="mt-1 break-words text-xl font-semibold tracking-tight">{compactGoalTitle(goal)}</h3>
          {goalLead(goal) && <p className="mt-2 line-clamp-2 max-w-3xl break-words text-sm leading-6 text-muted-foreground">{goalLead(goal)}</p>}
        </div>
        <GoalStatus goal={goal} pending={pending} />
      </div>
      <GoalStages goal={goal} />
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-muted-foreground">
        <span className="font-medium text-foreground">{statusText}</span>
        <span aria-hidden="true">·</span><span>计划修订版 {goal.revision}</span>
        {goal.status === 'ready' && <><span aria-hidden="true">·</span><span className="text-primary">下一步：执行任务</span></>}
        {goal.status === 'completed' && <><span aria-hidden="true">·</span><span className="text-primary">所有任务已合入本地</span></>}
      </div>
      <details className="group border-y border-border py-3">
        <summary className="flex cursor-pointer list-none items-center justify-between gap-3 text-sm font-medium outline-none focus-visible:ring-2 focus-visible:ring-primary/30"><span>目标上下文</span><ChevronDown aria-hidden="true" className="h-4 w-4 text-muted-foreground transition-transform group-open:rotate-180" /></summary>
        <div className="mt-3 space-y-3 border-t border-border/70 pt-3 text-sm leading-6">
          <p className="whitespace-pre-wrap break-words">{goal.content}</p>
          {goal.summary && <div><p className="mb-1 text-xs font-medium text-muted-foreground">计划概述</p><p className="whitespace-pre-wrap break-words">{goal.summary}</p></div>}
          {goal.run && <p className="text-xs text-muted-foreground">规划 Agent：{goal.run.runtime_id} · {new Date(goal.run.started_at).toLocaleString('zh-CN')}{goal.run.tokens != null ? ` · ${goal.run.tokens.toLocaleString()} tokens` : ''}</p>}
        </div>
      </details>
      {goal.error && <div role="alert" className="flex items-start gap-2 border-l-2 border-destructive pl-3 text-sm text-destructive"><AlertCircle aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0" /><div className="min-w-0 flex-1"><p className="whitespace-pre-wrap break-words">{goal.error}</p><Link to="/settings" className="mt-1 inline-block rounded text-xs underline underline-offset-4 focus-visible:ring-2 focus-visible:ring-primary/30">检查 Agent 设置</Link></div></div>}
      {planning && <div className="flex items-start gap-2 text-sm text-primary"><Loader2 aria-hidden="true" className="mt-0.5 h-4 w-4 animate-spin" /><div><p role="status">{goal.status === 'planning' ? 'Agent 正在只读分析项目、制定任务计划。' : '正在请求 Agent 规划…'}</p><p className="mt-1 text-xs text-muted-foreground">{startedAt != null && Number.isFinite(startedAt) && <><PlanningElapsed startedAt={startedAt} /> · </>}可以离开页面，结果会自动保存。</p></div></div>}
      {confirming && <p role="status" className="flex items-center gap-2 text-sm text-primary"><Loader2 aria-hidden="true" className="h-4 w-4 animate-spin" />正在保存确认的任务计划…</p>}
      {goal.status === 'completed' && <div role="status" className="flex items-center gap-2 text-sm text-primary"><CheckCircle2 aria-hidden="true" className="h-4 w-4 shrink-0" /><p>所有任务都已通过真实验证、独立审查并由你验收合入本地。</p></div>}
    </section>

    {goal.questions.length > 0 && <section className="border-l-2 border-primary pl-4">
      <h3 className="text-sm font-semibold">需要你明确</h3>
      <ol className="mt-2 list-decimal space-y-1 pl-5 text-sm leading-6">{goal.questions.map((question, index) => <li key={`${index}:${question}`}>{question}</li>)}</ol>
      <p className="mt-2 text-xs text-muted-foreground">回答后才能确认任务计划。</p>
    </section>}

    {!ready && (tasks.length > 0 || goal.status === 'awaiting_confirmation') && <section aria-label="任务计划" className="overflow-hidden border-y border-border">
      <header className="flex flex-wrap items-center justify-between gap-3 border-b border-border bg-muted/30 px-4 py-3">
        <div><h3 className="text-sm font-semibold">任务计划 <span className="ml-1 font-normal text-muted-foreground">{tasks.length} 项</span></h3><p className="mt-1 text-xs text-muted-foreground">选择任务查看细节{ready ? '' : '，确认前可编辑'}</p></div>
        {!ready && <Button type="button" size="sm" variant="ghost" disabled={busy} onClick={() => { const task = { id: `task-${crypto.randomUUID()}`, title: '', description: '', acceptance: [''], depends_on: [], capabilities: [] }; setTasks((items) => [...items, task]); setSelectedTaskId(task.id); }}><Plus aria-hidden="true" className="h-4 w-4" />增加任务</Button>}
      </header>
      <div className="grid min-w-0 lg:grid-cols-[minmax(180px,0.34fr)_minmax(0,0.66fr)]">
        <nav aria-label="任务队列" className="order-2 border-t border-border lg:order-1 lg:border-r lg:border-t-0">
          {tasks.length === 0 && <p className="p-4 text-sm text-muted-foreground">计划还没有任务。</p>}
          {tasks.map((task, index) => <div key={task.id} className={cn('flex items-stretch border-b border-border last:border-b-0', selectedTask?.id === task.id && 'bg-primary/5')}>
            <button type="button" aria-current={selectedTask?.id === task.id ? 'true' : undefined} onClick={() => setSelectedTaskId(task.id)} className="min-w-0 flex-1 px-4 py-3 text-left outline-none transition-colors hover:bg-accent/60 focus-visible:ring-2 focus-visible:ring-primary/30">
              <span className="flex items-start gap-2"><span className={cn('mt-0.5 grid h-5 w-5 shrink-0 place-items-center rounded-full border text-[10px]', selectedTask?.id === task.id ? 'border-primary text-primary' : 'border-border text-muted-foreground')}>{index + 1}</span><span className="min-w-0"><span className="block break-words text-sm font-medium">{task.title || '未命名任务'}</span><span className="mt-1 block text-xs text-muted-foreground">{task.depends_on.length ? `依赖 ${task.depends_on.length} 项` : '可独立开始'}</span></span></span>
            </button>
            {!ready && <Button type="button" size="sm" variant="ghost" className="my-2 mr-2 shrink-0" disabled={busy} aria-label={`删除任务 ${index + 1}`} onClick={() => removeTask(task.id)}><Trash2 aria-hidden="true" className="h-4 w-4" /></Button>}
          </div>)}
        </nav>
        <div className="order-1 min-w-0 p-4 sm:p-5 lg:order-2">
          {selectedTask ? <div className="space-y-5">
            <div className="flex flex-wrap items-start justify-between gap-3"><div className="min-w-0"><p className="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">任务 {tasks.findIndex((task) => task.id === selectedTask.id) + 1}</p>{ready ? <h4 className="mt-1 break-words text-base font-semibold">{selectedTask.title || '未命名任务'}</h4> : <label className="mt-1 block"><span className="sr-only">任务名称</span><input value={selectedTask.title} disabled={busy} onChange={(event) => editTask(selectedTask.id, { title: event.target.value })} className={cn(inputClass, 'text-base font-medium')} /></label>}</div><span className="text-xs text-muted-foreground">{selectedTask.capabilities.length ? selectedTask.capabilities.join(' · ') : '通用实现'}</span></div>
            {ready ? <>
              <details className="group border-t border-border py-3"><summary className="flex cursor-pointer list-none items-center justify-between gap-3 text-sm font-medium outline-none focus-visible:ring-2 focus-visible:ring-primary/30"><span>任务说明</span><ChevronDown aria-hidden="true" className="h-4 w-4 text-muted-foreground transition-transform group-open:rotate-180" /></summary><p className="mt-3 whitespace-pre-wrap break-words text-sm leading-6">{selectedTask.description || '暂无说明'}</p></details>
              <details className="group border-t border-border py-3"><summary className="flex cursor-pointer list-none items-center justify-between gap-3 text-sm font-medium outline-none focus-visible:ring-2 focus-visible:ring-primary/30"><span>验收标准 <span className="font-normal text-muted-foreground">{selectedTask.acceptance.length} 项</span></span><ChevronDown aria-hidden="true" className="h-4 w-4 text-muted-foreground transition-transform group-open:rotate-180" /></summary><ul className="mt-3 space-y-2 text-sm leading-6">{selectedTask.acceptance.map((item, index) => <li key={`${item}:${index}`} className="flex items-start gap-2"><CheckCircle2 aria-hidden="true" className="mt-1 h-4 w-4 shrink-0 text-primary" /><span className="whitespace-pre-wrap break-words">{item}</span></li>)}</ul></details>
            </> : <>
              <label className="block text-xs font-medium text-muted-foreground">要完成什么<AutoTextarea rows={3} maxAutoHeight={280} value={selectedTask.description} disabled={busy} onChange={(event) => editTask(selectedTask.id, { description: event.target.value })} className={cn(inputClass, 'mt-2 font-normal')} /></label>
              <label className="block text-xs font-medium text-muted-foreground">验收标准（每行一项）<AutoTextarea rows={3} maxAutoHeight={280} value={selectedTask.acceptance.join('\n')} disabled={busy} onChange={(event) => editTask(selectedTask.id, { acceptance: event.target.value.split('\n') })} className={cn(inputClass, 'mt-2 font-normal')} /></label>
              {tasks.length > 1 && <details className="group text-xs text-muted-foreground"><summary className="flex cursor-pointer list-none items-center gap-2 rounded py-1 outline-none focus-visible:ring-2 focus-visible:ring-primary/30"><span>前置任务：{selectedTask.depends_on.length ? selectedTask.depends_on.map((id) => tasks.find((item) => item.id === id)?.title || id).join('、') : '无，可独立开始'}</span><ChevronDown aria-hidden="true" className="h-3.5 w-3.5 transition-transform group-open:rotate-180" /></summary><fieldset disabled={busy} className="mt-2 space-y-2 border-l border-border pl-3"><legend className="sr-only">选择前置任务</legend>{tasks.filter((item) => item.id !== selectedTask.id).map((item) => <label key={item.id} className="flex items-start gap-2"><input type="checkbox" checked={selectedTask.depends_on.includes(item.id)} onChange={(event) => editTask(selectedTask.id, { depends_on: event.target.checked ? [...selectedTask.depends_on, item.id] : selectedTask.depends_on.filter((id) => id !== item.id) })} className="mt-0.5 accent-primary" /><span className="break-words">{item.title || '未命名任务'}</span></label>)}</fieldset></details>}
            </>}
          </div> : <p className="text-sm text-muted-foreground">选择一个任务开始查看。</p>}
        </div>
      </div>
    </section>}

    {ready && <NativeDeliveryPanel key={`${projectId}:${goal.id}:${goal.revision}`} projectId={projectId} goal={goal} onChanged={onChanged} />}

    {!ready && <section className="space-y-3 border-t border-border pt-5">
      <div><h3 className="text-sm font-semibold">确认前的补充</h3><p className="mt-1 text-xs text-muted-foreground">{goal.questions.length > 0 ? '逐项回答上面的疑问，再重新生成计划。' : '可选。告诉 Agent 约束、优先级或范围。'}</p></div>
      <label htmlFor={`goal-feedback-${goal.id}`} className="sr-only">{goal.questions.length > 0 ? '补充答案' : '补充要求'}</label>
      <AutoTextarea id={`goal-feedback-${goal.id}`} rows={2} maxLength={12000} maxAutoHeight={220} value={feedback} disabled={busy} onChange={(event) => setFeedback(event.target.value)} placeholder={goal.questions.length > 0 ? '逐项回答上面的疑问…' : '例如：先做最小版本，保留现有设计规范…'} className={inputClass} />
      {canConfirm && invalidTasks && <p className="text-xs text-destructive">请为每个任务填写名称、说明和至少一项验收标准。</p>}
      {canConfirm && feedback.trim() && <p className="text-xs text-muted-foreground">有新的补充要求，请先重新规划，再确认更新后的计划。</p>}
      <div className="flex flex-wrap items-center justify-end gap-2">
        <Button variant={canConfirm ? 'secondary' : 'primary'} disabled={busy || (goal.questions.length > 0 && !feedback.trim())} loading={planning} onClick={() => onPlan(feedback.trim())}>{!planning && (goal.status === 'draft' ? <Sparkles aria-hidden="true" className="h-4 w-4" /> : <RefreshCw aria-hidden="true" className="h-4 w-4" />)}{planning ? '正在生成任务计划…' : goal.status === 'draft' ? '生成任务计划' : goal.status === 'failed' ? '重试规划' : '重新规划'}</Button>
        {canConfirm && <Button loading={confirming} disabled={busy || invalidTasks || Boolean(feedback.trim())} onClick={() => onConfirm(tasks.map((task) => ({ ...task, title: task.title.trim(), description: task.description.trim(), acceptance: task.acceptance.map((item) => item.trim()).filter(Boolean) })))}>{!confirming && <CheckCircle2 aria-hidden="true" className="h-4 w-4" />}{confirming ? '正在保存计划…' : '确认任务计划'}</Button>}
      </div>
    </section>}
  </div>;
}
