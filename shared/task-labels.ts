import type { RequirementMode, StepType } from './types';

// 步骤/模式/状态中文标签 —— 主进程与渲染层共用（PRD《04》§1.3）。
export const STEP_LABELS: Record<StepType, string> = {
  understand: '理解需求',
  question: '质疑与澄清',
  plan: '制定方案',
  implement: '实施',
  verify: '验证',
  review: '审查',
  iterate: '迭代',
};

export const MODE_LABELS: Record<RequirementMode, string> = {
  fast: '快速模式',
  standard: '标准模式',
  high_risk: '高风险模式',
  emergency: '紧急模式',
};

export const STATUS_LABELS: Record<string, string> = {
  pending: '排队中',
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
