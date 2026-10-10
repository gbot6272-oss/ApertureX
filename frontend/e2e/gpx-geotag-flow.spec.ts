import { expect, test } from "@playwright/test";

import { installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000801";
const base = { width: 6000, height: 4000, missing: false, captured_at: "2023-11-14T22:13:25Z" };
const OHNE = { ...base, id: "01977f4a-0000-7000-8000-000000000802", filename: "OHNE.CR3", gps_lat: null, gps_lon: null };
const MIT = { ...base, id: "01977f4a-0000-7000-8000-000000000803", filename: "MIT.CR3", gps_lat: 1.0, gps_lon: 2.0 };

/**
 * Phase 34 F4 (siehe `DECISIONS.md` ADR-0070). Die Zuordnungsmathematik
 * liegt in `gpx_match.rs`s neun Rust-Tests — hier laeuft der Weg durch
 * die Oberflaeche: Track waehlen, Versatz setzen, Zuordnung ansehen,
 * schreiben. Und vor allem: dass eine VORHANDENE Position nicht
 * stillschweigend ueberschrieben wird.
 */
test.describe("Aus GPX-Track verorten (Phase 34 F4)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: "/home/user/Fotos/Tour", photo_count: 2, parent_id: null, missing: false }],
      photosByFolder: { [FOLDER_ID]: [OHNE, MIT] },
      pickFilePathResult: "/home/user/Tour.gpx",
      // Treffer nur beim richtigen Versatz (eine Stunde = 3600 s).
      gpxHits: {
        [`${OHNE.id}@3600`]: { lat: 50.001, lon: 8.002 },
        [`${MIT.id}@3600`]: { lat: 50.002, lon: 8.004 },
      },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Tour/ }).click();
    await page.getByRole("img", { name: OHNE.filename }).click();
    await page.getByRole("img", { name: MIT.filename }).click({ modifiers: ["Control"] });
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Aus GPX-Track verorten…" }).click();
    await page.getByTestId("gpx-geotag-choose").click();
  });

  test("ohne passenden Versatz gibt es keinen Treffer", async ({ page }) => {
    await page.getByTestId("gpx-geotag-preview").click();
    await expect(page.getByTestId("gpx-geotag-list")).toContainText("Keine Trackposition");
    await expect(page.getByTestId("gpx-geotag-apply")).toBeDisabled();
  });

  test("mit Versatz werden Positionen gezeigt und geschrieben", async ({ page }) => {
    await page.getByLabel("Kamerauhr-Versatz in Stunden").fill("1");
    await page.getByTestId("gpx-geotag-preview").click();

    await expect(page.getByTestId("gpx-geotag-list")).toContainText("50.00100, 8.00200");
    // Das Foto mit vorhandener Position bleibt standardmaessig aussen vor.
    await expect(page.getByTestId("gpx-geotag-summary")).toContainText("1 von 2");
    await expect(page.getByTestId("gpx-geotag-list")).toContainText("hat schon eine Position");

    await page.getByTestId("gpx-geotag-apply").click();
    await expect(page.getByRole("status")).toContainText("1 Foto verortet");
  });

  test("ausdruecklich eingeschaltetes Ueberschreiben nimmt beide", async ({ page }) => {
    await page.getByLabel("Kamerauhr-Versatz in Stunden").fill("1");
    await page.getByLabel("Vorhandene Positionen überschreiben").check();
    await page.getByTestId("gpx-geotag-preview").click();

    await expect(page.getByTestId("gpx-geotag-summary")).toContainText("2 von 2");
    await page.getByTestId("gpx-geotag-apply").click();
    await expect(page.getByRole("status")).toContainText("2 Fotos verortet");
  });
});
