import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false };

/**
 * Deckt Phase 9 Schritt 4 ab (`PLAN.md`, `DECISIONS.md` ADR-0035):
 * Histogramm/Punktfarbmesser/Navigator im Entwickeln-Modul — die
 * Berechnungslogik selbst (`computeHistogram`/`countClipping`/
 * `buildClippingOverlay`) ist bereits vollständig in `lib/histogram.test.ts`
 * abgedeckt, hier bewusst nur ein Frontend-Flow: Panel erscheint mit
 * Histogramm-Canvas, Punktfarbmesser zeigt beim Überfahren des Bilds den
 * (in der Mock-Entwickeln-Route fest verdrahteten) Farbwert an.
 */
test.describe("Entwickeln-Analysewerkzeuge (Phase 9 Schritt 4)", () => {
  test("Histogramm-Panel erscheint im Entwickeln-Modus und Punktfarbmesser zeigt einen Wert beim Überfahren", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO] } });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln" }).click();

    await expect(page.getByLabel("Histogramm")).toBeVisible();
    await expect(page.getByText("Bild überfahren…")).toBeVisible();

    const viewer = page.locator("main").filter({ has: page.getByLabel("Histogramm") });
    await viewer.hover();

    // Mock-Entwickeln-Route liefert immer denselben warm-orangen Farbwert
    // (180/140/100) — siehe `tauri-mock.ts`s Moduldoku dazu.
    await expect(page.getByText(/R 180 · G 140 · B 100/)).toBeVisible();
  });
});
