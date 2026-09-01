import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";

/**
 * Deckt Phase 9 Schritt 11 ab (`PLAN.md`, `DECISIONS.md` ADR-0035 Punkt 5):
 * Tethered Shooting im „Tethering…"-Dialog. Der eigentliche Kamera-/
 * Download-Ablauf (`apx_tether::FakeBackend`/`Gphoto2Backend`) und die
 * Import-Preset-Auflösung sind bereits vollständig in Rust-Unit-Tests
 * abgedeckt — hier bewusst nur ein Frontend-Flow: verbinden zeigt die
 * (klar als Simulation markierte) Kamera, auslösen zeigt die neue
 * Aufnahme.
 */
test.describe("Tethered Shooting (Phase 9 Schritt 11)", () => {
  test("verbinden zeigt die simulierte Kamera, auslösen zeigt die neue Aufnahme", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 0 }],
      photosByFolder: { [FOLDER_ID]: [] },
      tetherCapturePhoto: {
        id: "01977f4a-0000-7000-8000-000000000201",
        filename: "TETHER_0001.jpg",
        file_size: 12345,
        width: 4,
        height: 3,
        missing: false,
        rating: 0,
        flag: 0,
      },
    });
    await page.goto("/");

    await page.getByRole("button", { name: "Tethering…" }).click();
    const dialog = page.getByRole("dialog", { name: "Tethered Shooting" });
    await expect(dialog.getByText("Keine Kamera verbunden.")).toBeVisible();

    await dialog.getByRole("button", { name: "Kamera verbinden…" }).click();
    await expect(dialog.getByText("Simulierte Kamera (usb:mock,000)")).toBeVisible();
    await expect(dialog.getByText("Simulation")).toBeVisible();

    await dialog.getByRole("button", { name: "Auslösen" }).click();
    await expect(dialog.getByText("Aufgenommen: TETHER_0001.jpg")).toBeVisible();
  });
});
