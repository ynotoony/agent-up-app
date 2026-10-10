import { useLayoutEffect, useEffect, useRef, useState } from 'react';
import { BookOpen, Check, CheckCircle2, ChevronDown, ChevronRight, Clock, FileText, Loader2, RefreshCw, RotateCcw, ScrollText, Shield, Sparkles, X, AlertTriangle } from 'lucide-react';
import { format } from 'date-fns';
import { zhCN } from 'date-fns/locale';
import type { GovernanceItem, GovernanceTicketsResult, InitReport, ProjectDoc, Requirement } from '../../shared/types';
import { api, errorMessage } from '@/lib/api';
import { Badge } from './ui/badge';
import { Button } from './ui/button';
import { EmptyState } from './ui/empty';
import { TabPills } from './ui/tabs';
import { cn } from '@/lib/utils';

// 项目治理/文档面板 + 初始化报告卡片。数据全部来自 SQLite（governance_items / project_docs）。

const ACTION_CONFIG: Record<string, { label: string; className: string }> = {
  created: {
    label: '已创建',
    className: 'bg-emerald-600/10 dark:bg-emerald-400/10 text-emerald-600 dark:text-emerald-400',
  },
  updated: {
    label: '已更新',
    className: 'bg-emerald-600/10 dark:bg-emerald-400/10 text-emerald-600 dark:text-emerald-400',
  },
  kept: {
    label: '保持',
    className: 'bg-emerald-500/8 dark:bg-emerald-400/8 text-emerald-700/80 dark:text-emerald-300/80',
  },
  skipped: {
    label: '跳过',
    className: 'bg-emerald-500/8 dark:bg-emerald-400/8 text-emerald-700/80 dark:text-emerald-300/80',
  },
  warned: {
    label: '警告',
    className: 'bg-amber-500/15 text-amber-600 dark:text-amber-400',
  },
};

export function InitReportCard({ report, onClose }: { report: InitReport; onClose: () => void }) {
  return (
    <section className="bg-card border border-border rounded-xl p-4 space-y-2">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 text-sm font-medium">
          <Sparkles className="w-4 h-4 text-primary" />
          初始化报告 · {report.name}
        </div>
        <button type="button" aria-label="关闭初始化报告" onClick={onClose} className="w-6 h-6 rounded-md flex items-center justify-center text-muted-foreground/60 hover:text-foreground hover:bg-accent">
          <X className="w-3.5 h-3.5" />
        </button>
      </div>
      <div className="space-y-1">
        {report.steps.map((step, i) => {
          const cfg = ACTION_CONFIG[step.action] ?? ACTION_CONFIG.kept;
          return (
            <div key={i} className="flex items-center gap-2 text-xs">
              <Badge className={cfg.className}>{cfg.label}</Badge>
              <span className="font-medium">{step.item}</span>
              <span className="text-muted-foreground">{step.detail}</span>
            </div>
          );
        })}
      </div>
      <p className="text-[11px] text-muted-foreground/60">
        文档入库 {report.docs_added} 篇 · 治理种子 {report.governance_seeded} 条 · 待定 {report.pending_open} 条
        {report.requirements_created > 0 &&
          ` · 存量文档已转 ${report.requirements_created} 条需求${report.locally_archived > 0 ? `（本地规则预归档 ${report.locally_archived} 条已完成，其余依次理解）` : '（依次理解中）'}`}
        （治理数据存于应用数据库，目录保持零污染）
      </p>
      <div className="rounded-lg bg-emerald-500/5 border border-emerald-500/20 p-3 space-y-1.5">
        {[
          ...(report.locally_archived > 0 ? [`已自动归档 ${report.locally_archived} 条已完成内容，不占理解队列`] : []),
          '幂等可重跑：重复初始化不会重复创建，已有内容一律不动',
          '数据在库：文档全文与治理数据存于应用数据库，目录文件误删也可找回',
          ...(report.docs_missing > 0 ? [`${report.docs_missing} 篇文档在目录中已消失，库中记录与全文仍保留（标记缺失）`] : []),
        ].map((tip) => (
          <p key={tip} className="flex items-center gap-1.5 text-[11px] text-emerald-600 dark:text-emerald-400">
            <CheckCircle2 className="w-3 h-3 shrink-0" />
            {tip}
          </p>
        ))}
      </div>
    </section>
  );
}

// 初始化进度面板：初始化报告 + 队列消化进度的唯一入口。
// initializing 需求不在需求列表展示，这里看消化进度（5s 轮询）；队列为空但有报告时面板保持显示。
// 卡住判定：updated_at 距今 >2h 仍 initializing → 失败，可重试。
const STALL_THRESHOLD_MS = 2 * 60 * 60 * 1000;

function docPathOf(requirement: Requirement): string {
  return requirement.source_doc_path || requirement.content_preview.slice(0, 40);
}

export function InitializingPanel({ projectId, report, onCloseReport }: { projectId: string; report: InitReport | null; onCloseReport: () => void }) {
  const [items, setItems] = useState<Requirement[] | null>(null);
  const [open, setOpen] = useState(false);
  const [retrying, setRetrying] = useState<string | null>(null);
  // 批量重试的排队登记：id → 入队时刻。updated_at 刷新到入队时刻之后 → 该条已开始理解（排队结束）
  const [queuedAt, setQueuedAt] = useState<Map<string, number>>(new Map());

  useEffect(() => {
    let cancelled = false;
    const fetchItems = () => {
      api.projects
        .initializing(projectId)
        .then((list) => {
          if (!cancelled) setItems(list);
        })
        .catch(() => {
          if (!cancelled) setItems([]);
        });
    };
    fetchItems();
    const timer = window.setInterval(fetchItems, 5000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [projectId]);

  // 新报告到达时自动展开
  useEffect(() => {
    if (report) setOpen(true);
  }, [report]);

  const retryOne = async (id: string) => {
    setRetrying(id);
    try {
      await api.requirements.retryInitializing(id);
      setQueuedAt((prev) => new Map(prev).set(id, Date.now()));
      setItems(await api.projects.initializing(projectId));
    } catch (err) {
      console.error('[init-retry]', err);
    } finally {
      setRetrying(null);
    }
  };

  const retryAllStalled = async (stalled: Requirement[]) => {
    const ids = stalled.map((s) => s.id);
    try {
      await api.requirements.retryInitializingBatch(ids);
      const now = Date.now();
      setQueuedAt((prev) => new Map(ids.map((id) => [id, now])));
      setItems(await api.projects.initializing(projectId));
    } catch (err) {
      console.error('[init-retry-batch]', err);
    }
  };

  if (items === null) return null;
  const hasQueue = items.length > 0;
  // 状态机：排队中（批量重试登记后 updated_at 未刷新）> 失败（停滞超阈值，直接说失败）> 处理中
  const isQueued = (item: Requirement) => {
    const at = queuedAt.get(item.id);
    return at !== undefined && new Date(item.updated_at).getTime() < at;
  };
  const stalled = hasQueue ? items.filter((i) => !isQueued(i) && Date.now() - new Date(i.updated_at).getTime() > STALL_THRESHOLD_MS) : [];
  const queued = hasQueue ? items.filter(isQueued) : [];
  // 清理已离场条目的排队登记（理解完成会从 initializing 列表消失）
  if (hasQueue && queuedAt.size > 0) {
    const alive = new Set(items.map((i) => i.id));
    const stale = [...queuedAt.keys()].filter((id) => !alive.has(id) || !isQueued(items.find((i) => i.id === id)!));
    if (stale.length > 0) {
      setQueuedAt((prev) => {
        const next = new Map(prev);
        for (const id of stale) next.delete(id);
        return next;
      });
    }
  }
  if (!hasQueue && !report) return null; // 队列消化完且无报告，入口自动消失

  return (
    <section className="bg-violet-500/5 border border-violet-500/20 rounded-xl px-4 py-3 space-y-2">
      <button type="button" onClick={() => setOpen((v) => !v)} className="w-full flex items-center gap-2 text-left">
        {open ? <ChevronDown className="w-4 h-4 text-violet-500 shrink-0" /> : <ChevronRight className="w-4 h-4 text-violet-500 shrink-0" />}
        <span className="text-sm font-medium text-violet-600 dark:text-violet-400">初始化进度</span>
        {hasQueue ? (
          <>
            {/* 总数 + 互斥细分：N = 排队 + 失败 + 处理中（处理中不单列，可由总数推算） */}
            <Badge className="bg-violet-600/10 dark:bg-violet-400/10 text-violet-600 dark:text-violet-400">{items.length} 条初始化中</Badge>
            {queued.length > 0 && <Badge className="bg-muted text-muted-foreground">排队 {queued.length}</Badge>}
            {stalled.length > 0 && <Badge className="bg-destructive/10 text-destructive">失败 {stalled.length}</Badge>}
          </>
        ) : (
          <Badge className="bg-muted text-muted-foreground">队列空闲</Badge>
        )}
        {hasQueue && <span className="text-[11px] text-muted-foreground">完成后进入需求列表 · 5 秒自动刷新</span>}
      </button>
      {open && (
        // 展开区整体限高内滚：项目页宽屏是「固定头部 + overflow-hidden」布局，面板若无限撑高会裁掉底部
        <div className="space-y-2 max-h-[48vh] overflow-y-auto scrollbar-thin pr-1">
          {report && <InitReportCard report={report} onClose={onCloseReport} />}
          {hasQueue && (
            <div className="divide-y divide-border rounded-lg bg-card border border-border">
              {items.map((item) => {
                const queuedNow = isQueued(item);
                const isStalled = !queuedNow && Date.now() - new Date(item.updated_at).getTime() > STALL_THRESHOLD_MS;
                return (
                  <div key={item.id} className="flex items-center gap-2 px-3 py-2 text-xs">
                    {queuedNow ? (
                      <Clock className="w-3 h-3 text-muted-foreground shrink-0" />
                    ) : isStalled ? (
                      <AlertTriangle className="w-3 h-3 text-destructive shrink-0" />
                    ) : (
                      <Loader2 className="w-3 h-3 animate-spin text-violet-500 shrink-0" />
                    )}
                    <span className="font-mono truncate flex-1 min-w-0" title={docPathOf(item)}>
                      {docPathOf(item)}
                    </span>
                    {queuedNow && <Badge className="shrink-0 bg-muted text-muted-foreground">排队中</Badge>}
                    {isStalled && <Badge className="shrink-0 bg-destructive/10 text-destructive">失败</Badge>}
                    <span className="text-muted-foreground/60 shrink-0">
                      {(() => {
                        // 显示最近一次理解时间：重试过的条目 updated_at 晚于 created_at，标「重新理解」；否则标「入队」
                        const updated = new Date(item.updated_at).getTime();
                        const created = new Date(item.created_at).getTime();
                        const retried = updated - created > 60_000;
                        return `${format(new Date(item.updated_at), 'MM月dd日 HH:mm', { locale: zhCN })} ${retried ? '重新理解' : '入队'}`;
                      })()}
                    </span>
                    {isStalled && (
                      <Button size="sm" variant="secondary" className="h-6 px-2 text-[11px] shrink-0" onClick={() => void retryOne(item.id)} disabled={retrying === item.id}>
                        {retrying === item.id ? <Loader2 className="w-3 h-3 animate-spin" /> : '重试'}
                      </Button>
                    )}
                  </div>
                );
              })}
            </div>
          )}
          {stalled.length > 0 && (
            <div className="flex justify-end">
              <Button size="sm" variant="secondary" className="h-7 px-2.5 text-[11px]" onClick={() => void retryAllStalled(stalled)} disabled={retrying !== null}>
                {retrying !== null ? <Loader2 className="w-3 h-3 animate-spin" /> : <RotateCcw className="w-3 h-3" />}
                重试全部失败（{stalled.length}）→ 串行排队
              </Button>
            </div>
          )}
          <div className="rounded-lg bg-emerald-500/5 border border-emerald-500/20 p-3 space-y-1.5">
            {[
              '目录零污染：初始化不向项目目录写入任何文件',
              '自动归档：命中完成信号的文档直接归为已完成，不占理解队列',
              '自动进场：理解完成的需求会出现在下方需求列表并计入统计',
            ].map((tip) => (
              <p key={tip} className="flex items-center gap-1.5 text-[11px] text-emerald-600 dark:text-emerald-400">
                <CheckCircle2 className="w-3 h-3 shrink-0" />
                {tip}
              </p>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}

type GovernanceFilter = 'rule' | 'role' | 'term' | 'pending';

export function GovernancePanel({ projectId }: { projectId: string }) {
  const [items, setItems] = useState<GovernanceItem[] | null>(null);
  const [filter, setFilter] = useState<GovernanceFilter>('rule');
  const [error, setError] = useState('');
  const [answering, setAnswering] = useState<string | null>(null);
  const [answer, setAnswer] = useState('');
  const [busy, setBusy] = useState(false);

  const fetchItems = () => {
    api.projects
      .governance(projectId)
      .then(setItems)
      .catch((err) => setError(errorMessage(err)));
  };
  useLayoutEffect(fetchItems, [projectId]);

  const submitAnswer = async (itemId: string) => {
    if (!answer.trim()) return;
    setBusy(true);
    setError('');
    try {
      await api.projects.governanceAnswer(itemId, answer.trim());
      setAnswering(null);
      setAnswer('');
      fetchItems();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  if (error && !items) return <p className="text-xs text-destructive px-1">{error}</p>;
  if (!items) {
    return (
      <div className="space-y-2 animate-pulse p-1">
        {[0, 1, 2].map((i) => (
          <div key={i} className="h-10 rounded bg-muted" />
        ))}
      </div>
    );
  }

  const counts = {
    rule: items.filter((i) => i.kind === 'rule').length,
    role: items.filter((i) => i.kind === 'role').length,
    term: items.filter((i) => i.kind === 'term').length,
    pending: items.filter((i) => i.kind === 'pending' && i.status === 'active').length,
  };
  const filtered = items.filter((i) => i.kind === filter && (filter !== 'pending' || true));
  const label: Record<GovernanceFilter, string> = { rule: '规则', role: '角色合同', term: '领域词条', pending: '待定' };

  return (
    <div className="space-y-3">
      <TabPills
        value={filter}
        onChange={setFilter}
        options={[
          { value: 'rule', label: `规则 ${counts.rule}` },
          { value: 'role', label: `角色 ${counts.role}` },
          { value: 'term', label: `词条 ${counts.term}` },
          { value: 'pending', label: `待定 ${counts.pending}` },
        ]}
        className="self-start"
      />
      {error && <p className="text-xs text-destructive px-1">{error}</p>}
      {filtered.length === 0 ? (
        <div className="bg-card border border-border rounded-xl">
          <EmptyState
            icon={filter === 'pending' ? <ScrollText className="w-8 h-8" /> : <Shield className="w-8 h-8" />}
            title={`${label[filter]}为空`}
            description={filter === 'term' ? '词条是项目事实：无依据不编造，由治理补全或人工填写' : '点击「重新初始化」可补种治理数据'}
          />
        </div>
      ) : (
        <div className="bg-card border border-border rounded-xl divide-y divide-border overflow-hidden">
          {filtered.map((item) => (
            <GovernanceRow
              key={item.id}
              item={item}
              answering={answering === item.id}
              answer={answering === item.id ? answer : ''}
              busy={busy}
              onAnswerStart={() => {
                setAnswering(answering === item.id ? null : item.id);
                setAnswer(String(item.body.answer ?? ''));
              }}
              onAnswerChange={setAnswer}
              onAnswerSubmit={() => void submitAnswer(item.id)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function levelBadge(item: GovernanceItem) {
  if (item.kind !== 'rule') return null;
  const level = String(item.body.level ?? 'MUST');
  const cls =
    level === 'MUST NOT' ? 'bg-destructive/10 text-destructive' : level === 'MUST' ? 'bg-primary/10 text-primary' : 'bg-muted text-muted-foreground';
  return <Badge className={cls}>{level}</Badge>;
}

function GovernanceRow({
  item,
  answering,
  answer,
  busy,
  onAnswerStart,
  onAnswerChange,
  onAnswerSubmit,
}: {
  item: GovernanceItem;
  answering: boolean;
  answer: string;
  busy: boolean;
  onAnswerStart: () => void;
  onAnswerChange: (v: string) => void;
  onAnswerSubmit: () => void;
}) {
  const resolved = item.status === 'resolved';
  const question = String(item.body.question ?? item.title);
  const hint = String(item.body.hint ?? '');
  const answerText = String(item.body.answer ?? '');
  return (
    <div className="px-4 py-3 space-y-1.5">
      <div className="flex items-center gap-1.5 flex-wrap">
        {levelBadge(item)}
        {item.kind === 'pending' && (
          <Badge className={resolved ? 'bg-muted text-muted-foreground' : 'bg-amber-500/15 text-amber-600 dark:text-amber-400'}>
            {resolved ? '已解决' : '待定'}
          </Badge>
        )}
        {item.kind !== 'pending' && <span className="text-xs font-medium">{item.title}</span>}
        <span className="text-[11px] text-muted-foreground/50 font-mono">{item.key}</span>
      </div>
      {item.kind === 'rule' ? (
        <p className="text-xs text-foreground/80 leading-relaxed">
          {String(item.body.action ?? '')}
          {String(item.body.forbidden ?? '无') !== '无' && (
            <span className="text-muted-foreground">（禁止：{String(item.body.forbidden)}）</span>
          )}
        </p>
      ) : item.kind === 'role' ? (
        <p className="text-xs text-foreground/80 leading-relaxed">
          {String(item.body.scope ?? '')}
          <span className="text-muted-foreground">（能力：{(item.body.capabilities as string[] | undefined)?.join('、') ?? '—'}）</span>
        </p>
      ) : item.kind === 'term' ? (
        <p className="text-xs text-foreground/80 leading-relaxed">
          <span className="font-medium">{item.key}</span>：{String(item.body.definition ?? '')}
          {Boolean(item.body.avoid) && <span className="text-muted-foreground">（Avoid: {String(item.body.avoid)}）</span>}
        </p>
      ) : (
        <>
          <p className={cn('text-xs leading-relaxed', resolved ? 'text-muted-foreground' : 'text-foreground/90')}>
            {question}
            {hint && <span className="text-muted-foreground/60">（提示：{hint}）</span>}
          </p>
          {resolved ? (
            <p className="text-xs text-primary/90">答：{answerText}</p>
          ) : answering ? (
            <div className="flex items-center gap-1.5 pt-0.5">
              <input
                value={answer}
                onChange={(e) => onAnswerChange(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && onAnswerSubmit()}
                placeholder="输入答案，回车提交"
                className="flex-1 min-w-0 h-7 px-2 rounded-md bg-background border border-border text-xs focus:outline-none focus:border-primary/50"
              />
              <Button size="sm" className="h-6 px-2 text-[11px]" onClick={onAnswerSubmit} disabled={busy || !answer.trim()}>
                {busy ? <Loader2 className="w-3 h-3 animate-spin" /> : <Check className="w-3 h-3" />}
              </Button>
            </div>
          ) : (
            <button type="button" onClick={onAnswerStart} className="text-[11px] text-primary hover:underline">
              回答
            </button>
          )}
        </>
      )}
    </div>
  );
}

const DOC_GROUP: { kind: ProjectDoc['kind']; label: string }[] = [
  { kind: 'requirement', label: '需求类' },
  { kind: 'readme', label: '说明类' },
  { kind: 'doc', label: '文档' },
];
export function ProjectDocsPanel({ projectId, onConverted }: { projectId: string; onConverted: (requirementId: string) => void }) {
  const [docs, setDocs] = useState<ProjectDoc[] | null>(null);
  const [error, setError] = useState('');
  const [openId, setOpenId] = useState<string | null>(null);
  const [content, setContent] = useState<string>('');
  const [loadingId, setLoadingId] = useState<string | null>(null);
  const [converting, setConverting] = useState<string | null>(null);
  const docReadGeneration = useRef(0);

  useLayoutEffect(() => {
    let active = true;
    api.projects
      .docs(projectId)
      .then((result) => { if (active) setDocs(result); })
      .catch((err) => { if (active) setError(errorMessage(err)); });
    return () => {
      active = false;
      docReadGeneration.current += 1;
    };
  }, [projectId]);

  const toggle = async (doc: ProjectDoc) => {
    if (openId === doc.id) {
      docReadGeneration.current += 1;
      setOpenId(null);
      return;
    }
    const generation = ++docReadGeneration.current;
    setOpenId(doc.id);
    setContent('');
    setLoadingId(doc.id);
    try {
      const result = await api.projects.docRead(doc.id);
      if (generation !== docReadGeneration.current) return;
      setContent(result.content);
    } catch (err) {
      if (generation === docReadGeneration.current) setContent(errorMessage(err));
    } finally {
      if (generation === docReadGeneration.current) setLoadingId(null);
    }
  };

  const convert = async (doc: ProjectDoc) => {
    setConverting(doc.id);
    setError('');
    try {
      const requirement = await api.projects.docToRequirement(projectId, doc.id);
      onConverted(requirement.id);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setConverting(null);
    }
  };

  if (error && !docs) return <p className="text-xs text-destructive px-1">{error}</p>;
  if (!docs) {
    return (
      <div className="space-y-2 animate-pulse p-1">
        {[0, 1].map((i) => (
          <div key={i} className="h-10 rounded bg-muted" />
        ))}
      </div>
    );
  }
  if (docs.length === 0) {
    return (
      <div className="bg-card border border-border rounded-xl">
        <EmptyState icon={<BookOpen className="w-8 h-8" />} title="未发现存量文档" description="点击「重新初始化」可重扫目录（.md/.txt，文档目录下钻一层）" />
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {error && <p className="text-xs text-destructive px-1">{error}</p>}
      {DOC_GROUP.map(({ kind, label }) => {
        const group = docs.filter((d) => d.kind === kind);
        if (group.length === 0) return null;
        return (
          <div key={kind} className="space-y-1.5">
            <p className="text-[11px] font-medium text-muted-foreground px-1">
              {label}（{group.length}）
            </p>
            <div className="bg-card border border-border rounded-xl divide-y divide-border overflow-hidden">
              {group.map((doc) => (
                <div key={doc.id}>
                  <div className="flex items-center gap-2 px-4 py-2.5">
                    <button type="button" onClick={() => void toggle(doc)} className="flex-1 min-w-0 flex items-center gap-2 text-left">
                      {openId === doc.id ? <ChevronDown className="w-3.5 h-3.5 text-muted-foreground shrink-0" /> : <FileText className="w-3.5 h-3.5 text-muted-foreground shrink-0" />}
                      <span className="text-sm truncate">{doc.title}</span>
                      <span className="text-[11px] text-muted-foreground/50 font-mono truncate hidden sm:inline">{doc.rel_path}</span>
                      {doc.file_missing && <Badge className="bg-amber-500/15 text-amber-600 dark:text-amber-400">文件已消失</Badge>}
                      {!doc.has_content && <Badge className="bg-muted text-muted-foreground">二进制</Badge>}
                      {doc.requirement_id && <Badge className="bg-primary/10 text-primary">已转需求</Badge>}
                    </button>
                    {kind === 'requirement' && doc.has_content && !doc.requirement_id && (
                      <Button size="sm" variant="secondary" className="h-6 px-2 text-[11px] shrink-0" onClick={() => void convert(doc)} disabled={converting === doc.id}>
                        {converting === doc.id ? <Loader2 className="w-3 h-3 animate-spin" /> : '转为需求'}
                      </Button>
                    )}
                    {kind === 'requirement' && doc.has_content && doc.requirement_id && (
                      <Button size="sm" variant="ghost" className="h-6 px-2 text-[11px] shrink-0 text-muted-foreground" onClick={() => void convert(doc)} disabled={converting === doc.id} title="已自动转过一次；文档更新后可再次手动转为新需求">
                        再转一条
                      </Button>
                    )}
                  </div>
                  {openId === doc.id && (
                    <div className="px-4 pb-3">
                      {loadingId === doc.id ? (
                        <Loader2 className="w-4 h-4 animate-spin text-muted-foreground" />
                      ) : (
                        <pre className="text-[11px] leading-relaxed text-foreground/80 whitespace-pre-wrap max-h-72 overflow-y-auto scrollbar-thin bg-background border border-border rounded-lg p-3">
                          {content}
                        </pre>
                      )}
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>
        );
      })}
    </div>
  );
}

// ---------------------------------------------------------------- 治理票（只读源，票 #1）

const TICKET_LANES: { key: string; label: string; badge: string }[] = [
  { key: 'ready', label: '可开工', badge: 'bg-emerald-600/10 dark:bg-emerald-400/10 text-emerald-600 dark:text-emerald-400' },
  { key: 'in_progress', label: '进行中', badge: 'bg-primary/10 text-primary' },
  { key: 'blocked', label: '受阻', badge: 'bg-amber-500/15 text-amber-600 dark:text-amber-400' },
  { key: 'done', label: '已完成', badge: 'bg-muted text-muted-foreground' },
  { key: 'superseded', label: '已作废', badge: 'bg-muted text-muted-foreground/60' },
  { key: 'unknown', label: '未知状态', badge: 'bg-destructive/10 text-destructive' },
];

// 目标项目的真票源：docs/issues/index.json → 只读泳道视图。不写目标项目任何文件。
export function GovernanceTicketsPanel({ projectId }: { projectId: string }) {
  const [result, setResult] = useState<GovernanceTicketsResult | null>(null);
  const [error, setError] = useState('');
  const [openId, setOpenId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [launching, setLaunching] = useState<string | null>(null);

  const fetchTickets = () => {
    setLoading(true);
    api.projects
      .tickets(projectId, true)
      .then((r) => {
        setResult(r);
        setError('');
      })
      .catch((err) => setError(errorMessage(err)))
      .finally(() => setLoading(false));
  };
  useLayoutEffect(fetchTickets, [projectId]);

  const handleLaunch = (ticketId: string) => {
    setLaunching(ticketId);
    api.projects
      .ticketLaunch(projectId, ticketId)
      .then(() => {
        // 启动成功，无需额外处理
      })
      .catch((err) => alert(`启动失败：${errorMessage(err)}`))
      .finally(() => setLaunching(null));
  };

  if (error && !result) {
    return (
      <div className="bg-card border border-border rounded-xl">
        <EmptyState icon={<ScrollText className="w-8 h-8" />} title="没有读到治理票" description={error}>
          <button type="button" onClick={fetchTickets} className="mt-3 text-[11px] text-primary hover:underline">
            重试
          </button>
        </EmptyState>
      </div>
    );
  }
  if (!result) {
    return (
      <div className="space-y-2 animate-pulse p-1">
        {[0, 1, 2].map((i) => (
          <div key={i} className="h-10 rounded bg-muted" />
        ))}
      </div>
    );
  }

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2 px-1">
        <p className="text-[11px] text-muted-foreground flex-1">
          来自目标项目治理票索引（只读，共 {result.total} 张
          {result.skipped > 0 && <>，拒收异常行 {result.skipped}</>}）
        </p>
        <button type="button" aria-label="刷新治理票" onClick={fetchTickets} className="w-6 h-6 rounded-md flex items-center justify-center text-muted-foreground/60 hover:text-foreground hover:bg-accent">
          {loading ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <RefreshCw className="w-3.5 h-3.5" />}
        </button>
      </div>
      {result.total === 0 ? (
        <div className="bg-card border border-border rounded-xl">
          <EmptyState icon={<ScrollText className="w-8 h-8" />} title="索引为空" description="目标项目的治理票索引里还没有票" />
        </div>
      ) : (
        TICKET_LANES.map(({ key, label, badge }) => {
          const group = result.tickets.filter((t) => t.lane === key);
          if (group.length === 0) return null;
          return (
            <div key={key} className="space-y-1.5">
              <p className="text-[11px] font-medium text-muted-foreground px-1">
                {label}（{group.length}）
              </p>
              <div className="bg-card border border-border rounded-xl divide-y divide-border overflow-hidden">
                {group.map((t) => {
                  const open = openId === t.id;
                  return (
                    <div key={t.id}>
                      <button type="button" onClick={() => setOpenId(open ? null : t.id)} className="w-full flex items-center gap-2 px-4 py-2.5 text-left">
                        {open ? <ChevronDown className="w-3.5 h-3.5 text-muted-foreground shrink-0" /> : <ChevronRight className="w-3.5 h-3.5 text-muted-foreground shrink-0" />}
                        <span className="text-sm truncate flex-1 min-w-0">{t.title}</span>
                        {t.complexity && <Badge className="bg-muted text-muted-foreground">{String(t.complexity)}</Badge>}
                        {t.blocked_by.length > 0 && <Badge className="bg-amber-500/10 text-amber-600/80 dark:text-amber-400/80">依赖 {t.blocked_by.length}</Badge>}
                        <Badge className={badge}>{t.governance_status}</Badge>
                      </button>
                      {open && (
                        <div className="px-4 pb-3 space-y-2">
                          {t.body?.goal != null && (
                            <p className="text-xs text-foreground/80 leading-relaxed whitespace-pre-wrap">{String(t.body.goal)}</p>
                          )}
                          <div className="flex items-center gap-2 text-[11px] text-muted-foreground/60 font-mono">
                            <span>{t.id}</span>
                            {t.updated_at && <span>· {t.updated_at}</span>}
                          </div>
                          {t.blocked_by.length > 0 && (
                            <p className="text-[11px] text-muted-foreground">
                              前置：{t.blocked_by.join('、')}
                            </p>
                          )}
                          <div className="pt-1">
                            <Button
                              size="sm"
                              onClick={() => handleLaunch(t.id)}
                              disabled={launching === t.id}
                              className="h-7 text-xs"
                            >
                              {launching === t.id ? (
                                <>
                                  <Loader2 className="w-3 h-3 mr-1.5 animate-spin" />
                                  启动中...
                                </>
                              ) : (
                                <>
                                  <Sparkles className="w-3 h-3 mr-1.5" />
                                  跳转 Agent 执行
                                </>
                              )}
                            </Button>
                          </div>
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          );
        })
      )}
      {result.unknown_statuses && Object.keys(result.unknown_statuses).length > 0 && (
        <p className="text-[11px] text-destructive/80 px-1">
          未知状态票 {Object.keys(result.unknown_statuses).length} 张（原样列出，不静默丢弃）：
          {Object.entries(result.unknown_statuses).map(([id, s]) => `${id}=${s}`).join('，')}
        </p>
      )}
    </div>
  );
}
