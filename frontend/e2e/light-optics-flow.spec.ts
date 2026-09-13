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
 * Deckt Phase 28 ab (siehe `PLAN.md`, `DECISIONS.md` ADR-0058): die
 * zwoelf Licht-&-Optik-Werkzeuge und der gemeinsame Panel-Kopf. Die
 * Bildmathematik ist in `stages::light_optics`s Rust-Unit-Tests
 * abgedeckt — hier laeuft die Kette davor: erscheinen alle zwoelf,
 * landen Reglerwerte im committeten EDL, filtern Suche und
 * "Nur aktive" richtig, und kommen die vorbereiteten Karten im
 * Format an, das Rust wirklich liest.
 */
test.describe("Licht & Optik (Phase 28)", () => {
  async function openPanel(page: import("@playwright/test").Page) {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: "/home/user/Fotos/Urlaub", photo_count: 1, parent_id: null, missing: false }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln" }).click();
    await page.getByRole("tab", { name: "Licht & Optik" }).click();
    return page.getByTestId("light-optics-panel");
  }

  async function lastCommit(page: import("@playwright/test").Page) {
    const log = await getMockInvokeLog(page);
    const calls = log.filter((entry) => entry.cmd === "apply_develop_edit");
    expect(calls.length).toBeGreaterThan(0);
    const args = calls[calls.length - 1].args as { edlJson: string };
    return (JSON.parse(args.edlJson) as { payload: Record<string, never> }).payload as never as {
      light_optics: Record<string, Record<string, unknown>>;
    };
  }

  test("zeigt alle zwoelf Werkzeuge und committet einen Regler", async ({ page }) => {
    const panel = await openPanel(page);
    await expect(panel).toBeVisible();

    // Alle zwoelf muessen da sein — nicht elf, nicht zehn.
    for (const title of [
      "Tonwert-Angleich",
      "Zonensystem",
      "Detail-Pyramide",
      "Dunst entfernen",
      "Tiefenschärfe",
      "Neu beleuchten",
      "Himmel dramatisieren",
      "Bewegungsunschärfe",
      "Blendenstern",
      "Diffusionsfilter",
      "Kanalmatrix",
      "Poster-Look",
    ]) {
      await expect(panel.getByRole("region", { name: title })).toBeVisible();
    }

    const diffusion = panel.getByRole("region", { name: "Diffusionsfilter" });
    const amount = diffusion.getByRole("spinbutton", { name: /Stärke \(Zahlenwert\)/ });
    await amount.fill("0.8");
    await amount.blur();

    await expect
      .poll(async () => (await lastCommit(page)).light_optics.diffusion.amount)
      .toBeCloseTo(0.8, 5);
  });

  test("Suche und \"Nur aktive\" filtern die Kacheln", async ({ page }) => {
    const panel = await openPanel(page);

    // Die Suche greift auch auf den Wirkungs-Halbsatz zu: "Himmel"
    // steht im Titel EINES Werkzeugs.
    await panel.getByTestId("light-optics-search").fill("himmel");
    await expect(panel.getByRole("region", { name: "Himmel dramatisieren" })).toBeVisible();
    await expect(panel.getByRole("region", { name: "Blendenstern" })).toHaveCount(0);

    await panel.getByTestId("light-optics-search").fill("");
    await expect(panel.getByRole("region", { name: "Blendenstern" })).toBeVisible();

    // Ohne aktives Werkzeug blendet "Nur aktive" alles aus.
    await panel.getByTestId("light-optics-only-active").click();
    await expect(panel.getByRole("region", { name: "Blendenstern" })).toHaveCount(0);
    await expect(panel.getByText("Kein Werkzeug passt zur Suche.")).toBeVisible();

    // Ein Werkzeug aktivieren -> es taucht wieder auf, die anderen nicht.
    await panel.getByTestId("light-optics-only-active").click();
    const star = panel.getByRole("region", { name: "Blendenstern" });
    const amount = star.getByRole("spinbutton", { name: /Stärke \(Zahlenwert\)/ });
    await amount.fill("0.5");
    await amount.blur();
    await panel.getByTestId("light-optics-only-active").click();
    await expect(panel.getByRole("region", { name: "Blendenstern" })).toBeVisible();
    await expect(panel.getByRole("region", { name: "Poster-Look" })).toHaveCount(0);
  });

  test("Himmelsmaske kommt als Zahlen-Array an, Tonwerte als neun Dezile", async ({ page }) => {
    const panel = await openPanel(page);

    await panel.getByRole("region", { name: "Himmel dramatisieren" }).getByRole("button", { name: "Himmel erkennen" }).click();
    await expect
      .poll(async () => (await lastCommit(page)).light_optics.sky_drama.mask !== null)
      .toBe(true);

    const afterSky = await lastCommit(page);
    const mask = afterSky.light_optics.sky_drama.mask as { alpha: number[]; bitmap_width: number };
    // Dasselbe Wire-Format wie bei der Motivmaske: ein ZAHLEN-Array,
    // kein base64-String — Rust liest das Feld als schlichtes `Vec<u8>`
    // (siehe ADR-0057 fuer den Fehler, der genau daran haengen blieb).
    expect(Array.isArray(mask.alpha)).toBe(true);
    expect(mask.alpha).toHaveLength(16);
    expect(mask.bitmap_width).toBe(4);

    await panel.getByRole("region", { name: "Tonwert-Angleich" }).getByRole("button", { name: "Tonwerte übernehmen" }).click();
    await expect
      .poll(async () => (await lastCommit(page)).light_optics.tone_match.has_target)
      .toBe(true);
    const afterTone = await lastCommit(page);
    expect(afterTone.light_optics.tone_match.targets).toHaveLength(9);
  });

  test("Bewegungsart und Kanalmatrix-Vorgabe landen im EDL", async ({ page }) => {
    const panel = await openPanel(page);

    await panel.getByRole("region", { name: "Bewegungsunschärfe" }).getByRole("button", { name: "Zoom" }).click();
    await expect
      .poll(async () => (await lastCommit(page)).light_optics.motion_blur.kind)
      .toBe("Zoom");

    await panel.getByRole("region", { name: "Kanalmatrix" }).getByRole("button", { name: "Infrarot" }).click();
    const after = await lastCommit(page);
    const matrix = after.light_optics.channel_matrix.matrix as number[];
    expect(matrix).toHaveLength(9);
    expect(matrix).not.toEqual([1, 0, 0, 0, 1, 0, 0, 0, 1]);
    expect(after.light_optics.channel_matrix.amount).toBeGreaterThan(0);
  });
});
