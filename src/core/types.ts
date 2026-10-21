export type ProviderId = "codex" | "claude" | "gemini" | "auto";
export interface ProjectPolicy {
  allowedProviders: string[];
  defaultProvider: string;
  failover: string;
  requireCleanTree: boolean;
  requireCommit: boolean;
  requireTests: boolean;
  requireCI: boolean;
  closeIssue: boolean;
  manualApprovalLabels: string[];
  testCommands: string[][];
  maxRetries: number;
}
export interface Project {
  id: string;
  name: string;
  repository: string;
  path: string;
  branch: string;
  policy: ProjectPolicy;
}
export interface Issue {
  number: number;
  title: string;
  body: string;
  state: string;
  labels: string[];
  assignees: string[];
  milestone: string | null;
  url: string;
  updatedAt: string;
  comments: string[];
}
export interface QueueItem {
  id: string;
  projectId: string;
  issueNumber: number;
  providerId: string;
  position: number;
  state: string;
  approved: boolean;
}
export interface GitSnapshot {
  branch: string;
  head: string;
  status: string;
  root: string;
}
export interface Execution {
  id: string;
  itemId: string;
  projectId: string;
  providerId: string;
  state: string;
  startedAt: string;
  finishedAt: string | null;
  reason: string | null;
  beforeGit: GitSnapshot | null;
  afterGit: GitSnapshot | null;
  retryAt: string | null;
  attempt: number;
  sessionId: string | null;
}
export interface Capabilities {
  supportsResume: boolean;
  supportsStructuredOutput: boolean;
  supportsStreaming: boolean;
  supportsUsageInformation: boolean;
  supportsSessionId: boolean;
  supportsNonInteractive: boolean;
  supportsWorkingDirectory: boolean;
  supportsAutoApproval: boolean;
}
export interface ProviderStatus {
  id: string;
  name: string;
  status: string;
  version: string | null;
  authentication: string;
  capabilities: Capabilities;
  checkedAt: string;
  diagnostics: string;
  installationUrl: string;
}
export interface Usage {
  providerId: string;
  detectedAt: string;
  resetAt: string | null;
  retryAfter: number | null;
  source: string;
  confidence: string;
}
export interface ExecutionEvent {
  eventId: string;
  executionId: string;
  attemptId: string;
  sequence: number;
  timestamp: string;
  type: string;
  payload: {
    stream?: string;
    content?: string;
    state?: string;
    reason?: string;
    formatted?: {
      display?: string;
      text?: string;
      type?: string;
      [key: string]: unknown;
    };
    [key: string]: unknown;
  };
}
export interface Snapshot {
  projects: Project[];
  issues: { projectId: string; issues: Issue[] }[];
  queue: QueueItem[];
  executions: Execution[];
  providers: ProviderStatus[];
  usage: Usage[];
  runningProjects: string[];
  githubAccount: string | null;
}
export const defaultPolicy: ProjectPolicy = {
  allowedProviders: ["codex", "claude", "gemini"],
  defaultProvider: "codex",
  failover: "ask",
  requireCleanTree: true,
  requireCommit: true,
  requireTests: true,
  requireCI: false,
  closeIssue: false,
  manualApprovalLabels: ["production", "deployment", "manual-approval"],
  testCommands: [],
  maxRetries: 3,
};
export const providerNames: Record<string, string> = {
  auto: "Automatic",
  codex: "OpenAI Codex",
  claude: "Anthropic Claude",
  gemini: "Google Gemini",
};
export type TaskState =
  "TODO" | "IN_PROGRESS" | "DONE" | "WAITING" | "FAILED" | "BLOCKED" | "PAUSED";
export function taskState(item?: QueueItem): TaskState {
  if (!item) return "TODO";
  if (item.state === "SUCCESS") return "DONE";
  if (["PREPARING", "RUNNING", "VALIDATING", "COMMITTING"].includes(item.state))
    return "IN_PROGRESS";
  if (
    ["WAITING_FOR_PROVIDER", "WAITING_FOR_QUOTA", "RETRY_WAIT"].includes(
      item.state,
    )
  )
    return "WAITING";
  if (item.state === "FAILED") return "FAILED";
  if (["BLOCKED", "HUMAN_REVIEW", "CANCELLED"].includes(item.state))
    return "BLOCKED";
  if (item.state === "PAUSED") return "PAUSED";
  return "TODO";
}
export function countdown(at: string | null | undefined, now: number): string {
  if (!at) return "Unknown";
  const delta = new Date(at).getTime() - now;
  if (!Number.isFinite(delta)) return "Unknown";
  const seconds = Math.max(0, Math.ceil(delta / 1000));
  return [
    Math.floor(seconds / 3600),
    Math.floor(seconds / 60) % 60,
    seconds % 60,
  ]
    .map((n) => String(n).padStart(2, "0"))
    .join(":");
}
// Terminal controls are never interpreted as HTML or clickable escape sequences.
export function terminalText(value: string): string {
  // Control ranges are intentional: strip terminal instructions, not printable text.
  /* eslint-disable no-control-regex */
  return value
    .replace(/\u001b\][\s\S]*?(?:\u0007|\u001b\\)/g, "")
    .replace(/\u001b\[[0-?]*[ -/]*[@-~]/g, "")
    .replace(/\r(?!\n)/g, "\n")
    .replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/g, "");
  /* eslint-enable no-control-regex */
}
export function mergeEvents(
  current: ExecutionEvent[],
  incoming: ExecutionEvent[],
): ExecutionEvent[] {
  const unique = new Map(current.map((e) => [e.sequence, e]));
  for (const event of incoming) unique.set(event.sequence, event);
  return [...unique.values()].sort((a, b) => a.sequence - b.sequence);
}

export function boundedEvents(events: ExecutionEvent[]): ExecutionEvent[] {
  const output = events
    .filter((e) => e.type === "OUTPUT_RECEIVED")
    .slice(-10000);
  const timeline = events.filter((e) => e.type !== "OUTPUT_RECEIVED");
  return [...timeline, ...output].sort((a, b) => a.sequence - b.sequence);
}
