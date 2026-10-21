# GitHub integration

GitHub.com REST is implemented directly in the Rust backend. Repository listing and issue/comment fetching use pagination. Issue pages use ETag conditional requests and persistent cache. Manual sync is available; the active project refreshes every two minutes while connected. PR entries in the issue endpoint are excluded.

GitHub OPEN/CLOSED and AiFlow operational task state are independent. External closure is displayed as remote metadata, never converted into a validated local DONE. Queue/history records are preserved by refresh. Attempt preparation refuses an externally closed issue for manual review.

Issue descriptions and comments are untrusted prompt data. Raw HTML in Markdown is disabled, image content is not automatically loaded, and links go through a restricted official-domain opener. Tests and execution policies come from local project configuration, not issue bodies.

Default status sync is LocalOnly. Optional close-on-success is available per project after independent validation. There is no automatic creation of labels, issue comments, commits, pushes or Projects fields. CI validation reads check-runs; more than one page or no check-runs is conservatively not considered passed.

Remaining integrations: Enterprise host configuration, GitHub Projects field discovery/mapping, issue editing, linked PR enrichment, Actions and commit detail views, creation/removal/reconciliation of labels, organization-specific administration and live OAuth integration testing. These are not marked ready in the status document.
