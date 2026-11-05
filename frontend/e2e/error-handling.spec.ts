import { expect, test } from "@playwright/test";

const API_BASE = process.env.VITE_API_URL ?? "http://localhost:8080";

for (const status of [404, 403]) {
  test(`HTTP ${status} document URL renders an error page`, async ({ page }) => {
    const pageErrors: Error[] = [];
    page.on("pageerror", (error) => pageErrors.push(error));
    await page.route(`${API_BASE}/auth/refresh`, async (route) => {
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify({ access_token: "test-token", token_type: "Bearer" }),
      });
    });
    await page.route(`${API_BASE}/documents/missing-document`, async (route) => {
      await route.fulfill({
        status,
        contentType: "application/json",
        body: JSON.stringify({ detail: "Document not found" }),
      });
    });
    await page.route(`${API_BASE}/documents/missing-document/content`, async (route) => {
      await route.fulfill({
        status,
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
    const backLink = page.getByRole("link", { name: "Back to dashboard" });
    await expect(backLink).toHaveAttribute("href", "/");
    await backLink.click();
    await expect(page).toHaveURL("/");
    await expect(page.getByText("No documents yet")).toBeVisible();
    await expect(page.getByRole("button", { name: "Create document", exact: true })).toBeVisible();
    expect(pageErrors).toEqual([]);
  });
}

for (const failure of [500, 429, "network"] as const) {
  test(`${failure} shows an auto-dismissing toast and recovery page`, async ({ page }) => {
    const pageErrors: Error[] = [];
    page.on("pageerror", (error) => pageErrors.push(error));
    await page.route(`${API_BASE}/auth/refresh`, (route) => route.fulfill({ status: 401, body: "{}" }));
    await page.route(`${API_BASE}/documents/failing-document{,/content}`, (route) =>
      failure === "network"
        ? route.abort("failed")
        : route.fulfill({ status: failure, contentType: "application/problem+json", body: JSON.stringify({ detail: "Internal detail" }) }),
    );
    await page.goto("/documents/failing-document");
    await expect(page.getByRole("heading", { name: "Something went wrong" })).toBeVisible();
    await expect(page.getByRole("link", { name: "Back to dashboard" })).toHaveAttribute("href", "/");
    const toast = page.locator("[data-sonner-toast]");
    await expect(toast).toHaveText(failure === 500
      ? "Something went wrong on our side. Please try again in a moment."
      : failure === 429
        ? "You're doing that too quickly. Please wait a moment and try again."
        : "Network error. Check your connection and try again.");
    await expect(toast).toHaveCount(0, { timeout: 6000 });
    expect(pageErrors).toEqual([]);
  });
}
