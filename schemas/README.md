<!-- Input: docs/specs/001-m0-foundation.md and the M0-02 runtime contract. -->
<!-- Output: Machine-readable JSON Schema inventory and repeatable validation commands. -->
<!-- Pos: Contract directory index; update when a schema member is added, removed, or renamed. -->

# Schemas

This directory contains the machine-checkable contracts for project facts and runtime events. The schemas describe M0 envelopes and M1 type-specific fact/event content. They do not make SQLite, the renderer, or a runtime implementation a source of truth.

## Direct members

| Name | Status | Purpose |
| --- | --- | --- |
| `README.md` | index | Directory scope and validation commands. |
| `fact-record.schema.json` | contract | Draft 2020-12 schema for persisted fact records. |
| `event.schema.json` | contract | Draft 2020-12 schema for immutable runtime and persisted events. |
| `command-registry.schema.json` | contract | Machine registry for the M0-M4 commands, required inputs/outputs, side effects, errors, capabilities and emitted events. |
| `command-registry.json` | fixture | The M0-M4 command descriptors validated by `command-registry.schema.json`. |
| `command-result.schema.json` | contract | Typed successful Tauri command result envelope. |
| `command-error.schema.json` | contract | Typed failed Tauri command result envelope and stable error codes. |

## Validation

Run from the repository root:

```sh
python3 tests/m0_schema_parse.py    # every schema file parses as JSON
node tests/m0_schema_validate.mjs   # real runtime output against these schemas
git diff --check
```

`tests/m0_schema_validate.mjs` is the only place where **real** output meets these schemas: it runs `src-tauri/tests/m21_output_schema.rs`, which prints the envelopes the runtime actually returns and the persisted fact/event records it actually wrote under `.agentup/`, and validates them with Draft 2020-12 (`ajv@8`, a declared devDependency). Both checks run inside `npm test`.

A JSON parser check alone does not prove schema semantics, and a schema that no real output is ever fed to proves nothing about the commands it claims to describe - that is exactly how `scan` emitted two keys this directory forbade without any check turning red (see `docs/issues/40-w7-resource-budgets.md`).
