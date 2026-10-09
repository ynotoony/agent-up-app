import { Link, useLocation, useNavigate, useParams } from 'react-router-dom';
import { useLayoutEffect, useEffect, useRef, useState } from 'react';
import { Bot, Folder, FolderInput, Loader2, LayoutDashboard, Pencil, Plus, Settings, Trash2, X, Check, AlertCircle } from 'lucide-react';
import type { Project } from '../../shared/types';
import { api, errorMessage } from '@/lib/api';
import { cn } from '@/lib/utils';
import { ThemeToggle } from './theme-toggle';
import { Button } from './ui/button';

// 全局固定左栏：品牌 + 工作台 + 项目列表 + 底部设置/主题切换（DESIGN 4）。
export function Sidebar() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [loading, setLoading] = useState(true);
  const navigate = useNavigate();
  const location = useLocation();

  const fetchProjects = () => {
    api.projects
      .list()
      .then(setProjects)
      .catch(() => setProjects([]))
      .finally(() => setLoading(false));
  };

  useLayoutEffect(fetchProjects, [location.pathname]);

  // 队列消化中需求会持续进场（initializing → 可见），5s 轮询保持侧栏计数与项目页一致
  useEffect(() => {
    const timer = window.setInterval(fetchProjects, 5000);
    return () => window.clearInterval(timer);
  }, []);

  return (
    <aside className="w-60 shrink-0 h-screen flex flex-col bg-sidebar text-sidebar-foreground border-r border-border">
      <div data-tauri-drag-region className="drag-region h-14 pl-[76px] pr-3 flex items-center gap-2 select-none">
        <span className="no-drag inline-flex items-center justify-center w-7 h-7 shrink-0 rounded-lg bg-primary/10 text-primary">
          <Bot className="w-4 h-4" />
        </span>
        <div className="no-drag min-w-0">
          <p className="text-[13px] font-semibold tracking-tight leading-none whitespace-nowrap truncate">AgentUp Harness</p>
          <p className="mt-1 text-[10px] text-muted-foreground whitespace-nowrap truncate">面向结果的 Agent 交付</p>
        </div>
      </div>

      <nav className="px-3 mt-2 space-y-0.5">
        <SidebarLink to="/" active={location.pathname === '/'} icon={<LayoutDashboard className="w-4 h-4" />}>
          工作台
        </SidebarLink>
        <SidebarLink to="/connect" active={location.pathname === '/connect'} icon={<FolderInput aria-hidden="true" className="w-4 h-4" />}>
          接入项目
        </SidebarLink>
      </nav>

      <div className="mt-5 px-3 flex items-center justify-between">
        <span className="text-xs font-medium text-muted-foreground px-2">项目</span>
        <button
          type="button"
          aria-label="接入项目"
          onClick={() => navigate('/connect')}
          className="w-5 h-5 rounded flex items-center justify-center text-muted-foreground hover:text-primary hover:bg-accent transition-colors disabled:opacity-50"
        >
          <Plus className="w-3.5 h-3.5" />
        </button>
      </div>

      <div className="flex-1 min-h-0 overflow-y-auto scrollbar-thin px-3 py-1.5 space-y-0.5">
        {loading ? (
          <div className="px-2 py-2 space-y-2 animate-pulse">
            {[0, 1, 2].map((i) => (
              <div key={i} className="h-7 rounded bg-muted" />
            ))}
          </div>
        ) : projects.length === 0 ? (
          <p className="px-2 py-2 text-xs text-muted-foreground/50">还没有项目，点击 + 添加</p>
        ) : (
          projects.map((project) => <ProjectRow key={project.id} project={project} onChanged={fetchProjects} />)
        )}
      </div>

      <div className="border-t border-border p-3 space-y-0.5">
        <SidebarLink to="/settings" active={location.pathname === '/settings'} icon={<Settings className="w-4 h-4" />}>
          设置
        </SidebarLink>
        <ThemeToggle />
      </div>
    </aside>
  );
}

function SidebarLink({
  to,
  active,
  icon,
  children,
}: {
  to: string;
  active: boolean;
  icon: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <Link
      to={to}
      className={cn(
        'flex items-center gap-2.5 px-2 py-1.5 rounded-md text-sm transition-colors',
        active ? 'bg-primary/10 text-primary' : 'text-foreground/80 hover:bg-accent hover:text-foreground'
      )}
    >
      {icon}
      {children}
    </Link>
  );
}

function ProjectRow({ project, onChanged }: { project: Project; onChanged: () => void }) {
  const params = useParams();
  const location = useLocation();
  const navigate = useNavigate();
  const active = params.id === project.id && location.pathname.startsWith('/project/');
  const [editing, setEditing] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [name, setName] = useState(project.name);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);

  useLayoutEffect(() => {
    if (editing) inputRef.current?.focus();
  }, [editing]);

  const saveRename = async () => {
    const trimmed = name.trim();
    if (!trimmed || trimmed === project.name) {
      setEditing(false);
      setName(project.name);
      return;
    }
    setBusy(true);
    setError('');
    try {
      await api.projects.update(project.id, { name: trimmed });
      setEditing(false);
      onChanged();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const confirmDelete = async () => {
    setBusy(true);
    setError('');
    try {
      await api.projects.remove(project.id);
      setConfirming(false);
      onChanged();
      if (active) navigate('/');
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  if (confirming) {
    return (
      <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-2.5">
            <p className="text-xs text-foreground">删除「{project.name}」？其下需求与记录将一并删除，<span className="text-destructive font-medium">不可恢复</span>。</p>
        {error && (
          <p className="mt-1.5 flex items-center gap-1 text-[11px] text-destructive">
            <AlertCircle className="w-3 h-3" />
            {error}
          </p>
        )}
        <div className="mt-2 flex items-center gap-1.5">
          <Button size="sm" variant="destructive" className="h-6 px-2.5 text-[11px]" onClick={confirmDelete} disabled={busy}>
            {busy ? <Loader2 className="w-3 h-3 animate-spin" /> : <Check className="w-3 h-3" />}
            确认删除
          </Button>
          <Button size="sm" variant="secondary" className="h-6 px-2.5 text-[11px]" onClick={() => setConfirming(false)} disabled={busy}>
            取消
          </Button>
        </div>
      </div>
    );
  }

  if (editing) {
    return (
      <div className="flex items-center gap-1 px-1.5 py-1">
        <input
          ref={inputRef}
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') void saveRename();
            if (e.key === 'Escape') {
              setEditing(false);
              setName(project.name);
            }
          }}
          className="flex-1 min-w-0 h-7 px-2 rounded-md bg-background border border-border text-sm focus:outline-none focus:border-primary/50 focus:ring-1 focus:ring-primary/20"
        />
        <button
          type="button"
          aria-label="保存重命名"
          onClick={() => void saveRename()}
          disabled={busy}
          className="w-6 h-6 rounded flex items-center justify-center text-primary hover:bg-accent disabled:opacity-50"
        >
          {busy ? <Loader2 className="w-3 h-3 animate-spin" /> : <Check className="w-3.5 h-3.5" />}
        </button>
        <button
          type="button"
          aria-label="取消重命名"
          onClick={() => {
            setEditing(false);
            setName(project.name);
            setError('');
          }}
          className="w-6 h-6 rounded flex items-center justify-center text-muted-foreground hover:bg-accent"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>
    );
  }

  return (
    <div className="group flex items-center gap-1">
      <Link
        to={`/project/${project.id}`}
        className={cn(
          'flex-1 min-w-0 flex items-center gap-2 px-2 py-1.5 rounded-md text-sm transition-colors',
          active ? 'bg-primary/10 text-primary' : 'text-foreground/80 hover:bg-accent hover:text-foreground'
        )}
      >
        <Folder className="w-4 h-4 shrink-0 text-muted-foreground group-hover:text-current" />
        <span className="truncate">{project.name}</span>
      </Link>
      <div className="hidden group-hover:flex items-center">
        <button
          type="button"
          aria-label={`重命名 ${project.name}`}
          onClick={() => setEditing(true)}
          className="w-5 h-5 rounded flex items-center justify-center text-muted-foreground/70 hover:text-foreground hover:bg-accent transition-colors"
        >
          <Pencil className="w-3 h-3" />
        </button>
        <button
          type="button"
          aria-label={`删除 ${project.name}`}
          onClick={() => setConfirming(true)}
          className="w-5 h-5 rounded flex items-center justify-center text-muted-foreground/70 hover:text-destructive hover:bg-destructive/10 transition-colors"
        >
          <Trash2 className="w-3 h-3" />
        </button>
      </div>
    </div>
  );
}
