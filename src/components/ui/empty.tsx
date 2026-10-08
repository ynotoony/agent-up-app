import { cn } from '@/lib/utils';
import type { ReactNode } from 'react';

// 空态：居中 py-8 + 弱化图标 + 主文/副文（DESIGN 3.1）。
export function EmptyState({
  icon,
  title,
  description,
  className,
  children,
}: {
  icon?: ReactNode;
  title: string;
  description?: string;
  className?: string;
  children?: ReactNode;
}) {
  return (
    <div className={cn('flex flex-col items-center justify-center text-center py-8', className)}>
      {icon && <div className="w-10 h-10 mb-2 text-muted-foreground/20">{icon}</div>}
      <p className="text-sm text-muted-foreground/50">{title}</p>
      {description && <p className="mt-1 text-xs text-muted-foreground/30">{description}</p>}
      {children}
    </div>
  );
}
