import { invoke } from '@tauri-apps/api/core';
import type {
  CreateRequirementResult,
  Decision,
  DecisionOption,
  ExportOutcome,
  GovernanceItem,
  GovernanceTicketsResult,
  InitProjectResult,
  InitReport,
  OrchestrationSettings,
  PendingAttachment,
  Project,
  ProjectDashboard,
  ProjectDoc,
  RequirementDetail,
  Requirement,
  RoleInfo,
  Task,
  WorkspaceData,
} from '../../shared/types';

// 渲染层唯一数据入口 —— 通过 Tauri invoke 调用主进程命令。
// 命令层与 PRD《03-后端API契约》路由 1:1 映射，错误为 { code, message }（400/404/409 语义一致）。

export interface ApiErrorShape {
  code: number;
  message: string;
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    if (err && typeof err === 'object' && 'code' in err && 'message' in err) {
      throw Object.assign(new Error(String((err as ApiErrorShape).message)), {
        code: (err as ApiErrorShape).code,
      });
    }
    throw err instanceof Error ? err : new Error(String(err));
  }
}

export interface ApiClient {
  workspace: {
    get(): Promise<WorkspaceData>;
  };
  projects: {
    list(): Promise<Project[]>;
    create(body: { name: string; description?: string | null }): Promise<Project>;
    init(path: string): Promise<InitProjectResult>;
    reinit(id: string): Promise<InitReport>;
    docs(id: string): Promise<ProjectDoc[]>;
    docRead(docId: string): Promise<{ id: string; rel_path: string; kind: string; title: string; content: string }>;
    docToRequirement(projectId: string, docId: string): Promise<Requirement>;
    governance(id: string, kind?: string): Promise<GovernanceItem[]>;
    initializing(id: string): Promise<Requirement[]>;
    governanceAnswer(itemId: string, answer: string): Promise<GovernanceItem>;
    governanceComplete(id: string): Promise<Requirement>;
    export(id: string, kind: 'agentup-files' | 'json-snapshot', target?: string): Promise<ExportOutcome>;
    tickets(id: string, loadBodies?: boolean): Promise<GovernanceTicketsResult>;
    get(id: string): Promise<ProjectDashboard>;
    update(id: string, updates: { name?: string; description?: string | null; status?: string }): Promise<Project>;
    remove(id: string): Promise<{ deleted: boolean }>;
  };
  requirements: {
    create(body: { project_id: string; content: string; attachments?: PendingAttachment[]; estimated_minutes?: number }): Promise<CreateRequirementResult>;
    get(id: string): Promise<RequirementDetail>;
    listTasks(id: string): Promise<Task[]>;
    listDecisions(id: string): Promise<Decision[]>;
    createDecision(body: {
      requirement_id: string;
      question: string;
      context?: string;
      options: DecisionOption[];
      recommended?: string;
    }): Promise<Decision>;
    confirm(id: string): Promise<{ requirement: Requirement; planTask: Task; pendingTasks: Task[] }>;
    retry(id: string): Promise<{ requirement: Requirement; route: 'pipeline' | 'understanding' }>;
    cancel(id: string): Promise<{ cancelled: boolean }>;
    retryInitializing(id: string): Promise<Requirement>;
    retryInitializingBatch(ids: string[]): Promise<{ queued: number }>;
    reunderstand(id: string, body: { feedback?: string; answering?: boolean; manual?: boolean; quote?: string; attachments?: PendingAttachment[] }): Promise<{ requirement: Requirement; version: { version: number }; has_questions: boolean }>;
    iterate(id: string, body: { feedback: string; attachments?: PendingAttachment[] }): Promise<{ requirement: Requirement; version: { version: number } }>;
  };
  decisions: {
    resolve(id: string, choice: DecisionOption): Promise<Decision>;
  };
  runtimes: {
    list(): Promise<RuntimeInfo[]>;
    getSelected(): Promise<string>;
    setSelected(id: string): Promise<string>;
    check(id: string): Promise<{ ok: boolean; runtime: string; reply?: string; latency_ms?: number; error?: string }>;
    openApp(id: string): Promise<{ opened: boolean; app: string }>;
  };
  orchestration: {
    get(): Promise<OrchestrationSettings>;
    save(body: {
      default_runtime?: string | null;
      stage_runtimes?: Partial<Record<string, string | null>>;
    }): Promise<OrchestrationSettings>;
  };
  roles: {
    list(): Promise<RoleInfo[]>;
    get(stage: string): Promise<RoleInfo>;
    save(stage: string, content: string): Promise<RoleInfo>;
    reset(stage: string): Promise<RoleInfo>;
  };
}

export interface RuntimeInfo {
  id: string;
  name: string;
  description: string;
  available: boolean;
  path: string | null;
  version: string | null;
  /** CLI 配置文件里的当前模型（读不到为 null） */
  model: string | null;
  read_only_analysis: boolean;
}

export const api: ApiClient = {
  workspace: {
    get: () => call<WorkspaceData>('workspace_get'),
  },
  projects: {
    list: () => call<Project[]>('projects_list'),
    create: (body) => call<Project>('projects_create', { name: body.name, description: body.description ?? null }),
    init: (path) => call<InitProjectResult>('projects_init', { path }),
    reinit: (id) => call<InitReport>('projects_reinit', { id }),
    docs: (id) => call<ProjectDoc[]>('projects_docs', { id }),
    docRead: (docId) => call('projects_doc_read', { docId }),
    docToRequirement: (projectId, docId) => call<Requirement>('projects_doc_to_requirement', { projectId, docId }),
    governance: (id, kind) => call<GovernanceItem[]>('projects_governance', { id, kind: kind ?? null }),
    initializing: (id) => call<Requirement[]>('projects_initializing', { id }),
    governanceAnswer: (itemId, answer) => call<GovernanceItem>('projects_governance_answer', { itemId, answer }),
    governanceComplete: (id) => call<Requirement>('projects_governance_complete', { id }),
    export: (id, kind, target) => call<ExportOutcome>('projects_export', { id, kind, target: target ?? null }),
    tickets: (id, loadBodies) => call<GovernanceTicketsResult>('projects_tickets', { id, loadBodies: loadBodies ?? false }),
    get: (id) => call<ProjectDashboard>('projects_get', { id }),
    update: (id, updates) =>
      call<Project>('projects_update', {
        id,
        updates: {
          name: updates.name,
          description: updates.description === undefined ? undefined : updates.description,
          status: updates.status,
        },
      }),
    remove: (id) => call('projects_delete', { id }),
  },
  requirements: {
    create: (body) => call('requirements_create', { input: body }),
    get: (id) => call('requirements_get', { id }),
    listTasks: (id) => call<Task[]>('requirements_list_tasks', { id }),
    listDecisions: (id) => call<Decision[]>('requirements_list_decisions', { id }),
    createDecision: (body) => call('requirements_create_decision', { input: body }),
    confirm: (id) => call('requirements_confirm', { id }),
    retry: (id) => call('requirements_retry', { id }),
    cancel: (id) => call('requirements_cancel', { id }),
    retryInitializing: (id) => call<Requirement>('requirements_retry_initializing', { id }),
    retryInitializingBatch: (ids) => call('requirements_retry_initializing_batch', { ids }),
    reunderstand: (id, body) => call('requirements_reunderstand', { id, input: body }),
    iterate: (id, body) => call('requirements_iterate', { id, input: body }),
  },
  decisions: {
    resolve: (id, choice) => call('decisions_resolve', { input: { id, choice } }),
  },
  runtimes: {
    list: () => call<RuntimeInfo[]>('runtimes_list'),
    getSelected: () => call<string>('settings_get_runtime'),
    setSelected: (id: string) => call<string>('settings_set_runtime', { id }),
    check: (id: string) => call('runtimes_check', { id }),
    openApp: (id: string) => call('runtimes_open_app', { id }),
  },
  orchestration: {
    get: () => call('settings_get_orchestration'),
    save: (body) => call('settings_save_orchestration', { input: body }),
  },
  roles: {
    list: () => call<RoleInfo[]>('roles_list'),
    get: (stage: string) => call<RoleInfo>('roles_get', { stage }),
    save: (stage: string, content: string) => call<RoleInfo>('roles_save', { stage, content }),
    reset: (stage: string) => call<RoleInfo>('roles_reset', { stage }),
  },
};

export function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
