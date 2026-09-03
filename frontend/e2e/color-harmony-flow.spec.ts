import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

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

/**
 * Deckt Phase 14 Schritt 7 ab (`DECISIONS.md` ADR-0041 Nachtrag VII):
 * Farb-Harmonie-Rad im Entwickeln-Panel — die echte k-means-
 * Paletten-Extraktion (`apx_ai::palette`) ist bereits in
 * `palette.rs`s Rust-Unit-Tests abgedeckt, die reine Harmonie-Mathematik
 * bereits in `lib/colorHarmony.test.ts`; hier bewusst nur die Frontend-
 * Verdrahtung: Palette extrahieren zeigt die Farben an, "Harmonisieren"
 * committet die berechneten HSL-Band-Farbton-Deltas.
 */
test("Farb-Harmonie-Rad: extrahiert die Palette und committet die Harmonisieren-Deltas für die HSL-Bänder", async ({ page }) => {
  await installTauriMock(page, {
    folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
    photosByFolder: { [FOLDER_ID]: [PHOTO] },
    colorPaletteResult: [
      { r: 220, g: 40, b: 40, hue_degrees: 0, chroma: 40, lightness: 50, percentage: 0.7 },
      { r: 40, g: 200, b: 40, hue_degrees: 120, chroma: 40, lightness: 55, percentage: 0.3 },
    ],
  });
  await page.goto("/");
  await page.getByRole("button", { name: /Urlaub/ }).click();
  await page.getByRole("img", { name: PHOTO.filename }).click();
  await page.getByRole("button", { name: "Entwickeln" }).click();

  await page.getByRole("button", { name: "Palette extrahieren" }).click();

  await expect(page.getByText("70%")).toBeVisible();
  await expect(page.getByText("30%")).toBeVisible();

  // Komplementär ist der Vorgabe-Harmonietyp, explizit anklicken macht
  // den Test robust gegen eine spätere Änderung der Vorgabe.
  await page.getByRole("button", { name: "Komplementär", exact: true }).click();
  await page.getByRole("button", { name: "Harmonisieren" }).click();

  await expect
    .poll(async () => {
      const log = await getMockInvokeLog(page);
      return log.filter((entry) => entry.cmd === "apply_develop_edit").length >= 1;
    })
    .toBe(true);

  const log = await getMockInvokeLog(page);
  const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
  const hsl = JSON.parse((lastCommit?.args as { edlJson: string }).edlJson).payload.hsl as {
    red: { hue: number };
    green: { hue: number };
  };
  // Die dominante rote Farbe (70%, Farbton 0°) ist selbst schon eines
  // der beiden Komplementär-Ziele (0°/180°) — praktisch kein Delta.
  expect(hsl.red.hue).toBeCloseTo(0, 1);
  // Die grüne Farbe (Farbton 120°) liegt näher am 180°-Ziel als am
  // 0°-Ziel und muss deshalb deutlich positiv verschoben werden (auf
  // die Regler-Obergrenze gekappt, da 60° Abstand die 60°-Obergrenze
  // exakt erreicht).
  expect(hsl.green.hue).toBeCloseTo(100, 1);
});
