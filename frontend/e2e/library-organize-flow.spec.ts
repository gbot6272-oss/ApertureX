import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO_A = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false };
const PHOTO_B = { id: "01977f4a-0000-7000-8000-000000000102", filename: "IMG_0002.CR3", width: 6000, height: 4000, missing: false };

/**
 * Deckt Phase 9 Schritt 1 ab (`PLAN.md`, `DECISIONS.md` ADR-0032/ADR-0035):
 * den Bibliotheks-Backlog-Dialog. Die eigentliche CRUD-/Gruppierungslogik
 * ist bereits vollständig in `apx-catalog`s Rust-Unit-Tests abgedeckt —
 * hier bewusst nur drei Frontend-Flows, je einer der fünf Bereiche stellvertretend.
 */
test.describe("Bibliothek organisieren (Phase 9 Schritt 1)", () => {
  test("Sammlungssatz anlegen erscheint in der Liste", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO_A] } });
    await page.goto("/");
    await page.getByRole("button", { name: "Organisieren…" }).click();
    await page.getByPlaceholder("Neuer Sammlungssatz").fill("Reisen");
    await page.getByRole("button", { name: "Anlegen" }).first().click();
    await expect(page.getByText("Reisen")).toBeVisible();
  });

  test("Zwei ausgewählte Fotos stapeln erzeugt einen Stapel", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2 }], photosByFolder: { [FOLDER_ID]: [PHOTO_A, PHOTO_B] } });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO_A.filename }).click();
    await page.getByRole("img", { name: PHOTO_B.filename }).click({ modifiers: ["Control"] });

    await page.getByRole("button", { name: "Organisieren…" }).click();
    await page.getByRole("button", { name: "Stapel" }).click();
    await page.getByRole("button", { name: "Aus Auswahl stapeln" }).click();

    await expect(page.getByText("Stapel — 2 Fotos")).toBeVisible();
  });

  test("Duplikat-Assistent zeigt Gruppen mit einer Vorschlags-Markierung", async ({ page }) => {
    const small = { ...PHOTO_A, width: 1000, height: 800, file_size: 500 };
    const large = { ...PHOTO_B, width: 6000, height: 4000, file_size: 9000 };
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2 }],
      photosByFolder: { [FOLDER_ID]: [small, large] },
      perceptualDuplicateGroups: [[small, large]],
    });
    await page.goto("/");
    await page.getByRole("button", { name: "Organisieren…" }).click();
    await page.getByRole("button", { name: "Duplikate" }).click();
    await page.getByRole("button", { name: "Duplikate suchen" }).click();

    await expect(page.getByText("Gruppe 1 (2 Fotos)")).toBeVisible();
    const largeRow = page.locator("div").filter({ hasText: new RegExp(`^${large.filename}`) }).last();
    await expect(largeRow.getByText("Vorschlag")).toBeVisible();
  });
});
