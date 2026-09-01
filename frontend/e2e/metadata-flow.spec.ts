import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO_A = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false };

/**
 * Deckt Phase 9 Schritt 2 ab (`PLAN.md`, `DECISIONS.md` ADR-0035): den
 * Metadaten-Dialog. Die XMP-Erzeugung/-Parsing-Logik selbst ist bereits
 * vollständig in `apx-export`s Rust-Unit-Tests abgedeckt (`xmp.rs`) — hier
 * bewusst nur drei Frontend-Flows, je einer der drei Reiter stellvertretend.
 */
test.describe("Metadaten (Phase 9 Schritt 2)", () => {
  test("IPTC-Felder bearbeiten und speichern übernimmt den Titel", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO_A] } });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO_A.filename }).click();

    await page.getByRole("button", { name: "Metadaten…" }).click();
    await page.getByRole("button", { name: "Metadaten & XMP" }).click();
    await page.getByLabel("Titel").fill("Sonnenuntergang");
    await page.getByRole("button", { name: "Metadaten speichern" }).click();

    await expect(page.getByLabel("Titel")).toHaveValue("Sonnenuntergang");
  });

  test("Neue Tag-Regel erscheint in der Liste", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO_A] } });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO_A.filename }).click();

    // Schlagwort zuerst über die bestehende Phase-3-Zuweisung anlegen,
    // damit es als Ziel für eine Tag-Regel wählbar ist — vor dem Öffnen
    // des Dialogs, damit dessen `refreshKeywords()`-Effekt es mitlädt.
    await page.evaluate(async (photoId) => {
      await (window as unknown as { __TAURI_INTERNALS__: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> } }).__TAURI_INTERNALS__.invoke(
        "add_photo_keyword",
        { photoId, name: "Berge" },
      );
    }, PHOTO_A.id);

    await page.getByRole("button", { name: "Metadaten…" }).click();
    await page.getByRole("button", { name: "Tag-Regeln" }).click();
    await page.locator("select").filter({ has: page.locator("option", { hasText: "Ziel-Schlagwort" }) }).selectOption({ label: "Berge" });
    await page.getByPlaceholder("Name der Regel").fill("Berge im Objektiv");
    await page.getByRole("button", { name: "Regel anlegen" }).click();

    await expect(page.getByText("Berge im Objektiv")).toBeVisible();
  });

  test("XMP-Sidecar exportieren zeigt den Ziel-Pfad an", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO_A] },
      exportedXmpSidecarPath: "/home/user/Fotos/Urlaub/IMG_0001.xmp",
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO_A.filename }).click();
    await page.getByRole("button", { name: "Metadaten…" }).click();
    await page.getByRole("button", { name: "Metadaten & XMP" }).click();

    await page.getByRole("button", { name: ".xmp exportieren" }).click();

    await expect(page.getByText("/home/user/Fotos/Urlaub/IMG_0001.xmp")).toBeVisible();
  });
});
