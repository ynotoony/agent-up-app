// Input: Workspace route state and the shared task workbench.
// Output: Workspace and attention-filtered route views.
// Pos: Renderer page entry for the root workspace routes.
import { TaskWorkbench } from '@/components/task-workbench';

export default function WorkspacePage() { return <TaskWorkbench />; }
export function AttentionPage() { return <TaskWorkbench attention />; }
