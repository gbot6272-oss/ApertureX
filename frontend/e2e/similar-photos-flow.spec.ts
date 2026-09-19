import { expect, test } from "@playwright/test";

import { installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const REFERENZ = "01977f4a-0000-7000-8000-000000000101";
/** Gleiches Motiv, andere Farbe — die Schwarzweiß-Fassung. */
const GLEICHES_MOTIV = "01977f4a-0000-7000-8000-000000000102";
/** Anderes Motiv, gleiche Farbstimmung. */
const GLEICHE_FARBE = "01977f4a-0000-7000-8000-000000000103";

const base = { rating: 0, flag: 0, color_label: null, missing: false, width: 6000, height: 4000 };
const PHOTOS = [
  { ...base, id: REFERENZ, filename: "REFERENZ.CR3" },
  { ...base, id: GLEICHES_MOTIV, filename: "SW.CR3" },
  { ...base, id: GLEICHE_FARBE, filename: "STIMMUNG.CR3" },
];

/**
 * Deckt Phase 33 F9 ab: ähnliche Fotos zu einem Referenzfoto. Die Maße
 * (Hamming auf dem Perceptual Hash, L1 auf dem Farbhistogramm, die
 * Mischung) sind in `apx-app`s `similarity`-Tests abgedeckt — hier geht
 * es darum, dass der Regler in der Oberfläche wirklich die Rangfolge
 * umdreht. Genau dafür gibt es ihn.
 */
test.describe("Ähnliche Fotos (Phase 33 F9)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: PHOTOS.length }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
      similarityScores: {
        [GLEICHES_MOTIV]: { structure: 1, color: 0 },
        [GLEICHE_FARBE]: { structure: 0, color: 1 },
      },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("button", { name: "Raster" }).click();
    await page.getByRole("img", { name: "REFERENZ.CR3" }).first().click();
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Ähnliche Fotos finden…" }).click();
  });

  test("bei voller Motiv-Gewichtung gewinnt die Schwarzweiß-Fassung", async ({ page }) => {
    await page.getByLabel("Gewichtung zwischen Motiv und Farbe").fill("0");
    await page.getByRole("button", { name: "Neu suchen" }).click();

    const items = page.getByTestId("similar-list").getByRole("listitem");
    await expect(items).toHaveCount(1);
    await expect(items.first()).toContainText("SW.CR3");
    await expect(items.first()).toContainText("100 % ähnlich");
  });

  test("bei voller Farb-Gewichtung dreht sich die Rangfolge um", async ({ page }) => {
    await page.getByLabel("Gewichtung zwischen Motiv und Farbe").fill("1");
    await page.getByRole("button", { name: "Neu suchen" }).click();

    const items = page.getByTestId("similar-list").getByRole("listitem");
    await expect(items).toHaveCount(1);
    await expect(items.first()).toContainText("STIMMUNG.CR3");
  });

  test("eine niedrigere Schwelle lässt beide durch", async ({ page }) => {
    await page.getByLabel("Gewichtung zwischen Motiv und Farbe").fill("0.5");
    await page.getByLabel("Mindestähnlichkeit").fill("0.4");
    await page.getByRole("button", { name: "Neu suchen" }).click();

    await expect(page.getByTestId("similar-summary")).toContainText("2 Treffer");
  });

  test("das Referenzfoto taucht nie in den Treffern auf", async ({ page }) => {
    await page.getByLabel("Mindestähnlichkeit").fill("0");
    await page.getByRole("button", { name: "Neu suchen" }).click();

    await expect(page.getByTestId("similar-list")).not.toContainText("REFERENZ.CR3");
  });
});
