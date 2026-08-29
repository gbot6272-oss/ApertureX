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
  test("öffnet über den Kopfzeilen-Knopf und zeigt die sieben Regler", async ({ page }) => {
    await setUpWithSelectedPhoto(page);

    await page.getByRole("button", { name: "Entwickeln" }).click();

    await expect(page.getByRole("complementary", { name: "Entwickeln" })).toBeVisible();
    for (const label of ["Temperatur", "Tint", "Belichtung", "Kontrast", "Lichter", "Tiefen", "Weiß", "Schwarz"]) {
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
});
