import { expect, test } from "@playwright/test";

import { installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000901";
const base = { width: 6000, height: 4000, missing: false, captured_at: "2024-05-04T10:00:00Z" };
// Eines ist gepflegt, zwei haben Luecken.
const GEPFLEGT = { ...base, id: "01977f4a-0000-7000-8000-000000000902", filename: "GEPFLEGT.CR3", rating: 4, gps_lat: 50.0, gps_lon: 8.0 };
const OHNE_BEWERTUNG = { ...base, id: "01977f4a-0000-7000-8000-000000000903", filename: "UNBEWERTET.CR3", rating: 0, gps_lat: 50.0, gps_lon: 8.0 };
const VERSCHWUNDEN = { ...base, id: "01977f4a-0000-7000-8000-000000000904", filename: "WEG.CR3", rating: 5, gps_lat: 50.0, gps_lon: 8.0, missing: true };

/**
 * Phase 34 F5 (siehe `DECISIONS.md` ADR-0070). Die Abfragen selbst
 * liegen in `repository::health`s zehn Rust-Tests — hier geht es um den
 * Punkt, an dem sich der Dialog von einem Zahlen-Dashboard
 * unterscheidet: jede Zeile fuehrt zur Arbeit.
 */
test.describe("Katalog-Gesundheit (Phase 34 F5)", () => {
  test("zaehlt die Luecken und legt sie auf Klick in die Auswahl", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: "/home/user/Fotos/Bestand", photo_count: 3, parent_id: null, missing: false }],
      photosByFolder: { [FOLDER_ID]: [GEPFLEGT, OHNE_BEWERTUNG, VERSCHWUNDEN] },
      keywordsByPhoto: { [GEPFLEGT.id]: [{ id: "kw-1", name: "Hafen" }] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Bestand/ }).click();
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Katalog-Gesundheit…" }).click();

    await expect(page.getByTestId("catalog-health-count-without_rating")).toHaveText("1");
    await expect(page.getByTestId("catalog-health-count-missing")).toHaveText("1");
    // Das gepflegte Foto hat ein Schlagwort, die beiden anderen nicht.
    await expect(page.getByTestId("catalog-health-count-without_keywords")).toHaveText("2");
    // Alle drei haben Datum und Ort — diese Zeile ist bei null.
    await expect(page.getByTestId("catalog-health-count-without_position")).toHaveText("0");
    await expect(page.getByTestId("catalog-health-show-without_position")).toBeDisabled();

    await page.getByTestId("catalog-health-show-without_rating").click();
    await expect(page.getByTestId("catalog-health-picked")).toContainText("1 Foto ausgewählt");
    await expect(page.getByTestId("catalog-health-picked")).toContainText("Nie bewertet");
  });

  test("meldet einen gepflegten Katalog als solchen", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: "/home/user/Fotos/Bestand", photo_count: 1, parent_id: null, missing: false }],
      photosByFolder: { [FOLDER_ID]: [GEPFLEGT] },
      keywordsByPhoto: { [GEPFLEGT.id]: [{ id: "kw-1", name: "Hafen" }] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Bestand/ }).click();
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Katalog-Gesundheit…" }).click();

    await expect(page.getByTestId("catalog-health-summary")).toContainText("Keine Lücken gefunden");
  });
});
