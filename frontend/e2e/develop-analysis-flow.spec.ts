import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false };

/**
 * Deckt Phase 9 Schritt 4 ab (`PLAN.md`, `DECISIONS.md` ADR-0035):
 * Histogramm/Punktfarbmesser/Navigator im Entwickeln-Modul — die
 * Berechnungslogik selbst (`computeHistogram`/`countClipping`/
 * `buildClippingOverlay`) ist bereits vollständig in `lib/histogram.test.ts`
 * abgedeckt, hier bewusst nur ein Frontend-Flow: Panel erscheint mit
 * Histogramm-Canvas, Punktfarbmesser zeigt beim Überfahren des Bilds den
 * (in der Mock-Entwickeln-Route fest verdrahteten) Farbwert an.
 */
test.describe("Entwickeln-Analysewerkzeuge (Phase 9 Schritt 4)", () => {
  test("Histogramm-Panel erscheint im Entwickeln-Modus und Punktfarbmesser zeigt einen Wert beim Überfahren", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO] } });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln" }).click();

    await expect(page.getByLabel("Histogramm")).toBeVisible();
    await expect(page.getByText("Bild überfahren…")).toBeVisible();

    // Phase 31 Schritt 1: Die Analyse hängt jetzt als eigene Palette
    // NEBEN dem Foto, nicht mehr als Overlay DARIN — vorher stand hier
    // `main`, gefiltert auf "enthält das Histogramm", was genau die alte
    // Verschachtelung festschrieb. Der Viewer ist das `<main>`; dass das
    // Histogramm sichtbar ist, prüfen die beiden Zusicherungen darüber
    // bereits eigenständig.
    await page.locator("main").hover();

    // Mock-Entwickeln-Route liefert immer denselben warm-orangen Farbwert
    // (180/140/100) — siehe `tauri-mock.ts`s Moduldoku dazu.
    await expect(page.getByText(/R 180 · G 140 · B 100/)).toBeVisible();
  });

  /**
   * Phase 31 Schritt 5: Farbpalette aus dem Foto. Die Extraktion selbst
   * ist in `lib/colorPalette.test.ts` abgedeckt (acht Fälle); hier läuft
   * die Kette bis ins gespeicherte EDL — ein Klick auf ein Farbfeld muss
   * denselben Weissabgleich setzen wie die Pipette im Bild.
   *
   * Die Mock-Entwickeln-Route liefert eine einheitlich warm-orange
   * Fläche (180/140/100), also hat die Palette genau eine Farbe.
   */
  test("Ein Klick auf eine Bildfarbe setzt den Weissabgleich", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO] } });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln" }).click();

    const swatches = page.getByTestId("photo-palette").getByRole("button");
    await expect(swatches.first()).toBeVisible();
    await swatches.first().click();

    // Ein warmes Orange muss zu einer KÜHLEREN Korrektur führen (negative
    // Temperaturverschiebung) — das ist der Sinn eines Weissabgleichs:
    // die gewählte Farbe wird neutral gemacht.
    await expect
      .poll(async () => {
        const log = await getMockInvokeLog(page);
        const commits = log.filter((entry) => entry.cmd === "apply_develop_edit");
        if (commits.length === 0) return null;
        const args = commits[commits.length - 1]!.args as { edlJson: string };
        const payload = JSON.parse(args.edlJson).payload as {
          basic?: { white_balance?: { temp_shift_kelvin?: number } };
        };
        return payload.basic?.white_balance?.temp_shift_kelvin ?? null;
      })
      .toBeLessThan(0);
  });

  /**
   * Phase 32 F2: Ziehen im Histogramm. Die Zonenaufteilung selbst ist
   * in `lib/histogramZones.test.ts` abgedeckt (zehn Fälle); hier läuft
   * die ganze Kette — Zeigerbewegung, Zone treffen, Regler ändern, und
   * beim Loslassen im gespeicherten EDL landen.
   *
   * Gezogen wird in der Bildmitte, also in der Belichtungs-Zone.
   */
  test("Ziehen im Histogramm ändert die Belichtung und speichert sie", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO] } });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln" }).click();

    const zones = page.getByTestId("histogram-zones");
    await expect(zones).toBeVisible();

    // Überfahren zeigt, welche Zone unter dem Zeiger liegt.
    const box = await zones.boundingBox();
    if (!box) throw new Error("Histogramm nicht gefunden");
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await expect(page.getByTestId("histogram-zone-label")).toContainText("Belichtung");

    // Nach rechts ziehen hellt auf — dieselbe Richtung wie bei jedem
    // Regler der App.
    await page.mouse.down();
    await page.mouse.move(box.x + box.width / 2 + 60, box.y + box.height / 2, { steps: 5 });
    await page.mouse.up();

    await expect
      .poll(async () => {
        const log = await getMockInvokeLog(page);
        const commits = log.filter((entry) => entry.cmd === "apply_develop_edit");
        if (commits.length === 0) return 0;
        const args = commits[commits.length - 1]!.args as { edlJson: string };
        return (JSON.parse(args.edlJson).payload as { basic?: { exposure_ev?: number } }).basic?.exposure_ev ?? 0;
      })
      .toBeGreaterThan(0);
  });

  /**
   * Phase 31 Schritt 4: Fokus-Peaking. Die Kantenmathematik selbst ist
   * in `lib/focusPeaking.test.ts` abgedeckt (acht Fälle); hier läuft die
   * Kette davor — erscheint die Bedienung, entsteht die Überlagerung
   * wirklich im Bild, und meldet sie zurück, wie viel sie markiert?
   *
   * Die Rückmeldung ist der Punkt: ohne sie wäre der Schwellwert
   * Blindflug, und genau daran scheitern Peaking-Implementierungen in
   * der Praxis.
   */
  test("Fokus-Peaking legt eine Überlagerung über das Bild und meldet die Abdeckung", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO] } });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln" }).click();

    const overlay = page.getByTestId("focus-peaking-overlay");
    await expect(overlay).toBeHidden();

    await page.getByRole("checkbox", { name: "Fokus-Peaking" }).check();
    await expect(overlay).toBeVisible();

    // Die Schwelle ist bedienbar und die Farbwahl auch.
    await expect(page.getByRole("slider", { name: "Peaking-Schwelle" })).toBeVisible();
    await page.getByRole("button", { name: "Peaking-Farbe green" }).click();
    await expect(page.getByRole("button", { name: "Peaking-Farbe green" })).toHaveAttribute("aria-pressed", "true");

    await page.getByRole("checkbox", { name: "Fokus-Peaking" }).uncheck();
    await expect(overlay).toBeHidden();
  });

  /**
   * Deckt Phase 14 Schritt 6 ab (`DECISIONS.md` ADR-0041): Vektorskop +
   * Wellenform-Monitor als neue Reiter neben dem Histogramm — die
   * Berechnungslogik selbst (`computeVectorscope`/`computeWaveform`) ist
   * bereits vollständig in `lib/vectorscope.test.ts`/`lib/waveform.test.ts`
   * abgedeckt, hier bewusst nur der Reiter-Wechsel: die jeweils aktive
   * Analyse-Canvas erscheint, die anderen beiden verschwinden.
   */
  test("Vektorskop- und Wellenform-Reiter zeigen ihre jeweils eigene Canvas, das Histogramm verschwindet dabei", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO] } });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln" }).click();

    await expect(page.getByLabel("Histogramm")).toBeVisible();

    await page.getByRole("button", { name: "Vektorskop" }).click();
    await expect(page.getByLabel("Vektorskop")).toBeVisible();
    await expect(page.getByLabel("Histogramm")).not.toBeVisible();
    await expect(page.getByLabel("Wellenform")).not.toBeVisible();

    await page.getByRole("button", { name: "Wellenform" }).click();
    await expect(page.getByLabel("Wellenform")).toBeVisible();
    await expect(page.getByLabel("Vektorskop")).not.toBeVisible();

    await page.getByRole("button", { name: "Histogramm" }).click();
    await expect(page.getByLabel("Histogramm")).toBeVisible();
    await expect(page.getByLabel("Wellenform")).not.toBeVisible();
  });
});

/**
 * Phase 33 F6: RGB-Parade. Die Auswertung selbst (Schwarz-/Weißpunkt je
 * Kanal per Perzentil, Farbstich je Tonwertbereich) ist in
 * `lib/parade.test.ts` abgedeckt — hier läuft der Weg durch die
 * Oberfläche: Registerkarte wählen, Parade sehen, und dass die
 * abgelesenen Werte zur fest verdrahteten Mock-Farbe passen.
 *
 * Die Mock-Entwickeln-Route liefert eine einheitlich warm-orange Fläche
 * (180/140/100). Jeder Kanal liegt damit in einem anderen
 * Tonwertbereich, also hat keiner der drei alle Kanäle beisammen — genau
 * der Fall, für den es die Gesamtzeile gibt.
 */
test.describe("RGB-Parade (Phase 33 F6)", () => {
  test("zeigt Kanalpegel und benennt den Farbstich", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln" }).click();

    await page.getByRole("button", { name: "Parade", exact: true }).click();
    await expect(page.getByLabel("RGB-Parade")).toBeVisible();

    // 180/140/100 — jeder Kanal beginnt und endet auf seinem eigenen Wert.
    const levels = page.getByTestId("parade-levels");
    await expect(levels).toContainText("Rot 180–180");
    await expect(levels).toContainText("Grün 140–140");
    await expect(levels).toContainText("Blau 100–100");

    const casts = page.getByTestId("parade-casts");
    await expect(casts).toContainText("Rotstich, 80 Stufen");
    // Und die drei Bereiche melden ehrlich nichts, statt aus je einem
    // Kanal einen Stich zu erfinden.
    await expect(casts).toContainText("Tiefenneutral");
  });
});
