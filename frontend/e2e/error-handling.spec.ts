import { expect, test } from "@playwright/test";

const API_BASE = process.env.VITE_API_URL ?? "http://localhost:8080";

test("deleted or inaccessible document URL renders an error page", async ({ page }) => {
  await page.route(`${API_BASE}/auth/refresh`, async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ access_token: "test-token", token_type: "Bearer" }),
    });
  });
  await page.route(`${API_BASE}/documents/missing-document`, async (route) => {
    await route.fulfill({
      status: 404,
      contentType: "application/json",
      body: JSON.stringify({ detail: "Document not found" }),
    });
  });
  await page.route(`${API_BASE}/documents/missing-document/content`, async (route) => {
    await route.fulfill({
      status: 404,
      contentType: "application/json",
      body: JSON.stringify({ detail: "Document not found" }),
    });
  });
  await page.route(`${API_BASE}/documents`, async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ data: [], next_cursor: null, has_more: false }),
    });
  });

  await page.goto("/documents/missing-document");

  await expect(page.getByRole("heading", { name: "Document unavailable" })).toBeVisible();
  await expect(page.getByText("This document was deleted, or you do not have access to it.")).toBeVisible();
  await expect(page.getByRole("link", { name: "Back to dashboard" })).toHaveAttribute("href", "/");
});
