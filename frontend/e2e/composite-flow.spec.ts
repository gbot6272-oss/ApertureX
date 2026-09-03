import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000501";
const FOLDER_PATH = "/home/user/Fotos/Doppelbelichtung";

function samplePhoto(id: string, filename: string) {
  return {
    id,
    filename,
    file_size: 12_000_000,
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
}

const PHOTO = samplePhoto("01977f4a-0000-7000-8000-000000000601", "IMG_0601.CR3");
const PHOTO_TEXTURE_SOURCE = samplePhoto("01977f4a-0000-7000-8000-000000000602", "IMG_0602.CR3");

async function setUp(page: import("@playwright/test").Page) {
  await installTauriMock(page, {
    folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2 }],
    photosByFolder: { [FOLDER_ID]: [PHOTO, PHOTO_TEXTURE_SOURCE] },
  });
  await page.goto("/");
  await page.getByRole("button", { name: /Doppelbelichtung/ }).click();
  await page.getByRole("img", { name: PHOTO.filename }).click();
  await page.getByRole("button", { name: "Entwickeln" }).click();
}

/**
 * Deckt Phase 14 Schritt 3 End-to-End ab (siehe `DECISIONS.md` ADR-0041):
 * eine Compositing-Ebene aus einem zweiten Katalog-Foto hinzufügen, per
 * Blend-Modus/Sichtbarkeit/Deckkraft bedienen, wieder entfernen — Muster
 * wie `masks-flow.spec.ts`.
 */
test.describe("Compositing-Panel (Mehrfachbelichtung)", () => {
  test("fügt eine Ebene aus einem anderen Foto hinzu, blendet sie aus/ein und entfernt sie wieder", async ({ page }) => {
    await setUp(page);

    await expect(page.getByRole("group", { name: "Compositing" })).toBeVisible();
    await expect(page.getByText("Keine Compositing-Ebenen vorhanden.")).toBeVisible();

    await page.getByRole("combobox", { name: "Ebenen-Quellfoto" }).selectOption(PHOTO_TEXTURE_SOURCE.id);
    await page.getByRole("button", { name: "+ Ebene aus Foto" }).click();

    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      return log.filter((entry) => entry.cmd === "prepare_composite_layer_source").length;
    }).toBeGreaterThan(0);

    const sourceCall = (await getMockInvokeLog(page)).find((entry) => entry.cmd === "prepare_composite_layer_source");
    expect((sourceCall?.args as { photoId: string | null }).photoId).toBe(PHOTO_TEXTURE_SOURCE.id);

    await expect(page.getByText("Ebene 1")).toBeVisible();

    // Der Commit trägt die neue Ebene tatsächlich im EDL.
    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
      if (!lastCommit) return 0;
      const edl = JSON.parse((lastCommit.args as { edlJson: string }).edlJson);
      return (edl.payload.composite_layers as unknown[]).length;
    }).toBe(1);

    // Sichtbarkeit umschalten committet sofort (wie ein Masken-Sichtbarkeits-Klick).
    const visibilityToggle = page.getByRole("button", { name: "Ebene 1 ausblenden" });
    await visibilityToggle.click();
    await expect(page.getByRole("button", { name: "Ebene 1 einblenden" })).toBeVisible();
    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
      const edl = JSON.parse((lastCommit?.args as { edlJson: string }).edlJson);
      return (edl.payload.composite_layers as Array<{ visible: boolean }>)[0]?.visible;
    }).toBe(false);

    // Blend-Modus wechseln.
    await page.getByRole("combobox", { name: "Blend-Modus Ebene 1" }).selectOption("Multiply");
    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
      const edl = JSON.parse((lastCommit?.args as { edlJson: string }).edlJson);
      return (edl.payload.composite_layers as Array<{ blend_mode: string }>)[0]?.blend_mode;
    }).toBe("Multiply");

    // Entfernen.
    await page.getByRole("button", { name: "Ebene 1 löschen" }).click();
    await expect(page.getByText("Keine Compositing-Ebenen vorhanden.")).toBeVisible();
    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
      const edl = JSON.parse((lastCommit?.args as { edlJson: string }).edlJson);
      return (edl.payload.composite_layers as unknown[]).length;
    }).toBe(0);
  });
});
