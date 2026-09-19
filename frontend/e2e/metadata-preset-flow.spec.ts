import { expect, test } from "@playwright/test";

import { installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";

const base = { width: 6000, height: 4000, missing: false, camera_model: "Canon EOS R5" };
const PHOTOS = [
  { ...base, id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", caption: "Sonnenuntergang" },
  { ...base, id: "01977f4a-0000-7000-8000-000000000102", filename: "IMG_0002.CR3" },
];

/**
 * Deckt Phase 33 F4 ab: die Metadaten-Vorgaben. Die Auflösungsregeln
 * (Platzhalter, „nicht Teil der Vorgabe" gegen „leeren",
 * Stichwort-Modus) sind in `apx-app`s `metadata_preset`-Tests
 * abgedeckt — hier geht es um den Weg durch die Oberfläche: Felder
 * gezielt einschalten, anwenden, speichern und wieder laden.
 */
test.describe("Metadaten-Vorgaben (Phase 33 F4)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: PHOTOS.length }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("button", { name: "Raster" }).click();
    await page.getByRole("img", { name: "IMG_0001.CR3" }).first().click();
    await page.getByRole("img", { name: "IMG_0002.CR3" }).first().click({ modifiers: ["Control"] });
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Metadaten-Vorgaben…" }).click();
  });

  test("ein Feld ohne Haken lässt sich nicht bearbeiten und wird nicht angefasst", async ({ page }) => {
    await expect(page.getByTestId("metadata-preset-scope")).toContainText("2 Fotos ausgewählt");

    // Ohne Haken: gesperrt, mit dem Hinweis, dass nichts passiert.
    const caption = page.getByLabel("Bildunterschrift", { exact: true });
    await expect(caption).toBeDisabled();
    await expect(caption).toHaveAttribute("placeholder", "wird nicht angefasst");

    await page.getByLabel("Bildunterschrift übernehmen").check();
    await expect(caption).toBeEnabled();
    await expect(caption).toHaveAttribute("placeholder", "leer lassen = Feld leeren");
  });

  test("wendet Copyright mit Platzhalter auf die Auswahl an", async ({ page }) => {
    await page.getByLabel("Copyright übernehmen").check();
    await page.getByLabel("Copyright", { exact: true }).fill("© {year} Anna Beispiel");
    await page.getByTestId("metadata-preset-apply").click();

    await expect(page.getByTestId("metadata-preset-result")).toContainText("2 Fotos beschriftet");
  });

  test("speichert eine Vorgabe und lädt sie wieder", async ({ page }) => {
    await page.getByLabel("Urheber übernehmen").check();
    await page.getByLabel("Urheber", { exact: true }).fill("Anna Beispiel");
    await page.getByLabel("Stichwort hinzufügen").fill("Reise");
    await page.getByRole("button", { name: "Stichwort übernehmen" }).click();
    await expect(page.getByTestId("metadata-preset-keywords")).toContainText("Reise");

    await page.getByLabel("Name der Vorgabe").fill("Standard-Urheber");
    await page.getByRole("button", { name: "Vorgabe speichern" }).click();
    await expect(page.getByTestId("metadata-preset-list")).toContainText("Standard-Urheber");

    // Entwurf verwerfen, dann die Vorgabe zurückladen.
    await page.getByLabel("Urheber", { exact: true }).fill("etwas anderes");
    await page.getByRole("button", { name: "Standard-Urheber", exact: true }).click();
    await expect(page.getByLabel("Urheber", { exact: true })).toHaveValue("Anna Beispiel");
    await expect(page.getByTestId("metadata-preset-keywords")).toContainText("Reise");
  });
});
