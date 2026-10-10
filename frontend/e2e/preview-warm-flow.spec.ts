import { expect, test } from "@playwright/test";

import { emitMockEvent, installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTOS = [
  { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false },
  { id: "01977f4a-0000-7000-8000-000000000102", filename: "IMG_0002.CR3", width: 6000, height: 4000, missing: false },
];

/**
 * Deckt Phase 34 F10 ab (`DECISIONS.md` ADR-0070): das Vorbereiten der
 * 2048px-Vorschauen. Das Dekodieren selbst ist Rust-Sache und im Mock
 * gar nicht da — hier geht es um die drei Zusagen der Oberfläche: dass
 * vorher eine Zahl dasteht, dass der Fortschritt ankommt, und dass ein
 * Abbruch das Fertige stehen lässt.
 */
test.describe("Vorschauen vorbereiten (Phase 34 F10)", () => {
  async function openDialog(page: import("@playwright/test").Page) {
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: /Vorschauen vorbereiten/ }).click();
    return page.getByRole("dialog", { name: "Vorschauen vorbereiten" });
  }

  test("sagt vorher, wie viel Arbeit anfällt, und meldet danach das Ergebnis", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2 }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
    });
    const dialog = await openDialog(page);

    await expect(dialog.getByTestId("preview-warm-summary")).toContainText("2 Fotos offen");
    await dialog.getByTestId("preview-warm-start").click();
    await expect(dialog.getByTestId("preview-warm-progress")).toBeVisible();

    await emitMockEvent(page, "preview-warm:progress", { done: 1, total: 2, current_file: "IMG_0001.CR3" });
    await expect(dialog.getByTestId("preview-warm-progress")).toContainText("1 von 2");

    await emitMockEvent(page, "preview-warm:finished", { prepared: 2, failed: 0, cancelled: false });
    await expect(dialog.getByTestId("preview-warm-result")).toContainText("2 vorbereitet");
    // Die Zahl wird danach neu gerechnet: es ist nichts mehr offen.
    await expect(dialog.getByTestId("preview-warm-summary")).toContainText("0 Fotos offen");
  });

  test("zählt bereits vorbereitete Fotos nicht noch einmal", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2 }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
      warmedPreviewIds: [PHOTOS[0].id],
    });
    const dialog = await openDialog(page);

    await expect(dialog.getByTestId("preview-warm-summary")).toContainText("1 Foto offen · 1 schon vorbereitet");

    // „Vorhandene neu berechnen" nimmt beide wieder mit.
    await dialog.getByLabel("Vorhandene Vorschauen neu berechnen").check();
    await expect(dialog.getByTestId("preview-warm-summary")).toContainText("2 Fotos offen");
  });

  test("ein Abbruch lässt das Fertige stehen", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2 }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
    });
    const dialog = await openDialog(page);
    await dialog.getByTestId("preview-warm-start").click();
    await dialog.getByTestId("preview-warm-cancel").click();

    await emitMockEvent(page, "preview-warm:finished", { prepared: 1, failed: 0, cancelled: true });
    await expect(dialog.getByTestId("preview-warm-result")).toContainText("abgebrochen, das Fertige bleibt erhalten");
  });
});
