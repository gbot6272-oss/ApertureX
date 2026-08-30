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

async function setUpWithSelectedPhoto(page: import("@playwright/test").Page) {
  await installTauriMock(page, {
    folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
    photosByFolder: { [FOLDER_ID]: [PHOTO] },
  });
  await page.goto("/");
  await page.getByRole("button", { name: /Urlaub/ }).click();
  await page.getByRole("img", { name: PHOTO.filename }).click();
}

test.describe("Entwickeln-Panel", () => {
  test("öffnet über den Kopfzeilen-Knopf und zeigt die zwölf Grundeinstellungs-Regler", async ({ page }) => {
    await setUpWithSelectedPhoto(page);

    await page.getByRole("button", { name: "Entwickeln" }).click();

    await expect(page.getByRole("complementary", { name: "Entwickeln" })).toBeVisible();

    const whiteBalanceGroup = page.getByRole("group", { name: "Weißabgleich" });
    for (const label of ["Temperatur", "Tint"]) {
      await expect(whiteBalanceGroup.getByRole("slider", { name: label })).toBeVisible();
    }

    // Eigene, per sr-only-Legende benannte Gruppe (siehe DevelopPanel.tsx) —
    // sonst wäre z. B. "Sättigung" mit dem gleichnamigen HSL-Band-Regler
    // mehrdeutig, da beide Abschnitte gleichzeitig sichtbar sind.
    const toneGroup = page.getByRole("group", { name: "Grundeinstellungen (Ton)" });
    for (const label of ["Belichtung", "Kontrast", "Lichter", "Tiefen", "Weiß", "Schwarz", "Textur", "Klarheit", "Dunst entfernen", "Dynamik", "Sättigung"]) {
      await expect(toneGroup.getByRole("slider", { name: label })).toBeVisible();
    }
  });

  test("Direkteingabe committet den Regler-Wert ans Backend", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "Entwickeln" }).click();

    const exposureInput = page.getByRole("spinbutton", { name: "Belichtung (Zahlenwert)" });
    await exposureInput.fill("1.5");
    await exposureInput.blur();

    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      return log.filter((entry) => entry.cmd === "apply_develop_edit").length;
    }).toBeGreaterThan(0);

    const log = await getMockInvokeLog(page);
    const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
    const edlJson = (lastCommit?.args as { edlJson: string }).edlJson;
    expect(JSON.parse(edlJson).payload.basic.exposure_ev).toBeCloseTo(1.5);
  });

  test("Rückgängig stellt den vorherigen Zustand über das Backend wieder her", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "Entwickeln" }).click();

    const exposureInput = page.getByRole("spinbutton", { name: "Belichtung (Zahlenwert)" });
    await exposureInput.fill("2");
    await exposureInput.blur();

    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      return log.some((entry) => entry.cmd === "apply_develop_edit");
    }).toBe(true);
    await expect(exposureInput).toHaveValue("2");

    await page.getByRole("button", { name: "Rückgängig" }).click();

    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      return log.some((entry) => entry.cmd === "undo_develop_edit");
    }).toBe(true);
    await expect(exposureInput).toHaveValue("0");
  });

  test("Doppelklick auf einen Regler setzt ihn auf neutral zurück und committet", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "Entwickeln" }).click();

    const contrastInput = page.getByRole("spinbutton", { name: "Kontrast (Zahlenwert)" });
    await contrastInput.fill("40");
    await contrastInput.blur();
    await expect(contrastInput).toHaveValue("40");

    await page.getByRole("slider", { name: "Kontrast" }).dblclick();

    await expect(contrastInput).toHaveValue("0");
    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      return log.filter((entry) => entry.cmd === "apply_develop_edit").length;
    }).toBeGreaterThanOrEqual(2);
  });

  test("ein Weißabgleich-Preset setzt Temperatur/Tint absolut und committet", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "Entwickeln" }).click();

    await page.getByRole("combobox", { name: "Weißabgleich-Preset" }).selectOption("tungsten");

    await expect(page.getByRole("spinbutton", { name: "Temperatur (Zahlenwert)" })).toHaveValue("-1200");
    await expect(page.getByRole("spinbutton", { name: "Tint (Zahlenwert)" })).toHaveValue("-5");
    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      return log.some((entry) => entry.cmd === "apply_develop_edit");
    }).toBe(true);
  });

  test("die Pipette liest einen Bildpunkt und korrigiert den Weißabgleich", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "Entwickeln" }).click();

    const pipetteButton = page.getByRole("button", { name: "Pipette" });
    await expect(pipetteButton).toHaveAttribute("aria-pressed", "false");
    await pipetteButton.click();
    await expect(pipetteButton).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByText("Klicken Sie in einen neutral-grauen Bildpunkt.")).toBeVisible();

    // Das Mock-Entwickeln-Bild ist bewusst warm-orange gefärbt (siehe
    // `tauri-mock.ts`) — ein Klick muss also eine echte, von 0
    // verschiedene Korrektur auslösen (kühlt Temperatur ab). Geklickt wird
    // auf den Viewer-Container selbst statt auf das `<canvas>` (das trägt
    // `pointer-events-none`, damit die Info-Kachel und der Canvas
    // Mausereignisse an den Container durchreichen).
    await page.getByRole("main").click();

    await expect(pipetteButton).toHaveAttribute("aria-pressed", "false");
    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      return log.filter((entry) => entry.cmd === "apply_develop_edit").length;
    }).toBeGreaterThan(0);

    const log = await getMockInvokeLog(page);
    const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
    const edlJson = (lastCommit?.args as { edlJson: string }).edlJson;
    const whiteBalance = JSON.parse(edlJson).payload.basic.white_balance as { temp_shift_kelvin: number; tint_shift: number };
    expect(whiteBalance.temp_shift_kelvin).toBeLessThan(0);
  });

  test("Kurven: ein Preset auf einem Kanal wird als Punktkurve committet", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "Entwickeln" }).click();

    await page.getByRole("group", { name: "Kurven" }).getByRole("button", { name: "Grün" }).click();
    await page.getByRole("combobox", { name: "Kurven-Preset" }).selectOption("strong_contrast");

    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      return log.some((entry) => entry.cmd === "apply_develop_edit");
    }).toBe(true);

    const log = await getMockInvokeLog(page);
    const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
    const edlJson = (lastCommit?.args as { edlJson: string }).edlJson;
    const green = JSON.parse(edlJson).payload.curves.green as { kind: string; points: Array<{ input: number; output: number }> };
    expect(green.kind).toBe("Points");
    expect(green.points).toHaveLength(4);
    expect(green.points[0]).toEqual({ input: 0, output: 0 });
    expect(green.points[3]).toEqual({ input: 1, output: 1 });
  });

  test("Kurven: der Parametrisch-Modus committet Regler-Werte statt Punkten", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "Entwickeln" }).click();

    await page.getByRole("group", { name: "Kurven" }).getByRole("button", { name: "Rot" }).click();
    await page.getByRole("button", { name: "Parametrisch" }).click();

    const shadowsInput = page.getByRole("spinbutton", { name: "Tiefen (Kurve) (Zahlenwert)" });
    await shadowsInput.fill("40");
    await shadowsInput.blur();

    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      return log.some((entry) => entry.cmd === "apply_develop_edit");
    }).toBe(true);

    const log = await getMockInvokeLog(page);
    const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
    const edlJson = (lastCommit?.args as { edlJson: string }).edlJson;
    const red = JSON.parse(edlJson).payload.curves.red as { kind: string; shadows: number };
    expect(red.kind).toBe("Parametric");
    expect(red.shadows).toBeCloseTo(40);
  });

  test("Kurven: ein Klick in den Editor fügt einen neuen Kontrollpunkt hinzu", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "Entwickeln" }).click();

    // RGB ist standardmäßig aktiv und startet mit der neutralen
    // Zwei-Punkte-Kurve — ein Klick fernab der beiden Eckpunkte muss einen
    // dritten Punkt einfügen.
    await page.getByRole("img", { name: "Kurven-Diagramm" }).click({ position: { x: 80, y: 60 } });

    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      return log.some((entry) => entry.cmd === "apply_develop_edit");
    }).toBe(true);

    const log = await getMockInvokeLog(page);
    const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
    const edlJson = (lastCommit?.args as { edlJson: string }).edlJson;
    const rgb = JSON.parse(edlJson).payload.curves.rgb as { kind: string; points: Array<{ input: number; output: number }> };
    expect(rgb.kind).toBe("Points");
    expect(rgb.points).toHaveLength(3);
  });

  test("Kurven: die numerische Punkteingabe committet einen exakten Ausgabewert", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "Entwickeln" }).click();

    // Erster Endpunkt (Eingabe fest 0) fokussieren, dann per Zahlenfeld
    // einen exakten Ausgabewert setzen statt zu ziehen.
    await page.getByRole("slider", { name: "Kurvenpunkt 1" }).focus();
    const outputInput = page.getByRole("spinbutton", { name: "Kurvenpunkt Ausgabe" });
    await outputInput.fill("0.3");
    await outputInput.blur();

    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      return log.some((entry) => entry.cmd === "apply_develop_edit");
    }).toBe(true);

    const log = await getMockInvokeLog(page);
    const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
    const edlJson = (lastCommit?.args as { edlJson: string }).edlJson;
    const rgb = JSON.parse(edlJson).payload.curves.rgb as { points: Array<{ input: number; output: number }> };
    expect(rgb.points[0]).toEqual({ input: 0, output: 0.3 });
  });

  test("HSL: ein Band-Regler committet das entsprechende Feld", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "Entwickeln" }).click();

    const hslGroup = page.getByRole("group", { name: "HSL" });
    await hslGroup.getByRole("button", { name: "Aqua" }).click();
    const saturationInput = hslGroup.getByRole("spinbutton", { name: "Sättigung (Zahlenwert)" });
    await saturationInput.fill("-40");
    await saturationInput.blur();

    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      return log.some((entry) => entry.cmd === "apply_develop_edit");
    }).toBe(true);

    const log = await getMockInvokeLog(page);
    const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
    const edlJson = (lastCommit?.args as { edlJson: string }).edlJson;
    const hsl = JSON.parse(edlJson).payload.hsl as { aqua: { saturation: number }; red: { saturation: number } };
    expect(hsl.aqua.saturation).toBeCloseTo(-40);
    expect(hsl.red.saturation).toBe(0);
  });

  test("Farbmischer: ein Bildklick legt eine Region an ihrem Farbton an", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "Entwickeln" }).click();

    const addButton = page.getByRole("button", { name: "Region hinzufügen" });
    await expect(addButton).toHaveAttribute("aria-pressed", "false");
    await addButton.click();
    await expect(addButton).toHaveAttribute("aria-pressed", "true");

    await page.getByRole("main").click();

    await expect(addButton).toHaveAttribute("aria-pressed", "false");
    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      return log.some((entry) => entry.cmd === "apply_develop_edit");
    }).toBe(true);

    const log = await getMockInvokeLog(page);
    const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
    const edlJson = (lastCommit?.args as { edlJson: string }).edlJson;
    const colorMixer = JSON.parse(edlJson).payload.color_mixer as { regions: Array<{ target_hue_degrees: number }> };
    expect(colorMixer.regions).toHaveLength(1);
    // Das Mock-Entwickeln-Bild ist warm-orange (180, 140, 100) — der
    // resultierende Farbton liegt im Gelb/Orange-Bereich (grob 20-45°).
    expect(colorMixer.regions[0]?.target_hue_degrees).toBeGreaterThan(0);
    expect(colorMixer.regions[0]?.target_hue_degrees).toBeLessThan(60);

    // Die neu angelegte Region wird sofort zur Bearbeitung ausgewählt.
    const hueShiftInput = page.getByRole("spinbutton", { name: "Farbton-Verschiebung (Zahlenwert)" });
    await hueShiftInput.fill("25");
    await hueShiftInput.blur();

    await expect.poll(async () => {
      const updatedLog = await getMockInvokeLog(page);
      return updatedLog.filter((entry) => entry.cmd === "apply_develop_edit").length;
    }).toBeGreaterThan(1);
    const updatedLog = await getMockInvokeLog(page);
    const lastUpdate = [...updatedLog].reverse().find((entry) => entry.cmd === "apply_develop_edit");
    const updatedEdlJson = (lastUpdate?.args as { edlJson: string }).edlJson;
    const updatedRegions = JSON.parse(updatedEdlJson).payload.color_mixer.regions as Array<{ hue_shift: number }>;
    expect(updatedRegions[0]?.hue_shift).toBeCloseTo(25);
  });

  test("Color Grading: ein Farbrad per Tastatur committet Farbton/Sättigung", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "Entwickeln" }).click();
    // Der Klick oben löst `loadDevelopStateForPhoto` (ein asynchrones
    // `current_develop_edit`) aus, das den Panel-Zustand — ungefragt —
    // mit dem (noch neutralen) Backend-Stand überschreibt, sobald es
    // durchläuft. Ohne diese kurze, deterministische Wartezeit (die
    // Mock-Zusage löst ohne echte Netzwerklatenz fast sofort auf) würde
    // ein sofort danach gesetzter Regler-Wert von diesem Überschreiben
    // wieder verworfen — anders als in den übrigen Tests dieser Datei
    // gibt es hier keine weitere await-Aktion dazwischen, die dafür
    // zufällig genug Zeit ließe.
    await page.waitForTimeout(100);

    const shadowWheel = page.getByRole("slider", { name: "Schatten-Farbrad" });
    await shadowWheel.focus();
    // Je ein Schritt nach rechts (2° fein, siehe ColorWheel.tsx) und nach
    // oben (Sättigung anheben) — zwischen den Tastendrücken auf das
    // sichtbare `aria-valuenow` gewartet, statt Tastendrücke schneller
    // als Reacts Re-Render aufeinanderfolgen zu lassen.
    await shadowWheel.press("ArrowRight");
    await expect(shadowWheel).toHaveAttribute("aria-valuenow", "2");
    await shadowWheel.press("ArrowUp");

    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      return log.some((entry) => entry.cmd === "apply_develop_edit");
    }).toBe(true);

    const log = await getMockInvokeLog(page);
    const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
    const edlJson = (lastCommit?.args as { edlJson: string }).edlJson;
    const shadows = JSON.parse(edlJson).payload.color_grading.shadows as { hue_degrees: number; saturation: number };
    expect(shadows.hue_degrees).toBeCloseTo(2);
    expect(shadows.saturation).toBeGreaterThan(0);
  });

  test("Color Grading: Balance und Überblendung committen", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "Entwickeln" }).click();

    const balanceInput = page.getByRole("spinbutton", { name: "Balance (Zahlenwert)" });
    await balanceInput.fill("-30");
    await balanceInput.blur();

    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      return log.some((entry) => entry.cmd === "apply_develop_edit");
    }).toBe(true);

    const log = await getMockInvokeLog(page);
    const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
    const edlJson = (lastCommit?.args as { edlJson: string }).edlJson;
    const colorGrading = JSON.parse(edlJson).payload.color_grading as { balance: number; blending: number };
    expect(colorGrading.balance).toBeCloseTo(-30);
    expect(colorGrading.blending).toBe(50); // unverändertes Default
  });
});
