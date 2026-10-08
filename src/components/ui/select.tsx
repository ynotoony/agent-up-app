import { cn } from '@/lib/utils';
import type { SelectHTMLAttributes } from 'react';

// 原生 select + 语义样式（DESIGN 3.3 输入配方）。
export function Select({ className, children, ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <select
      className={cn(
        'w-full appearance-none bg-background border border-border rounded-lg pl-3 pr-8 py-2 text-sm text-foreground',
        'placeholder:text-muted-foreground/50 focus:outline-none focus:border-primary/50 focus:ring-1 focus:ring-primary/20 transition-all',
        'disabled:opacity-50',
        className
      )}
      {...props}
    >
      {children}
    </select>
  );
}

export function SelectChevron({ className }: { className?: string }) {
  return (
    <svg
      className={cn('pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground', className)}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d="M4 6l4 4 4-4" />
    </svg>
  );
}
