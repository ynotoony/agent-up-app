// Input: 已确认的目标、任务依赖及后端保存的真实交付证据。
// Output: 隔离执行、验证审查状态、差异与用户本地收口；不自动发布。
// Pos: 原生目标详情的执行区；沿用既有设计 tokens 和组件。
import { useCallback, useEffect, useRef, useState } from 'react';
import { AlertCircle, CheckCircle2, GitMerge, Loader2, Play, RefreshCw } from 'lucide-react';
import type { NativeDelivery, NativeGoal, VerificationCommand } from '../../shared/types';
import { api, errorMessage } from '@/lib/api';
import { Button } from './ui/button';
import { Badge } from './ui/badge';

const running = (run: NativeDelivery) => ['running', 'verifying', 'reviewing'].includes(run.status);
const labels: Record<NativeDelivery['status'], string> = { running: '执行中', verifying: '验证中', reviewing: '独立审查中', awaiting_acceptance: '等待你验收', accepted: '已合入本地', failed: '未完成' };
const inputClass = 'w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-primary/30 disabled:opacity-60';

export function NativeDeliveryPanel({ projectId, goal }: { projectId: string; goal: NativeGoal }) {
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

  const active = runs.some(running);
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
    } catch (e) { if (alive.current) setError(errorMessage(e)); }
    finally { operation.current = false; if (alive.current) { setBusy(null); await refresh(); } }
  };

  const current = runs.filter((r) => r.goal_revision === goal.revision);
  const accepted = new Set(current.filter((r) => r.status === 'accepted').map((r) => r.task_id));
  const pending = runs.some((r) => running(r) || r.status === 'awaiting_acceptance');
  const locked = Boolean(busy) || pending;

  return <section className="space-y-4" aria-label="任务执行与交付">
    <header className="flex items-start justify-between gap-3"><div><h3 className="font-medium">执行与交付</h3><p className="mt-1 text-xs leading-5 text-muted-foreground">按依赖在隔离目录执行，验证、审查通过后由你验收合入。不会推送远端。</p></div><Button size="sm" variant="ghost" disabled={Boolean(busy)} onClick={() => { setError(''); void refresh(); }} aria-label="刷新交付状态"><RefreshCw aria-hidden="true" className="h-4 w-4" /></Button></header>
    {error && <div role="alert" className="flex gap-2 rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive"><AlertCircle aria-hidden="true" className="h-4 w-4 shrink-0" /><span className="break-words">{error}</span></div>}
    {loading ? <p role="status" className="text-sm text-muted-foreground">正在读取执行配置…</p> : <>
      <div className="space-y-3 rounded-xl border border-border bg-card p-5">
        <p className="text-xs text-muted-foreground">执行 Agent：{runtimeId || '未配置'} · {accepted.size}/{goal.tasks.length} 项已合入</p>
        <details><summary className="cursor-pointer text-sm focus-visible:ring-2 focus-visible:ring-primary/30">验证命令（{commands.length} 项）</summary><p className="my-2 text-xs text-muted-foreground">这些命令会在隔离目录实际执行，全部通过才进入审查。无需填写 shell 拼接符。</p><label className="block"><span className="sr-only">验证命令 JSON</span><textarea rows={7} disabled={locked} className={`${inputClass} font-mono text-xs`} value={commandText} onChange={(e) => setCommandText(e.target.value)} /></label></details>
        {commandError && <p role="alert" className="text-xs text-destructive">{commandError}</p>}
        <label className="block text-xs font-medium">执行补充说明（可选）<textarea rows={2} disabled={locked} className={`${inputClass} mt-2`} value={feedback} onChange={(e) => setFeedback(e.target.value)} placeholder="例如：保留当前设计规范，只改本任务范围。" /></label>
      </div>
      <div className="divide-y divide-border overflow-hidden rounded-xl border border-border bg-card">{goal.tasks.map((task) => {
        const missing = task.depends_on.filter((id) => !accepted.has(id));
        const finished = accepted.has(task.id);
        const latest = [...current].sort((a,b) => b.started_at.localeCompare(a.started_at)).find((r) => r.task_id === task.id);
        return <div key={task.id} className="flex flex-wrap items-center justify-between gap-3 p-4"><div className="min-w-0 flex-1"><p className="text-sm font-medium">{task.title}</p><p className="mt-1 text-xs text-muted-foreground">{finished ? '已验收并合入本地' : missing.length ? `等待前置任务：${missing.map((id) => goal.tasks.find((t) => t.id === id)?.title ?? id).join('、')}` : latest ? labels[latest.status] : '可以执行'}</p></div><Button size="sm" variant={finished ? 'secondary' : 'primary'} disabled={finished || missing.length > 0 || locked || Boolean(commandError)} loading={busy === task.id} onClick={() => void act(task.id, () => api.deliveries.execute(projectId, goal.id, task.id, goal.revision, commands, feedback))}>{finished ? <CheckCircle2 aria-hidden="true" className="h-3.5 w-3.5" /> : <Play aria-hidden="true" className="h-3.5 w-3.5" />}{finished ? '已合入' : latest?.status === 'failed' ? '重新执行' : '执行任务'}</Button></div>;
      })}</div>
      {busy && !active && <p role="status" className="flex gap-2 text-sm text-primary"><Loader2 aria-hidden="true" className="h-4 w-4 animate-spin" />正在处理，请稍候…</p>}
      {[...runs].sort((a,b) => b.started_at.localeCompare(a.started_at)).map((run) => <DeliveryResult key={run.id} run={run} busy={Boolean(busy)} currentRevision={goal.revision} onAccept={() => void act(run.id, () => api.deliveries.accept(projectId, run.id, run.diff!.fingerprint))} onReject={() => void act(run.id, () => api.deliveries.reject(projectId, run.id))} />)}
    </>}
  </section>;
}

function DeliveryResult({ run, busy, currentRevision, onAccept, onReject }: { run: NativeDelivery; busy: boolean; currentRevision: number; onAccept: () => void; onReject: () => void }) {
  const canAccept = run.status === 'awaiting_acceptance' && run.goal_revision === currentRevision && Boolean(run.diff) && run.review?.passed && run.checks.length > 0 && run.checks.every((c) => c.passed && c.exit_code === 0);
  return <article className="space-y-3 rounded-xl border border-border bg-card p-5">
    <div className="flex flex-wrap items-center justify-between gap-2"><h4 className="text-sm font-medium">{run.task_title}</h4><Badge className={run.status === 'failed' ? 'bg-destructive/10 text-destructive' : 'bg-primary/10 text-primary'}>{running(run) && <Loader2 aria-hidden="true" className="mr-1 h-3 w-3 animate-spin" />}{labels[run.status]}</Badge></div>
    <p className="text-xs text-muted-foreground">{new Date(run.started_at).toLocaleString('zh-CN')} · {run.runtime_id}{run.goal_revision !== currentRevision ? ' · 旧版计划记录' : ''}</p>
    {running(run) && <p role="status" className="text-sm text-primary">{run.status === 'running' ? 'Agent 正在隔离目录完成任务。' : run.status === 'verifying' ? 'App 正在运行验证命令。' : '独立 Agent 正在检查改动与验收结果。'}可以离开页面，记录会保存。</p>}
    {run.error && <p role="alert" className="whitespace-pre-wrap break-words text-sm text-destructive">{run.error}</p>}
    {run.worktree && <details><summary className="cursor-pointer text-xs text-muted-foreground">隔离工作目录</summary><p className="mt-2 break-all font-mono text-xs text-muted-foreground">{run.worktree.path}</p></details>}
    {run.diff && <details><summary className="cursor-pointer text-sm">改动文件（{run.diff.files.length}）与代码差异</summary><p className="mt-2 break-words text-xs text-muted-foreground">{run.diff.files.join('、')}</p><pre className="mt-2 max-h-96 overflow-auto rounded-lg bg-muted p-3 text-xs leading-relaxed">{run.diff.stat}{'\n'}{run.diff.patch}</pre></details>}
    {run.checks.length > 0 && <div className="space-y-2"><h5 className="text-xs font-medium">真实验证结果</h5>{run.checks.map((check,index) => <details key={index} className="rounded-lg border border-border p-3"><summary className="cursor-pointer break-all text-xs"><span className={check.passed ? 'text-primary' : 'text-destructive'}>{check.passed ? '通过' : '失败'}</span> · {check.program} {check.args.join(' ')} · 退出码 {check.exit_code ?? '未正常退出'}</summary><pre className="mt-2 max-h-60 overflow-auto whitespace-pre-wrap break-words text-xs text-muted-foreground">{check.output || '无输出'}</pre></details>)}</div>}
    {run.review && <div className="space-y-2 border-t border-border pt-3"><h5 className="text-xs font-medium">独立审查：{run.review.passed ? '通过' : '未通过'}</h5><p className="text-sm leading-6">{run.review.summary}</p>{run.review.findings.length > 0 && <ul className="list-disc space-y-1 pl-5 text-xs text-muted-foreground">{run.review.findings.map((finding,index) => <li key={index}>{finding}</li>)}</ul>}</div>}
    {run.status === 'awaiting_acceptance' && <div className="space-y-3 border-t border-border pt-4"><p className="text-xs leading-5 text-muted-foreground">确认结果符合目标后，App 将创建本地提交并合入原分支。尚未验收的改动只留在隔离目录。</p><div className="flex flex-wrap justify-end gap-2"><Button size="sm" variant="ghost" disabled={busy} onClick={onReject}>暂不采纳（保留目录）</Button><Button disabled={busy || !canAccept} onClick={onAccept}><GitMerge aria-hidden="true" className="h-4 w-4" />验收并合入本地</Button></div></div>}
    {run.commit && <p className="break-all font-mono text-xs text-primary">本地提交：{run.commit}</p>}
    {run.status === 'accepted' && run.error && run.diff && <div className="flex justify-end"><Button variant="secondary" disabled={busy} onClick={onAccept}><RefreshCw aria-hidden="true" className="h-4 w-4" />重试记录归档与清理</Button></div>}
  </article>;
}
