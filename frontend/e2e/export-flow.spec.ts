import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = {
  id: "01977f4a-0000-7000-8000-000000000101",
  filename: "IMG_0001.CR3",
  width: 6000,
  height: 4000,
  missing: false,
};

/**
 * Deckt Phase 8 Schritt 1 ab (`DECISIONS.md` ADR-0034): das Export-
 * Exportdialog-Grundgerüst — Zielordner wählen, Format/Qualität/
 * Größenbegrenzung/Schärfung einstellen, exportieren. Die eigentliche
 * Render-/Kodierlogik ist bereits vollständig in `apx-export`s
 * Rust-Unit-Tests abgedeckt; hier nur die Frontend-Verdrahtung.
 */
test.describe("Export (Phase 8 Schritt 1)", () => {
  test("Exportieren-Knopf ist ohne Auswahl deaktiviert, mit Auswahl geht der Dialog auf", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
    });
    await page.goto("/");

    await expect(page.getByRole("button", { name: "Exportieren…" })).toBeDisabled();

    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();

    await expect(page.getByRole("button", { name: "Exportieren…" })).toBeEnabled();
    await page.getByRole("button", { name: "Exportieren…" }).click();
    await expect(page.getByText("1 Foto mit aktuellem Bearbeitungsstand")).toBeVisible();
  });

  test("Export mit gewähltem Zielordner ruft export_photo mit den passenden Optionen auf", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
      selectFolderResult: "/home/user/Exporte",
      exportPhotoOutcome: { path: "/home/user/Exporte/IMG_0001.jpg", width: 6000, height: 4000, byte_size: 987654 },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Exportieren…" }).click();

    await page.getByRole("button", { name: "Wählen…" }).click();

    await page.getByRole("radio").nth(1).check(); // "Längere Kante höchstens"
    await page.getByRole("spinbutton").first().fill("2048");

    await page.getByRole("button", { name: "Exportieren", exact: true }).click();

    await expect(page.getByText("1 Datei(en) geschrieben.")).toBeVisible();

    const log = await getMockInvokeLog(page);
    const call = log.find((e) => e.cmd === "export_photo");
    expect(call).toBeDefined();
    const args = call?.args as { photoId: string; destFolder: string; options: { format: string; maxEdge?: number } };
    expect(args.photoId).toBe(PHOTO.id);
    expect(args.destFolder).toBe("/home/user/Exporte");
    expect(args.options.format).toBe("jpeg");
    expect(args.options.maxEdge).toBe(2048);
  });

  test("Fehlgeschlagener Export zeigt eine Fehlermeldung statt den Dialog stillschweigend zu schließen", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
      selectFolderResult: "/home/user/Exporte",
      exportPhotoShouldFail: true,
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Exportieren…" }).click();
    await page.getByRole("button", { name: "Wählen…" }).click();

    await page.getByRole("button", { name: "Exportieren", exact: true }).click();

    await expect(page.getByText(/Fehler:/)).toBeVisible();
  });
});
