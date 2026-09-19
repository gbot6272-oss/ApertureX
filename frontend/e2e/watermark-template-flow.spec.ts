import { expect, test } from "@playwright/test";

import { installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";

const PHOTOS = [
  {
    id: "01977f4a-0000-7000-8000-000000000101",
    filename: "IMG_0001.CR3",
    width: 6000,
    height: 4000,
    missing: false,
  },
];

/**
 * Deckt Phase 33 F5 ab: die Wasserzeichen-Vorlagen. Die Geometrie
 * (Prozent der kürzeren Kante, zentriertes Kachelmuster, Drehung) ist in
 * `apx-export`s `watermark_layout`-Tests und in
 * `lib/watermarkTemplate.test.ts` abgedeckt — hier geht es um den Weg
 * durch die Oberfläche: Vorlage bauen, speichern, im Export wählen.
 */
test.describe("Wasserzeichen-Vorlagen (Phase 33 F5)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: PHOTOS.length }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
  });

  test("zeigt eine Vorschau und schaltet beim Kacheln auf andere Regler um", async ({ page }) => {
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Wasserzeichen-Vorlagen…" }).click();

    await expect(page.getByTestId("watermark-preview")).toBeVisible();
    // Ohne Kachelung: Position und Rand.
    await expect(page.getByLabel("Position")).toBeVisible();
    await expect(page.getByLabel("Rand in Prozent")).toBeVisible();

    await page.getByLabel("Über das ganze Bild kacheln").check();
    // Mit Kachelung: Abstand und Drehung statt Position und Rand.
    await expect(page.getByLabel("Position")).toHaveCount(0);
    await expect(page.getByLabel("Kachelabstand in Prozent")).toBeVisible();
    await expect(page.getByLabel("Drehung in Grad")).toBeVisible();
  });

  test("eine gespeicherte Vorlage steht im Export-Dialog zur Wahl", async ({ page }) => {
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Wasserzeichen-Vorlagen…" }).click();

    await page.getByLabel("Wasserzeichen-Text").fill("© Anna Beispiel");
    await page.getByLabel("Name der Wasserzeichen-Vorlage").fill("Andruck");
    await page.getByRole("button", { name: "Wasserzeichen-Vorlage speichern" }).click();
    await expect(page.getByTestId("watermark-template-list")).toContainText("Andruck");

    await page.getByRole("button", { name: "Schließen" }).click();

    await page.getByRole("img", { name: "IMG_0001.CR3" }).first().click();
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: /^Exportieren/ }).click();
    await expect(page.getByTestId("export-watermark-template")).toContainText("Andruck");
  });

  test("eine gewählte Vorlage sagt, dass die Regler darunter ohne Wirkung sind", async ({ page }) => {
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Wasserzeichen-Vorlagen…" }).click();
    await page.getByLabel("Name der Wasserzeichen-Vorlage").fill("Andruck");
    await page.getByRole("button", { name: "Wasserzeichen-Vorlage speichern" }).click();
    await page.getByRole("button", { name: "Schließen" }).click();

    await page.getByRole("img", { name: "IMG_0001.CR3" }).first().click();
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: /^Exportieren/ }).click();
    await page.getByTestId("export-watermark-template").selectOption({ label: "Andruck" });
    await expect(page.getByText("Die Vorlage gilt")).toBeVisible();
  });
});
