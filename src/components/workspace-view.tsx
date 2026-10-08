import { Link, useNavigate } from 'react-router-dom';
import { useLayoutEffect, useState } from 'react';
import { Gavel } from 'lucide-react';
import type { Decision, WorkspaceData } from '../../shared/types';
import { api, errorMessage } from '@/lib/api';
import { cn } from '@/lib/utils';
import { RequirementInput } from './requirement-input';
import { KanbanBoard } from './kanban';

// 工作台（首页）主体：一句话需求录入 + 全量需求看板（数据全量来自 SQLite）。
export function WorkspaceView({ onCreated }: { onCreated: (requirementId: string) => void }) {
  const [data, setData] = useState<WorkspaceData | null>(null);
  const [error, setError] = useState('');

  const refresh = () => {
    api.workspace
      .get()
      .then(setData)
      .catch((err) => setError(errorMessage(err)));
  };

  useLayoutEffect(refresh, []);

  if (error) return <ErrorPanel message={error} />;
  if (!data) return <SkeletonPanel />;

  return (
    <div className="flex flex-col gap-4 h-full min-h-0">
      <RequirementInput projects={data.projects} onCreated={onCreated} />
      <div className="flex-1 min-h-0">
        <KanbanBoard requirements={data.requirements} />
      </div>
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
              'transition-all hover:bg-accent/40'
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
