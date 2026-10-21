# OpenAI Codex adapter

Official CLI: `codex`. Discovery probes `codex exec --help`, `codex --version` and the independent `codex login status`. Login delegates to `codex login --device-auth`; logout delegates to `codex logout`. No credential files are read.

New attempts use `exec --sandbox workspace-write --json -` with prompt stdin. Resume uses `exec resume <captured-session-id> --json -`. No danger-full-access or approval bypass flags are used. JSONL remains available in Raw; item text, command output and errors are surfaced in Formatted. `thread_id` is retained as the resume handle.

Locally detected at initial inspection: codex-cli 0.163.0. A successful version/help probe does not prove a real task execution, account entitlement or quota availability. Live account-consuming execution is not part of the test suite.

See [official Codex CLI docs](https://developers.openai.com/codex/cli) and [noninteractive mode](https://developers.openai.com/codex/noninteractive). CLI permission requests may require human review; AiFlow does not bypass them.
