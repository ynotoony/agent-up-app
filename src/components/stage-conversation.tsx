import { useEffect, useRef, useState } from 'react';
import { format } from 'date-fns';
import { Bot, Loader2, Square } from 'lucide-react';
import type { ExecutionLog } from '../../shared/types';
import { api } from '@/lib/api';
import { cn } from '@/lib/utils';
import { Button } from '@/components/ui/button';

// 阶段对话流（全阶段统一交互）：理解/方案/实施/验证/审查的运行态都用同一套
// AI 对话流呈现 —— 圆形头像气泡、逐条流入、末尾打字光标、自动滚底、秒级运行计时。

export type ConversationState = 'running' | 'waiting' | 'failed';

const STATE_DOT: Record<ConversationState, string> = {
  running: 'bg-blue-600 dark:bg-blue-400 animate-pulse',
  waiting: 'bg-amber-600 dark:bg-amber-400 animate-pulse',
  failed: 'bg-red-600 dark:bg-red-400',
};

function formatElapsed(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  return `${Math.floor(seconds / 60)}m${String(seconds % 60).padStart(2, '0')}s`;
}

export function StageConversation({
  stageLabel,
  logs,
  executor,
  runtimeKind = null,
  startedAt,
  completedAt = null,
  failureMessage = null,
  state = 'running',
  streamText = null,
  streamTicking = false,
  onCancel = null,
  cancelling = false,
}: {
  stageLabel: string;
  /** 该阶段日志（时间正序），由调用方按阶段过滤 */
  logs: ExecutionLog[];
  executor: string | null;
  /** 当前 runtime 种类（zcode/codex/opencode）：读日志 details.runtime_kind，替代按名称子串嗅探 */
  runtimeKind?: string | null;
  startedAt: string | null;
  /** 失败态的结束时间：计时冻结为真实耗时 */
  completedAt?: string | null;
  /** 失败态的错误原因，气泡流末尾展示 */
  failureMessage?: string | null;
  state?: ConversationState;
  /** 流式文本（token 增量累计或最近消息）：有值时在气泡流末尾追加实时流 */
  streamText?: string | null;
  /** 流仍在推进：显示打字光标 */
  streamTicking?: boolean;
  /** 取消当前运行（有值时头部出现取消按钮） */
  onCancel?: (() => void) | null;
  cancelling?: boolean;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [now, setNow] = useState(() => Date.now());

  // 秒级心跳：驱动「已运行 Xs」实时跳动，不依赖页面 5s 轮询的重渲染
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);
  // 新日志到来自动滚到底部，始终看最新进展
  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: 'smooth' });
  }, [logs.length, streamText]);

  const endMs = state === 'failed' && completedAt ? new Date(completedAt).getTime() : now;
  const elapsed = startedAt ? Math.max(0, Math.floor((endMs - new Date(startedAt).getTime()) / 1000)) : null;
  const heading = state === 'waiting' ? '等待你的决策' : `${stageLabel}${state === 'failed' ? '失败' : '进行中'}`;

  return (
    <div className="bg-card border border-border rounded-xl overflow-hidden">
      <div className="px-4 py-3 border-b border-border bg-muted/40 flex items-center gap-2">
        <span className={cn('w-2 h-2 rounded-full shrink-0', STATE_DOT[state])} />
        <span className="text-sm font-medium">{heading}</span>
        {executor && <span className="text-xs text-muted-foreground truncate">执行 Agent：{executor}</span>}
        {elapsed !== null && (
          <span className="ml-auto text-[11px] text-muted-foreground/70 tabular-nums shrink-0">
            {state === 'failed' ? '耗时' : '已运行'} {formatElapsed(elapsed)}
          </span>
        )}
        {onCancel && state === 'running' && (
          <Button
            size="sm"
            variant="secondary"
            className="h-7 px-2.5 text-xs gap-1 shrink-0 text-red-600 dark:text-red-400 hover:bg-red-600/10 dark:hover:bg-red-400/10 border-red-600/20 dark:border-red-400/20"
            onClick={onCancel}
            disabled={cancelling}
          >
            {cancelling ? <Loader2 className="w-3 h-3 animate-spin" /> : <Square className="w-3 h-3" />}
            取消
          </Button>
        )}
      </div>
      <div ref={scrollRef} className="px-4 py-3 space-y-2.5 max-h-[420px] overflow-y-auto scrollbar-thin">
        {logs.map((log, i) => (
          <div key={log.id} className="flex gap-2.5 items-start">
            <span
              className={cn(
                'w-5 h-5 rounded-full flex items-center justify-center shrink-0 mt-0.5',
                log.level === 'error' ? 'bg-red-600/10 text-red-600 dark:text-red-400' : 'bg-primary/10 text-primary'
              )}
            >
              <Bot className="w-3 h-3" />
            </span>
            <div className="min-w-0 flex-1">
              <span className="text-[10px] text-muted-foreground/50 font-mono">{format(new Date(log.created_at), 'HH:mm:ss')}</span>
              <p
                className={cn(
                  'text-xs leading-relaxed whitespace-pre-wrap',
                  log.level === 'error'
                    ? 'text-red-600 dark:text-red-400'
                    : log.level === 'warn'
                      ? 'text-amber-600 dark:text-amber-400'
                      : 'text-foreground/85'
                )}
              >
                {log.message}
                {state === 'running' && i === logs.length - 1 && (
                  <span className="inline-block w-1.5 h-3 bg-primary/70 animate-pulse align-middle ml-0.5" />
                )}
              </p>
            </div>
          </div>
        ))}
        {logs.length === 0 && state !== 'failed' && (
          <div className="flex items-center gap-2 py-1">
            <Loader2 className="w-3.5 h-3.5 animate-spin text-primary" />
            <p className="text-xs text-muted-foreground">{executor ?? 'Agent'} 正在启动…</p>
          </div>
        )}
        {state === 'running' && streamText !== null && (
          <div className="flex gap-2.5 items-start">
            <span className="w-5 h-5 rounded-full bg-primary/10 text-primary flex items-center justify-center shrink-0 mt-0.5">
              <Bot className="w-3 h-3" />
            </span>
            <div className="min-w-0 flex-1">
              <span className="text-[10px] text-muted-foreground/50 font-mono">实时输出</span>
              <p className="text-xs leading-relaxed whitespace-pre-wrap break-words text-foreground/85 border-l-2 border-primary/40 pl-2.5">
                {streamText}
                {streamTicking && <span className="inline-block w-1.5 h-3 bg-primary/70 animate-pulse align-middle ml-0.5" />}
              </p>
            </div>
          </div>
        )}
        {logs.length === 0 && state === 'failed' && <p className="text-xs text-muted-foreground py-1">该阶段没有留下日志</p>}
        {state === 'failed' && failureMessage && (
          <p className="text-xs text-red-600 dark:text-red-400 leading-relaxed break-all">失败原因：{failureMessage}</p>
        )}
        {state === 'waiting' && (
          <p className="text-xs text-amber-600 dark:text-amber-400 pt-1">请在下方决策卡中作出选择，实施将从中继续。</p>
        )}
        {state === 'running' && runtimeKind === 'zcode' && (
          <button
            type="button"
            onClick={() => void api.runtimes.openApp('zcode-app').catch(() => undefined)}
            className="text-xs text-primary hover:underline inline-flex items-center gap-1"
          >
            打开 ZCode 桌面 App 查看会话 ↗
          </button>
        )}
      </div>
    </div>
  );
}
