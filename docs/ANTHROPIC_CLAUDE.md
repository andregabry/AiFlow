# Anthropic Claude adapter

Official CLI: `claude`. Help must advertise print mode and stream-json. Execution uses `--print --output-format stream-json --verbose` with stdin. Resume passes the previously captured `session_id` with `--resume`.

Authentication is checked independently with `claude auth status`. Login/logout delegate to `claude auth login` and `claude auth logout`. An execution error alone never invalidates credentials: a separate negative auth check is required.

Claude was not found during local environment inspection. Adapter and parser tests are distinct from live integration validation. Older versions without the official auth commands may report authentication UNKNOWN and need a supported CLI version. No unrestricted tool or permission bypass is enabled.

Reference: [official CLI reference](https://code.claude.com/docs/en/cli-reference).
