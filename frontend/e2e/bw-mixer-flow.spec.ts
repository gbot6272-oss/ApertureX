import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false };

/**
 * Deckt Phase 9 Schritt 5 ab (`PLAN.md`, `DECISIONS.md` ADR-0035): den
 * Schwarzweiß-Mixer im Entwickeln-Panel. Die Rendering-Logik selbst
 * (`stages::bw_mixer::apply_rgba8`) ist bereits vollständig in
 * `apx-pipeline`s Rust-Unit-Tests abgedeckt — hier bewusst nur ein
 * Frontend-Flow: Behandlung umschalten committet sofort, ein Band-Regler
 * committet sein Gewicht.
 */
test.describe("Schwarzweiß-Mixer (Phase 9 Schritt 5)", () => {
  test("Behandlung auf Schwarzweiß umschalten und ein Band-Regler committen beide", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO] } });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln" }).click();

    const treatmentGroup = page.getByRole("group", { name: "Behandlung" });
    await treatmentGroup.getByRole("button", { name: "Schwarzweiß" }).click();

    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      const last = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
      const edlJson = (last?.args as { edlJson?: string } | undefined)?.edlJson;
      return edlJson ? (JSON.parse(edlJson).payload.treatment as string) : null;
    }).toBe("BlackAndWhite");

    const bwGroup = page.getByRole("group", { name: "Behandlung" });
    await bwGroup.getByRole("button", { name: "Aqua" }).click();
    const weightInput = bwGroup.getByRole("spinbutton", { name: "Aqua (Zahlenwert)" });
    await weightInput.fill("40");
    await weightInput.blur();

    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      const last = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
      const edlJson = (last?.args as { edlJson?: string } | undefined)?.edlJson;
      return edlJson ? (JSON.parse(edlJson).payload.bw_mixer.aqua as number) : null;
    }).toBe(40);
  });
});
