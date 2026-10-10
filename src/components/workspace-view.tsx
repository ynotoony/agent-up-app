import { Link } from 'react-router-dom';
import { useEffect, useState } from 'react';
import { FolderInput, Gavel, Plus } from 'lucide-react';
import type { Decision, Project, WorkspaceData } from '../../shared/types';
import { api, errorMessage } from '@/lib/api';
import { cn } from '@/lib/utils';
import { RequirementInput } from './requirement-input';
import { KanbanBoard } from './kanban';
import { Button } from './ui/button';

// 工作台：原生项目进入目标工作区；仅明确没有原生档案的项目可创建旧需求。
export function WorkspaceView({ onCreated }: { onCreated: (requirementId: string) => void }) {
  const [data, setData] = useState<WorkspaceData | null>(null);
  const [nativeProjects, setNativeProjects] = useState<Project[]>([]);
  const [legacyProjects, setLegacyProjects] = useState<Project[]>([]);
  const [profileErrors, setProfileErrors] = useState<string[]>([]);
  const [error, setError] = useState('');
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let active = true;
    setError('');
    api.workspace
      .get()
      .then(async (workspace) => {
        const profiles = await Promise.allSettled(workspace.projects.map((project) => api.projects.nativeProfile(project.id)));
        if (!active) return;
        const native: Project[] = [];
        const legacy: Project[] = [];
        const errors: string[] = [];
        profiles.forEach((profile, index) => {
          const project = workspace.projects[index];
          if (profile.status === 'rejected') errors.push(`${project.name}：${errorMessage(profile.reason)}`);
          else if (profile.value === null) legacy.push(project);
          else native.push(project);
        });
        setNativeProjects(native);
        setLegacyProjects(legacy);
        setProfileErrors(errors);
        setData(workspace);
      })
      .catch((err) => { if (active) setError(errorMessage(err)); });
    return () => { active = false; };
  }, [attempt]);

  if (error) return <div className="space-y-3"><ErrorPanel message={error} /><Button variant="secondary" onClick={() => setAttempt((value) => value + 1)}>重新读取</Button></div>;
  if (!data) return <SkeletonPanel />;

  return (
    <div className="flex flex-col gap-4 h-full min-h-0 overflow-y-auto scrollbar-thin">
      {profileErrors.length > 0 && <div className="shrink-0 space-y-2"><ErrorPanel message={`部分项目档案读取失败，已暂停其新建入口：${profileErrors.join('；')}`} /><Button variant="secondary" size="sm" onClick={() => setAttempt((value) => value + 1)}>重新读取项目档案</Button></div>}
      <section className="shrink-0 space-y-3">
        <div className="flex items-center justify-between gap-3"><h2 className="text-sm font-medium">项目目标</h2><Link to="/connect" className="inline-flex items-center gap-1.5 rounded-lg px-2 py-1.5 text-xs text-primary hover:bg-primary/10 focus-visible:ring-2 focus-visible:ring-primary/30"><FolderInput aria-hidden="true" className="h-3.5 w-3.5" />接入项目</Link></div>
        {nativeProjects.length > 0 ? <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">{nativeProjects.map((project) => <Link key={project.id} to={`/project/${project.id}`} className="min-w-0 rounded-xl border border-border bg-card p-4 outline-none transition-colors hover:border-primary/40 hover:bg-accent focus-visible:ring-2 focus-visible:ring-primary/30"><h3 className="truncate text-sm font-medium">{project.name}</h3><p className="mt-1 truncate text-xs text-muted-foreground">{project.path}</p></Link>)}</div> : <div className="rounded-xl border border-border bg-card p-5"><p className="text-sm font-medium">先接入一个项目，再创建目标</p><p className="mt-1 text-xs leading-5 text-muted-foreground">选择已有 Git 项目，确认只读扫描结果后，就可以规划任务。</p><Link to="/connect" className="mt-4 inline-flex rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 focus-visible:ring-2 focus-visible:ring-primary/30">接入项目</Link></div>}
      </section>
      {legacyProjects.length > 0 && <section className="shrink-0 space-y-2 rounded-xl border border-dashed border-border p-4"><div><h2 className="text-xs font-medium text-muted-foreground">兼容流程 · 存量项目需求</h2><p className="mt-1 text-xs leading-5 text-muted-foreground">这是旧版需求流水线，仅用于尚未建立原生项目档案的项目。新工作从上面的“项目目标”进入。</p></div><RequirementInput projects={legacyProjects} onCreated={onCreated} /></section>}
      {data.requirements.length > 0 && <section className="flex min-h-72 flex-1 flex-col gap-2">
        <div><h2 className="shrink-0 text-xs font-medium text-muted-foreground">兼容流程 · 需求记录</h2><p className="mt-1 text-xs text-muted-foreground">历史记录保留用于追溯，不作为原生目标交付入口。</p></div>
        <div className="flex-1 min-h-0">
        <KanbanBoard requirements={data.requirements} />
        </div>
      </section>}
    </div>
  );
}

export function PendingDecisionsPanel({
  decisions,
}: {
  decisions: (Decision & { requirement_content?: string; project_name?: string })[];
}) {
  if (decisions.length === 0) return null;
  return (
    <section className="space-y-3">
      <h2 className="text-sm font-medium text-muted-foreground px-1">待决策</h2>
      <div className="space-y-3">
        {decisions.map((decision) => (
          <Link
            key={decision.id}
            to={`/requirement/${decision.requirement_id}`}
            className={cn(
              'block bg-card border border-amber-600/30 dark:border-amber-400/20 rounded-xl p-4',
              'transition-colors hover:bg-accent/40'
            )}
          >
            <div className="flex items-center gap-1.5 text-xs text-amber-600 dark:text-amber-400">
              <Gavel className="w-3.5 h-3.5" />
              <span>{decision.project_name ? `${decision.project_name} · ` : ''}需要你的决定</span>
            </div>
            <p className="mt-1.5 text-sm text-foreground">{decision.question}</p>
            {decision.requirement_content && (
              <p className="mt-1 text-xs text-muted-foreground/60 line-clamp-1">{decision.requirement_content}</p>
            )}
          </Link>
        ))}
      </div>
    </section>
  );
}

export function ErrorPanel({ message }: { message: string }) {
  return (
    <div className="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{message}</div>
  );
}

export function SkeletonPanel() {
  return (
    <div className="space-y-4 animate-pulse">
      <div className="h-36 rounded-xl bg-card border border-border" />
      <div className="grid grid-cols-2 md:grid-cols-6 gap-3">
        {[0, 1, 2, 3, 4, 5].map((i) => (
          <div key={i} className="h-64 rounded-xl bg-card border border-border" />
        ))}
      </div>
    </div>
  );
}
