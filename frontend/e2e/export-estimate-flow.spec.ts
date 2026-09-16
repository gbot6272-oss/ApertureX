import { expect, test } from "@playwright/test";

import { installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const base = { width: 6000, height: 4000, missing: false };
const PHOTOS = [
  { ...base, id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3" },
  { ...base, id: "01977f4a-0000-7000-8000-000000000102", filename: "IMG_0002.CR3" },
];

/**
 * Deckt Phase 32 F10 ab: die Export-Vorschau. Die Rechnung selbst
 * (Begrenzungen, Qualitätskurve, Formatfaktoren) ist in
 * `src/lib/exportEstimate.test.ts` abgedeckt — hier geht es darum, dass
 * die Anzeige auf jede Einstellung im Dialog reagiert.
 */
test.describe("Export-Vorschau (Phase 32 F10)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: PHOTOS.length }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("button", { name: "Raster" }).click();
    await page.getByRole("img", { name: "IMG_0001.CR3" }).first().click();
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Exportieren…" }).click();
  });

  test("zeigt Ausgabemaße und geschätzte Dateigröße", async ({ page }) => {
    const estimate = page.getByTestId("export-estimate");
    await expect(estimate).toContainText("6000 × 4000");
    await expect(estimate).toContainText("24.0 MP");
    // Für JPEG eine Schätzung mit Spanne, keine Scheingenauigkeit.
    await expect(estimate).toContainText("Schätzung");
  });

  test("eine Kantenbegrenzung verkleinert Maße und Schätzung", async ({ page }) => {
    const before = await page.getByTestId("export-estimate-per-photo").textContent();

    // Das Radio trägt kein eigenes Label — die Beschriftung umschließt
    // zusätzlich das Zahlenfeld. Über die Zeile gehen statt über einen
    // Namen, den es nicht gibt.
    const edgeRow = page.getByText("Längere Kante höchstens");
    await edgeRow.getByRole("radio").check();
    await edgeRow.getByRole("spinbutton").fill("1500");

    await expect(page.getByTestId("export-estimate")).toContainText("1500 × 1000");
    await expect.poll(async () => page.getByTestId("export-estimate-per-photo").textContent()).not.toBe(before);
  });

  test("unkomprimiertes TIFF wird als exakt ausgewiesen", async ({ page }) => {
    await page.getByLabel("Ausgabeformat").selectOption("tiff");
    await expect(page.getByTestId("export-estimate")).toContainText("exakt");
  });
});
