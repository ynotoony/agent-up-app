import { cn } from '@/lib/utils';
import type { Requirement, Task } from '../../shared/types';

// 需求阶段进度条：吸顶目标区下方的全宽横向 stepper（理解→方案→实施→验证→审查）。
// 状态色遵循 DESIGN 2.2：完成绿 / 进行中蓝脉冲 / 决策等待琥珀 / 失败红 / 未开始灰。

type StageState = 'pending' | 'running' | 'completed' | 'failed' | 'waiting';

const STATE_DOT: Record<StageState, string> = {
  pending: 'bg-border',
  running: 'bg-blue-600 dark:bg-blue-400 animate-pulse',
  completed: 'bg-emerald-600 dark:bg-emerald-400',
  failed: 'bg-red-600 dark:bg-red-400',
  waiting: 'bg-amber-600 dark:bg-amber-400 animate-pulse',
};

const STATE_TEXT: Record<StageState, string> = {
  pending: 'text-muted-foreground/50',
  running: 'text-blue-600 dark:text-blue-400 font-medium',
  completed: 'text-muted-foreground',
  failed: 'text-red-600 dark:text-red-400 font-medium',
  waiting: 'text-amber-600 dark:text-amber-400 font-medium',
};

function stageStateOf(tasks: Task[], step: string): StageState {
  const relevant = tasks.filter((t) => t.step_type === step);
  if (relevant.length === 0) return 'pending';
  const last = relevant[relevant.length - 1];
  if (last.status === 'failed') return 'failed';
  if (last.status === 'running') return 'running';
  if (last.status === 'waiting_decision') return 'waiting';
  // 该阶段历史上有完成记录即视为完成（迭代会重建任务，取最近一次）
  const anyCompleted = relevant.some((t) => t.status === 'completed');
  if (anyCompleted) return 'completed';
  return 'pending';
}

export function StageProgress({
  requirement,
  tasks,
  iterations,
  selected,
  onSelect,
  className,
}: {
  requirement: Requirement;
  tasks: Task[];
  iterations: number;
  /** 当前查看的历史阶段（null = 跟随实时流程）；有任务记录的阶段可点击回看 */
  selected: string | null;
  onSelect: (step: string | null) => void;
  className?: string;
}) {
  const { steps } = require_mode_steps(requirement.mode);
  const stages = steps.filter((s) => s !== 'question');
  const states = stages.map((s) => stageStateOf(tasks, s));

  return (
    <section
      data-tauri-drag-region={false}
      className={cn(
        'shrink-0 bg-card border border-border rounded-xl px-5 py-3',
        className
      )}
    >
      <div className="flex items-center">
        {stages.map((step, i) => {
          const state = states[i];
          const label =
            step === 'understand' ? '理解' : step === 'plan' ? '方案' : step === 'implement' ? '实施' : step === 'verify' ? '验证' : '审查';
          const isLast = i === stages.length - 1;
          const clickable = tasks.some((t) => t.step_type === step);
          const isSelected = selected === step;
          return (
            <div key={step} className="flex items-center min-w-0">
              {/* 连接线：当前阶段之前的线若两端均完成则染绿 */}
              {i > 0 && (
                <span
                  aria-hidden
                  className={cn(
                    'h-[2px] flex-1 min-w-6 rounded transition-colors',
                    states[i - 1] === 'completed' && state !== 'pending' ? 'bg-emerald-600/40 dark:bg-emerald-400/40' : 'bg-border'
                  )}
                />
              )}
              <button
                type="button"
                disabled={!clickable}
                onClick={() => onSelect(isSelected ? null : step)}
                title={clickable ? `查看「${label}」阶段产出` : undefined}
                className={cn(
                  'flex items-center gap-1.5 px-1.5 py-1 rounded-md transition-colors',
                  clickable ? 'hover:bg-accent/60 cursor-pointer' : 'cursor-default',
                  isSelected && 'bg-primary/10 ring-1 ring-primary/30'
                )}
              >
                <span className={cn('w-2 h-2 rounded-full shrink-0', STATE_DOT[state])} />
                <span className={cn('text-xs whitespace-nowrap', isSelected ? 'text-primary font-medium' : STATE_TEXT[state])}>
                  {label}
                  {state === 'waiting' && <span className="ml-0.5 text-[10px]">·待决策</span>}
                </span>
              </button>
              {isLast && (
                <span className="ml-auto pl-4 text-[11px] text-muted-foreground/60 whitespace-nowrap">
                  v{requirement.current_version}
                  {iterations > 0 && ` · 迭代 ${iterations} 次`}
                </span>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}

// getModeSteps 的浅包装（避免循环依赖命名冲突）
function require_mode_steps(mode: Requirement['mode']): { steps: string[] } {
  // 与 task_engine 保持一致的模式步骤表（前端静态副本）
  const map: Record<Requirement['mode'], string[]> = {
    fast: ['understand', 'implement', 'verify'],
    standard: ['understand', 'question', 'plan', 'implement', 'verify'],
    high_risk: ['understand', 'question', 'plan', 'implement', 'verify', 'review'],
    emergency: ['understand', 'implement', 'verify'],
  };
  return { steps: map[mode] ?? map.standard };
}
