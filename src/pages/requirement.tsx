import { useNavigate, useParams } from 'react-router-dom';
import { useCallback, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { ArrowLeft, FolderClosed, Image as ImageIcon, File as FileIcon, RotateCcw, Target, X, CheckCircle2 } from 'lucide-react';
import type { ExecutionLog as ExecutionLogItem, PendingAttachment, RequirementDetail, RequirementVersion } from '../../shared/types';
import { api, errorMessage } from '@/lib/api';
import { attachmentUrl, formatTokens } from '@/lib/utils';
import { useRequirementStream } from '@/lib/stream';
import { UnderstandingPanel, QuestionCards, buildAnswerFeedback, type QuestionDraft } from '@/components/understanding-panel';
import { PlanPanel } from '@/components/plan-panel';
import { DecisionPanel } from '@/components/decision-panel';
import { ArtifactList } from '@/components/artifact-list';
import { ExecutionLog } from '@/components/execution-log';
import { StageConversation } from '@/components/stage-conversation';
import { FeedbackComments, type FeedbackHistoryItem } from '@/components/feedback-input';
import { StageProgress } from '@/components/stage-progress';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { TabPills } from '@/components/ui/tabs';
import { MODE_CONFIG, STATUS_CONFIG, TERMINAL_STATUSES, isTerminal } from '@/components/ui/status';
import { toast } from 'sonner';

const POLL_INTERVAL = 5000;

const STAGE_LABELS: Record<string, string> = {
  understand: '理解',
  iterate: '迭代',
  question: '质疑',
  plan: '方案',
  implement: '实施',
  verify: '验证',
  review: '审查',
};

// 需求详情页：桌面锁定视口、两栏独立滚动；5s 轮询直至终结态。
export default function RequirementPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [detail, setDetail] = useState<RequirementDetail | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [viewingVersion, setViewingVersion] = useState<number | null>(null);
  // 步进条点击回看：null 跟随实时流程；非 null 强制查看该阶段的历史产出
  const [viewStage, setViewStage] = useState<string | null>(null);
  const [bottomTab, setBottomTab] = useState<'artifacts' | 'logs'>('artifacts');
  const [quote, setQuote] = useState<string | null>(null);
  const [questionDrafts, setQuestionDrafts] = useState<Record<string, QuestionDraft>>({});
  const [lightbox, setLightbox] = useState<string | null>(null);
  const quoteScrollRef = useRef<HTMLDivElement>(null);

  // 真流式：订阅 requirement://{id}/stream（token 级 delta / 消息级 message）
  const stream = useRequirementStream(id);
  const [cancelling, setCancelling] = useState(false);

  const handleCancel = async () => {
    if (!requirement) return;
    setCancelling(true);
    try {
      await api.requirements.cancel(requirement.id);
      toast.success('已请求取消，正在终止当前运行');
    } catch (err) {
      toast.error(errorMessage(err));
    } finally {
      setCancelling(false);
      void fetchData();
    }
  };

  const fetchData = useCallback(async () => {
    if (!id) return;
    try {
      const data = await api.requirements.get(id);
      setDetail(data);
    } catch (err) {
      setError(errorMessage(err));
    }
  }, [id]);

  useLayoutEffect(() => {
    void fetchData();
  }, [fetchData]);

  // 5s 轮询：仅在非终结态；每次交互成功后主动刷新。
  useLayoutEffect(() => {
    if (!detail) return;
    if (isTerminal(detail.requirement.status)) return;
    const timer = window.setInterval(() => void fetchData(), POLL_INTERVAL);
    return () => window.clearInterval(timer);
  }, [detail?.requirement.status, fetchData]);

  const requirement = detail?.requirement;
  // 终态（completed/failed）一律视为已确认：自动归档的需求 confirmed_at 为空，但同样不该出现「确认需求」
  const confirmed = Boolean(requirement?.confirmed_at) || (requirement ? isTerminal(requirement.status) : false);
  const inUnderstandingPhase = Boolean(
    requirement &&
      !requirement.confirmed_at &&
      ['pending', 'understanding', 'questioning', 'awaiting_confirmation'].includes(requirement.status)
  );

  const versions: RequirementVersion[] = detail?.versions ?? [];
  const effectiveViewingVersion = viewingVersion ?? requirement?.current_version ?? 1;
  const currentSnap = versions.find((v) => v.version === (requirement?.current_version ?? 1));
  const currentQuestions = currentSnap?.questions ?? [];

  const iterations = useMemo(
    () => (detail?.tasks ?? []).filter((t) => t.step_type === 'iterate' && t.status === 'completed').length,
    [detail?.tasks]
  );

  const planTask = useMemo(() => {
    const plans = (detail?.tasks ?? []).filter((t) => t.step_type === 'plan');
    return plans[plans.length - 1] ?? null;
  }, [detail?.tasks]);

  // 统一工作区的核心：当前正在跑的任务（质疑等待不算——那是等用户输入，不是 Agent 在干活）
  const runningStageTask = useMemo(
    () => (detail?.tasks ?? []).find((t) => t.status === 'running' && t.step_type !== 'question'),
    [detail?.tasks]
  );
  const waitingTask = useMemo(
    () => (detail?.tasks ?? []).find((t) => t.status === 'waiting_decision'),
    [detail?.tasks]
  );
  // 失败态：最近一个失败任务，对话流冻结为真实耗时并展示失败原因
  const failedStageTask = useMemo(
    () => [...(detail?.tasks ?? [])].reverse().find((t) => t.status === 'failed' && t.step_type !== 'question'),
    [detail?.tasks]
  );

  const pendingDecisions = useMemo(
    () => (detail?.decisions ?? []).filter((d) => d.status === 'pending'),
    [detail?.decisions]
  );

  const feedbackHistory: FeedbackHistoryItem[] = useMemo(
    () =>
      versions
        .filter((v) => v.user_input)
        .map((v) => ({ id: v.id, text: v.user_input!, source: v.source, createdAt: v.created_at }))
        .sort((a, b) => b.createdAt.localeCompare(a.createdAt)),
    [versions]
  );

  const withBusy = async (fn: () => Promise<unknown>, successMessage?: string) => {
    setBusy(true);
    try {
      await fn();
      if (successMessage) toast.success(successMessage);
      await fetchData();
    } catch (err) {
      toast.error(errorMessage(err));
      // 409 幂等拒绝后也刷新一次，让本地状态与主进程对齐
      await fetchData().catch(() => undefined);
    } finally {
      setBusy(false);
    }
  };

  const handleConfirm = () => withBusy(() => api.requirements.confirm(requirement!.id), '已确认，进入方案制定');

  const handleRetry = () =>
    withBusy(() => api.requirements.retry(requirement!.id), '已发起重试，从失败阶段恢复执行');

  const handleAnswers = (answersText: string) =>
    withBusy(
      () => api.requirements.reunderstand(requirement!.id, { feedback: answersText, answering: true }),
      '回答已提交，重新理解中'
    ).then(() => {
      setQuestionDrafts({});
    });

  const handleReunderstand = (feedback: string, attachments: PendingAttachment[]) =>
    withBusy(() => api.requirements.reunderstand(requirement!.id, { feedback, quote: quote ?? undefined, attachments })).then(() => {
      setQuote(null);
    });

  const handleIterate = (feedback: string, attachments: PendingAttachment[]) =>
    withBusy(() => api.requirements.iterate(requirement!.id, { feedback, attachments }));

  const handleResolve = () => withBusy(() => Promise.resolve());

  const handleQuoteSelected = useCallback((text: string) => {
    setQuote(text);
    quoteScrollRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }, []);

  if (error) {
    return (
      <main className="h-screen overflow-y-auto scrollbar-thin">
        <div className="max-w-3xl mx-auto px-6 py-16 text-center">
          <p className="text-sm text-destructive">{error}</p>
          <Button variant="secondary" size="sm" className="mt-4" onClick={() => navigate(-1)}>
            返回
          </Button>
        </div>
      </main>
    );
  }

  if (!detail || !requirement) {
    return (
      <main className="h-screen overflow-y-auto scrollbar-thin">
        <div className="max-w-3xl mx-auto px-6 py-16 space-y-4 animate-pulse">
          <div className="h-8 w-64 rounded bg-muted" />
          <div className="h-24 rounded-xl bg-card border border-border" />
          <div className="h-64 rounded-xl bg-card border border-border" />
        </div>
      </main>
    );
  }

  const status = STATUS_CONFIG[requirement.status];
  const mode = MODE_CONFIG[requirement.mode];
  const viewingSnap = versions.find((v) => v.version === effectiveViewingVersion);
  // 目标题只认理解产物 goal_summary；理解前不裸吐文档原文（注释/标记等内脏），给友好占位。
  // 来源走结构化字段 source_doc_path，不做内容解析。
  const sourceDocPath = requirement.source_doc_path ?? null;
  const goalText =
    viewingSnap?.goal_summary ||
    requirement.goal_summary ||
    (sourceDocPath ? null : requirement.content_preview) ||
    '理解完成后，这里会显示本次需求的目标总结';
  const originalText = requirement.content ?? requirement.content_preview;
  const showOriginal = goalText !== originalText && !!goalText;
  const attachments = requirement.attachments ?? [];

  // 某阶段的日志（理解对话流并收 iterate 日志：迭代也是理解在跑）
  const stageLogs = (stage: string): ExecutionLogItem[] =>
    (detail.logs ?? []).filter((l) => l.step === stage || (stage === 'understand' && l.step === 'iterate'));

  // 工作区渲染（统一交互）：Agent 在跑 → 对话流；等待决策 → 对话流 + 决策卡；
  // 失败 → 冻结的失败对话流；其余按回看/终态展示阶段产出。
  const conversationFor = (step: string, state: 'running' | 'waiting' | 'failed' = 'running') => {
    const task = state === 'failed' ? failedStageTask : state === 'waiting' ? waitingTask : runningStageTask;
    const streaming = state === 'running' && stream.active && stream.stage === step;
    return (
      <StageConversation
        key={`${step}-${state}`}
        stageLabel={STAGE_LABELS[step] ?? step}
        logs={stageLogs(step)}
        executor={executorOf(stageLogs(step))}
        runtimeKind={runtimeKindOf(stageLogs(step))}
        startedAt={task?.started_at ?? null}
        completedAt={state === 'failed' ? (task?.completed_at ?? null) : null}
        failureMessage={state === 'failed' ? (task?.error_message ?? null) : null}
        state={state}
        streamText={streaming ? stream.streamText || stream.lastMessage : null}
        streamTicking={stream.ticking}
        onCancel={state === 'running' ? () => void handleCancel() : null}
        cancelling={cancelling}
      />
    );
  };

  let workspace: React.ReactNode;
  if (viewStage && runningStageTask?.step_type === viewStage) {
    // 步进条点到正在跑的阶段：同样是对话流（统一交互）
    workspace = conversationFor(viewStage);
  } else if (viewStage === null && runningStageTask) {
    workspace = conversationFor(runningStageTask.step_type);
  } else if (viewStage === null && waitingTask) {
    workspace = conversationFor(waitingTask.step_type, 'waiting');
  } else if (viewStage === null && requirement.status === 'failed' && failedStageTask) {
    workspace = conversationFor(failedStageTask.step_type, 'failed');
  } else if (viewStage === 'implement' || viewStage === 'verify' || viewStage === 'review') {
    workspace = <StageArtifactPanel artifacts={detail.artifacts} stage={viewStage} />;
  } else if (viewStage === 'plan' || (viewStage === null && confirmed && planTask)) {
    workspace = <PlanPanel planTask={planTask} planning={requirement.status === 'planning'} onRetry={handleRetry} retrying={busy} />;
  } else {
    workspace = (
      <UnderstandingPanel
        requirement={requirement}
        versions={versions}
        viewingVersion={effectiveViewingVersion}
        // 历史查看（步进条回看）与终态（completed/failed）均为只读：不出现确认/回答等操作控件
        confirmed={viewStage !== null || isTerminal(requirement.status)}
        busy={busy}
        onViewVersion={setViewingVersion}
        onConfirm={handleConfirm}
        onQuoteSelected={handleQuoteSelected}
      />
    );
  }

  return (
    <main className="h-screen overflow-y-auto scrollbar-thin lg:h-screen lg:overflow-hidden">
      <div className="lg:h-screen lg:overflow-hidden lg:[grid-template-rows:minmax(0,1fr)] flex flex-col">
        {/* 吸顶 Header：返回 + 面包屑 + 模式/状态徽章 */}
        <header className="sticky top-0 z-40 h-14 shrink-0 flex items-center gap-3 px-6 border-b border-border bg-background/80 backdrop-blur-sm">
          <button
            type="button"
            aria-label="返回"
            onClick={() => navigate(-1)}
            className="w-7 h-7 rounded-md flex items-center justify-center text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
          >
            <ArrowLeft className="w-4 h-4" />
          </button>
          <nav className="flex items-center gap-1.5 text-sm min-w-0">
            <button type="button" onClick={() => navigate('/')} className="text-muted-foreground hover:text-primary transition-colors shrink-0">
              工作台
            </button>
            <span className="text-muted-foreground/40">/</span>
            <button
              type="button"
              onClick={() => navigate(`/project/${requirement.project_id}`)}
              className="text-muted-foreground hover:text-primary transition-colors truncate max-w-48"
            >
              {requirement.project_name}
            </button>
            <span className="text-muted-foreground/40">/</span>
            <span className="text-foreground truncate">需求详情</span>
          </nav>
          <div className="ml-auto flex items-center gap-1.5">
            {iterations > 0 && <span className="text-[11px] text-muted-foreground/60 mr-1">迭代 {iterations} 次</span>}
            {requirement.complexity && (
              <Badge className="bg-muted text-muted-foreground">{requirement.complexity}</Badge>
            )}
            {requirement.lane && (
              <Badge className="bg-muted text-muted-foreground">
                {{ full: '全三阶段', user_review: '用户即Review', micro: '微任务道' }[requirement.lane] ?? requirement.lane}
              </Badge>
            )}
            {(requirement.estimated_minutes !== null || requirement.actual_minutes !== null || requirement.total_tokens !== null) && (
              <span className="text-[11px] text-muted-foreground/60">
                {requirement.estimated_minutes !== null && <span>预计 {requirement.estimated_minutes}min </span>}
                {requirement.actual_minutes !== null && (
                  <span className={
                    requirement.estimated_minutes !== null &&
                    requirement.estimated_minutes > 0 &&
                    requirement.actual_minutes > requirement.estimated_minutes * 1.2
                      ? 'text-destructive font-medium'
                      : ''
                  }>
                    实际 {requirement.actual_minutes}min{' '}
                  </span>
                )}
                {requirement.total_tokens !== null && <span>· {formatTokens(requirement.total_tokens)} tokens</span>}
              </span>
            )}
            <Badge className={mode.className}>{mode.label}</Badge>
            {requirement.status === 'completed' ? (
              // 已完成：实底绿 + ✓，终态一眼可辨
              <span className="inline-flex items-center gap-1 h-6 px-2.5 rounded-full bg-emerald-600 text-white text-xs font-medium shadow-sm shadow-emerald-600/30">
                <CheckCircle2 className="w-3.5 h-3.5" />
                已完成
              </span>
            ) : (
              <Badge className={status.className} dot={status.pulse}>
                {status.label}
              </Badge>
            )}
            {requirement.status === 'failed' && (
              <Button size="sm" variant="secondary" className="h-7 px-2.5 text-xs" onClick={handleRetry} loading={busy}>
                {!busy && <RotateCcw className="w-3 h-3" />}
                重试
              </Button>
            )}
          </div>
        </header>

        {/* 当前目标区（吸顶） */}
        <section className="sticky top-14 z-30 shrink-0 border-b border-border bg-background/95 backdrop-blur-md animate-slide-down">
          <div className="px-6 py-3">
            <div className="flex items-center gap-2">
              <span className="inline-flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
                <Target className="w-3.5 h-3.5" />
                当前目标
              </span>
              {status.pulse && <span className="w-1.5 h-1.5 rounded-full bg-current text-primary animate-pulse" />}
              {!isCurrentViewing(effectiveViewingVersion, requirement.current_version) && (
                <Badge className="bg-amber-600/10 dark:bg-amber-400/10 text-amber-600 dark:text-amber-400">
                  历史版本 v{effectiveViewingVersion}
                </Badge>
              )}
            </div>
            {goalText ? (
              <p className="mt-1 text-sm text-foreground leading-relaxed line-clamp-2">{goalText}</p>
            ) : (
              <p className="mt-1 text-sm text-muted-foreground/70 leading-relaxed animate-pulse">
                理解完成后，这里会显示本次需求的目标总结
                {sourceDocPath && <>（来自 <span className="font-mono text-[13px]">{sourceDocPath}</span>）</>}
              </p>
            )}
            <div className="mt-1.5 flex items-center gap-2 flex-wrap">
              {showOriginal && (
                <p className="text-xs text-muted-foreground/60 line-clamp-1">你的原始需求：{originalText}</p>
              )}
              {attachments.length > 0 && (
                <div className="flex items-center gap-1.5 flex-wrap">
                  {attachments.map((attachment) => (
                    <span key={attachment.key} className="inline-flex items-center gap-1 h-8 px-1.5 rounded-lg border border-border bg-card text-[11px] text-muted-foreground">
                      {attachment.kind === 'image' ? (
                        <button
                          type="button"
                          aria-label={`预览 ${attachment.name}`}
                          onClick={() => setLightbox(attachmentUrl(attachment.key))}
                          className="shrink-0"
                        >
                          <img src={attachmentUrl(attachment.key)} alt={attachment.name} className="w-8 h-8 rounded object-cover -my-1" />
                        </button>
                      ) : (
                        <>
                          {attachment.kind === 'folder' ? (
                            <FolderClosed className="w-3 h-3" />
                          ) : (
                            <FileIcon className="w-3 h-3" />
                          )}
                          <span className="max-w-32 truncate">{attachment.name}</span>
                        </>
                      )}
                    </span>
                  ))}
                </div>
              )}
            </div>
          </div>
        </section>

        {/* 阶段进度条 */}
        <div className="shrink-0 px-6 pt-3">
          <StageProgress requirement={requirement} tasks={detail.tasks} iterations={iterations} selected={viewStage} onSelect={setViewStage} />
        </div>

        {/* 两栏主区：左主 2 / 右辅 1，桌面各自滚动 */}
        <div className="flex-1 min-h-0 lg:overflow-hidden grid grid-cols-1 lg:grid-cols-3 gap-5 p-6">
          <div className="lg:col-span-2 lg:overflow-y-auto lg:pr-1 scrollbar-thin space-y-5 min-w-0">
            {viewStage && (
              <div className="flex items-center justify-between gap-3 rounded-lg border border-primary/30 bg-primary/5 px-3.5 py-2.5">
                <p className="text-xs text-foreground/80">
                  正在回看「{STAGE_LABELS[viewStage] ?? viewStage}」阶段的历史产出
                </p>
                <Button size="sm" variant="secondary" className="h-6 px-2.5 text-[11px] shrink-0" onClick={() => setViewStage(null)}>
                  回到当前
                </Button>
              </div>
            )}
            {workspace}

            {viewStage === null && !confirmed && currentQuestions.length > 0 && isCurrentViewing(effectiveViewingVersion, requirement.current_version) && (
              <QuestionCards
                questions={currentQuestions}
                drafts={questionDrafts}
                onChange={(qid, draft) => setQuestionDrafts((prev) => ({ ...prev, [qid]: draft }))}
                onSubmit={() => void handleAnswers(buildAnswerFeedback(currentQuestions, questionDrafts))}
                submitting={busy}
                answeredCount={
                  currentQuestions.filter((q) => {
                    const d = questionDrafts[q.id];
                    return d && (d.option !== null || d.text.trim().length > 0);
                  }).length
                }
              />
            )}

            {pendingDecisions.length > 0 && <DecisionPanel decisions={pendingDecisions} onResolved={handleResolve} />}

            {requirement.status === 'failed' && (
              <div className="flex items-center justify-between gap-3 rounded-lg border border-red-600/30 dark:border-red-400/20 bg-red-600/10 dark:bg-red-400/10 px-3.5 py-2.5">
                <p className="text-xs text-red-600 dark:text-red-400">需求在执行中失败，可从失败阶段重试恢复</p>
                <Button size="sm" variant="secondary" className="h-7 px-2.5 text-xs shrink-0" onClick={handleRetry} loading={busy}>
                  {!busy && <RotateCcw className="w-3 h-3" />}
                  重试
                </Button>
              </div>
            )}
            <div className="space-y-3">
              <TabPills
                value={bottomTab}
                onChange={setBottomTab}
                options={[
                  { value: 'artifacts', label: `产物（${detail.artifacts.length}）` },
                  { value: 'logs', label: `日志（${detail.logs.length}）` },
                ]}
                className="self-start"
              />
              {bottomTab === 'artifacts' ? <ArtifactList artifacts={detail.artifacts} /> : <ExecutionLog logs={detail.logs} />}
            </div>
          </div>

          <div ref={quoteScrollRef} className="lg:col-span-1 lg:overflow-y-auto lg:pl-1 scrollbar-thin space-y-4 min-w-0">
            <FeedbackComments
              items={feedbackHistory}
              composer={
                requirement.status !== 'understanding' && requirement.status !== 'questioning'
                  ? {
                      mode: inUnderstandingPhase ? 'reunderstand' : 'iterate',
                      hasQuestions: currentQuestions.length > 0 && !confirmed,
                      quote,
                      onClearQuote: () => setQuote(null),
                      onSubmit: inUnderstandingPhase ? handleReunderstand : handleIterate,
                    }
                  : undefined
              }
            />
          </div>
        </div>
      </div>

      {lightbox && (
        <div className="fixed inset-0 z-[100] bg-black/85 flex items-center justify-center" onClick={() => setLightbox(null)}>
          <button
            type="button"
            aria-label="关闭预览"
            className="absolute top-5 right-5 w-8 h-8 rounded-md text-white/80 hover:text-white hover:bg-white/10 flex items-center justify-center"
            onClick={() => setLightbox(null)}
          >
            <X className="w-4 h-4" />
          </button>
          <img src={lightbox} alt="附件预览" className="max-w-[80vw] max-h-[80vh] rounded-lg" />
        </div>
      )}
    </main>
  );
}

function isCurrentViewing(viewing: number, current: number): boolean {
  return viewing === current;
}

// 当前 Agent 名称：优先读日志 details.executor（结构化），老数据回退解析「执行者：」文案。
function executorOf(logs: ExecutionLogItem[]): string | null {
  for (let i = logs.length - 1; i >= 0; i--) {
    const fromDetails = (logs[i].details as { executor?: unknown } | null)?.executor;
    if (typeof fromDetails === 'string' && fromDetails) return fromDetails;
    if (logs[i].message.startsWith('执行者：')) {
      return logs[i].message.replace(/^执行者：/, '').split('，')[0].trim();
    }
  }
  return null;
}

// 当前阶段 runtime 种类：读 details.runtime_kind（zcode/codex/opencode），供「打开桌面 App」等按 kind 的分支。
function runtimeKindOf(logs: ExecutionLogItem[]): string | null {
  for (let i = logs.length - 1; i >= 0; i--) {
    const kind = (logs[i].details as { runtime_kind?: unknown } | null)?.runtime_kind;
    if (typeof kind === 'string' && kind) return kind;
  }
  return null;
}

// 步进条回看：按结构化 stage 匹配产物（title 只是展示文案，不参与匹配），取最新版本。
const STAGE_LABEL: Record<string, string> = {
  plan: '实施方案',
  implement: '交付产物',
  verify: '验证报告',
  review: '审查报告',
};

function StageArtifactPanel({
  artifacts,
  stage,
}: {
  artifacts: RequirementDetail['artifacts'];
  stage: string;
}) {
  const label = STAGE_LABEL[stage] ?? stage;
  const latest = artifacts.filter((a) => a.stage === stage).sort((a, b) => b.version - a.version)[0];
  if (latest) {
    return <ArtifactList artifacts={[latest]} />;
  }
  return (
    <div className="bg-card border border-border rounded-xl px-4 py-8 text-center">
      <p className="text-sm text-muted-foreground">「{label}」尚未生成——该阶段可能还在进行或未到达</p>
    </div>
  );
}
