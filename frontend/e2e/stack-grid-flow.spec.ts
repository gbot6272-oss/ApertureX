import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";

const base = { rating: 0, flag: 0, color_label: null, missing: false, width: 6000, height: 4000 };
const PHOTOS = [
  { ...base, id: "01977f4a-0000-7000-8000-000000000101", filename: "SERIE_1.CR3" },
  { ...base, id: "01977f4a-0000-7000-8000-000000000102", filename: "SERIE_2.CR3" },
  { ...base, id: "01977f4a-0000-7000-8000-000000000103", filename: "SERIE_3.CR3" },
  { ...base, id: "01977f4a-0000-7000-8000-000000000104", filename: "EINZELN.CR3" },
];

/**
 * Deckt Phase 33 F10 ab: Stapel im Raster. Die Gruppierungsregeln
 * (Position des ersten sichtbaren Mitglieds, Zählung nur der sichtbaren,
 * Ausweichen bei weggefiltertem Deckblatt) sind in
 * `lib/stackGrouping.test.ts` abgedeckt — hier geht es um den Weg durch
 * die Oberfläche.
 */
test.describe("Stapel im Raster (Phase 33 F10)", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: PHOTOS.length }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
      stacks: [
        {
          id: "stack-1",
          name: "Reihenaufnahme",
          cover_photo_id: PHOTOS[1]!.id,
          photo_ids: [PHOTOS[0]!.id, PHOTOS[1]!.id, PHOTOS[2]!.id],
        },
      ],
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("button", { name: "Raster" }).click();
  });

  test("ein Stapel belegt eingeklappt eine Kachel und zeigt sein Deckblatt", async ({ page }) => {
    const grid = page.getByTestId("photo-grid");
    // Drei Serienbilder werden zu einer Kachel; das Einzelfoto bleibt.
    await expect(grid.getByRole("img")).toHaveCount(2);
    await expect(grid.getByRole("img", { name: "SERIE_2.CR3" })).toBeVisible();
    await expect(grid.getByRole("img", { name: "SERIE_1.CR3" })).toHaveCount(0);
    await expect(grid.getByRole("img", { name: "EINZELN.CR3" })).toBeVisible();

    await expect(page.getByTestId("grid-stack-badge")).toContainText("3");
  });

  test("ein Klick aufs Abzeichen klappt den Stapel auf und wieder zu", async ({ page }) => {
    const grid = page.getByTestId("photo-grid");
    // Die Kachel ist selbst ein `role="button"` und nimmt den Namen
    // des Abzeichens in ihren eigenen auf — deshalb hier über die
    // Test-ID statt über den Namen.
    const badge = page.getByTestId("grid-stack-badge");
    await expect(badge).toHaveAttribute("aria-label", "Stapel mit 3 Fotos aufklappen");
    await badge.click();
    await expect(grid.getByRole("img")).toHaveCount(4);
    await expect(grid.getByRole("img", { name: "SERIE_1.CR3" })).toBeVisible();

    // Aufgeklappt trägt keine Kachel mehr ein Abzeichen.
    await expect(page.getByTestId("grid-stack-badge")).toHaveCount(0);

    await page.getByTestId("toggle-all-stacks").click();
    await expect(grid.getByRole("img")).toHaveCount(2);
  });

  test("der Sammel-Schalter klappt alle Stapel der Ansicht auf", async ({ page }) => {
    const toggle = page.getByTestId("toggle-all-stacks");
    await expect(toggle).toContainText("1 Stapel aufklappen");
    await toggle.click();
    await expect(toggle).toContainText("1 Stapel einklappen");
    await expect(page.getByTestId("photo-grid").getByRole("img")).toHaveCount(4);
  });
});
