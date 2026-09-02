import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

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

/**
 * Phase 11 Schritt 11 (siehe DECISIONS.md ADR-0038): `DevelopPanel.tsx`s
 * eigener Ctrl/Cmd+Z-Handler lief bis Phase 10 fest verdrahtet — jetzt über
 * `lib/keybindings.ts`s `matchesBinding("undo"/"redo")`, dieselben IDs wie
 * `App.tsx`s Bibliotheks-Metadaten-Undo (beide Kontexte schließen sich
 * gegenseitig aus). Dieser Test deckt zwei Dinge in einem Lauf ab (siehe
 * die vom Nutzer ab Schritt 4 gelockerte Testdisziplin — max. 1 Test pro
 * Schritt): (1) dass Ctrl+Z/Ctrl+Shift+Z im geöffneten Entwickeln-Panel
 * tatsächlich über den Tastatur-Pfad committen (die bestehende
 * develop-flow.spec.ts-Suite deckt bislang nur den Rückgängig-Knopf ab,
 * nicht die Taste), und (2) dass eine Neu-Belegung über das
 * Cheatsheet-Overlay diesen Pfad tatsächlich umsteuert. Nutzt bewusst
 * Redo (unverändert an "mod+shift+z" gebunden) statt eines zweiten
 * Regler-Commits, um den Ausgangszustand vor der Neu-Belegung
 * wiederherzustellen — ein zweiter `fill()`-Commit direkt nach einem
 * frischen Undo wäre ein eigenes Wettrennen mit dessen asynchroner
 * Backend-Antwort, unabhängig von der hier getesteten Umbelegung.
 */
test("Entwickeln-Panel: Ctrl+Z/Ctrl+Shift+Z committen per Tastatur, eine Neu-Belegung im Cheatsheet steuert Ctrl+Z um", async ({ page }) => {
  await installTauriMock(page, {
    folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
    photosByFolder: { [FOLDER_ID]: [PHOTO] },
  });
  await page.goto("/");
  await page.getByRole("button", { name: /Urlaub/ }).click();
  await page.getByRole("img", { name: PHOTO.filename }).click();
  await page.getByRole("button", { name: "Entwickeln" }).click();

  const exposureInput = page.getByRole("spinbutton", { name: "Belichtung (Zahlenwert)" });
  await exposureInput.fill("2");
  await exposureInput.blur();
  await expect
    .poll(async () => {
      const log = await getMockInvokeLog(page);
      return log.some((entry) => entry.cmd === "apply_develop_edit");
    })
    .toBe(true);
  await expect(exposureInput).toHaveValue("2");

  // Standardbelegung Ctrl/Cmd+Z (Playwright: "Control+z") — noch nicht
  // umbelegt — committet über den Tastatur-Pfad, nicht den Knopf.
  await page.getByRole("heading", { name: "Entwickeln" }).click();
  await page.keyboard.press("Control+z");
  await expect(exposureInput).toHaveValue("0");

  // Ctrl/Cmd+Shift+Z (Redo, unverändert) stellt "2" wieder her — ohne
  // einen zweiten Regler-Commit, der mit der Undo-Antwort um den
  // Anwendungszustand konkurrieren würde.
  await page.keyboard.press("Control+Shift+z");
  await expect(exposureInput).toHaveValue("2");

  // "undo" im Cheatsheet-Overlay auf die bloße Taste "u" umbelegen (ohne
  // Modifikator — vermeidet jede Mehrdeutigkeit mit nativen Browser-
  // Tastenkombinationen wie Strg+U "Quelltext anzeigen").
  await page.keyboard.press("?");
  const cheatsheet = page.getByRole("dialog", { name: "Tastenkürzel" });
  await expect(cheatsheet).toBeVisible();
  const undoRow = cheatsheet.getByText("Rückgängig (Bibliotheks-Metadaten", { exact: false }).locator("..");
  await undoRow.getByRole("button", { name: "Strg/Cmd + Z" }).click();
  // `locator.press` fokussiert das Element zuverlässig selbst, bevor es die
  // Taste sendet — vermeidet ein Wettrennen mit React `autoFocus`, das ein
  // bloßes `page.keyboard.press` hier hätte.
  await undoRow.locator("input").press("u");
  await expect(undoRow.getByRole("button", { name: "U" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(cheatsheet).toBeHidden();
  await expect(exposureInput).toHaveValue("2");

  // Die alte Taste Ctrl+Z wirkt jetzt nicht mehr.
  await page.getByRole("heading", { name: "Entwickeln" }).click();
  await page.keyboard.press("Control+z");
  await expect(exposureInput).toHaveValue("2");

  // Die neue Taste "u" committet den Rückgängig-Schritt.
  await page.keyboard.press("u");
  await expect(exposureInput).toHaveValue("0");
});
