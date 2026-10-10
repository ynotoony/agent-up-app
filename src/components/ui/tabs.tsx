import { cn } from '@/lib/utils';
import type { ReactNode } from 'react';

interface TabOption<T extends string> {
  value: T;
  label: ReactNode;
}

// Tab 选中态统一 bg-primary/10 text-primary（DESIGN 3.2）。
export function TabPills<T extends string>({
  options,
  value,
  onChange,
  className,
}: {
  options: TabOption<T>[];
  value: T;
  onChange: (value: T) => void;
  className?: string;
}) {
  return (
    <div className={cn('inline-flex items-center gap-1 p-1 rounded-lg bg-muted/60', className)}>
      {options.map((opt) => (
        <button
          key={opt.value}
          type="button"
          onClick={() => onChange(opt.value)}
          className={cn(
            'px-3 h-7 rounded-md text-xs font-medium transition-colors outline-none focus-visible:ring-2 focus-visible:ring-primary/30',
            value === opt.value ? 'bg-primary/10 text-primary' : 'text-muted-foreground hover:text-foreground hover:bg-accent'
          )}
        >
          {opt.label}
        </button>
      ))}
    </div>
  );
}
