import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false };

/**
 * Deckt Phase 32 F6 ab: Notizen mit Ort im Bild. Die Koordinaten-
 * Umrechnung ist in `src/lib/notePins.test.ts` abgedeckt, die Datenbank-
 * Seite in `apx-catalog`s `notes`-Tests — hier geht es um den Weg durch
 * die Oberfläche.
 */
test.describe("Notizen am Foto (Phase 32 F6)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).first().click();
  });

  async function placeNote(page: import("@playwright/test").Page, text: string) {
    await page.getByRole("button", { name: "Notiz ins Bild setzen" }).click();
    // In die Mitte klicken: der obere Rand des Bildes liegt unter der
    // schwebenden Kopfzeile, der untere unter den Werkzeugleisten.
    await page.getByTestId("notes-overlay").click();
    await page.getByLabel("Text der neuen Notiz").fill(text);
    await page.getByRole("button", { name: "Notiz anlegen" }).click();
  }

  test("setzt eine Notiz ins Bild und zeigt sie als Pin", async ({ page }) => {
    // Ohne aktivierten Modus ist ein Klick ins Bild kein Notiz-Setzen —
    // die Notiz-Ebene nimmt dann gar keine Klicks an, der Klick geht ans
    // Bild darunter.
    await page.locator("main").click({ position: { x: 200, y: 200 } });
    await expect(page.getByTestId("note-draft")).toHaveCount(0);

    await placeNote(page, "Staubfleck hier weg");

    await expect(page.getByTestId("note-pin")).toHaveCount(1);
    await expect(page.getByTestId("notes-count")).toContainText("1 offen");
    // Der Setzen-Modus schaltet sich nach dem Anlegen wieder ab.
    await expect(page.getByRole("button", { name: "Notiz ins Bild setzen" })).toHaveAttribute("aria-pressed", "false");
  });

  test("abhaken zählt die offenen Notizen herunter, löschen entfernt den Pin", async ({ page }) => {
    await placeNote(page, "Horizont schief");

    await page.getByTestId("note-pin").click();
    await page.getByRole("checkbox", { name: "Erledigt" }).check();
    await expect(page.getByTestId("notes-count")).toContainText("0 offen · 1 gesamt");
    await expect(page.getByTestId("note-pin")).toHaveAttribute("data-done", "true");

    await page.getByRole("button", { name: "Notiz löschen" }).click();
    await expect(page.getByTestId("note-pin")).toHaveCount(0);
  });

  test("das Raster markiert Fotos mit offenen Notizen", async ({ page }) => {
    await placeNote(page, "Noch zu bearbeiten");

    await page.getByRole("button", { name: "Raster" }).click();
    await expect(page.getByTestId("grid-note-badge").first()).toContainText("1");
  });
});
