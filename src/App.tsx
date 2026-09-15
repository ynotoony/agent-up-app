import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type CommandName =
  | "scan_project"
  | "preview_initialize"
  | "initialize_project"
  | "create_request"
  | "load_project";

type CommandResult = {
  ok: boolean;
  command: CommandName;
  data?: unknown;
  error?: { code: string; message: string; details?: Record<string, unknown> };
};

const EVENTS = [
  "project.scan_completed",
  "project.initialized",
  "request.created",
  "request.rehydrated",
] as const;

export default function App() {
  const [projectPath, setProjectPath] = useState("");
  const [projectId, setProjectId] = useState("");
  const [fingerprint, setFingerprint] = useState("");
  const [token, setToken] = useState("");
  const [requestId, setRequestId] = useState("req-demo-1");
  const [title, setTitle] = useState("Draft request");
  const [body, setBody] = useState("Created from the M0 runtime slice.");
  const [output, setOutput] = useState("Ready.");
  const [events, setEvents] = useState<string[]>([]);

  useEffect(() => {
    let cancelled = false;
    const unlisten: Array<() => void> = [];
    void (async () => {
      for (const name of EVENTS) {
        const fn = await listen(name, (event) => {
          setEvents((current) => [`${name}: ${JSON.stringify(event.payload)}`, ...current].slice(0, 20));
        });
        if (cancelled) {
          fn();
        } else {
          unlisten.push(fn);
        }
      }
    })();
    return () => {
      cancelled = true;
      unlisten.forEach((fn) => fn());
    };
  }, []);

  async function run(command: CommandName, payload: Record<string, unknown>) {
    const result = await invoke<CommandResult>(command, payload);
    setOutput(JSON.stringify(result, null, 2));
    if (result.ok && result.data && typeof result.data === "object") {
      const data = result.data as Record<string, unknown>;
      if (typeof data.project_id === "string") setProjectId(data.project_id);
      if (typeof data.root_fingerprint === "string") setFingerprint(data.root_fingerprint);
      if (typeof data.confirmation_token === "string") setToken(data.confirmation_token);
    }
    return result;
  }

  return (
    <main>
      <h1>AgentUp</h1>
      <p>Select a project directory, preview with zero writes, confirm initialization, then create a draft request.</p>
      <label htmlFor="project-path">Project directory</label>
      <input
        id="project-path"
        value={projectPath}
        onChange={(event) => setProjectPath(event.target.value)}
        placeholder="/absolute/path/to/project"
      />
      <div className="row">
        <button type="button" onClick={() => void run("scan_project", { project_path: projectPath })}>
          scan_project
        </button>
        <button
          type="button"
          onClick={() =>
            void run("preview_initialize", {
              project_id: projectId,
              project_path: projectPath,
              expected_root_fingerprint: fingerprint,
            })
          }
        >
          preview_initialize
        </button>
        <button
          type="button"
          onClick={() =>
            void run("initialize_project", {
              project_id: projectId,
              project_path: projectPath,
              confirmation_token: token,
              expected_root_fingerprint: fingerprint,
            })
          }
        >
          initialize_project
        </button>
        <button
          type="button"
          onClick={() => void run("load_project", { project_path: projectPath, project_id: projectId || undefined })}
        >
          load_project
        </button>
      </div>
      <label htmlFor="request-id">request_id</label>
      <input id="request-id" value={requestId} onChange={(event) => setRequestId(event.target.value)} />
      <label htmlFor="title">title</label>
      <input id="title" value={title} onChange={(event) => setTitle(event.target.value)} />
      <label htmlFor="body">body</label>
      <textarea id="body" rows={4} value={body} onChange={(event) => setBody(event.target.value)} />
      <button
        type="button"
        onClick={() =>
          void run("create_request", {
            project_id: projectId,
            request_id: requestId,
            content: { lifecycle: "draft", title, body },
            metadata: {},
            expected_revision: 0,
          })
        }
      >
        create_request
      </button>
      <div className="row">
        <button type="button" onClick={() => void invoke("post_discussion", { projectId, requestId, body })}>
          post_discussion
        </button>
        <button type="button" onClick={() => void invoke("load_board", { projectId, requestId })}>
          load_board
        </button>
        <button type="button" onClick={() => void invoke("load_result_history", { projectId, requestId })}>
          load_result_history
        </button>
        <button type="button" onClick={() => void invoke("export_diagnostics", { projectId })}>
          export_diagnostics
        </button>
      </div>
      <h2>Result</h2>
      <pre>{output}</pre>
      <h2>Events</h2>
      <pre>{events.join("\n") || "(none)"}</pre>
    </main>
  );
}
