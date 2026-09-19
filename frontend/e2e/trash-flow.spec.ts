import { expect, test } from "@playwright/test";

import { installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";

const base = { width: 6000, height: 4000, missing: false, camera_model: "Canon EOS R5" };
const PHOTOS = [
  { ...base, id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3" },
  { ...base, id: "01977f4a-0000-7000-8000-000000000102", filename: "IMG_0002.CR3" },
];

/**
 * Deckt Phase 33 F1 ab: den Papierkorb. Dass ein weggeworfenes Foto aus
 * Ordnerliste, Suche und Statistik verschwindet und alles daran hängende
 * stehen bleibt, ist in `apx-catalog`s `repository::trash`-Tests
 * abgedeckt — hier geht es um den Weg durch die Oberfläche: wegwerfen,
 * im Papierkorb wiederfinden, zurückholen, und dass das endgültige
 * Löschen eine zweite Bestätigung verlangt.
 */
test.describe("Papierkorb (Phase 33 F1)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: PHOTOS.length }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("button", { name: "Raster" }).click();
  });

  test("ein weggeworfenes Foto verlässt das Raster und lässt sich zurückholen", async ({ page }) => {
    await page.getByRole("img", { name: "IMG_0001.CR3" }).first().click();
    await page.keyboard.press("Delete");

    await expect(page.getByRole("img", { name: "IMG_0001.CR3" })).toHaveCount(0);
    await expect(page.getByRole("img", { name: "IMG_0002.CR3" }).first()).toBeVisible();

    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Papierkorb…" }).click();
    await expect(page.getByTestId("trash-summary")).toContainText("1 Foto weggeworfen");

    await page.getByTestId("trash-list").getByRole("button", { name: "IMG_0001.CR3" }).click();
    await page.getByRole("button", { name: "Wiederherstellen" }).click();
    await expect(page.getByTestId("trash-summary")).toContainText("Der Papierkorb ist leer");

    await page.getByRole("button", { name: "Schließen" }).click();
    await expect(page.getByRole("img", { name: "IMG_0001.CR3" }).first()).toBeVisible();
  });

  test("endgültiges Löschen verlangt eine zweite Bestätigung", async ({ page }) => {
    await page.getByRole("img", { name: "IMG_0001.CR3" }).first().click();
    await page.keyboard.press("Delete");

    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Papierkorb…" }).click();

    const purge = page.getByTestId("trash-purge");
    await expect(purge).toContainText("Papierkorb leeren");
    await purge.click();

    // Erster Klick bestätigt nur — das Foto liegt noch im Papierkorb.
    await expect(purge).toContainText("Wirklich?");
    await expect(page.getByTestId("trash-summary")).toContainText("1 Foto weggeworfen");

    await purge.click();
    await expect(page.getByTestId("trash-summary")).toContainText("Der Papierkorb ist leer");
  });

  test("der Papierkorb gruppiert nach Grund", async ({ page }) => {
    await page.getByRole("img", { name: "IMG_0001.CR3" }).first().click();
    await page.keyboard.press("Delete");

    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Papierkorb…" }).click();
    await expect(page.getByTestId("trash-list").getByRole("heading", { name: /Von Hand weggeworfen/ })).toBeVisible();
  });
});
