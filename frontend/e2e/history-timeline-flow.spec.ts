import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false };

/**
 * Deckt Phase 9 Schritt 7 ab (`PLAN.md`, `DECISIONS.md` ADR-0035): die
 * Zeitleisten-Ansicht und den Verlaufs-Vergleich im „Verlauf"-Dialog. Die
 * Backend-Logik (`repository::edits::goto`/`list_history`) ist bereits
 * vollständig in `apx-catalog`s Rust-Unit-Tests abgedeckt — hier bewusst
 * nur ein Frontend-Flow: zwei Bearbeitungsschritte committen, den Dialog
 * öffnen, per Zeitleisten-Punkt zum ersten Schritt zurückspringen, und
 * den Diff zwischen beiden Schritten sehen.
 */
test.describe("Verlauf: Zeitleiste & Vergleich (Phase 9 Schritt 7)", () => {
  test("Zeitleiste zeigt beide Schritte, ein Klick springt zurück, der Diff zeigt die Belichtungsänderung", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO] } });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln" }).click();

    const exposureInput = page.getByRole("spinbutton", { name: "Belichtung (Zahlenwert)" });
    await exposureInput.fill("1");
    await exposureInput.blur();
    await expect(exposureInput).toHaveValue("1");

    await exposureInput.fill("2");
    await exposureInput.blur();
    await expect(exposureInput).toHaveValue("2");

    await page.getByRole("button", { name: "Verlauf", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "Verlauf" });
    await expect(dialog.getByRole("group", { name: "Zeitleiste" })).toBeVisible();

    const timelinePoints = dialog.getByRole("button", { name: /Zu Verlaufsschritt/ });
    await expect(timelinePoints).toHaveCount(2);

    await timelinePoints.first().click();
    await dialog.getByRole("button", { name: "Schließen" }).click();
    await expect(exposureInput).toHaveValue("1");

    await page.getByRole("button", { name: "Verlauf", exact: true }).click();
    const diffDialog = page.getByRole("dialog", { name: "Verlauf" });
    await diffDialog.getByLabel("Verlaufsschritt A").selectOption("0");
    await diffDialog.getByLabel("Verlaufsschritt B").selectOption("1");
    await expect(diffDialog.getByText("basic.exposure_ev")).toBeVisible();
  });
});
