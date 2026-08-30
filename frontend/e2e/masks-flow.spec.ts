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
  await page.getByRole("button", { name: "Entwickeln" }).click();
}

async function lastMasks(page: import("@playwright/test").Page): Promise<Array<{ name: string; visible: boolean }>> {
  const log = await getMockInvokeLog(page);
  const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
  const edlJson = (lastCommit?.args as { edlJson: string }).edlJson;
  return JSON.parse(edlJson).payload.masks;
}

/**
 * Deckt Phase 6 Schritt 3 ab (`DECISIONS.md` ADR-0032): Anlegen einer
 * Linearen-/Radialen-Verlauf-Maske, Ziehgriffe im Viewer (per
 * Tastatur-Feinsteuerung, analog zum bestehenden Freistellen-Werkzeug-
 * Test), Sichtbarkeit/Umbenennen/Löschen, sowie ein Regler für die
 * Maskeneigenen Grundeinstellungen.
 */
test.describe("Masken-Panel", () => {
  test("legt eine Lineare-Verlauf-Maske an, zeigt sie in der Liste und committet sofort", async ({ page }) => {
    await setUpWithSelectedPhoto(page);

    await expect(page.getByRole("complementary", { name: "Masken" })).toBeVisible();
    await expect(page.getByText("Keine Masken vorhanden.")).toBeVisible();

    await page.getByRole("button", { name: "+ Linearer Verlauf" }).click();

    await expect(page.getByRole("button", { name: "Linearer Verlauf", exact: true })).toBeVisible();
    const masks = await lastMasks(page);
    expect(masks).toHaveLength(1);
    expect(masks[0].name).toBe("Linearer Verlauf");
    expect(masks[0].visible).toBe(true);
  });

  test("Ziehgriff des Linearen Verlaufs verschiebt den Startpunkt per Pfeiltaste und committet", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "+ Linearer Verlauf" }).click();

    const startHandle = page.getByRole("slider", { name: "Linearer Verlauf: Startpunkt" });
    await expect(startHandle).toBeVisible();
    await startHandle.focus();
    await startHandle.press("ArrowRight");
    await startHandle.press("ArrowRight");

    await expect
      .poll(async () => {
        const log = await getMockInvokeLog(page);
        return log.filter((entry) => entry.cmd === "apply_develop_edit").length;
      })
      .toBeGreaterThan(1);

    const log = await getMockInvokeLog(page);
    const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
    const edlJson = (lastCommit?.args as { edlJson: string }).edlJson;
    const masks = JSON.parse(edlJson).payload.masks as Array<{ components: Array<{ geometry: { x1: number } }> }>;
    // Startwert war x1 = 0.5 (siehe `defaultLinearGradientGeometry`).
    expect(masks[0].components[0].geometry.x1).toBeGreaterThan(0.5);
  });

  test("Radialer-Verlauf-Ziehgriff vergrößert den Radius per Pfeiltaste und committet", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "+ Radialer Verlauf" }).click();

    const radiusHandle = page.getByRole("slider", { name: "Radialer Verlauf: Radius" });
    await expect(radiusHandle).toBeVisible();
    await radiusHandle.focus();
    await radiusHandle.press("ArrowRight");

    await expect
      .poll(async () => {
        const masks = await lastMasks(page);
        return (masks[0] as unknown as { components: Array<{ geometry: { radius_x: number } }> }).components[0].geometry.radius_x;
      })
      .toBeGreaterThan(0.3);
  });

  test("Sichtbarkeit umschalten, umbenennen und löschen committen jeweils sofort", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "+ Linearer Verlauf" }).click();

    const visibilityButton = page.getByRole("button", { name: "Linearer Verlauf ausblenden" });
    await visibilityButton.click();
    await expect.poll(async () => (await lastMasks(page))[0].visible).toBe(false);

    page.once("dialog", (dialog) => {
      void dialog.accept("Mein Vordergrund");
    });
    await page.getByTitle("Umbenennen").click();
    await expect.poll(async () => (await lastMasks(page))[0].name).toBe("Mein Vordergrund");

    await page.getByRole("button", { name: "Mein Vordergrund löschen" }).click();
    await expect.poll(async () => (await lastMasks(page)).length).toBe(0);
    await expect(page.getByText("Keine Masken vorhanden.")).toBeVisible();
  });

  test("Maskeneigener Belichtung-Regler ändert nur die Anpassungen der ausgewählten Maske", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "+ Linearer Verlauf" }).click();

    const maskExposure = page.getByRole("spinbutton", { name: "Belichtung (Zahlenwert)" }).nth(1);
    await maskExposure.fill("0.8");
    await maskExposure.blur();

    await expect
      .poll(async () => {
        const masks = await lastMasks(page);
        return (masks[0] as unknown as { adjustments: { basic: { exposure_ev: number } } }).adjustments.basic.exposure_ev;
      })
      .toBeCloseTo(0.8, 2);

    // Der globale Belichtung-Regler bleibt bei 0 — nur die Maske hat sich geändert.
    const globalExposure = page.getByRole("spinbutton", { name: "Belichtung (Zahlenwert)" }).first();
    await expect(globalExposure).toHaveValue("0");
  });
});
