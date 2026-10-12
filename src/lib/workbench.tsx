// Input: Registered project directories and their file-backed task indexes.
// Output: Shared, refreshable task snapshot with per-project failures retained visibly.
// Pos: Data boundary for the task board and navigation; does not mutate project tasks.
import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from 'react';
import type { GovernanceTicket, GovernanceTicketsResult, Project } from '../../shared/types';
import { api, errorMessage } from './api';

export type WorkTicket = GovernanceTicket & { project: Project; key: string };
export type ProjectSource = { project: Project; result: GovernanceTicketsResult | null; error: string | null };
type Snapshot = { projects: Project[]; sources: ProjectSource[]; tickets: WorkTicket[]; loading: boolean; refreshing: boolean; error: string; refreshedAt: Date | null; refresh: () => Promise<void> };
const Context = createContext<Snapshot | null>(null);

export function WorkbenchProvider({ children }: { children: ReactNode }) {
  const [data, setData] = useState<Omit<Snapshot, 'refresh'>>({ projects: [], sources: [], tickets: [], loading: true, refreshing: false, error: '', refreshedAt: null });
  const alive = useRef(false);
  const reading = useRef(false);
  const queued = useRef(false);
  const refresh = useCallback(async () => {
    if (reading.current) { queued.current = true; return; }
    reading.current = true;
    queued.current = false;
    setData(previous => ({ ...previous, refreshing: true }));
    try {
      const projects = await api.projects.list();
      const results = await Promise.allSettled(projects.map(project => api.projects.tickets(project.id, false)));
      if (!alive.current) return;
      const sources = projects.map((project, i): ProjectSource => {
        const result = results[i];
        return result.status === 'fulfilled' ? { project, result: result.value, error: null } : { project, result: null, error: errorMessage(result.reason) };
      });
      const tickets = sources.flatMap(({ project, result }) => (result?.tickets ?? []).map(ticket => ({ ...ticket, project, key: `${project.id}:${ticket.id}` })));
      setData({ projects, sources, tickets, loading: false, refreshing: false, error: '', refreshedAt: new Date() });
    } catch (error) {
      if (alive.current) setData(previous => ({ ...previous, error: errorMessage(error), loading: false, refreshing: false }));
    } finally {
      reading.current = false;
      if (queued.current && alive.current) {
        queued.current = false;
        void refresh();
      }
    }
  }, []);
  useEffect(() => {
    alive.current = true;
    void refresh();
    const onFocus = () => { if (document.visibilityState === 'visible') void refresh(); };
    const timer = window.setInterval(onFocus, 15000);
    window.addEventListener('focus', onFocus);
    window.addEventListener('agentup:projects-changed', onFocus);
    document.addEventListener('visibilitychange', onFocus);
    return () => { alive.current = false; clearInterval(timer); window.removeEventListener('focus', onFocus); window.removeEventListener('agentup:projects-changed', onFocus); document.removeEventListener('visibilitychange', onFocus); };
  }, [refresh]);
  return <Context.Provider value={{ ...data, refresh }}>{children}</Context.Provider>;
}

export function useWorkbench() {
  const state = useContext(Context);
  if (!state) throw new Error('WorkbenchProvider is required');
  return state;
}

export type TaskLane = 'ready' | 'active' | 'attention' | 'done';
export const LANES: { id: TaskLane; label: string; hint: string }[] = [
  { id: 'ready', label: '待开始', hint: '准备好就交给 Agent' },
  { id: 'active', label: '进行中', hint: '项目中正在推进的任务' },
  { id: 'attention', label: '待处理', hint: '审查、阻塞与需要你的决定' },
  { id: 'done', label: '已完成', hint: '已经收口的任务' },
];
export function taskLane(ticket: GovernanceTicket): TaskLane | null {
  switch (ticket.lane) {
    case 'ready': return ticket.ready ? 'ready' : 'attention';
    case 'in_progress': return 'active';
    case 'blocked': return 'attention';
    case 'done': return 'done';
    default: return null;
  }
}
export function taskStatus(ticket: GovernanceTicket): string {
  if (taskLane(ticket) === null && ticket.lane !== 'superseded') {
    return `未知状态 · ${ticket.governance_status || ticket.lane || '未标注'}`;
  }
  const labels: Record<string, string> = { ready: ticket.ready ? '待开始' : '前置未就绪', in_progress: '进行中', blocked: '已阻塞', review_ready: '待审查', review_pass: '待收口', review_fail: '需返工', done: '已完成', superseded: '已归档' };
  return labels[ticket.governance_status] ?? `未知状态 · ${ticket.governance_status}`;
}
export function textItems(value: unknown): string[] {
  if (typeof value === 'string') return value.trim() ? [value] : [];
  if (Array.isArray(value)) return value.flatMap(textItems);
  if (value && typeof value === 'object') return Object.entries(value).flatMap(([key, item]) => textItems(item).map(text => `${key}：${text}`));
  return [];
}
export function shortDate(value: string | null) {
  if (!value) return '未记录时间';
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : new Intl.DateTimeFormat('zh-CN', { month: 'numeric', day: 'numeric' }).format(date);
}
