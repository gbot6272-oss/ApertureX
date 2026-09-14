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

    // Phase 31 Schritt 1: Die Analyse hängt jetzt als eigene Palette
    // NEBEN dem Foto, nicht mehr als Overlay DARIN — vorher stand hier
    // `main`, gefiltert auf "enthält das Histogramm", was genau die alte
    // Verschachtelung festschrieb. Der Viewer ist das `<main>`; dass das
    // Histogramm sichtbar ist, prüfen die beiden Zusicherungen darüber
    // bereits eigenständig.
    await page.locator("main").hover();

    // Mock-Entwickeln-Route liefert immer denselben warm-orangen Farbwert
    // (180/140/100) — siehe `tauri-mock.ts`s Moduldoku dazu.
    await expect(page.getByText(/R 180 · G 140 · B 100/)).toBeVisible();
  });

  /**
   * Deckt Phase 14 Schritt 6 ab (`DECISIONS.md` ADR-0041): Vektorskop +
   * Wellenform-Monitor als neue Reiter neben dem Histogramm — die
   * Berechnungslogik selbst (`computeVectorscope`/`computeWaveform`) ist
   * bereits vollständig in `lib/vectorscope.test.ts`/`lib/waveform.test.ts`
   * abgedeckt, hier bewusst nur der Reiter-Wechsel: die jeweils aktive
   * Analyse-Canvas erscheint, die anderen beiden verschwinden.
   */
  test("Vektorskop- und Wellenform-Reiter zeigen ihre jeweils eigene Canvas, das Histogramm verschwindet dabei", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO] } });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln" }).click();

    await expect(page.getByLabel("Histogramm")).toBeVisible();

    await page.getByRole("button", { name: "Vektorskop" }).click();
    await expect(page.getByLabel("Vektorskop")).toBeVisible();
    await expect(page.getByLabel("Histogramm")).not.toBeVisible();
    await expect(page.getByLabel("Wellenform")).not.toBeVisible();

    await page.getByRole("button", { name: "Wellenform" }).click();
    await expect(page.getByLabel("Wellenform")).toBeVisible();
    await expect(page.getByLabel("Vektorskop")).not.toBeVisible();

    await page.getByRole("button", { name: "Histogramm" }).click();
    await expect(page.getByLabel("Histogramm")).toBeVisible();
    await expect(page.getByLabel("Wellenform")).not.toBeVisible();
  });
});
