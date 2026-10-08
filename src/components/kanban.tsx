import { Link } from 'react-router-dom';
import { format } from 'date-fns';
import { zhCN } from 'date-fns/locale';
import { FileText } from 'lucide-react';
import type { Requirement, RequirementStatus } from '../../shared/types';
import { cn } from '@/lib/utils';

// 看板：需求状态 → 泳道。列头点色即泳道主色，列内卡片用状态点区分合并进来的具体状态。
const COLUMNS: { label: string; dot: string; statuses: RequirementStatus[] }[] = [
  { label: '理解中', dot: 'bg-blue-500', statuses: ['pending', 'initializing', 'understanding', 'questioning'] },
  { label: '待处理', dot: 'bg-amber-500', statuses: ['awaiting_confirmation', 'waiting_decision', 'failed'] },
  { label: '执行中', dot: 'bg-primary', statuses: ['planning', 'implementing', 'verifying'] },
  { label: '已完成', dot: 'bg-emerald-500', statuses: ['completed'] },
];

const STATUS_DOT: Record<RequirementStatus, string> = {
  pending: 'bg-gray-400',
  initializing: 'bg-violet-500 animate-pulse',
  understanding: 'bg-blue-500 animate-pulse',
  questioning: 'bg-amber-500 animate-pulse',
  awaiting_confirmation: 'bg-amber-500',
  planning: 'bg-primary animate-pulse',
  implementing: 'bg-blue-500 animate-pulse',
  verifying: 'bg-cyan-500 animate-pulse',
  completed: 'bg-emerald-500',
  failed: 'bg-red-500',
  waiting_decision: 'bg-gray-400',
};

const STATUS_SHORT: Record<RequirementStatus, string> = {
  pending: '排队中',
  initializing: '初始化中',
  understanding: '理解中',
  questioning: '质疑中',
  awaiting_confirmation: '待处理',
  planning: '方案中',
  implementing: '实施中',
  verifying: '验证中',
  completed: '已完成',
  failed: '失败',
  waiting_decision: '待决策',
};

// 看板视图：四列等宽占满工作区 + 纵向卡片流；只读监控（状态流转由 agent 驱动），点击卡片进详情。
export function KanbanBoard({ requirements, showProject = true }: { requirements: Requirement[]; showProject?: boolean }) {
  return (
    <div className="h-full min-h-0 flex gap-3 pb-1">
      {COLUMNS.map((column) => {
        const cards = requirements.filter((r) => column.statuses.includes(r.status));
        return (
          <section key={column.label} className="flex-1 min-w-0 flex flex-col rounded-xl border border-border bg-muted/30 overflow-hidden">
            <header className="shrink-0 flex items-center gap-2 px-3 py-2.5 border-b border-border bg-card">
              <span className={cn('w-2 h-2 rounded-full shrink-0', column.dot)} />
              <span className="text-xs font-medium text-foreground/90">{column.label}</span>
              <span className="ml-auto text-xs text-muted-foreground tabular-nums">{cards.length}</span>
            </header>
            <div className="flex-1 min-h-0 overflow-y-auto scrollbar-thin px-2 py-2 space-y-2">
              {cards.length === 0 ? (
                <p className="px-1.5 py-3 text-[11px] text-muted-foreground/50">暂无需求</p>
              ) : (
                cards.map((requirement) => (
                  <KanbanCard key={requirement.id} requirement={requirement} showProject={showProject} />
                ))
              )}
            </div>
          </section>
        );
      })}
    </div>
  );
}

// 标题回答「这是干嘛的」：理解产物 goal_summary 优先；未理解的存量需求用文档路径当标题（文件名比原文内脏可读）；
// 普通需求直接显示用户原话。来源走结构化字段 source_doc_path，不做任何内容解析。
function cardTitle(requirement: Requirement): { title: string; sourcePath: string | null } {
  if (requirement.goal_summary) return { title: requirement.goal_summary, sourcePath: requirement.source_doc_path ?? null };
  if (requirement.source_doc_path) return { title: requirement.source_doc_path, sourcePath: requirement.source_doc_path };
  return { title: requirement.content_preview, sourcePath: null };
}

function KanbanCard({ requirement, showProject }: { requirement: Requirement; showProject?: boolean }) {
  const isFailed = requirement.status === 'failed';
  const needsUser = isFailed || ['awaiting_confirmation', 'waiting_decision'].includes(requirement.status);
  const { title, sourcePath } = cardTitle(requirement);
  return (
    <Link
      to={`/requirement/${requirement.id}`}
      className={cn(
        'block rounded-lg border border-border bg-card px-2.5 py-2 space-y-1 transition-colors hover:border-primary/40 hover:bg-accent/40',
        needsUser && 'border-l-2',
        isFailed
          ? 'border-l-red-500/70 dark:border-l-red-400/70'
          : needsUser
            ? 'border-l-amber-500/70 dark:border-l-amber-400/70'
            : '',
        requirement.status === 'completed' && 'opacity-75',
      )}
    >
      {/* 标题：单行截断，存量文档需求前挂小文档图标作为来源暗示 */}
      <p className="flex items-center gap-1 min-w-0">
        {sourcePath && <FileText className="w-3 h-3 shrink-0 text-muted-foreground/50" aria-hidden />}
        <span className="truncate text-xs font-medium text-foreground/90">{title}</span>
      </p>
      {/* meta：全部弱化灰字，单行截断；状态点+子状态是唯一的彩色锚点 */}
      <p className="flex items-center gap-1 text-[10px] text-muted-foreground/60 min-w-0">
        <span className={cn('w-1.5 h-1.5 rounded-full shrink-0', STATUS_DOT[requirement.status])} />
        <span className="shrink-0">{STATUS_SHORT[requirement.status]}</span>
        {sourcePath ? <span className="truncate text-muted-foreground/40">· {sourcePath}</span> : showProject && requirement.project_name ? <span className="shrink-0">· {requirement.project_name}</span> : null}
        <span className="ml-auto shrink-0 tabular-nums">{format(new Date(requirement.created_at), 'MM/dd HH:mm', { locale: zhCN })}</span>
      </p>
    </Link>
  );
}
