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
 * Deckt Phase 8 Schritt 6 ab (`PLAN.md`, `apx_export::web`s Moduldoku):
 * den Web-Galerie-Dialog und den HTML-Export. Die HTML-/Upload-Logik
 * selbst ist bereits vollständig in `apx-export`s Rust-Unit-Tests
 * (`web.rs`) abgedeckt — hier bewusst nur die wichtigsten drei
 * Frontend-Fälle.
 */
test.describe("Web-Galerie (Phase 8 Schritt 6)", () => {
  test("Web-Knopf ist ohne Auswahl deaktiviert, mit Auswahl geht der Dialog auf", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO] } });
    await page.goto("/");
    await expect(page.getByRole("button", { name: "Web…" })).toBeDisabled();

    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Web…" }).click();
    await expect(page.getByText(/^1 Foto —/)).toBeVisible();
  });

  test("Export sendet die gewählten Einstellungen und zeigt das Ergebnis", async ({ page }) => {
    await setUpWithSelectedPhoto(page, { selectFolderResult: "/home/user/Web/Galerie" });
    await page.getByRole("button", { name: "Web…" }).click();

    await page.getByLabel("Titel").fill("Urlaub 2026");
    await page.getByLabel("Theme").selectOption("dark");
    await page.getByRole("button", { name: "Galerie erzeugen" }).click();

    await expect(page.getByText("Gespeichert: /mock/web/galerie (1 Fotos)")).toBeVisible();
    const log = await getMockInvokeLog(page);
    const call = log.find((e) => e.cmd === "export_web_gallery");
    expect(call).toBeDefined();
    const args = call?.args as { photoIds: string[]; destDir: string; options: { title: string; theme: string; upload?: unknown } };
    expect(args.photoIds).toEqual([PHOTO.id]);
    expect(args.destDir).toBe("/home/user/Web/Galerie");
    expect(args.options.title).toBe("Urlaub 2026");
    expect(args.options.theme).toBe("dark");
    expect(args.options.upload).toBeUndefined();
  });

  test("fehlgeschlagener Export zeigt die Fehlermeldung", async ({ page }) => {
    await setUpWithSelectedPhoto(page, { selectFolderResult: "/home/user/Web/Galerie", webExportShouldFail: true });
    await page.getByRole("button", { name: "Web…" }).click();
    await page.getByRole("button", { name: "Galerie erzeugen" }).click();
    await expect(page.getByText(/Fehler: Test-Stub: Web-Export fehlgeschlagen/)).toBeVisible();
  });
});
