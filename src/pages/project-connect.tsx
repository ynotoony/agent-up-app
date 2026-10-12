// Input: 本机目录、三类仓库的只读预览与用户明确确认。
// Output: 枚举补缺写入后接入真实任务工作台，不生成示例任务。
// Pos: /connect 接入页；采用现有语义 tokens，src/ 目录登记豁免。
import { useEffect, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { open } from '@tauri-apps/plugin-dialog';
import { AlertCircle, ArrowLeft, ArrowRight, Check, ChevronDown, FolderOpen, GitBranch, Loader2, RefreshCw } from 'lucide-react';
import type { WorkspacePreview } from '../../shared/types';
import { api, errorMessage } from '@/lib/api';
import { Button } from '@/components/ui/button';

const modeCopy = {
  new: { title: '新工作目录', description: '确认后初始化 Git，并在缺少任务来源时建立空索引。' },
  git: { title: '已有代码仓库', description: '沿用已有 Git 历史，补上真实任务的存放位置。' },
  governed: { title: '已有任务来源', description: '直接读取仓库里的任务索引、依赖和状态。' },
};

export default function ProjectConnectPage() {
  const navigate = useNavigate();
  const [search, setSearch] = useSearchParams();
  const [path, setPath] = useState('');
  const [preview, setPreview] = useState<WorkspacePreview | null>(null);
  const [phase, setPhase] = useState<'idle' | 'picking' | 'scanning' | 'saving'>('idle');
  const [error, setError] = useState('');
  const [approved, setApproved] = useState(false);
  const [stale, setStale] = useState(false);
  const generation = useRef(0);
  const saving = useRef(false);
  useEffect(() => () => { generation.current += 1; }, []);
  useEffect(() => {
    const requestedPath = search.get('path');
    if (requestedPath) {
      setSearch({}, { replace: true });
      void scan(requestedPath);
    }
  // Initial URL path supports the repair link from a failed project source.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const scan = async (candidate: string) => {
    if (!candidate.trim() || saving.current) return;
    const version = ++generation.current;
    setPath(candidate.trim()); setPreview(null); setApproved(false); setStale(false); setError(''); setPhase('scanning');
    try {
      const result = await api.projects.onboardingPreview(candidate.trim());
      if (version !== generation.current) return;
      setPreview(result); setPath(result.path);
    } catch (failure) {
      if (version === generation.current) setError(errorMessage(failure));
    } finally {
      if (version === generation.current) setPhase('idle');
    }
  };

  const choose = async () => {
    if (saving.current) return;
    const version = ++generation.current;
    setPhase('picking'); setError('');
    try {
      const result = await open({ directory: true, multiple: false, title: '选择项目目录' });
      if (version !== generation.current) return;
      if (typeof result === 'string') await scan(result);
      else setPhase('idle');
    } catch (failure) {
      if (version === generation.current) { setError(errorMessage(failure)); setPhase('idle'); }
    }
  };

  const connect = async () => {
    if (!preview || !approved || stale || phase !== 'idle' || saving.current) return;
    saving.current = true;
    const version = ++generation.current;
    setPhase('saving'); setError('');
    try {
      const result = await api.projects.onboardingConfirm({ path: preview.path, fingerprint: preview.fingerprint });
      if (version === generation.current) {
        window.dispatchEvent(new Event('agentup:projects-changed'));
        navigate(`/project/${result.project.id}`);
      }
    } catch (failure) {
      if (version === generation.current) { setError(`${errorMessage(failure)}。请重新预览，已存在的文件会保留。`); setStale(true); setApproved(false); }
    } finally {
      saving.current = false;
      if (version === generation.current) setPhase('idle');
    }
  };

  const busy = phase !== 'idle';
  return <main id="main-content" tabIndex={-1} className="h-screen overflow-y-auto scrollbar-thin">
    <div data-tauri-drag-region className="drag-region h-3" />
    <div className="mx-auto max-w-6xl px-6 py-7 pb-16 sm:px-10">
      <header className="mb-10 flex flex-wrap items-start justify-between gap-5">
        <div><p className="text-xs font-medium tracking-widest text-muted-foreground">工作空间 / 项目接入</p><h1 className="mt-3 text-3xl font-semibold tracking-tight">把工作带进来</h1><p className="mt-3 max-w-xl text-sm leading-6 text-muted-foreground">选择本机目录，读取真实任务，然后在你使用的 Agent 中继续。</p></div>
        <Button variant="ghost" disabled={phase === 'saving'} onClick={() => navigate('/')}><ArrowLeft aria-hidden className="h-4 w-4" />返回工作台</Button>
      </header>
      <section aria-labelledby="directory-heading" className="border-y border-border bg-card px-5 py-7 sm:px-8">
        <div className="grid gap-7 lg:grid-cols-[minmax(0,0.32fr)_minmax(0,0.68fr)] lg:gap-12">
          <div><p className="text-xs font-medium text-muted-foreground">01 / 选择目录</p><h2 id="directory-heading" className="mt-3 text-lg font-semibold">项目从哪里开始</h2><p id="path-help" className="mt-3 text-sm leading-6 text-muted-foreground">空目录、已有代码、已治理项目都可接入。先预览，再确认具体写入。</p></div>
          <form className="space-y-3" onSubmit={(event) => { event.preventDefault(); if (!busy) void scan(path); }}>
            <label htmlFor="workspace-directory" className="block text-sm font-medium">项目目录</label>
            <div className="flex flex-wrap gap-2"><input id="workspace-directory" value={path} disabled={busy} aria-describedby="path-help" autoComplete="off" spellCheck={false} placeholder="/Users/you/Projects/my-project" onChange={(event) => { generation.current += 1; setPath(event.target.value); setPreview(null); setApproved(false); setStale(false); setError(''); }} className="h-11 min-w-0 flex-1 rounded-lg border border-input bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-primary/30 disabled:opacity-60" /><Button type="button" variant="secondary" disabled={busy} className="h-11" onClick={() => void choose()}><FolderOpen aria-hidden className="h-4 w-4" />浏览</Button></div>
            <div className="flex flex-wrap items-center justify-between gap-3"><p className="text-xs leading-5 text-muted-foreground">此步骤只读取目录与任务来源。</p><Button type="submit" variant={preview ? 'ghost' : 'primary'} disabled={!path.trim() || busy} loading={phase === 'scanning'}>{preview ? <RefreshCw aria-hidden className="h-4 w-4" /> : null}{phase === 'scanning' ? '正在读取…' : preview ? '重新预览' : '预览项目'}</Button></div>
          </form>
        </div>
        {phase === 'scanning' && <p role="status" className="mt-7 flex items-center gap-2 border-t border-border pt-5 text-sm text-muted-foreground"><Loader2 aria-hidden className="h-4 w-4 animate-spin" />正在检查 Git、任务索引与项目上下文…</p>}
        {error && <div role="alert" className="mt-6 flex items-start gap-3 border-l-2 border-destructive pl-4 text-sm leading-6 text-destructive"><AlertCircle aria-hidden className="mt-1 h-4 w-4 shrink-0" /><p className="break-words">{error}</p></div>}
        {preview && <div className="mt-8 border-t border-border pt-7"><div className="grid gap-7 lg:grid-cols-[minmax(0,0.32fr)_minmax(0,0.68fr)] lg:gap-12">
          <div><p className="text-xs font-medium text-muted-foreground">02 / 确认接入</p><h2 className="mt-3 break-words text-lg font-semibold">{preview.name}</h2><p className="mt-3 text-sm font-medium">{modeCopy[preview.mode].title}</p><p className="mt-2 text-sm leading-6 text-muted-foreground">{modeCopy[preview.mode].description}</p>{preview.git && <p className="mt-4 flex flex-wrap items-center gap-2 text-xs text-muted-foreground"><GitBranch aria-hidden className="h-3.5 w-3.5" />{preview.git.branch ?? '未创建分支提交'}{preview.git.dirty ? ' · 有未提交改动' : ''}</p>}</div>
          <div className="min-w-0 space-y-6">
            <div><h3 className="text-sm font-medium">任务来源</h3><p className="mt-2 break-all font-mono text-xs leading-6 text-muted-foreground">{preview.index_path ?? 'facts/requirements/tickets/index.json（将建立空索引）'}</p><p className="mt-2 text-xs leading-5 text-muted-foreground">任务数量与状态来自仓库文件，接入过程不生成任务。</p></div>
            <div><h3 className="text-sm font-medium">确认后将执行</h3><ol className="mt-3 divide-y divide-border border-y border-border">{preview.writes.map((write, index) => <li key={`${write.action}:${write.path}`} className="flex items-start gap-3 py-3"><span className="mt-0.5 text-xs tabular-nums text-muted-foreground">{String(index + 1).padStart(2, '0')}</span><div className="min-w-0"><p className="break-all text-sm font-medium">{write.path}</p><p className="mt-1 text-xs leading-5 text-muted-foreground">{write.description}</p></div></li>)}</ol></div>
            {preview.warnings.length > 0 && <div role="status" className="border-l-2 border-border pl-4"><h3 className="text-sm font-medium">需要留意</h3><ul className="mt-2 space-y-2 text-xs leading-5 text-muted-foreground">{preview.warnings.map((warning, index) => <li key={index}>{warning}</li>)}</ul></div>}
            {preview.discovery && preview.discovery.sources.length > 0 && <details className="group"><summary className="flex cursor-pointer items-center justify-between gap-2 py-1 text-sm outline-none focus-visible:ring-2 focus-visible:ring-primary/30"><span>已识别上下文 · {preview.discovery.sources.length} 个来源</span><ChevronDown aria-hidden className="h-4 w-4 transition-transform group-open:rotate-180" /></summary><ul className="mt-3 space-y-2 text-xs text-muted-foreground">{preview.discovery.sources.map((source) => <li key={source.path} className="break-all font-mono">{source.path}</li>)}</ul></details>}
            <div className="border-t border-border pt-5"><label className="flex cursor-pointer items-start gap-3 text-sm leading-6"><input type="checkbox" checked={approved} onChange={(event) => setApproved(event.target.checked)} disabled={busy || stale} className="mt-1 h-4 w-4 shrink-0 accent-primary" /><span>我确认以上操作。已有代码、文档和任务文件保留原样。</span></label><div className="mt-5 flex flex-wrap items-center justify-between gap-3"><p className="text-xs text-muted-foreground">{stale ? '重新预览后才能接入。' : '接入后可选择真实任务并启动 Agent。'}</p><Button disabled={!approved || busy || stale} loading={phase === 'saving'} onClick={() => void connect()}>{phase === 'saving' ? '正在接入…' : <><Check aria-hidden className="h-4 w-4" />确认并接入<ArrowRight aria-hidden className="h-4 w-4" /></>}</Button></div></div>
          </div>
        </div></div>}
      </section>
      {!preview && <p className="mt-6 max-w-2xl text-xs leading-6 text-muted-foreground">已有 Git 仓库直接沿用。非 Git 目录只在确认后初始化；已有任务来源优先读取，缺少时建立空索引。每一步写入都会在预览中列明。</p>}
    </div>
  </main>;
}
