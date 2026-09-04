import { expect, test } from "@playwright/test";

import { base64ToByteArray } from "../src/lib/edl";
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
 * Deckt Phase 14 Schritt 9 ab (`DECISIONS.md` ADR-0041 Nachtrag IX):
 * KI-Stiltransfer zwischen Fotos — die echte `fast_neural_style`-
 * Inferenz (`apx_ai::style_transfer`) und der Überblend-Kurzschluss
 * (`stages::style_transfer`) sind bereits in ihren jeweiligen
 * Rust-Unit-Tests abgedeckt; hier bewusst nur die Frontend-Verdrahtung:
 * Stil herunterladen, stilisieren, "Betrag"-Regler committet.
 */
test("Stiltransfer: Stil herunterladen, ein Foto stilisieren und den Betrag-Regler committen", async ({ page }) => {
  await installTauriMock(page, {
    folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
    photosByFolder: { [FOLDER_ID]: [PHOTO] },
    // Modell bereits heruntergeladen (Testdefault wäre ein leeres
    // Objekt, siehe `tauri-mock.ts`) — der Download-Knopf selbst ist
    // derselbe "Opt-in-Download"-Mechanismus wie bei MiDaS/LaMa und
    // nicht Gegenstand dieses Tests (dieselbe Vereinfachung wie
    // `virtual-aperture-flow.spec.ts`).
    styleTransferModelPaths: { mosaic: "/mock/models/style_transfer_mosaic.onnx" },
    styleTransferPatchResult: { bitmap_width: 2, bitmap_height: 2, pixels_base64: "3t7e3t7e3t7e3t7e" },
  });
  await page.goto("/");
  await page.getByRole("button", { name: /Urlaub/ }).click();
  await page.getByRole("img", { name: PHOTO.filename }).click();
  await page.getByRole("button", { name: "Entwickeln" }).click();

  // Auf die Zeile mit "Mosaik" eingeschränkt, weil "Stilisieren" bei
  // mehreren Stilen vorkommen könnte.
  const styleTransferSection = page.locator("#stage-style_transfer");
  const mosaicRow = styleTransferSection.locator("li", { hasText: "Mosaik" });
  const stylizeButton = mosaicRow.getByRole("button", { name: "Stilisieren" });
  await expect(stylizeButton).toBeVisible();
  await stylizeButton.click();

  await expect
    .poll(async () => {
      const payload = await lastCommit(page);
      return payload.style_transfer.patch !== null;
    })
    .toBe(true);
  const afterStylize = await lastCommit(page);
  expect(afterStylize.style_transfer.patch.bitmap_width).toBe(2);
  expect(afterStylize.style_transfer.patch.pixels).toEqual(base64ToByteArray("3t7e3t7e3t7e3t7e"));

  const amountInput = page.getByRole("spinbutton", { name: "Stiltransfer: Betrag (Zahlenwert)" });
  await amountInput.fill("40");
  await amountInput.blur();

  await expect
    .poll(async () => {
      const payload = await lastCommit(page);
      return payload.style_transfer.amount;
    })
    .toBeCloseTo(0.4, 5);
});
