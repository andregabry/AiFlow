# Testing and evidence

Frontend unit tests cover operational states, validation-before-DONE semantics, quota countdowns, timezone/unknown reset behavior, replay/live deduplication, ANSI/OSC stripping and safe policy defaults.

Browser regression navigates all pages, checks offline onboarding and project forms, command palette keyboard access, light/dark themes and absence of runtime errors. Screenshots are written to test-results. Browser tests are UI evidence, not proof of native GitHub or provider integration.

Rust tests cover adapter capability degradation, quota parsers, secret redaction, repository validation, explicit transitions, incremental streaming and persistence/recovery. Tests must use fake providers or disposable temporary repositories; they must never consume real provider quota or alter user repositories.

The orchestrator suite runs real disposable Git repositories and a test-only provider child process through queue dispatch, independent validation, commit/tree checks, quota resume, failure and production approval gates. GitHub comments are explicitly injected fixtures in those tests; no live GitHub completion is claimed. Windows test executables receive the same Common Controls v6 activation dependency as the native app, avoiding loader failures before the harness starts.

Run frontend checks with `npm run lint`, `npm test`, `npm run typecheck`, `npm run format:check`, `npm run build`, `npm run test:e2e`. Run Rust checks with cargo fmt --check, cargo test and cargo clippy -- -D warnings. Use the local toolchain environment when needed. Run npm audit and git diff --check before a milestone is marked complete.

Still required for full certification: live GitHub Device Flow, issue/cache reconciliation against a real authorized repository, all three CLIs with real account authentication and permission behavior, kill/restart during active child trees on each OS, exact quota/reset fixtures from supported real versions, load/disk-exhaustion testing, semantic acceptance gates, and signed native installers. Missing live verification must not be replaced with a passing mock claim.
