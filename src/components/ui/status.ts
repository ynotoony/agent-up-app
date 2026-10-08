import type { Requirement, RequirementMode, RequirementStatus } from '../../../shared/types';

// 状态/模式/阶段配置 —— 全站单一来源（对应 PRD 01 §5、DESIGN 2.2）。
// 双主题可读规则：亮色 -600 + bg-*-600/10；暗色 -400 + bg-*-400/10。

export interface StatusConfig {
  label: string;
  className: string;
  pulse: boolean;
}

export const STATUS_CONFIG: Record<RequirementStatus, StatusConfig> = {
  pending: {
    label: '排队中',
    className: 'text-muted-foreground bg-muted',
    pulse: false,
  },
  initializing: {
    label: '初始化中',
    className: 'text-violet-600 dark:text-violet-400 bg-violet-600/10 dark:bg-violet-400/10',
    pulse: true,
  },
  understanding: {
    label: '理解中',
    className: 'text-blue-600 dark:text-blue-400 bg-blue-600/10 dark:bg-blue-400/10',
    pulse: true,
  },
  questioning: {
    label: '质疑中',
    className: 'text-amber-600 dark:text-amber-400 bg-amber-600/10 dark:bg-amber-400/10',
    pulse: true,
  },
  awaiting_confirmation: {
    label: '待处理',
    className: 'text-amber-600 dark:text-amber-400 bg-amber-600/10 dark:bg-amber-400/10',
    pulse: false,
  },
  planning: {
    label: '方案中',
    className: 'text-primary bg-primary/10',
    pulse: true,
  },
  implementing: {
    label: '实施中',
    className: 'text-blue-600 dark:text-blue-400 bg-blue-600/10 dark:bg-blue-400/10',
    pulse: true,
  },
  verifying: {
    label: '验证中',
    className: 'text-cyan-600 dark:text-cyan-400 bg-cyan-600/10 dark:bg-cyan-400/10',
    pulse: true,
  },
  completed: {
    label: '已完成',
    className: 'text-emerald-600 dark:text-emerald-400 bg-emerald-600/10 dark:bg-emerald-400/10',
    pulse: false,
  },
  failed: {
    label: '失败',
    className: 'text-red-600 dark:text-red-400 bg-red-600/10 dark:bg-red-400/10',
    pulse: false,
  },
  waiting_decision: {
    label: '待决策',
    className: 'text-gray-500 dark:text-gray-400 bg-gray-400/10',
    pulse: false,
  },
};

export const MODE_CONFIG: Record<RequirementMode, { label: string; shortLabel: string; className: string }> = {
  fast: {
    label: '快速模式',
    shortLabel: '快速',
    className: 'text-emerald-600 dark:text-emerald-400 bg-emerald-600/10 dark:bg-emerald-400/10',
  },
  standard: {
    label: '标准模式',
    shortLabel: '标准',
    className: 'text-blue-600 dark:text-blue-400 bg-blue-600/10 dark:bg-blue-400/10',
  },
  high_risk: {
    label: '高风险模式',
    shortLabel: '高风险',
    className: 'text-amber-600 dark:text-amber-400 bg-amber-600/10 dark:bg-amber-400/10',
  },
  emergency: {
    label: '紧急模式',
    shortLabel: '紧急',
    className: 'text-red-600 dark:text-red-400 bg-red-600/10 dark:bg-red-400/10',
  },
};

// 需求卡片阶段标识（D5：阶段/类型标签 rounded，状态/模式徽章 rounded-full）。
export type RequirementStage = 'requirement' | 'plan' | 'implement' | 'done';

export const STAGE_CONFIG: Record<RequirementStage, { label: string; className: string }> = {
  requirement: { label: '需求阶段', className: 'text-muted-foreground bg-muted' },
  plan: { label: '方案阶段', className: 'text-primary bg-primary/10' },
  implement: { label: '实施阶段', className: 'text-blue-600 dark:text-blue-400 bg-blue-600/10 dark:bg-blue-400/10' },
  done: { label: '已完成', className: 'text-emerald-600 dark:text-emerald-400 bg-emerald-600/10 dark:bg-emerald-400/10' },
};

export function stageOf(status: RequirementStatus): RequirementStage {
  switch (status) {
    case 'pending':
    case 'initializing':
    case 'understanding':
    case 'questioning':
    case 'awaiting_confirmation':
      return 'requirement';
    case 'planning':
      return 'plan';
    case 'implementing':
    case 'verifying':
    case 'waiting_decision':
      return 'implement';
    case 'completed':
    case 'failed':
    default:
      return 'done';
  }
}

export const TERMINAL_STATUSES: RequirementStatus[] = ['completed', 'failed'];

export function isTerminal(status: RequirementStatus): boolean {
  return TERMINAL_STATUSES.includes(status);
}

// 统计卡里「进行中」的口径：非终结态全部算。
export function isInFlight(status: RequirementStatus): boolean {
  return !isTerminal(status);
}

export function statusBadgeClass(status: RequirementStatus): string {
  return STATUS_CONFIG[status]?.className ?? STATUS_CONFIG.pending.className;
}
