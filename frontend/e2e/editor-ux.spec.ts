import { expect, test, type Page } from "@playwright/test";

const MOD = process.platform === "darwin" ? "Meta" : "Control";
const USER = {
  email: process.env.E2E_USER_A_EMAIL ?? "alice@example.com",
  password: process.env.E2E_USER_A_PASSWORD ?? "password123",
};

async function login(page: Page) {
  await page.goto("/login");
  await page.getByLabel(/email/i).fill(USER.email);
  await page.getByLabel(/password/i).fill(USER.password);
  await page.getByRole("button", { name: /log in|sign in/i }).click();
  await page.waitForURL(/\/$/);
}

async function createDocument(page: Page, title: string) {
  await page.goto("/");
  await page.getByRole("button", { name: /new document/i }).click();
  await page.waitForURL(/\/documents\//);
  const titleInput = page.getByPlaceholder("Untitled");
  await titleInput.fill(title);
  await titleInput.blur();
  await expect(titleInput).toHaveValue(title);
  return page.url();
}

test.describe("Editor UX", () => {
  test("toolbar bold action wraps the current selection", async ({ page }) => {
    await login(page);
    await createDocument(page, "Toolbar test");

    const editor = page.locator(".cm-editor-container .cm-editor");
    await editor.click();
    await page.keyboard.type("Hello world");
    await page.keyboard.press(`${MOD}+A`);
    await page.getByLabel("Bold").click();

    await expect(page.locator(".cm-content")).toContainText("**Hello world**");
  });

  test("focus mode hides editor chrome and exits with escape", async ({ page }) => {
    await login(page);
    await createDocument(page, "Focus mode test");

    await page.keyboard.press(`${MOD}+Shift+.`);

    await expect(page.getByText("Drafthouse")).toHaveCount(0);
    await expect(page.getByTestId("editor-toolbar")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Exit focus mode" })).toBeVisible();

    await page.keyboard.press("Escape");

    await expect(page.getByText("Drafthouse")).toBeVisible();
    await expect(page.getByTestId("editor-toolbar")).toBeVisible();
  });

  test("font choice applies immediately and persists across reload", async ({ page }) => {
    await login(page);
    await createDocument(page, "Font preference test");

    await page.getByLabel("Editor font").selectOption("lora");

    await expect(page.getByTestId("editor-container")).toHaveClass(/font-serif/);
    await expect(page.locator(".cm-content")).toHaveCSS("font-family", /Lora/);

    await page.reload();

    await expect(page.getByLabel("Editor font")).toHaveValue("lora");
    await expect(page.getByTestId("editor-container")).toHaveClass(/font-serif/);
    await expect(page.locator(".cm-content")).toHaveCSS("font-family", /Lora/);
  });

  test("command palette filters documents and navigates on enter", async ({ page }) => {
    await login(page);
    await createDocument(page, "Alpha doc");
    await createDocument(page, "Beta search target");

    await page.goto("/");
    await page.keyboard.press(`${MOD}+K`);

    const input = page.getByLabel("Search documents");
    await expect(input).toBeVisible();
    await input.fill("Beta");
    await page.keyboard.press("Enter");

    await expect(page).toHaveURL(/\/documents\//);
    await expect(page.locator('input[value="Beta search target"]')).toBeVisible();
  });

  test("sidebar toggles with the global shortcut", async ({ page }) => {
    await login(page);
    await page.goto("/");

    await expect(page.getByText("Drafthouse")).toBeVisible();
    await page.keyboard.press(`${MOD}+Shift+\\`);
    await expect(page.getByText("Drafthouse")).toHaveCount(0);
    await page.keyboard.press(`${MOD}+Shift+\\`);
    await expect(page.getByText("Drafthouse")).toBeVisible();
  });
});
