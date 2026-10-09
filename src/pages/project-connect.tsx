// Input: 用户选择的 Git 项目、只读发现结果与经确认的来源引用。
// Output: 项目接入预览、来源选择与 App 原生项目档案回执；不生成任务。
// Pos: /connect 独立接入页，使用既有设计 tokens；目录登记按 src/ 豁免规则维护。
import { useEffect, useRef, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { AlertCircle, CheckCircle2, FolderInput, GitBranch, RefreshCw, ScanSearch, ShieldCheck } from 'lucide-react';
import type { ManagementSource, ProjectDiscovery } from '../../shared/types';
import { api, errorMessage } from '@/lib/api';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

const categoryLabels: Record<ManagementSource['category'], string> = {
  current: '当前候选',
  history: '历史资料',
  context: '项目上下文',
  mixed: '混合记录',
  unknown: '待识别',
};

function formatDate(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN');
}

function sourceSummary(source: ManagementSource): string {
  if (source.category === 'context') return `${source.item_count} 项上下文`;
  const counts = [
    source.current_count > 0 ? `当前 ${source.current_count}` : '',
    source.history_count > 0 ? `历史 ${source.history_count}` : '',
    source.unknown_count > 0 ? `待识别 ${source.unknown_count}` : '',
  ].filter(Boolean);
  return counts.length > 0 ? counts.join(' · ') : `${source.item_count} 条记录`;
}

export default function ProjectConnectPage() {
  const [path, setPath] = useState('');
  const [discovery, setDiscovery] = useState<ProjectDiscovery | null>(null);
  const [selected, setSelected] = useState<Set<string>>(() => new Set());
  const [phase, setPhase] = useState<'idle' | 'picking' | 'scanning' | 'saving'>('idle');
  const [error, setError] = useState('');
  const [requiresScan, setRequiresScan] = useState(false);
  const [notice, setNotice] = useState('');
  // Tauri invoke is not abortable: generations discard outdated reads, including on unmount.
  const requestVersion = useRef(0);
  useEffect(() => () => { requestVersion.current += 1; }, []);

  const scan = async (requestedPath: string) => {
    const trimmed = requestedPath.trim();
    if (!trimmed) return;
    const version = ++requestVersion.current;
    setPath(trimmed);
    setDiscovery(null);
    setSelected(new Set());
    setError('');
    setNotice('');
    setRequiresScan(false);
    setPhase('scanning');
    try {
      const result = await api.projects.discover(trimmed);
      if (version !== requestVersion.current) return;
      setPath(result.path);
      setDiscovery(result);
      setSelected(new Set(result.sources.filter((source) =>
        source.category === 'current' || source.category === 'context' ||
        (source.category === 'mixed' && source.current_count > 0),
      ).map((source) => source.path)));
      setNotice(result.native_project ? '已读取现有项目档案。' : '扫描完成。请确认要保存的来源引用。');
    } catch (err) {
      if (version === requestVersion.current) setError(errorMessage(err));
    } finally {
      if (version === requestVersion.current) setPhase('idle');
    }
  };

  const chooseFolder = async () => {
    const version = ++requestVersion.current;
    setError('');
    setNotice('');
    setPhase('picking');
    try {
      const chosen = await open({ directory: true, multiple: false, title: '选择已有 Git 项目' });
      if (version !== requestVersion.current) return;
      if (typeof chosen === 'string' && chosen) {
        await scan(chosen);
      } else {
        setPhase('idle');
        setNotice('已取消选择，项目文件未修改。');
      }
    } catch (err) {
      if (version === requestVersion.current) {
        setPhase('idle');
        setError(`无法打开文件夹选择器：${errorMessage(err)}。也可以在下方输入项目绝对路径。`);
      }
    }
  };

  const changePath = (value: string) => {
    requestVersion.current += 1;
    setPath(value);
    setDiscovery(null);
    setSelected(new Set());
    setPhase('idle');
    setError('');
    setRequiresScan(false);
    setNotice('');
  };

  const cancelScan = () => {
    requestVersion.current += 1;
    setPhase('idle');
    setNotice('已取消等待。本次只读扫描的结果将被忽略，项目文件未修改。');
  };

  const confirmImport = async () => {
    if (!discovery || discovery.native_project || phase !== 'idle' || requiresScan || path !== discovery.path) return;
    const version = ++requestVersion.current;
    setPhase('saving');
    setError('');
    setNotice('');
    try {
      const profile = await api.projects.importConfirm({
        path: discovery.path,
        fingerprint: discovery.fingerprint,
        selected_sources: discovery.sources.filter((source) => selected.has(source.path)).map((source) => source.path),
      });
      if (version !== requestVersion.current) return;
      setDiscovery({ ...discovery, native_project: profile });
      setNotice('项目档案已保存，可以随时重新扫描此目录查看。');
    } catch (err) {
      if (version === requestVersion.current) {
        setError(errorMessage(err));
        setRequiresScan(true);
      }
    } finally {
      if (version === requestVersion.current) setPhase('idle');
    }
  };

  const profile = discovery?.native_project;
  const locked = phase === 'saving' || phase === 'picking';
  const totals = discovery?.sources.reduce((counts, source) => ({
    current: counts.current + source.current_count,
    history: counts.history + source.history_count,
    context: counts.context + (source.category === 'context' ? source.item_count : 0),
    unknown: counts.unknown + source.unknown_count,
  }), { current: 0, history: 0, context: 0, unknown: 0 });

  return (
    <main className="h-screen overflow-y-auto scrollbar-thin">
      <div data-tauri-drag-region className="drag-region h-2" />
      <div className="mx-auto flex max-w-5xl flex-col gap-5 px-6 py-5 pb-12">
        <header>
          <h1 className="text-lg font-semibold tracking-tight">接入项目</h1>
          <p className="mt-1 text-sm text-muted-foreground">先了解已有项目，再由你决定保存哪些来源。</p>
        </header>

        <section aria-labelledby="choose-project" className="rounded-xl border border-border bg-card p-5">
          <div className="mb-4 flex items-start justify-between gap-4">
            <div>
              <h2 id="choose-project" className="font-medium">选择已有 Git 项目</h2>
              <p id="project-path-hint" className="mt-1 text-xs leading-5 text-muted-foreground">扫描只读取目录与管理资料。建立档案前，不会写入项目。</p>
            </div>
            <Badge className="shrink-0 bg-primary/10 text-primary">只读发现</Badge>
          </div>
          <form onSubmit={(event) => { event.preventDefault(); if (!locked && phase !== 'scanning') void scan(path); }}>
            <label htmlFor="project-path" className="mb-2 block text-xs font-medium">项目绝对路径</label>
            <div className="flex flex-wrap items-center gap-2">
              <input
                id="project-path"
                value={path}
                onChange={(event) => changePath(event.target.value)}
                disabled={locked}
                aria-describedby="project-path-hint"
                placeholder="输入项目目录，或选择文件夹"
                autoComplete="off"
                spellCheck={false}
                className="h-9 min-w-48 flex-1 rounded-lg border border-input bg-background px-3 text-sm outline-none focus-visible:border-primary focus-visible:ring-2 focus-visible:ring-primary/30 disabled:opacity-50"
              />
              <Button type="button" variant="secondary" disabled={locked} onClick={() => void chooseFolder()}>
                <FolderInput aria-hidden="true" className="h-4 w-4" />
                {phase === 'picking' ? '选择中…' : '选择文件夹'}
              </Button>
              <Button type="submit" variant="secondary" disabled={locked || phase === 'scanning' || !path.trim()}>
                <ScanSearch aria-hidden="true" className="h-4 w-4" />
                {discovery ? '重新扫描' : '扫描项目'}
              </Button>
            </div>
          </form>
          {phase === 'scanning' ? (
            <div role="status" className="mt-4 flex items-center justify-between gap-3 rounded-lg bg-muted px-3 py-2 text-sm">
              <span>正在只读扫描项目…</span>
              <Button variant="ghost" size="sm" onClick={cancelScan}>取消等待</Button>
            </div>
          ) : null}
          {notice ? <p role="status" className="mt-3 text-sm text-muted-foreground">{notice}</p> : null}
          {error ? (
            <div role="alert" className="mt-4 rounded-lg border border-destructive/40 bg-destructive/5 p-3">
              <div className="flex items-start gap-2">
                <AlertCircle aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0 text-destructive" />
                <p className="min-w-0 break-words text-sm leading-6">{error}</p>
              </div>
              <div className="mt-2 flex flex-wrap items-center gap-3 pl-6">
                <span className="text-xs text-muted-foreground">{requiresScan ? '请重新扫描核对项目，再确认。' : '检查路径后可以重试，或重新选择文件夹。'}</span>
                <Button variant="secondary" size="sm" disabled={locked || !path.trim()} onClick={() => void scan(path)}>
                  <RefreshCw aria-hidden="true" className="h-3.5 w-3.5" />重新扫描
                </Button>
              </div>
            </div>
          ) : null}
        </section>

        {!discovery && phase !== 'scanning' ? (
          <div className="flex items-start gap-3 px-1 text-muted-foreground">
            <ShieldCheck aria-hidden="true" className="mt-0.5 h-5 w-5 shrink-0" />
            <p className="max-w-2xl text-sm leading-6">接入会展示 Git 状态、主要目录及识别到的管理资料。这一步只建立项目档案和来源引用，不导入具体任务，也不启动 Agent。</p>
          </div>
        ) : null}

        {discovery ? (
          <>
            <section aria-labelledby="discovery-heading" className="rounded-xl border border-border bg-card p-5">
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div className="min-w-0">
                  <p className="mb-1 text-xs text-muted-foreground">项目概况</p>
                  <h2 id="discovery-heading" className="break-words text-base font-semibold">{discovery.name}</h2>
                  <p className="mt-1 break-all font-mono text-xs leading-5 text-muted-foreground">{discovery.path}</p>
                </div>
                <Badge className="bg-muted text-foreground">{discovery.git.dirty ? '有未提交改动' : '工作区干净'}</Badge>
              </div>
              <div className="mt-4 flex flex-wrap items-center gap-x-5 gap-y-2 border-t border-border pt-3 text-xs text-muted-foreground">
                <span className="inline-flex min-w-0 items-center gap-1.5 break-all"><GitBranch aria-hidden="true" className="h-3.5 w-3.5 shrink-0" />{discovery.git.branch ?? '未检出分支'}</span>
                <span>提交：{discovery.git.head ? discovery.git.head.slice(0, 12) : '尚无提交'}</span>
                <span>扫描于 {formatDate(discovery.scanned_at)}</span>
              </div>
              <div className="mt-4">
                <h3 className="mb-2 text-xs font-medium">主要目录</h3>
                {discovery.directories.length > 0 ? (
                  <div className="flex flex-wrap gap-2">{discovery.directories.map((directory) => <span key={directory} className="max-w-full break-all rounded bg-muted px-2 py-1 font-mono text-xs">{directory}/</span>)}</div>
                ) : <p className="text-xs text-muted-foreground">未发现子目录。</p>}
              </div>
            </section>

            {discovery.warnings.length > 0 ? (
              <section aria-labelledby="scan-warnings" className="rounded-xl border border-border bg-card p-4">
                <h2 id="scan-warnings" className="flex items-center gap-2 text-sm font-medium"><AlertCircle aria-hidden="true" className="h-4 w-4" />扫描提示</h2>
                <ul className="mt-2 list-disc space-y-1 pl-6 text-xs leading-5 text-muted-foreground">{discovery.warnings.map((warning, index) => <li className="break-words" key={`${index}-${warning}`}>{warning}</li>)}</ul>
              </section>
            ) : null}

            {profile ? (
              <section aria-labelledby="saved-profile" className="rounded-xl border border-primary/30 bg-card p-5">
                <h2 id="saved-profile" className="flex items-center gap-2 font-medium"><CheckCircle2 aria-hidden="true" className="h-4 w-4 text-primary" />项目档案已建立</h2>
                <p className="mt-2 text-sm leading-6 text-muted-foreground">已保存项目身份和 {profile.sources.length} 个来源引用。原始资料保持不变；具体任务尚未导入。</p>
                <dl className="mt-3 grid gap-2 text-xs sm:grid-cols-[auto_1fr]">
                  <dt className="text-muted-foreground">档案位置</dt><dd className="break-all font-mono">{discovery.path}/.agentup-app/project.json</dd>
                  <dt className="text-muted-foreground">项目 ID</dt><dd className="break-all font-mono">{profile.id}</dd>
                  <dt className="text-muted-foreground">建立时间</dt><dd>{formatDate(profile.created_at)}</dd>
                </dl>
                <h3 className="mt-5 mb-2 text-xs font-medium">已保存的来源</h3>
                {profile.sources.length === 0 ? <p className="text-xs text-muted-foreground">未保存来源引用，仅建立了项目档案。</p> : (
                  <ul className="divide-y divide-border rounded-lg border border-border">
                    {profile.sources.map((source) => {
                      const currentSource = discovery.sources.find((current) => current.path === source.path);
                      const changed = !currentSource || currentSource.fingerprint !== source.fingerprint;
                      return <li key={source.path} className="flex flex-wrap items-start justify-between gap-2 px-3 py-3">
                        <div className="min-w-0 flex-1"><p className="break-all font-mono text-xs">{source.path}</p><p className="mt-1 text-xs text-muted-foreground">{categoryLabels[source.category]} · {sourceSummary(source)}</p></div>
                        {changed ? <Badge className="bg-muted text-foreground">来源已变化</Badge> : null}
                      </li>;
                    })}
                  </ul>
                )}
                <p className="mt-3 text-xs leading-5 text-muted-foreground">重新接入会读取此档案，不重复创建，也不自动覆盖已保存的来源快照。</p>
              </section>
            ) : (
              <section aria-labelledby="sources-heading" className="overflow-hidden rounded-xl border border-border bg-card">
                <div className="p-5">
                  <h2 id="sources-heading" className="font-medium">确认来源引用</h2>
                  <p className="mt-1 text-xs leading-5 text-muted-foreground">选择要记录的外部来源。数量来自可识别的资料记录，不代表已创建 App 任务。</p>
                  <dl className="mt-4 grid grid-cols-2 gap-3 sm:grid-cols-4">
                    {([
                      ['当前候选', totals?.current ?? 0], ['历史资料', totals?.history ?? 0],
                      ['项目上下文', totals?.context ?? 0], ['待识别', totals?.unknown ?? 0],
                    ] as const).map(([label, value]) => <div key={label} className="rounded-lg bg-muted px-3 py-2"><dt className="text-xs text-muted-foreground">{label}</dt><dd className="mt-1 text-lg font-semibold tabular-nums">{value}</dd></div>)}
                  </dl>
                </div>
                {discovery.sources.length > 0 ? (
                  <fieldset disabled={phase === 'saving' || requiresScan} className="min-w-0 border-t border-border">
                    <legend className="sr-only">选择来源引用</legend>
                    <div className="divide-y divide-border">
                      {discovery.sources.map((source) => (
                        <label key={source.path} className="flex cursor-pointer items-start gap-3 px-5 py-4 hover:bg-muted/50 has-[:disabled]:cursor-default">
                          <input type="checkbox" checked={selected.has(source.path)} onChange={(event) => {
                            const checked = event.target.checked;
                            setSelected((previous) => { const next = new Set(previous); if (checked) next.add(source.path); else next.delete(source.path); return next; });
                          }} className="mt-0.5 h-4 w-4 shrink-0 accent-primary focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary" />
                          <span className="min-w-0 flex-1"><span className="block break-all font-mono text-xs leading-5">{source.path}</span><span className="mt-1 block text-xs leading-5 text-muted-foreground">{sourceSummary(source)}</span></span>
                          <Badge className="shrink-0 bg-muted text-foreground">{categoryLabels[source.category]}</Badge>
                        </label>
                      ))}
                    </div>
                  </fieldset>
                ) : <p className="border-t border-border px-5 py-4 text-sm leading-6 text-muted-foreground">未发现可识别的管理记录。可以仅建立项目档案；App 不会从文档中的 TODO 自动生成任务。</p>}
                <div className="border-t border-border bg-muted/30 p-5">
                  <p className="text-xs leading-5 text-muted-foreground">确认后在项目内建立 <span className="font-mono">.agentup-app/</span>，保存项目档案和已选来源引用。不会改写原文件、导入具体任务或启动 Agent。</p>
                  <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
                    <p className="text-xs text-muted-foreground">已选择 {selected.size} / {discovery.sources.length} 个来源{selected.size === 0 ? ' · 仅建立档案' : ''}</p>
                    <Button disabled={phase !== 'idle' || requiresScan || path !== discovery.path} loading={phase === 'saving'} onClick={() => void confirmImport()}>
                      {phase === 'saving' ? '正在保存…' : '建立项目档案'}
                    </Button>
                  </div>
                  {phase === 'saving' ? <p role="status" className="mt-3 text-xs text-muted-foreground">正在核对扫描结果并保存项目档案…</p> : null}
                </div>
              </section>
            )}
          </>
        ) : null}
      </div>
    </main>
  );
}
