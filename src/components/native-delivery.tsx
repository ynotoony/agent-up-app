// Input: 已确认的目标、任务依赖及后端保存的真实交付证据。
// Output: 隔离执行、验证审查状态、差异与用户本地收口；不自动发布。
// Pos: 原生目标详情的执行区；沿用既有设计 tokens 和组件。
import { useCallback, useEffect, useRef, useState } from 'react';
import { AlertCircle, CheckCircle2, ChevronDown, GitMerge, Loader2, Play, RefreshCw } from 'lucide-react';
import type { NativeDelivery, NativeGoal, VerificationCommand } from '../../shared/types';
import { api, errorMessage } from '@/lib/api';
import { Button } from './ui/button';
import { Badge } from './ui/badge';
import { AutoTextarea } from './ui/auto-textarea';

const running = (run: NativeDelivery) => ['running', 'verifying', 'reviewing'].includes(run.status);
const labels: Record<NativeDelivery['status'], string> = { running: '执行中', verifying: '验证中', reviewing: '独立审查中', awaiting_acceptance: '等待你验收', accepted: '已合入本地', failed: '未完成' };
const inputClass = 'w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-primary/30 disabled:opacity-60';

const stageState = (run: NativeDelivery, stage: 'execute' | 'verify' | 'review' | 'accept') => {
  if (run.status === 'failed') return 'failed';
  if (stage === 'execute') return run.status === 'running' ? 'active' : 'done';
  if (stage === 'verify') return run.status === 'running' ? 'pending' : run.checks.length > 0 ? (run.checks.every((check) => check.passed && check.exit_code === 0) ? 'done' : 'failed') : run.status === 'verifying' ? 'active' : 'pending';
  if (stage === 'review') return run.review ? (run.review.passed ? 'done' : 'failed') : run.status === 'reviewing' ? 'active' : 'pending';
  return run.status === 'accepted' ? 'done' : run.status === 'awaiting_acceptance' ? 'active' : 'pending';
};

function DeliveryStages({ run }: { run: NativeDelivery }) {
  const items = [
    ['execute', '隔离执行'],
    ['verify', '真实验证'],
    ['review', '独立审查'],
    ['accept', '用户验收'],
  ] as const;
  return <ol aria-label="交付阶段" className="flex min-w-0 flex-wrap items-center gap-y-2 text-xs">
    {items.map(([stage, label], index) => {
      const state = stageState(run, stage);
      return <li key={stage} className="flex items-center">
        <span className={`inline-flex items-center gap-1.5 whitespace-nowrap ${state === 'done' ? 'text-primary' : state === 'active' ? 'font-medium text-primary' : state === 'failed' ? 'text-destructive' : 'text-muted-foreground'}`}>
          <span className={`grid h-5 w-5 place-items-center rounded-full border text-[10px] ${state === 'done' ? 'border-primary bg-primary text-primary-foreground' : state === 'active' ? 'border-primary text-primary' : state === 'failed' ? 'border-destructive text-destructive' : 'border-border'}`}>
            {state === 'done' ? <CheckCircle2 aria-hidden="true" className="h-3.5 w-3.5" /> : state === 'active' ? <span className="h-1.5 w-1.5 rounded-full bg-current" /> : index + 1}
          </span>
          {label}
        </span>
        {index < items.length - 1 && <span aria-hidden="true" className="mx-2 h-px w-5 bg-border sm:w-8" />}
      </li>;
    })}
  </ol>;
}

export function NativeDeliveryPanel({ projectId, goal, onChanged }: { projectId: string; goal: NativeGoal; onChanged?: () => Promise<void> }) {
  const [runs, setRuns] = useState<NativeDelivery[]>([]);
  const [commandText, setCommandText] = useState('');
  const [runtimeId, setRuntimeId] = useState('');
  const [feedback, setFeedback] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const alive = useRef(false);
  const operation = useRef(false);
  const readVersion = useRef(0);

  const refresh = useCallback(async () => {
    const version = ++readVersion.current;
    try {
      const next = await api.deliveries.list(projectId, goal.id);
      if (alive.current && version === readVersion.current) setRuns(next);
    } catch (e) {
      if (alive.current && version === readVersion.current) setError(errorMessage(e));
    }
  }, [projectId, goal.id]);

  useEffect(() => {
    alive.current = true;
    Promise.all([api.deliveries.options(projectId), api.deliveries.list(projectId, goal.id)])
      .then(([options, items]) => {
        if (!alive.current) return;
        setCommandText(JSON.stringify(options.commands, null, 2));
        setRuntimeId(options.runtime_id);
        setRuns(items);
      }).catch((e) => { if (alive.current) setError(errorMessage(e)); })
      .finally(() => { if (alive.current) setLoading(false); });
    return () => { alive.current = false; readVersion.current++; };
  }, [projectId, goal.id]);

  const active = runs.some((run) => running(run));
  useEffect(() => {
    if (!active && !busy) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const tick = () => { timer = setTimeout(async () => { await refresh(); if (!stopped) tick(); }, 2500); };
    tick();
    return () => { stopped = true; clearTimeout(timer); };
  }, [active, busy, refresh]);

  let commands: VerificationCommand[] = [];
  let commandError = '';
  try {
    const parsed: unknown = JSON.parse(commandText || '[]');
    if (!Array.isArray(parsed) || parsed.length === 0 || parsed.some((c) => !c || typeof c.program !== 'string' || !c.program.trim() || !Array.isArray(c.args) || c.args.some((v: unknown) => typeof v !== 'string'))) throw new Error('请配置至少一条真实验证命令（program 与 args）。');
    commands = parsed;
  } catch (e) { commandError = e instanceof SyntaxError ? '验证命令格式不是有效 JSON。' : errorMessage(e); }

  const act = async (key: string, action: () => Promise<NativeDelivery>) => {
    if (operation.current) return;
    operation.current = true;
    readVersion.current++;
    setBusy(key);
    setError('');
    try {
      const result = await action();
      if (!alive.current) return;
      readVersion.current++;
      setRuns((items) => [result, ...items.filter((r) => r.id !== result.id)]);
      if (result.status === 'accepted') await onChanged?.();
    } catch (e) { if (alive.current) setError(errorMessage(e)); }
    finally { operation.current = false; if (alive.current) { setBusy(null); await refresh(); } }
  };

  const evidenceRevision = goal.status === 'completed' ? goal.revision - 1 : goal.revision;
  const current = runs.filter((r) => r.goal_revision === evidenceRevision);
  const accepted = new Set(current.filter((r) => r.status === 'accepted').map((r) => r.task_id));
  // A previous plan revision may still have retained evidence. It must not
  // block execution for the current confirmed plan.
  // A retained run from an older revision can still be mutating or awaiting
  // cleanup. Keep the whole workbench locked while any run is in-flight.
  const pending = runs.some((r) => running(r) || r.status === 'awaiting_acceptance');
  const locked = Boolean(busy) || pending;
  const nextTaskId = goal.status === 'ready' && !locked && !commandError
    ? goal.tasks.find((task) => !accepted.has(task.id) && task.depends_on.every((id) => accepted.has(id)))?.id
    : undefined;

  return <section className="space-y-5 border-t border-border pt-6" aria-label="任务执行与交付">
    <header className="flex items-center justify-between gap-3"><div><p className="text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground">{goal.status === 'completed' ? '完成凭证' : '执行与验收'}</p><h3 className="mt-1 text-lg font-semibold tracking-tight">{goal.status === 'completed' ? `${accepted.size}/${goal.tasks.length} 项已合入` : '按顺序完成任务'}</h3><p className="mt-1 text-sm text-muted-foreground">{goal.status === 'completed' ? '真实验证、独立审查和用户验收均已留存。' : '一次只推进一个当前任务，完成后再进入下一项。'}</p></div><Button size="sm" variant="ghost" disabled={Boolean(busy)} onClick={() => { setError(''); void refresh(); }} aria-label="刷新交付状态"><RefreshCw aria-hidden="true" className="h-4 w-4" /></Button></header>
    {error && <div role="alert" className="flex gap-2 rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive"><AlertCircle aria-hidden="true" className="h-4 w-4 shrink-0" /><span className="break-words">{error}</span></div>}
    {loading ? <p role="status" className="text-sm text-muted-foreground">正在读取执行配置…</p> : <>
      <details className="group border-y border-border py-3"><summary className="flex cursor-pointer list-none items-center justify-between gap-3 text-sm font-medium outline-none focus-visible:ring-2 focus-visible:ring-primary/30"><span>执行配置 <span className="font-normal text-muted-foreground">{runtimeId || '未配置'} · {commands.length} 条验证命令</span></span><ChevronDown aria-hidden="true" className="h-4 w-4 text-muted-foreground transition-transform group-open:rotate-180" /></summary><div className="mt-3 space-y-3 border-t border-border/70 pt-3"><label className="block text-xs font-medium">验证命令 JSON<span className="mt-1 block font-normal text-muted-foreground">命令会在隔离目录实际执行，全部通过后才进入独立审查。</span><AutoTextarea rows={7} maxAutoHeight={300} disabled={locked} className={`${inputClass} mt-2 font-mono text-xs`} value={commandText} onChange={(e) => setCommandText(e.target.value)} /></label>{commandError && <p role="alert" className="text-xs text-destructive">{commandError}</p>}<label className="block text-xs font-medium">执行补充说明（可选）<AutoTextarea rows={2} maxAutoHeight={180} disabled={locked} className={`${inputClass} mt-2`} value={feedback} onChange={(e) => setFeedback(e.target.value)} placeholder="例如：保留当前设计规范，只改本任务范围。" /></label></div></details>
      <div className="divide-y divide-border border-y border-border">{goal.tasks.map((task) => {
        const missing = task.depends_on.filter((id) => !accepted.has(id));
        const finished = accepted.has(task.id);
        const next = task.id === nextTaskId;
        const latest = [...current].sort((a,b) => b.started_at.localeCompare(a.started_at)).find((r) => r.task_id === task.id);
        const executable = goal.status === 'ready' && next;
        return <div key={task.id} className="py-3"><div className="flex flex-wrap items-center justify-between gap-3"><div className="min-w-0 flex-1"><div className="flex flex-wrap items-center gap-2"><p className="break-words text-sm font-medium">{task.title}</p>{next && !finished && <span className="text-xs font-medium text-primary">下一步</span>}</div><p className="mt-1 text-xs text-muted-foreground">{finished ? '已验收并合入本地' : goal.status === 'completed' ? '目标已完成' : missing.length ? `等待前置任务：${missing.map((id) => goal.tasks.find((t) => t.id === id)?.title ?? id).join('、')}` : latest ? labels[latest.status] : next ? '可以开始' : '排队中'}</p></div>{goal.status === 'completed' ? <span className="text-xs font-medium text-primary">已合入</span> : <Button size="sm" variant={finished ? 'secondary' : executable ? 'primary' : 'ghost'} disabled={finished || !executable} loading={busy === task.id} onClick={() => void act(task.id, () => api.deliveries.execute(projectId, goal.id, task.id, goal.revision, commands, feedback))}>{finished ? <CheckCircle2 aria-hidden="true" className="h-3.5 w-3.5" /> : latest?.status === 'failed' ? <RefreshCw aria-hidden="true" className="h-3.5 w-3.5" /> : <Play aria-hidden="true" className="h-3.5 w-3.5" />}{finished ? '已合入' : latest?.status === 'failed' ? '重新执行' : executable ? '开始任务' : latest && running(latest) ? '执行中' : '等待中'}</Button>}</div><details className="group mt-2"><summary className="flex cursor-pointer list-none items-center gap-2 py-1 text-xs text-muted-foreground outline-none focus-visible:ring-2 focus-visible:ring-primary/30"><span>任务说明与验收标准</span><span>· {task.acceptance.length} 项</span><ChevronDown aria-hidden="true" className="h-3.5 w-3.5 transition-transform group-open:rotate-180" /></summary><div className="mt-2 space-y-3 border-l border-border pl-3 text-sm leading-6"><p className="whitespace-pre-wrap break-words">{task.description || '暂无说明'}</p><ul className="space-y-1">{task.acceptance.map((item, index) => <li key={`${task.id}:${index}`} className="flex items-start gap-2"><CheckCircle2 aria-hidden="true" className="mt-1 h-3.5 w-3.5 shrink-0 text-primary" /><span className="whitespace-pre-wrap break-words">{item}</span></li>)}</ul></div></details></div>;
      })}</div>
      {busy && !active && <p role="status" className="flex gap-2 text-sm text-primary"><Loader2 aria-hidden="true" className="h-4 w-4 animate-spin" />正在处理，请稍候…</p>}
      {runs.length > 0 && <div className="space-y-3"><div><h4 className="text-sm font-semibold">{goal.status === 'completed' ? '完成凭证' : '交付证据'}</h4><p className="mt-1 text-xs text-muted-foreground">{goal.status === 'completed' ? '已完成目标的验收、验证与合入记录。' : '当前计划修订版的记录可参与验收；历史版本仅供追溯。'}</p></div>{[...runs].filter((run) => run.goal_revision === evidenceRevision).sort((a,b) => b.started_at.localeCompare(a.started_at)).map((run) => <DeliveryResult key={run.id} run={run} busy={Boolean(busy)} currentRevision={evidenceRevision} onAccept={() => void act(run.id, () => api.deliveries.accept(projectId, run.id, run.diff!.fingerprint))} onReject={() => void act(run.id, () => api.deliveries.reject(projectId, run.id))} />)}{runs.some((run) => run.goal_revision !== evidenceRevision) && <details className="group border-t border-border pt-3"><summary className="flex cursor-pointer list-none items-center justify-between gap-3 text-sm font-medium outline-none focus-visible:ring-2 focus-visible:ring-primary/30"><span>历史证据 <span className="font-normal text-muted-foreground">{runs.filter((run) => run.goal_revision !== evidenceRevision).length} 条旧修订记录</span></span><ChevronDown aria-hidden="true" className="h-4 w-4 text-muted-foreground transition-transform group-open:rotate-180" /></summary><div className="mt-3 divide-y divide-border border-y border-border">{[...runs].filter((run) => run.goal_revision !== evidenceRevision).sort((a,b) => b.started_at.localeCompare(a.started_at)).map((run) => <DeliveryResult key={run.id} run={run} busy={Boolean(busy)} currentRevision={evidenceRevision} onAccept={() => void act(run.id, () => api.deliveries.accept(projectId, run.id, run.diff!.fingerprint))} onReject={() => void act(run.id, () => api.deliveries.reject(projectId, run.id))} />)}</div></details>}</div>}
    </>}
  </section>;
}

function DeliveryResult({ run, busy, currentRevision, onAccept, onReject }: { run: NativeDelivery; busy: boolean; currentRevision: number; onAccept: () => void; onReject: () => void }) {
  const canAccept = run.status === 'awaiting_acceptance' && run.goal_revision === currentRevision && Boolean(run.diff) && run.review?.passed && run.checks.length > 0 && run.checks.every((c) => c.passed && c.exit_code === 0);
  const acceptanceBlocker = run.status === 'awaiting_acceptance' && !canAccept
    ? run.goal_revision !== currentRevision ? '这条记录属于旧版计划，不能合入当前计划。'
      : !run.diff ? '缺少代码差异证据，不能验收。'
        : !run.review?.passed ? '独立审查尚未通过，不能验收。'
          : run.checks.length === 0 || !run.checks.every((c) => c.passed && c.exit_code === 0) ? '真实验证未全部通过，不能验收。'
            : '验收条件尚未满足。'
    : '';
  return <article className="space-y-4 border-b border-border py-5 first:pt-0 last:border-b-0">
    <div className="flex flex-wrap items-start justify-between gap-2"><div className="min-w-0"><p className="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">任务交付</p><h4 className="mt-1 break-words text-sm font-semibold">{run.task_title}</h4></div><Badge className={run.status === 'failed' ? 'bg-destructive/10 text-destructive' : 'bg-primary/10 text-primary'}>{running(run) && <Loader2 aria-hidden="true" className="mr-1 h-3 w-3 animate-spin" />}{labels[run.status]}</Badge></div>
    <p className="text-xs text-muted-foreground">{new Date(run.started_at).toLocaleString('zh-CN')} · {run.runtime_id}{run.goal_revision !== currentRevision ? ' · 旧版计划记录' : ''}</p>
    <DeliveryStages run={run} />
    {running(run) && <p role="status" className="text-sm text-primary">{run.status === 'running' ? 'Agent 正在隔离目录完成任务。' : run.status === 'verifying' ? 'App 正在运行验证命令。' : '独立 Agent 正在检查改动与验收结果。'}可以离开页面，记录会保存。</p>}
    {run.error && <div role="alert" className="flex items-start gap-2 border-l-2 border-destructive pl-3 text-sm text-destructive"><AlertCircle aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0" /><p className="whitespace-pre-wrap break-words">{run.error}</p></div>}
    <div className="divide-y divide-border border-y border-border">
      {run.worktree && <details className="group py-3"><summary className="flex cursor-pointer list-none items-center justify-between gap-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-primary/30"><span>隔离工作目录</span><ChevronDown aria-hidden="true" className="h-4 w-4 text-muted-foreground transition-transform group-open:rotate-180" /></summary><p className="mt-2 break-all font-mono text-xs text-muted-foreground">{run.worktree.path}</p></details>}
      {run.diff && <details className="group py-3"><summary className="flex cursor-pointer list-none items-center justify-between gap-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-primary/30"><span>代码差异 <span className="text-xs text-muted-foreground">{run.diff.files.length} 个文件</span></span><ChevronDown aria-hidden="true" className="h-4 w-4 text-muted-foreground transition-transform group-open:rotate-180" /></summary><div className="mt-2 space-y-2"><p className="break-words text-xs text-muted-foreground">{run.diff.files.join('、')}</p><pre className="max-h-96 overflow-auto rounded-lg bg-muted p-3 text-xs leading-relaxed">{run.diff.stat}{'\n'}{run.diff.patch}</pre></div></details>}
      {run.checks.length > 0 && <details className="group py-3"><summary className="flex cursor-pointer list-none items-center justify-between gap-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-primary/30"><span>真实验证 <span className={run.checks.every((check) => check.passed && check.exit_code === 0) ? 'text-primary' : 'text-destructive'}>{run.checks.filter((check) => check.passed && check.exit_code === 0).length}/{run.checks.length} 通过</span></span><ChevronDown aria-hidden="true" className="h-4 w-4 text-muted-foreground transition-transform group-open:rotate-180" /></summary><div className="mt-2 divide-y divide-border border-y border-border">{run.checks.map((check,index) => <details key={index} className="py-3 first:pt-0 last:pb-0"><summary className="cursor-pointer break-all text-xs"><span className={check.passed ? 'text-primary' : 'text-destructive'}>{check.passed ? '通过' : '失败'}</span> · {check.program} {check.args.join(' ')} · 退出码 {check.exit_code ?? '未正常退出'}</summary><pre className="mt-2 max-h-60 overflow-auto whitespace-pre-wrap break-words text-xs text-muted-foreground">{check.output || '无输出'}</pre></details>)}</div></details>}
      {run.review && <details className="group py-3" open={!run.review.passed}><summary className="flex cursor-pointer list-none items-center justify-between gap-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-primary/30"><span>独立审查 <span className={run.review.passed ? 'text-primary' : 'text-destructive'}>{run.review.passed ? '通过' : '未通过'}</span></span><ChevronDown aria-hidden="true" className="h-4 w-4 text-muted-foreground transition-transform group-open:rotate-180" /></summary><div className="mt-2 space-y-2 text-sm leading-6"><p>{run.review.summary}</p>{run.review.findings.length > 0 && <ul className="list-disc space-y-1 pl-5 text-xs text-muted-foreground">{run.review.findings.map((finding,index) => <li key={index}>{finding}</li>)}</ul>}</div></details>}
    </div>
    {run.status === 'awaiting_acceptance' && <div className="space-y-3 border-t border-border pt-4"><p className="text-xs leading-5 text-muted-foreground">确认结果符合目标后，App 将创建本地提交并合入原分支。尚未验收的改动只留在隔离目录。</p>{acceptanceBlocker && <p role="status" className="text-xs text-destructive">{acceptanceBlocker}</p>}<div className="flex flex-wrap justify-end gap-2"><Button size="sm" variant="ghost" disabled={busy} onClick={onReject}>暂不采纳（保留目录）</Button><Button disabled={busy || !canAccept} onClick={onAccept}><GitMerge aria-hidden="true" className="h-4 w-4" />验收并合入本地</Button></div></div>}
    {run.commit && <p className="break-all font-mono text-xs text-primary">本地提交：{run.commit}</p>}
    {run.status === 'accepted' && run.error && run.diff && <div className="flex justify-end"><Button variant="secondary" disabled={busy} onClick={onAccept}><RefreshCw aria-hidden="true" className="h-4 w-4" />重试记录归档与清理</Button></div>}
  </article>;
}
