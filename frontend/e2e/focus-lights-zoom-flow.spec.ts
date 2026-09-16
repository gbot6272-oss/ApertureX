import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const PHOTO = {
  id: "01977f4a-0000-7000-8000-000000000101",
  filename: "IMG_0001.CR3",
  width: 6000,
  height: 4000,
  missing: false,
};

/**
 * Deckt Phase 26 ab (siehe `PLAN.md`, `DECISIONS.md` ADR-0056): die drei
 * neuen Ansichts-Funktionen, die erst durch die schwebende Kopfzeile aus
 * Nachtrag III sinnvoll wurden — Fokus-Modus (alle Paletten aus),
 * "Lichter aus" (dreistufiges Abdunkeln der Umgebung) und die
 * schwebende Zoom-Steuerung im Viewer. Ein Test für alle drei, wie in
 * dieser Sitzung vereinbart (max. ein Test je Schritt, siehe
 * ADR-0038-Nachtrag).
 */
test.describe("Fokus-Modus, Lichter aus, Zoom-Steuerung (Phase 26)", () => {
  test("blendet Paletten aus, dimmt dreistufig und zoomt über die schwebende Steuerung", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: "/home/user/Fotos/Urlaub", photo_count: 1, parent_id: null, missing: false }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();

    const sidebar = page.getByRole("complementary", { name: "Ordner" });
    const zoomGroup = page.getByRole("group", { name: "Zoom" });
    await expect(sidebar).toBeVisible();
    await expect(zoomGroup).toBeVisible();

    // --- Zoom-Steuerung: ohne Tastatur gezielt auf 100 % und zurück ---
    await zoomGroup.getByRole("button", { name: "Zoom 100 Prozent" }).click();
    await expect(page.getByTestId("zoom-readout")).toHaveText("100 %");
    await zoomGroup.getByRole("button", { name: "Zoom einpassen" }).click();
    await expect(zoomGroup.getByRole("button", { name: "Zoom einpassen" })).toHaveAttribute("aria-pressed", "true");

    // --- Fokus-Modus: T blendet ALLE angedockten Paletten aus ---
    await page.keyboard.press("t");
    await expect(sidebar).toBeHidden();
    await expect(page.getByRole("complementary", { name: "Presets" })).toBeHidden();
    // Das Foto selbst (und damit die Zoom-Steuerung darin) bleibt da.
    await expect(zoomGroup).toBeVisible();

    await page.keyboard.press("t");
    await expect(sidebar).toBeVisible();

    // --- Lichter aus: dreistufiger Ringtausch aus → gedimmt → schwarz ---
    const overlay = page.getByTestId("lights-out-overlay");
    await expect(overlay).toHaveCount(0);

    await page.keyboard.press("l");
    await expect(overlay).toHaveCSS("background-color", "rgba(0, 0, 0, 0.55)");

    await page.keyboard.press("l");
    await expect(overlay).toHaveCSS("background-color", "rgba(0, 0, 0, 0.92)");

    await page.keyboard.press("l");
    await expect(overlay).toHaveCount(0);
  });
});
