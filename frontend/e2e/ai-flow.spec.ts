import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock, setMockFixtures } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = {
  id: "01977f4a-0000-7000-8000-000000000101",
  filename: "IMG_0001.CR3",
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

async function setUpWithSelectedPhoto(page: import("@playwright/test").Page) {
  await installTauriMock(page, {
    folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
    photosByFolder: { [FOLDER_ID]: [PHOTO] },
  });
  await page.goto("/");
  await page.getByRole("button", { name: /Urlaub/ }).click();
  await page.getByRole("img", { name: PHOTO.filename }).click();
  await page.getByRole("button", { name: "Entwickeln" }).click();
}

async function lastCommit(page: import("@playwright/test").Page) {
  const log = await getMockInvokeLog(page);
  const entry = [...log].reverse().find((e) => e.cmd === "apply_develop_edit");
  if (!entry) throw new Error("kein apply_develop_edit im Protokoll");
  return JSON.parse((entry.args as { edlJson: string }).edlJson).payload;
}

/**
 * Deckt Phase 7 ab (`DECISIONS.md` ADR-0033): die fünf KI-Masken,
 * Reparatur-Erweiterungen (Auto-Quellenfindung/Sensorflecken), den
 * Preset-Generator und Auto-Tagging — jeweils ein Kernszenario pro
 * Funktion, nicht jede Variante (die liegt schon in den Rust-Unit-Tests
 * der jeweiligen `apx-ai`-Module).
 */
test.describe("KI-Funktionen (Phase 7)", () => {
  test("KI-Maske 'Himmel' erzeugt sofort eine AiGenerated-Maske", async ({ page }) => {
    await setUpWithSelectedPhoto(page);

    await expect(page.getByText("KI-Maske hinzufügen")).toBeVisible();
    await page.getByRole("button", { name: "Himmel", exact: true }).click();

    const payload = await lastCommit(page);
    expect(payload.masks).toHaveLength(1);
    expect(payload.masks[0].components[0].geometry.kind).toBe("AiGenerated");
    expect(payload.masks[0].components[0].geometry.ai_kind).toBe("Sky");
    expect(payload.masks[0].components[0].geometry.alpha).toHaveLength(16);

    const log = await getMockInvokeLog(page);
    const call = log.find((e) => e.cmd === "generate_ai_mask");
    expect((call?.args as { kind: string }).kind).toBe("sky");
  });

  test("KI-Maske 'Objekte' braucht einen Bildklick als Startpunkt", async ({ page }) => {
    await setUpWithSelectedPhoto(page);

    const pickButton = page.getByRole("button", { name: "Objekte…" });
    await pickButton.click();
    await expect(pickButton).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByText("Klicken Sie ins Bild, um den Objektbereich auszuwählen.")).toBeVisible();

    await page.getByRole("main").click();

    await expect(pickButton).toHaveAttribute("aria-pressed", "false");
    const payload = await lastCommit(page);
    expect(payload.masks[0].components[0].geometry.ai_kind).toBe("ClickRegion");
    const call = (await getMockInvokeLog(page)).find((e) => e.cmd === "generate_ai_mask");
    expect((call?.args as { clickX: number | null }).clickX).not.toBeNull();
  });

  test("Auto-Quellenfindung setzt den Quellpunkt per Klick automatisch, statt ihn direkt zu übernehmen", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await setMockFixtures(page, { repairSourceSuggestion: { x: 0.33, y: 0.44 } });

    await page.getByRole("button", { name: "Reparatur-Pinsel" }).click();
    await page.getByRole("checkbox", { name: "Quelle automatisch vorschlagen" }).check();

    await page.getByRole("main").click();
    await expect(page.getByText(/Ziel im Bild malen/)).toBeVisible();

    const mainBox = await page.getByRole("main").boundingBox();
    if (!mainBox) throw new Error("Viewer-Container nicht gefunden");
    const centerX = mainBox.x + mainBox.width / 2;
    const centerY = mainBox.y + mainBox.height / 2;
    await page.mouse.move(centerX + 15, centerY + 15);
    await page.mouse.down();
    await page.mouse.move(centerX + 35, centerY + 25, { steps: 4 });
    await page.mouse.up();

    const payload = await lastCommit(page);
    const repair = payload.repair as Array<{ source: { x: number; y: number } }>;
    expect(repair).toHaveLength(1);
    expect(repair[0].source.x).toBeCloseTo(0.33, 2);
    expect(repair[0].source.y).toBeCloseTo(0.44, 2);

    const log = await getMockInvokeLog(page);
    expect(log.some((e) => e.cmd === "suggest_repair_source")).toBe(true);
  });

  test("Sensorflecken suchen zeigt Fundstellen, 'Reparieren' committet einen ContentAwareFill-Strich", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await setMockFixtures(page, { sensorSpots: [{ x: 0.6, y: 0.25, radius: 0.02, strength: 0.9 }] });

    await page.getByRole("button", { name: "Sensorflecken suchen" }).click();
    await expect(page.getByText("Fleck 1 (90 %)")).toBeVisible();

    await page.getByRole("button", { name: "Reparieren" }).click();

    const payload = await lastCommit(page);
    const repair = payload.repair as Array<{ mode: string; target_path: Array<{ x: number; y: number }> }>;
    expect(repair).toHaveLength(1);
    expect(repair[0].mode).toBe("ContentAwareFill");
    expect(repair[0].target_path[0].x).toBeCloseTo(0.6, 2);
  });

  test("Auto-Tagging: Vorschläge übernehmen legt echte Schlagworte an", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await setMockFixtures(page, { tagSuggestions: ["Wenig Licht", "Freistellung"] });

    await page.getByRole("button", { name: "Info" }).click();
    await page.getByRole("button", { name: "Tag-Vorschläge" }).click();

    const suggestion = page.getByRole("button", { name: "+ Wenig Licht" });
    await expect(suggestion).toBeVisible();
    await suggestion.click();

    // Der übernommene Vorschlag erscheint als echtes Schlagwort-Chip
    // (identifiziert über dessen Entfernen-Knopf, da der Chip-Text selbst
    // mit dem "×"-Knopf im selben Element steht).
    await expect(page.getByRole("button", { name: 'Schlagwort "Wenig Licht" entfernen' })).toBeVisible();
    await expect(suggestion).not.toBeVisible();
    const log = await getMockInvokeLog(page);
    expect(log.some((e) => e.cmd === "add_photo_keyword" && (e.args as { name: string }).name === "Wenig Licht")).toBe(true);
  });

  test("Preset-Generator (LLM-Modus): Beschreibung erzeugt einen Vorschlag, der sich auf das Foto anwenden lässt", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await setMockFixtures(page, { presetGeneratorSubsetJson: JSON.stringify({ basic: { exposure_ev: 0.75, contrast: 20 } }) });

    await expect(page.getByText("KI-Preset-Generator")).toBeVisible();
    await page.getByText("Anthropic-API-Schlüssel (fehlt)").click();
    await page.getByLabel("Anthropic-API-Schlüssel").fill("sk-ant-test-key");
    await page.getByRole("button", { name: "Speichern", exact: true }).click();
    await expect(page.getByText(/Anthropic-API-Schlüssel \(hinterlegt\)/)).toBeVisible();

    await page.getByLabel("Beschreibung (LLM-Modus)").fill("warmer Filmlook");
    await page.getByRole("button", { name: "Aus Beschreibung erzeugen" }).click();

    await expect(page.getByText("Vorschlag", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Auf aktuelles Foto anwenden" }).click();

    const payload = await lastCommit(page);
    expect(payload.basic.exposure_ev).toBeCloseTo(0.75, 2);
    expect(payload.basic.contrast).toBeCloseTo(20, 2);
  });
});
