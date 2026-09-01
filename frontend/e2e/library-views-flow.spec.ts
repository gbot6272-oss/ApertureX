import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO_A = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false, rating: 4 };
const PHOTO_B = { id: "01977f4a-0000-7000-8000-000000000102", filename: "IMG_0002.CR3", width: 6000, height: 4000, missing: false };

/**
 * Deckt Phase 9 Schritt 3 ab (`PLAN.md`, `DECISIONS.md` ADR-0035):
 * Filter-Presets, Statistik-Dashboard, Vergleichsansicht — je ein
 * stellvertretender Frontend-Flow, die eigentliche Aggregations-/
 * Cache-Logik ist bereits in `apx-catalog`s (`repository::stats`) und
 * `apx-app`s Rust-Tests abgedeckt.
 */
test.describe("Bibliotheks-Ansichten (Phase 9 Schritt 3)", () => {
  test("Filter als Preset speichern und wieder anwenden setzt den Bewertungs-Filter", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2 }], photosByFolder: { [FOLDER_ID]: [PHOTO_A, PHOTO_B] } });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("button", { name: "Raster" }).click();

    await page.getByRole("button", { name: "4★+" }).click();
    await page.getByPlaceholder("Neues Preset…").fill("Top-Bewertungen");
    await page.getByRole("button", { name: "Speichern" }).click();

    // Filter zurücksetzen, dann per Preset wiederherstellen.
    await page.getByRole("button", { name: "Filter zurücksetzen" }).click();
    await expect(page.getByRole("button", { name: "4★+" })).toHaveAttribute("aria-pressed", "false");

    await page.getByLabel("Filter-Preset anwenden").selectOption({ label: "Top-Bewertungen" });
    await expect(page.getByRole("button", { name: "4★+" })).toHaveAttribute("aria-pressed", "true");
  });

  test("Statistik-Dashboard zeigt die Foto-Gesamtzahl", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO_A, PHOTO_B] },
      catalogStatistics: {
        total_photos: 2,
        total_file_size: 2048,
        earliest_captured_at: null,
        latest_captured_at: null,
        rating_distribution: [[0, 1], [4, 1]],
        top_camera_models: [["Canon EOS R5", 2]],
        top_lenses: [],
      },
    });
    await page.goto("/");
    await page.getByRole("button", { name: "Statistik…" }).click();

    await expect(page.getByText("Canon EOS R5: 2")).toBeVisible();
  });

  test("Vergleichsansicht zeigt beide ausgewählten Fotos nebeneinander", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2 }], photosByFolder: { [FOLDER_ID]: [PHOTO_A, PHOTO_B] } });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO_A.filename }).click();
    await page.getByRole("img", { name: PHOTO_B.filename }).click({ modifiers: ["Control"] });

    await page.getByRole("button", { name: "Vergleichen" }).click();

    await expect(page.getByLabel("Vergleichsansicht").getByText(PHOTO_A.filename)).toBeVisible();
    await expect(page.getByLabel("Vergleichsansicht").getByText(PHOTO_B.filename)).toBeVisible();
  });
});
