<!-- Input: Agent Up ticket schema and license, plus existing repository ticket scripts. -->
<!-- Output: The schema and license copied into the Tauri resource bundle for governed task creation. -->
<!-- Pos: Third-party runtime resource source; update only by reconciling against the recorded Agent Up source. -->

# Agent Up Runtime Resources

`ticket-ops.sh` and `generate-progress.sh` are pinned snapshots of this project's Agent Up-derived scripts, adapted to the `facts/requirements/tickets/` layout. `ticket-record.schema.json` and `issue-index.schema.json` are copied from the locally installed Agent Up skill package. The MIT license is included for redistribution. Their source and update policy are recorded in `facts/project/artifacts.yaml`.

Tauri maps these files into the packaged `agent-up/` resource tree. The scripts are tracked snapshots so a source checkout can build without the maintainer's local `rules/` directory. Update them by reconciling against their recorded project sources and keeping the scripts, schemas, and license aligned.

| File | Purpose |
| --- | --- |
| `README.md` | Provenance and direct-member index for this resource group. |
| `ticket-ops.sh` | Creates a task and updates its authoritative index, README, ledger, and progress projection. |
| `generate-progress.sh` | Regenerates the ticket status projection. |
| `ticket-record.schema.json` | Validates the formal task body fields. |
| `issue-index.schema.json` | Defines the formal task index rows. |
| `LICENSE` | MIT license required for redistributing these resources. |
