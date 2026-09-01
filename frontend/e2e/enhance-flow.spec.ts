import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false };

/**
 * Deckt Phase 9 Schritt 6 ab (`PLAN.md`, `DECISIONS.md` ADR-0035):
 * Entrauschung/Hochskalierung im Entwickeln-Panel. Die Algorithmen selbst
 * (`apx_ai::denoise`/`apx_ai::upscale`) sind bereits vollständig in
 * Rust-Unit-Tests abgedeckt — hier bewusst nur ein Frontend-Flow: beide
 * Knöpfe lösen den jeweiligen Aufruf aus und zeigen den Ziel-Pfad an.
 */
test.describe("Entrauschung & Hochskalierung (Phase 9 Schritt 6)", () => {
  test("Entrauschen und Hochskalieren zeigen jeweils den Ziel-Pfad an", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
      denoisedPhotoPath: "/home/user/Fotos/Urlaub/IMG_0001_entrauscht.png",
      upscaledPhotoPath: "/home/user/Fotos/Urlaub/IMG_0001_hochskaliert.png",
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln" }).click();

    const group = page.getByRole("group", { name: "Entrauschung & Hochskalierung" });
    await group.getByRole("button", { name: "Entrauschen" }).click();
    await expect(page.getByText("Entrauscht: /home/user/Fotos/Urlaub/IMG_0001_entrauscht.png")).toBeVisible();

    await group.getByRole("button", { name: "2× hochskalieren" }).click();
    await expect(page.getByText("Hochskaliert: /home/user/Fotos/Urlaub/IMG_0001_hochskaliert.png")).toBeVisible();
  });
});
