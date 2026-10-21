# GitHub integration

GitHub.com REST is implemented directly in the Rust backend. Repository listing and issue/comment fetching use pagination. Issue pages use ETag conditional requests and persistent cache. Manual sync is available; the active project refreshes every two minutes while connected. PR entries in the issue endpoint are excluded.

GitHub OPEN/CLOSED and AiFlow operational task state are independent. External closure is displayed as remote metadata, never converted into a validated local DONE. Queue/history records are preserved by refresh. Attempt preparation refuses an externally closed issue for manual review.

Issue descriptions and comments are untrusted prompt data. Raw HTML in Markdown is disabled, image content is not automatically loaded, and links go through a restricted official-domain opener. Tests and execution policies come from local project configuration, not issue bodies.

Validation evidence is posted to the source GitHub issue and must be confirmed before SUCCESS releases the next task. Optional close-on-success remains project policy. Managed local checkouts publish validated commits to an isolated work branch without force and verify the remote SHA. Linked worktrees retain local-only Git publication. The controller does not merge PRs, deploy or rewrite issue criteria. CI validation reads check-runs; missing checks and pagination limits are not considered passed.

Remaining integrations: Enterprise host configuration, GitHub Projects field discovery/mapping, issue editing, linked PR enrichment, Actions and commit detail views, creation/removal/reconciliation of labels, organization-specific administration and live OAuth integration testing. These are not marked ready in the status document.

## Projects are GitHub repositories

Connecting a GitHub CLI account automatically refreshes Projects from the paginated authenticated `/user/repos` endpoint, including owned, collaborator and organization-member repositories visible to that account. The catalog is scoped by account login and stable GitHub repository ID. Refreshing handles repository renames, removes inaccessible repositories from the visible list and preserves local paths, execution policy and historical task data.

Selecting a repository immediately fetches its issues (`state=all`), without a local checkout. Issues defaults to List and exposes separate GitHub open/closed filtering. Pull requests are excluded. Successful responses replace the remote issue cache transactionally while retaining execution history; failures preserve the previous cache. Enqueue needs no local folder. Local execution can prepare a managed checkout; provider-cloud tracking does not clone locally. Repo identity is read-only in local settings.

## Queue and dry run

Add to queue schedules an open issue without requiring a folder. Start queue asks for local computer or provider cloud. Dry run previews workspace requirements and provider health; Git is inspected only when a folder is already linked. Validation policy can be configured before a checkout exists. Closed GitHub issues explain why enqueue is unavailable.

Cloud mode currently supports Codex with a configured environment ID, published base branch and exact required GitHub check names. An execution-specific branch and PR must exist in the expected repository, and all configured checks must pass on the exact published commit. A provider completion message is insufficient. If the provider leaves only a diff, the user must publish it from the provider task. Cloud references persist across restart without resubmission. Ambiguous launches or publication failures require review; stopping tracking does not cancel provider compute. Claude/Gemini cloud dispatch is unavailable in this build.
