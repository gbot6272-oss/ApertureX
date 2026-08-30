import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

/**
 * Deckt Phase 6 Schritt 8 ab (`DECISIONS.md` ADR-0032, `SPEC.md` §3.4):
 * Schnappschüsse (benannte EDL-Zwischenstände zusätzlich zum linearen
 * Verlauf) und Vorher/Nachher (vier Ansichten im Viewer).
 */
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

async function lastCommittedExposure(page: import("@playwright/test").Page): Promise<number> {
  const log = await getMockInvokeLog(page);
  const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
  return JSON.parse((lastCommit?.args as { edlJson: string }).edlJson).payload.basic.exposure_ev;
}

test.describe("Workflow: Schnappschüsse + Vorher/Nachher", () => {
  test("Schnappschuss speichern und wiederherstellen committet jeweils, Umbenennen und Löschen auch", async ({ page }) => {
    await setUpWithSelectedPhoto(page);

    const exposureInput = page.getByRole("spinbutton", { name: "Belichtung (Zahlenwert)" }).first();
    await exposureInput.fill("0.8");
    await exposureInput.blur();
    await expect.poll(async () => lastCommittedExposure(page)).toBeCloseTo(0.8, 2);

    page.once("dialog", (dialog) => void dialog.accept("Erste Version"));
    await page.getByRole("button", { name: "+ Schnappschuss vom aktuellen Stand" }).click();
    await expect(page.getByRole("button", { name: "Erste Version", exact: true })).toBeVisible();

    // Weiter bearbeiten, dann den Schnappschuss wiederherstellen.
    await exposureInput.fill("-1.2");
    await exposureInput.blur();
    await expect.poll(async () => lastCommittedExposure(page)).toBeCloseTo(-1.2, 2);

    await page.getByRole("button", { name: "Erste Version", exact: true }).click();
    await expect.poll(async () => lastCommittedExposure(page)).toBeCloseTo(0.8, 2);
    await expect(exposureInput).toHaveValue("0.8");

    page.once("dialog", (dialog) => void dialog.accept("Referenz"));
    await page.getByTitle("Umbenennen").click();
    await expect(page.getByRole("button", { name: "Referenz", exact: true })).toBeVisible();

    await page.getByRole("button", { name: "Schnappschuss Referenz löschen" }).click();
    await expect(page.getByText("Noch keine Schnappschüsse.")).toBeVisible();
  });

  test("Vorher/Nachher-Modi schalten die Ansicht um, jeweils nur ein Modus aktiv", async ({ page }) => {
    await setUpWithSelectedPhoto(page);

    const sideBySideButton = page.getByRole("button", { name: "Links/Rechts" });
    const stackedButton = page.getByRole("button", { name: "Oben/Unten" });
    const splitButton = page.getByRole("button", { name: "Geteilt", exact: true });

    await expect(page.getByLabel("Vorher/Nachher")).toHaveCount(0);

    await sideBySideButton.click();
    await expect(sideBySideButton).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByLabel("Vorher/Nachher")).toBeVisible();
    await expect(page.getByText("Vorher", { exact: true })).toBeVisible();
    await expect(page.getByText("Nachher", { exact: true })).toBeVisible();

    // Ein anderer Modus übernimmt, der vorherige wird deaktiviert.
    await stackedButton.click();
    await expect(stackedButton).toHaveAttribute("aria-pressed", "true");
    await expect(sideBySideButton).toHaveAttribute("aria-pressed", "false");
    await expect(page.getByLabel("Vorher/Nachher")).toBeVisible();

    // Erneutes Klicken auf den aktiven Modus schaltet zurück auf "keine Ansicht".
    await stackedButton.click();
    await expect(stackedButton).toHaveAttribute("aria-pressed", "false");
    await expect(page.getByLabel("Vorher/Nachher")).toHaveCount(0);

    await splitButton.click();
    await expect(splitButton).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByLabel("Vorher/Nachher")).toBeVisible();
  });
});
