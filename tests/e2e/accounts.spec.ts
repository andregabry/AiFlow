import { test, expect, type Page } from "@playwright/test";

async function accountFixture(
  page: Page,
  failGitHub = false,
  githubCli = false,
) {
  await page.addInitScript(
    ({ failGitHub, githubCli }) => {
      const fixture = window as unknown as {
        isTauri: boolean;
        __TAURI_INTERNALS__: unknown;
        __TAURI_EVENT_PLUGIN_INTERNALS__: unknown;
        finishLogin: (success: boolean) => void;
        commands: string[];
        internalCodes: string[];
        providerWindows: string[];
        forceUnknown: boolean;
        useConnected: () => void;
      };
      const callbacks = new Map<number, (event: unknown) => void>();
      const listeners = new Map<string, number>();
      let counter = 0,
        connected = false,
        providerConnected = false;
      let resolveLogin: (() => void) | undefined,
        rejectLogin: ((reason: string) => void) | undefined;
      const statuses = () =>
        ["codex", "claude", "gemini"].map((id) => ({
          id,
          name: id,
          status: "AVAILABLE",
          authentication:
            id === "gemini" && (fixture.forceUnknown || !providerConnected)
              ? "UNKNOWN"
              : providerConnected
                ? "CONNECTED"
                : "DISCONNECTED",
          version: "fixture",
          diagnostics: "Test fixture",
          checkedAt: new Date().toISOString(),
          installationUrl: "https://github.com",
          capabilities: {},
        }));
      fixture.isTauri = true;
      fixture.commands = [];
      fixture.internalCodes = [];
      fixture.providerWindows = [];
      fixture.forceUnknown = false;
      fixture.useConnected = () => {
        providerConnected = true;
      };
      fixture.finishLogin = (success) => {
        if (success) {
          providerConnected = true;
          if (githubCli) connected = true;
          resolveLogin?.();
        } else rejectLogin?.("Official sign-in failed. Please try again.");
      };
      localStorage.setItem("aiflow.theme", "Dark");
      fixture.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
        unregisterListener: () => undefined,
      };
      fixture.__TAURI_INTERNALS__ = {
        transformCallback: (callback: (event: unknown) => void) => {
          callbacks.set(++counter, callback);
          return counter;
        },
        unregisterCallback: (id: number) => callbacks.delete(id),
        invoke: async (command: string, args: Record<string, unknown>) => {
          fixture.commands.push(command);
          if (command === "plugin:event|listen") {
            listeners.set(String(args.event), Number(args.handler));
            return ++counter;
          }
          if (command === "plugin:event|unlisten") return;
          if (command === "sync_repositories" || command === "sync_issues")
            return 0;
          if (command === "snapshot")
            return {
              projects: [],
              issues: [],
              queue: [],
              executions: [],
              providers: statuses(),
              usage: [],
              runningProjects: [],
              githubAccount: connected ? "fixture-user" : null,
            };
          if (command === "execution_event_window") return [];
          if (command === "scan_providers") return statuses();
          if (command === "github_cli_start") {
            if (failGitHub)
              throw new Error(
                "Install the official GitHub CLI before connecting.",
              );
            if (!githubCli) {
              connected = true;
              return { method: "existing" };
            }
            return { method: "cli" };
          }
          if (command === "provider_auth_window_open") {
            fixture.providerWindows.push(String(args.url));
            return;
          }
          if (command === "provider_auth_window_close") return;
          if (command === "github_auth_window_open") {
            fixture.internalCodes.push(String(args.code));
            return;
          }
          if (command === "github_auth_window_close") return;
          if (command === "github_device_cancel") return;
          if (command === "cancel_account_connection") {
            rejectLogin?.("Account connection cancelled");
            return;
          }
          if (command === "provider_auth" || command === "github_cli_login") {
            const handler = listeners.get("aiflow:event");
            if (handler)
              callbacks.get(handler)?.({
                event: "aiflow:event",
                id: 1,
                payload: {
                  eventId: "auth-output",
                  executionId: args.executionId,
                  attemptId: `${args.executionId}:1`,
                  sequence: ++counter,
                  timestamp: new Date().toISOString(),
                  type: "OUTPUT_RECEIVED",
                  payload: {
                    stream: "stdout",
                    content: githubCli
                      ? "! One-time code (AIFL-5678) copied to clipboard\nOpen this URL to continue in your web browser: https://github.com/login/device\n"
                      : args.providerId === "claude"
                        ? "Open https://claude.ai/oauth/authorize?client_id=fixture\n"
                        : args.providerId === "gemini"
                          ? "Open https://accounts.google.com/o/oauth2/v2/auth?client_id=fixture\n"
                          : "Open https://auth.openai.com/oauth/authorize?client_id=fixture\n",
                  },
                },
              });
            return new Promise<void>((resolve, reject) => {
              resolveLogin = resolve;
              rejectLogin = reject;
            });
          }
          throw new Error(`Unexpected fixture command: ${command}`);
        },
      };
    },
    { failGitHub, githubCli },
  );
  await page.goto("/");
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Accounts", exact: true })
    .click();
  await expect(page.locator(".page-top h1")).toHaveText("Accounts");
}

test("GitHub reuses the existing official CLI account without another consent flow", async ({
  page,
}) => {
  await accountFixture(page);
  await page
    .getByRole("button", { name: "Connect GitHub", exact: true })
    .click();
  const modal = page.getByRole("dialog");
  await modal.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(
    modal.getByRole("heading", { name: "GitHub is connected" }),
  ).toBeVisible();
  expect(
    await page.evaluate(() =>
      (window as unknown as { commands: string[] }).commands.includes(
        "github_cli_login",
      ),
    ),
  ).toBe(false);
});

test("provider login is a form without a terminal, and cancellation stops sign-in", async ({
  page,
}) => {
  await accountFixture(page);
  const card = page.locator(".account-section").filter({
    has: page.getByRole("heading", { name: "OpenAI Codex", exact: true }),
  });
  // Names are supplied by the adapter; this fixture exposes its own display names.
  const providerCard = (await card.count())
    ? card
    : page.locator(".account-section").filter({
        has: page.getByRole("heading", { name: "codex", exact: true }),
      });
  await providerCard
    .getByRole("button", { name: "Connect", exact: true })
    .click();
  const modal = page.getByRole("dialog");
  await modal.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(
    modal.getByRole("heading", { name: "Sign in to codex" }),
  ).toBeVisible();
  await expect(modal.locator(".console")).toHaveCount(0);
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as unknown as { providerWindows: string[] }).providerWindows
            .length,
      ),
    )
    .toBe(1);
  await expect(modal.locator('[aria-current="step"]')).toContainText(
    "Authorize",
  );
  await page.screenshot({
    path: "test-results/account-provider-output-modal.png",
  });
  await modal.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(modal).not.toBeVisible();
  expect(
    await page.evaluate(() =>
      (window as unknown as { commands: string[] }).commands.includes(
        "cancel_account_connection",
      ),
    ),
  ).toBe(true);
  await providerCard
    .getByRole("button", { name: "Connect", exact: true })
    .click();
  await expect(
    page
      .getByRole("dialog")
      .getByText(
        "Open https://auth.openai.com/oauth/authorize?client_id=fixture",
        { exact: true },
      ),
  ).toHaveCount(0);
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Continue", exact: true })
    .click();
  await page.evaluate(() =>
    (
      window as unknown as { finishLogin: (success: boolean) => void }
    ).finishLogin(true),
  );
  await expect(
    page
      .getByRole("dialog")
      .getByRole("heading", { name: "codex is connected" }),
  ).toBeVisible();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Done", exact: true })
    .click();
});

test("sign-in errors stay in the modal and allow retry; unknown auth never becomes connected", async ({
  page,
}) => {
  await accountFixture(page, true);
  await page
    .getByRole("button", { name: "Connect GitHub", exact: true })
    .click();
  let modal = page.getByRole("dialog");
  await modal.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(modal.getByRole("alert")).toContainText(
    "Install the official GitHub CLI",
  );
  await expect(page.locator(".error-banner")).toHaveCount(0);
  await expect(
    modal.getByRole("button", { name: "Continue", exact: true }),
  ).toBeEnabled();
  await page.keyboard.press("Escape");
  await expect(modal).not.toBeVisible();
  await page
    .locator(".account-section")
    .filter({ has: page.getByRole("heading", { name: "gemini", exact: true }) })
    .getByRole("button", { name: "Connect", exact: true })
    .click();
  modal = page.getByRole("dialog");
  await modal.getByRole("button", { name: "Continue", exact: true }).click();
  await page.evaluate(() => {
    const f = window as unknown as {
      forceUnknown: boolean;
      finishLogin: (success: boolean) => void;
    };
    f.forceUnknown = true;
    f.finishLogin(true);
  });
  await expect(modal.getByRole("alert")).toContainText(
    "cannot confirm sign-in",
  );
  await expect(
    modal.getByRole("button", { name: "Done", exact: true }),
  ).toHaveCount(0);
});

test("build without publisher OAuth uses official GitHub CLI and verifies before connecting", async ({
  page,
}) => {
  await accountFixture(page, false, true);
  await page
    .getByRole("button", { name: "Connect GitHub", exact: true })
    .click();
  const modal = page.getByRole("dialog");
  await modal.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(
    modal.getByRole("heading", {
      name: "Enter your code on GitHub",
      exact: true,
    }),
  ).toBeVisible();
  await expect(modal.getByText("AIFL-5678", { exact: true })).toBeVisible();
  await expect(
    modal.getByRole("button", {
      name: "Reopen GitHub sign-in window",
      exact: true,
    }),
  ).toBeVisible();
  await expect(modal.getByText("AIFL-5678", { exact: true })).toBeVisible();
  await expect(
    modal.getByRole("heading", { name: "GitHub is connected" }),
  ).toHaveCount(0);
  await expect
    .poll(() =>
      page.evaluate(
        () => (window as unknown as { internalCodes: string[] }).internalCodes,
      ),
    )
    .toEqual(["AIFL-5678"]);
  expect(
    await page.evaluate(() =>
      (window as unknown as { commands: string[] }).commands.some((c) =>
        c.startsWith("plugin:opener"),
      ),
    ),
  ).toBe(false);
  await modal
    .getByRole("button", { name: "Reopen GitHub sign-in window", exact: true })
    .click();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as unknown as { internalCodes: string[] }).internalCodes
            .length,
      ),
    )
    .toBe(2);
  await expect(modal.locator(".console")).toHaveCount(0);
  await expect(
    modal.getByText("Connection output", { exact: true }),
  ).toHaveCount(0);
  await page.screenshot({ path: "test-results/account-github-cli-modal.png" });
  await page.evaluate(() =>
    (
      window as unknown as { finishLogin: (success: boolean) => void }
    ).finishLogin(true),
  );
  await expect(
    modal.getByRole("heading", { name: "GitHub is connected" }),
  ).toBeVisible();
  await expect(modal.getByText(/Signed in as fixture-user/)).toBeVisible();
});

for (const provider of ["claude", "gemini"]) {
  test(`${provider} uses the official CLI and a form without terminal output`, async ({
    page,
  }) => {
    await accountFixture(page);
    await page
      .locator(".account-section")
      .filter({
        has: page.getByRole("heading", { name: provider, exact: true }),
      })
      .getByRole("button", { name: "Connect", exact: true })
      .click();
    const modal = page.getByRole("dialog");
    await modal.getByRole("button", { name: "Continue", exact: true }).click();
    await expect(
      modal.getByRole("heading", { name: `Sign in to ${provider}` }),
    ).toBeVisible();
    await expect(modal.locator(".console")).toHaveCount(0);
    if (provider === "gemini") {
      await expect(
        modal.getByText(/Google sign-in page opens in your external browser/),
      ).toBeVisible();
      expect(
        await page.evaluate(
          () =>
            (window as unknown as { providerWindows: string[] }).providerWindows
              .length,
        ),
      ).toBe(0);
    } else
      await expect
        .poll(() =>
          page.evaluate(
            () =>
              (window as unknown as { providerWindows: string[] })
                .providerWindows.length,
          ),
        )
        .toBe(1);
    await page.evaluate(() =>
      (
        window as unknown as { finishLogin: (success: boolean) => void }
      ).finishLogin(true),
    );
    await expect(
      modal.getByRole("heading", { name: `${provider} is connected` }),
    ).toBeVisible();
  });
}
test("an authenticated provider CLI is reused without starting login", async ({
  page,
}) => {
  await accountFixture(page);
  await page.evaluate(() =>
    (window as unknown as { useConnected: () => void }).useConnected(),
  );
  await page
    .locator(".account-section")
    .filter({ has: page.getByRole("heading", { name: "codex", exact: true }) })
    .getByRole("button", { name: "Connect", exact: true })
    .click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Continue", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "codex is connected" }),
  ).toBeVisible();
  expect(
    await page.evaluate(() =>
      (window as unknown as { commands: string[] }).commands.includes(
        "provider_auth",
      ),
    ),
  ).toBe(false);
});
