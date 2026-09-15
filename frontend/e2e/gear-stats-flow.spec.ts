import { expect, test } from "@playwright/test";

import { installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false, camera_model: "Canon EOS R5" };

const GEAR = {
  cameras: [
    ["Canon EOS R5", 60],
    ["Nikon Z9", 40],
  ],
  lenses: [["RF 24-70mm F2.8", 100]],
  focal_lengths: [
    { label: "25–35 mm", count: 70, missing: false },
    { label: "51–85 mm", count: 20, missing: false },
    { label: "keine Angabe", count: 10, missing: true },
  ],
  apertures: [{ label: "f/2,8–4,0", count: 100, missing: false }],
  isos: [{ label: "≤ 100", count: 100, missing: false }],
  shutters: [{ label: "1/250–1/60 s", count: 100, missing: false }],
  total: 100,
};

/**
 * Deckt Phase 32 F5 ab: die Aufnahme-Statistik. Die Klassengrenzen und
 * die Aggregation sind in `apx-catalog`s `stats`-Tests abgedeckt — hier
 * geht es um die Darstellung und darum, dass der Klick auf eine Kamera
 * wirklich im Katalogfilter landet statt nur so auszusehen.
 */
test.describe("Aufnahme-Statistik (Phase 32 F5)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
      gearStatistics: GEAR,
    });
    await page.goto("/");
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Aufnahme-Statistik…" }).click();
  });

  test("zeigt Kameras, Objektive und die vier Verteilungen", async ({ page }) => {
    await expect(page.getByTestId("gear-total")).toContainText("100 Aufnahmen ausgewertet");
    await expect(page.getByTestId("gear-cameras")).toContainText("Canon EOS R5");
    await expect(page.getByTestId("gear-lenses")).toContainText("RF 24-70mm F2.8");

    await expect(page.getByTestId("gear-focal")).toContainText("25–35 mm");
    // Der Sammelbalken für fehlende Werte wird benannt, nicht verschwiegen.
    await expect(page.getByTestId("gear-focal")).toContainText("keine Angabe");
    await expect(page.getByTestId("gear-aperture")).toContainText("f/2,8–4,0");
    await expect(page.getByTestId("gear-iso")).toContainText("≤ 100");
    await expect(page.getByTestId("gear-shutter")).toContainText("1/250–1/60 s");
  });

  test("Umschalten auf Anteil rechnet in Prozent um", async ({ page }) => {
    await expect(page.getByTestId("gear-cameras")).toContainText("60");
    await page.getByRole("button", { name: "Anteil" }).click();
    await expect(page.getByTestId("gear-cameras")).toContainText("60.0 %");
  });

  test("Klick auf eine Kamera filtert den Katalog danach", async ({ page }) => {
    await page.getByRole("button", { name: "Nur Fotos von Canon EOS R5 zeigen" }).click();

    // Der Dialog schließt, das Raster erscheint, und der Kamerafilter der
    // Filterleiste trägt den Wert — derselbe Filter, den man dort von Hand
    // eintippen könnte.
    await expect(page.getByTestId("gear-total")).toHaveCount(0);
    await expect(page.getByLabel("Nach Kameramodell filtern")).toHaveValue("Canon EOS R5");
  });
});
