import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const base = { width: 6000, height: 4000, missing: false };
const PHOTOS = [
  { ...base, id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3" },
  { ...base, id: "01977f4a-0000-7000-8000-000000000102", filename: "IMG_0002.CR3" },
];

/**
 * Deckt Phase 32 F9 ab: das Sammlungs-Board. Geprüft wird der Weg, den
 * ein Nutzer geht — Spalte anlegen, Foto hineinziehen, wieder
 * heraussortieren — und dass die Zählungen danach stimmen.
 */
test.describe("Sammlungs-Board (Phase 32 F9)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: PHOTOS.length }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();

    await page.keyboard.press("ControlOrMeta+k");
    await page.getByRole("textbox").first().fill("Sammlungs-Board");
    await page.getByText("Sammlungs-Board").first().click();
    await expect(page.getByTestId("board-view")).toBeVisible();
  });

  test("zeigt unsortierte Fotos in der Ordner-Spalte", async ({ page }) => {
    await expect(page.getByTestId("board-summary")).toContainText("2 unsortiert");
    await expect(page.locator('[data-column-id="unassigned"] [data-testid="board-card"]')).toHaveCount(2);
  });

  test("legt eine Sammlung an und nimmt ein hineingezogenes Foto aus der Ordner-Spalte", async ({ page }) => {
    await page.getByLabel("Name der neuen Sammlung").fill("Auswahl");
    await page.getByRole("button", { name: "Anlegen" }).click();

    const target = page.locator('[data-column-id]:not([data-column-id="unassigned"])').first();
    await expect(target).toBeVisible();

    const card = page.locator('[data-column-id="unassigned"] [data-testid="board-card"]').first();
    await card.dragTo(target);

    await expect(target.locator('[data-testid="board-card"]')).toHaveCount(1);
    // Ziehen verschiebt: das Foto ist aus der Ordner-Spalte verschwunden.
    await expect(page.locator('[data-column-id="unassigned"] [data-testid="board-card"]')).toHaveCount(1);
    await expect(page.getByTestId("board-summary")).toContainText("1 unsortiert");
  });

  test("das × an einer Karte nimmt das Foto wieder aus der Sammlung", async ({ page }) => {
    await page.getByLabel("Name der neuen Sammlung").fill("Auswahl");
    await page.getByRole("button", { name: "Anlegen" }).click();
    const target = page.locator('[data-column-id]:not([data-column-id="unassigned"])').first();
    await page.locator('[data-column-id="unassigned"] [data-testid="board-card"]').first().dragTo(target);
    await expect(target.locator('[data-testid="board-card"]')).toHaveCount(1);

    await target.getByRole("button", { name: /aus Auswahl entfernen/ }).click();
    await expect(target.locator('[data-testid="board-card"]')).toHaveCount(0);
    await expect(page.getByTestId("board-summary")).toContainText("2 unsortiert");
  });
});
