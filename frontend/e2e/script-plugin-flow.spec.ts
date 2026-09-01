import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false };

/**
 * Deckt Phase 9 Schritt 9 ab (`PLAN.md`, `DECISIONS.md` ADR-0035 Punkt 3):
 * Skript-API (Rhai) + Plugin-System im „Skript & Plugins…"-Dialog. Die
 * eigentliche Rhai-Bridge/ABI-Prüfung (`apx_script::run_script`,
 * `apx_plugin_host::LoadedPlugin`) ist bereits vollständig in Rust-Unit-
 * Tests abgedeckt — hier bewusst nur ein Frontend-Flow je Tab: Skript
 * ausführen zeigt eine Bestätigung, Plugin anwenden zeigt den erzeugten
 * Dateipfad.
 */
test.describe("Skript & Plugins (Phase 9 Schritt 9)", () => {
  test("Skript-Tab: Skript gegen das aktive Foto ausführen zeigt eine Bestätigung", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO] } });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln" }).click();

    await page.getByRole("button", { name: "Skript & Plugins…" }).click();
    const dialog = page.getByRole("dialog", { name: "Skript & Plugins" });
    await expect(dialog.getByRole("button", { name: "Skript" })).toBeVisible();

    await dialog.getByRole("button", { name: "Ausführen" }).click();
    await expect(dialog.getByText("Skript angewendet")).toBeVisible();
  });

  test("Plugin-Tab: Plugin auf das aktive Foto anwenden zeigt den erzeugten Dateipfad", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO] } });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln" }).click();

    await page.getByRole("button", { name: "Skript & Plugins…" }).click();
    const dialog = page.getByRole("dialog", { name: "Skript & Plugins" });
    await dialog.getByRole("button", { name: "Plugin" }).click();
    await dialog.getByLabel("Plugin-Datei (.so/.dylib/.dll)").fill("/pfad/zu/plugin.so");

    await dialog.getByRole("button", { name: "Anwenden" }).click();
    await expect(dialog.getByText(new RegExp(`Plugin angewendet: /mock/derived/${PHOTO.id}-plugin\\.png`))).toBeVisible();
  });
});
