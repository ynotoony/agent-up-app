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
| `command-registry.schema.json` | contract | Machine registry for the M0, M2 and M3 commands, required inputs/outputs, side effects, errors, capabilities and emitted events. |
| `command-registry.json` | fixture | The M0, M2 and M3 command descriptors validated by `command-registry.schema.json`. |
| `command-result.schema.json` | contract | Typed successful Tauri command result envelope. |
| `command-error.schema.json` | contract | Typed failed Tauri command result envelope and stable error codes. |

## Validation

Run from the repository root:

```sh
ruby -rjson -e 'ARGV.each { |path| JSON.parse(File.read(path)); puts "valid JSON: #{path}" }' schemas/*.json
git diff --check
```

If a Draft 2020-12 JSON Schema validator is available, validate representative fixtures with that validator as an additional check. A JSON parser check alone does not prove schema semantics.
