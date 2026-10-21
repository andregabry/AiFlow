import { test, expect } from "@playwright/test";
import type { QueueItem } from "../../src/core/types";

test("Projects lists GitHub repositories and selecting one loads its issues without a local folder", async ({
  page,
}) => {
  await page.addInitScript(() => {
    const w = window as unknown as {
      isTauri: boolean;
      __TAURI_INTERNALS__: unknown;
      __TAURI_EVENT_PLUGIN_INTERNALS__: unknown;
      issueRequests: string[];
      runtimeRequests: Record<string, unknown>[];
    };
    w.isTauri = true;
    w.issueRequests = [];
    w.runtimeRequests = [];
    localStorage.clear();
    localStorage.setItem("aiflow.theme", "Dark");
    const policy = {
      allowedProviders: ["codex"],
      defaultProvider: "codex",
      failover: "ask",
      requireCleanTree: true,
      requireCommit: true,
      requireTests: true,
      requireCI: false,
      closeIssue: false,
      manualApprovalLabels: [],
      testCommands: [],
      maxRetries: 3,
    };
    const repos = [
      {
        id: "repo-library",
        name: "sample/library",
        repository: "sample/library",
        path: "",
        branch: "main",
        policy,
      },
      {
        id: "repo-tools",
        name: "sample/tools",
        repository: "sample/tools",
        path: "",
        branch: "main",
        policy,
      },
    ];
    let projects: typeof repos = [];
    const issue = (number: number, title: string, state = "OPEN") => ({
      number,
      title,
      state,
      body: "GitHub issue content",
      labels: [],
      assignees: [],
      milestone: null,
      url: `https://github.com/sample/tools/issues/${number}`,
      updatedAt: new Date().toISOString(),
      comments: [],
    });
    const issues = new Map<string, ReturnType<typeof issue>[]>();
    const queue: QueueItem[] = [];
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener: () => undefined,
    };
    w.__TAURI_INTERNALS__ = {
      transformCallback: () => 1,
      unregisterCallback: () => undefined,
      invoke: async (command: string, args: Record<string, unknown>) => {
        if (command === "plugin:event|listen") return 1;
        if (command === "plugin:event|unlisten") return;
        if (command === "snapshot")
          return {
            projects,
            issues: [...issues].map(([projectId, issues]) => ({
              projectId,
              issues,
            })),
            queue,
            executions: [],
            providers: [],
            usage: [],
            runningProjects: [],
            githubAccount: "sample",
          };
        if (command === "sync_repositories") {
          projects = repos;
          return repos.length;
        }
        if (command === "sync_issues") {
          const id = String(args.projectId);
          w.issueRequests.push(id);
          issues.set(
            id,
            id === "repo-tools"
              ? [
                  issue(201, "Open tools issue"),
                  issue(202, "Closed tools issue", "CLOSED"),
                ]
              : [issue(101, "Library issue")],
          );
          return issues.get(id)?.length;
        }
        if (command === "queue_runtime")
          return {
            mode: "local",
            environmentId: "",
            baseBranch: "main",
            requiredChecks: [],
          };
        if (command === "queue_control") {
          w.runtimeRequests.push(args.runtime as Record<string, unknown>);
          return;
        }
        if (command === "save_project") {
          const project = args.project as (typeof repos)[number];
          projects = projects.map((p) => (p.id === project.id ? project : p));
          return project;
        }
        if (command === "dry_run") {
          return {
            prompt: "Preview only: agent has not run",
            validationCommands: [],
          };
        }
        if (command === "enqueue") {
          queue.push({
            id: "queued-issue",
            projectId: String(args.projectId),
            issueNumber: Number(args.number),
            providerId: "codex",
            position: 1,
            state: "QUEUED",
            approved: false,
          });
          return;
        }
        throw new Error(`Unexpected repository IPC: ${command}`);
      },
    };
  });
  await page.goto("/");
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Projects", exact: true })
    .click();
  await expect(page.locator(".project-card")).toHaveCount(2);
  await expect(
    page.getByRole("heading", { name: "sample/tools", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "New project", exact: true }),
  ).toHaveCount(0);
  await page
    .locator(".project-card")
    .filter({
      has: page.getByRole("heading", { name: "sample/tools", exact: true }),
    })
    .click();
  await expect(page.locator(".page-top h1")).toHaveText("Issues");
  await expect(page.locator(".issue-table-row")).toHaveCount(2);
  await expect(page.getByText("Library issue", { exact: true })).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Link local folder", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => (window as unknown as { issueRequests: string[] }).issueRequests,
    ),
  ).toContain("repo-tools");
  await page.getByLabel("GitHub issue state").selectOption("CLOSED");
  await expect(page.locator(".issue-table-row")).toHaveCount(1);
  await expect(page.locator(".issue-table-row")).toContainText(
    "Closed tools issue",
  );
  await page.getByLabel("GitHub issue state").selectOption("OPEN");
  await page.locator(".issue-table-row").click();
  await expect(page.getByRole("dialog", { name: "Issue #201" })).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Add to queue", exact: true }),
  ).toBeEnabled();
  const detail = page.getByRole("dialog", { name: "Issue #201" });
  await expect(
    detail.getByRole("button", { name: "Dry run", exact: true }),
  ).toBeEnabled();
  await expect(detail.getByRole("status")).toContainText(
    "No local folder is required",
  );
  await detail
    .getByRole("button", { name: "Execution settings", exact: true })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Project settings" }),
  ).toBeVisible();
  await expect(
    page.getByLabel("GitHub repository", { exact: false }).first(),
  ).toHaveValue("sample/tools");
  await page
    .getByLabel("Independent validation commands", { exact: false })
    .fill('[["git","status"]]');
  await page.getByRole("button", { name: "Save project", exact: true }).click();
  await expect(detail).toBeVisible();
  await expect(
    detail.getByRole("button", { name: "Add to queue", exact: true }),
  ).toBeEnabled();
  await detail.getByRole("button", { name: "Dry run", exact: true }).click();
  await expect(detail.locator(".dry-run")).toContainText(
    "Preview only: agent has not run",
  );
  await expect(detail.getByText("Not queued", { exact: true })).toBeVisible();
  await detail
    .getByRole("button", { name: "Add to queue", exact: true })
    .click();
  await expect(
    detail.getByRole("button", { name: "Add to queue", exact: true }),
  ).toHaveCount(0);
  await expect(
    detail.getByRole("button", { name: "Retry", exact: true }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Start queue", exact: true }).click();
  const launch = page.getByRole("dialog", { name: "Start queue", exact: true });
  await expect(launch.getByLabel("Execution environment")).toHaveValue("local");
  await expect(launch).toContainText(
    "AiFlow clones the repository into its own workspace",
  );
  await expect(
    launch.getByRole("button", { name: "Start local queue", exact: true }),
  ).toBeEnabled();
  await launch
    .getByRole("button", { name: "Start local queue", exact: true })
    .click();
  await expect(launch).toHaveCount(0);
  await page.getByRole("button", { name: "Start queue", exact: true }).click();
  await launch.getByLabel("Execution environment").selectOption("cloud");
  await expect(
    launch.getByRole("button", { name: "Start cloud queue", exact: true }),
  ).toBeDisabled();
  await launch.getByLabel("Codex cloud environment ID").fill("env_fixture");
  await launch.getByLabel("Published base branch").fill("main");
  await launch.getByLabel("Required GitHub check names").fill("tests\nbuild");
  await expect(launch).toHaveCSS("opacity", "1");
  await expect(launch.locator("..")).toHaveCSS("opacity", "1");
  await page.screenshot({ path: "test-results/queue-environment-cloud.png" });
  await launch
    .getByRole("button", { name: "Start cloud queue", exact: true })
    .click();
  await expect(launch).toHaveCount(0);
  const requests = await page.evaluate(
    () =>
      (window as unknown as { runtimeRequests: Record<string, unknown>[] })
        .runtimeRequests,
  );
  expect(requests.map((r) => r.mode)).toEqual(["local", "cloud"]);
  expect(requests[1].requiredChecks).toEqual(["tests", "build"]);
  await page.screenshot({ path: "test-results/github-repository-issues.png" });
});
