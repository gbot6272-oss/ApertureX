import { expect, test } from "@playwright/test";

import { installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";

const base = { width: 6000, height: 4000, missing: false };
// Drei Aufnahmen mit gestufter Zeit (Belichtungsreihe), dann nach einer
// Minute drei mit identischer Belichtung (Reihenaufnahme).
const PHOTOS = [
  { ...base, id: "01977f4a-0000-7000-8000-000000000101", filename: "BRK_1.CR3", captured_at: "2024-05-04T10:00:00Z", aperture: 8, shutter: 1 / 250, iso: 100 },
  { ...base, id: "01977f4a-0000-7000-8000-000000000102", filename: "BRK_2.CR3", captured_at: "2024-05-04T10:00:01Z", aperture: 8, shutter: 1 / 1000, iso: 100 },
  { ...base, id: "01977f4a-0000-7000-8000-000000000103", filename: "BRK_3.CR3", captured_at: "2024-05-04T10:00:02Z", aperture: 8, shutter: 1 / 60, iso: 100 },
  { ...base, id: "01977f4a-0000-7000-8000-000000000104", filename: "SER_1.CR3", captured_at: "2024-05-04T10:01:00Z", aperture: 4, shutter: 1 / 500, iso: 400 },
  { ...base, id: "01977f4a-0000-7000-8000-000000000105", filename: "SER_2.CR3", captured_at: "2024-05-04T10:01:01Z", aperture: 4, shutter: 1 / 500, iso: 400 },
  { ...base, id: "01977f4a-0000-7000-8000-000000000106", filename: "SER_3.CR3", captured_at: "2024-05-04T10:01:02Z", aperture: 4, shutter: 1 / 500, iso: 400 },
];

/**
 * Deckt Phase 32 F7 ab. Die Klassifikation selbst (EV-Rechnung,
 * Grenzfälle, ISO-Bracketing) ist in `apx-catalog`s `series`-Tests
 * abgedeckt — hier geht es darum, dass beide Arten in der Oberfläche
 * getrennt ankommen und die Knöpfe daran hängen.
 */
test.describe("Serien-Erkennung (Phase 32 F7)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: PHOTOS.length }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Serien erkennen…" }).click();
  });

  test("unterscheidet Belichtungsreihe und Reihenaufnahme", async ({ page }) => {
    await expect(page.getByTestId("series-summary")).toContainText("2 Serien gefunden");
    await expect(page.locator('[data-testid="series-row"][data-kind="exposure_bracket"]')).toHaveCount(1);
    await expect(page.locator('[data-testid="series-row"][data-kind="burst"]')).toHaveCount(1);

    // Die Belichtungsreihe weist ihre EV-Spanne aus: 1/60 gegen 1/1000
    // sind log2(1000/60) = 4,06 Blendenstufen.
    await expect(page.locator('[data-testid="series-row"][data-kind="exposure_bracket"]')).toContainText("4.1 EV Spanne");
  });

  test("ein größerer Höchstabstand fasst beide Gruppen zusammen", async ({ page }) => {
    await page.getByLabel("Höchstabstand in Sekunden").fill("90");
    await expect(page.getByTestId("series-summary")).toContainText("1 Serie gefunden");
    await expect(page.getByTestId("series-row")).toHaveCount(1);
  });

  test("„Auswählen“ übernimmt die Serie in die Mehrfachauswahl", async ({ page }) => {
    await page.locator('[data-testid="series-row"][data-kind="burst"]').getByRole("button", { name: "Auswählen" }).click();

    await page.getByRole("button", { name: "Entwickeln" }).click();
    await page.getByRole("tab", { name: "Verlauf & Werkzeuge" }).click();
    await expect(page.getByRole("button", { name: "Auf 2 weitere ausgewählte Fotos synchronisieren" })).toBeEnabled();
  });
});
