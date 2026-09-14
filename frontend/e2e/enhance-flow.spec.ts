import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false };

/**
 * Deckt Phase 9 Schritt 6 ab (`PLAN.md`, `DECISIONS.md` ADR-0035):
 * Entrauschung/Hochskalierung im Entwickeln-Panel. Die Algorithmen selbst
 * (`apx_ai::denoise`/`apx_ai::upscale`) sind bereits vollständig in
 * Rust-Unit-Tests abgedeckt — hier bewusst nur ein Frontend-Flow: beide
 * Knöpfe lösen den jeweiligen Aufruf aus und zeigen den Ziel-Pfad an.
 */
/**
 * Phase 31 Schritt 7: "Horizont ausrichten" in der Befehlspalette. Die
 * Kantenerkennung selbst (Canny + Hough) ist seit Phase 13 Schritt 4 in
 * `apx_ai::upright` samt Rust-Tests abgedeckt — sie lag nur als Eintrag
 * einer Klappliste tief in den Objektivkorrekturen, wo sie niemand
 * sucht. Geprüft wird deshalb genau das, was neu ist: dass der Befehl
 * unter seinem gebräuchlichen Namen auffindbar ist und den vorhandenen
 * Erkennungs-Command auslöst.
 */
test("Horizont ausrichten ist über die Befehlspalette erreichbar", async ({ page }) => {
  await installTauriMock(page, {
    folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
    photosByFolder: { [FOLDER_ID]: [PHOTO] },
  });
  await page.goto("/");
  await page.getByRole("button", { name: /Urlaub/ }).click();
  await page.getByRole("img", { name: PHOTO.filename }).click();

  await page.keyboard.press("ControlOrMeta+k");
  await page.getByRole("textbox").first().fill("Horizont");
  await page.getByText("Horizont ausrichten").first().click();

  await expect
    .poll(async () => {
      const log = await getMockInvokeLog(page);
      return log.filter((entry) => entry.cmd === "detect_upright_correction").length;
    })
    .toBeGreaterThan(0);
});

test.describe("Entrauschung & Hochskalierung (Phase 9 Schritt 6)", () => {
  test("Entrauschen und Hochskalieren zeigen jeweils den Ziel-Pfad an", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
      denoisedPhotoPath: "/home/user/Fotos/Urlaub/IMG_0001_entrauscht.png",
      upscaledPhotoPath: "/home/user/Fotos/Urlaub/IMG_0001_hochskaliert.png",
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln" }).click();

    // Phase 18 Schritt 4: dieser Abschnitt liegt jetzt hinter einer eigenen Registerkarte.
    await page.getByRole("tab", { name: "Verlauf & Werkzeuge" }).click();

    const group = page.getByRole("group", { name: "Entrauschung & Hochskalierung" });
    await group.getByRole("button", { name: "Entrauschen" }).click();
    await expect(page.getByText("Entrauscht: /home/user/Fotos/Urlaub/IMG_0001_entrauscht.png")).toBeVisible();

    await group.getByRole("button", { name: "2× hochskalieren" }).click();
    await expect(page.getByText("Hochskaliert: /home/user/Fotos/Urlaub/IMG_0001_hochskaliert.png")).toBeVisible();
  });
});

/**
 * Deckt Phase 11 Schritt 1 ab (`PLAN.md`, `DECISIONS.md` ADR-0038): DNG-
 * Konvertierung im Entwickeln-Panel. Der Encoder selbst
 * (`apx_export::dng::encode_linear_dng`) ist bereits per Rust-Unit-Tests
 * abgedeckt — hier bewusst nur ein Frontend-Flow: der Knopf löst den
 * Aufruf aus und zeigt den Ziel-Pfad an.
 */
test.describe("DNG-Konvertierung (Phase 11 Schritt 1)", () => {
  test("Als DNG konvertieren zeigt den Ziel-Pfad an", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
      convertedDngPath: "/home/user/Fotos/Urlaub/IMG_0001.dng",
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln" }).click();

    await page.getByRole("tab", { name: "Verlauf & Werkzeuge" }).click();

    const group = page.getByRole("group", { name: "DNG-Konvertierung" });
    await group.getByRole("button", { name: "Als DNG konvertieren" }).click();
    await expect(page.getByText("Als DNG konvertiert: /home/user/Fotos/Urlaub/IMG_0001.dng")).toBeVisible();
  });
});
