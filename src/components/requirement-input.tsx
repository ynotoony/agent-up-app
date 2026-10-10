import { ArrowUp, Sparkles } from 'lucide-react';
import { useRef, useState, type ClipboardEvent as ReactClipboardEvent, type KeyboardEvent as ReactKeyboardEvent } from 'react';
import type { PendingAttachment, Project } from '../../shared/types';
import { api, errorMessage } from '@/lib/api';
import { arrayBufferToBase64, cn } from '@/lib/utils';
import { AttachmentField, AttachmentList, clipsToImageItems, revokeAttachmentUrls, type AttachmentDraft } from './attachment-field';
import { Button } from './ui/button';
import { Select, SelectChevron } from './ui/select';

// 一句话需求录入：textarea + 附件采集（加号/粘贴截图）+ 归属项目 + 主 CTA「开始理解」。
export function RequirementInput({
  projects,
  defaultProjectId,
  lockProject,
  onCreated,
}: {
  projects: Project[];
  defaultProjectId?: string;
  lockProject?: boolean;
  onCreated: (requirementId: string) => void;
}) {
  const [content, setContent] = useState('');
  const [projectId, setProjectId] = useState(defaultProjectId ?? projects[0]?.id ?? '');
  const [attachments, setAttachments] = useState<AttachmentDraft[]>([]);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const submitLock = useRef(false);

  const addAttachments = (items: AttachmentDraft[]) => setAttachments((prev) => [...prev, ...items]);
  const removeAttachment = (id: string) => setAttachments((prev) => prev.filter((a) => a.id !== id));

  const handlePaste = (e: ReactClipboardEvent<HTMLTextAreaElement>) => {
    const images = clipsToImageItems(e);
    if (images.length > 0) {
      e.preventDefault();
      addAttachments(images);
    }
  };

  const handleKeyDown = (e: ReactKeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      void submit();
    }
  };

  const submit = async () => {
    if (submitLock.current) return;
    const trimmed = content.trim();
    if (trimmed.length < 4) {
      setError('需求内容至少 4 个字');
      return;
    }
    if (!projectId) {
      setError('请选择归属项目');
      return;
    }
    submitLock.current = true;
    setSubmitting(true);
    setError('');
    try {
      const payload: PendingAttachment[] = [];
      for (const draft of attachments) {
        const data = arrayBufferToBase64(await draft.file.arrayBuffer());
        payload.push({
          name: draft.name,
          type: draft.type,
          size: draft.size,
          kind: draft.kind,
          relativePath: draft.relativePath,
          data,
        });
      }
      const result = await api.requirements.create({
        project_id: projectId,
        content: trimmed,
        attachments: payload,
      });
      revokeAttachmentUrls(attachments);
      setAttachments([]);
      setContent('');
      onCreated(result.requirement.id);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      submitLock.current = false;
      setSubmitting(false);
    }
  };

  return (
    // 紧凑命令条：项目选择器（未锁定时）+ 输入 + 提交全部一行；单行起步自动增高
    <section className="bg-card border border-border rounded-xl px-3 py-2.5">
      <div className="flex items-start gap-2">
        <Sparkles className="w-4 h-4 text-primary shrink-0 mt-2.5" />
        {!lockProject && (
          <div className="relative w-36 shrink-0 mt-0.5">
            <Select aria-label="归属项目" value={projectId} onChange={(e) => setProjectId(e.target.value)} className="h-9 text-xs">
              {projects.length === 0 && <option value="">暂无项目</option>}
              {projects.map((project) => (
                <option key={project.id} value={project.id}>
                  {project.name}
                </option>
              ))}
            </Select>
            <SelectChevron />
          </div>
        )}
        <div className="relative flex-1 min-w-0">
          <textarea
            ref={textareaRef}
            value={content}
            onChange={(e) => {
              setContent(e.target.value);
              const el = e.target;
              el.style.height = 'auto';
              el.style.height = `${Math.min(el.scrollHeight, 120)}px`;
            }}
            onPaste={handlePaste}
            onKeyDown={handleKeyDown}
            rows={1}
            placeholder="一句话需求：描述你想做成的事…（Enter 提交 · Shift+Enter 换行 · Ctrl+V 粘贴截图）"
            className="w-full resize-none bg-background border border-border rounded-lg pl-3 pr-10 py-2 text-sm leading-relaxed placeholder:text-muted-foreground/50 focus:outline-none focus:border-primary/50 focus:ring-1 focus:ring-primary/20 transition-all"
          />
          <AttachmentField variant="inline" onAdd={addAttachments} />
        </div>
        <Button size="sm" className="mt-0.5 h-8 px-3 shrink-0" onClick={() => void submit()} loading={submitting} disabled={content.trim().length < 4}>
          {submitting ? '理解中…' : '开始理解'}
          {!submitting && <ArrowUp className="w-3.5 h-3.5" />}
        </Button>
      </div>
      {attachments.length > 0 && <AttachmentList items={attachments} onRemove={removeAttachment} />}
      {error && <p className="text-xs text-destructive">{error}</p>}
    </section>
  );
}
