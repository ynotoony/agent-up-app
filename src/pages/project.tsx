// Input: The project identifier from the current router path.
// Output: A task workbench scoped to that project.
// Pos: Project task route adapter.
import { useParams } from 'react-router-dom';
import { TaskWorkbench } from '@/components/task-workbench';

export default function ProjectPage() {
  const { id } = useParams();
  return <TaskWorkbench projectId={id} />;
}
