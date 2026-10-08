import { cn } from '@/lib/utils';
import type { ButtonHTMLAttributes, ReactNode } from 'react';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost' | 'destructive';
  size?: 'sm' | 'md';
  loading?: boolean;
  children?: ReactNode;
}

// 主 CTA：bg-primary，禁用态用透明度（DESIGN 3.2，禁 disabled:text-muted-foreground）。
export function Button({ variant = 'primary', size = 'md', loading, className, disabled, children, ...props }: ButtonProps) {
  const base =
    'inline-flex items-center justify-center gap-2 rounded-lg text-sm font-medium transition-all outline-none focus-visible:ring-2 focus-visible:ring-primary/30 disabled:pointer-events-none';
  const variants = {
    primary: 'bg-primary text-primary-foreground hover:bg-primary/90 disabled:bg-primary/30 disabled:text-primary/30',
    secondary: 'bg-muted text-foreground border border-border hover:bg-accent disabled:opacity-50',
    ghost: 'text-foreground hover:bg-accent disabled:opacity-50',
    destructive: 'bg-destructive text-white hover:bg-destructive/90 disabled:opacity-50',
  };
  const sizes = {
    sm: 'h-8 px-3 text-xs',
    md: 'h-9 px-4',
  };
  return (
    <button className={cn(base, variants[variant], sizes[size], className)} disabled={disabled || loading} {...props}>
      {loading && <Spinner inButton />}
      {children}
    </button>
  );
}

export function Spinner({ inButton, className }: { inButton?: boolean; className?: string }) {
  return (
    <span
      aria-hidden
      className={cn(
        'inline-block w-3.5 h-3.5 rounded-full border-2 animate-spin',
        inButton ? 'border-primary-foreground/30 border-t-primary-foreground' : 'border-border border-t-primary',
        className
      )}
    />
  );
}
