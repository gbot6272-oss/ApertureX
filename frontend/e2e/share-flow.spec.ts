import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false };

/**
 * Deckt Phase 9 Schritt 10 ab (`PLAN.md`, `DECISIONS.md` ADR-0035 Punkt 4):
 * Kollaborationsmodus im „Kollaboration…"-Dialog. Der eigentliche
 * `content_hash`-Abgleich (`Catalog::find_photo_by_content_hash`/
 * `diff_share_edit`) ist bereits vollständig in `apx-catalog`s Rust-Unit-
 * Tests abgedeckt — hier bewusst nur ein Frontend-Flow je Tab: Export
 * zeigt den geschriebenen Pfad, Import zeigt einen Konflikt an und löst
 * ihn auf.
 */
test.describe("Kollaboration (Phase 9 Schritt 10)", () => {
  test("Export-Tab: Freigabe der Auswahl zeigt den geschriebenen Pfad", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO] } });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();

    await page.getByRole("button", { name: "Kollaboration…" }).click();
    const dialog = page.getByRole("dialog", { name: "Kollaboration" });
    await expect(dialog.getByText("1 Foto ausgewählt.")).toBeVisible();

    await dialog.getByRole("button", { name: "Als .apxs speichern…" }).click();
    await expect(dialog.getByText("Freigabe geschrieben: /mock/export.apxs")).toBeVisible();
  });

  test("Import-Tab: ein Konflikt lässt sich auflösen und verschwindet aus der Liste", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
      importShareResult: {
        name: "Urlaub-Freigabe",
        unmatched: [],
        unchanged: [],
        conflicts: [
          {
            photo_id: PHOTO.id,
            filename: PHOTO.filename,
            incoming_edl_json: JSON.stringify({ schema_version: 4, payload: {} }),
            prefer_incoming: true,
            local_edited_at: "2026-01-01T00:00:00Z",
            incoming_edited_at: "2026-06-01T00:00:00Z",
          },
        ],
      },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();

    await page.getByRole("button", { name: "Kollaboration…" }).click();
    const dialog = page.getByRole("dialog", { name: "Kollaboration" });
    await dialog.getByRole("button", { name: "Importieren" }).click();
    await dialog.getByRole("button", { name: ".apxs öffnen…" }).click();

    await expect(dialog.getByText(PHOTO.filename)).toBeVisible();
    await dialog.getByRole("button", { name: "Übernehmen" }).click();
    await expect(dialog.getByText("Keine offenen Konflikte.")).toBeVisible();
  });
});
