import { expect, test } from "@playwright/test";

import { installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000a01";
const PHOTO = {
  id: "01977f4a-0000-7000-8000-000000000a02",
  filename: "IMG_0001.CR3",
  width: 6000,
  height: 4000,
  missing: false,
  camera_model: "Canon EOS R5",
};

/**
 * Phase 34 F7 (siehe `DECISIONS.md` ADR-0070). Die Zuordnungslogik
 * liegt in `camera_default.rs`s sechs Rust-Tests — hier laeuft der Weg
 * durch die Oberflaeche: Foto entwickeln, Stand zur Vorgabe erklaeren,
 * Vorgabe wieder entfernen.
 */
test.describe("Standardentwicklung je Kamera (Phase 34 F7)", () => {
  test("erklaert den aktuellen Entwickeln-Stand zur Vorgabe und nimmt sie zurueck", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: "/home/user/Fotos/Shooting", photo_count: 1, parent_id: null, missing: false }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Shooting/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln", exact: true }).click();

    const exposure = page.getByRole("spinbutton", { name: "Belichtung (Zahlenwert)" });
    await exposure.fill("0.5");
    await exposure.blur();
    await expect(exposure).toHaveValue("0.5");

    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Standardentwicklung je Kamera…" }).click();

    await expect(page.getByTestId("camera-defaults-list")).toContainText("Noch keine Vorgabe");
    await page.getByTestId("camera-defaults-save").click();

    await expect(page.getByRole("status")).toContainText("Canon EOS R5");
    await expect(page.getByTestId("camera-defaults-list")).toContainText("Canon EOS R5");

    await page.getByRole("button", { name: "Vorgabe für Canon EOS R5 entfernen" }).click();
    await expect(page.getByTestId("camera-defaults-list")).toContainText("Noch keine Vorgabe");
  });

  test("ohne geoeffnetes Foto laesst sich keine Vorgabe setzen", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: "/home/user/Fotos/Shooting", photo_count: 1, parent_id: null, missing: false }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Shooting/ }).click();
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Standardentwicklung je Kamera…" }).click();

    await expect(page.getByTestId("camera-defaults-save")).toBeDisabled();
  });
});
