import { useState } from 'react';
import { Code2, FileCode2, FileText, Globe, PackageOpen, Settings2 } from 'lucide-react';
import type { Artifact, ArtifactType } from '../../shared/types';
import { cn } from '@/lib/utils';
import { PanelHeader } from './understanding-panel';
import { EmptyState } from './ui/empty';
import { Badge } from './ui/badge';

const TYPE_CONFIG: Record<ArtifactType, { label: string; icon: typeof Code2; className: string }> = {
  code: { label: '代码', icon: Code2, className: 'text-blue-600 dark:text-blue-400 bg-blue-600/10 dark:bg-blue-400/10' },
  document: { label: '文档', icon: FileText, className: 'text-primary bg-primary/10' },
  preview: { label: '预览', icon: Globe, className: 'text-cyan-600 dark:text-cyan-400 bg-cyan-600/10 dark:bg-cyan-400/10' },
  config: { label: '配置', icon: Settings2, className: 'text-amber-600 dark:text-amber-400 bg-amber-600/10 dark:bg-amber-400/10' },
  markdown: { label: '报告', icon: FileCode2, className: 'text-emerald-600 dark:text-emerald-400 bg-emerald-600/10 dark:bg-emerald-400/10' },
};

// 产物列表：类型徽章 + 标题 + 可展开内容。
export function ArtifactList({ artifacts }: { artifacts: Artifact[] }) {
  return (
    <div className="bg-card border border-border rounded-xl overflow-hidden">
      <PanelHeader title={`产物（${artifacts.length}）`} icon={<PackageOpen className="w-3.5 h-3.5" />} />
      {artifacts.length === 0 ? (
        <EmptyState className="py-12" icon={<PackageOpen className="w-10 h-10" />} title="还没有产物" description="方案与实施完成后会在这里沉淀" />
      ) : (
        <div className="p-4 space-y-3">
          {[...artifacts].reverse().map((artifact) => (
            <ArtifactCard key={artifact.id} artifact={artifact} />
          ))}
        </div>
      )}
    </div>
  );
}

function ArtifactCard({ artifact }: { artifact: Artifact }) {
  const [open, setOpen] = useState(false);
  const config = TYPE_CONFIG[artifact.type] ?? TYPE_CONFIG.document;
  const Icon = config.icon;
  return (
    <div className="rounded-lg border border-border overflow-hidden">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="w-full flex items-center gap-2.5 px-3.5 py-2.5 text-left hover:bg-accent/40 transition-colors"
      >
        <span className={cn('inline-flex items-center justify-center w-7 h-7 rounded-md shrink-0', config.className)}>
          <Icon className="w-3.5 h-3.5" />
        </span>
        <span className="min-w-0 flex-1">
          <span className="block text-sm text-foreground truncate">{artifact.title}</span>
          <span className="block text-[11px] text-muted-foreground/60">
            {config.label}
            {artifact.version > 1 ? ` · v${artifact.version}` : ''}
            {' · '}
            {new Date(artifact.created_at).toLocaleString('zh-CN')}
          </span>
        </span>
        <span className="text-[11px] text-muted-foreground/60 shrink-0">{open ? '收起' : '展开'}</span>
      </button>
      {open && artifact.content && (
        <div className="border-t border-border bg-muted/30 px-3.5 py-3">
          <pre className="text-xs leading-relaxed text-foreground/85 whitespace-pre-wrap break-words font-sans max-h-80 overflow-y-auto scrollbar-thin">
            {artifact.content}
          </pre>
        </div>
      )}
    </div>
  );
}

export function ArtifactTypeBadge({ type }: { type: ArtifactType }) {
  const config = TYPE_CONFIG[type] ?? TYPE_CONFIG.document;
  return <Badge className={config.className}>{config.label}</Badge>;
}
