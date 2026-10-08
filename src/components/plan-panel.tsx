import { ClipboardList, RotateCcw, ShieldCheck } from 'lucide-react';
import type { Task, TaskResultPlan } from '../../shared/types';
import { PanelHeader } from './understanding-panel';
import { Button } from './ui/button';
import { EmptyState } from './ui/empty';
import { Spinner } from './ui/button';

// 方案面板：展示方案任务的产出（approach/steps/considerations）。
export function PlanPanel({
  planTask,
  planning,
  onRetry,
  retrying,
}: {
  planTask: Task | null;
  planning: boolean;
  onRetry?: () => void;
  retrying?: boolean;
}) {
  if (!planTask) {
    return (
      <div className="bg-card border border-border rounded-xl">
        <PanelHeader title="实施方案" icon={<ClipboardList className="w-3.5 h-3.5" />} right={planning ? <Spinner /> : undefined} />
        <EmptyState
          className="py-12"
          icon={<ClipboardList className="w-10 h-10" />}
          title={planning ? '方案中…' : '方案尚未生成'}
          description={planning ? '正在基于理解结果编排实施步骤' : undefined}
        />
      </div>
    );
  }

  if (planTask.status === 'failed') {
    return (
      <div className="bg-card border border-destructive/30 rounded-xl">
        <PanelHeader title="实施方案" icon={<ClipboardList className="w-3.5 h-3.5" />} />
        <div className="p-5 space-y-3">
          <p className="text-sm text-destructive leading-relaxed break-all">
            方案制定失败：{planTask.error_message ?? '未知错误'}
          </p>
          {onRetry && (
            <Button size="sm" variant="secondary" onClick={onRetry} loading={retrying}>
              {!retrying && <RotateCcw className="w-3.5 h-3.5" />}
              重试
            </Button>
          )}
        </div>
      </div>
    );
  }

  const result = planTask.result as TaskResultPlan | null;
  return (
    <div className="bg-card border border-border rounded-xl overflow-hidden">
      <PanelHeader
        title="实施方案"
        icon={<ClipboardList className="w-3.5 h-3.5" />}
        right={planTask.status === 'running' ? <Spinner /> : undefined}
      />
      <div className="p-5 space-y-5">
        {result?.approach && (
          <section>
            <h3 className="text-xs font-medium text-muted-foreground">总体思路</h3>
            <p className="mt-1.5 text-sm leading-relaxed text-foreground/90">{result.approach}</p>
          </section>
        )}
        {result?.steps && result.steps.length > 0 && (
          <section>
            <h3 className="text-xs font-medium text-muted-foreground">实施步骤</h3>
            <ol className="mt-2 space-y-1.5">
              {result.steps.map((step, i) => (
                <li key={i} className="flex items-start gap-2.5 text-sm text-foreground/90">
                  <span className="inline-flex items-center justify-center w-5 h-5 mt-0.5 shrink-0 rounded-full bg-primary/10 text-primary text-[11px] font-medium">
                    {i + 1}
                  </span>
                  <span className="leading-relaxed">{step}</span>
                </li>
              ))}
            </ol>
          </section>
        )}
        {result?.considerations && result.considerations.length > 0 && (
          <section>
            <h3 className="text-xs font-medium text-muted-foreground flex items-center gap-1.5">
              <ShieldCheck className="w-3.5 h-3.5" />
              风险与考量
            </h3>
            <ul className="mt-2 space-y-1">
              {result.considerations.map((item, i) => (
                <li key={i} className="text-sm text-foreground/90 leading-relaxed">
                  · {item}
                </li>
              ))}
            </ul>
          </section>
        )}
        {planTask.completed_at && (
          <p className="text-[11px] text-muted-foreground/50">方案生成于 {new Date(planTask.completed_at).toLocaleString('zh-CN')}</p>
        )}
      </div>
    </div>
  );
}
