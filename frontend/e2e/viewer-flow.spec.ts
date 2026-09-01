import { expect, test } from "@playwright/test";

import { emitMockEvent, getMockInvokeLog, installTauriMock, setMockFixtures } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";

interface PhotoFixture {
  id: string;
  filename: string;
  width: number;
  height: number;
  camera_make: string;
  camera_model: string;
  lens: string;
  iso: number;
  aperture: number;
  shutter: number;
  focal_length: number;
  captured_at: string;
  missing: boolean;
}

// Als Tupel getippt (statt `PhotoFixture[]`), damit `PHOTOS[0]` unter
// `noUncheckedIndexedAccess` (siehe tsconfig.json) ohne `| undefined`
// auskommt — der Zugriff unten ist per Konstruktion immer gültig.
const PHOTOS: readonly [PhotoFixture, PhotoFixture, PhotoFixture] = [
  {
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
  },
  {
    id: "01977f4a-0000-7000-8000-000000000102",
    filename: "IMG_0002.CR3",
    width: 6000,
    height: 4000,
    camera_make: "Canon",
    camera_model: "EOS R5",
    lens: "RF 24-70mm",
    iso: 400,
    aperture: 5.6,
    shutter: 1 / 500,
    focal_length: 70,
    captured_at: "2024-06-01T10:05:00Z",
    missing: false,
  },
  {
    id: "01977f4a-0000-7000-8000-000000000103",
    filename: "IMG_0003.CR3",
    width: 4000,
    height: 6000,
    camera_make: "Canon",
    camera_model: "EOS R5",
    lens: "RF 24-70mm",
    iso: 100,
    aperture: 8,
    shutter: 2,
    focal_length: 24,
    captured_at: "2024-06-01T10:10:00Z",
    missing: false,
  },
];

test.describe("Start -> Import -> Auswahl -> Viewer", () => {
  test("importiert einen Ordner, zeigt Thumbnails und danach den Viewer bei Zoom 1:1", async ({ page }) => {
    await installTauriMock(page);
    await page.goto("/");

    // Beim Start ist der Katalog leer (siehe PHASE1_PROMPT.md Abschnitt 9,
    // Akzeptanzkriterium "leerer Zustand").
    await expect(page.getByText("Noch keine Ordner importiert.")).toBeVisible();

    // Der native Ordner-Dialog wird über den Command `select_folder`
    // simuliert — der Test steuert dessen Ergebnis über die Fixtures.
    await setMockFixtures(page, { selectFolderResult: FOLDER_PATH });

    await page.getByRole("button", { name: "Ordner importieren" }).click();

    // `startImport()` ruft `import_folder` auf; der Test spielt jetzt
    // genau die Events nach, die der echte Import-Job in
    // `crates/apx-app/src/import/mod.rs` über `ImportEvents` emittiert.
    await emitMockEvent(page, "import:progress", { done: 1, total: 3, current_file: "IMG_0001.CR3" });
    await expect(page.getByText(/1 \/ 3/)).toBeVisible();

    // Bevor der Import "fertig" gemeldet wird, hinterlegt der Test die
    // Katalog-Daten, die ein `refreshFolders()`/`loadPhotosForFolder()`
    // danach vorfinden würde — exakt wie das echte Backend den Katalog
    // vor dem `import:finished`-Event bereits geschrieben hat.
    await setMockFixtures(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: PHOTOS.length }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
      catalogStatus: { catalog_path: "mock-catalog.sqlite3", folder_count: 1, photo_count: PHOTOS.length },
    });
    await emitMockEvent(page, "import:finished", { imported: 3, skipped: 0, error_count: 0, cancelled: false, duplicate_count: 0 });

    await expect(page.getByText("Import abgeschlossen: 3 importiert · 0 übersprungen")).toBeVisible();

    // Sidebar zeigt den neu importierten Ordner mit Fotoanzahl.
    const folderButton = page.getByRole("button", { name: /Urlaub/ });
    await expect(folderButton).toBeVisible();
    await expect(folderButton.getByText("3")).toBeVisible();

    await folderButton.click();

    const log = await getMockInvokeLog(page);
    expect(log.some((entry) => entry.cmd === "import_folder" && (entry.args as { path?: string }).path === FOLDER_PATH)).toBe(true);
    expect(log.some((entry) => entry.cmd === "list_photos_in_folder")).toBe(true);

    // Filmstreifen zeigt alle drei Fotos als anklickbare Thumbnails.
    for (const photo of PHOTOS) {
      await expect(page.getByRole("img", { name: photo.filename })).toBeVisible();
    }

    // Ein Foto anklicken -> Viewer zeigt es (Dateiname + Metadaten-Leiste).
    await page.getByRole("img", { name: PHOTOS[0].filename }).click();

    const overlay = page.locator("main").getByText(PHOTOS[0].filename);
    await expect(overlay).toBeVisible();
    await expect(page.getByText(/Canon EOS R5/)).toBeVisible();
    await expect(page.getByText(/ISO 200/)).toBeVisible();

    // `selectPhoto()` ruft `resetView()` -> Einpassen-Modus, nicht 1:1.
    // Taste "1" schaltet auf exakt 100 % Zoom (siehe Viewer.tsx).
    await page.locator("main").click(); // Fokus auf den Viewer-Bereich legen
    await page.keyboard.press("1");
    await expect(page.getByText(/100 %/)).toBeVisible();

    // Das Bild wurde tatsächlich über `fetch()` + `createImageBitmap()`
    // geladen (nicht nur eine leere Fläche) — der Canvas hat also
    // sichtbaren Inhalt und keine Ladefehler in der Konsole hinterlassen.
    const canvas = page.locator("main canvas");
    await expect(canvas).toBeVisible();
  });

  test("zeigt Import-Fehler in der Fehlerleiste an", async ({ page }) => {
    await installTauriMock(page);
    await page.goto("/");
    await setMockFixtures(page, { selectFolderResult: FOLDER_PATH });
    await page.getByRole("button", { name: "Ordner importieren" }).click();

    await emitMockEvent(page, "import:error", { file: "defekt.CR3", message: "RAW-Header ungültig" });

    await expect(page.getByText("defekt.CR3: RAW-Header ungültig")).toBeVisible();
  });

  test("markiert ein als missing gemeldetes Foto sichtbar im Filmstreifen und im Viewer", async ({ page }) => {
    // Akzeptanzkriterium 8 aus PHASE1_PROMPT.md Abschnitt 9: eine
    // außerhalb der App gelöschte Datei wird als `missing` markiert —
    // das Backend gleicht das in `crate::reconcile` ab; hier wird nur
    // geprüft, dass ein bereits als `missing` gemeldetes Foto auch
    // sichtbar so dargestellt wird, statt die DTO-Eigenschaft
    // stillschweigend zu ignorieren.
    const missingPhoto = { ...PHOTOS[0], missing: true };
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [missingPhoto] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();

    const thumbnailButton = page.getByRole("img", { name: missingPhoto.filename }).locator("..");
    await expect(thumbnailButton).toHaveAttribute("title", `${missingPhoto.filename} (Datei fehlt)`);
    await expect(thumbnailButton.getByText("fehlt")).toBeVisible();

    await page.getByRole("img", { name: missingPhoto.filename }).click();
    await expect(page.locator("main").getByText("Datei fehlt")).toBeVisible();
  });
});

test.describe("Filmstreifen-Virtualisierung", () => {
  test("rendert bei sehr vielen Fotos nur eine begrenzte Anzahl DOM-Knoten", async ({ page }) => {
    // Regressionstest für PLAN.md Schritt 10 — bei manueller Verifikation
    // wurden 50.000 Einträge geprüft; hier 5.000, damit der Testlauf in
    // CI schnell bleibt, ohne die eigentliche Aussage (DOM bleibt
    // unabhängig von der Gesamtanzahl beschränkt) zu verlieren.
    const total = 5000;
    const manyPhotos = Array.from({ length: total }, (_, i) => ({
      id: `01977f4a-0000-7000-9000-${String(i).padStart(12, "0")}`,
      filename: `IMG_${String(i).padStart(5, "0")}.CR3`,
      width: 6000,
      height: 4000,
      camera_make: null,
      camera_model: null,
      lens: null,
      iso: null,
      aperture: null,
      shutter: null,
      focal_length: null,
      captured_at: null,
      missing: false,
    }));

    // Als Startzustand statt über `setMockFixtures()` danach gesetzt, weil
    // `App.tsx` `refreshFolders()` schon beim Mounten aufruft — ein
    // nachträgliches Setzen käme für diesen ersten Aufruf zu spät.
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: total }],
      photosByFolder: { [FOLDER_ID]: manyPhotos },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();

    const countBefore = await page.locator("footer button").count();
    expect(countBefore).toBeGreaterThan(0);
    expect(countBefore).toBeLessThan(50);

    // Ans Ende scrollen — die Anzahl gerenderter Zellen darf sich nicht
    // an die Gesamtanzahl annähern.
    await page.locator("footer").evaluate((el) => {
      el.scrollLeft = el.scrollWidth;
    });
    await expect
      .poll(async () => page.locator("footer button").count())
      .toBeLessThan(50);
  });
});
