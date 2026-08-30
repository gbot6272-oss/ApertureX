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
 * Deckt Phase 8 Schritt 3 ab (`DECISIONS.md` ADR-0034): das Druckdialog-
 * Grundgerüst (Layoutwahl, Raster-/Bilderpaket-Optionen, Seitenmaße,
 * Speichern-unter-Dialog, `print_photos`-Aufruf). Die eigentliche
 * Seitenkomposition (`compose_page`, Slot-Geometrie je Layout) ist bereits
 * vollständig in `apx-export`s Rust-Unit-Tests abgedeckt; hier nur die
 * Frontend-Verdrahtung.
 */
test.describe("Drucken (Phase 8 Schritt 3)", () => {
  test("Drucken-Knopf ist ohne Auswahl deaktiviert, mit Auswahl geht der Dialog auf", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await expect(page.getByRole("button", { name: "Drucken…" })).toBeEnabled();
    await page.getByRole("button", { name: "Drucken…" }).click();
    await expect(page.getByText("1 Foto — wird als druckfertige JPEG-Seite gespeichert")).toBeVisible();
  });

  test("Drucken ohne Auswahl ist deaktiviert", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
    });
    await page.goto("/");
    await expect(page.getByRole("button", { name: "Drucken…" })).toBeDisabled();
  });

  test("Abbruch des Speichern-unter-Dialogs löst keinen print_photos-Aufruf aus", async ({ page }) => {
    await setUpWithSelectedPhoto(page, { pickSaveFilePathResult: null });
    await page.getByRole("button", { name: "Drucken…" }).click();
    await page.getByRole("button", { name: "Als JPEG speichern" }).click();

    const log = await getMockInvokeLog(page);
    expect(log.find((e) => e.cmd === "print_photos")).toBeUndefined();
  });

  test("Einzelbild-Layout druckt mit gewählten Seitenmaßen und zeigt den Zielpfad", async ({ page }) => {
    await setUpWithSelectedPhoto(page, { pickSaveFilePathResult: "/home/user/Drucke/Seite.jpg" });
    await page.getByRole("button", { name: "Drucken…" }).click();

    await page.getByRole("spinbutton").first().fill("6"); // Breite (Zoll)

    await page.getByRole("button", { name: "Als JPEG speichern" }).click();
    await expect(page.getByText("Gespeichert: /mock/print/Druckseite.jpg")).toBeVisible();

    const log = await getMockInvokeLog(page);
    const call = log.find((e) => e.cmd === "print_photos");
    expect(call).toBeDefined();
    const args = call?.args as { photoIds: string[]; destPath: string; options: { layout: string; pageWidthIn: number } };
    expect(args.photoIds).toEqual([PHOTO.id]);
    expect(args.destPath).toBe("/home/user/Drucke/Seite.jpg");
    expect(args.options.layout).toBe("single");
    expect(args.options.pageWidthIn).toBe(6);
  });

  test("Kontaktbogen-Layout zeigt Spalten/Zeilen und schickt sie mit", async ({ page }) => {
    await setUpWithSelectedPhoto(page, { pickSaveFilePathResult: "/home/user/Drucke/Kontaktbogen.jpg" });
    await page.getByRole("button", { name: "Drucken…" }).click();

    await page.getByLabel("Layout").selectOption("contact_sheet");
    await expect(page.getByText("Spalten")).toBeVisible();

    await page.getByRole("button", { name: "Als JPEG speichern" }).click();

    const log = await getMockInvokeLog(page);
    const call = log.find((e) => e.cmd === "print_photos");
    const args = call?.args as { options: { layout: string; cols?: number; rows?: number } };
    expect(args.options.layout).toBe("contact_sheet");
    expect(args.options.cols).toBe(2); // Vorgabewert
    expect(args.options.rows).toBe(2); // Vorgabewert
  });

  test("Bilderpaket-Layout zeigt die Vorlagenauswahl und schickt das gewählte Template mit", async ({ page }) => {
    await setUpWithSelectedPhoto(page, { pickSaveFilePathResult: "/home/user/Drucke/Paket.jpg" });
    await page.getByRole("button", { name: "Drucken…" }).click();

    await page.getByLabel("Layout").selectOption("picture_package");
    await page.getByLabel("Vorlage").selectOption("eight_wallet");

    await page.getByRole("button", { name: "Als JPEG speichern" }).click();

    const log = await getMockInvokeLog(page);
    const call = log.find((e) => e.cmd === "print_photos");
    const args = call?.args as { options: { layout: string; picturePackageTemplate?: string } };
    expect(args.options.layout).toBe("picture_package");
    expect(args.options.picturePackageTemplate).toBe("eight_wallet");
  });

  test("ICC-Profil, Zoom-Modus und Druckschärfung werden mit übergeben", async ({ page }) => {
    await setUpWithSelectedPhoto(page, { pickSaveFilePathResult: "/home/user/Drucke/Seite.jpg" });
    await page.getByRole("button", { name: "Drucken…" }).click();

    await page.getByLabel("Zoom").selectOption("cover");
    await page.getByLabel("Farbraum (ICC)").selectOption("pro_photo_rgb");

    await page.getByRole("button", { name: "Als JPEG speichern" }).click();

    const log = await getMockInvokeLog(page);
    const call = log.find((e) => e.cmd === "print_photos");
    const args = call?.args as { options: { fit: string; iccProfile?: string; sharpenAmount?: number } };
    expect(args.options.fit).toBe("cover");
    expect(args.options.iccProfile).toBe("pro_photo_rgb");
    expect(args.options.sharpenAmount).toBe(0.5); // Vorgabewert des Schiebereglers
  });

  test("Fehlgeschlagenes Drucken zeigt die Fehlermeldung", async ({ page }) => {
    await setUpWithSelectedPhoto(page, {
      pickSaveFilePathResult: "/home/user/Drucke/Seite.jpg",
      printPhotoShouldFail: true,
    });
    await page.getByRole("button", { name: "Drucken…" }).click();
    await page.getByRole("button", { name: "Als JPEG speichern" }).click();

    await expect(page.getByText(/Fehler: Test-Stub: Drucken fehlgeschlagen/)).toBeVisible();
  });
});
