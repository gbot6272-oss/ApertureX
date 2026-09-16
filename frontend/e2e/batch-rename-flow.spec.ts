import { expect, test } from "@playwright/test";

import { installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";

const base = { width: 6000, height: 4000, missing: false, camera_model: "Canon EOS R5" };
const PHOTOS = [
  { ...base, id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", captured_at: "2024-05-04T10:00:00+02:00" },
  { ...base, id: "01977f4a-0000-7000-8000-000000000102", filename: "IMG_0002.CR3", captured_at: "2024-05-04T11:00:00+02:00" },
];

/**
 * Deckt Phase 32 F4 ab: die Stapel-Umbenennung. Die Planung (Kollisionen,
 * Ringtausch, virtuelle Kopien) und das zweiphasige Umbenennen auf der
 * Platte sind in `apx-app`s `batch_rename`-Tests abgedeckt — hier geht es
 * um den Weg durch die Oberfläche: Muster bauen, Vorschau lesen,
 * anwenden, und dass ein Muster ohne {seq} vor dem Anwenden geblockt wird.
 */
test.describe("Stapel-Umbenennung (Phase 32 F4)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: PHOTOS.length }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("button", { name: "Raster" }).click();
    // Beide Fotos in die Mehrfachauswahl (Strg-Klick auf das zweite).
    await page.getByRole("img", { name: "IMG_0001.CR3" }).first().click();
    await page.getByRole("img", { name: "IMG_0002.CR3" }).first().click({ modifiers: ["Control"] });
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Stapel-Umbenennung…" }).click();
  });

  test("zeigt die Vorschau aus dem Backend und benennt nach dem Anwenden um", async ({ page }) => {
    await expect(page.getByTestId("batch-rename-scope")).toContainText("2 Fotos ausgewählt");

    const preview = page.getByTestId("batch-rename-preview");
    await expect(preview).toContainText("20240504_0001.CR3");
    await expect(preview).toContainText("20240504_0002.CR3");
    await expect(page.getByTestId("batch-rename-summary")).toContainText("2 werden umbenannt");

    await page.getByRole("button", { name: "Umbenennen" }).click();
    await expect(page.getByTestId("batch-rename-result")).toContainText("2 umbenannt");

    // Das Raster zeigt die neuen Namen — der Katalog wurde wirklich
    // nachgezogen, nicht nur die Vorschau.
    await page.getByRole("button", { name: "Schließen" }).click();
    await expect(page.getByRole("img", { name: "20240504_0001.CR3" }).first()).toBeVisible();
  });

  test("ein Muster ohne {seq} wird als doppelter Zielname geblockt", async ({ page }) => {
    await page.getByLabel("Umbenennungsmuster").fill("{date}");

    await expect(page.getByTestId("batch-rename-preview")).toContainText("doppelter Zielname");
    await expect(page.getByTestId("batch-rename-summary")).toContainText("1 blockiert");
    await expect(page.getByRole("button", { name: "Umbenennen" })).toBeDisabled();
  });

  test("Token-Knöpfe bauen das Muster, gespeicherte Muster kommen zurück", async ({ page }) => {
    const patternInput = page.getByLabel("Umbenennungsmuster");
    await patternInput.fill("");
    await page.getByRole("button", { name: "Token {camera} einfügen" }).click();
    await page.getByRole("button", { name: "Token {seq} einfügen" }).click();
    await expect(patternInput).toHaveValue("{camera}{seq}");

    await page.getByLabel("Name für das neue Muster").fill("Kamera-Nummer");
    await page.getByRole("button", { name: "Muster speichern" }).click();

    // Anderes Muster einstellen, dann das gespeicherte zurückholen.
    await patternInput.fill("{original}");
    await page.getByLabel("Gespeichertes Muster anwenden").selectOption({ label: "Kamera-Nummer" });
    await expect(patternInput).toHaveValue("{camera}{seq}");
  });
});
