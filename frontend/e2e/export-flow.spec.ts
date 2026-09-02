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

async function setUpWithSelectedPhoto(page: import("@playwright/test").Page, extraFixtures: Record<string, unknown> = {}) {
  await installTauriMock(page, {
    folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
    photosByFolder: { [FOLDER_ID]: [PHOTO] },
    ...extraFixtures,
  });
  await page.goto("/");
  await page.getByRole("button", { name: /Urlaub/ }).click();
  await page.getByRole("img", { name: PHOTO.filename }).click();
}

/**
 * Deckt Phase 8 Schritt 1+2 ab (`DECISIONS.md` ADR-0034): das Export-
 * Exportdialog-Grundgerüst (Zielordner, Format/Qualität/Größenbegrenzung/
 * Schärfung) sowie die Export-Warteschlange (Fortschritt/Pausieren). Die
 * eigentliche Render-/Kodier-/ICC-/Wasserzeichen-/Metadaten-Logik ist
 * bereits vollständig in `apx-export`s Rust-Unit-Tests abgedeckt; hier
 * nur die Frontend-Verdrahtung.
 */
test.describe("Export (Phase 8 Schritt 1+2)", () => {
  test("Exportieren-Knopf ist ohne Auswahl deaktiviert, mit Auswahl geht der Dialog auf", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await expect(page.getByRole("button", { name: "Exportieren…" })).toBeEnabled();
    await page.getByRole("button", { name: "Exportieren…" }).click();
    await expect(page.getByText("1 Foto mit aktuellem Bearbeitungsstand")).toBeVisible();
  });

  test("Export ohne Auswahl ist deaktiviert", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
    });
    await page.goto("/");
    await expect(page.getByRole("button", { name: "Exportieren…" })).toBeDisabled();
  });

  test("Export mit gewähltem Zielordner reiht den Auftrag in die Warteschlange ein und zeigt den Abschluss", async ({ page }) => {
    await setUpWithSelectedPhoto(page, { selectFolderResult: "/home/user/Exporte" });
    await page.getByRole("button", { name: "Exportieren…" }).click();

    await page.getByRole("button", { name: "Wählen…" }).click();

    await page.getByRole("radio").nth(1).check(); // "Längere Kante höchstens"
    await page.getByRole("spinbutton").first().fill("2048");

    await page.getByRole("button", { name: "Exportieren", exact: true }).click();

    await expect(page.getByText("1 Datei(en) geschrieben.")).toBeVisible();

    const log = await getMockInvokeLog(page);
    const call = log.find((e) => e.cmd === "enqueue_export_photo");
    expect(call).toBeDefined();
    const args = call?.args as { photoId: string; destFolder: string; options: { format: string; maxEdge?: number } };
    expect(args.photoId).toBe(PHOTO.id);
    expect(args.destFolder).toBe("/home/user/Exporte");
    expect(args.options.format).toBe("jpeg");
    expect(args.options.maxEdge).toBe(2048);
  });

  test("Fehlgeschlagener Export zeigt die Fehlschlagszahl statt den Dialog stillschweigend zu schließen", async ({ page }) => {
    await setUpWithSelectedPhoto(page, { selectFolderResult: "/home/user/Exporte", exportPhotoShouldFail: true });
    await page.getByRole("button", { name: "Exportieren…" }).click();
    await page.getByRole("button", { name: "Wählen…" }).click();

    await page.getByRole("button", { name: "Exportieren", exact: true }).click();

    await expect(page.getByText(/1 fehlgeschlagen/)).toBeVisible();
  });

  test("Pausieren hält den Auftrag an, Fortsetzen schließt ihn ab", async ({ page }) => {
    await setUpWithSelectedPhoto(page, { selectFolderResult: "/home/user/Exporte", exportQueueStartsPaused: true });
    await page.getByRole("button", { name: "Exportieren…" }).click();
    await page.getByRole("button", { name: "Wählen…" }).click();

    await page.getByRole("button", { name: "Exportieren", exact: true }).click();

    const resumeButton = page.getByRole("button", { name: "Fortsetzen" });
    await expect(resumeButton).toBeVisible();
    await expect(page.getByText("0 / 1 exportiert")).toBeVisible();

    await resumeButton.click();

    await expect(page.getByText("1 Datei(en) geschrieben.")).toBeVisible();
  });

  test("ICC-Profil und Wasserzeichen-Optionen werden mit exportiert", async ({ page }) => {
    await setUpWithSelectedPhoto(page, {
      selectFolderResult: "/home/user/Exporte",
      pickFilePathResult: "/home/user/Schriften/Beispiel.ttf",
    });
    await page.getByRole("button", { name: "Exportieren…" }).click();
    await page.getByRole("button", { name: "Wählen…" }).click();

    await page.getByLabel("Farbraum (ICC)").selectOption("adobe_rgb");

    await page.getByLabel("Wasserzeichen-Art").selectOption("text");
    await page.getByPlaceholder("Text").fill("© Aperture X");
    await page.getByRole("button", { name: "Wählen…" }).nth(1).click();

    await page.getByRole("button", { name: "Exportieren", exact: true }).click();
    await expect(page.getByText("1 Datei(en) geschrieben.")).toBeVisible();

    const log = await getMockInvokeLog(page);
    const call = log.find((e) => e.cmd === "enqueue_export_photo");
    const args = call?.args as { options: { iccProfile?: string; watermarkText?: string; watermarkFontPath?: string } };
    expect(args.options.iccProfile).toBe("adobe_rgb");
    expect(args.options.watermarkText).toBe("© Aperture X");
    expect(args.options.watermarkFontPath).toBe("/home/user/Schriften/Beispiel.ttf");
  });

  test("PSD und JPEG-XL stehen als Formate zur Auswahl und werden mit exportiert", async ({ page }) => {
    // Deckt Phase 11 Schritt 2 ab (`DECISIONS.md` ADR-0038): die beiden
    // neuen Formate müssen in der Auswahl auftauchen und tatsächlich als
    // `options.format` beim Export-Aufruf ankommen. Der Qualitätsregler
    // (JXL: 100 = verlustfrei) ist bereits Rust-seitig in
    // `apx_export::format`s Unit-Tests abgedeckt — hier nur, dass das
    // Frontend den gewählten Wert überhaupt mitschickt.
    await setUpWithSelectedPhoto(page, { selectFolderResult: "/home/user/Exporte" });
    await page.getByRole("button", { name: "Exportieren…" }).click();
    await page.getByRole("button", { name: "Wählen…" }).click();

    const formatSelect = page.getByLabel("Format");
    await expect(formatSelect.locator("option", { hasText: "Photoshop (PSD)" })).toHaveCount(1);
    await expect(formatSelect.locator("option", { hasText: "JPEG XL" })).toHaveCount(1);

    await formatSelect.selectOption("jxl");
    await page.getByRole("button", { name: "Exportieren", exact: true }).click();
    await expect(page.getByText("1 Datei(en) geschrieben.")).toBeVisible();

    const log = await getMockInvokeLog(page);
    const call = log.find((e) => e.cmd === "enqueue_export_photo");
    const args = call?.args as { options: { format: string } };
    expect(args.options.format).toBe("jxl");
  });

  test("Mehrfachziel-Export reicht das Foto an jedes hinzugefügte Ziel weiter (Phase 12 Schritt 5)", async ({ page }) => {
    await setUpWithSelectedPhoto(page, { selectFolderResult: "/home/user/Exporte" });
    await page.getByRole("button", { name: "Exportieren…" }).click();
    await page.getByRole("button", { name: "Wählen…" }).click();

    // Erstes Ziel: Standard-Format (JPEG) im gewählten Ordner.
    await page.getByRole("button", { name: "+ Weiteres Ziel hinzufügen" }).click();

    // Zweites Ziel: PNG statt JPEG, derselbe Ordner (der Mock liefert
    // immer denselben Pfad zurück) — reicht, um zu zeigen, dass jedes
    // Ziel seine eigene Options-Momentaufnahme behält.
    await page.getByLabel("Format").selectOption("png");
    await page.getByRole("button", { name: "+ Weiteres Ziel hinzufügen" }).click();

    await expect(page.getByText("Alle 2 Ziele exportieren")).toBeVisible();
    await page.getByRole("button", { name: "Alle 2 Ziele exportieren" }).click();

    await expect(page.getByText("1 Datei(en) geschrieben.")).toBeVisible();

    const log = await getMockInvokeLog(page);
    const calls = log.filter((e) => e.cmd === "enqueue_export_photo");
    expect(calls).toHaveLength(2);
    const formats = calls.map((c) => (c.args as { options: { format: string } }).options.format);
    expect(formats.sort()).toEqual(["jpeg", "png"]);
  });
});
