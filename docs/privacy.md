<!-- Input: SPEC-011。 -->
<!-- Output: 隐私说明。 -->
<!-- Pos: 首版隐私政策。 -->

# Privacy

AgentUp is a local single-user app. Project facts live in the project `.agentup/` directory.

- No account. No telemetry by default.
- `export_diagnostics` writes a local redacted JSON file. It is not uploaded.
- Diagnostics omit file bodies, absolute paths, and secret-like fields.
- Renderer cannot use fs, shell, network, or secret plugins.
