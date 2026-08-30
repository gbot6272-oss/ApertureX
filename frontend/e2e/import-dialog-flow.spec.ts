import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock, setMockFixtures } from "./tauri-mock";

const TARGET_DIR = "/home/user/Fotos/Ziel";

/**
 * Deckt Phase 5 Schritt 9 ab (`DECISIONS.md` ADR-0031 Punkt 7): den
 * erweiterten Import-Dialog, der den seit Phase 3 im Backend bestehenden
 * Kopieren-/Verschieben-Modus samt Umbenennungsmuster und Presets ans
 * Frontend anbindet — additiv zum einfachen „Ordner importieren"-Knopf
 * (weiterhin `import_folder`, siehe `viewer-flow.spec.ts`).
 */
test.describe("Import mit Vorlage", () => {
  test("wählt Kopieren-Modus mit Umbenennungsmuster, speichert als Preset und importiert", async ({ page }) => {
    await installTauriMock(page);
    await page.goto("/");
    // Der Mock hat nur einen einzigen `selectFolderResult` (kein
    // Ordnerauswahl-Dialog-Verlauf) — Quell- und Zielordner-Auswahl liefern
    // hier deshalb denselben Pfad zurück, was für die Assertions unten
    // unerheblich ist (Quelle wird als `path`, Ziel als `mode.target_dir`
    // separat geprüft).
    await setMockFixtures(page, { selectFolderResult: TARGET_DIR });

    await page.getByRole("button", { name: "Import mit Vorlage…" }).click();
    const dialog = page.getByRole("dialog", { name: "Import mit Vorlage" });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByText(`Quelle: ${TARGET_DIR}`)).toBeVisible();

    // Importieren ist ohne Zielordner blockiert, sobald Kopieren gewählt ist.
    await dialog.getByRole("radio", { name: "Kopieren nach…" }).check();
    await expect(dialog.getByRole("button", { name: "Importieren" })).toBeDisabled();
    await dialog.getByRole("button", { name: "Wählen…" }).click();
    await expect(dialog.getByLabel("Zielordner")).toHaveValue(TARGET_DIR);
    await expect(dialog.getByRole("button", { name: "Importieren" })).toBeEnabled();

    // Umbenennungsmuster über die Token-Knöpfe zusammensetzen, Vorschau prüfen.
    await dialog.getByRole("button", { name: "{date}", exact: true }).click();
    await dialog.getByLabel("Umbenennungsmuster").fill("{date}_{seq}");
    await expect(dialog.getByText(/Vorschau:/)).toContainText("_0007");

    // Als Preset speichern.
    await dialog.getByLabel("Preset-Name").fill("Mein Import-Preset");
    await dialog.getByRole("button", { name: "Speichern", exact: true }).click();
    await expect(dialog.getByLabel("Import-Preset")).toContainText("Mein Import-Preset");

    await dialog.getByRole("button", { name: "Importieren" }).click();
    await expect(dialog).not.toBeVisible();

    const log = await getMockInvokeLog(page);
    const call = log.find((entry) => entry.cmd === "import_folder_with_mode");
    expect(call).toBeDefined();
    const args = call?.args as { path: string; mode: { kind: string; target_dir?: string }; renamePattern: string | null };
    expect(args.path).toBe(TARGET_DIR);
    expect(args.mode).toEqual({ kind: "Copy", target_dir: TARGET_DIR });
    expect(args.renamePattern).toBe("{date}_{seq}");
  });

  test("wendet ein gespeichertes Preset an und übernimmt seinen Modus", async ({ page }) => {
    await installTauriMock(page, {
      importPresets: [
        {
          name: "Urlaub-Preset",
          mode: { mode: "Move", target_dir: TARGET_DIR },
          rename_pattern: "{camera}_{original}",
        },
      ],
    });
    await page.goto("/");
    await setMockFixtures(page, { selectFolderResult: "/home/user/Fotos/Quelle" });

    await page.getByRole("button", { name: "Import mit Vorlage…" }).click();
    const dialog = page.getByRole("dialog", { name: "Import mit Vorlage" });
    await dialog.getByLabel("Import-Preset").selectOption({ label: "Urlaub-Preset" });

    await expect(dialog.getByRole("radio", { name: "Verschieben nach…" })).toBeChecked();
    await expect(dialog.getByLabel("Zielordner")).toHaveValue(TARGET_DIR);
    await expect(dialog.getByLabel("Umbenennungsmuster")).toHaveValue("{camera}_{original}");

    await dialog.getByRole("button", { name: "Importieren" }).click();
    const log = await getMockInvokeLog(page);
    const call = log.find((entry) => entry.cmd === "import_folder_with_mode");
    const args = call?.args as { mode: { kind: string; target_dir?: string } };
    expect(args.mode).toEqual({ kind: "Move", target_dir: TARGET_DIR });
  });
});
