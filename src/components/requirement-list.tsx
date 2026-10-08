import { Link } from 'react-router-dom';
import { useState } from 'react';
import { format } from 'date-fns';
import { zhCN } from 'date-fns/locale';
import type { Requirement } from '../../shared/types';
import { cn, formatTokens } from '@/lib/utils';
import { Badge } from './ui/badge';
import { EmptyState } from './ui/empty';
import { MODE_CONFIG, STATUS_CONFIG, stageOf } from './ui/status';
import { TabPills } from './ui/tabs';
import { Inbox } from 'lucide-react';

type StageFilter = 'all' | 'requirement' | 'plan' | 'implement' | 'done';

// 需求卡片列表：阶段徽章 + 状态/模式徽章 + 内容 + 归属项目 + 时间；阶段 Tab 筛选。
export function RequirementList({
  requirements,
  showProject,
  filterable = true,
  className,
}: {
  requirements: Requirement[];
  showProject?: boolean;
  filterable?: boolean;
  className?: string;
}) {
  const [filter, setFilter] = useState<StageFilter>('all');
  // initializing（文档引入、理解归类中）不在需求列表展示，理解完成后自然出现
  const visible = requirements.filter((r) => r.status !== 'initializing');
  const filtered = filter === 'all' ? visible : visible.filter((r) => stageOf(r.status) === filter);

  return (
    <div className={cn('space-y-3', className)}>
      {filterable && requirements.length > 0 && (
        <TabPills
          value={filter}
          onChange={setFilter}
          options={[
            { value: 'all', label: '全部' },
            { value: 'requirement', label: '需求阶段' },
            { value: 'plan', label: '方案阶段' },
            { value: 'implement', label: '实施阶段' },
            { value: 'done', label: '已完成' },
          ]}
          className="self-start"
        />
      )}
      {filtered.length === 0 ? (
        <div className="bg-card border border-border rounded-xl">
          <EmptyState icon={<Inbox className="w-10 h-10" />} title={visible.length === 0 ? '还没有需求' : '该阶段暂无需求'} description="在上方输入一句话需求即可开始" />
        </div>
      ) : (
        <div className="bg-card border border-border rounded-xl divide-y divide-border overflow-hidden">
          {filtered.map((requirement) => (
            <RequirementRow key={requirement.id} requirement={requirement} showProject={showProject} />
          ))}
        </div>
      )}
    </div>
  );
}

// 需求行：状态徽章是唯一彩色锚点；正文截断为单行标题；复杂度/版本靠右弱化；
// 其余（时间/预估/tokens/模式/车道）全部降级到 meta 行。需用户介入的行加左色条。
const ACTION_NEEDED_STATUSES = ['questioning', 'awaiting_confirmation', 'waiting_decision'];

function RequirementRow({ requirement, showProject }: { requirement: Requirement; showProject?: boolean }) {
  const status = STATUS_CONFIG[requirement.status];
  const overBudget =
    requirement.estimated_minutes !== null &&
    requirement.actual_minutes !== null &&
    requirement.estimated_minutes > 0 &&
    requirement.actual_minutes > requirement.estimated_minutes * 1.2;
  const dimmed = requirement.status === 'completed';
  const accent = requirement.status === 'failed'
    ? 'border-l-2 border-l-red-500/70 dark:border-l-red-400/70'
    : ACTION_NEEDED_STATUSES.includes(requirement.status)
      ? 'border-l-2 border-l-amber-500/70 dark:border-l-amber-400/70'
      : '';
  const laneLabel =
    requirement.lane && requirement.lane !== 'full'
      ? ({ user_review: '用户即Review', micro: '微任务道' }[requirement.lane] ?? requirement.lane)
      : null;
  const loudMode = requirement.mode === 'high_risk' || requirement.mode === 'emergency';

  return (
    <Link to={`/requirement/${requirement.id}`} className={cn('block px-4 py-2.5 transition-colors hover:bg-accent/40', accent)}>
      <div className="flex items-center gap-2 min-w-0">
        <Badge className={cn('shrink-0', status.className)} dot={status.pulse}>
          {status.label}
        </Badge>
        <p className={cn('flex-1 min-w-0 truncate text-sm font-medium', dimmed && 'text-foreground/55')}>{requirement.content_preview}</p>
        {requirement.complexity && (
          <span className="shrink-0 text-[11px] font-mono text-muted-foreground/70 bg-muted rounded px-1.5 py-0.5">{requirement.complexity}</span>
        )}
        {requirement.current_version > 1 && <span className="shrink-0 text-[11px] text-muted-foreground/50">v{requirement.current_version}</span>}
      </div>
      <p className="mt-1 pl-0.5 text-[11px] text-muted-foreground/60 flex items-center flex-wrap">
        {showProject && requirement.project_name && <span>{requirement.project_name} · </span>}
        <span>{format(new Date(requirement.created_at), 'MM月dd日 HH:mm', { locale: zhCN })}</span>
        {requirement.estimated_minutes !== null && <span>· 预计 {requirement.estimated_minutes}min</span>}
        {requirement.actual_minutes !== null && (
          <span className={cn(overBudget && 'text-destructive font-medium')}>· 实际 {requirement.actual_minutes}min</span>
        )}
        {requirement.total_tokens !== null && <span>· {formatTokens(requirement.total_tokens)} tokens</span>}
        <span className={cn(loudMode && 'text-amber-600 dark:text-amber-400 font-medium')}>· {MODE_CONFIG[requirement.mode].shortLabel}</span>
        {laneLabel && <span>· {laneLabel}</span>}
      </p>
    </Link>
  );
}
