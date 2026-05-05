import { test, expect, type Browser, type BrowserContext } from "@playwright/test";

/** Credentials must be pre-seeded in the test DB via fixtures or env. */
const USER_A = {
  email: process.env.E2E_USER_A_EMAIL ?? "alice@example.com",
  password: process.env.E2E_USER_A_PASSWORD ?? "password123",
};
const USER_B = {
  email: process.env.E2E_USER_B_EMAIL ?? "bob@example.com",
  password: process.env.E2E_USER_B_PASSWORD ?? "password123",
};

async function loginAs(
  context: BrowserContext,
  credentials: { email: string; password: string }
): Promise<void> {
  const page = await context.newPage();
  await page.goto("/login");
  await page.getByLabel(/email/i).fill(credentials.email);
  await page.getByLabel(/password/i).fill(credentials.password);
  await page.getByRole("button", { name: /log in|sign in/i }).click();
  await page.waitForURL(/\/$/);
  await page.close();
}

async function createDoc(context: BrowserContext): Promise<string> {
  const page = await context.newPage();
  await page.goto("/");
  await page.getByRole("button", { name: /new document/i }).click();
  await page.waitForURL(/\/documents\//);
  const docId = page.url().split("/documents/")[1];
  await page.close();
  return docId!;
}

test.describe("Presence & Awareness", () => {
  let ctxA: BrowserContext;
  let ctxB: BrowserContext;

  test.beforeEach(async ({ browser }: { browser: Browser }) => {
    ctxA = await browser.newContext();
    ctxB = await browser.newContext();
    await loginAs(ctxA, USER_A);
    await loginAs(ctxB, USER_B);
  });

  test.afterEach(async () => {
    await ctxA.close();
    await ctxB.close();
  });

  test("two editors see each other's avatars", async () => {
    const docId = await createDoc(ctxA);
    const docUrl = `/documents/${docId}`;

    const pageA = await ctxA.newPage();
    const pageB = await ctxB.newPage();

    await pageA.goto(docUrl);
    await pageB.goto(docUrl);

    // Wait for both to connect (connection status = "Synced")
    await expect(pageA.getByText("Synced")).toBeVisible({ timeout: 15_000 });
    await expect(pageB.getByText("Synced")).toBeVisible({ timeout: 15_000 });

    // Page A should show Bob's avatar in the strip
    const bobInitials = USER_B.email.split("@")[0]!.slice(0, 2).toUpperCase();
    await expect(pageA.locator(".avatar-strip")).toBeVisible();
    await expect(pageA.locator(`.avatar[title^="${USER_B.email.split("@")[0]}"]`)).toBeVisible({
      timeout: 10_000,
    });

    // Page B should show Alice's avatar in the strip
    await expect(pageB.locator(".avatar-strip")).toBeVisible();
    await expect(pageB.locator(`.avatar[title^="${USER_A.email.split("@")[0]}"]`)).toBeVisible({
      timeout: 10_000,
    });

    // Each user should NOT see themselves in the strip
    const aliceInitials = USER_A.email.split("@")[0]!.slice(0, 2).toUpperCase();
    await expect(pageA.locator(`.avatar[title^="${USER_A.email.split("@")[0]}"]`)).toHaveCount(0);
    await expect(pageB.locator(`.avatar[title^="${USER_B.email.split("@")[0]}"]`)).toHaveCount(0);

    // Suppress unused variable warnings
    void bobInitials;
    void aliceInitials;
  });

  test("avatar shows colored caret cursor in editor after typing", async () => {
    const docId = await createDoc(ctxA);
    const docUrl = `/documents/${docId}`;

    const pageA = await ctxA.newPage();
    const pageB = await ctxB.newPage();

    await pageA.goto(docUrl);
    await pageB.goto(docUrl);

    await expect(pageA.getByText("Synced")).toBeVisible({ timeout: 15_000 });
    await expect(pageB.getByText("Synced")).toBeVisible({ timeout: 15_000 });

    // User B types in the editor — their cursor should appear in page A's editor
    const editorB = pageB.locator(".cm-editor-container .cm-editor");
    await editorB.click();
    await pageB.keyboard.type("Hello from Bob");

    // y-codemirror renders remote cursors as .cm-ySelectionCaret elements
    await expect(
      pageA.locator(".cm-ySelectionCaret, .cm-ySelection")
    ).toBeVisible({ timeout: 10_000 });
  });

  test("anonymous public viewer sees live editor updates", async ({ browser }) => {
    const docId = await createDoc(ctxA);
    const docUrl = `/documents/${docId}`;
    const pageA = await ctxA.newPage();
    const publicContext = await browser.newContext();
    const publicPage = await publicContext.newPage();

    await pageA.goto(docUrl);
    await expect(pageA.getByText("Synced")).toBeVisible({ timeout: 15_000 });

    const editorA = pageA.locator(".cm-editor-container .cm-editor");
    await editorA.click();
    await pageA.keyboard.type("Public live draft");

    await pageA.getByRole("button", { name: /share/i }).click();
    await pageA.locator("section", { hasText: "Public access" }).getByRole("switch").click();
    await expect(pageA.getByText("Anyone with link")).toBeVisible();

    await publicPage.goto(docUrl);
    await expect(publicPage.getByText("Sign up to edit")).toBeVisible();
    await expect(publicPage.getByTestId("editor-toolbar")).toHaveCount(0);
    await expect(publicPage.locator(".cm-content")).toContainText("Public live draft", {
      timeout: 15_000,
    });

    await editorA.click();
    await pageA.keyboard.type(" updated");

    await expect(publicPage.locator(".cm-content")).toContainText("Public live draft updated", {
      timeout: 15_000,
    });

    await publicContext.close();
  });
});
