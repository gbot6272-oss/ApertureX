import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO_A = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false };
const PHOTO_B = { id: "01977f4a-0000-7000-8000-000000000102", filename: "IMG_0002.CR3", width: 6000, height: 4000, missing: false };

/**
 * Deckt Phase 9 Schritt 8 ab (`PLAN.md`, `DECISIONS.md` ADR-0035 Punkt 2):
 * Fokus-/HDR-/Panorama-/Astro-Stacking im „Stacking…"-Dialog. Die
 * eigentlichen Algorithmen (`apx_stacking::focus`/`hdr`/`panorama`/
 * `astro`) sind bereits vollständig in Rust-Unit-Tests abgedeckt — hier
 * bewusst nur ein Frontend-Flow: zwei Fotos auswählen, Fokus-Stacking
 * auslösen, Statuszeile zeigt das Ergebnis.
 */
test.describe("Stacking (Phase 9 Schritt 8)", () => {
  test("Fokus-Stacking über zwei ausgewählte Fotos zeigt das Ergebnis", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO_A, PHOTO_B] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("button", { name: "Raster" }).click();
    await page.getByRole("img", { name: PHOTO_A.filename }).first().click();
    await page.getByRole("img", { name: PHOTO_B.filename }).first().click({ modifiers: ["Control"] });

    await page.getByRole("button", { name: "Stacking…" }).click();
    const dialog = page.getByRole("dialog", { name: "Stacking" });
    await expect(dialog.getByText("2 Fotos ausgewählt")).toBeVisible();

    await dialog.getByRole("button", { name: "Fokus-Stacking" }).click();
    await expect(dialog.getByText(/Fokus-Stack fertig: \d+×\d+/)).toBeVisible();
  });
});
