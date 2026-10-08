import { useEffect, useRef, useState } from 'react';
import { ChevronDown, CircleCheck, CircleHelp, Flag, History, ListChecks, Quote, Sparkles } from 'lucide-react';
import type { Requirement, RequirementVersion, UnderstandingQuestion } from '../../shared/types';
import { cn } from '@/lib/utils';
import { Badge } from './ui/badge';
import { Button } from './ui/button';
import { EmptyState } from './ui/empty';
import { Spinner } from './ui/button';

// 需求理解面板（对应 PRD 05 §8 understanding-panel 职责）：
// 目标复述 / 成功标准 ✓ / 风险圆点 / 歧义 ？ / 理解版本（卡片内悬浮切换）/ 选区引用 / 确认按钮。
// 质疑作答（QuestionCards）由页面渲染在本面板下方，不再内嵌于此。

const SOURCE_LABELS: Record<RequirementVersion['source'], string> = {
  initial: '初始理解',
  user_feedback: '依据反馈更新',
  question_answer: '回答质疑后更新',
  iteration: '反馈迭代',
  manual: '手动重新理解',
};

export interface QuestionDraft {
  option: string | null;
  text: string;
}

export function UnderstandingPanel({
  requirement,
  versions,
  viewingVersion,
  confirmed,
  busy,
  onViewVersion,
  onConfirm,
  onQuoteSelected,
}: {
  requirement: Requirement;
  versions: RequirementVersion[];
  viewingVersion: number;
  confirmed: boolean;
  busy: boolean;
  onViewVersion: (version: number) => void;
  onConfirm: () => Promise<void>;
  onQuoteSelected: (quote: string) => void;
}) {
  const snap = versions.find((v) => v.version === viewingVersion) ?? versions[versions.length - 1];
  const isCurrent = viewingVersion === requirement.current_version;
  const questions = snap?.questions ?? [];

  const [versionOpen, setVersionOpen] = useState(false);
  const cardRef = useRef<HTMLDivElement>(null);

  // 悬浮版本面板：外点 / Esc 关闭
  useEffect(() => {
    if (!versionOpen) return;
    const onPointerDown = (e: MouseEvent) => {
      if (cardRef.current && !cardRef.current.contains(e.target as Node)) setVersionOpen(false);
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setVersionOpen(false);
    };
    window.addEventListener('mousedown', onPointerDown);
    window.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('mousedown', onPointerDown);
      window.removeEventListener('keydown', onKeyDown);
    };
  }, [versionOpen]);

  const contentRef = useRef<HTMLDivElement>(null);

  const handleMouseUp = () => {
    const selection = window.getSelection();
    const text = selection?.toString().trim() ?? '';
    if (text.length >= 4 && contentRef.current?.contains(selection?.anchorNode ?? null)) {
      onQuoteSelected(text);
    }
  };

  if (!snap) {
    // 理解快照尚未生成：运行态由页面对话流（StageConversation）承担，此处仅在回看空版本时出现
    return (
      <div className="bg-card border border-border rounded-xl px-4 py-8 text-center">
        <p className="text-sm text-muted-foreground">该版本没有理解内容</p>
      </div>
    );
  }

  const versionTrigger =
    versions.length > 1 ? (
      <button
        type="button"
        onClick={() => setVersionOpen((open) => !open)}
        title="切换理解版本"
        className={cn(
          'inline-flex items-center gap-1 h-6 px-2 rounded-md text-[11px] font-medium border transition-colors',
          versionOpen || !isCurrent
            ? 'bg-accent border-border text-foreground'
            : 'bg-background border-border text-muted-foreground hover:text-foreground hover:bg-accent'
        )}
      >
        <History className="w-3 h-3" />
        v{snap.version}
        <ChevronDown className={cn('w-3 h-3 transition-transform', versionOpen && 'rotate-180')} />
      </button>
    ) : null;

  return (
    <div ref={cardRef} className="relative">
      <div
        className={cn(
          'bg-card border rounded-xl overflow-hidden',
          isCurrent ? 'border-border' : 'border-amber-600/30 dark:border-amber-400/20'
        )}
      >
        {isCurrent ? (
          <PanelHeader
            title="需求理解"
            icon={<Sparkles className="w-3.5 h-3.5" />}
            right={
              versionTrigger && (
                <div className="flex items-center gap-2">
                  {versionTrigger}
                  {busy && <Spinner />}
                </div>
              )
            }
          />
        ) : (
          <div className="flex items-center justify-between px-5 py-2.5 bg-amber-600/10 dark:bg-amber-400/[0.06] border-b border-amber-600/20 dark:border-amber-400/20">
            <p className="flex items-center gap-1.5 text-xs text-amber-600 dark:text-amber-400">
              <History className="w-3.5 h-3.5" />
              正在查看历史版本 v{snap.version}（只读）
            </p>
            <div className="flex items-center gap-2">
              {versionTrigger}
              <button
                type="button"
                onClick={() => onViewVersion(requirement.current_version)}
                className="text-xs text-primary hover:text-primary/80 transition-colors"
              >
                回到当前版本
              </button>
            </div>
          </div>
        )}

        <div ref={contentRef} onMouseUp={handleMouseUp} className="p-5 space-y-5 select-text">
          <section>
            <h3 className="text-xs font-medium text-muted-foreground flex items-center gap-1.5">
              <Flag className="w-3.5 h-3.5" />
              目标复述
              <span className="inline-flex items-center gap-0.5 text-muted-foreground/40 cursor-pointer hover:text-primary transition-colors" title="选中文字可引用到反馈区">
                <Quote className="w-3 h-3" />
              </span>
            </h3>
            <p className="mt-1.5 text-sm leading-relaxed text-foreground">{snap.goal_summary || requirement.content}</p>
          </section>

          {(snap.success_criteria?.length ?? 0) > 0 && (
            <section>
              <h3 className="text-xs font-medium text-muted-foreground flex items-center gap-1.5">
                <CircleCheck className="w-3.5 h-3.5" />
                成功标准
              </h3>
              <ul className="mt-2 space-y-1.5">
                {snap.success_criteria!.map((criteria) => (
                  <li key={criteria.id} className="flex items-start gap-2 text-sm text-foreground/90">
                    <CircleCheck className="w-3.5 h-3.5 mt-0.5 shrink-0 text-emerald-600 dark:text-emerald-400" />
                    <span className="leading-relaxed">{criteria.criteria}</span>
                  </li>
                ))}
              </ul>
            </section>
          )}

          {(snap.risks?.length ?? 0) > 0 && (
            <section>
              <h3 className="text-xs font-medium text-muted-foreground flex items-center gap-1.5">
                <ListChecks className="w-3.5 h-3.5" />
                风险
              </h3>
              <ul className="mt-2 space-y-1.5">
                {snap.risks!.map((risk) => (
                  <li key={risk.id} className="flex items-start gap-2 text-sm text-foreground/90">
                    <span
                      className={cn(
                        'w-1.5 h-1.5 rounded-full mt-[7px] shrink-0',
                        risk.level === 'high' && 'bg-red-600 dark:bg-red-400',
                        risk.level === 'medium' && 'bg-amber-600 dark:bg-amber-400',
                        risk.level === 'low' && 'bg-gray-400'
                      )}
                    />
                    <span className="leading-relaxed">
                      {risk.risk}
                      <span className="ml-1.5 text-[11px] text-muted-foreground/60">
                        {risk.level === 'high' ? '高' : risk.level === 'medium' ? '中' : '低'}风险
                      </span>
                    </span>
                  </li>
                ))}
              </ul>
            </section>
          )}

          {(snap.ambiguities?.length ?? 0) > 0 && (
            <section>
              <h3 className="text-xs font-medium text-muted-foreground flex items-center gap-1.5">
                <CircleHelp className="w-3.5 h-3.5" />
                歧义与默认解释
              </h3>
              <ul className="mt-2 space-y-1.5">
                {snap.ambiguities!.map((ambiguity, i) => (
                  <li key={i} className="text-sm text-foreground/90 leading-relaxed">
                    <span className="text-muted-foreground">{ambiguity.item}</span>
                    <span className="mx-1.5 text-muted-foreground/40">→</span>
                    {ambiguity.clarification}
                  </li>
                ))}
              </ul>
            </section>
          )}
        </div>

        {!confirmed && isCurrent && (
          <div className="px-5 py-3.5 border-t border-border bg-muted/40 flex items-center justify-between gap-3">
            <p className="text-xs text-muted-foreground">
              {questions.length > 0
                ? `有 ${questions.length} 个质疑等待你的回答（见下方），也可直接确认`
                : '确认理解无误后，进入方案制定与实施'}
            </p>
            <Button onClick={() => void onConfirm()} loading={busy} size="sm" disabled={busy}>
              确认需求
            </Button>
          </div>
        )}
      </div>

      {/* 理解版本：悬浮在理解卡片内，不随卡片滚动裁切 */}
      {versionOpen && versions.length > 1 && (
        <div className="absolute right-4 top-11 z-30 w-80 rounded-xl border border-border bg-popover shadow-lg overflow-hidden animate-slide-down">
          <div className="flex items-center gap-1.5 px-4 py-2 border-b border-border bg-muted/40 text-xs font-medium text-muted-foreground">
            <History className="w-3.5 h-3.5" />
            理解版本
          </div>
          <div className="p-1.5 max-h-72 overflow-y-auto scrollbar-thin">
            {[...versions].reverse().map((version) => {
              const active = version.version === viewingVersion;
              return (
                <button
                  key={version.id}
                  type="button"
                  onClick={() => {
                    onViewVersion(version.version);
                    setVersionOpen(false);
                  }}
                  className={cn(
                    'w-full text-left px-3 py-2 rounded-lg transition-colors',
                    active ? 'bg-primary/10' : 'hover:bg-accent'
                  )}
                >
                  <span className="flex items-center gap-2">
                    <span className={cn('text-sm font-semibold', active ? 'text-primary' : 'text-foreground/80')}>
                      v{version.version}
                    </span>
                    <span className="text-xs text-muted-foreground">{SOURCE_LABELS[version.source]}</span>
                    {version.version === requirement.current_version && (
                      <Badge className="ml-auto bg-primary/10 text-primary">当前</Badge>
                    )}
                  </span>
                  {version.user_input && (
                    <span className="block mt-0.5 text-xs text-muted-foreground/60 line-clamp-1">{version.user_input}</span>
                  )}
                </button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}

// 质疑作答（DESIGN 3.9）：选项独立点选徽章，与手输互斥；逐题挑完统一提交。
export function QuestionCards({
  questions,
  drafts,
  onChange,
  onSubmit,
  submitting,
  answeredCount,
}: {
  questions: UnderstandingQuestion[];
  drafts: Record<string, QuestionDraft>;
  onChange: (questionId: string, draft: QuestionDraft) => void;
  onSubmit: () => void;
  submitting: boolean;
  answeredCount: number;
}) {
  return (
    <div className="space-y-3">
      {questions.map((question, index) => {
        const draft = drafts[question.id] ?? { option: null, text: '' };
        return (
          <div key={question.id} className="bg-card border border-amber-600/30 dark:border-amber-400/20 rounded-xl p-4">
            <p className="text-sm text-foreground">
              <span className="inline-flex items-center justify-center w-5 h-5 mr-1.5 rounded-full bg-amber-600/10 dark:bg-amber-400/10 text-amber-600 dark:text-amber-400 text-[11px] font-medium align-middle">
                {index + 1}
              </span>
              {question.question}
            </p>
            {question.reason && <p className="mt-1 text-xs text-muted-foreground/70">{question.reason}</p>}
            {question.options.length > 0 && (
              <div className="mt-2.5 flex flex-wrap gap-1.5">
                {question.options.map((option) => {
                  const selected = draft.option === option;
                  return (
                    <button
                      key={option}
                      type="button"
                      onClick={() => {
                        // 点选选项即清空该题手输（互斥，非快捷填充）
                        onChange(question.id, selected ? { option: null, text: '' } : { option, text: '' });
                      }}
                      className={cn(
                        'px-2.5 py-1 rounded-full text-xs border transition-all',
                        selected
                          ? 'bg-primary/10 text-primary border-primary/40'
                          : 'bg-muted border-border text-foreground/80 hover:bg-accent hover:border-primary/20'
                      )}
                    >
                      {option}
                    </button>
                  );
                })}
              </div>
            )}
            <input
              value={draft.text}
              onChange={(e) => onChange(question.id, { option: null, text: e.target.value })}
              placeholder="输入你的回答…"
              className="mt-2.5 w-full bg-background border border-border rounded-lg px-3 py-1.5 text-xs placeholder:text-muted-foreground/50 focus:outline-none focus:border-amber-500/50 focus:ring-1 focus:ring-amber-500/20 transition-all"
            />
          </div>
        );
      })}
      <Button className="w-full" onClick={onSubmit} loading={submitting} disabled={submitting || answeredCount === 0}>
        {submitting ? '提交中…' : `提交回答（${answeredCount}）`}
      </Button>
    </div>
  );
}

export function buildAnswerFeedback(questions: UnderstandingQuestion[], drafts: Record<string, QuestionDraft>): string {
  return questions
    .map((question, i) => {
      const draft = drafts[question.id];
      if (!draft) return null;
      const answer = draft.option ?? draft.text.trim();
      if (!answer) return null;
      return `${i + 1}. ${question.question}\n答：${answer}`;
    })
    .filter(Boolean)
    .join('\n');
}

export function PanelHeader({ title, icon, right }: { title: string; icon?: React.ReactNode; right?: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between px-5 py-2.5 bg-muted/40 border-b border-border">
      <span className="inline-flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
        {icon}
        {title}
      </span>
      {right}
    </div>
  );
}
