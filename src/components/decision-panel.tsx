import { Gavel, Loader2 } from 'lucide-react';
import { useState } from 'react';
import type { Decision } from '../../shared/types';
import { api, errorMessage } from '@/lib/api';
import { cn } from '@/lib/utils';
import { PanelHeader } from './understanding-panel';
import { Button } from './ui/button';
import { toast } from 'sonner';

// 决策交互面板：待决策点呈现 + 选项选择/解决（推荐项高亮）。
export function DecisionPanel({ decisions, onResolved }: { decisions: Decision[]; onResolved: () => void }) {
  const [resolvingId, setResolvingId] = useState<string | null>(null);
  const [chosen, setChosen] = useState<Record<string, string>>({});

  const resolve = async (decision: Decision) => {
    const value = chosen[decision.id] ?? decision.recommended ?? decision.options[0]?.value;
    const option = decision.options.find((o) => o.value === value);
    if (!option) return;
    setResolvingId(decision.id);
    try {
      await api.decisions.resolve(decision.id, option);
      toast.success('决策已提交，继续执行');
      onResolved();
    } catch (err) {
      toast.error(errorMessage(err));
    } finally {
      setResolvingId(null);
    }
  };

  return (
    <div className="bg-card border border-amber-600/30 dark:border-amber-400/20 rounded-xl overflow-hidden">
      <PanelHeader
        title={`待决策（${decisions.length}）`}
        icon={<Gavel className="w-3.5 h-3.5" />}
        right={<span className="text-[11px] text-amber-600 dark:text-amber-400">实施已挂起，等待你的定夺</span>}
      />
      <div className="p-5 space-y-5">
        {decisions.map((decision) => {
          const selected = chosen[decision.id] ?? decision.recommended;
          return (
            <div key={decision.id}>
              <p className="text-sm font-medium text-foreground">{decision.question}</p>
              {decision.context && <p className="mt-1 text-xs text-muted-foreground leading-relaxed">{decision.context}</p>}
              <div className="mt-2.5 space-y-1.5">
                {decision.options.map((option) => {
                  const active = selected === option.value;
                  const recommended = decision.recommended === option.value;
                  return (
                    <button
                      key={option.value}
                      type="button"
                      disabled={resolvingId !== null}
                      onClick={() => setChosen((prev) => ({ ...prev, [decision.id]: option.value }))}
                      className={cn(
                        'w-full text-left rounded-lg border px-3 py-2 transition-colors disabled:opacity-60',
                        active
                          ? 'border-primary/40 bg-primary/10'
                          : 'border-border bg-muted hover:bg-accent'
                      )}
                    >
                      <span className="flex items-center gap-2 text-sm text-foreground">
                        {option.label}
                        {recommended && (
                          <span className="px-1.5 py-0.5 rounded text-[11px] bg-primary/10 text-primary">推荐</span>
                        )}
                      </span>
                      {option.description && <span className="block mt-0.5 text-xs text-muted-foreground">{option.description}</span>}
                    </button>
                  );
                })}
              </div>
              <Button
                size="sm"
                className="mt-3"
                loading={resolvingId === decision.id}
                disabled={resolvingId !== null}
                onClick={() => void resolve(decision)}
              >
                {resolvingId === decision.id ? '提交中…' : '确认并继续'}
              </Button>
            </div>
          );
        })}
      </div>
    </div>
  );
}

export function InlineDecisionBusy() {
  return <Loader2 className="w-3.5 h-3.5 animate-spin text-muted-foreground" />;
}
