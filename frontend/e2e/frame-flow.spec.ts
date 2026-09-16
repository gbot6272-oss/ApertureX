import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false };

/**
 * Deckt Phase 32 F8 ab: Rahmen und Passepartout. Die Bildseite (Bänder,
 * Breiten, Rückwärtskompatibilität) ist in `apx-pipeline`s
 * `stages::frame`- und `edl::v4`-Tests abgedeckt — hier geht es darum,
 * dass die Regler im gespeicherten EDL landen.
 */
test.describe("Rahmen und Passepartout (Phase 32 F8)", () => {
  async function lastFrame(page: import("@playwright/test").Page) {
    const log = await getMockInvokeLog(page);
    const edits = log.filter((entry) => entry.cmd === "apply_develop_edit");
    const last = edits[edits.length - 1];
    if (!last) return null;
    const payload = JSON.parse((last.args as { edlJson: string }).edlJson) as { payload: { frame: Record<string, unknown> } };
    return payload.payload.frame;
  }

  test.beforeEach(async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).first().click();
    await page.getByRole("button", { name: "Entwickeln" }).click();
    await page.getByRole("tab", { name: "Kreativ" }).click();
  });

  test("eine Vorlage schreibt alle sechs Rahmenfelder ins EDL", async ({ page }) => {
    await page.getByTestId("frame-panel").getByRole("button", { name: "Galerie" }).click();

    await expect.poll(async () => (await lastFrame(page))?.mat_width).toBe(12);
    const frame = await lastFrame(page);
    // Weißes Passepartout, dunkle Linien — die Farben reisen mit.
    expect(frame?.mat_color).toEqual([1, 1, 1]);
    expect(frame?.border_width).toBe(0.6);
  });

  test("„Ohne Rahmen“ setzt alle Breiten wieder auf null", async ({ page }) => {
    await page.getByTestId("frame-panel").getByRole("button", { name: "Museum" }).click();
    await expect.poll(async () => (await lastFrame(page))?.mat_width).toBe(15);

    await page.getByTestId("frame-panel").getByRole("button", { name: "Ohne Rahmen" }).click();
    await expect.poll(async () => (await lastFrame(page))?.mat_width).toBe(0);
    const frame = await lastFrame(page);
    expect(frame?.border_width).toBe(0);
    expect(frame?.inner_line_width).toBe(0);
  });

  test("der Passepartout-Regler landet im EDL", async ({ page }) => {
    const input = page.getByRole("spinbutton", { name: "Passepartout (Zahlenwert)" });
    await input.fill("8");
    await input.blur();

    await expect.poll(async () => (await lastFrame(page))?.mat_width).toBe(8);
  });
});
