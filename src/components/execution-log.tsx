import { ScrollText } from 'lucide-react';
import type { ExecutionLog } from '../../shared/types';
import { cn } from '@/lib/utils';
import { PanelHeader } from './understanding-panel';
import { EmptyState } from './ui/empty';

const LEVEL_STYLE: Record<ExecutionLog['level'], string> = {
  info: 'text-muted-foreground',
  warn: 'text-amber-600 dark:text-amber-400',
  error: 'text-red-600 dark:text-red-400',
};

// 执行日志：最新在前。
export function ExecutionLog({ logs }: { logs: ExecutionLog[] }) {
  const ordered = [...logs].reverse();
  return (
    <div className="bg-card border border-border rounded-xl overflow-hidden">
      <PanelHeader title={`执行日志（${logs.length}）`} icon={<ScrollText className="w-3.5 h-3.5" />} />
      {ordered.length === 0 ? (
        <EmptyState className="py-12" icon={<ScrollText className="w-10 h-10" />} title="暂无日志" />
      ) : (
        <div className="p-4 space-y-1 max-h-[420px] overflow-y-auto scrollbar-thin">
          {ordered.map((log) => (
            <div key={log.id} className="flex items-start gap-2.5 px-2 py-1.5 rounded-md hover:bg-accent/30 transition-colors">
              <span className="text-[10px] font-mono text-muted-foreground/50 mt-[3px] shrink-0 w-14 text-right">
                {new Date(log.created_at).toLocaleTimeString('zh-CN', { hour12: false })}
              </span>
              <span className={cn('px-1.5 rounded text-[10px] leading-[18px] shrink-0 bg-muted', LEVEL_STYLE[log.level])}>
                {log.step}
              </span>
              <span className={cn('text-xs leading-relaxed', log.level === 'error' ? 'text-red-600 dark:text-red-400' : 'text-foreground/85')}>
                {log.message}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
