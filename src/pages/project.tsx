import { useNavigate, useParams } from 'react-router-dom';
import { useLayoutEffect, useState } from 'react';
import { FolderKanban, FolderOpen, Pencil, X, Check, Loader2, AlertCircle, AlertTriangle, RefreshCw, Sparkles, Download, Shield, FileText, MoreHorizontal, Trash2 } from 'lucide-react';
import { toast } from 'sonner';
import type { GovernanceItem, InitReport, ProjectDashboard } from '../../shared/types';
import { api, errorMessage } from '@/lib/api';
import { RequirementInput } from '@/components/requirement-input';
import { RequirementList } from '@/components/requirement-list';
import { GovernancePanel, GovernanceTicketsPanel, InitializingPanel, ProjectDocsPanel } from '@/components/project-governance';
import { PendingDecisionsPanel, ErrorPanel, SkeletonPanel } from '@/components/workspace-view';
import { Button } from '@/components/ui/button';
import { TabPills } from '@/components/ui/tabs';
import { isTerminal } from '@/components/ui/status';

type StatusFilter = 'all' | 'active' | 'completed' | 'failed';

// 项目工作区：/project/:id。双栏工作台：上（头部 + 一句话输入）固定，下左需求列表 / 下右治理文档，桌面各自滚动。
export default function ProjectPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [data, setData] = useState<ProjectDashboard | null>(null);
  const [error, setError] = useState('');
  const [report, setReport] = useState<InitReport | null>(null);
  const [reiniting, setReiniting] = useState(false);
  const [completing, setCompleting] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);
  const [workTab, setWorkTab] = useState<'governance' | 'tickets' | 'docs'>('governance');
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all');
  const [pendingOpen, setPendingOpen] = useState<number | null>(null);

  const fetchData = () => {
    if (!id) return;
    api.projects
      .get(id)
      .then(setData)
      .catch((err) => setError(errorMessage(err)));
    api.projects
      .governance(id, 'pending')
      .then((items: GovernanceItem[]) => setPendingOpen(items.filter((i) => i.status === 'active').length))
      .catch(() => setPendingOpen(null));
  };

  useLayoutEffect(fetchData, [id]);

  if (error) {
    return (
      <main className="h-screen overflow-y-auto scrollbar-thin">
        <div className="max-w-7xl mx-auto px-6 py-6">
          <ErrorPanel message={error} />
        </div>
      </main>
    );
  }
  if (!data) {
    return (
      <main className="h-screen overflow-y-auto scrollbar-thin">
        <div className="max-w-7xl mx-auto px-6 py-6">
          <SkeletonPanel />
        </div>
      </main>
    );
  }

  const reinit = async () => {
    setReiniting(true);
    try {
      const r = await api.projects.reinit(data.project.id);
      setReport(r);
      toast.success(`重新初始化完成：文档 +${r.docs_added} · 治理补缺 ${r.governance_seeded} 条`);
      fetchData();
    } catch (err) {
      toast.error(errorMessage(err));
    } finally {
      setReiniting(false);
    }
  };

  const complete = async () => {
    setCompleting(true);
    try {
      const requirement = await api.projects.governanceComplete(data.project.id);
      toast.success('治理补全需求已创建，codex 将自行查证可核实项，查不到才向你提问');
      navigate(`/requirement/${requirement.id}`);
    } catch (err) {
      toast.error(errorMessage(err));
    } finally {
      setCompleting(false);
    }
  };

  const doExport = async (kind: 'agentup-files' | 'json-snapshot') => {
    setExportOpen(false);
    setExporting(true);
    try {
      const target = kind === 'json-snapshot' ? `${data.project.path ?? '.'}/agentup-snapshot.json` : undefined;
      const outcome = await api.projects.export(data.project.id, kind, target);
      toast.success(`导出完成：写入 ${outcome.written.length} 个文件${outcome.skipped.length ? `，跳过 ${outcome.skipped.length} 个（内容未变）` : ''}`);
    } catch (err) {
      toast.error(errorMessage(err));
    } finally {
      setExporting(false);
    }
  };

  return (
    <main className="h-screen overflow-y-auto scrollbar-thin lg:overflow-hidden">
      <div className="max-w-7xl mx-auto px-6 py-6 lg:h-full lg:py-5 flex flex-col gap-5">
        {/* 固定区：头部 + 初始化报告 + 一句话需求 */}
        <div className="shrink-0 space-y-5">
          <ProjectHeader
            project={data.project}
            pendingOpen={pendingOpen}
            requirementCount={data.stats.totalRequirements}
            reiniting={reiniting}
            completing={completing}
            exporting={exporting}
            exportOpen={exportOpen}
            onExportToggle={() => setExportOpen((v) => !v)}
            onExport={doExport}
            onReinit={() => void reinit()}
            onComplete={() => void complete()}
            onChanged={fetchData}
            onDeleted={() => navigate('/')}
          />
          <InitializingPanel projectId={data.project.id} report={report} onCloseReport={() => setReport(null)} />
          <RequirementInput projects={[data.project]} defaultProjectId={data.project.id} lockProject onCreated={(requirementId) => navigate(`/requirement/${requirementId}`)} />
        </div>

        {/* 双栏工作区：左需求列表（统计转为筛选计数）/ 右治理与文档 */}
        <div className="flex-1 min-h-0 lg:overflow-hidden grid grid-cols-1 lg:grid-cols-3 gap-5">
          <div className="lg:col-span-2 lg:overflow-y-auto lg:pr-1 scrollbar-thin space-y-4 min-w-0">
            <PendingDecisionsPanel decisions={data.pendingDecisions} />
            <section className="space-y-3">
              <div className="lg:sticky lg:top-0 z-10 bg-background pb-3 space-y-3">
                <h2 className="text-sm font-medium text-muted-foreground px-1">需求</h2>
                <TabPills
                  value={statusFilter}
                  onChange={setStatusFilter}
                  options={[
                    { value: 'all', label: `全部 ${data.stats.totalRequirements}` },
                    { value: 'active', label: `进行中 ${data.stats.inProgress}` },
                    { value: 'completed', label: `已完成 ${data.stats.completed}` },
                    { value: 'failed', label: `失败 ${data.stats.failed}` },
                  ]}
                  className="self-start"
                />
              </div>
              <RequirementList
                requirements={data.requirements.filter((r) => {
                  if (statusFilter === 'all') return true;
                  if (statusFilter === 'active') return !isTerminal(r.status);
                  return r.status === statusFilter;
                })}
                filterable={false}
              />
            </section>
          </div>

          <div className="lg:col-span-1 lg:overflow-y-auto lg:pl-1 scrollbar-thin space-y-3 min-w-0">
            <div className="flex items-center gap-3 lg:sticky lg:top-0 z-10 bg-background pb-3">
              <h2 className="text-sm font-medium text-muted-foreground px-1">治理与文档</h2>
              <TabPills
                value={workTab}
                onChange={setWorkTab}
                options={[
                  { value: 'governance', label: '治理' },
                  { value: 'tickets', label: '治理票' },
                  { value: 'docs', label: '文档' },
                ]}
              />
            </div>
            {workTab === 'governance' ? (
              <GovernancePanel projectId={data.project.id} />
            ) : workTab === 'tickets' ? (
              <GovernanceTicketsPanel projectId={data.project.id} />
            ) : (
              <ProjectDocsPanel projectId={data.project.id} onConverted={(requirementId) => navigate(`/requirement/${requirementId}`)} />
            )}
          </div>
        </div>
      </div>
    </main>
  );
}

function ProjectHeader({
  project,
  pendingOpen,
  requirementCount,
  reiniting,
  completing,
  exporting,
  exportOpen,
  onExportToggle,
  onExport,
  onReinit,
  onComplete,
  onChanged,
  onDeleted,
}: {
  project: ProjectDashboard['project'];
  pendingOpen: number | null;
  requirementCount: number;
  reiniting: boolean;
  completing: boolean;
  exporting: boolean;
  exportOpen: boolean;
  onExportToggle: () => void;
  onExport: (kind: 'agentup-files' | 'json-snapshot') => void;
  onReinit: () => void;
  onComplete: () => void;
  onChanged: () => void;
  onDeleted: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(project.name);
  const [description, setDescription] = useState(project.description ?? '');
  const [busy, setBusy] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [confirmText, setConfirmText] = useState('');
  const [moreOpen, setMoreOpen] = useState(false);
  const [error, setError] = useState('');

  const save = async () => {
    setBusy(true);
    setError('');
    try {
      await api.projects.update(project.id, { name: name.trim() || project.name, description: description.trim() || null });
      setEditing(false);
      onChanged();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setBusy(true);
    try {
      await api.projects.remove(project.id);
      onDeleted();
    } catch (err) {
      setError(errorMessage(err));
      setBusy(false);
    }
  };

  return (
    <header className="flex items-start justify-between gap-4">
      <div className="min-w-0">
        <div className="flex items-center gap-2">
          <FolderKanban className="w-5 h-5 text-primary shrink-0" />
          {editing ? (
            <div className="space-y-2 w-full max-w-xl">
              <input
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="w-full h-8 px-2.5 rounded-lg bg-background border border-border text-sm font-semibold focus:outline-none focus:border-primary/50 focus:ring-1 focus:ring-primary/20"
                placeholder="项目名"
              />
              <input
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                className="w-full h-8 px-2.5 rounded-lg bg-background border border-border text-xs focus:outline-none focus:border-primary/50 focus:ring-1 focus:ring-primary/20"
                placeholder="项目描述（可选）"
              />
              {error && (
                <p className="flex items-center gap-1 text-[11px] text-destructive">
                  <AlertCircle className="w-3 h-3" />
                  {error}
                </p>
              )}
              <div className="flex items-center gap-1.5">
                <Button size="sm" className="h-6 px-2.5 text-[11px]" onClick={() => void save()} loading={busy}>
                  <Check className="w-3 h-3" />
                  保存
                </Button>
                <Button size="sm" variant="secondary" className="h-6 px-2.5 text-[11px]" onClick={() => setEditing(false)}>
                  <X className="w-3 h-3" />
                  取消
                </Button>
              </div>
            </div>
          ) : (
            <>
              <h1 className="text-lg font-semibold tracking-tight truncate">{project.name}</h1>
              <button
                type="button"
                aria-label="编辑项目"
                onClick={() => setEditing(true)}
                className="w-6 h-6 rounded-md flex items-center justify-center text-muted-foreground/60 hover:text-foreground hover:bg-accent transition-colors"
              >
                <Pencil className="w-3.5 h-3.5" />
              </button>
            </>
          )}
        </div>
        {!editing && project.path && (
          <p className="mt-1 text-xs text-muted-foreground font-mono truncate flex items-center gap-1">
            <FolderOpen className="w-3 h-3 shrink-0" />
            {project.path}
          </p>
        )}
        {!editing && !project.path && <p className="mt-1 text-xs text-muted-foreground/60">未绑定目录（旧项目，可删除后重新以文件夹方式添加）</p>}
        {!editing && project.description && <p className="mt-1 text-xs text-muted-foreground">{project.description}</p>}
      </div>

      <div className="shrink-0 flex items-center gap-1.5">
        {confirming && (
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 px-6" onClick={() => !busy && setConfirming(false)}>
            <div className="w-full max-w-md bg-card border border-destructive/30 rounded-xl p-5 space-y-3 shadow-xl" onClick={(e) => e.stopPropagation()}>
              <div className="flex items-center gap-2">
                <AlertTriangle className="w-5 h-5 text-destructive shrink-0" />
                <h3 className="text-sm font-semibold">删除项目「{project.name}」？</h3>
              </div>
              <div className="text-xs text-foreground/80 space-y-1 rounded-lg bg-destructive/5 border border-destructive/20 p-3">
                <p>⚠️ 此操作<strong className="text-destructive">不可恢复</strong>，将永久删除：</p>
                <p>· 项目记录与全部 <strong>{requirementCount}</strong> 条需求（含理解版本、任务、决策、产物、执行日志）</p>
                {project.path && <p className="text-muted-foreground">· 项目目录里的文件<strong>不会</strong>被删除（{project.path}）</p>}
              </div>
              <div className="space-y-1.5">
                <p className="text-xs text-muted-foreground">输入项目名 <span className="font-mono text-foreground">{project.name}</span> 以确认：</p>
                <input
                  value={confirmText}
                  onChange={(e) => setConfirmText(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && confirmText === project.name && !busy && void remove()}
                  className="w-full h-8 px-2.5 rounded-lg bg-background border border-border text-sm focus:outline-none focus:border-destructive/50 focus:ring-1 focus:ring-destructive/20"
                  placeholder={project.name}
                />
              </div>
              {error && (
                <p className="flex items-center gap-1 text-[11px] text-destructive">
                  <AlertCircle className="w-3 h-3" />
                  {error}
                </p>
              )}
              <div className="flex items-center justify-end gap-1.5">
                <Button size="sm" variant="secondary" className="h-7 px-3 text-xs" onClick={() => setConfirming(false)} disabled={busy}>
                  取消
                </Button>
                <Button size="sm" variant="destructive" className="h-7 px-3 text-xs" onClick={() => void remove()} disabled={busy || confirmText !== project.name}>
                  {busy ? <Loader2 className="w-3 h-3 animate-spin" /> : <Trash2 className="w-3 h-3" />}
                  永久删除
                </Button>
              </div>
            </div>
          </div>
        )}
        <>
          <Button size="sm" variant="secondary" onClick={onReinit} disabled={reiniting} title="重扫目录并补种治理数据（幂等）">
            {reiniting ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <RefreshCw className="w-3.5 h-3.5" />}
            重新初始化
          </Button>
          <Button size="sm" variant="secondary" onClick={onComplete} disabled={completing || pendingOpen === null || pendingOpen === 0} title="自动发补全需求：codex 查证自答，查不到才问用户">
            {completing ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Sparkles className="w-3.5 h-3.5" />}
            治理补全{pendingOpen !== null && pendingOpen > 0 ? ` ${pendingOpen}` : ''}
          </Button>
          <div className="relative">
            <Button size="sm" variant="secondary" onClick={onExportToggle} disabled={exporting} title="把库中治理数据投影为文件（单向）">
              {exporting ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Download className="w-3.5 h-3.5" />}
              导出
            </Button>
            {exportOpen && (
              <>
                {/* 点击菜单外任意区域关闭（exportOpen 由父组件持有，toggle 即关闭） */}
                <div className="fixed inset-0 z-10" onClick={onExportToggle} />
                <div className="absolute right-0 top-9 z-20 w-64 bg-card border border-border rounded-lg shadow-lg p-1.5 space-y-0.5">
                  <button
                    type="button"
                    onClick={() => onExport('agentup-files')}
                    disabled={!project.path}
                    className="w-full flex items-start gap-2 px-2.5 py-2 rounded-md text-left hover:bg-accent disabled:opacity-40"
                  >
                    <Shield className="w-4 h-4 mt-0.5 text-primary shrink-0" />
                    <span className="text-xs">
                      agent-up 兼容文件集
                      <span className="block text-[11px] text-muted-foreground">AGENTS.md / development-process / roles 写入项目目录</span>
                    </span>
                  </button>
                  <button
                    type="button"
                    onClick={() => onExport('json-snapshot')}
                    disabled={!project.path}
                    className="w-full flex items-start gap-2 px-2.5 py-2 rounded-md text-left hover:bg-accent disabled:opacity-40"
                  >
                    <FileText className="w-4 h-4 mt-0.5 text-primary shrink-0" />
                    <span className="text-xs">
                      JSON 快照
                      <span className="block text-[11px] text-muted-foreground">治理 + 文档元数据全量备份到项目目录</span>
                    </span>
                  </button>
                </div>
              </>
            )}
          </div>
          <div className="relative">
            <Button
              size="sm"
              variant="ghost"
              className="text-muted-foreground px-2"
              aria-label="更多操作"
              onClick={() => setMoreOpen((v) => !v)}
            >
              <MoreHorizontal className="w-4 h-4" />
            </Button>
            {moreOpen && (
              <>
                {/* 点击菜单外任意区域关闭 */}
                <div className="fixed inset-0 z-10" onClick={() => setMoreOpen(false)} />
                <div className="absolute right-0 top-9 z-20 w-44 bg-card border border-border rounded-lg shadow-lg p-1.5">
                  <button
                    type="button"
                    onClick={() => {
                      setMoreOpen(false);
                      setConfirmText('');
                      setConfirming(true);
                    }}
                    className="w-full flex items-center gap-2 px-2.5 py-2 rounded-md text-left text-destructive hover:bg-destructive/10"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                    <span className="text-xs">删除项目…</span>
                  </button>
                </div>
              </>
            )}
          </div>
        </>
      </div>
    </header>
  );
}
