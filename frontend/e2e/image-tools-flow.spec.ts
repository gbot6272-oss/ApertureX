import { expect, test, type Page } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000601";
const PHOTO = {
  id: "01977f4a-0000-7000-8000-000000000602",
  filename: "IMG_0001.CR3",
  width: 6000,
  height: 4000,
  missing: false,
};

/**
 * Deckt Phase 30 ab (siehe `DECISIONS.md` ADR-0060): die sieben am Bild
 * bedienten Werkzeuge und die drei neuen Bedienelemente.
 *
 * Die Bildmathematik ist in `stages::interactive`s zehn Rust-Tests
 * abgedeckt, die Koordinatenumrechnung in `imageToolMath.test.ts` —
 * hier laeuft die Kette dazwischen: erscheint das Overlay im richtigen
 * Modus, landen die im Bild gesetzten Punkte im committeten EDL, und
 * tun die drei neuen Bedienelemente, was sie versprechen.
 */
test.describe("Am Bild (Phase 30)", () => {
  async function openPanel(page: Page) {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: "/home/user/Fotos/Serie", photo_count: 1, parent_id: null, missing: false }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Serie/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln", exact: true }).click();
    await page.getByRole("tab", { name: "Am Bild" }).click();
    return page.getByTestId("image-tools-panel");
  }

  async function lastCommit(page: Page) {
    const log = await getMockInvokeLog(page);
    const calls = log.filter((entry) => entry.cmd === "apply_develop_edit");
    expect(calls.length).toBeGreaterThan(0);
    const args = calls[calls.length - 1].args as { edlJson: string };
    return JSON.parse(args.edlJson).payload as {
      interactive: Record<string, Record<string, unknown>>;
      light_optics: Record<string, Record<string, unknown>>;
      virtual_aperture: Record<string, unknown>;
    };
  }

  test("zeigt alle zehn Kacheln der Registerkarte", async ({ page }) => {
    const panel = await openPanel(page);
    await expect(panel).toBeVisible();
    for (const title of [
      "Lichtquellen",
      "Lichtkegel",
      "Abwedeln & Nachbelichten",
      "Split-Lighting",
      "Farbe ersetzen",
      "Verlaufsband",
      "Horizont-Verlaufsfilter",
      "Kanalmatrix",
      "Blendenform",
      "Zonen anzeigen",
    ]) {
      await expect(panel.getByRole("region", { name: title })).toBeVisible();
    }
  });

  test("setzt ein Licht per Klick ins Bild und zieht es an eine andere Stelle", async ({ page }) => {
    const panel = await openPanel(page);

    // Ohne aktiven Modus darf das Overlay gar nicht da sein — sonst
    // wuerde es Zoom und Verschieben blockieren.
    await expect(page.getByTestId("image-tool-overlay")).toHaveCount(0);

    await panel.getByRole("region", { name: "Lichtquellen" }).getByRole("button", { name: "Lichter setzen" }).click();
    const overlay = page.getByTestId("image-tool-overlay");
    await expect(overlay).toBeVisible();

    // Klick ins Bild legt ein Licht an DIESER Stelle an.
    const box = (await overlay.boundingBox())!;
    await page.mouse.click(box.x + box.width * 0.25, box.y + box.height * 0.75);

    await expect
      .poll(async () => ((await lastCommit(page)).interactive.point_lights.lights as unknown[]).length)
      .toBe(1);
    const afterClick = await lastCommit(page);
    const light = (afterClick.interactive.point_lights.lights as { x: number; y: number }[])[0];
    expect(light.x).toBeCloseTo(0.25, 1);
    expect(light.y).toBeCloseTo(0.75, 1);
    // Der Klick dreht die Gesamtstaerke mit auf — sonst waere er folgenlos.
    expect(afterClick.interactive.point_lights.amount).toBeGreaterThan(0);

    // Griff ziehen: das Licht muss der Maus folgen.
    const handle = overlay.locator('[data-handle="light:0"]');
    await expect(handle).toBeVisible();
    await handle.hover();
    await page.mouse.down();
    await page.mouse.move(box.x + box.width * 0.7, box.y + box.height * 0.3, { steps: 8 });
    await page.mouse.up();

    await expect
      .poll(async () => (((await lastCommit(page)).interactive.point_lights.lights as { x: number }[])[0]).x)
      .toBeCloseTo(0.7, 1);
  });

  test("greift eine Farbe aus dem Bild und legt sie als Quellfarbe ab", async ({ page }) => {
    const panel = await openPanel(page);
    const tile = panel.getByRole("region", { name: "Farbe ersetzen" });
    await expect(tile.getByText("noch keine Farbe gegriffen")).toBeVisible();

    await tile.getByRole("button", { name: "Farbe greifen" }).click();
    await page.getByRole("main").click();

    await expect.poll(async () => (await lastCommit(page)).interactive.color_replace.has_source).toBe(true);
    // Nach dem Griff ist der Modus wieder aus — ein Pipettenmodus, der
    // anbleibt, greift beim naechsten Klick versehentlich neu.
    await expect(tile.getByRole("button", { name: "Farbe greifen" })).toBeVisible();
  });

  test("Verlaufsband legt Stützstellen an und färbt sie", async ({ page }) => {
    const panel = await openPanel(page);
    const tile = panel.getByRole("region", { name: "Verlaufsband" });
    await expect(page.getByTestId("gradient-ramp-editor")).toBeVisible();

    // Erster Griff: der Startverlauf mit drei Stuetzstellen.
    await tile.getByRole("button", { name: "+ Stützstelle" }).click();
    await expect
      .poll(async () => ((await lastCommit(page)).interactive.gradient_ramp.stops as unknown[]).length)
      .toBe(3);

    // Zweiter Griff legt eine vierte an.
    await tile.getByRole("button", { name: "+ Stützstelle" }).click();
    await expect
      .poll(async () => ((await lastCommit(page)).interactive.gradient_ramp.stops as unknown[]).length)
      .toBe(4);

    const after = await lastCommit(page);
    expect(after.interactive.gradient_ramp.amount).toBeGreaterThan(0);
  });

  test("Kanalmatrix-Gitter schreibt einzelne Zellen, Blendenform zeichnet", async ({ page }) => {
    const panel = await openPanel(page);

    const cell = panel.getByRole("spinbutton", { name: "Rot aus Grün" });
    await cell.fill("0.8");
    await cell.blur();
    await expect
      .poll(async () => ((await lastCommit(page)).light_optics.channel_matrix.matrix as number[])[1])
      .toBeCloseTo(0.8, 5);

    // Die Blendenform-Vorschau ist ein echtes Canvas, kein Platzhalter.
    await expect(panel.getByTestId("aperture-shape-preview")).toBeVisible();
    const blades = panel.getByRole("spinbutton", { name: "Lamellen (Zahlenwert)" });
    await blades.fill("6");
    await blades.blur();
    await expect.poll(async () => (await lastCommit(page)).virtual_aperture.blades).toBe(6);
  });

  test("Zonenstreifen schaltet die Überlagerung ein und bedient den Zonenregler", async ({ page }) => {
    const panel = await openPanel(page);
    const tile = panel.getByRole("region", { name: "Zonen anzeigen" });

    await expect(page.getByTestId("zone-overlay-canvas")).toHaveCount(0);
    await tile.getByRole("button", { name: "Zone 7" }).click();
    // Eine Zone zu waehlen schaltet die Ueberlagerung gleich mit ein —
    // sonst waere der Klick folgenlos.
    await expect(page.getByTestId("zone-overlay-canvas")).toBeVisible();

    const zoneSlider = tile.getByRole("spinbutton", { name: "Zone 7 (Zahlenwert)" });
    await zoneSlider.fill("0.6");
    await zoneSlider.blur();
    await expect
      .poll(async () => ((await lastCommit(page)).light_optics.zone_system.zones as number[])[7])
      .toBeCloseTo(0.6, 5);
  });
});
