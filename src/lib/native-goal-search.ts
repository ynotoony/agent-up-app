import type { NativeGoal } from '../../shared/types';

export function filterNativeGoals(goals: readonly NativeGoal[], query: string): NativeGoal[] {
  const keyword = query.trim().toLowerCase();
  return goals.filter((goal) => !keyword
    || goal.content.toLowerCase().includes(keyword)
    || goal.summary.toLowerCase().includes(keyword));
}
