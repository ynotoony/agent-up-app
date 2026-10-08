-- AgentUp Harness 本地库结构 —— 镜像 PRD《02-数据模型》(Supabase PG → SQLite)。
-- ID 均为 varchar(36) UUID；时间戳存 ISO-8601 文本。

CREATE TABLE IF NOT EXISTS projects (
  id varchar(36) PRIMARY KEY,
  name varchar(255) NOT NULL,
  description text,
  path text,
  status varchar(20) NOT NULL DEFAULT 'active',
  created_at text NOT NULL,
  updated_at text NOT NULL
);
CREATE INDEX IF NOT EXISTS projects_status_idx ON projects(status);
CREATE INDEX IF NOT EXISTS projects_created_at_idx ON projects(created_at);
-- path 唯一索引由 db.rs 迁移在建库后按「无重复」前提创建（存量库可能已有同路径重复行）。

CREATE TABLE IF NOT EXISTS project_docs (
  id varchar(36) PRIMARY KEY,
  project_id varchar(36) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  rel_path text NOT NULL,
  kind varchar(20) NOT NULL,
  title text NOT NULL,
  excerpt text,
  content text,
  content_chars integer,
  file_mtime text,
  file_missing integer NOT NULL DEFAULT 0,
  requirement_id varchar(36),
  created_at text NOT NULL,
  updated_at text NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS project_docs_uniq ON project_docs(project_id, rel_path);
CREATE INDEX IF NOT EXISTS project_docs_project_id_idx ON project_docs(project_id);

CREATE TABLE IF NOT EXISTS governance_items (
  id varchar(36) PRIMARY KEY,
  project_id varchar(36) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  kind varchar(20) NOT NULL,
  key varchar(64) NOT NULL,
  title text NOT NULL,
  body jsonb NOT NULL,
  status varchar(20) NOT NULL DEFAULT 'active',
  created_at text NOT NULL,
  updated_at text NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS governance_items_uniq ON governance_items(project_id, kind, key);
CREATE INDEX IF NOT EXISTS governance_items_project_id_idx ON governance_items(project_id);

CREATE TABLE IF NOT EXISTS requirements (
  id varchar(36) PRIMARY KEY,
  project_id varchar(36) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  content text NOT NULL,
  source_doc_path text,
  goal_summary text,
  success_criteria jsonb,
  risks jsonb,
  ambiguities jsonb,
  attachments jsonb,
  status varchar(30) NOT NULL DEFAULT 'pending',
  mode varchar(20) NOT NULL DEFAULT 'standard',
  current_step varchar(30),
  current_version integer NOT NULL DEFAULT 1,
  confirmed_at text,
  complexity varchar(4),
  complexity_reason text,
  profile jsonb,
  risk_surfaces jsonb,
  lane varchar(20),
  estimated_minutes integer,
  actual_minutes integer,
  total_tokens integer,
  created_at text NOT NULL,
  updated_at text NOT NULL
);
CREATE INDEX IF NOT EXISTS requirements_project_id_idx ON requirements(project_id);
CREATE INDEX IF NOT EXISTS requirements_status_idx ON requirements(status);
CREATE INDEX IF NOT EXISTS requirements_created_at_idx ON requirements(created_at);

CREATE TABLE IF NOT EXISTS requirement_versions (
  id varchar(36) PRIMARY KEY,
  requirement_id varchar(36) NOT NULL REFERENCES requirements(id) ON DELETE CASCADE,
  version integer NOT NULL,
  goal_summary text,
  success_criteria jsonb,
  risks jsonb,
  ambiguities jsonb,
  questions jsonb,
  source varchar(20) DEFAULT 'initial',
  user_input text,
  created_at text NOT NULL
);
CREATE INDEX IF NOT EXISTS req_versions_requirement_id_idx ON requirement_versions(requirement_id);
CREATE INDEX IF NOT EXISTS req_versions_requirement_version_idx ON requirement_versions(requirement_id, version);

CREATE TABLE IF NOT EXISTS tasks (
  id varchar(36) PRIMARY KEY,
  requirement_id varchar(36) NOT NULL REFERENCES requirements(id) ON DELETE CASCADE,
  step_type varchar(30) NOT NULL,
  title varchar(255),
  description text,
  status varchar(20) DEFAULT 'pending',
  result jsonb,
  error_message text,
  started_at text,
  completed_at text,
  tokens integer,
  created_at text NOT NULL
);
CREATE INDEX IF NOT EXISTS tasks_requirement_id_idx ON tasks(requirement_id);
CREATE INDEX IF NOT EXISTS tasks_status_idx ON tasks(status);
CREATE INDEX IF NOT EXISTS tasks_step_type_idx ON tasks(step_type);

CREATE TABLE IF NOT EXISTS decisions (
  id varchar(36) PRIMARY KEY,
  requirement_id varchar(36) NOT NULL REFERENCES requirements(id) ON DELETE CASCADE,
  task_id varchar(36) REFERENCES tasks(id) ON DELETE SET NULL,
  question text NOT NULL,
  context text,
  options jsonb NOT NULL,
  recommended varchar(255),
  status varchar(20) DEFAULT 'pending',
  user_choice jsonb,
  resolved_at text,
  created_at text NOT NULL
);
CREATE INDEX IF NOT EXISTS decisions_requirement_id_idx ON decisions(requirement_id);
CREATE INDEX IF NOT EXISTS decisions_task_id_idx ON decisions(task_id);
CREATE INDEX IF NOT EXISTS decisions_status_idx ON decisions(status);

CREATE TABLE IF NOT EXISTS artifacts (
  id varchar(36) PRIMARY KEY,
  requirement_id varchar(36) NOT NULL REFERENCES requirements(id) ON DELETE CASCADE,
  task_id varchar(36) REFERENCES tasks(id) ON DELETE SET NULL,
  type varchar(30) NOT NULL,
  title varchar(255),
  content text,
  url text,
  metadata jsonb,
  stage varchar(30),
  version integer DEFAULT 1,
  created_at text NOT NULL
);
CREATE INDEX IF NOT EXISTS artifacts_requirement_id_idx ON artifacts(requirement_id);
CREATE INDEX IF NOT EXISTS artifacts_task_id_idx ON artifacts(task_id);
CREATE INDEX IF NOT EXISTS artifacts_type_idx ON artifacts(type);

-- agents 表已随 Agent 目录机制废弃（2026-09-30 执行编排重构：阶段 × 运行时 × 角色文件）。
-- 存量库中的旧表无害保留，新建库不再创建。

-- 理解队列：所有初始理解入口统一入队，单例泵串行消化。
-- 行即锁：requirement_id UNIQUE 防重复入队；状态 queued → running，完成后删行（失败语义落在需求状态与日志上）。
CREATE TABLE IF NOT EXISTS understanding_queue (
  id varchar(36) PRIMARY KEY,
  requirement_id varchar(36) NOT NULL UNIQUE REFERENCES requirements(id) ON DELETE CASCADE,
  status varchar(20) NOT NULL DEFAULT 'queued',
  attempts integer NOT NULL DEFAULT 0,
  created_at text NOT NULL,
  updated_at text NOT NULL
);
CREATE INDEX IF NOT EXISTS understanding_queue_status_idx ON understanding_queue(status, created_at);

CREATE TABLE IF NOT EXISTS app_settings (
  key varchar(64) PRIMARY KEY,
  value jsonb,
  updated_at text NOT NULL
);

CREATE TABLE IF NOT EXISTS execution_logs (
  id varchar(36) PRIMARY KEY,
  requirement_id varchar(36) NOT NULL REFERENCES requirements(id) ON DELETE CASCADE,
  task_id varchar(36) REFERENCES tasks(id) ON DELETE SET NULL,
  step varchar(50) NOT NULL,
  level varchar(10) DEFAULT 'info',
  message text NOT NULL,
  details jsonb,
  created_at text NOT NULL
);
CREATE INDEX IF NOT EXISTS logs_requirement_id_idx ON execution_logs(requirement_id);
CREATE INDEX IF NOT EXISTS logs_task_id_idx ON execution_logs(task_id);
