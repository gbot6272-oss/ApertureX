import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Reise";
const GEOTAGGED_PHOTO = {
  id: "01977f4a-0000-7000-8000-000000000101",
  filename: "IMG_0001.CR3",
  width: 6000,
  height: 4000,
  missing: false,
  gps_lat: 52.52,
  gps_lon: 13.405,
};
const UNTAGGED_PHOTO = {
  id: "01977f4a-0000-7000-8000-000000000102",
  filename: "IMG_0002.CR3",
  width: 6000,
  height: 4000,
  missing: false,
};

/**
 * Deckt Phase 8 Schritt 7 ab (`PLAN.md`, `apx_export::map`s Moduldoku):
 * die Kartenansicht selbst. Reverse-Geocoding und GPX-Parsing sind
 * bereits vollständig in `apx-export`s Rust-Unit-Tests (`map.rs`)
 * abgedeckt — hier bewusst nur die Frontend-Verdrahtung.
 */
test.describe("Karte (Phase 8 Schritt 7)", () => {
  test("Karte-Knopf schaltet zur Kartenansicht mit der Anzahl geotaggter Fotos", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2 }],
      photosByFolder: { [FOLDER_ID]: [GEOTAGGED_PHOTO, UNTAGGED_PHOTO] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: "Karte" }).click();

    await expect(page.getByText("1 Foto mit GPS")).toBeVisible();
    const log = await getMockInvokeLog(page);
    expect(log.some((e) => e.cmd === "list_geotagged_photos")).toBe(true);
  });

  test("GPX-Import lädt den Track und zeigt die Punktanzahl", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [GEOTAGGED_PHOTO] },
      pickFilePathResult: "/home/user/Reisen/track.gpx",
      gpxTrackPoints: [
        { lat: 52.52, lon: 13.405, elevation: 34.0, time: null },
        { lat: 52.53, lon: 13.41, elevation: 35.0, time: null },
      ],
    });
    await page.goto("/");
    await page.getByRole("button", { name: "Karte" }).click();
    await page.getByRole("button", { name: "GPX-Track importieren…" }).click();

    await expect(page.getByRole("button", { name: /GPX-Track entfernen \(2 Punkte\)/ })).toBeVisible();
  });
});
