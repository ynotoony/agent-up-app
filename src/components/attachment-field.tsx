import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import { FileImage, File as FileIcon, FolderClosed, Plus, X } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import type { ClipboardEvent as ReactClipboardEvent, ReactNode } from 'react';
import { cn, formatBytes } from '@/lib/utils';

// 附件采集统一组件（DESIGN 3.10）：
// - AttachmentField 只负责采集（加号下拉：添加文件/添加文件夹 + 两个 hidden input）。
// - 截图仅通过输入区 Ctrl+V 粘贴捕获（onPaste → clipsToImageItems），不设上传按钮。
// - AttachmentList 渲染 chip + 图片灯箱预览。

export interface AttachmentDraft {
  id: string;
  name: string;
  type: string;
  size: number;
  kind: 'file' | 'folder' | 'image';
  relativePath?: string;
  file: File;
  previewUrl?: string;
}

export function clipsToImageItems(e: ReactClipboardEvent<HTMLElement>): AttachmentDraft[] {
  const items: AttachmentDraft[] = [];
  const clips = e.clipboardData?.items ?? [];
  for (const clip of clips) {
    if (clip.kind === 'file' && clip.type.startsWith('image/')) {
      const file = clip.getAsFile();
      if (file) {
        items.push({
          id: crypto.randomUUID(),
          name: file.name || `截图_${Date.now()}.png`,
          type: file.type,
          size: file.size,
          kind: 'image',
          file,
          previewUrl: URL.createObjectURL(file),
        });
      }
    }
  }
  return items;
}

export function revokeAttachmentUrls(items: AttachmentDraft[]): void {
  for (const item of items) {
    if (item.previewUrl) URL.revokeObjectURL(item.previewUrl);
  }
}

const KIND_BADGE: Record<AttachmentDraft['kind'], { label: string; className: string }> = {
  image: { label: '截图', className: 'text-blue-600 dark:text-blue-400 bg-blue-600/10 dark:bg-blue-400/10' },
  folder: { label: '文件夹', className: 'text-amber-600 dark:text-amber-400 bg-amber-600/10 dark:bg-amber-400/10' },
  file: { label: '文件', className: 'text-muted-foreground bg-muted' },
};

export function AttachmentField({
  variant,
  onAdd,
  showCount,
  count,
}: {
  variant: 'inline' | 'toolbar';
  onAdd: (items: AttachmentDraft[]) => void;
  showCount?: boolean;
  count?: number;
}) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const folderInputRef = useRef<HTMLInputElement>(null);

  const collect = useCallback(
    (files: FileList | null, defaultKind: 'file' | 'folder') => {
      if (!files || files.length === 0) return;
      const drafts: AttachmentDraft[] = [];
      for (const file of Array.from(files)) {
        const relativePath = (file as File & { webkitRelativePath?: string }).webkitRelativePath;
        const kind = file.type.startsWith('image/') ? 'image' : relativePath ? 'folder' : defaultKind;
        drafts.push({
          id: crypto.randomUUID(),
          name: file.name,
          type: file.type || 'application/octet-stream',
          size: file.size,
          kind,
          relativePath: relativePath || file.name,
          file,
          previewUrl: kind === 'image' ? URL.createObjectURL(file) : undefined,
        });
      }
      onAdd(drafts);
    },
    [onAdd]
  );

  const plusButton = (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <button
          type="button"
          aria-label="添加附件"
          className={cn(
            'no-drag inline-flex items-center justify-center w-7 h-7 rounded-md bg-muted text-muted-foreground hover:text-foreground hover:bg-accent transition-colors outline-none focus-visible:ring-2 focus-visible:ring-primary/30',
            variant === 'inline' && 'absolute right-1.5 top-1/2 -translate-y-1/2'
          )}
        >
          <Plus className="w-4 h-4" />
        </button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          side="top"
          align="end"
          sideOffset={8}
          className="z-50 min-w-32 rounded-lg border border-border bg-popover p-1 shadow-lg animate-in"
        >
          <DropdownMenu.Item
            className="flex items-center gap-2 px-2.5 py-1.5 rounded-md text-xs text-foreground outline-none cursor-pointer data-[highlighted]:bg-accent"
            onSelect={() => fileInputRef.current?.click()}
          >
            <FileIcon className="w-3.5 h-3.5 text-muted-foreground" />
            添加文件
          </DropdownMenu.Item>
          <DropdownMenu.Item
            className="flex items-center gap-2 px-2.5 py-1.5 rounded-md text-xs text-foreground outline-none cursor-pointer data-[highlighted]:bg-accent"
            onSelect={() => folderInputRef.current?.click()}
          >
            <FolderClosed className="w-3.5 h-3.5 text-muted-foreground" />
            添加文件夹
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
      <input
        ref={fileInputRef}
        type="file"
        multiple
        className="hidden"
        onChange={(e) => {
          collect(e.target.files, 'file');
          e.target.value = '';
        }}
      />
      {/* 文件夹采集依赖 webkitdirectory */}
      <input
        ref={folderInputRef}
        type="file"
        multiple
        className="hidden"
        // @ts-expect-error webkitdirectory 为非标准属性
        webkitdirectory="true"
        directory=""
        onChange={(e) => {
          collect(e.target.files, 'folder');
          e.target.value = '';
        }}
      />
    </DropdownMenu.Root>
  );

  if (variant === 'inline') return plusButton;
  return (
    <div className="flex items-center gap-2">
      {plusButton}
      {showCount && count !== undefined && count > 0 && (
        <span className="text-xs text-muted-foreground">{count} 个附件</span>
      )}
    </div>
  );
}

export function AttachmentList({
  items,
  onRemove,
  className,
}: {
  items: AttachmentDraft[];
  onRemove?: (id: string) => void;
  className?: string;
}) {
  const [lightbox, setLightbox] = useState<AttachmentDraft | null>(null);

  useEffect(() => {
    if (!lightbox) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setLightbox(null);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [lightbox]);

  if (items.length === 0) return null;
  return (
    <div className={cn('flex flex-wrap gap-2', className)}>
      {items.map((item) => (
        <AttachmentChip key={item.id} item={item} onRemove={onRemove} onPreview={setLightbox} />
      ))}
      {lightbox && (
        <div
          className="fixed inset-0 z-[100] bg-black/85 flex items-center justify-center"
          onClick={() => setLightbox(null)}
        >
          <button
            type="button"
            aria-label="关闭预览"
            className="absolute top-5 right-5 w-8 h-8 rounded-md text-white/80 hover:text-white hover:bg-white/10 flex items-center justify-center"
            onClick={() => setLightbox(null)}
          >
            <X className="w-4 h-4" />
          </button>
          <img src={lightbox.previewUrl} alt={lightbox.name} className="max-w-[80vw] max-h-[80vh] rounded-lg" />
        </div>
      )}
    </div>
  );
}

function AttachmentChip({
  item,
  onRemove,
  onPreview,
}: {
  item: AttachmentDraft;
  onRemove?: (id: string) => void;
  onPreview: (item: AttachmentDraft) => void;
}) {
  const badge = KIND_BADGE[item.kind];
  return (
    <div className="group inline-flex items-center gap-1.5 h-9 pl-1.5 pr-1 rounded-lg border border-border bg-card text-xs">
      {item.kind === 'image' && item.previewUrl ? (
        <button type="button" aria-label={`预览 ${item.name}`} onClick={() => onPreview(item)} className="shrink-0">
          <img src={item.previewUrl} alt={item.name} className="w-8 h-8 rounded object-cover" />
        </button>
      ) : (
        <span className="inline-flex items-center justify-center w-7 h-7 rounded bg-muted text-muted-foreground">
          {item.kind === 'folder' ? <FolderClosed className="w-3.5 h-3.5" /> : <FileIcon className="w-3.5 h-3.5" />}
        </span>
      )}
      <span className="inline-flex items-center gap-1.5 max-w-44">
        <span className={cn('px-1.5 py-0.5 rounded text-[11px]', badge.className)}>{badge.label}</span>
        <span className="truncate text-foreground/80" title={item.relativePath ?? item.name}>
          {item.name}
        </span>
        <span className="text-muted-foreground/50 whitespace-nowrap">{formatBytes(item.size)}</span>
      </span>
      {onRemove && (
        <button
          type="button"
          aria-label={`移除 ${item.name}`}
          onClick={() => onRemove(item.id)}
          className="w-5 h-5 rounded flex items-center justify-center text-muted-foreground/60 hover:text-destructive hover:bg-destructive/10 transition-colors"
        >
          <X className="w-3 h-3" />
        </button>
      )}
    </div>
  );
}

// 渲染已持久化附件（详情页「当前目标」区参考附件 chips）。children 为自定义预览态。
export function AttachmentBadge({ kind, children }: { kind: 'file' | 'folder' | 'image'; children: ReactNode }) {
  const badge = KIND_BADGE[kind];
  return <span className={cn('inline-flex items-center px-1.5 py-0.5 rounded text-[11px]', badge.className)}>{children}</span>;
}

export const AttachmentKindIcon = {
  image: FileImage,
  folder: FolderClosed,
  file: FileIcon,
};
