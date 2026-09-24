import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";

// Drei Aufnahmen am 4. Mai, eine am 6. Mai, eine ohne Datum — deckt die
// drei Fälle ab, die die Ansicht unterscheiden muss: starker Tag,
// schwacher Tag, undatiert.
const base = { width: 6000, height: 4000, missing: false };
const PHOTOS = [
  { ...base, id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", captured_at: "2024-05-04T10:00:00+02:00" },
  { ...base, id: "01977f4a-0000-7000-8000-000000000102", filename: "IMG_0002.CR3", captured_at: "2024-05-04T11:00:00+02:00" },
  { ...base, id: "01977f4a-0000-7000-8000-000000000103", filename: "IMG_0003.CR3", captured_at: "2024-05-04T12:00:00+02:00" },
  { ...base, id: "01977f4a-0000-7000-8000-000000000104", filename: "IMG_0004.CR3", captured_at: "2024-05-06T09:00:00+02:00" },
  { ...base, id: "01977f4a-0000-7000-8000-000000000105", filename: "IMG_0005.CR3", captured_at: null },
];

/**
 * Deckt Phase 32 F3 ab: die Kalenderansicht. Die Datumsmathematik selbst
 * ist in `src/lib/calendarGrid.test.ts` abgedeckt — hier geht es um den
 * Weg durch die Oberfläche: Ansicht öffnen, Tag anklicken, Tagespanel,
 * ganzen Tag auswählen.
 */
test.describe("Kalenderansicht (Phase 32 F3)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: PHOTOS.length }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("button", { name: "Kalender" }).click();
  });

  test("zeigt Monate, Tageszahlen und die undatierten Fotos gesondert", async ({ page }) => {
    await expect(page.getByTestId("calendar-view")).toBeVisible();
    await expect(page.getByTestId("calendar-summary")).toContainText("4 Aufnahmen an 2 Tagen");
    // Das Foto ohne Aufnahmedatum wird nicht geraten, sondern benannt.
    await expect(page.getByText("1 ohne Aufnahmedatum", { exact: false })).toBeVisible();

    const may = page.getByRole("region", { name: "Mai 2024" });
    await expect(may).toBeVisible();
    await expect(may.getByRole("button", { name: "4. Mai 2024: 3 Aufnahmen", exact: true })).toBeVisible();
    await expect(may.getByRole("button", { name: "6. Mai 2024: 1 Aufnahme", exact: true })).toBeVisible();
    // Ein Tag ohne Aufnahmen ist sichtbar, aber nicht anklickbar.
    await expect(may.getByRole("button", { name: "5. Mai 2024: 0 Aufnahmen", exact: true })).toBeDisabled();
  });

  test("Tagesklick öffnet das Tagespanel mit genau den Fotos dieses Tages", async ({ page }) => {
    await page.getByRole("button", { name: "4. Mai 2024: 3 Aufnahmen", exact: true }).click();

    const panel = page.getByTestId("calendar-day-panel");
    await expect(panel).toBeVisible();
    await expect(panel.getByRole("heading")).toContainText("4. Mai 2024");
    await expect(panel.getByRole("listitem")).toHaveCount(3);

    // Weiterblättern springt auf den nächsten Tag MIT Aufnahmen, nicht
    // auf den leeren 5. Mai.
    await panel.getByRole("button", { name: "Nächster Tag mit Aufnahmen" }).click();
    await expect(panel.getByRole("heading")).toContainText("6. Mai 2024");
    await expect(panel.getByRole("listitem")).toHaveCount(1);
  });

  test("„Ganzen Tag auswählen“ füllt die Mehrfachauswahl", async ({ page }) => {
    await page.getByRole("button", { name: "4. Mai 2024: 3 Aufnahmen", exact: true }).click();
    await page.getByTestId("calendar-day-panel").getByRole("button", { name: "Ganzen Tag auswählen" }).click();

    // Es ist dieselbe Mehrfachauswahl, die das Entwickeln-Panel zum
    // Synchronisieren benutzt — dort abgelesen statt im Store, damit der
    // Test den Weg prüft, den ein Nutzer auch sieht: drei Fotos des Tages
    // = das aktive plus zwei weitere.
    await page.getByRole("button", { name: "Entwickeln" }).click();
    await page.getByRole("tab", { name: "Verlauf & Werkzeuge" }).click();
    await expect(page.getByRole("button", { name: "Auf 2 weitere ausgewählte Fotos synchronisieren" })).toBeEnabled();
  });

  /**
   * Regressionstest zu ADR-0068: bis Phase 34 bekam kein importiertes
   * JPEG/PNG/TIFF ein Aufnahmedatum, der Kalender blieb fuer solche
   * Kataloge komplett leer — und die Ansicht sagte zwar, wie viele
   * Fotos betroffen sind, aber nicht, was man dagegen tun kann.
   */
  test("laesst die Aufnahmedaten der undatierten Fotos nachlesen", async ({ page }) => {
    await expect(page.getByTestId("calendar-summary")).toContainText("4 Aufnahmen an 2 Tagen");
    await page.getByTestId("calendar-rescan").click();

    await expect(page.getByTestId("calendar-rescan-result")).toContainText("1 Foto hat jetzt ein Aufnahmedatum");
    // Und der Kalender zeigt es danach wirklich an, statt es nur zu melden.
    await expect(page.getByTestId("calendar-summary")).toContainText("5 Aufnahmen an 2 Tagen");
  });
});
