import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false };

/**
 * Deckt Phase 9 Schritt 7 ab (`PLAN.md`, `DECISIONS.md` ADR-0035): das
 * „Vergleichs-Grid" — bis zu 9 Versionen/virtuelle Kopien nebeneinander,
 * mit synchronisiertem Zoom (`CompareGridView.tsx`s neuem
 * `compareViewZoom`). Reused bewusst die bestehende Vergleichsansicht aus
 * Phase 9 Schritt 3 statt eines zweiten Rendering-Pfads — hier bewusst
 * nur ein Frontend-Flow: eine virtuelle Kopie anlegen, „Versionen
 * vergleichen" öffnet das Original plus die Kopie, ein Zoom-Knopf
 * skaliert alle Kacheln gemeinsam.
 */
test.describe("Vergleichs-Grid: Versionen (Phase 9 Schritt 7)", () => {
  test("Versionen vergleichen zeigt Original und virtuelle Kopie, Zoom wirkt auf alle Kacheln", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO] } });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("button", { name: "Raster" }).click();
    await page.getByRole("img", { name: PHOTO.filename }).first().click();

    await page.getByRole("button", { name: "Organisieren…" }).click();
    await page.getByRole("button", { name: "Virtuelle Kopien" }).click();
    await page.getByRole("button", { name: "Virtuelle Kopie vom ausgewählten Foto erstellen" }).click();
    await page.getByRole("button", { name: "Schließen" }).click();

    await page.getByRole("button", { name: "Versionen vergleichen" }).click();

    const compareView = page.getByLabel("Vergleichsansicht");
    await expect(compareView.getByText(/Vergleichsansicht — 2 Fotos/)).toBeVisible();
    await expect(compareView.getByRole("img", { name: PHOTO.filename })).toHaveCount(2);

    const zoomGroup = compareView.getByRole("group", { name: "Zoom (synchronisiert)" });
    await zoomGroup.getByRole("button", { name: "2×" }).click();
    const images = compareView.getByRole("img", { name: PHOTO.filename });
    await expect(images.first()).toHaveCSS("transform", /matrix\(2, 0, 0, 2/);
    await expect(images.last()).toHaveCSS("transform", /matrix\(2, 0, 0, 2/);
  });
});
