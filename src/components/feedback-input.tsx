import { MessageSquareQuote, Quote, Send, UserRound, X } from 'lucide-react';
import { useState, type ClipboardEvent as ReactClipboardEvent, type ReactNode } from 'react';
import type { PendingAttachment, RequirementVersion } from '../../shared/types';
import { arrayBufferToBase64 } from '@/lib/utils';
import { AttachmentField, AttachmentList, clipsToImageItems, revokeAttachmentUrls, type AttachmentDraft } from './attachment-field';
import { PanelHeader } from './understanding-panel';
import { Button } from './ui/button';
import { EmptyState } from './ui/empty';

export interface FeedbackHistoryItem {
  id: string;
  text: string;
  source: RequirementVersion['source'];
  createdAt: string;
}

export interface FeedbackComposerProps {
  mode: 'reunderstand' | 'iterate';
  hasQuestions: boolean;
  quote: string | null;
  onClearQuote: () => void;
  onSubmit: (feedback: string, attachments: PendingAttachment[]) => Promise<void>;
}

const HISTORY_SOURCE_LABEL: Record<RequirementVersion['source'], string> = {
  initial: '初始理解',
  user_feedback: '反馈',
  question_answer: '回答质疑',
  iteration: '迭代',
  manual: '手动重跑',
};

// 评论区（右栏）：评论框 + 历史评论一体；composer 缺省时（理解/质疑阶段）只渲染评论流。
export function FeedbackComments({ items, composer }: { items: FeedbackHistoryItem[]; composer?: FeedbackComposerProps }) {
  const hint = composer?.mode === 'reunderstand' ? '将生成新理解版本' : composer ? '回到实施并生成新版本' : null;
  return (
    <div className="bg-card border border-border rounded-xl overflow-hidden">
      <PanelHeader
        title={`评论（${items.length}）`}
        icon={<MessageSquareQuote className="w-3.5 h-3.5" />}
        right={hint && <span className="text-[11px] text-muted-foreground/60">{hint}</span>}
      />
      {items.length === 0 ? (
        <EmptyState
          className="py-8"
          icon={<MessageSquareQuote className="w-10 h-10" />}
          title={composer ? '还没有评论，写下第一条反馈吧' : '还没有反馈记录'}
        />
      ) : (
        <div className="max-h-96 overflow-y-auto scrollbar-thin divide-y divide-border/60">
          {items.map((item) => (
            <CommentRow key={item.id} item={item} />
          ))}
        </div>
      )}
      {composer && <CommentComposer {...composer} />}
    </div>
  );
}

// 评论流：头像 + 「你 · 来源 · 时间」+ 正文，发丝线分隔（不再用气泡）。
function CommentRow({ item }: { item: FeedbackHistoryItem }) {
  return (
    <div className="flex items-start gap-2.5 px-4 py-3">
      <span className="mt-0.5 inline-flex items-center justify-center w-7 h-7 shrink-0 rounded-full bg-primary/15 border border-primary/20 text-primary">
        <UserRound className="w-3.5 h-3.5" />
      </span>
      <div className="min-w-0 flex-1">
        <p className="flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
          你
          <span className="px-1 rounded text-[10px] bg-primary/10 text-primary">{HISTORY_SOURCE_LABEL[item.source]}</span>
          <span className="text-[11px] font-normal text-muted-foreground/50">
            {new Date(item.createdAt).toLocaleString('zh-CN')}
          </span>
        </p>
        <p className="mt-1 text-sm text-foreground leading-relaxed break-words whitespace-pre-wrap">{item.text}</p>
      </div>
    </div>
  );
}

// 评论框：头像 + 一体化输入容器（引用 chip、无边框文本域、附件与发送按钮），提交中 spinner、3s 防连点。
function CommentComposer({ mode, hasQuestions, quote, onClearQuote, onSubmit }: FeedbackComposerProps) {
  const [text, setText] = useState('');
  const [attachments, setAttachments] = useState<AttachmentDraft[]>([]);
  const [submitting, setSubmitting] = useState(false);
  const [cooldown, setCooldown] = useState(false);

  const addAttachments = (items: AttachmentDraft[]) => setAttachments((prev) => [...prev, ...items]);

  const handlePaste = (e: ReactClipboardEvent<HTMLTextAreaElement>) => {
    const images = clipsToImageItems(e);
    if (images.length > 0) {
      e.preventDefault();
      addAttachments(images);
    }
  };

  const submit = async () => {
    const trimmed = text.trim();
    if (!trimmed && attachments.length === 0) return;
    setSubmitting(true);
    try {
      const payload: PendingAttachment[] = [];
      for (const draft of attachments) {
        const data = arrayBufferToBase64(await draft.file.arrayBuffer());
        payload.push({ name: draft.name, type: draft.type, size: draft.size, kind: draft.kind, relativePath: draft.relativePath, data });
      }
      const feedback = quote ? `【引用】"${quote}"\n${trimmed}` : trimmed;
      await onSubmit(feedback, payload);
      revokeAttachmentUrls(attachments);
      setAttachments([]);
      setText('');
      // 成功后 3s 防连点
      setCooldown(true);
      setTimeout(() => setCooldown(false), 3000);
    } finally {
      setSubmitting(false);
    }
  };

  const disabled = submitting || cooldown || (!text.trim() && attachments.length === 0);

  return (
    <div className="p-4 border-t border-border">
      <div className="flex items-start gap-2.5">
        <span className="mt-2 inline-flex items-center justify-center w-7 h-7 shrink-0 rounded-full bg-primary/15 border border-primary/20 text-primary">
          <UserRound className="w-3.5 h-3.5" />
        </span>
        <div className="flex-1 min-w-0 rounded-xl border border-border bg-background transition-all focus-within:border-primary/50 focus-within:ring-1 focus-within:ring-primary/20">
          {quote && <QuoteChip quote={quote} onClearQuote={onClearQuote} />}
          <textarea
            value={text}
            onChange={(e) => setText(e.target.value)}
            onPaste={handlePaste}
            rows={3}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) {
                e.preventDefault();
                void submit();
              }
            }}
            placeholder={hasQuestions ? '补充说明你的回答，或直接在上方点选选项…' : '说说哪里不满意，或想调整什么…'}
            className="w-full resize-none bg-transparent px-3 py-2 text-sm leading-relaxed placeholder:text-muted-foreground/50 focus:outline-none"
          />
          {attachments.length > 0 && (
            <div className="px-2.5 pb-1">
              <AttachmentList items={attachments} onRemove={(id) => setAttachments((prev) => prev.filter((a) => a.id !== id))} />
            </div>
          )}
          <div className="flex items-center justify-between px-2.5 pb-2">
            <AttachmentField variant="toolbar" onAdd={addAttachments} showCount count={attachments.length} />
            <Button size="sm" loading={submitting} disabled={disabled} onClick={() => void submit()}>
              {!submitting && <Send className="w-3.5 h-3.5" />}
              {hasQuestions ? '提交回答' : '发送'}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}

function QuoteChip({ quote, onClearQuote }: { quote: string; onClearQuote: () => void }): ReactNode {
  return (
    <div className="mx-2.5 mt-2.5 flex items-start gap-2 rounded-lg border border-primary/30 bg-primary/10 px-2.5 py-2">
      <Quote className="w-3.5 h-3.5 mt-0.5 shrink-0 text-primary" />
      <p className="flex-1 text-xs text-primary leading-relaxed line-clamp-3">{quote}</p>
      <button
        type="button"
        aria-label="移除引用"
        onClick={onClearQuote}
        className="w-4 h-4 rounded flex items-center justify-center text-primary/60 hover:text-primary"
      >
        <X className="w-3 h-3" />
      </button>
    </div>
  );
}
