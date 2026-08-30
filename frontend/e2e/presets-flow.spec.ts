import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000401";
const FOLDER_PATH = "/home/user/Fotos/Presets-Test";
const PHOTO = {
  id: "01977f4a-0000-7000-8000-000000000402",
  filename: "IMG_0201.CR3",
  width: 6000,
  height: 4000,
  camera_make: "Canon",
  camera_model: "EOS R5",
  lens: "RF 24-70mm",
  iso: 200,
  aperture: 4,
  shutter: 1 / 250,
  focal_length: 50,
  captured_at: "2024-06-01T10:00:00Z",
  missing: false,
};

const PRESET_FOLDER = { id: "preset-folder-seed-1", name: "Filmlooks", parent_id: null, position: 0 };
const PRESET = {
  id: "preset-seed-1",
  folder_id: PRESET_FOLDER.id,
  name: "Warmer Filmlook",
  is_favorite: false,
  tags: ["warm", "film"],
  conditions_json: "[]",
  created_at: "2026-01-01T00:00:00Z",
};

async function setUp(page: import("@playwright/test").Page) {
  await installTauriMock(page, {
    folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
    photosByFolder: { [FOLDER_ID]: [PHOTO] },
    presetFolders: [PRESET_FOLDER],
    presets: [PRESET],
  });
  await page.goto("/");
  await page.getByRole("button", { name: /Presets-Test/ }).click();
  await page.getByRole("img", { name: PHOTO.filename }).click();
  await page.getByRole("button", { name: "Entwickeln" }).click();
}

/**
 * Deckt Phase 5 Schritt 3 ab (`DECISIONS.md` ADR-0031): Ordnerbaum
 * anzeigen/anlegen/umbenennen/löschen sowie Presets favorisieren/
 * umbenennen/verschieben/löschen — reines Organisieren bereits
 * vorhandener Presets (Anlegen aus dem aktuellen Entwickeln-Zustand ist
 * `SavePresetDialog`, Schritt 4, und dort getestet).
 */
test.describe("Presets-Panel", () => {
  test("zeigt den Ordnerbaum, filtert die Presetliste nach Ordner und favorisiert ein Preset", async ({ page }) => {
    await setUp(page);

    await expect(page.getByRole("heading", { name: "Presets" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Wurzel" })).toBeVisible();
    await expect(page.getByRole("button", { name: PRESET_FOLDER.name })).toBeVisible();

    // Wurzel-Ansicht ist anfangs ausgewählt — das Preset gehört zu
    // "Filmlooks", ist also zunächst nicht sichtbar.
    await expect(page.getByText(PRESET.name)).not.toBeVisible();

    await page.getByRole("button", { name: PRESET_FOLDER.name }).click();
    await expect(page.getByText(PRESET.name)).toBeVisible();

    const favoriteButton = page.getByRole("button", { name: `${PRESET.name} zu Favoriten hinzufügen` });
    await favoriteButton.click();
    await expect(page.getByRole("button", { name: `${PRESET.name} aus Favoriten entfernen` })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  test("legt einen Unterordner an, verschiebt das Preset dorthin und löscht es danach", async ({ page }) => {
    await setUp(page);

    await page.getByRole("button", { name: PRESET_FOLDER.name }).click();
    await page.getByRole("textbox", { name: "Neuer Preset-Ordner" }).fill("Portrait");
    await page.getByRole("button", { name: "Ordner anlegen" }).click();

    const newFolderButton = page.getByRole("button", { name: "Portrait" });
    await expect(newFolderButton).toBeVisible();

    await page.getByRole("button", { name: PRESET_FOLDER.name }).click();
    await page.getByRole("combobox", { name: `${PRESET.name}: Ordner` }).selectOption({ label: "Portrait" });

    await newFolderButton.click();
    await expect(page.getByText(PRESET.name)).toBeVisible();

    await page.getByRole("button", { name: `${PRESET.name} löschen` }).click();
    await expect(page.getByText(PRESET.name)).not.toBeVisible();
    await expect(page.getByText("Keine Presets in diesem Ordner.")).toBeVisible();
  });

  test("speichert die aktuellen Einstellungen als neues Preset mit den ausgewählten Sektionen", async ({ page }) => {
    await setUp(page);

    // Regler-Wert setzen, damit die gespeicherte EDL-Teilmenge auch
    // tatsächlich etwas von neutral Abweichendes enthält.
    const exposureInput = page.getByRole("spinbutton", { name: "Belichtung (Zahlenwert)" });
    await exposureInput.fill("0.6");
    await exposureInput.blur();

    await page.getByRole("button", { name: "Preset speichern" }).click();
    const dialog = page.getByRole("dialog", { name: "Preset speichern" });
    await expect(dialog).toBeVisible();

    await dialog.getByLabel("Name").fill("Mein neues Preset");
    // Nur "Grundeinstellungen" behalten — alle anderen Sektionen abwählen.
    for (const label of ["Kurven", "HSL", "Farbmischer", "Color Grading", "Details", "Objektivkorrekturen", "Effekte", "Kalibrierung", "Geometrie"]) {
      await dialog.getByLabel(label).uncheck();
    }
    await dialog.getByRole("button", { name: "Speichern" }).click();

    await expect(dialog).not.toBeVisible();
    // Neues Preset landet an der Wurzel (kein Ordner ausgewählt) — die
    // Wurzel-Ansicht ist bereits der Standardzustand des Panels.
    await expect(page.getByText("Mein neues Preset")).toBeVisible();
  });

  async function readLastCommittedExposure(page: import("@playwright/test").Page): Promise<number> {
    const log = await getMockInvokeLog(page);
    const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
    const edlJson = (lastCommit?.args as { edlJson: string }).edlJson;
    return (JSON.parse(edlJson).payload.basic as { exposure_ev: number }).exposure_ev;
  }

  test("wendet ein Preset an und passt die Stärke nachträglich an", async ({ page }) => {
    await setUp(page);

    // Preset mit Belichtung 0.6 speichern, Regler danach wieder auf 0
    // zurücksetzen, damit "Anwenden" sichtbar etwas ändert.
    const exposureInput = page.getByRole("spinbutton", { name: "Belichtung (Zahlenwert)" });
    await exposureInput.fill("0.6");
    await exposureInput.blur();
    await page.getByRole("button", { name: "Preset speichern" }).click();
    const strengthTestDialog = page.getByRole("dialog", { name: "Preset speichern" });
    await strengthTestDialog.getByLabel("Name").fill("Stärke-Test");
    for (const label of ["Kurven", "HSL", "Farbmischer", "Color Grading", "Details", "Objektivkorrekturen", "Effekte", "Kalibrierung", "Geometrie"]) {
      await strengthTestDialog.getByLabel(label).uncheck();
    }
    await strengthTestDialog.getByRole("button", { name: "Speichern" }).click();
    await exposureInput.fill("0");
    await exposureInput.blur();

    await page.getByRole("button", { name: "Stärke-Test", exact: true }).click();
    await expect.poll(async () => readLastCommittedExposure(page)).toBeCloseTo(0.6, 2);

    const strengthInput = page.getByRole("spinbutton", { name: "Stärke (%) (Zahlenwert)" });
    await expect(strengthInput).toBeVisible();
    await strengthInput.fill("50");
    await strengthInput.blur();
    await expect.poll(async () => readLastCommittedExposure(page)).toBeCloseTo(0.3, 2);

    await page.getByRole("button", { name: "Stärke-Regler schließen" }).click();
    await expect(strengthInput).not.toBeVisible();
  });

  test("Preset-Stapel: zwei Presets nacheinander anwenden, Reihenfolge editierbar", async ({ page }) => {
    await setUp(page);

    async function savePreset(name: string, exposure: string) {
      await exposureInput.fill(exposure);
      await exposureInput.blur();
      await page.getByRole("button", { name: "Preset speichern" }).click();
      const dialog = page.getByRole("dialog", { name: "Preset speichern" });
      await dialog.getByLabel("Name").fill(name);
      for (const label of ["Kurven", "HSL", "Farbmischer", "Color Grading", "Details", "Objektivkorrekturen", "Effekte", "Kalibrierung", "Geometrie"]) {
        await dialog.getByLabel(label).uncheck();
      }
      await dialog.getByRole("button", { name: "Speichern" }).click();
      await expect(dialog).not.toBeVisible();
    }

    const exposureInput = page.getByRole("spinbutton", { name: "Belichtung (Zahlenwert)" });
    await savePreset("Preset A", "0.2");
    await savePreset("Preset B", "0.9");

    await page.getByRole("button", { name: "Preset A zum Stapel hinzufügen" }).click();
    await page.getByRole("button", { name: "Preset B zum Stapel hinzufügen" }).click();
    await expect(page.getByRole("heading", { name: "Preset-Stapel" })).toBeVisible();

    await page.getByRole("button", { name: "Stapel anwenden" }).click();
    // Späterer Stapel-Eintrag (Preset B) gewinnt bei derselben Sektion.
    await expect.poll(async () => readLastCommittedExposure(page)).toBeCloseTo(0.9, 2);
  });

  test("benennt einen Preset-Ordner über den Dialog um", async ({ page }) => {
    await setUp(page);

    page.once("dialog", (dialog) => {
      void dialog.accept("Umbenannt");
    });
    await page.getByTitle("Ordner umbenennen").click();

    await expect(page.getByRole("button", { name: "Umbenannt" })).toBeVisible();
    await expect(page.getByRole("button", { name: PRESET_FOLDER.name })).not.toBeVisible();
  });
});
