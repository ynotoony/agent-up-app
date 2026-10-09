import { useNavigate } from 'react-router-dom';
import { WorkspaceView } from '@/components/workspace-view';

// 工作台（首页）：/。视口锁定：上（标题 + 一句话需求）固定，下方看板横向泳道、列内纵向滚动。
export default function WorkspacePage() {
  const navigate = useNavigate();
  return (
    <main className="h-screen overflow-y-auto scrollbar-thin lg:overflow-hidden">
      <div className="drag-region h-2 shrink-0" />
      <div className="max-w-7xl mx-auto px-6 py-5 lg:h-full lg:pb-5 flex flex-col gap-4 min-h-0">
        <header data-tauri-drag-region className="shrink-0">
          <h1 className="text-lg font-semibold tracking-tight">工作台</h1>
          <p className="mt-0.5 text-xs text-muted-foreground">从一个目标开始：接入项目 → 规划任务 → 确认计划</p>
        </header>
        <div className="flex-1 min-h-0">
          <WorkspaceView onCreated={(requirementId) => navigate(`/requirement/${requirementId}`)} />
        </div>
      </div>
    </main>
  );
}
