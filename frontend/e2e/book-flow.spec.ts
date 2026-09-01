import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false };

async function setUpWithSelectedPhoto(page: import("@playwright/test").Page, extraFixtures: Record<string, unknown> = {}) {
  await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO] }, ...extraFixtures });
  await page.goto("/");
  await page.getByRole("button", { name: /Urlaub/ }).click();
  await page.getByRole("img", { name: PHOTO.filename }).click();
}

/**
 * Deckt Phase 8 Schritt 5 ab (`PLAN.md`, `apx_export::book`s Moduldoku):
 * den Buch-Dialog und den PDF-Export. Die Layout-/PDF-Logik selbst ist
 * bereits vollständig in `apx-export`s Rust-Unit-Tests (`book.rs`)
 * abgedeckt — hier bewusst nur die wichtigsten drei Frontend-Fälle.
 */
test.describe("Buch (Phase 8 Schritt 5)", () => {
  test("Buch-Knopf ist ohne Auswahl deaktiviert, mit Auswahl geht der Dialog auf", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO] } });
    await page.goto("/");
    await expect(page.getByRole("button", { name: "Buch…" })).toBeDisabled();

    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Buch…" }).click();
    await expect(page.getByText(/^1 Foto —/)).toBeVisible();
  });

  test("Export sendet die gewählten Einstellungen und zeigt das Ergebnis", async ({ page }) => {
    await setUpWithSelectedPhoto(page, { pickSaveFilePathResult: "/home/user/Buecher/Fotobuch.pdf" });
    await page.getByRole("button", { name: "Buch…" }).click();

    await page.getByLabel("Seitenvorlage").selectOption("full_bleed");
    await page.getByLabel("Titelseite (leer = keine)").fill("Urlaub 2026");
    await page.getByRole("button", { name: "Als PDF speichern" }).click();

    await expect(page.getByText("Gespeichert: /mock/book/Fotobuch.pdf (2 Seiten)")).toBeVisible();
    const log = await getMockInvokeLog(page);
    const call = log.find((e) => e.cmd === "export_book_pdf");
    expect(call).toBeDefined();
    const args = call?.args as { photoIds: string[]; destPath: string; options: { template: string; title?: string } };
    expect(args.photoIds).toEqual([PHOTO.id]);
    expect(args.destPath).toBe("/home/user/Buecher/Fotobuch.pdf");
    expect(args.options.template).toBe("full_bleed");
    expect(args.options.title).toBe("Urlaub 2026");
  });

  test("fehlgeschlagener Export zeigt die Fehlermeldung", async ({ page }) => {
    await setUpWithSelectedPhoto(page, { pickSaveFilePathResult: "/home/user/Buecher/Fotobuch.pdf", bookExportShouldFail: true });
    await page.getByRole("button", { name: "Buch…" }).click();
    await page.getByRole("button", { name: "Als PDF speichern" }).click();
    await expect(page.getByText(/Fehler: Test-Stub: Buch-Export fehlgeschlagen/)).toBeVisible();
  });
});
