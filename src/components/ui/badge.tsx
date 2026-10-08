import { cn } from '@/lib/utils';
import type { ReactNode } from 'react';

interface BadgeProps {
  className?: string;
  pill?: boolean;
  dot?: boolean;
  children: ReactNode;
}

// 徽章 = tint 底 + 同色文字；pill=true → rounded-full（状态/模式），否则 rounded（阶段/类型）。
export function Badge({ className, pill = true, dot, children }: BadgeProps) {
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 text-xs px-2 py-0.5 leading-4 whitespace-nowrap',
        pill ? 'rounded-full' : 'rounded',
        className
      )}
    >
      {dot && <span className="w-1.5 h-1.5 rounded-full bg-current animate-pulse" />}
      {children}
    </span>
  );
}
