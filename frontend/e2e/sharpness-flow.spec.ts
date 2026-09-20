import { expect, test } from "@playwright/test";

import { installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const SHARP = "01977f4a-0000-7000-8000-000000000101";
const SOFT = "01977f4a-0000-7000-8000-000000000102";

const base = { width: 6000, height: 4000, missing: false, camera_model: "Canon EOS R5" };
const PHOTOS = [
  { ...base, id: SHARP, filename: "IMG_0001.CR3" },
  { ...base, id: SOFT, filename: "IMG_0002.CR3" },
];

/**
 * Deckt Phase 33 F3 ab: die Schärfe-Bewertung. Die Messung selbst
 * (Laplace je Kachel, Kontrastnormierung, Perzentil, Rangfolge) ist in
 * `apx-stacking`s Rust-Tests abgedeckt — hier geht es um den Weg durch
 * die Oberfläche: bewerten, die Rangfolge lesen, und den Rest mit einer
 * Bestätigung in den Papierkorb schieben.
 */
test.describe("Schärfe-Bewertung (Phase 33 F3)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: PHOTOS.length }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
      // Bewusst in umgekehrter Reihenfolge zur Rasterreihenfolge: der
      // Dialog soll nach Schärfe sortieren, nicht nach Dateiname.
      sharpnessScores: { [SHARP]: 4, [SOFT]: 1 },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("button", { name: "Raster" }).click();
    await page.getByRole("img", { name: "IMG_0001.CR3" }).first().click();
    await page.getByRole("img", { name: "IMG_0002.CR3" }).first().click({ modifiers: ["Control"] });
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Schärfe bewerten…" }).click();
  });

  test("sortiert nach Schärfe und nennt die beste Aufnahme", async ({ page }) => {
    await expect(page.getByTestId("sharpness-summary")).toContainText("schärfste: IMG_0001.CR3");
    const rows = page.getByTestId("sharpness-list").getByRole("listitem");
    await expect(rows.first()).toContainText("IMG_0001.CR3");
    await expect(rows.first()).toContainText("100 %");
    await expect(rows.nth(1)).toContainText("IMG_0002.CR3");
    await expect(rows.nth(1)).toContainText("25 %");
  });

  test("behält nach zwei Klicks nur die schärfste und wirft den Rest weg", async ({ page }) => {
    const keep = page.getByTestId("sharpness-keep-best");
    await expect(keep).toContainText("Nur die schärfste behalten (1 weg)");
    await keep.click();
    await expect(keep).toContainText("Wirklich?");
    await keep.click();

    await expect(page.getByTestId("sharpness-summary")).toContainText("1 Aufnahme bewertet");
    await page.getByRole("button", { name: "Schließen" }).click();
    await expect(page.getByRole("img", { name: "IMG_0002.CR3" })).toHaveCount(0);

    // Und landet wirklich im Papierkorb, mit dem Grund „unscharf".
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Papierkorb…" }).click();
    await expect(page.getByTestId("trash-list").getByRole("heading", { name: /Als unscharf aussortiert/ })).toBeVisible();
  });
});
