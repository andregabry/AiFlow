# Implementation status

Updated: 2026-10-21. **Development preview. Not production ready.** The requested full product has 126 requirements; implemented source is not evidence that every requirement is complete.

## DONE — implemented source, validation tracked below

| Area                                                          | UI             | Backend                                                             | Integration evidence                                         |
| ------------------------------------------------------------- | -------------- | ------------------------------------------------------------------- | ------------------------------------------------------------ |
| Home, sidebar, project selection, command palette             | UI CONNECTED   | BACKEND READY: snapshot IPC                                         | Browser suite passed; native startup checked                 |
| Issues Board/List, search/filter/sort, detail sheet           | UI CONNECTED   | Real GitHub issue cache                                             | Live GitHub pending                                          |
| Queue, per-item provider, drag reorder and keyboard move      | UI CONNECTED   | BACKEND READY: persistent queue commands                            | Fake-provider queue/validation tests passed                  |
| Execution Center, timeline, Raw/Formatted, search/copy/follow | UI CONNECTED   | BACKEND READY: incremental stdout/stderr, segmented archive         | Real subprocess and UI IPC streaming tests passed            |
| Provider dashboard, capabilities and diagnostics              | UI CONNECTED   | Three official CLI adapters                                         | Local Codex help/version only                                |
| GitHub Accounts / Device Flow and CLI fallback                | UI CONNECTED   | OAuth + OS vault; official CLI login/output and API operations      | Wizard regression tests; live browser authorization pending  |
| Project/worktree/policy editor                                | UI CONNECTED   | BACKEND READY: Git root, branch, HEAD, tree probes                  | Disposable Git tests passed                                  |
| Quota state, unknown reset, shared clock                      | UI CONNECTED   | Per-provider persistence + bounded retry                            | Parser tests, real quota pending                             |
| Independent completion validation                             | UI CONNECTED   | Exit, branch/tree, descendant commit, configured tests, optional CI | Fake provider tested end-to-end; live provider pending       |
| Recovery, leases, manual approval, audit                      | UI CONNECTED   | BACKEND READY: SQLite + OS locks, paused startup                    | Restart, lease contention and gate tests passed              |
| Themes and accessibility                                      | UI implemented | System theme, reduced-motion/transparency CSS                       | Browser keyboard/theme checks and screenshot review passed   |
| Native lifecycle                                              | Controls wired | Tray, notifications, sleep assertion, Windows job containment       | Windows startup/detection checked; full lifecycle QA pending |

## IN PROGRESS

- Public release certification and real-account integration testing.
- The complete requested product remains in progress. The items below are explicitly unfinished.

Output storage rotates at 8 MiB per JSONL shard and indexes offsets in SQLite. Replay reads 2,000 output events at a time; the renderer retains at most 10,000 output events per opened console plus its timeline. Older output can be paged without discarding durable history. Unopened consoles replay on demand rather than accumulating live output in memory. Multi-console cache eviction and disk-retention preferences remain to be added.

## NEXT — required for full requested product

- Real account integrations for GitHub and each provider, including subscription entitlement and installed-version compatibility.
- Explicit provider handoff with reviewed partial changes; currently dirty-tree handoff blocks safely.
- Rich issue administration (comments, assignees, linked PRs), GitHub Projects discovery/mapping, Enterprise hosts, Actions and check pagination.
- Rule-based Auto routing and configurable Automatic failover. Currently only project default and explicit change are supported.
- Provider plugin packaging; current registry is compiled in.
- Rich terminal rendering (safe stripping implemented), detailed tool/file normalized event coverage, full large-issue virtualization and memory-bounded multi-execution replay.
- Semantic acceptance evidence schema, dependency/blocker editor, configurable execution timeouts and CI wait/recheck workflow.
- Google/AiFlow identity, optional OS-specific reduced-transparency detection beyond CSS, polished context-menu action set, spring reorder UX refinement.
- Automatic startup resume preference, configurable wait sleep behavior and tray countdown/status detail.
- Native macOS compilation, visual QA, Keychain/login, PATH discovery, process-tree crash behavior and notifications.
- Signing, notarization and installation tests on clean machines.

## BLOCKED / external prerequisites

- The publisher OAuth App ID is not provisioned. This build uses installed GitHub CLI for sign-in and API operations instead. Its modal code/output, verified-success behavior and cancellation are covered by browser fixtures; a real fresh browser authorization and clean-machine installation are still pending. Local GitHub CLI 2.102.0 is installed and its read-only authentication status and a `/user` API request succeed.
- Claude and Gemini CLIs were absent at initial inspection. They are not installed silently.
- macOS host, Apple signing/notarization identity and Windows signing certificate are not available in this Windows environment.

## Validation results

Recorded checks:

- TypeScript typecheck, ESLint, Prettier and frontend production build passed.
- 14 TypeScript unit tests passed.
- 10 Playwright scenarios cover offline navigation/themes, project sheet keyboard behavior, connected-state/live IPC, GitHub's sequential connection modal, isolated provider login output/cancellation, inline errors plus unverifiable-account handling, and the build-without-OAuth fallback through official GitHub CLI, automatic internal-window commands and code display. Fixture data is **UI MOCK for tests only**, never shipped as real state.
- 25 Rust tests passed: state transitions, provider classification, quota parsing, redaction, process streaming/cancellation, persistent replay/recovery, independent quotas, disposable Git probes/leases, sequential queue validation, quota resume preserving partial changes, and failure/production gates.
- Rust formatting and Clippy `--all-targets -- -D warnings` passed with no warnings.
- npm audit reported 0 vulnerabilities. No failed test was converted into a skip.
- Native Windows release built and opened successfully, showing Local backend connected, Codex CLI 0.163.0 detected, and Claude/Gemini NOT INSTALLED.
- NSIS EXE and MSI were generated successfully, unsigned. Installer installation on a clean machine has not been validated.
- macOS workflow/configuration exists; no macOS build or live integration is claimed.

Artifacts:

- `src-tauri/target/release/aiflow.exe`
- `src-tauri/target/release/bundle/nsis/AiFlow_0.1.0_x64-setup.exe`
- `src-tauri/target/release/bundle/msi/AiFlow_0.1.0_x64_en-US.msi`
- Browser evidence under `test-results/` (ignored by Git).

Windows unit tests need the Common Controls v6 activation manifest once the native desktop bridge is linked. The build script embeds that manifest into all Windows targets and keeps icon/version resources in Tauri's normal binary resource path.

**INTEGRATION VALIDATED applies only to the named local/fake-process and browser test boundaries. Live GitHub, real provider task completion, macOS, signed distribution and all 126 requirements are not certified.**

GitHub sign-in window update: the UI opens an AiFlow-owned incognito webview once a valid device user code arrives, displays/copies the code in the modal and shows it in the window title. The official CLI browser hook suppresses duplicate system-browser launches. The remote webview has no application capabilities, allows only HTTPS github.com navigation and denies new windows. Fixture-based open/reopen/close commands and Rust code validation passed; live webview page loading and account authorization are not claimed as tested.

## Official CLI account workflow

Accounts now uses verifiable existing CLI sessions first, forms without terminal panels for all providers, normal Codex browser login instead of forced device auth, Claude Code's supported login/status commands, and Gemini CLI ACP authentication with a two-minute current-session verification window. The Gemini integration performs protocol authentication only, without inference, API keys or an AiFlow OAuth client. Google uses the external secure browser required by its policy; other supported official URLs can open in isolated internal windows with an external-browser fallback. Client consent and organizational access rules remain in force. Provider browser fixtures, a fake Gemini ACP process and origin-validation tests passed. Real Claude/Gemini accounts and macOS authorization remain unvalidated.
