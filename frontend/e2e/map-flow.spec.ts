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
 * Deckt Phase 8 Schritt 7 ab (`PLAN.md`, `apx_export::map`s Moduldoku)
 * sowie dessen Erweiterung um Globus/Heatmap (siehe `DECISIONS.md`
 * ADR-0044). Reverse-Geocoding und GPX-Parsing sind bereits
 * vollständig in `apx-export`s Rust-Unit-Tests (`map.rs`) abgedeckt —
 * hier bewusst nur die Frontend-Verdrahtung.
 */
test.describe("Karte (Phase 8 Schritt 7 + ADR-0044)", () => {
  test("Karte-Knopf schaltet zum Globus mit der Anzahl geotaggter Fotos", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2 }],
      photosByFolder: { [FOLDER_ID]: [GEOTAGGED_PHOTO, UNTAGGED_PHOTO] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: "Karte" }).click();

    // Standard-Einstieg ist jetzt der Globus (ADR-0044) — "kleinste
    // Ebene" der Kartenansicht, nicht mehr direkt die flache Karte.
    await expect(page.getByText("1 Foto mit GPS")).toBeVisible();
    const log = await getMockInvokeLog(page);
    expect(log.some((e) => e.cmd === "list_geotagged_photos")).toBe(true);
  });

  /**
   * Leerzustand (ADR-0073). Nutzer-Rueckmeldung war „die Karte … und
   * keine gefundenen gps daten": ein Globus ohne einen einzigen Punkt
   * sieht aus wie ein Fehler, und die Anzeige „0 Fotos mit GPS"
   * bestaetigte das nur, ohne den Grund zu nennen. Der Grund ist meist
   * keiner: die wenigsten Kameras haben einen GPS-Empfaenger. Dass das
   * Auslesen selbst funktioniert, ist in `apx-raw`s
   * `exif_gps_tests` belegt — hier geht es darum, dass die App es auch
   * sagt und den Weg zur Abhilfe zeigt.
   */
  test("Ohne verortete Fotos erklaert der Globus den Leerzustand und zeigt den Weg", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [UNTAGGED_PHOTO] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: "Karte" }).click();

    const empty = page.getByTestId("globe-empty");
    await expect(empty).toBeVisible();
    await expect(empty).toContainText("Keine Fotos mit Koordinaten");
    // Ohne diesen Satz bliebe der Nutzer bei "irgendwas ist kaputt".
    await expect(empty).toContainText("GPS-Empfänger");
    // Und ohne den Weg zur Abhilfe waere es nur eine hoeflichere Form
    // von "geht nicht".
    await expect(empty).toContainText("GPX-Track");

    await empty.getByRole("button", { name: /Zur Karte/ }).click();
    const mapEmpty = page.getByTestId("map-empty");
    await expect(mapEmpty).toBeVisible();
    await expect(page.getByText("GPX-Track importieren…")).toBeVisible();
  });

  test("Mit verorteten Fotos steht der Leerzustand NICHT da", async ({ page }) => {
    // Gegenkontrolle: ohne sie koennte der Hinweis immer sichtbar sein
    // und der Test oben trotzdem gruen.
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2 }],
      photosByFolder: { [FOLDER_ID]: [GEOTAGGED_PHOTO, UNTAGGED_PHOTO] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: "Karte" }).click();
    await expect(page.getByTestId("globe-empty")).toHaveCount(0);
  });

  test("Vom Globus zur flachen Karte wechseln zeigt die Heatmap/Pin-Steuerung", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2 }],
      photosByFolder: { [FOLDER_ID]: [GEOTAGGED_PHOTO, UNTAGGED_PHOTO] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: "Karte" }).click();
    await page.getByRole("button", { name: "Zur Karte →" }).click();

    await expect(page.getByRole("button", { name: "🌐 Globus" })).toBeVisible();
    await expect(page.getByText("GPX-Track importieren…")).toBeVisible();
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
    await page.getByRole("button", { name: "Zur Karte →" }).click();
    await page.getByRole("button", { name: "GPX-Track importieren…" }).click();

    await expect(page.getByRole("button", { name: /GPX-Track entfernen \(2 Punkte\)/ })).toBeVisible();
  });
});
