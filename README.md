# AiFlow

A free desktop workspace for everyone to manage GitHub issues, ordered execution queues and official AI coding clients. Built with **Tauri 2, React, TypeScript, Rust and SQLite**. Every user connects their own accounts; there is no AiFlow account subscription or shared developer credential at runtime. Provider-specific commands and output parsing are isolated from orchestration.

**Development preview, not a production-certified release.** See [implementation status](docs/IMPLEMENTATION_STATUS.md) for precise verification boundaries. No demo issues, fabricated account states or simulated agent output are loaded into the application.

## Run

```powershell
npm ci
npm run desktop
```

Requires Node.js, Git, Rust stable with the platform toolchain, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). If `.tools/cargo` and `.tools/rustup` exist, the desktop script uses this local toolchain without changing the global PATH. `npm run dev` opens the frontend preview on port 1420; real operations require the desktop backend.

## First project

1. Open **Accounts → Connect GitHub**. AiFlow verifies and reuses your existing official GitHub CLI account first. If sign-in is needed, it launches `gh auth login`, displays the real code without a terminal and opens the GitHub authorization window. No AiFlow OAuth App, developer account or manual token is required. All GitHub operations use `gh api`; credentials stay with GitHub CLI.
2. Install any desired official provider CLI separately. AiFlow does not install providers. Existing official CLI authentication can be reused. **Providers → Scan** probes installed versions and capabilities.
3. **Projects** automatically lists repositories accessible to your GitHub account. Select one to load its open and closed **Issues**. A local folder is optional; configure execution policy in repository settings. AiFlow can prepare a managed checkout automatically, or use an existing linked worktree. Detached HEAD and subdirectory selections are rejected.
4. Configure independent validation as argument arrays, e.g. `[["npm","test"],["npm","run","build"]]`. These are trusted project configuration; issues cannot define commands or policy.
5. Sync issues. Open an issue, select its provider and **Add to queue**. **Dry run** previews setup and provider health, inspecting Git when a folder is already linked, without launching an agent or modifying GitHub.
6. Start the queue and choose the execution environment. Follow execution and validation in **Execution Center**. Completed requires validation and a confirmed GitHub issue report.

## Default safety policy

- No automatic provider failover; **Auto** resolves to the project's concrete default.
- Only one execution per canonical worktree; OS file leases plus persistent lock records.
- Clean tree, unchanged branch, a new descendant commit and independent tests required.
- Validation reports are published to GitHub before releasing the next issue. Closing issues must be explicitly enabled per project.
- `production`, `deployment` and `manual-approval` labels block until reviewed and approved.
- Pause stops future scheduling. Cancellation stops the process tree and preserves partial work.
- After a crash, interrupted attempts require review. Waiting quota attempts retain their retry time; start the paused queue to continue.
- No frontend arbitrary shell, no password storage, no browser cookie access, no private provider endpoints.

## Account connections

All integrated accounts use their installed official CLI, with a Prepare → Authorize → Verify modal and no terminal output:

| Account          | Official authentication                                                                    | Verification                                                                                             |
| ---------------- | ------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------- |
| GitHub           | Reuse `gh` account, otherwise `gh auth login --web`                                        | `gh api /user`                                                                                           |
| OpenAI / ChatGPT | Reuse Codex session, otherwise normal `codex login` browser flow                           | `codex login status`                                                                                     |
| Anthropic Claude | Reuse Claude Code session, otherwise `claude auth login`                                   | `claude auth status`                                                                                     |
| Google Gemini    | Gemini CLI ACP `initialize` / `authenticate`, selecting its advertised Google login method | Official ACP authentication response; recent evidence is kept for two minutes in the current app session |

The modal shows a code only when the official CLI supplies one. Codex no longer defaults to `--device-auth`. Google requires a secure external browser, so Gemini uses the system browser opened by the official CLI. Codex and Claude can open their official URL in an isolated AiFlow window when the CLI provides it; the system-browser fallback remains available for SSO and provider restrictions. A CLI may also open the system browser itself.

Using a CLI still involves the provider's own authentication and authorization rules. AiFlow does not impersonate a CLI, borrow another application's client ID, bypass organizational approval, read credential files or copy tokens. An organization may still require its administrator to permit the official client. See [authentication details and sources](docs/AUTHENTICATION.md).

AiFlow itself is intended to be free. Official provider clients may require the user's own supported account or subscription; AiFlow does not include provider credits or bypass provider billing and limits.

## Commands

| Command                                                            | Purpose                                        |
| ------------------------------------------------------------------ | ---------------------------------------------- |
| `npm run desktop`                                                  | Native development app                         |
| `npm run desktop:build`                                            | Native release executable and platform bundles |
| `npm run build`                                                    | TypeScript check and frontend production build |
| `npm run lint`                                                     | Frontend static checks                         |
| `npm test`                                                         | Domain and frontend utilities                  |
| `npm run test:e2e`                                                 | Browser UI regression and screenshots          |
| `cargo test --manifest-path src-tauri/Cargo.toml`                  | Rust unit/integration tests                    |
| `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` | Backend static checks                          |

The local database and output archive are under Tauri's application data directory for `app.aiflow.desktop`. Keep the SQLite database, WAL files and archive folder together when backing up a stopped app.

Read [architecture](docs/ARCHITECTURE.md), [security](docs/SECURITY.md), [Windows builds](docs/BUILD_WINDOWS.md) and [macOS builds](docs/BUILD_MACOS.md).

## Source and build artifacts

Source is published at [andregabry/AiFlow](https://github.com/andregabry/AiFlow). The [validation workflow](https://github.com/andregabry/AiFlow/actions/workflows/validate.yml) checks and packages Windows and macOS. Completed successful runs expose unsigned installer artifacts on their run page; source publication alone does not certify either platform or the live account integrations.

Local verification on 2026-10-21 covers 41 Rust, 14 TypeScript and 11 browser tests. New Claude/Gemini account logins, live provider-cloud task completion and macOS runtime behavior still require live validation.

## Execution environment at queue start

Adding a GitHub issue to the queue requires no local folder. Choose **This computer** or **Provider cloud** beside **Start queue**. Local execution starts directly; cloud setup is requested only when its saved configuration is missing. Local mode can clone into an AiFlow-managed workspace, run independent validation commands and publish the validated commit to an isolated `work/aiflow-*` branch. Existing user-linked worktrees retain local-only Git publication. A confirmed validation report on the source GitHub issue is required before releasing the next task. Closing issues remains an explicit project policy.

Provider cloud currently supports the official Codex CLI. Supply the repository's cloud environment ID, published base branch and exact required GitHub check names. The controller waits for the expected branch, pull request and successful checks on the exact published commit. If Codex leaves only a diff, publish it from the provider task before the gate can pass. Cloud completion text alone does not release the next issue. Claude/Gemini cloud adapters are not implemented. Provider plan eligibility and usage limits apply; no local sign-in credentials are copied into a runner.

Cloud references survive restart and resume tracking without resubmission. Stopping tracking does not cancel remote compute. AiFlow must be open to validate and dispatch subsequent tasks. The cloud CLI was probed read-only; no live cloud model task was launched during verification.

The circled exclamation button beside Start queue opens a practical setup tutorial. Local execution detects declared npm tests/build, Cargo tests, Go tests or pytest automatically when no validation commands are configured. Explicit commands are preserved. Unknown test suites stop before the agent starts and point to the tutorial; tests are never silently disabled.
