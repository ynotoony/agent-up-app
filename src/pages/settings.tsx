// Input: 本机 Agent 探测、已保存默认选项与历史自动化编排。
// Output: 交互 Agent 默认选择、明确触发的健康检测与折叠的兼容设置。
// Pos: 工作台设置页；沿用语义 tokens，src/ 目录登记豁免。
import { useCallback, useEffect, useRef, useState } from 'react';
import { AlertCircle, Check, ChevronDown, FileCog, Loader2, RefreshCw, RotateCcw, Save, Terminal, X } from 'lucide-react';
import type { OrchestrationSettings, RoleInfo } from '../../shared/types';
import { api, errorMessage, type RuntimeInfo } from '@/lib/api';
import { cn } from '@/lib/utils';
import { Button } from '@/components/ui/button';
import { Select, SelectChevron } from '@/components/ui/select';
import { AutoTextarea } from '@/components/ui/auto-textarea';
import { toast } from 'sonner';

const STAGES = [
  { key: 'understand', label: '需求理解' }, { key: 'plan', label: '方案规划' },
  { key: 'implement', label: '代码实施' }, { key: 'verify', label: '验证审查' }, { key: 'review', label: '交付审查' },
];
const interactiveIds = new Set(['codex-cli', 'opencode', 'zcode-app']);

export default function SettingsPage() {
  const [runtimes, setRuntimes] = useState<RuntimeInfo[]>([]);
  const [selected, setSelected] = useState('');
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const version = useRef(0);
  const mutation = useRef(false);
  const fetchData = useCallback(async () => {
    const current = ++version.current;
    setLoading(true); setError('');
    try {
      const [installed, preferred] = await Promise.all([api.runtimes.list(), api.runtimes.getSelected()]);
      if (current !== version.current) return;
      setRuntimes(installed); setSelected(preferred);
    } catch (failure) {
      if (current === version.current) setError(errorMessage(failure));
    } finally {
      if (current === version.current) setLoading(false);
    }
  }, []);
  useEffect(() => { void fetchData(); return () => { version.current += 1; }; }, [fetchData]);
  const chooseDefault = async (id: string) => {
    if (mutation.current || loading) return;
    mutation.current = true; setSaving(true); setError('');
    const current = ++version.current;
    try {
      const saved = await api.runtimes.setSelected(id);
      if (current === version.current) { setSelected(saved); toast.success('默认 Agent 已保存'); }
    } catch (failure) {
      if (current === version.current) setError(errorMessage(failure));
    } finally {
      mutation.current = false;
      if (current === version.current) setSaving(false);
    }
  };
  const agents = runtimes.filter((runtime) => interactiveIds.has(runtime.id));
  const preferred = agents.find((runtime) => runtime.id === selected);
  return <main id="main-content" tabIndex={-1} className="h-screen overflow-y-auto scrollbar-thin">
    <div data-tauri-drag-region className="drag-region h-3" />
    <div className="mx-auto max-w-6xl px-6 py-7 pb-16 sm:px-10">
      <header className="mb-10 flex flex-wrap items-start justify-between gap-5"><div><p className="text-xs font-medium tracking-widest text-muted-foreground">偏好设置 / 本机环境</p><h1 className="mt-3 text-3xl font-semibold tracking-tight">你的 Agent</h1><p className="mt-3 text-sm leading-6 text-muted-foreground">选择打开任务时使用的 Agent，确认本机环境可以工作。</p></div><Button variant="secondary" loading={loading} disabled={saving} onClick={() => void fetchData()}><RefreshCw aria-hidden className="h-4 w-4" />重新检测安装</Button></header>
      {error && <p role="alert" className="mb-6 flex items-start gap-2 border-l-2 border-destructive pl-4 text-sm leading-6 text-destructive"><AlertCircle aria-hidden className="mt-1 h-4 w-4 shrink-0" />{error}</p>}
      <section aria-labelledby="default-agent" className="border-y border-border bg-card px-5 py-7 sm:px-8">
        <div className="grid gap-7 lg:grid-cols-[minmax(0,0.32fr)_minmax(0,0.68fr)] lg:gap-12">
          <div><p className="text-xs font-medium text-muted-foreground">默认启动方式</p><h2 id="default-agent" className="mt-3 text-lg font-semibold">工作交给谁</h2><p className="mt-3 text-sm leading-6 text-muted-foreground">从真实任务打开交互会话。任务上下文交给所选 Agent，你在它的窗口里继续工作。</p></div>
          <div className="min-w-0"><label htmlFor="default-runtime" className="block text-sm font-medium">默认 Agent</label><div className="relative mt-3 max-w-md"><Select id="default-runtime" className="h-11 w-full pr-9" disabled={loading || saving || !agents.some((runtime) => runtime.available)} value={selected} onChange={(event) => void chooseDefault(event.target.value)}>{!selected && <option value="">正在读取…</option>}{selected && !agents.some((runtime) => runtime.id === selected) && <option value={selected} disabled>{selected}（暂不支持交互启动）</option>}{agents.map((runtime) => <option key={runtime.id} value={runtime.id} disabled={!runtime.available}>{runtime.name}{runtime.available ? '' : ' · 未安装'}</option>)}</Select><SelectChevron /></div>{saving ? <p role="status" className="mt-3 text-xs text-muted-foreground">正在保存默认选项…</p> : preferred && !preferred.available ? <p role="status" className="mt-3 text-sm text-destructive">默认 Agent 当前不可用。请安装后重新检测，或选择已安装的 Agent。</p> : <p className="mt-3 text-xs leading-5 text-muted-foreground">在单次启动前也可临时选择其他已安装 Agent。</p>}</div>
        </div>
        <div className="mt-8 border-t border-border pt-6"><div className="mb-2 flex flex-wrap items-center justify-between gap-3"><h2 className="text-sm font-semibold">安装与健康状态</h2><span className="text-xs text-muted-foreground">{agents.filter((runtime) => runtime.available).length} / {agents.length} 已安装</span></div>{loading ? <p role="status" className="flex items-center gap-2 py-7 text-sm text-muted-foreground"><Loader2 aria-hidden className="h-4 w-4 animate-spin" />正在探测本机 Agent…</p> : agents.length ? <div className="divide-y divide-border">{agents.map((runtime) => <RuntimeRow key={runtime.id} runtime={runtime} isDefault={runtime.id === selected} />)}</div> : <p className="py-6 text-sm text-muted-foreground">未读取到 Agent 信息。请重新检测安装。</p>}<p className="border-t border-border pt-4 text-xs leading-6 text-muted-foreground">安装探测只检查本机可执行文件。健康检测会发送一条最小测试请求，验证登录、模型与连接，最长约 60 秒。</p></div>
      </section>
      <LegacySettings runtimes={runtimes} defaultRuntime={selected} />
    </div>
  </main>;
}

function RuntimeRow({ runtime, isDefault }: { runtime: RuntimeInfo; isDefault: boolean }) {
  const [checking, setChecking] = useState(false);
  const [result, setResult] = useState<{ ok: boolean; text: string } | null>(null);
  const active = useRef(true);
  const lock = useRef(false);
  useEffect(() => { active.current = true; return () => { active.current = false; }; }, []);
  const check = async () => {
    if (lock.current) return;
    lock.current = true; setChecking(true); setResult(null);
    try {
      const response = await api.runtimes.check(runtime.id);
      if (active.current) setResult(response.ok ? { ok: true, text: `连接正常${response.latency_ms != null ? ` · ${(response.latency_ms / 1000).toFixed(1)} 秒` : ''}${response.reply ? ` · ${response.reply}` : ''}` } : { ok: false, text: response.error || '检测失败，请检查 Agent 登录与模型配置。' });
    } catch (failure) { if (active.current) setResult({ ok: false, text: errorMessage(failure) }); }
    finally { lock.current = false; if (active.current) setChecking(false); }
  };
  return <div className="flex flex-wrap items-start gap-4 py-5"><Terminal aria-hidden className="mt-1 h-5 w-5 shrink-0 text-muted-foreground" /><div className="min-w-0 flex-1"><div className="flex flex-wrap items-center gap-3"><h3 className="text-sm font-semibold">{runtime.name}</h3><span className={cn('text-xs', runtime.available ? 'text-primary' : 'text-muted-foreground')}>{runtime.available ? '已安装' : '未找到'}</span>{isDefault && <span className="text-xs text-muted-foreground">默认</span>}</div><p className="mt-2 text-xs leading-5 text-muted-foreground">{runtime.available ? [runtime.version, runtime.model ? `模型 ${runtime.model}` : '模型沿用 Agent 配置'].filter(Boolean).join(' · ') : '安装并完成登录后，点击上方“重新检测安装”。'}</p>{runtime.path && <p className="mt-1 break-all font-mono text-xs leading-5 text-muted-foreground">{runtime.path}</p>}{checking && <p role="status" className="mt-2 text-xs text-muted-foreground">正在发送最小测试请求…</p>}{result && <p role={result.ok ? 'status' : 'alert'} className={cn('mt-3 flex items-start gap-2 break-words text-xs leading-5', result.ok ? 'text-primary' : 'text-destructive')}>{result.ok ? <Check aria-hidden className="mt-0.5 h-3.5 w-3.5 shrink-0" /> : <AlertCircle aria-hidden className="mt-0.5 h-3.5 w-3.5 shrink-0" />}<span>{result.text}</span></p>}</div>{runtime.available && <Button variant="secondary" size="sm" loading={checking} onClick={() => void check()}>{checking ? '检测中…' : result ? '重新检测' : '健康检测'}</Button>}</div>;
}

function LegacySettings({ runtimes, defaultRuntime }: { runtimes: RuntimeInfo[]; defaultRuntime: string }) {
  const [opened, setOpened] = useState(false);
  const [orch, setOrch] = useState<OrchestrationSettings | null>(null);
  const [roles, setRoles] = useState<RoleInfo[]>([]);
  const [error, setError] = useState('');
  const [saving, setSaving] = useState(false);
  const lock = useRef(false);
  useEffect(() => {
    if (!opened) return;
    let alive = true;
    setError('');
    Promise.all([api.orchestration.get(), api.roles.list()]).then(([settings, items]) => { if (alive) { setOrch(settings); setRoles(items); } }).catch((failure) => { if (alive) setError(errorMessage(failure)); });
    return () => { alive = false; };
  }, [opened]);
  const save = async (key: string, runtime: string | null) => {
    if (!orch || lock.current) return;
    lock.current = true; setSaving(true); setError('');
    try { const result = await api.orchestration.save({ stage_runtimes: { ...orch.stage_runtimes, [key]: runtime } }); setOrch(result); toast.success('兼容流程设置已保存'); }
    catch (failure) { setError(errorMessage(failure)); }
    finally { lock.current = false; setSaving(false); }
  };
  return <details className="group mt-8 border-t border-border pt-5" onToggle={(event) => setOpened(event.currentTarget.open)}><summary className="flex cursor-pointer items-center justify-between gap-4 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-primary/30"><span>兼容流程设置 <span className="ml-2 text-xs text-muted-foreground">历史阶段自动化与角色指令</span></span><ChevronDown aria-hidden className="h-4 w-4 transition-transform group-open:rotate-180" /></summary><div className="mt-4 space-y-4"><p className="text-xs leading-6 text-muted-foreground">仅影响历史需求的自动化流程。真实任务启动使用上方默认 Agent。</p>{error && <p role="alert" className="text-sm text-destructive">{error}</p>}{!orch ? <p role="status" className="text-sm text-muted-foreground">{error ? '关闭并重新展开可重试。' : '正在读取兼容配置…'}</p> : <div className="divide-y divide-border border-y border-border">{STAGES.map((stage) => <StageRow key={stage.key} stage={stage} runtimes={runtimes} defaultRuntime={defaultRuntime} configured={orch.stage_runtimes[stage.key]} role={roles.find((role) => role.stage === stage.key)} saving={saving} onSaveRuntime={(value) => void save(stage.key, value)} onRoleChanged={(role) => setRoles((previous) => previous.map((item) => item.stage === role.stage ? role : item))} />)}</div>}</div></details>;
}

function StageRow({ stage, runtimes, defaultRuntime, configured, role, saving, onSaveRuntime, onRoleChanged }: { stage: { key: string; label: string }; runtimes: RuntimeInfo[]; defaultRuntime: string; configured?: string; role?: RoleInfo; saving: boolean; onSaveRuntime: (value: string | null) => void; onRoleChanged: (role: RoleInfo) => void }) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState('');
  const [busy, setBusy] = useState(false);
  const lock = useRef(false);
  const update = async (reset: boolean) => {
    if (!role || lock.current) return;
    lock.current = true; setBusy(true);
    try { const saved = reset ? await api.roles.reset(role.stage) : await api.roles.save(role.stage, draft); onRoleChanged(saved); setDraft(saved.content); if (!reset) setEditing(false); toast.success(reset ? '角色已恢复默认' : '角色已保存'); }
    catch (failure) { toast.error(errorMessage(failure)); }
    finally { lock.current = false; setBusy(false); }
  };
  return <div className="py-4"><div className="flex flex-wrap items-center gap-4"><label htmlFor={`stage-${stage.key}`} className="w-24 text-sm font-medium">{stage.label}</label><div className="relative min-w-0 flex-1"><Select id={`stage-${stage.key}`} className="w-full max-w-sm pr-8" value={configured ?? ''} disabled={saving} onChange={(event) => onSaveRuntime(event.target.value || null)}><option value="">跟随默认（{runtimes.find((runtime) => runtime.id === defaultRuntime)?.name ?? defaultRuntime}）</option>{configured && !runtimes.some((runtime) => runtime.id === configured && runtime.available) && <option value={configured}>{configured} · 当前不可用</option>}{runtimes.filter((runtime) => runtime.available).map((runtime) => <option key={runtime.id} value={runtime.id}>{runtime.name}</option>)}</Select><SelectChevron /></div><Button size="sm" variant="ghost" disabled={!role || busy} onClick={() => { setDraft(role?.content ?? ''); setEditing((value) => !value); }}><FileCog aria-hidden className="h-4 w-4" />角色指令</Button></div>{editing && role && <div className="mt-4 space-y-3"><label htmlFor={`role-${stage.key}`} className="block text-xs font-medium">{stage.label}角色指令</label><AutoTextarea id={`role-${stage.key}`} value={draft} disabled={busy} onChange={(event) => setDraft(event.target.value)} rows={8} maxAutoHeight={420} spellCheck={false} className="w-full rounded-lg border border-input bg-background px-3 py-2 font-mono text-xs leading-6 outline-none focus-visible:ring-2 focus-visible:ring-primary/30" /><p className="break-all font-mono text-xs text-muted-foreground">{role.path}</p><div className="flex flex-wrap items-center gap-2"><Button size="sm" disabled={draft === role.content || busy} loading={busy} onClick={() => void update(false)}><Save aria-hidden className="h-3.5 w-3.5" />保存</Button><Button size="sm" variant="ghost" disabled={busy} onClick={() => void update(true)}><RotateCcw aria-hidden className="h-3.5 w-3.5" />恢复默认</Button><Button size="sm" variant="ghost" disabled={busy} onClick={() => setEditing(false)}><X aria-hidden className="h-3.5 w-3.5" />取消</Button></div></div>}</div>;
}
