import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const VIDEO = {
  id: "01977f4a-0000-7000-8000-000000000301",
  filename: "MVI_0042.MP4",
  width: 1920,
  height: 1080,
  missing: false,
  media_kind: "video",
  duration_ms: 8000,
  frame_rate: 30,
  rating: 0,
  flag: 0,
  color_label: null,
};

/**
 * Deckt Phase 17 Schritt 9 ab (siehe `PLAN.md`, `DECISIONS.md`
 * ADR-0062): die Video-Stabilisierung im Video-Modul.
 *
 * Was hier NICHT geprüft werden kann und auch nicht geprüft wird: die
 * eigentliche Stabilisierung. Sie braucht `ffmpeg` und läuft komplett in
 * Rust — geprüft ist sie dort, in `apx_stacking::stabilize`s sechzehn
 * Unit-Tests. Hier läuft die Kette davor: kommen die Reglerwerte
 * unverfälscht am Command an, schaltet die Oberfläche auf das Ergebnis
 * um, und sagt sie es, wenn die gewählte Einstellung gar nichts
 * bewirken kann.
 *
 * Zugleich der erste e2e-Test überhaupt, der das Video-Modul betritt —
 * die Phasen 16/17 haben es bis hierher nur über Rust-Tests und `tsc`
 * abgesichert.
 */
test.describe("Video-Stabilisierung (Phase 17 Schritt 9)", () => {
  async function openVideo(page: import("@playwright/test").Page) {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: "/home/user/Videos/Urlaub", photo_count: 1, parent_id: null, missing: false }],
      photosByFolder: { [FOLDER_ID]: [VIDEO] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: VIDEO.filename }).click();
  }

  async function stabilizeCalls(page: import("@playwright/test").Page) {
    const log = await getMockInvokeLog(page);
    return log.filter((entry) => entry.cmd === "stabilize_video");
  }

  test("schickt die eingestellten Regler an den Command", async ({ page }) => {
    await openVideo(page);

    const smoothing = page.getByRole("slider", { name: "Glättung" });
    const crop = page.getByRole("slider", { name: "Zuschnitt" });
    await expect(smoothing).toBeVisible();
    await expect(crop).toBeVisible();

    // Vorgaben spiegeln `StabilizeParams::default()` drüben in Rust.
    await expect(smoothing).toHaveValue("12");
    await expect(crop).toHaveValue("1.1");

    await smoothing.fill("30");
    await crop.fill("1.2");
    await page.getByRole("button", { name: "Stabilisieren" }).click();

    await expect
      .poll(async () => (await stabilizeCalls(page)).length, { timeout: 5000 })
      .toBe(1);
    const [call] = await stabilizeCalls(page);
    expect(call.args).toMatchObject({
      photoId: VIDEO.id,
      smoothingRadius: 30,
      cropZoom: 1.2,
    });
  });

  test("schaltet nach dem Lauf auf das entstandene Video um", async ({ page }) => {
    await openVideo(page);
    await page.getByRole("button", { name: "Stabilisieren" }).click();

    // `register_video_result_as_new_photo` legt ein NEUES Katalog-Video
    // neben dem Original an — das Original muss danach unverändert
    // danebenstehen, sonst wäre die Bearbeitung destruktiv.
    await expect(page.getByRole("img", { name: "MVI_0042_stabilisiert.MP4" })).toBeVisible();
    await expect(page.getByRole("img", { name: VIDEO.filename })).toBeVisible();
  });

  test("warnt, wenn ohne Zuschnitt gar nichts korrigiert werden kann", async ({ page }) => {
    await openVideo(page);
    const hint = page.getByText("Ohne Zuschnitt bleibt kein Rand zum Verdecken");
    await expect(hint).toBeHidden();

    await page.getByRole("slider", { name: "Zuschnitt" }).fill("1");
    await expect(hint).toBeVisible();
  });
});
