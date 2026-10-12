<!-- Input: Approved AgentUp product route, implemented workflows, and verified local commands. -->
<!-- Output: Product orientation, user workflow, data boundaries, and developer commands. -->
<!-- Pos: Repository entry point; keep aligned with the task board and project onboarding implementation. -->

# AgentUp

AgentUp is a local-first workbench for real tasks across software projects. It reads the task index already owned by each repository, places those tasks on one board, and opens a visible interactive Agent session with the selected task and repository context.

**Repository files remain authoritative. AgentUp indexes and displays them; it does not copy tasks into a competing database, invent sample tasks, or mark a task complete because a terminal opened.** The user reviews Agent changes and updates the project task state when the work is actually ready.

## Workflow

1. Connect an empty directory, an existing Git repository, or a repository that already has a task source.
2. Review the exact setup writes before connecting. Existing files and history are preserved. An empty task index is created only when no supported index exists; no example tasks are added.
3. Scan the board across projects, open a task to inspect its original goal, scope, dependencies, acceptance conditions, and source file.
4. In a supported formal-ticket project, describe a result in one sentence. A read-only Codex session drafts the task; review the ticket fields and confirm before it is written. Projects missing their ticket README, ledger, bundled tools, or schema stay read-only and show the specific requirement.
5. Start an interactive session in Codex CLI, OpenCode, or ZCode. AgentUp passes the repository path and task context. For ZCode, paste the copied context in its visible terminal.
6. Review the Agent's work in the repository. Refresh the board to read the current task status from project files.

The board reads `facts/requirements/tickets/index.json`, `docs/issues/index.json`, or `docs/project-management-mvp/issues/index.json`, in that order. Task bodies are loaded on demand for inspection and dispatch. Invalid rows, unknown statuses, unreadable sources, and unresolved dependencies remain visible; they are never silently treated as ready or done. Task creation currently writes only to the formal `facts/requirements/tickets/index.json` source, through the packaged ticket-ops writer; legacy indexes remain read-only.

## Project Setup

Choose a local directory from **Connect project**. AgentUp previews the directory and enumerates each planned write before confirmation.

- Existing Git repositories keep their Git history and supported task files.
- A non-Git directory is initialized as a local Git repository only after confirmation.
- If no supported task index exists, AgentUp offers to create an empty `facts/requirements/tickets/index.json`.
- New ticket creation is available only after the project's formal ticket README and change ledger exist and the bundled writer requirements are present. An empty index alone does not make the project writable.
- A local `.agentup-app/project.json` profile and local project registration provide navigation without replacing the repository's task source.

## Source Map

- `src/components/task-workbench.tsx` renders the multi-project task board, task detail, task creation, and Agent handoff UI. `src/lib/workbench.tsx` loads and groups project tasks; `src/workbench.css` styles the responsive workbench.
- `src/pages/project-records.tsx` keeps legacy goals, deliveries, and requirement history readable.
- `src-tauri/src/workspace_onboarding.rs` previews project setup writes and applies them after confirmation. `src-tauri/src/ticket_handoff.rs` validates a ready task and opens an interactive local Agent session.
- `src-tauri/resources/agent-up-runtime/` contains the packaged ticket writer, progress generator, schemas, and license used by formal task creation.

## Agents

AgentUp opens interactive local sessions using installed Codex CLI, OpenCode, or ZCode runtimes. The default Agent can be selected in **Agent settings**; each task can use a different installed Agent before dispatch. Dispatch is a handoff, not a background execution receipt: the terminal may still require login, confirmation, or a pasted prompt.

AgentUp does not automatically commit, merge, push, publish, or change repository task statuses when an Agent session starts. The separate legacy goal-delivery workflow remains available from project records for existing local data, but it is not the task board's source of truth.

## Development

Requirements: macOS, Node.js, pnpm, Rust, and Tauri system dependencies.

```bash
pnpm install
pnpm dev
pnpm typecheck
cargo test --manifest-path src-tauri/Cargo.toml --tests
pnpm build
```

`pnpm dev` opens the Tauri desktop application with the Vite development server. User-facing project files are not modified by development or tests unless a workflow explicitly asks the user to confirm a project setup write.

## Release

Release packaging is handled by `scripts/release.sh <version>`. The script verifies version parity, tests, builds an app and DMG, checks the DMG contents, then creates a version tag and GitHub release. Push and release are explicit release actions, not part of ordinary task dispatch.
