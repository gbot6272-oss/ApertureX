import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000201";
const FOLDER_PATH = "/home/user/Fotos/Bibliothek";

interface PhotoFixture {
  id: string;
  filename: string;
  file_size: number;
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
  /** Nur für die Duplikaterkennung (Schritt 8.2) relevant — zwei Fotos mit
   * demselben `content_hash` gelten als Duplikat. */
  content_hash?: string | null;
}

function samplePhoto(id: string, filename: string): PhotoFixture {
  return {
    id,
    filename,
    file_size: 12_000_000,
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
    content_hash: null,
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

    // Rückgängig/Wiederholen für Bibliotheks-Metadaten (Schritt 8.1,
    // `DECISIONS.md` ADR-0027): Strg/Cmd+Z macht die Bewertung rückgängig,
    // Strg/Cmd+Umschalt+Z stellt sie wieder her.
    await page.keyboard.press("ControlOrMeta+z");
    await expect(cellA.getByRole("button", { name: "4 Sterne" })).toHaveAttribute("aria-pressed", "false");
    await page.keyboard.press("ControlOrMeta+Shift+z");
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

  /** Belegt Schritt 8.4 (`DECISIONS.md` ADR-0027): Suchtext und
   * Bewertungsfilter wirken jetzt kombiniert (per UND) statt sich
   * gegenseitig zu ersetzen. */
  test("kombiniert Suchtext und Bewertungsfilter", async ({ page }) => {
    const strandHigh = samplePhoto("01977f4a-0000-7000-8000-000000000401", "Strand_Sonnenuntergang.CR3");
    const strandLow = samplePhoto("01977f4a-0000-7000-8000-000000000402", "Strand_Mittag.CR3");
    const bergHigh = samplePhoto("01977f4a-0000-7000-8000-000000000403", "Berg_Gipfel.CR3");
    strandHigh.rating = 5;
    strandLow.rating = 1;
    bergHigh.rating = 5;

    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 3, parent_id: null, missing: false }],
      photosByFolder: { [FOLDER_ID]: [strandHigh, strandLow, bergHigh] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Bibliothek/ }).click();
    await page.getByRole("button", { name: "Raster" }).click();
    const grid = page.locator("main");

    const searchInput = page.getByPlaceholder("Suche (Dateiname, Kamera, Objektiv)…");
    await searchInput.fill("Strand");
    await searchInput.press("Enter");
    await expect(grid.getByRole("img", { name: strandHigh.filename })).toBeVisible();
    await expect(grid.getByRole("img", { name: strandLow.filename })).toBeVisible();
    await expect(grid.getByRole("img", { name: bergHigh.filename })).not.toBeVisible();

    // Zusätzlich nach Bewertung 4+ filtern, während der Suchtext gesetzt
    // bleibt — übrig bleibt nur das eine Foto, das *beide* Kriterien
    // erfüllt (früher hätte das Setzen des Filters die Suche gelöscht).
    await page.getByRole("button", { name: "4★+" }).click();
    await expect(grid.getByRole("img", { name: strandHigh.filename })).toBeVisible();
    await expect(grid.getByRole("img", { name: strandLow.filename })).not.toBeVisible();
    await expect(grid.getByRole("img", { name: bergHigh.filename })).not.toBeVisible();
    await expect(searchInput).toHaveValue("Strand");
  });

  /** Belegt Schritt 8.2 (`DECISIONS.md` ADR-0027): Fotos mit identischem
   * `content_hash` werden über "Duplikate anzeigen" gefunden. */
  test("zeigt Duplikate anhand des Inhalts-Hashes an", async ({ page }) => {
    const original = samplePhoto("01977f4a-0000-7000-8000-000000000501", "original.CR3");
    const kopie = samplePhoto("01977f4a-0000-7000-8000-000000000502", "kopie.CR3");
    const einzelstueck = samplePhoto("01977f4a-0000-7000-8000-000000000503", "einzelstueck.CR3");
    original.content_hash = "gleicherhash";
    kopie.content_hash = "gleicherhash";
    einzelstueck.content_hash = "andererhash";

    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 3, parent_id: null, missing: false }],
      photosByFolder: { [FOLDER_ID]: [original, kopie, einzelstueck] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Bibliothek/ }).click();
    await page.getByRole("button", { name: "Raster" }).click();
    const grid = page.locator("main");

    await page.getByRole("button", { name: "Duplikate anzeigen" }).click();
    await expect(grid.getByRole("img", { name: original.filename })).toBeVisible();
    await expect(grid.getByRole("img", { name: kopie.filename })).toBeVisible();
    await expect(grid.getByRole("img", { name: einzelstueck.filename })).not.toBeVisible();
  });

  /** Belegt Schritt 8.3 (`DECISIONS.md` ADR-0027): Sortierung nach
   * beliebigem Feld, hier Dateigröße statt des Standardfelds Dateiname. */
  test("sortiert das Raster nach Dateigröße", async ({ page }) => {
    const small = samplePhoto("01977f4a-0000-7000-8000-000000000601", "b_klein.CR3");
    const big = samplePhoto("01977f4a-0000-7000-8000-000000000602", "a_gross.CR3");
    small.file_size = 1_000;
    big.file_size = 90_000_000;

    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2, parent_id: null, missing: false }],
      photosByFolder: { [FOLDER_ID]: [small, big] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Bibliothek/ }).click();
    await page.getByRole("button", { name: "Raster" }).click();
    const grid = page.locator("main");

    // Standard-Sortierung (Dateiname aufsteigend): "a_gross" vor "b_klein".
    await expect(grid.locator("[title]").first()).toHaveAttribute("title", "a_gross.CR3");

    await page.getByLabel("Sortieren nach").selectOption("file_size");
    // Nach Dateigröße aufsteigend steht "b_klein" (1000 Byte) jetzt vor
    // "a_gross" (90 MB).
    await expect(grid.locator("[title]").first()).toHaveAttribute("title", "b_klein.CR3");
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
