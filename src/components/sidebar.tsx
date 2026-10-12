// Input: Shared project/task snapshot and current route.
// Output: Stable workspace navigation, attention count and project shortcuts.
// Pos: App shell; uses the same file-backed snapshot as the board.
import { Link, NavLink } from 'react-router-dom';
import { ArrowUpRight, CircleHelp, Folder, Layers3, Plus, Settings2 } from 'lucide-react';
import { taskLane, useWorkbench } from '@/lib/workbench';
import { ThemeToggle } from './theme-toggle';

export function Sidebar() {
  const { projects, tickets, loading } = useWorkbench();
  const attentionCount = tickets.filter(ticket => taskLane(ticket) === 'attention').length;
  return <aside className="app-sidebar">
    <div className="window-drag-space" data-tauri-drag-region />
    <Link to="/" className="app-brand"><span className="brand-symbol" aria-hidden="true"><i /><i /><i /></span><span>AgentUp<span className="brand-caption">LOCAL WORKSPACE</span></span></Link>
    <nav className="main-navigation" aria-label="工作区导航">
      <NavLink to="/" end aria-label="工作台"><Layers3 size={17} aria-hidden="true" /><span>工作台</span></NavLink>
      <NavLink to="/attention" aria-label={attentionCount > 0 ? `待我处理，${attentionCount} 项` : '待我处理'}><CircleHelp size={17} aria-hidden="true" /><span>待我处理</span>{attentionCount > 0 && <span className="nav-count" aria-hidden="true">{attentionCount}</span>}</NavLink>
    </nav>
    <div className="project-nav-label"><span>项目</span><Link to="/connect" aria-label="接入项目" className="icon-control"><Plus size={16} aria-hidden="true" /></Link></div>
    <nav className="project-navigation" aria-label="项目">
      {loading && <p className="nav-empty">读取项目…</p>}
      {!loading && !projects.length && <p className="nav-empty">还没有项目</p>}
      {projects.map(project => <NavLink key={project.id} to={`/project/${project.id}`} aria-label={project.name}><Folder size={16} aria-hidden="true" /><span className="truncate">{project.name}</span><span className="project-task-count" aria-hidden="true">{tickets.filter(ticket => ticket.project.id === project.id && taskLane(ticket) !== 'done' && taskLane(ticket) !== null).length}</span></NavLink>)}
    </nav>
    <Link to="/connect" className="connect-shortcut" aria-label="接入本地项目"><Plus size={16} aria-hidden="true" /><span>接入本地项目</span><ArrowUpRight size={13} aria-hidden="true" /></Link>
    <div className="sidebar-bottom"><NavLink to="/settings" aria-label="Agent 设置"><Settings2 size={17} aria-hidden="true" /><span>Agent 设置</span></NavLink><ThemeToggle /><div className="local-status"><span />本地优先 · 文件是真相源</div></div>
  </aside>;
}
