import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

/**
 * Deckt Phase 12 Schritt 7 ab (`DECISIONS.md` ADR-0039-Nachtrag III): der
 * neue "Import"-Tab im Einstellungsdialog für den beobachteten Ordner
 * (`apx_core::settings::WatchedFolderSettings`). Der eigentliche
 * Hintergrund-Worker (`watched_folder_worker` in `crates/apx-app/src/
 * main.rs`) läuft serverseitig und ist hier nicht testbar (kein laufender
 * Tauri-Kontext) — hier nur die Frontend-Verdrahtung: Ordner wählen und
 * Umschalten speichern tatsächlich über `set_watched_folder_settings`.
 */
test.describe("Einstellungen: Beobachteter Ordner (Phase 12 Schritt 7)", () => {
  test("Ordner wählen und automatischen Import einschalten speichert beides", async ({ page }) => {
    await installTauriMock(page, { selectFolderResult: "/home/user/Fotos/Eingang" });
    await page.goto("/");

    await page.getByRole("button", { name: "Einstellungen…" }).click();
    await page.getByRole("button", { name: "Import", exact: true }).click();

    await page.getByLabel("Beobachteten Ordner automatisch importieren").check();
    await page.getByRole("button", { name: "Wählen…" }).click();

    await expect(page.getByPlaceholder("Wählen…")).toHaveValue("/home/user/Fotos/Eingang");
    await expect(page.getByLabel("Beobachteten Ordner automatisch importieren")).toBeChecked();

    const log = await getMockInvokeLog(page);
    const calls = log.filter((entry) => entry.cmd === "set_watched_folder_settings");
    expect(calls.length).toBeGreaterThan(0);
    const lastArgs = calls[calls.length - 1]?.args as { settings: { path: string | null; enabled: boolean } };
    expect(lastArgs.settings.path).toBe("/home/user/Fotos/Eingang");
    expect(lastArgs.settings.enabled).toBe(true);
  });
});
