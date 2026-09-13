import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000501";
const PHOTO_A = { id: "01977f4a-0000-7000-8000-000000000502", filename: "IMG_A.CR3", width: 6000, height: 4000, missing: false };
const PHOTO_B = { id: "01977f4a-0000-7000-8000-000000000503", filename: "IMG_B.CR3", width: 6000, height: 4000, missing: false };

/**
 * Deckt Phase 29 ab (siehe `DECISIONS.md` ADR-0059): die Kreativ- und
 * Licht-&-Optik-Werkzeuge sind jetzt preset-faehig, WEIL ihre
 * fotospezifischen Karten beim Speichern herausgeschnitten werden.
 *
 * Die Einheitentests in `presets.test.ts` pruefen das Schneiden und
 * Zusammenfuehren direkt; dieser Test geht den ganzen Weg durch die
 * Oberflaeche — Maske berechnen, Preset speichern, auf ein ZWEITES Foto
 * anwenden — und stellt sicher, dass die Regler ankommen und die Maske
 * des ersten Fotos eben NICHT mitwandert.
 */
test.describe("Werkzeug-Presets (Phase 29)", () => {
  async function lastCommit(page: import("@playwright/test").Page) {
    const log = await getMockInvokeLog(page);
    const calls = log.filter((entry) => entry.cmd === "apply_develop_edit");
    expect(calls.length).toBeGreaterThan(0);
    const args = calls[calls.length - 1].args as { edlJson: string; photoId?: string };
    return {
      photoId: args.photoId,
      payload: JSON.parse(args.edlJson).payload as {
        light_optics: {
          sky_drama: { amount: number; mask: unknown };
          star_filter: { amount: number };
        };
      },
    };
  }

  test("traegt die Regler auf ein zweites Foto, die Himmelsmaske aber nicht", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: "/home/user/Fotos/Serie", photo_count: 2, parent_id: null, missing: false }],
      photosByFolder: { [FOLDER_ID]: [PHOTO_A, PHOTO_B] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Serie/ }).click();

    // --- Foto A: Himmel erkennen + einen zweiten Regler verstellen ------
    await page.getByRole("img", { name: PHOTO_A.filename }).click();
    await page.getByRole("button", { name: "Entwickeln", exact: true }).click();
    await page.getByRole("tab", { name: "Licht & Optik" }).click();
    const panel = page.getByTestId("light-optics-panel");

    await panel.getByRole("region", { name: "Himmel dramatisieren" }).getByRole("button", { name: "Himmel erkennen" }).click();
    await expect.poll(async () => (await lastCommit(page)).payload.light_optics.sky_drama.mask !== null).toBe(true);

    const star = panel.getByRole("region", { name: "Blendenstern" });
    const starAmount = star.getByRole("spinbutton", { name: /Stärke \(Zahlenwert\)/ });
    await starAmount.fill("0.6");
    await starAmount.blur();
    await expect.poll(async () => (await lastCommit(page)).payload.light_optics.star_filter.amount).toBeCloseTo(0.6, 5);

    // --- Preset speichern, nur die Licht-&-Optik-Sektion ----------------
    await page.getByRole("button", { name: "Preset speichern" }).click();
    const dialog = page.getByRole("dialog", { name: "Preset speichern" });
    await dialog.getByLabel("Name").fill("Serienlook");
    for (const label of [
      "Grundeinstellungen",
      "Kurven",
      "HSL",
      "Farbmischer",
      "Color Grading",
      "Details",
      "Objektivkorrekturen",
      "Effekte",
      "Kalibrierung",
      "Geometrie",
      "Compositing-Ebenen",
      "Filter",
      "Kreativ-Werkzeuge",
    ]) {
      await dialog.getByLabel(label, { exact: true }).uncheck();
    }
    await dialog.getByRole("button", { name: "Speichern" }).click();
    await expect(dialog).not.toBeVisible();
    await expect(page.getByText("Serienlook")).toBeVisible();

    // --- Foto B: Preset anwenden ----------------------------------------
    // Bewusst ueber den Filmstreifen statt ueber Raster + Entwickeln:
    // der Wechsel bleibt im Entwickeln-Modus, damit die Preset-Palette
    // sichtbar bleibt und der Test genau eine Sache prueft.
    await page.getByRole("img", { name: PHOTO_B.filename }).last().click();
    await expect.poll(async () => (await lastCommit(page)).photoId).not.toBe(undefined);
    await page.getByRole("button", { name: "Serienlook", exact: true }).click();

    await expect.poll(async () => (await lastCommit(page)).payload.light_optics.star_filter.amount).toBeCloseTo(0.6, 5);
    const applied = await lastCommit(page);
    // Die Regler sind angekommen …
    expect(applied.payload.light_optics.sky_drama.amount).toBeCloseTo(0.7, 5);
    // … die Maske des ERSTEN Fotos ausdruecklich nicht. Genau das ist
    // der Grund, warum diese Sektion ueberhaupt preset-faehig sein darf.
    expect(applied.payload.light_optics.sky_drama.mask).toBeNull();

    // Und die Oberflaeche sagt es auch, statt still nichts zu tun.
    await page.getByRole("tab", { name: "Licht & Optik" }).click();
    await expect(
      page.getByTestId("light-optics-panel").getByText("Ohne Himmelsmaske wirkungslos"),
    ).toBeVisible();
  });
});
