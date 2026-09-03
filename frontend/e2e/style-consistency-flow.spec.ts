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
const OUTLIER_PHOTO = { ...PHOTO, id: "01977f4a-0000-7000-8000-000000000102", filename: "IMG_0002.CR3" };

/**
 * Deckt Phase 14 Schritt 5 ab (`DECISIONS.md` ADR-0041 Nachtrag V):
 * Stil-Konsistenz-Check fürs Shooting im "Bibliothek organisieren"-Dialog
 * (`LibraryOrganizeDialog.tsx`, "Stil-Konsistenz"-Reiter). Die echte
 * Lab-Statistik selbst (`apx-ai::style_consistency`) ist bereits in
 * Rust-Unit-Tests abgedeckt — dieser Test prüft die Frontend-Verdrahtung:
 * Ergebnis anzeigen (nur Ausreißer, nicht die konsistenten Fotos),
 * "An Shooting angleichen" committet die vorgeschlagenen Weißabgleich-/
 * Belichtungs-Deltas additiv auf den bestehenden EDL-Stand des
 * betroffenen Fotos — auch ohne es im Entwickeln-Modul zu öffnen.
 */
test("Stil-Konsistenz-Check: zeigt nur den Ausreißer und committet dessen Angleichungs-Vorschlag", async ({ page }) => {
  await installTauriMock(page, {
    folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2 }],
    photosByFolder: { [FOLDER_ID]: [PHOTO, OUTLIER_PHOTO] },
    styleConsistencyResult: [
      {
        photo: PHOTO,
        mean_l: 60.0,
        mean_a: 5.0,
        mean_b: 15.0,
        distance_from_group: 0.4,
        is_outlier: false,
        suggested_exposure_ev_delta: 0.0,
        suggested_temp_shift_kelvin_delta: 0.0,
        suggested_tint_shift_delta: 0.0,
      },
      {
        photo: OUTLIER_PHOTO,
        mean_l: 25.0,
        mean_a: -10.0,
        mean_b: -20.0,
        distance_from_group: 2.3,
        is_outlier: true,
        suggested_exposure_ev_delta: 1.2,
        suggested_temp_shift_kelvin_delta: 300.0,
        suggested_tint_shift_delta: 20.0,
      },
    ],
  });
  await page.goto("/");
  await page.getByRole("button", { name: /Urlaub/ }).click();

  await page.getByRole("button", { name: "Organisieren…" }).click();
  await page.getByRole("button", { name: "Stil-Konsistenz" }).click();

  await page.getByRole("button", { name: "Shooting prüfen" }).click();

  // Nur der Ausreißer erscheint in der Liste, nicht das konsistente Foto.
  await expect(page.getByText(OUTLIER_PHOTO.filename)).toBeVisible();
  await expect(page.getByText("Ausreißer")).toBeVisible();
  await expect(page.getByText(PHOTO.filename)).not.toBeVisible();

  await page.getByRole("button", { name: "An Shooting angleichen" }).click();

  await expect
    .poll(async () => {
      const log = await getMockInvokeLog(page);
      return log.some((entry) => entry.cmd === "apply_develop_edit" && (entry.args as { photoId: string }).photoId === OUTLIER_PHOTO.id);
    })
    .toBe(true);

  const log = await getMockInvokeLog(page);
  const commit = log.find((entry) => entry.cmd === "apply_develop_edit" && (entry.args as { photoId: string }).photoId === OUTLIER_PHOTO.id);
  const payload = JSON.parse((commit?.args as { edlJson: string }).edlJson).payload;
  expect(payload.basic.white_balance.temp_shift_kelvin).toBeCloseTo(300.0, 1);
  expect(payload.basic.white_balance.tint_shift).toBeCloseTo(20.0, 1);
  expect(payload.basic.exposure_ev).toBeCloseTo(1.2, 1);
});
