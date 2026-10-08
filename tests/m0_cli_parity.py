#!/usr/bin/env python3
"""Route-S PRD §5.3: the CLI is the command registry's second projection.

The registry (`schemas/command-registry.json`) is the single source of
command truth; the App IPC face (lib.rs) and the CLI face (bin/agentup.rs)
must both be projections of it. Ticket 66 closes the gate over the FULL
surface, both directions:

  1. HARD GATE: {CLI commands} == {registry commands}. Both diffs
     (CLI-only, registry-only) must be empty. Ticket 67 closed the one
     sanctioned exception (`import_run`); the gate is now the full surface.
  2. the smoke run executes real shell commands covering EVERY exposed
     command at least once (reads, writes, agent-config round trips and
     error paths) and validates every envelope against command-result/
     command-error schemas with ajv (same validator as
     tests/m0_schema_validate.mjs).

The smoke run builds a throwaway project and uses an isolated app-data dir
(AGENTUP_TEST_APP_DATA) so the developer's registry and logs are untouched.
Each `agentup` call is its own process, so the agent-settings round trip
(put_agent_settings in one process, get_agent_settings in a later one)
exercises the shared app-data seam the App reads through (ticket 66 AC3;
the App-face read is pinned in src-tauri/tests/m33_cli_batch2.rs).
"""
from __future__ import annotations

import base64
import json
import os
import struct
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# Same target dir the repo's cargo config pins (.cargo/config.toml
# build.target-dir); fall back to the crate-local path when a contributor
# builds without it.
TARGET_DIR = Path(os.environ.get("CARGO_TARGET_DIR", "/Users/bic/Projects/agent-up-cargo-target"))
BIN_CANDIDATES = [
    TARGET_DIR / "debug/agentup",
    ROOT / "src-tauri/target/debug/agentup",
]

# Ticket 67 closed the last exemption (import_run, the controlled
# governance-run import). The gate is now the full registry surface: every
# registry command must be wired in bin/agentup.rs, and the CLI may expose
# nothing else. Keep the name for the hard gate below; it stays empty.
CLI_EXCLUDED_COMMANDS: set[str] = set()


def cli_commands() -> list[str]:
    text = (ROOT / "src-tauri/src/bin/agentup.rs").read_text()
    marker = text.split("const COMMANDS: [&str; ", 1)
    if len(marker) != 2:
        return []
    block = marker[1].split("] = [", 1)[1].split("];", 1)[0]
    return [line.strip().strip('",') for line in block.splitlines() if line.strip()]


def registry_names() -> list[str]:
    data = json.loads((ROOT / "schemas/command-registry.json").read_text())
    return [entry["name"] for entry in data["commands"]]


CLI_ERROR_CODES = {
    "cli_usage",
    "cli_invalid_input",
    "cli_app_data_dir",
    "cli_session_store",
}


def ajv_validate_payload(payload: dict, schema_name: str, problems: list[str], label: str) -> None:
    """Validate an arbitrary JSON object against a schema in schemas/ (used by
    the import_run double check: the stored fact must pass
    fact-record.schema.json - the landing-side half of PRD §5.4)."""
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as handle:
        json.dump(payload, handle)
        path = handle.name
    script = """
    const fs = require("fs");
    (async () => {
      const { default: Ajv2020 } = await import("ajv/dist/2020.js");
      const ajv = new Ajv2020({ allErrors: true, strict: true });
      ajv.addFormat("date-time", /^\\d{4}-\\d{2}-\\d{2}[Tt]\\d{2}:\\d{2}:\\d{2}(\\.\\d+)?([Zz]|[+-]\\d{2}:\\d{2})$/);
      const payload = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
      const validate = ajv.compile(JSON.parse(fs.readFileSync(`${process.argv[2]}/${process.argv[3]}`, "utf8")));
      if (!validate(payload)) {
        console.error(JSON.stringify(validate.errors));
        process.exit(1);
      }
    })();
    """
    proc = subprocess.run(
        ["node", "-e", script, path, str(ROOT / "schemas"), schema_name],
        capture_output=True,
        text=True,
        cwd=ROOT,
    )
    os.unlink(path)
    if proc.returncode != 0:
        problems.append(f"{label}: payload fails {schema_name}: {proc.stderr.strip()}")


def ajv_validate(envelope: dict, problems: list[str], label: str) -> None:
    code = envelope.get("error", {}).get("code")
    if envelope.get("ok") is False and code in CLI_ERROR_CODES:
        # Pre-contract parse failure: the registered error-code enum cannot
        # describe it by design. Shape checks only (mirrors the error
        # envelope so consumers treat the two alike).
        if envelope.get("command") is None and not label.startswith("cli_"):
            problems.append(f"{label}: cli error envelope missing command")
        if not isinstance(code, str) or not envelope["error"].get("message"):
            problems.append(f"{label}: cli error envelope missing message")
        return
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as handle:
        json.dump(envelope, handle)
        path = handle.name
    script = """
    const fs = require("fs");
    (async () => {
      const { default: Ajv2020 } = await import("ajv/dist/2020.js");
      const ajv = new Ajv2020({ allErrors: true, strict: true });
      ajv.addFormat("date-time", /^\\d{4}-\\d{2}-\\d{2}[Tt]\\d{2}:\\d{2}:\\d{2}(\\.\\d+)?([Zz]|[+-]\\d{2}:\\d{2})$/);
      const envelope = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
      const schemaPath = `${process.argv[2]}/${envelope.ok ? "command-result" : "command-error"}.schema.json`;
      const validate = ajv.compile(JSON.parse(fs.readFileSync(schemaPath, "utf8")));
      if (!validate(envelope)) {
        console.error(JSON.stringify(validate.errors));
        process.exit(1);
      }
    })();
    """
    proc = subprocess.run(
        ["node", "-e", script, path, str(ROOT / "schemas")],
        capture_output=True,
        text=True,
        cwd=ROOT,
    )
    os.unlink(path)
    if proc.returncode != 0:
        problems.append(f"{label}: envelope fails schema validation: {proc.stderr.strip()}")


def run_cli(args: list[str], app_data: str) -> subprocess.CompletedProcess:
    env = dict(os.environ, AGENTUP_TEST_APP_DATA=app_data)
    return subprocess.run([str(BIN), *args], capture_output=True, text=True, env=env)


BIN = next(path for path in BIN_CANDIDATES if path.exists()) if any(
    path.exists() for path in BIN_CANDIDATES
) else BIN_CANDIDATES[0]


def png_bytes(width: int = 2, height: int = 2) -> bytes:
    """A real decodable PNG built with the stdlib only, so the thumbnail
    command's success path (decode → scale → PNG re-encode) is exercised
    without test dependencies."""

    def chunk(kind: bytes, payload: bytes) -> bytes:
        return (
            struct.pack(">I", len(payload))
            + kind
            + payload
            + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFFFFFF)
        )

    # RGB rows, one filter byte (0 = None) per row.
    raw = b"".join(
        b"\x00" + bytes([0x40, 0x80, 0xC0] * width) for _ in range(height)
    )
    return b"".join(
        [
            b"\x89PNG\r\n\x1a\n",
            chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)),
            chunk(b"IDAT", zlib.compress(raw)),
            chunk(b"IEND", b""),
        ]
    )


def smoke_run(problems: list[str]) -> None:
    covered: set[str] = set()

    with tempfile.TemporaryDirectory() as tmp:
        app_data = tempfile.mkdtemp(prefix="agentup-cli-parity-")
        project = Path(tmp)
        (project / "src").mkdir()
        (project / "src/App.tsx").write_text("export const app = () => null;\n")
        root = str(project)

        def check(args: list[str], expect_exit: int, label: str) -> dict:
            command = args[0]
            covered.add(command.split(".")[0])
            proc = run_cli(args, app_data)
            if proc.returncode != expect_exit:
                problems.append(
                    f"{label}: exit {proc.returncode} != {expect_exit}: "
                    f"{(proc.stderr or proc.stdout).strip()[:400]}"
                )
                return {}
            text = proc.stdout if expect_exit == 0 else proc.stderr
            try:
                envelope = json.loads(text)
            except json.JSONDecodeError as err:
                problems.append(f"{label}: output is not JSON: {err}")
                return {}
            if expect_exit == 0:
                assert envelope.get("ok") is True, label
            else:
                assert envelope.get("ok") is False, label
            ajv_validate(envelope, problems, label)
            return envelope

        # --- Phase A: project onboarding (batch-1 spine kept from ticket 65).
        scan = check(["scan_project", "--project", root], 0, "scan_project")
        if not scan:
            return
        fingerprint = scan["data"]["root_fingerprint"]
        project_id = scan["data"]["project_id"]

        preview = check(
            [
                "preview_initialize", "--project", root, "--arg-json",
                json.dumps({"project_id": project_id, "expected_root_fingerprint": fingerprint}),
            ],
            0, "preview_initialize",
        )
        if not preview:
            return
        token = preview["data"]["confirmation_token"]

        check(
            [
                "initialize_project", "--project", root, "--arg-json",
                json.dumps({
                    "project_id": project_id,
                    "confirmation_token": token,
                    "expected_root_fingerprint": fingerprint,
                }),
            ],
            0, "initialize_project",
        )
        check(
            [
                "create_request", "--project", root, "--arg-json",
                json.dumps({
                    "project_id": project_id,
                    "request_id": "req-parity",
                    "content": {"title": "门禁", "body": "烟测需求"},
                    "metadata": {},
                    "expected_revision": 0,
                }),
            ],
            0, "create_request",
        )
        # Duplicate id → already_exists → the error face of the map (exit 3).
        check(
            [
                "create_request", "--project", root, "--arg-json",
                json.dumps({
                    "project_id": project_id,
                    "request_id": "req-parity",
                    "content": {"title": "门禁", "body": "烟测需求"},
                    "metadata": {},
                    "expected_revision": 0,
                }),
            ],
            3, "create_request.duplicate(already_exists)",
        )
        # Missing required field → exit 2 (invalid_input branch).
        check(
            [
                "load_request_thread", "--project", root, "--arg-json",
                json.dumps({"request_id": "req-parity"}),
            ],
            2, "load_request_thread.missing_project_id(cli_invalid_input)",
        )

        # --- Phase B: batch-2 lifecycle commands.
        check(
            [
                "register_project", "--project", root, "--arg-json",
                json.dumps({"project_id": project_id}),
            ],
            0, "register_project",
        )
        # rebind_project is for MOVED projects: the runtime refuses while the
        # original registered path still exists (m2's flow). That explicit
        # refusal is the error face of the map here (exit 2).
        check(
            [
                "rebind_project", "--project", root, "--arg-json",
                json.dumps({"project_id": project_id}),
            ],
            2, "rebind_project.original_path_present(invalid_input)",
        )
        check(["load_workspace"], 0, "load_workspace")
        # Ticket 74: the smoke project has no docs/issues/index.json, so the
        # honest envelope here is the fail-visible error (exit 0 with ok:false
        # would hide it; the runtime maps a missing index to that code and the
        # CLI surfaces it as an error exit, matching read_paths discipline).
        check(
            [
                "load_governance_tickets", "--project", root, "--arg-json",
                json.dumps({"project_id": project_id}),
            ],
            3, "load_governance_tickets.index_missing",
        )
        # ROUTE-PRODUCT stage-1 A2/A3 commands against the smoke project
        # (real files, no docs/issues/, un-governed). open_ticket_agent fails
        # visible on the missing ticket index; assess_governance succeeds
        # read-only and reports all five seeds missing; a confirm-gated
        # install with an empty plan is refused (confirmation_required → the
        # every-other-code exit 3); git init refuses a non-empty directory
        # (invalid_input → exit 2).
        check(
            [
                "open_ticket_agent", "--project", root, "--arg-json",
                json.dumps({"project_id": project_id, "ticket_id": "01-x"}),
            ],
            3, "open_ticket_agent.index_missing",
        )
        assess = check(
            [
                "assess_governance", "--project", root, "--arg-json",
                json.dumps({"project_id": project_id}),
            ],
            0, "assess_governance",
        )
        if assess:
            missing = assess["data"]["missing_seeds"]
            assert missing == [
                "AGENTS.md",
                "docs/README.md",
                "docs/development-process.md",
                "docs/requests/README.md",
                "docs/agent/artifacts.yaml",
            ], "assess_governance seeds"
        check(
            [
                "install_governance_files", "--project", root, "--arg-json",
                json.dumps({
                    "project_id": project_id,
                    "answers": {},
                    "planned_paths": [],
                    "expected_root_fingerprint": fingerprint,
                }),
            ],
            3, "install_governance_files.empty_plan(confirmation_required)",
        )
        check(
            [
                "init_empty_repo", "--project", root, "--arg-json",
                json.dumps({"project_id": project_id}),
            ],
            2, "init_empty_repo.not_empty(invalid_input)",
        )
        check(
            [
                "put_task", "--project", root, "--arg-json",
                json.dumps({
                    "project_id": project_id,
                    "task_id": "task-parity",
                    "request_id": "req-parity",
                    "title": "改主按钮",
                    "expected_revision": 0,
                }),
            ],
            0, "put_task",
        )
        check(
            [
                "set_task_state", "--project", root, "--arg-json",
                json.dumps({"task_id": "task-parity", "task_state": "in_progress", "expected_revision": 1}),
            ],
            0, "set_task_state",
        )
        check(
            [
                "put_scope", "--project", root, "--arg-json",
                json.dumps({
                    "project_id": project_id,
                    "scope_id": "scope-parity",
                    "request_id": "req-parity",
                    "task_id": "task-parity",
                    "entries": [
                        {"relative_path": "src/App.tsx", "reason": "主按钮样式", "source": "user", "included": True}
                    ],
                    "expected_revision": 0,
                }),
            ],
            0, "put_scope",
        )
        check(
            [
                "diff_scope", "--project", root, "--arg-json",
                json.dumps({"scope_id": "scope-parity", "from_revision": 1, "to_revision": 1}),
            ],
            0, "diff_scope",
        )
        check(
            [
                "search_requests", "--project", root, "--arg-json",
                json.dumps({"project_id": project_id, "query": "烟测"}),
            ],
            0, "search_requests",
        )
        check(
            [
                "set_agent_recording", "--arg-json",
                json.dumps({
                    "recording": [
                        {
                            "input_digest": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                            "output": {"ok": True},
                        }
                    ]
                }),
            ],
            0, "set_agent_recording",
        )

        # --- Phase C: attachments (add → read → thumbnail success path).
        added = check(
            [
                "add_attachment", "--project", root, "--arg-json",
                json.dumps({
                    "project_id": project_id,
                    "request_id": "req-parity",
                    "media_type": "image/png",
                    "bytes": list(png_bytes()),
                }),
            ],
            0, "add_attachment",
        )
        if not added:
            return
        attachment_id = added["data"]["fact"]["id"]
        check(
            [
                "read_attachment", "--project", root, "--arg-json",
                json.dumps({"project_id": project_id, "attachment_id": attachment_id}),
            ],
            0, "read_attachment",
        )
        check(
            [
                "read_attachment_thumbnail", "--project", root, "--arg-json",
                json.dumps({"project_id": project_id, "attachment_id": attachment_id}),
            ],
            0, "read_attachment_thumbnail",
        )

        # --- Phase D: results loop; commit without a review is the runtime's
        # explicit error (exit 2), still a real pass-through execution.
        published = check(
            [
                "publish_result", "--project", root, "--arg-json",
                json.dumps({
                    "project_id": project_id,
                    "request_id": "req-parity",
                    "summary": "主按钮改为品牌色",
                }),
            ],
            0, "publish_result",
        )
        check(
            [
                "publish_result", "--project", root, "--arg-json",
                json.dumps({
                    "project_id": project_id,
                    "request_id": "req-parity",
                    "summary": "主按钮改为品牌色并补对比度",
                }),
            ],
            0, "publish_result.v2",
        )
        check(
            [
                "load_results", "--project", root, "--arg-json",
                json.dumps({"project_id": project_id, "request_id": "req-parity"}),
            ],
            0, "load_results",
        )
        if published:
            check(
                [
                    "accept_result", "--project", root, "--arg-json",
                    json.dumps({
                        "project_id": project_id,
                        "request_id": "req-parity",
                        "result_id": published["data"]["result_id"],
                        "verdict": "accepted",
                    }),
                ],
                0, "accept_result",
            )
            check(
                [
                    "compare_results", "--project", root, "--arg-json",
                    json.dumps({
                        "project_id": project_id,
                        "request_id": "req-parity",
                        "from_version": 1,
                        "to_version": 2,
                    }),
                ],
                0, "compare_results",
            )
            check(
                [
                    "submit_result_feedback", "--project", root, "--arg-json",
                    json.dumps({
                        "project_id": project_id,
                        "request_id": "req-parity",
                        "result_id": published["data"]["result_id"],
                        "body": "下一轮请补焦点环",
                    }),
                ],
                0, "submit_result_feedback",
            )
        check(
            [
                "commit_agent_task", "--project", root, "--arg-json",
                json.dumps({
                    "project_id": project_id,
                    "request_id": "req-parity",
                    "task_id": "task-parity",
                    "implement_run_id": "run-no-review",
                    "expected_revision": 1,
                }),
            ],
            3, "commit_agent_task.no_runs_dir(path_not_found)",
        )

        # --- Phase B2: the batch-1 remainder so the coverage check below can
        # demand the whole face (decide needs an open decision fact; the
        # confirmation's audit-trail decision is answered after put/confirm).
        check(
            [
                "post_discussion", "--project", root, "--arg-json",
                json.dumps({"project_id": project_id, "request_id": "req-parity", "body": "讨论"}),
            ],
            0, "post_discussion",
        )
        # Current request revision comes from load_project (the rehydration
        # projection), the same way the App renderer reads it.
        loaded = check(["load_project", "--project", root], 0, "load_project")
        request_revision = 0
        if loaded:
            for row in loaded["data"]["requests"]:
                if row.get("request_id") == "req-parity":
                    request_revision = row["revision"]
        check(
            [
                "put_understanding", "--project", root, "--arg-json",
                json.dumps({
                    "project_id": project_id,
                    "request_id": "req-parity",
                    "content": {
                        "goal_summary": "把主按钮改成品牌色",
                        "success_criteria": ["主按钮使用品牌色"],
                        "source": "initial",
                        "risks": [],
                        "ambiguities": [],
                        "questions": []
                    },
                    "expected_revision": request_revision,
                }),
            ],
            0, "put_understanding",
        )
        understandings = check(
            [
                "load_understandings", "--project", root, "--arg-json",
                json.dumps({"project_id": project_id, "request_id": "req-parity"}),
            ],
            0, "load_understandings",
        )
        if understandings:
            understanding_id = understandings["data"]["understandings"][0]["id"]
            loaded = check(["load_project", "--project", root], 0, "load_project.again")
            request_revision = 0
            if loaded:
                for row in loaded["data"]["requests"]:
                    if row.get("request_id") == "req-parity":
                        request_revision = row["revision"]
            check(
                [
                    "confirm_understanding", "--project", root, "--arg-json",
                    json.dumps({
                        "project_id": project_id,
                        "request_id": "req-parity",
                        "understanding_id": understanding_id,
                        "expected_revision": request_revision,
                    }),
                ],
                0, "confirm_understanding",
            )
        check(
            [
                "load_decisions", "--project", root, "--arg-json",
                json.dumps({"project_id": project_id, "request_id": "req-parity"}),
            ],
            0, "load_decisions",
        )
        # The confirmation leaves an audit-trail decision in `chosen` state;
        # deciding it closes the question (the m32 chain's rule).
        decisions = check(
            [
                "load_decisions", "--project", root, "--arg-json",
                json.dumps({"project_id": project_id, "request_id": "req-parity"}),
            ],
            0, "load_decisions.for_decide",
        )
        if decisions:
            target = next(
                (
                    decision
                    for decision in decisions["data"]["decisions"]
                    if decision.get("status") in ("open", "chosen")
                ),
                None,
            )
            if target:
                # SPEC-012: object-options facts pin `chosen` to a listed
                # label; take the first offered label from the fact itself.
                offered = target.get("options") or []
                label = next(
                    (
                        option["label"]
                        for option in offered
                        if isinstance(option, dict) and option.get("label")
                    ),
                    None,
                ) or next((option for option in offered if isinstance(option, str)), None)
                if label:
                    check(
                        [
                            "decide", "--project", root, "--arg-json",
                            json.dumps({
                                "project_id": project_id,
                                "request_id": "req-parity",
                                "decision_id": target["id"],
                                "chosen": label,
                            }),
                        ],
                        0, "decide",
                    )
        check(
            [
                "load_board", "--project", root, "--arg-json",
                json.dumps({"project_id": project_id, "request_id": "req-parity"}),
            ],
            0, "load_board",
        )
        # start_implement / start_review without a recording configured in this
        # process: the runtime's explicit no_provider_configured error (exit 2)
        # is the contract face for a recording-less CLI run (m32 pins it too).
        check(
            [
                "start_implement", "--project", root, "--arg-json",
                json.dumps({"project_id": project_id, "request_id": "req-parity", "task_id": "task-parity"}),
            ],
            2, "start_implement.no_recording(invalid_input)",
        )
        check(
            [
                "start_review", "--project", root, "--arg-json",
                json.dumps({
                    "project_id": project_id,
                    "request_id": "req-parity",
                    "task_id": "task-parity",
                    "implement_run_id": "run-none",
                }),
            ],
            2, "start_review.no_recording(invalid_input)",
        )
        check(["list_projects"], 0, "list_projects")
        check(["read_logs"], 0, "read_logs")

        # --- Phase E: agent registry (app_config writes through the shared
        # app-data seam; every call is a separate process, so the settings
        # written here are read back by later processes from disk).
        check(["discover_agents"], 0, "discover_agents")
        check(["list_agents"], 0, "list_agents")
        check(
            [
                "upsert_agent", "--arg-json",
                json.dumps({
                    "agent": {
                        "name": "本机审查员",
                        "description": "本机接入的审查执行体",
                        "capabilities": ["verify"],
                    }
                }),
            ],
            0, "upsert_agent",
        )
        check(
            [
                "put_agent_settings", "--arg-json",
                json.dumps({"settings": {"default_agent": "agent-build"}}),
            ],
            0, "put_agent_settings",
        )
        settings = check(["get_agent_settings"], 0, "get_agent_settings.after_write")
        if settings and settings["data"]["settings"].get("default_agent") != "agent-build":
            problems.append(
                "get_agent_settings.after_write: settings written by an earlier "
                f"CLI process did not survive through the app-data seam: {settings}"
            )
        # Remove the custom agent (built-ins are protected by the runtime).
        listing = check(["list_agents"], 0, "list_agents.pre_remove")
        if listing:
            custom = next(
                (
                    agent["agent_key"]
                    for agent in listing["data"]["agents"]
                    if agent.get("source") == "custom"
                ),
                None,
            )
            if custom:
                check(
                    [
                        "remove_agent", "--arg-json",
                        json.dumps({"agent_key": custom}),
                    ],
                    0, "remove_agent",
                )

        # --- Phase E2 (ticket 67): the controlled governance-run import.
        # A governance run record is not an App business run, so it carries no
        # request fact of its own; the mirror pass names the linkage request.
        governance_record = {
            "run_id": "20260928-par67a",
            "mode": "delivery",
            "phase": "implementation",
            "task": "ticket 67-import-run-command-and-cli-docs",
            "status": {"session": "closed", "task": "done"},
            "scope": ["src-tauri/src/typed_facts.rs"],
            "baseline": {
                "vcs_ref": "9aa1984",
                "workspace_fingerprint": "fp-v1:0123456789abcdef",
            },
            "modified_files": ["src-tauri/src/typed_facts.rs"],
            "last_verified": {
                "command": "cargo test --offline --test m34_import_run",
                "result": "pass",
                "at": "2026-09-28",
            },
            "next_step": "independent review",
            "blocker": "",
            "network": "unavailable",
            "updated_at": "2026-09-28T17:00:00+08:00",
        }
        imported = check(
            [
                "import_run", "--project", root, "--arg-json",
                json.dumps({
                    "project_id": project_id,
                    "request_id": "req-parity",
                    "record": governance_record,
                }),
            ],
            0, "import_run",
        )
        if imported:
            fact = imported["data"]["fact"]
            if fact.get("source") != "governance_mirror":
                problems.append(f"import_run: fact source is not governance_mirror: {fact}")
            if imported["data"].get("existing") is not False:
                problems.append(f"import_run: first import must be existing:false: {imported}")
            # Landing-side half of the double validation: the stored fact
            # envelope must pass fact-record.schema.json.
            ajv_validate_payload(fact, "fact-record.schema.json", problems, "import_run.fact")
            if fact.get("content", {}).get("governance", {}).get("run_id") != "20260928-par67a":
                problems.append(f"import_run: governance record not carried verbatim: {fact}")
        # Idempotency: same run_id again → the stored fact comes back,
        # existing:true, and no second revision file appears.
        again = check(
            [
                "import_run", "--project", root, "--arg-json",
                json.dumps({
                    "project_id": project_id,
                    "request_id": "req-parity",
                    "record": governance_record,
                }),
            ],
            0, "import_run.idempotent",
        )
        if again:
            if again["data"].get("existing") is not True:
                problems.append(f"import_run.idempotent: re-import must be existing:true: {again}")
            if again["data"]["fact"] != imported["data"]["fact"]:
                problems.append("import_run.idempotent: stored fact changed on re-import")
        # A schema-invalid record is refused with zero writes.
        broken = dict(governance_record, mode="not-a-mode")
        check(
            [
                "import_run", "--project", root, "--arg-json",
                json.dumps({
                    "project_id": project_id,
                    "request_id": "req-parity",
                    "record": broken,
                }),
            ],
            2, "import_run.invalid_record(invalid_input)",
        )

        # --- Phase F: teardown path last — preview + confirm the removal.
        remove_preview = check(
            [
                "preview_remove_agentup", "--project", root, "--arg-json",
                json.dumps({"project_id": project_id}),
            ],
            0, "preview_remove_agentup",
        )
        if remove_preview:
            check(
                [
                    "remove_agentup", "--project", root, "--arg-json",
                    json.dumps({
                        "project_id": project_id,
                        "confirmation_token": remove_preview["data"]["confirmation_token"],
                        "mode": "backup",
                    }),
                ],
                0, "remove_agentup",
            )

    # AC2: every exposed command was really executed at least once above.
    exposed = set(cli_commands())
    missing = sorted(exposed - covered)
    if missing:
        problems.append(f"smoke run did not execute these commands: {missing}")


def main() -> int:
    problems: list[str] = []

    exposed = cli_commands()
    registry = registry_names()

    # HARD GATE (ticket 66 AC1; ticket 67 closed the import_run exemption):
    # {CLI} == {registry}, both directions, no duplicates on either side.
    duplicated = sorted({name for name in exposed if exposed.count(name) > 1})
    if duplicated:
        problems.append(f"bin/agentup.rs COMMANDS contains duplicates: {duplicated}")
    registry_duplicated = sorted({name for name in registry if registry.count(name) > 1})
    if registry_duplicated:
        problems.append(f"schemas/command-registry.json contains duplicates: {registry_duplicated}")

    expected = set(registry) - CLI_EXCLUDED_COMMANDS
    cli_only = sorted(set(exposed) - expected)
    registry_only = sorted(expected - set(exposed))
    if cli_only:
        problems.append(f"CLI exposes commands the registry does not know: {cli_only}")
    if registry_only:
        problems.append(
            f"registry commands the CLI does not expose: {registry_only}"
        )

    if not BIN.exists():
        problems.append(
            "agentup binary not found - build it first "
            "(cargo build --offline --manifest-path src-tauri/Cargo.toml --bin agentup)"
        )
    else:
        smoke_run(problems)

    if problems:
        print("cli parity problems:", file=sys.stderr)
        print("\n".join(problems), file=sys.stderr)
        return 1
    print(
        f"cli parity ok: CLI face ({len(set(exposed))} commands) == registry set "
        f"({len(set(registry))} commands) minus exempt {sorted(CLI_EXCLUDED_COMMANDS)}; "
        "every command executed once through real shell invocations with ajv-"
        "validated envelopes (scan→preview→initialize→register/rebind→task/scope→"
        "results loop→agent registry→remove_agentup + error paths)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
