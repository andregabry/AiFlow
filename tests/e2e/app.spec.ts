import { test, expect } from "@playwright/test";
test("offline onboarding, navigation, command palette and themes", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Good work, in flow." }),
  ).toBeVisible();
  await expect(
    page.getByText("Browser preview · backend offline"),
  ).toBeVisible();
  await page.screenshot({ path: "test-results/home-dark.png", fullPage: true });
  for (const name of [
    "Projects",
    "Issues",
    "Queue",
    "Executions",
    "Providers",
    "Usage",
    "Accounts",
    "Logs",
    "Settings",
  ]) {
    await page
      .getByRole("navigation")
      .getByRole("button", { name, exact: true })
      .click();
    await expect(
      page.getByRole("heading", {
        name:
          name === "Providers"
            ? "AI Providers"
            : name === "Executions"
              ? "Execution Center"
              : name,
        exact: true,
      }),
    ).toBeVisible();
    await page.screenshot({
      path: `test-results/${name.toLowerCase()}-dark.png`,
      fullPage: true,
    });
  }
  await page.getByRole("button", { name: "Light", exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await page.screenshot({
    path: "test-results/settings-light.png",
    fullPage: true,
  });
  await page.keyboard.press("Control+k");
  await expect(
    page.getByRole("dialog", { name: "Find your flow" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).not.toBeVisible();
  expect(errors).toEqual([]);
});
test("repository browsing requires GitHub instead of creating a local project", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("button", { name: "Browse GitHub repositories" })
    .click();
  await expect(page.locator(".page-top h1")).toHaveText("Projects");
  await expect(
    page.getByRole("heading", { name: "Your GitHub repositories" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "New project", exact: true }),
  ).toHaveCount(0);
  await page
    .getByRole("button", { name: "Connect GitHub", exact: true })
    .click();
  await expect(page.locator(".page-top h1")).toHaveText("Accounts");
});
