import { expect, test } from "@playwright/test";

import { installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO_ID = "01977f4a-0000-7000-8000-000000000101";

const base = { width: 6000, height: 4000, missing: false, camera_model: "Canon EOS R5" };
const PHOTOS = [{ ...base, id: PHOTO_ID, filename: "IMG_0001.CR3" }];

/**
 * Deckt Phase 33 F2 ab: den Ordner-Abgleich. Die Planung (neu,
 * verschwunden, geändert, wieder da; virtuelle Kopien; Umbenennung als
 * zwei Einträge) ist in `apx-app`s `folder_sync`-Tests abgedeckt — hier
 * geht es um den Weg durch die Oberfläche: Vorschau lesen, anwenden,
 * und dass ein übereinstimmender Ordner das auch sagt statt einen
 * leeren Dialog zu zeigen.
 */
test.describe("Ordner-Abgleich (Phase 33 F2)", () => {
  async function open(page: import("@playwright/test").Page) {
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Ordner abgleichen…" }).click();
  }

  test("meldet neue, geänderte und verschwundene Dateien getrennt", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: PHOTOS.length }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
      folderSyncPlan: [
        { filename: "IMG_0001.CR3", change: "modified", photo_id: PHOTO_ID },
        { filename: "IMG_0009.CR3", change: "new", photo_id: null },
        { filename: "IMG_0042.CR3", change: "vanished", photo_id: "01977f4a-0000-7000-8000-000000000199" },
      ],
    });
    await open(page);

    await expect(page.getByTestId("folder-sync-summary")).toContainText("3 Abweichungen");
    const list = page.getByTestId("folder-sync-list");
    await expect(list.getByRole("heading", { name: /Neu im Ordner/ })).toBeVisible();
    await expect(list.getByRole("heading", { name: /Datei geändert/ })).toBeVisible();
    await expect(list.getByRole("heading", { name: /Nicht mehr im Ordner/ })).toBeVisible();
    await expect(list).toContainText("IMG_0009.CR3");
  });

  test("das Anwenden meldet, was passiert ist, und leert die Vorschau", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: PHOTOS.length }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
      folderSyncPlan: [
        { filename: "IMG_0009.CR3", change: "new", photo_id: null },
        { filename: "IMG_0042.CR3", change: "vanished", photo_id: "01977f4a-0000-7000-8000-000000000199" },
      ],
    });
    await open(page);

    await page.getByTestId("folder-sync-apply").click();
    const result = page.getByTestId("folder-sync-result");
    await expect(result).toContainText("1 verschwundene Datei behandelt");
    await expect(result).toContainText("Import für die neuen und geänderten Dateien läuft");
    await expect(page.getByTestId("folder-sync-summary")).toContainText("stimmen überein");
  });

  test("ein übereinstimmender Ordner sagt das auch", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: PHOTOS.length }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
      folderSyncPlan: [],
    });
    await open(page);

    await expect(page.getByTestId("folder-sync-summary")).toContainText("stimmen überein");
    await expect(page.getByTestId("folder-sync-apply")).toBeDisabled();
  });
});
