# Authentication

AiFlow connects through the installed official GitHub, OpenAI Codex, Claude Code and Gemini CLIs. Every connection uses a Prepare → Authorize → Verify form, without a terminal panel. Existing verifiable CLI sessions are reused before a new login. Account credentials remain with the client; AiFlow does not create a product OAuth grant for the normal account workflow, scrape cookies, read private credential files or export CLI tokens.

## Provider flows

- GitHub: `github_cli_start` verifies an existing account with `gh api /user`. If needed, `gh auth login --hostname github.com --web --git-protocol https --skip-ssh-key` starts official sign-in. Both colon-delimited and parenthesized CLI user-code messages are recognized. The code appears as a copyable field and in the internal window title. GitHub API operations use `gh api`. Disconnect only removes AiFlow's local connection marker; the shared GitHub CLI remains signed in. Legacy direct OAuth commands remain for compatibility but are not called by the Accounts workflow.
- Codex: `codex login status` verifies the CLI session. Login uses normal `codex login`, with the official browser callback. Device-code authentication is no longer forced. The client's own account, entitlement and workspace restrictions apply.
- Claude: `claude auth status` verifies the CLI session and `claude auth login` starts official browser sign-in. CLI credentials and organizational restrictions remain under Claude Code's control.
- Gemini: the app probes official CLI help for `--acp` or `--experimental-acp`, negotiates ACP version 1 with no filesystem/terminal/tool capabilities, and selects the advertised Google agent authentication method (`oauth-personal`). Only `initialize` and `authenticate` are sent. No chat session, model prompt, API key or inference is involved. The CLI reuses valid credentials or opens its Google browser flow. Only an authenticated RPC response confirms success; exit status alone is insufficient. A recent successful response is reflected in scans for two minutes in the current app session. Beyond that, the status returns to UNKNOWN until the CLI supplies new evidence. No synthetic account identity or billing information is shown. Logout is not exposed when the CLI does not advertise a supported operation.

Google does not permit its OAuth authorization request in an embedded user-agent controlled by the app. Gemini therefore uses the secure system browser. Other supported provider URLs can open in isolated, incognito AiFlow windows; only that provider's approved HTTPS origins and the exact loopback callback supplied in its authorization URL are allowed. New windows are denied, remote webviews receive no app capabilities, and no page scripts or credentials are injected. A system-browser option supports external SSO and providers that reject embedded login. The official CLI can also open the browser itself. GitHub's documented GH_BROWSER hook uses an AiFlow helper to prevent duplicate browser launch for its code flow.

Closing a running connection cancels the supervised CLI process and waits for it to stop. Errors remain in the form. Codes are displayed only when actually supplied by the CLI. A browser callback flow normally has no code to enter. Verification must succeed before CONNECTED appears. Missing or incompatible CLIs receive installation/update instructions and are never installed silently.

## Account policy limits

CLI login still uses the provider's official OAuth or account session. Calling it a CLI connection does not remove consent, enterprise SSO, entitlements, or administrator approval. AiFlow does not bypass those requirements or claim guaranteed administrator-free login. Workspace policy may restrict Codex authentication; Google Workspace may require enabling the official Gemini client; Claude organization access may require an invitation.

## Official references

- [Codex authentication](https://learn.chatgpt.com/docs/auth): normal browser login, cached CLI sessions, login status and device-code policy.
- [Claude Code authentication](https://code.claude.com/docs/en/authentication) and [CLI commands](https://code.claude.com/docs/en/cli-reference).
- [Gemini CLI authentication](https://geminicli.com/docs/get-started/authentication/) and [ACP mode](https://geminicli.com/docs/cli/acp-mode/).
- [ACP authentication](https://agentclientprotocol.com/protocol/v1/authentication) and [Gemini agent implementation](https://github.com/google-gemini/gemini-cli/blob/main/packages/cli/src/acp/acpRpcDispatcher.ts).
- [Google secure-browser policy](https://developers.google.com/identity/protocols/oauth2/policies#use-secure-browsers).
- [GitHub CLI login](https://cli.github.com/manual/gh_auth_login), [API](https://cli.github.com/manual/gh_api) and [environment](https://cli.github.com/manual/gh_help_environment).

## Verification boundary

UI fixtures cover each provider, reuse of existing CLI sessions, code display, window commands, cancellation and unverified accounts. Rust tests cover Gemini protocol negotiation and authentication using a fake CLI process that rejects model prompts. The installed Codex help and GitHub account API were checked locally. Real new logins for Claude and Gemini and native cross-platform authorization are not claimed as validated; these clients are not installed on this machine.
