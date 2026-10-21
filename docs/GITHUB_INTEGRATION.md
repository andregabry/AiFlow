# GitHub integration

GitHub.com REST is implemented directly in the Rust backend. Repository listing and issue/comment fetching use pagination. Issue pages use ETag conditional requests and persistent cache. Manual sync is available; the active project refreshes every two minutes while connected. PR entries in the issue endpoint are excluded.

GitHub OPEN/CLOSED and AiFlow operational task state are independent. External closure is displayed as remote metadata, never converted into a validated local DONE. Queue/history records are preserved by refresh. Attempt preparation refuses an externally closed issue for manual review.

Issue descriptions and comments are untrusted prompt data. Raw HTML in Markdown is disabled, image content is not automatically loaded, and links go through a restricted official-domain opener. Tests and execution policies come from local project configuration, not issue bodies.

Default status sync is LocalOnly. Optional close-on-success is available per project after independent validation. There is no automatic creation of labels, issue comments, commits, pushes or Projects fields. CI validation reads check-runs; more than one page or no check-runs is conservatively not considered passed.

Remaining integrations: Enterprise host configuration, GitHub Projects field discovery/mapping, issue editing, linked PR enrichment, Actions and commit detail views, creation/removal/reconciliation of labels, organization-specific administration and live OAuth integration testing. These are not marked ready in the status document.

## Projects are GitHub repositories

Connecting a GitHub CLI account automatically refreshes Projects from the paginated authenticated `/user/repos` endpoint, including owned, collaborator and organization-member repositories visible to that account. The catalog is scoped by account login and stable GitHub repository ID. Refreshing handles repository renames, removes inaccessible repositories from the visible list and preserves local paths, execution policy and historical task data.

Selecting a repository immediately fetches its issues (`state=all`), without a local checkout. Issues defaults to List and exposes separate GitHub open/closed filtering. Pull requests are excluded. Complete successful responses replace the remote issue cache transactionally, removing stale remote rows while retaining queue/execution history. Failures or pagination safety limits preserve the prior cache. A local worktree is required only for enqueuing/running tasks; both frontend and backend enforce that boundary. Repo identity is read-only in local settings.

## Queue and dry run

Add to queue schedules an open issue for the selected provider; execution starts when the queue is started. Dry run checks the local Git worktree and provider health and returns the proposed prompt, validation commands and approval policy without invoking the agent. Both actions need a local folder. Issue details explains this prerequisite and opens the folder settings directly, returning to the same issue after saving. Closed GitHub issues explain why enqueue is unavailable.
