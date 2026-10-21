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
3. Add a project, choose `owner/repository`, and select an existing Git worktree root. Detached HEAD and subdirectory selections are rejected. Set a concrete default provider and allowed providers.
4. Configure independent validation as argument arrays, e.g. `[["npm","test"],["npm","run","build"]]`. These are trusted project configuration; issues cannot define commands or policy.
5. Sync issues. Open an issue, select its provider and **Add to queue**. **Dry run** checks Git, probes the provider and shows the prompt without launching an agent or modifying GitHub.
6. Start the queue. Follow stdout/stderr and validation in **Execution Center**. Completed means the backend validator passed, not merely that the CLI said “done”.

## Default safety policy

- No automatic provider failover; **Auto** resolves to the project's concrete default.
- Only one execution per canonical worktree; OS file leases plus persistent lock records.
- Clean tree, unchanged branch, a new descendant commit and independent tests required.
- GitHub state is local-only by default. Closing issues must be explicitly enabled per project.
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
