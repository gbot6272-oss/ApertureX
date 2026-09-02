import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false };

async function setUpAndOpenOverview(page: import("@playwright/test").Page) {
  await installTauriMock(page, {
    folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
    photosByFolder: { [FOLDER_ID]: [PHOTO] },
  });
  await page.goto("/");
  await page.getByRole("button", { name: /Urlaub/ }).click();
  await page.getByRole("button", { name: "Übersicht" }).click();
}

/**
 * Deckt Phase 11 Schritt 3 ab (`PLAN.md`, `DECISIONS.md` ADR-0038):
 * Übersichtsansicht (dritter `centerView`-Modus, größere Kacheln,
 * reduziertes Metadaten-Overlay statt Bewertungs-/Flaggen-/Farb-Widgets)
 * und Schnellentwicklung im Raster (die sieben Phase-2-Basisregler als
 * Overlay bei Hover/Auswahl, committet über denselben
 * `apply_develop_edit`-Pfad wie das Entwickeln-Panel). Die
 * Regler-Zuordnung selbst ist bereits über `DevelopSlider`/
 * `develop-flow.spec.ts` abgedeckt — hier nur der neue Raster-Modus und
 * seine Verdrahtung.
 */
test.describe("Übersichtsansicht + Schnellentwicklung im Raster (Phase 11 Schritt 3)", () => {
  test("Übersicht zeigt größere Kacheln mit Dateinamen statt Bewertungs-/Flaggen-/Farb-Widgets", async ({ page }) => {
    await setUpAndOpenOverview(page);

    await expect(page.getByRole("button", { name: "Übersicht" })).toHaveAttribute("aria-pressed", "true");
    // Auf den Raster-`<main>` beschränkt — der Filmstreifen unten zeigt
    // dieselbe Fotominiatur unabhängig vom `centerView` und hätte
    // dieselbe zugängliche Rolle/Namen, wäre also sonst mehrdeutig.
    const overviewMain = page.getByRole("main");
    // Reduziertes Overlay: der Dateiname ersetzt die Bewertungs-Widgets
    // aus dem normalen Raster, solange die Kachel nicht gehovert/gewählt
    // ist (siehe `GridView.tsx`s `showQuickDevelop`).
    await expect(overviewMain.getByText(PHOTO.filename)).toBeVisible();
    await expect(overviewMain.getByRole("group", { name: "Bewertung" })).toHaveCount(0);
  });

  test("Schnellentwicklung im Raster committet einen Regler-Wert über apply_develop_edit", async ({ page }) => {
    await setUpAndOpenOverview(page);

    // Klick wählt die Kachel aus *und* hovert sie (Playwright bewegt die
    // Maus real dorthin) — beides zusammen blendet laut `GridView.tsx`s
    // `showQuickDevelop`-Logik das Schnellentwicklung-Overlay ein. Auf
    // den Raster-`<main>` beschränkt, siehe Kommentar im Test oben.
    await page.getByRole("main").getByRole("img", { name: PHOTO.filename }).click();

    const quickDevelopGroup = page.getByRole("group", { name: "Schnellentwicklung" });
    await expect(quickDevelopGroup).toBeVisible();

    const exposureInput = quickDevelopGroup.getByRole("spinbutton", { name: "Belichtung (Zahlenwert)" });
    await exposureInput.fill("1.5");
    await exposureInput.blur();

    await expect
      .poll(async () => {
        const log = await getMockInvokeLog(page);
        return log.some((entry) => entry.cmd === "apply_develop_edit");
      })
      .toBe(true);

    const log = await getMockInvokeLog(page);
    const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
    const args = lastCommit?.args as { photoId: string; edlJson: string };
    expect(args.photoId).toBe(PHOTO.id);
    expect(JSON.parse(args.edlJson).payload.basic.exposure_ev).toBeCloseTo(1.5);
  });
});
