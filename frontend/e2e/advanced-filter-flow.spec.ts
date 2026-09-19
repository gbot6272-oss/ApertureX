import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";

const base = { rating: 0, flag: 0, color_label: null, missing: false, media_kind: "photo" };
const PHOTOS = [
  {
    ...base,
    id: "01977f4a-0000-7000-8000-000000000101",
    filename: "WEIT.CR3",
    lens: "RF 16-35mm",
    iso: 100,
    captured_at: "2024-05-04T10:00:00+02:00",
    width: 6000,
    height: 4000,
    orientation: 1,
  },
  {
    ...base,
    id: "01977f4a-0000-7000-8000-000000000102",
    filename: "PORTRAIT.CR3",
    lens: "RF 85mm",
    iso: 6400,
    captured_at: "2023-01-02T10:00:00+01:00",
    // Liegt quer in der Datei, steht per EXIF-Orientierung 6 hochkant.
    width: 6000,
    height: 4000,
    orientation: 6,
  },
];

/**
 * Deckt Phase 33 F7 ab: die erweiterten Katalogfilter. Die SQL-Regeln
 * (beide Bereichsgrenzen einschließlich, fehlende Werte fallen heraus,
 * Seitenverhältnis nach EXIF-Drehung) sind in `apx-catalog`s
 * `repository::search`-Tests abgedeckt — hier geht es um den Weg durch
 * die Oberfläche.
 */
test.describe("Erweiterte Katalogfilter (Phase 33 F7)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: PHOTOS.length }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("button", { name: "Raster" }).click();
  });

  test("die erweiterten Filter sind eingeklappt, bis man sie aufklappt", async ({ page }) => {
    await expect(page.getByTestId("advanced-filters")).toHaveCount(0);
    await page.getByRole("button", { name: "Erweiterte Filter" }).click();
    await expect(page.getByTestId("advanced-filters")).toBeVisible();
  });

  test("filtert nach Objektiv", async ({ page }) => {
    await page.getByRole("button", { name: "Erweiterte Filter" }).click();
    await page.getByLabel("Nach Objektiv filtern").fill("RF 85mm");
    await page.getByLabel("Nach Objektiv filtern").press("Enter");

    await expect(page.getByRole("img", { name: "PORTRAIT.CR3" }).first()).toBeVisible();
    await expect(page.getByRole("img", { name: "WEIT.CR3" })).toHaveCount(0);
  });

  test("filtert nach ISO-Bereich", async ({ page }) => {
    await page.getByRole("button", { name: "Erweiterte Filter" }).click();
    await page.getByLabel("ISO höchstens").fill("400");

    await expect(page.getByRole("img", { name: "WEIT.CR3" }).first()).toBeVisible();
    await expect(page.getByRole("img", { name: "PORTRAIT.CR3" })).toHaveCount(0);
  });

  test("das Seitenverhältnis folgt der EXIF-Drehung, nicht den Dateikanten", async ({ page }) => {
    // Beide Fotos liegen 6000x4000 in der Datei; nur die EXIF-Drehung
    // unterscheidet sie.
    await page.getByRole("button", { name: "Erweiterte Filter" }).click();
    await page.getByLabel("Seitenverhältnis").selectOption("portrait");

    await expect(page.getByRole("img", { name: "PORTRAIT.CR3" }).first()).toBeVisible();
    await expect(page.getByRole("img", { name: "WEIT.CR3" })).toHaveCount(0);
  });

  test("ein gesetzter erweiterter Filter hält den Bereich offen", async ({ page }) => {
    await page.getByRole("button", { name: "Erweiterte Filter" }).click();
    await page.getByLabel("Medienart").selectOption("photo");
    // Zuklappen versucht — der Bereich bleibt sichtbar, weil ein Filter
    // gesetzt ist. Ein unsichtbarer aktiver Filter wäre die schlimmste
    // Sorte Filter.
    await page.getByRole("button", { name: "Erweiterte Filter" }).click();
    await expect(page.getByTestId("advanced-filters")).toBeVisible();
  });
});
