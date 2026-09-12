import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const PHOTO = {
  id: "01977f4a-0000-7000-8000-000000000101",
  filename: "IMG_0001.CR3",
  width: 6000,
  height: 4000,
  missing: false,
};

/**
 * Deckt Phase 27 ab (siehe `PLAN.md`, `DECISIONS.md` ADR-0057): die zehn
 * Kreativ-Werkzeuge im Entwickeln-Panel. Die Bildmathematik selbst ist
 * in `stages::creative`s 15 Rust-Unit-Tests abgedeckt — dieser Test
 * prueft die Kette davor: erscheinen alle zehn, landen Reglerwerte
 * wirklich im committeten EDL, und tun die beiden Ein-Klick-Knoepfe
 * (Motiv freistellen, Referenzfoto) das, was sie versprechen.
 */
test.describe("Kreativ-Werkzeuge (Phase 27)", () => {
  test("zeigt alle zehn Werkzeuge, committet Regler und die beiden Ein-Klick-Vorbereitungen", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: "/home/user/Fotos/Urlaub", photo_count: 1, parent_id: null, missing: false }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln" }).click();
    await page.getByRole("tab", { name: "Kreativ" }).click();

    const panel = page.getByTestId("creative-panel");
    await expect(panel).toBeVisible();

    // Alle zehn Werkzeuge muessen da sein — nicht neun, nicht acht.
    for (const title of [
      "Farbabgleich",
      "Tiefennebel",
      "Motiv freistellen",
      "Tilt-Shift",
      "Sonnenstrahlen",
      "Orton-Glanz",
      "Filmlabor",
      "Verlaufsabbildung",
      "Farbisolierung",
      "Lichtleck",
    ]) {
      await expect(panel.getByRole("region", { name: title })).toBeVisible();
    }

    // --- Ein Regler committet wirklich in die EDL ---
    const orton = panel.getByRole("region", { name: "Orton-Glanz" });
    const strength = orton.getByRole("spinbutton", { name: "Stärke" });
    await strength.fill("0.8");
    await strength.blur();

    await expect
      .poll(async () => {
        const log = await getMockInvokeLog(page);
        return log.filter((e) => e.cmd === "apply_develop_edit").length;
      })
      .toBeGreaterThan(0);

    const readCreative = async () => {
      const log = await getMockInvokeLog(page);
      const last = [...log].reverse().find((e) => e.cmd === "apply_develop_edit");
      const edlJson = (last?.args as { edlJson: string }).edlJson;
      return JSON.parse(edlJson).payload.creative;
    };
    expect((await readCreative()).orton.amount).toBeCloseTo(0.8, 2);

    // --- Motiv freistellen: Maske landet im EDL ---
    await panel.getByRole("button", { name: "Motiv freistellen" }).click();
    await expect
      .poll(async () => (await readCreative()).subject_focus.mask?.alpha?.length ?? 0)
      .toBe(16);
    // Die Maske muss als ZAHLEN-Array ankommen, nicht als base64-String:
    // die Rust-Seite liest ein `Vec<u8>` (Phase 27 real nachgemessen,
    // siehe ADR-0057 — genau daran krankte die Tiefenkarte bisher).
    expect(typeof (await readCreative()).subject_focus.mask.alpha[0]).toBe("number");

    // --- Farbabgleich: Referenzstatistik landet im EDL ---
    await panel.getByRole("button", { name: "Referenzfoto übernehmen" }).click();
    await expect.poll(async () => (await readCreative()).color_match.has_target).toBe(true);
    expect((await readCreative()).color_match.target_l_mean).toBeCloseTo(0.52, 2);

    // --- Filmlabor-Prozess laesst sich umschalten ---
    const lab = panel.getByRole("region", { name: "Filmlabor" });
    await lab.getByRole("button", { name: "Cross-Processing" }).click();
    await expect.poll(async () => (await readCreative()).film_lab.process).toBe("CrossProcess");
  });
});
