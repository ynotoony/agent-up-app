// Input: 已接入项目 ID、用户目标、真实规划结果与用户确认。
// Output: App 原生目标的创建、规划进度、任务编辑与确认；不启动实施。
// Pos: 项目工作区的目标面板，沿用既有组件与设计 tokens；src/ 目录登记豁免。
import { useCallback, useEffect, useRef, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { AlertCircle, CheckCircle2, ChevronRight, Loader2, Plus, RefreshCw, Sparkles, Trash2 } from 'lucide-react';
import type { NativeGoal, NativeTask } from '../../shared/types';
import { api, errorMessage } from '@/lib/api';
import { cn } from '@/lib/utils';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { NativeDeliveryPanel } from './native-delivery';

const inputClass = 'w-full rounded-lg border border-input bg-background px-3 py-2 text-sm leading-relaxed text-foreground outline-none focus-visible:border-primary focus-visible:ring-2 focus-visible:ring-primary/30 disabled:opacity-60';
const statusLabels: Record<NativeGoal['status'], string> = {
  draft: '待规划', planning: '方案制定中', awaiting_confirmation: '待确认', ready: '计划已确认', failed: '规划失败',
};

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
      {showComposer && <form onSubmit={(event) => { event.preventDefault(); void create(); }} className="space-y-4 rounded-xl border border-border bg-card p-5">
        <div><h3 className="font-medium">新建目标</h3><p id="goal-create-help" className="mt-1 text-sm text-muted-foreground">描述你希望完成什么。先保存目标，再让 Agent 只读分析项目、拆分任务。</p></div>
        <label htmlFor="native-goal-content" className="sr-only">目标描述</label>
        <textarea id="native-goal-content" autoFocus rows={3} maxLength={12000} value={content} onChange={(event) => setContent(event.target.value)} disabled={creating} aria-describedby="goal-create-help" placeholder="例如：让工作台显示每个项目的目标进度" className={cn(inputClass, 'resize-y')} />
        {content.trim() && Array.from(content.trim()).length < 4 && <p className="text-xs text-muted-foreground">请至少输入 4 个字，描述你想完成的事。</p>}
        <div className="flex items-center justify-end gap-2">
          {goals.length > 0 && <Button type="button" variant="ghost" disabled={creating} onClick={() => setComposing(false)}>返回目标</Button>}
          <Button type="submit" loading={creating} disabled={Array.from(content.trim()).length < 4}><Plus aria-hidden="true" className="h-4 w-4" />保存目标</Button>
        </div>
      </form>}

      {!showComposer && goals.length > 0 && <div className="grid grid-cols-1 items-start gap-5 lg:grid-cols-3">
        <nav aria-label="目标列表" className="space-y-2 lg:sticky lg:top-0">
          {goals.map((goal) => <button key={goal.id} type="button" aria-current={selectedId === goal.id ? 'true' : undefined} onClick={() => { setSelectedId(goal.id); setError(''); }} className={cn('w-full rounded-xl border bg-card p-4 text-left outline-none transition-colors focus-visible:ring-2 focus-visible:ring-primary/30', selectedId === goal.id ? 'border-primary/40 bg-primary/5' : 'border-border hover:bg-accent')}>
            <div className="mb-2 flex items-center justify-between gap-2"><GoalStatus goal={goal} pending={pendingAction} /><ChevronRight aria-hidden="true" className="h-4 w-4 shrink-0 text-muted-foreground" /></div>
            <p className="line-clamp-3 break-words text-sm font-medium leading-relaxed">{goal.content}</p>
            <p className="mt-2 text-xs text-muted-foreground">{goal.tasks.length > 0 ? `${goal.tasks.length} 个任务 · ` : ''}{new Date(goal.updated_at).toLocaleDateString('zh-CN')}</p>
          </button>)}
        </nav>
        {selected && <GoalDetail key={selected.id} projectId={projectId} goal={selected} pending={pendingAction?.goalId === selected.id ? pendingAction : null} busy={pendingAction !== null || selected.status === 'planning'} onPlan={(feedback) => void act(selected, 'plan', () => api.goals.plan(projectId, selected.id, feedback))} onConfirm={(tasks) => void act(selected, 'confirm', () => api.goals.confirm(projectId, selected.id, selected.revision, tasks))} />}
      </div>}
    </>}
  </section>;
}

function GoalDetail({ projectId, goal, pending, busy, onPlan, onConfirm }: { projectId: string; goal: NativeGoal; pending: PendingAction | null; busy: boolean; onPlan: (feedback: string) => void; onConfirm: (tasks: NativeTask[]) => void }) {
  const [feedback, setFeedback] = useState('');
  const [tasks, setTasks] = useState<NativeTask[]>(() => goal.tasks.map((task) => ({ ...task })));
  const [planRevision, setPlanRevision] = useState(goal.revision);
  // Only a new completed plan replaces edits; polling, startup and failure keep the draft.
  if ((goal.status === 'awaiting_confirmation' || goal.status === 'ready') && goal.revision !== planRevision) {
    setPlanRevision(goal.revision);
    setTasks(goal.tasks.map((task) => ({ ...task })));
    setFeedback('');
  }
  const planning = pending?.kind === 'plan' || goal.status === 'planning';
  const confirming = pending?.kind === 'confirm';
  const startedAt = goal.status === 'planning' && goal.run ? Date.parse(goal.run.started_at) : pending?.startedAt;
  const ready = goal.status === 'ready';
  const canConfirm = goal.status === 'awaiting_confirmation' && goal.questions.length === 0 && tasks.length > 0;
  const invalidTasks = tasks.some((task) => !task.title.trim() || !task.description.trim() || !task.acceptance.some((item) => item.trim()));
  const editTask = (id: string, update: Partial<NativeTask>) => setTasks((items) => items.map((task) => task.id === id ? { ...task, ...update } : task));

  return <div className="min-w-0 space-y-4 lg:col-span-2">
    <section className="space-y-4 rounded-xl border border-border bg-card p-5">
      <div className="flex flex-wrap items-center justify-between gap-2"><h3 className="font-medium">当前目标</h3><GoalStatus goal={goal} pending={pending} /></div>
      <p className="whitespace-pre-wrap break-words text-sm leading-relaxed">{goal.content}</p>
      {goal.summary && <div className="border-t border-border pt-4"><h4 className="mb-2 text-xs font-medium text-muted-foreground">计划概述</h4><p className="whitespace-pre-wrap text-sm leading-relaxed">{goal.summary}</p></div>}
      {goal.run && <p className="text-xs text-muted-foreground">规划 Agent：{goal.run.runtime_id} · {new Date(goal.run.started_at).toLocaleString('zh-CN')}{goal.run.tokens != null ? ` · ${goal.run.tokens.toLocaleString()} tokens` : ''}</p>}
      {goal.error && <div className="space-y-2"><p role="alert" className="whitespace-pre-wrap break-words text-sm text-destructive">{goal.error}</p><Link to="/settings" className="rounded text-xs text-primary underline underline-offset-4 focus-visible:ring-2 focus-visible:ring-primary/30">检查 Agent 设置</Link></div>}
      {planning && <div className="space-y-1 text-sm text-primary"><p role="status" className="flex items-center gap-2"><Loader2 aria-hidden="true" className="h-4 w-4 animate-spin" />{goal.status === 'planning' ? 'Agent 正在只读分析项目、制定任务计划。' : '正在请求 Agent 规划…'}</p><p className="text-xs text-muted-foreground">{startedAt != null && Number.isFinite(startedAt) && <><PlanningElapsed startedAt={startedAt} /> · </>}规划可能需要几分钟，可以离开此页，结果会自动保存。</p></div>}
      {confirming && <p role="status" className="flex items-center gap-2 text-sm text-primary"><Loader2 aria-hidden="true" className="h-4 w-4 animate-spin" />正在保存确认的任务计划…</p>}
      {ready && <div role="status" className="flex items-start gap-2 rounded-lg bg-primary/10 p-3 text-sm text-primary"><CheckCircle2 aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0" /><p>任务计划已确认。可以在下方按依赖执行任务，验证与审查通过后再验收合入。</p></div>}
    </section>

    {goal.questions.length > 0 && <section className="rounded-xl border border-border bg-card p-5">
      <h3 className="font-medium">需要你明确</h3>
      <ol className="mt-3 list-decimal space-y-2 pl-5 text-sm leading-relaxed">{goal.questions.map((question, index) => <li key={`${index}:${question}`}>{question}</li>)}</ol>
      <p className="mt-3 text-xs text-muted-foreground">在下方补充答案，再生成计划。问题解决后才能确认任务。</p>
    </section>}

    {(tasks.length > 0 || goal.status === 'awaiting_confirmation') && <section className="overflow-hidden rounded-xl border border-border bg-card">
      <div className="flex items-center justify-between gap-3 border-b border-border bg-muted/50 px-5 py-3"><h3 className="text-sm font-medium">任务计划 · {tasks.length} 项</h3>{!ready && <span className="text-xs text-muted-foreground">确认前可编辑</span>}</div>
      <div className="divide-y divide-border">
        {tasks.map((task, index) => <div key={task.id} className="space-y-3 p-5">
          <div className="flex items-start gap-2"><span className="pt-2 text-sm text-muted-foreground">{index + 1}.</span><label className="min-w-0 flex-1"><span className="sr-only">任务 {index + 1} 名称</span><input value={task.title} disabled={busy || ready} onChange={(event) => editTask(task.id, { title: event.target.value })} className={inputClass} /></label>{!ready && <Button size="sm" variant="ghost" disabled={busy} aria-label={`删除任务 ${index + 1}`} onClick={() => setTasks((items) => items.filter((item) => item.id !== task.id).map((item) => ({ ...item, depends_on: item.depends_on.filter((id) => id !== task.id) })))}><Trash2 aria-hidden="true" className="h-4 w-4" /></Button>}</div>
          <label className="block text-xs font-medium text-muted-foreground">要完成什么<textarea rows={2} value={task.description} disabled={busy || ready} onChange={(event) => editTask(task.id, { description: event.target.value })} className={cn(inputClass, 'mt-1 resize-y font-normal')} /></label>
          <label className="block text-xs font-medium text-muted-foreground">验收标准（每行一项）<textarea rows={2} value={task.acceptance.join('\n')} disabled={busy || ready} onChange={(event) => editTask(task.id, { acceptance: event.target.value.split('\n') })} className={cn(inputClass, 'mt-1 resize-y font-normal')} /></label>
          {task.capabilities.length > 0 && <p className="text-xs text-muted-foreground">需要的能力：{task.capabilities.join('、')}</p>}
          {tasks.length > 1 && <details className="text-xs text-muted-foreground"><summary className="cursor-pointer rounded py-1 outline-none focus-visible:ring-2 focus-visible:ring-primary/30">前置任务：{task.depends_on.length ? task.depends_on.map((id) => tasks.find((item) => item.id === id)?.title || id).join('、') : '无，可独立开始'}</summary><fieldset disabled={busy || ready} className="mt-2 space-y-2"><legend className="sr-only">选择任务 {index + 1} 的前置任务</legend>{tasks.filter((item) => item.id !== task.id).map((item) => <label key={item.id} className="flex items-start gap-2"><input type="checkbox" checked={task.depends_on.includes(item.id)} onChange={(event) => editTask(task.id, { depends_on: event.target.checked ? [...task.depends_on, item.id] : task.depends_on.filter((id) => id !== item.id) })} className="mt-0.5 accent-primary" /><span className="break-words">{item.title}</span></label>)}</fieldset></details>}
        </div>)}
      </div>
      {!ready && <div className="border-t border-border px-5 py-3"><Button variant="ghost" size="sm" disabled={busy} onClick={() => setTasks((items) => [...items, { id: `task-${crypto.randomUUID()}`, title: '', description: '', acceptance: [''], depends_on: [], capabilities: [] }])}><Plus aria-hidden="true" className="h-4 w-4" />增加任务</Button></div>}
    </section>}

    {ready && <NativeDeliveryPanel key={`${projectId}:${goal.id}:${goal.revision}`} projectId={projectId} goal={goal} />}

    {!ready && <section className="space-y-3 rounded-xl border border-border bg-card p-5">
      <label htmlFor={`goal-feedback-${goal.id}`} className="block text-sm font-medium">{goal.questions.length > 0 ? '补充答案' : '补充要求（可选）'}</label>
      <textarea id={`goal-feedback-${goal.id}`} rows={2} maxLength={12000} value={feedback} disabled={busy} onChange={(event) => setFeedback(event.target.value)} placeholder={goal.questions.length > 0 ? '逐项回答上面的疑问…' : '例如：先做最小版本，保留现有设计规范…'} className={cn(inputClass, 'resize-y')} />
      {goal.status === 'draft' && <p className="text-xs text-muted-foreground">规划 Agent 只读分析项目，返回任务、依赖和验收标准。此步骤不修改代码。</p>}
      {canConfirm && invalidTasks && <p className="text-xs text-destructive">请为每个任务填写名称、说明和至少一项验收标准。</p>}
      {canConfirm && feedback.trim() && <p className="text-xs text-muted-foreground">有新的补充要求，请先重新规划，再确认更新后的计划。</p>}
      <div className="flex flex-wrap justify-end gap-2">
        <Button variant={canConfirm ? 'secondary' : 'primary'} disabled={busy || (goal.questions.length > 0 && !feedback.trim())} loading={planning} onClick={() => onPlan(feedback.trim())}>{!planning && (goal.status === 'draft' ? <Sparkles aria-hidden="true" className="h-4 w-4" /> : <RefreshCw aria-hidden="true" className="h-4 w-4" />)}{planning ? '正在生成任务计划…' : goal.status === 'draft' ? '生成任务计划' : goal.status === 'failed' ? '重试规划' : '重新规划'}</Button>
        {canConfirm && <Button loading={confirming} disabled={busy || invalidTasks || Boolean(feedback.trim())} onClick={() => onConfirm(tasks.map((task) => ({ ...task, title: task.title.trim(), description: task.description.trim(), acceptance: task.acceptance.map((item) => item.trim()).filter(Boolean) })))}>{!confirming && <CheckCircle2 aria-hidden="true" className="h-4 w-4" />}{confirming ? '正在保存计划…' : '确认任务计划'}</Button>}
      </div>
    </section>}
  </div>;
}
