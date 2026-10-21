import { test, expect } from "@playwright/test";
import {
  defaultPolicy,
  type Snapshot,
  type ExecutionEvent,
} from "../../src/core/types";

// Test-only IPC fixture. It is never bundled with the application or used as real account evidence.
test("connected views, issue states and incremental IPC output", async ({
  page,
}) => {
  const at = new Date().toISOString();
  const project = {
    id: "fixture",
    name: "eNetwork",
    repository: "fixture/eNetwork",
    path: "C:/test/worktree",
    branch: "migration/postgresql-finalization",
    policy: defaultPolicy,
  };
  const titles = [
    "Gmail ingestion",
    "ePredit orchestration",
    "Jobs",
    "SQLite inventory",
    "Zero SQLite runtime",
    "PostgreSQL-only runtime",
    "Full regression",
    "Security hardening",
    "Production cutover",
  ];
  const states = [
    "SUCCESS",
    "SUCCESS",
    "SUCCESS",
    "SUCCESS",
    "SUCCESS",
    "RUNNING",
    "QUEUED",
    "FAILED",
    "BLOCKED",
  ];
  const issues = titles.map((title, n) => ({
    number: n + 2,
    title,
    body: `## Acceptance criteria\n\n- Validate ${title}\n- Preserve production data\n\n<script>alert('untrusted')</script>`,
    state: n < 5 ? "CLOSED" : "OPEN",
    labels: n === 8 ? ["production", "P0"] : ["P1", "runtime"],
    assignees: [],
    milestone: "PostgreSQL certification",
    url: `https://github.com/fixture/eNetwork/issues/${n + 2}`,
    updatedAt: at,
    comments: [],
  }));
  const queue = issues.map((i, n) => ({
    id: `q${n}`,
    projectId: "fixture",
    issueNumber: i.number,
    providerId: n === 6 ? "claude" : n === 7 ? "gemini" : "codex",
    position: n + 1,
    state: states[n],
    approved: false,
  }));
  const execution = {
    id: "fixture-execution",
    itemId: "q5",
    projectId: "fixture",
    providerId: "codex",
    state: "RUNNING",
    startedAt: at,
    finishedAt: null,
    reason: null,
    beforeGit: {
      branch: project.branch,
      head: "a".repeat(40),
      root: project.path,
      status: "",
    },
    afterGit: null,
    retryAt: null,
    attempt: 1,
    sessionId: null,
  };
  const snapshot: Snapshot = {
    projects: [project],
    issues: [{ projectId: "fixture", issues }],
    queue,
    executions: [execution],
    providers: ["codex", "claude", "gemini"].map((id) => ({
      id,
      name: id,
      status: "AVAILABLE",
      authentication: "CONNECTED",
      version: "test fixture",
      checkedAt: at,
      diagnostics: "UI test fixture only",
      installationUrl: "https://github.com",
      capabilities: {
        supportsResume: true,
        supportsStreaming: true,
        supportsStructuredOutput: true,
        supportsUsageInformation: false,
        supportsSessionId: true,
        supportsNonInteractive: true,
        supportsWorkingDirectory: true,
        supportsAutoApproval: false,
      },
    })),
    usage: [],
    runningProjects: ["fixture"],
    githubAccount: "fixture-user",
  };
  const event: ExecutionEvent = {
    eventId: "fixture-event",
    executionId: execution.id,
    attemptId: "fixture-execution:1",
    sequence: 1,
    timestamp: at,
    type: "OUTPUT_RECEIVED",
    payload: {
      stream: "stdout",
      content: "line 1 — arrived before completion\n",
    },
  };
  await page.addInitScript(
    ({ snapshot, event }) => {
      const w = window as unknown as {
        isTauri: boolean;
        __TAURI_INTERNALS__: unknown;
        __TAURI_EVENT_PLUGIN_INTERNALS__: unknown;
        testEmit: (e: unknown) => void;
      };
      let counter = 0;
      const callbacks = new Map<number, (e: unknown) => void>();
      const handlers = new Map<string, number>();
      w.isTauri = true;
      localStorage.setItem("aiflow.theme", "Dark");
      w.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
        unregisterListener: () => undefined,
      };
      w.__TAURI_INTERNALS__ = {
        transformCallback: (fn: (e: unknown) => void) => {
          const id = ++counter;
          callbacks.set(id, fn);
          return id;
        },
        unregisterCallback: (id: number) => callbacks.delete(id),
        invoke: async (command: string, args: Record<string, unknown>) => {
          if (command === "snapshot") return snapshot;
          if (command === "execution_event_window") return [event];
          if (command === "plugin:event|listen") {
            handlers.set(String(args.event), Number(args.handler));
            return ++counter;
          }
          if (command === "plugin:event|unlisten") return;
          throw new Error(`Unexpected test IPC: ${command}`);
        },
      };
      w.testEmit = (payload) => {
        const handler = handlers.get("aiflow:event");
        if (handler)
          callbacks.get(handler)?.({ event: "aiflow:event", id: 1, payload });
      };
    },
    { snapshot, event },
  );
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "eNetwork", exact: true }),
  ).toBeVisible();
  await page.screenshot({
    path: "test-results/connected-home-dark.png",
    fullPage: true,
  });
  const nav = page.getByRole("navigation");
  await nav.getByRole("button", { name: /^Issues/ }).click();
  await expect(page.locator(".page-top h1")).toHaveText("Issues");
  await expect(page.locator(".page")).toHaveCSS("opacity", "1");
  await expect(
    page.getByRole("heading", { name: "PostgreSQL-only runtime", exact: true }),
  ).toBeVisible();
  await page.screenshot({
    path: "test-results/connected-board-dark.png",
    fullPage: true,
  });
  await page.getByRole("button", { name: "List", exact: true }).click();
  await expect(page.locator(".issue-table")).toBeVisible();
  await page.screenshot({
    path: "test-results/connected-list-dark.png",
    fullPage: true,
  });
  await page
    .locator(".issue-table-row")
    .filter({ hasText: "PostgreSQL-only runtime" })
    .click();
  await expect(page.getByRole("dialog", { name: "Issue #7" })).toBeVisible();
  await expect(page.locator(".markdown script")).toHaveCount(0);
  await page.screenshot({
    path: "test-results/connected-issue-dark.png",
    fullPage: true,
  });
  await page.keyboard.press("Escape");
  await nav.getByRole("button", { name: /^Queue/ }).click();
  await expect(page.locator(".page-top h1")).toHaveText("Queue");
  await expect(page.locator(".page")).toHaveCSS("opacity", "1");
  await page.screenshot({
    path: "test-results/connected-queue-dark.png",
    fullPage: true,
  });
  await nav.getByRole("button", { name: "Executions", exact: true }).click();
  await expect(
    page.getByText("line 1 — arrived before completion", { exact: true }),
  ).toBeVisible();
  await page.evaluate(
    (e) =>
      (window as unknown as { testEmit: (e: unknown) => void }).testEmit(e),
    {
      ...event,
      eventId: "second-event",
      sequence: 2,
      payload: { stream: "stderr", content: "line 2 — stderr is visible\n" },
    },
  );
  await expect(
    page.getByText("line 2 — stderr is visible", { exact: true }),
  ).toBeVisible();
  await expect(page.locator(".execution-title-line .status")).toContainText(
    "RUNNING",
  );
  await page.screenshot({
    path: "test-results/connected-execution-dark.png",
    fullPage: true,
  });
  await nav.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByRole("button", { name: "Light", exact: true }).click();
  await nav.getByRole("button", { name: "Home", exact: true }).click();
  await page.screenshot({
    path: "test-results/connected-home-light.png",
    fullPage: true,
  });
  await page.setViewportSize({ width: 900, height: 760 });
  await page.screenshot({
    path: "test-results/connected-compact.png",
    fullPage: true,
  });
  expect(errors).toEqual([]);
});
