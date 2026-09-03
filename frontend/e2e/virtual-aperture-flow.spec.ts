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

async function lastCommit(page: import("@playwright/test").Page) {
  const log = await getMockInvokeLog(page);
  const entry = [...log].reverse().find((e) => e.cmd === "apply_develop_edit");
  if (!entry) throw new Error("kein apply_develop_edit im Protokoll");
  return JSON.parse((entry.args as { edlJson: string }).edlJson).payload;
}

/**
 * Deckt Phase 14 Schritt 8 ab (`DECISIONS.md` ADR-0041 Nachtrag VIII):
 * KI-Tiefenschärfe-Simulator "Virtuelle Blende" — die echte MiDaS-
 * Inferenz (`apx_ai::depth`) und der Blur-Level-Blend
 * (`stages::virtual_aperture`) sind bereits in ihren jeweiligen
 * Rust-Unit-Tests abgedeckt; hier bewusst nur die Frontend-Verdrahtung:
 * Fokuspunkt per Bildklick setzen, Tiefenkarte berechnen, "Betrag"-Regler
 * committet.
 */
test("Virtuelle Blende: Fokuspunkt per Klick setzen, Tiefenkarte berechnen und den Betrag-Regler committen", async ({ page }) => {
  await installTauriMock(page, {
    folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
    photosByFolder: { [FOLDER_ID]: [PHOTO] },
    // Modell bereits heruntergeladen (Testdefault wäre `null`, siehe
    // `tauri-mock.ts`) — der Download-Knopf selbst ist derselbe
    // "Opt-in-Download"-Mechanismus wie beim LaMa-Modell und nicht
    // Gegenstand dieses Tests.
    depthModelPath: "/mock/models/midas_v21_small.onnx",
    depthMapResult: { bitmap_width: 4, bitmap_height: 4, depth_base64: "gICAgICAgICAgICAgICAgA==" },
  });
  await page.goto("/");
  await page.getByRole("button", { name: /Urlaub/ }).click();
  await page.getByRole("img", { name: PHOTO.filename }).click();
  await page.getByRole("button", { name: "Entwickeln" }).click();

  const focusButton = page.getByRole("button", { name: "Fokuspunkt setzen" });
  await focusButton.click();
  await expect(focusButton).toHaveAttribute("aria-pressed", "true");

  await page.getByRole("main").click();

  await expect(focusButton).toHaveAttribute("aria-pressed", "false");
  const afterFocusClick = await lastCommit(page);
  expect(afterFocusClick.virtual_aperture.focus_x).toBeGreaterThanOrEqual(0);
  expect(afterFocusClick.virtual_aperture.focus_y).toBeGreaterThanOrEqual(0);

  await page.getByRole("button", { name: "Tiefenkarte berechnen" }).click();

  await expect
    .poll(async () => {
      const payload = await lastCommit(page);
      return payload.virtual_aperture.depth_map !== null;
    })
    .toBe(true);
  const afterDepth = await lastCommit(page);
  expect(afterDepth.virtual_aperture.depth_map.bitmap_width).toBe(4);
  expect(afterDepth.virtual_aperture.depth_map.depth).toBe("gICAgICAgICAgICAgICAgA==");

  const amountInput = page.getByRole("spinbutton", { name: "Virtuelle Blende: Betrag (Zahlenwert)" });
  await amountInput.fill("60");
  await amountInput.blur();

  await expect
    .poll(async () => {
      const payload = await lastCommit(page);
      return payload.virtual_aperture.amount;
    })
    .toBeCloseTo(0.6, 5);
});
