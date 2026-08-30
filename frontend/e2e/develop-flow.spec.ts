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
    for (const label of [
      "Temperatur",
      "Tint",
      "Belichtung",
      "Kontrast",
      "Lichter",
      "Tiefen",
      "Weiß",
      "Schwarz",
      "Textur",
      "Klarheit",
      "Dunst entfernen",
      "Dynamik",
      "Sättigung",
    ]) {
      await expect(page.getByRole("slider", { name: label })).toBeVisible();
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

    await page.getByRole("button", { name: "Grün" }).click();
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

    await page.getByRole("button", { name: "Rot" }).click();
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
});
