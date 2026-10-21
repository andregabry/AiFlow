# Provider architecture

`AIExecutionProvider` is an async Rust trait registered in `ProviderManager`. Orchestration receives a trait object selected by ID and project policy. New implementations belong under `providers/` and are registered once; the queue, GitHub and UI business rules need no new provider branches.

Each adapter owns executable resolution, capability detection, CLI arguments, official authentication commands, structured output parsing, error classification and quota parsing. Help is probed with a 15-second timeout. Missing required headless/streaming features yield INCOMPATIBLE. Capability values are conservative: advertised options do not prove end-to-end account compatibility.

Windows npm shims are resolved to the provider's known JavaScript entry point and invoked directly with Node; task strings never pass through `cmd.exe`. Native binaries execute directly. Prompts go to stdin instead of becoming command-line options.

`Launch` and the process supervisor implement shared child lifecycle, stdout/stderr capture, bounded channels, error preservation and cancellation. The provider does not own queue advancement. Resume uses an explicit captured session ID only; never `--last`, which could resume an unrelated session. Switching providers creates a new attempt and requires a clean-tree review before it can proceed.

Quota reset information is accepted only from validated structured timestamps or numeric retry-after values. Free-text reset times are not guessed. Unreported reset means Unknown, with a separately labelled next retry. Fallback remains ask-only in this version; automatic routing rules and external plugins are extension points, not implemented features.

Official references checked during implementation:

- [Codex noninteractive mode](https://developers.openai.com/codex/noninteractive)
- [Claude CLI reference](https://code.claude.com/docs/en/cli-reference)
- [Gemini headless mode](https://geminicli.com/docs/cli/headless/)
