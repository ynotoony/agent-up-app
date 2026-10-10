// Input: 原生目标数组与查询字符串。
// Output: 过滤后的目标数组，保持顺序和引用，不修改输入。
// Pos: 原生目标搜索纯函数；src/ 目录登记豁免。

import type { NativeGoal } from '../../shared/types';

/**
 * 本地关键词搜索：匹配目标的 content 或 summary，忽略大小写。
 *
 * @param goals - 原生目标数组
 * @param query - 搜索查询字符串（自动去除首尾空格）
 * @returns 匹配的目标数组，保持原顺序和对象引用
 *
 * @example
 * // 空查询返回全部
 * filterNativeGoals(goals, ''); // => goals
 *
 * // 忽略大小写匹配
 * filterNativeGoals(goals, 'TEST'); // 匹配 content/summary 包含 "test" 的目标
 *
 * // 中文搜索
 * filterNativeGoals(goals, '搜索'); // 匹配 content/summary 包含 "搜索" 的目标
 */
export function filterNativeGoals(goals: NativeGoal[], query: string): NativeGoal[] {
  const trimmed = query.trim();

  // 空查询或空白查询返回全部
  if (trimmed === '') {
    return goals;
  }

  const lowerQuery = trimmed.toLowerCase();

  return goals.filter((goal) => {
    const content = (goal.content || '').toLowerCase();
    const summary = (goal.summary || '').toLowerCase();
    return content.includes(lowerQuery) || summary.includes(lowerQuery);
  });
}
