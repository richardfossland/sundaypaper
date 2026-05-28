import { test, expect } from "@playwright/test";

// The app shell must render in a plain browser even without the Tauri backend
// (the dashboard will show its "IPC failed" state, but the chrome is intact).
test("app shell renders the brand and primary action", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("SundayPaper").first()).toBeVisible();
  await expect(
    page.getByRole("button", { name: /Nytt dokument/ }),
  ).toBeVisible();
});
