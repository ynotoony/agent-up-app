#!/usr/bin/env node
// Real command output has to survive the contract that claims to describe it.
//
// Why this file exists: `schemas/` is the machine contract for command results,
// but nothing ever fed real output to it - `m0_schema_parse.py` only calls
// `json.load`. So `scan` emitted `walk_ms` / `fingerprint_ms` while its schema said
// `additionalProperties: false` over five keys, and that drift survived two
// independent reviews plus a full `npm test`
// (docs/issues/40-w7-resource-budgets.md, fourth-round P1).
//
// The envelopes come from `src-tauri/tests/m21_output_schema.rs`, which prints the
// objects the runtime really hands back. This script never builds a sample: if the
// capture stops printing a case, this gate fails instead of going quiet.
//
// Dependency decision (recorded in the ticket): Node `ajv` 8 as a declared
// devDependency, not `import jsonschema` against whatever the machine happens to
// have installed, and not a hand-written validator that could quietly accept what
// it does not implement.

import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const label = "schema validate";
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

let Ajv2020;
try {
  ({ default: Ajv2020 } = await import("ajv/dist/2020.js"));
} catch (error) {
  console.error(`${label}: ajv 8 is a declared devDependency - run \`npm install\` (${error.message})`);
  process.exit(1);
}

// One real envelope per schema shape this repo claims to enforce. Adding an
// `emit(...)` in the capture test without adding it here turns this red.
// Every one of the 43 registered commands appears at least once with a real
// `.ok` envelope; `.error` entries are a representative set of failure codes.
const CASES = [
  { name: "scan_project.ok", schema: "command-result" },
  { name: "scan_project.truncated", schema: "command-result" },
  { name: "preview_initialize.ok", schema: "command-result" },
  { name: "initialize_project.ok", schema: "command-result" },
  { name: "create_request.ok", schema: "command-result" },
  { name: "load_project.ok", schema: "command-result" },
  { name: "load_project.damaged_events", schema: "command-result" },
  { name: "load_project.error", schema: "command-error" },
  { name: "list_projects.ok", schema: "command-result" },
  { name: "register_project.ok", schema: "command-result" },
  { name: "rebind_project.ok", schema: "command-result" },
  { name: "remove_agentup.ok", schema: "command-result" },
  { name: "preview_remove_agentup.ok", schema: "command-result" },
  { name: "remove_agentup.error", schema: "command-error" },
  { name: "post_discussion.ok", schema: "command-result" },
  { name: "add_attachment.ok", schema: "command-result" },
  { name: "read_attachment.ok", schema: "command-result" },
  { name: "read_attachment.error", schema: "command-error" },
  { name: "read_attachment_thumbnail.ok", schema: "command-result" },
  { name: "read_attachment_thumbnail.error", schema: "command-error" },
  { name: "load_request_thread.ok", schema: "command-result" },
  { name: "put_scope.ok", schema: "command-result" },
  { name: "diff_scope.ok", schema: "command-result" },
  { name: "put_task.ok", schema: "command-result" },
  { name: "put_task.error", schema: "command-error" },
  { name: "put_task.revision_conflict", schema: "command-error" },
  { name: "set_task_state.ok", schema: "command-result" },
  { name: "set_task_state.error", schema: "command-error" },
  { name: "load_board.ok", schema: "command-result" },
  { name: "set_agent_recording.ok", schema: "command-result" },
  { name: "start_implement.ok", schema: "command-result" },
  { name: "start_implement.error", schema: "command-error" },
  { name: "start_review.ok", schema: "command-result" },
  { name: "commit_agent_task.ok", schema: "command-result" },
  { name: "publish_result.ok", schema: "command-result" },
  { name: "load_results.ok", schema: "command-result" },
  { name: "accept_result.ok", schema: "command-result" },
  { name: "submit_result_feedback.ok", schema: "command-result" },
  { name: "compare_results.ok", schema: "command-result" },
  { name: "load_decisions.ok", schema: "command-result" },
  { name: "decide.ok", schema: "command-result" },
  { name: "decide.error", schema: "command-error" },
  { name: "search_requests.ok", schema: "command-result" },
  { name: "read_logs.ok", schema: "command-result" },
  { name: "put_understanding.ok", schema: "command-result" },
  { name: "put_understanding.error", schema: "command-error" },
  { name: "confirm_understanding.ok", schema: "command-result" },
  { name: "confirm_understanding.error", schema: "command-error" },
  { name: "load_understandings.ok", schema: "command-result" },
  { name: "discover_agents.ok", schema: "command-result" },
  { name: "list_agents.ok", schema: "command-result" },
  { name: "upsert_agent.ok", schema: "command-result" },
  { name: "remove_agent.ok", schema: "command-result" },
  { name: "get_agent_settings.ok", schema: "command-result" },
  { name: "put_agent_settings.ok", schema: "command-result" },
  { name: "load_workspace.ok", schema: "command-result" },
  // Persisted facts and events, read from the real files under `.agentup/`.
  { name: "fact.record.request", schema: "fact-record" },
  { name: "fact.record.discussion", schema: "fact-record" },
  { name: "fact.record.task", schema: "fact-record" },
  { name: "fact.record.request_revision", schema: "fact-record" },
  { name: "fact.record.understanding", schema: "fact-record" },
  { name: "fact.record.run", schema: "fact-record" },
  { name: "event.persisted.discussion_posted", schema: "event" },
  { name: "event.persisted.understanding_published", schema: "event" },
  { name: "event.persisted.request_lifecycle_changed", schema: "event" },
];
const CAPTURE_TEST = "m21_output_schema";
const MARKER = "AGENTUP-ENVELOPE ";
/** First N validator errors per case; the rest go to the count. */
const MAX_ERRORS = 5;

const ajv = new Ajv2020({ allErrors: true, strict: true });
// Ajv 2020 has no formats vocabulary of its own and throws on an unknown format in
// strict mode. `date-time` is the only format the contracts use; the producer is
// Chrono and the only consumer is this gate, so a local RFC 3339 shape is enough
// (it is deliberately not a general-purpose validator).
ajv.addFormat("date-time", /^\d{4}-\d{2}-\d{2}[Tt]\d{2}:\d{2}:\d{2}(\.\d+)?([Zz]|[+-]\d{2}:\d{2})$/);

const compiled = new Map();

function validator(file) {
  if (!compiled.has(file)) {
    const schema = JSON.parse(readFileSync(path.join(root, "schemas", `${file}.schema.json`), "utf8"));
    try {
      compiled.set(file, ajv.compile(schema));
    } catch (error) {
      console.error(`${label}: schemas/${file}.schema.json does not compile: ${error.message}`);
      process.exit(1);
    }
  }
  return compiled.get(file);
}

// Load every schema this gate uses before running cargo: a schema that cannot even
// compile should fail in a second, not after a build.
for (const { schema } of CASES) validator(schema);

const capture = spawnSync(
  "cargo",
  ["test", "--manifest-path", "src-tauri/Cargo.toml", "--offline", "--test", CAPTURE_TEST, "--", "--nocapture"],
  { cwd: root, encoding: "utf8", maxBuffer: 256 * 1024 * 1024 },
);
if (capture.error) {
  console.error(`${label}: could not run the real-output capture test: ${capture.error.message}`);
  process.exit(1);
}
if (capture.status !== 0) {
  console.error(`${label}: src-tauri/tests/${CAPTURE_TEST}.rs failed, so there is no real output to check.\n`);
  console.error([capture.stdout, capture.stderr].filter(Boolean).join("\n").trim());
  process.exit(1);
}

const captured = new Map();
const problems = [];
for (const line of capture.stdout.split("\n")) {
  const at = line.indexOf(MARKER);
  if (at < 0) continue;
  const rest = line.slice(at + MARKER.length);
  const gap = rest.indexOf(" ");
  const name = gap < 0 ? rest.trim() : rest.slice(0, gap);
  try {
    captured.set(name, JSON.parse(gap < 0 ? "" : rest.slice(gap + 1)));
  } catch {
    problems.push(`${name}: the capture printed something that is not JSON`);
  }
}

const width = Math.max(...CASES.map((entry) => entry.name.length));
const schemaWidth = Math.max(...CASES.map((entry) => entry.schema.length));
console.log(`${label} (real command output vs schemas/)`);
for (const { name, schema } of CASES) {
  if (!captured.has(name)) {
    problems.push(`${name}: the capture test printed no envelope for this case`);
    console.log(`  ${name.padEnd(width)}  ${schema.padEnd(schemaWidth)}  MISSING`);
    continue;
  }
  const validate = validator(schema);
  const ok = validate(captured.get(name));
  console.log(`  ${name.padEnd(width)}  ${schema.padEnd(schemaWidth)}  ${ok ? "PASS" : "FAIL"}`);
  if (!ok) {
    const errors = validate.errors ?? [];
    problems.push(`${name} violates ${schema}`);
    for (const error of errors.slice(0, MAX_ERRORS)) {
      // Ajv reports one error per unexpected key and names it; without the name
      // the message "must NOT have additional properties" is not actionable.
      const extra = error.params?.additionalProperty;
      const named = typeof extra === "string" ? `: ${extra}` : "";
      console.log(`      ${error.instancePath || "/"} ${error.message}${named}`);
    }
    if (errors.length > MAX_ERRORS) {
      console.log(`      ... and ${errors.length - MAX_ERRORS} more`);
    }
  }
}
for (const name of captured.keys()) {
  if (!CASES.some((entry) => entry.name === name)) {
    problems.push(`${name}: the capture test printed this envelope but no schema case claims it`);
  }
}

if (problems.length > 0) {
  console.error(`\n${label}: FAIL (${problems.length} problem(s))`);
  for (const problem of problems) console.error(`  - ${problem}`);
  process.exit(1);
}
console.log(`${label} ok: ${CASES.length} real runtime envelopes match their schemas`);
