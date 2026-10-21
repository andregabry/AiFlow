# Execution lifecycle

QUEUED → PREPARING → RUNNING → VALIDATING → SUCCESS. Explicit failure branches include BLOCKED, HUMAN_REVIEW, FAILED, CANCELLED, WAITING_FOR_PROVIDER and RETRY_WAIT. The central transition table refuses RUNNING → SUCCESS.

Before launch: resolve the provider through project allow-list; fetch the current GitHub issue so approval labels and remote state are fresh; enforce approval gates and validation configuration; obtain an OS worktree lease; capture canonical root, expected branch, HEAD and status; probe capabilities and independent authentication; inspect known quota state; refresh comments; build a policy-scoped prompt. Before completion, refetch the issue and require review if its remote state or updated timestamp changed during execution.

While running: read stdout and stderr concurrently, redact secrets, parse provider output inside the adapter, persist ordered events before notifying UI, retain a bounded failure tail, and capture an explicit resume handle when provided. Cancellation preserves Git changes and does not claim completion.

On provider failure, independently recheck authentication before classifying AUTH_FAILURE. Usage limits schedule a provider-specific waiting state rather than FAILED. Unknown reset uses a 15-minute next retry, labelled independently of reset time. Network/provider failures use bounded exponential retry. The default maxRetries is 3; exceeding the cap requires review.

Quota retry preserves the original HEAD baseline and tells the next attempt to inspect partial work. A changed branch always needs review. Explicit provider changes invalidate manual approval, retain attempt history and require review of a dirty worktree; there is no silent fallback.

Completion checks process exit, structured error result, unchanged branch, configured independent commands, clean tree, descendant commit and optional GitHub CI. Tests stream through the same event path. Policy may close the GitHub issue only afterward. No automatic push or deployment is performed. Acceptance criteria needing product, manual or external validation are not mechanically proved by tests; configure sufficient trusted validation before unattended use.

Pause stops future tasks. Stop-after-current leaves the active task running and stops subsequent dispatch. Close during an active attempt hides to tray. Tray quit refuses while an execution is active. Crash recovery pauses queues and requires review of interrupted active states; quota waits and past output persist.
