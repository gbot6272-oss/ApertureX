import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000201";
const FOLDER_PATH = "/home/user/Fotos/Bibliothek";

interface PhotoFixture {
  id: string;
  filename: string;
  width: number | null;
  height: number | null;
  camera_make: string | null;
  camera_model: string | null;
  lens: string | null;
  iso: number | null;
  aperture: number | null;
  shutter: number | null;
  focal_length: number | null;
  captured_at: string | null;
  missing: boolean;
  rating: number;
  flag: number;
  color_label: string | null;
}

function samplePhoto(id: string, filename: string): PhotoFixture {
  return {
    id,
    filename,
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
    rating: 0,
    flag: 0,
    color_label: null,
  };
}

const PHOTO_A = samplePhoto("01977f4a-0000-7000-8000-000000000301", "IMG_0101.CR3");
const PHOTO_B = samplePhoto("01977f4a-0000-7000-8000-000000000302", "IMG_0102.CR3");

/**
 * Deckt Phase 3, Schritt 6 End-to-End ab: Raster anzeigen, ein Foto
 * bewerten (Raster-Widget), zu einer neu angelegten Sammlung hinzufügen,
 * und über die Filterleiste nach Bewertung filtern — Muster wie
 * `develop-flow.spec.ts`/`viewer-flow.spec.ts`.
 */
test.describe("Bibliothek: Raster, Bewertung, Sammlungen, Filter", () => {
  test("bewertet ein Foto im Raster, sammelt es und filtert danach danach", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2, parent_id: null, missing: false }],
      photosByFolder: { [FOLDER_ID]: [PHOTO_A, PHOTO_B] },
    });
    await page.goto("/");

    await page.getByRole("button", { name: /Bibliothek/ }).click();
    await page.getByRole("button", { name: "Raster" }).click();

    // Der Filmstreifen bleibt unabhängig vom Raster immer sichtbar (siehe
    // `App.tsx`) und zeigt dieselben Fotos — Bild-Abfragen müssen deshalb
    // auf das Raster (`<main>`, siehe `GridView.tsx`) beschränkt werden,
    // sonst träfe `getByRole("img", ...)` beide Stellen zugleich.
    const grid = page.locator("main");

    // Beide Fotos erscheinen als Kacheln im Raster.
    await expect(grid.getByRole("img", { name: PHOTO_A.filename })).toBeVisible();
    await expect(grid.getByRole("img", { name: PHOTO_B.filename })).toBeVisible();

    // Foto A auswählen (setzt den Fokus, Grundlage für Tastenkürzel und
    // "Auswahl hinzufügen") und mit 4 Sternen bewerten. Die Zelle trägt
    // exakt den Dateinamen als `title` (siehe `GridView.tsx`), das ist
    // hier zuverlässiger als eine Rollen-Namens-Abfrage über die ganze
    // Zelle (deren berechneter Accessible Name sonst Bild-Alt-Text und
    // alle verschachtelten Widget-Beschriftungen mit einschließen würde).
    const cellA = grid.getByTitle(PHOTO_A.filename, { exact: true });
    await cellA.click();
    await cellA.getByRole("button", { name: "4 Sterne" }).click();
    await expect(cellA.getByRole("button", { name: "4 Sterne" })).toHaveAttribute("aria-pressed", "true");

    // Neue Sammlung anlegen und Foto A hinzufügen.
    await page.getByRole("button", { name: "Neue Sammlung" }).click();
    await page.getByPlaceholder("Name der Sammlung…").fill("Favoriten");
    await page.getByPlaceholder("Name der Sammlung…").press("Enter");
    await expect(page.getByRole("button", { name: "Favoriten" })).toBeVisible();

    await page.getByRole("button", { name: "Auswahl zu dieser Sammlung hinzufügen" }).click();

    await page.getByRole("button", { name: "Favoriten" }).click();
    await expect(grid.getByRole("img", { name: PHOTO_A.filename })).toBeVisible();
    await expect(grid.getByRole("img", { name: PHOTO_B.filename })).not.toBeVisible();

    // Zurück zum Ordner, dann über die Filterleiste nach Bewertung 4+
    // filtern — nur Foto A (bewertet) darf übrig bleiben.
    await page.getByRole("button", { name: /Bibliothek/ }).click();
    await page.getByRole("button", { name: "4★+" }).click();
    await expect(grid.getByRole("img", { name: PHOTO_A.filename })).toBeVisible();
    await expect(grid.getByRole("img", { name: PHOTO_B.filename })).not.toBeVisible();

    await page.getByRole("button", { name: "Filter zurücksetzen" }).click();
    await expect(grid.getByRole("img", { name: PHOTO_B.filename })).toBeVisible();
  });
});

test.describe("Raster-Virtualisierung", () => {
  /** Regressionstest für `SPEC.md` §2.4 ("Bibliotheks-Raster mit 100.000
   * Bildern: flüssiges Scrollen, virtualisiert") und `PLAN.md` Phase 3
   * Schritt 7 — Muster wie der bestehende Filmstreifen-Virtualisierungs-
   * test in `viewer-flow.spec.ts`: 5.000 statt 100.000 synthetische Fotos,
   * damit der CI-Lauf schnell bleibt, ohne die eigentliche Aussage
   * (DOM-Knotenanzahl bleibt unabhängig von der Gesamtanzahl beschränkt)
   * zu verlieren — 100.000 wurden bei der Abnahme manuell verifiziert
   * (siehe Abschlussbericht im Chat).
   */
  test("rendert bei sehr vielen Fotos nur eine begrenzte Anzahl DOM-Knoten", async ({ page }) => {
    const total = 5000;
    const manyPhotos = Array.from({ length: total }, (_, i) => samplePhoto(`01977f4a-0000-7000-9000-${String(i).padStart(12, "0")}`, `IMG_${String(i).padStart(5, "0")}.CR3`));

    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: total, parent_id: null, missing: false }],
      photosByFolder: { [FOLDER_ID]: manyPhotos },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Bibliothek/ }).click();
    await page.getByRole("button", { name: "Raster" }).click();

    const grid = page.locator("main");
    const countBefore = await grid.locator('[role="button"]').count();
    expect(countBefore).toBeGreaterThan(0);
    expect(countBefore).toBeLessThan(200);

    // Ans Ende scrollen — die Anzahl gerenderter Zellen darf sich nicht an
    // die Gesamtanzahl annähern.
    await grid.locator("div.overflow-y-auto").evaluate((el) => {
      el.scrollTop = el.scrollHeight;
    });
    await expect
      .poll(async () => grid.locator('[role="button"]').count())
      .toBeLessThan(200);
  });
});
