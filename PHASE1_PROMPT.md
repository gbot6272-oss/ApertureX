# Claude Code Prompt — Aperture X, Phase 1: Fundament

> Das hier ist der Prompt, den du **zuerst** in Claude Code einfügst. Der Gesamtprompt bleibt als `SPEC.md` im Repo liegen — lege ihn vorher dort ab.

---

## Kontext

Wir bauen **Aperture X**, einen nicht-destruktiven RAW-Editor. Die vollständige Zielspezifikation liegt in `SPEC.md` — lies sie einmal, aber implementiere in dieser Session **ausschließlich Phase 1**. Nichts aus späteren Phasen vorziehen, auch nicht „schon mal vorbereiten".

**Ziel von Phase 1, in einem Satz:** Ich kann die App starten, einen Ordner mit RAW-Dateien importieren, die Bilder in einer Liste sehen, eines auswählen und es flüssig mit Zoom und Pan betrachten — und nach einem Neustart ist alles noch da.

Was Phase 1 **nicht** enthält: keine GPU-Pipeline, keine Regler, keine Bearbeitung, keine Presets, kein Export, keine Module außer der einen Ansicht. Farbverarbeitung ist bewusst provisorisch und wird in Phase 2 ersetzt.

---

## 1. Repo- und Workspace-Struktur

Lege exakt diese Struktur an:

```
/
├── Cargo.toml                  # Workspace-Root
├── SPEC.md                     # Gesamtspezifikation (liegt schon vor)
├── PLAN.md  ARCHITECTURE.md  DECISIONS.md  FEATURES.md
├── THIRD_PARTY.md
├── rust-toolchain.toml         # feste Toolchain-Version
├── crates/
│   ├── apx-core/               # Basistypen, IDs, Fehler, Konfiguration
│   ├── apx-raw/                # RAW-Dekodierung, Metadaten, Thumbnails
│   ├── apx-catalog/            # SQLite, Migrationen, Repositories
│   └── apx-app/               # Tauri-Binary, Commands, IPC, Protokoll-Handler
├── frontend/                   # React 19 + TS + Vite
└── testdata/                   # kleine RAWs für Tests (Lizenz beachten!)
```

Regeln:
- `apx-core` hängt von **nichts** aus dem Workspace ab. `apx-raw` und `apx-catalog` hängen nur von `apx-core` ab, **nicht** voneinander. `apx-app` kennt alle drei.
- Keine Geschäftslogik in `apx-app` — das ist reine Verdrahtung.
- Frontend spricht ausschließlich über Tauri-Commands und einen Custom-Protokoll-Handler mit dem Backend. Kein direkter Dateisystemzugriff aus dem Frontend.

---

## 2. `apx-core`

Enthält:
- `PhotoId`, `FolderId`, `CatalogId` als typisierte Newtypes über UUIDv7 (zeitlich sortierbar).
- `AppError` mit `thiserror`, Varianten mindestens: `Io`, `Decode`, `Database`, `NotFound`, `Unsupported`, `Cancelled`. Alle Crates geben `Result<T, AppError>` zurück.
- `AppPaths`: plattformkorrekte Pfade für Katalog, Cache, Logs, Einstellungen (`directories`-Crate). Auf Windows unter `%APPDATA%`, macOS `~/Library/Application Support`, Linux XDG.
- `Settings`-Struct mit Serde, Laden/Speichern als TOML, Defaults bei fehlender Datei.
- Logging-Setup: `tracing` + `tracing-subscriber`, Ausgabe in Datei mit Rotation **und** stdout im Debug-Build.

---

## 3. `apx-raw` — Dekodierung

**Bibliothek:** `rawler`. Falls ein benötigtes Format fehlt, ergänze `kamadak-exif` für Metadaten und dokumentiere die Lücke in `DECISIONS.md`. Baue **kein** eigenes RAW-Parsing.

**Unterstützte Formate in Phase 1:** CR2, CR3, NEF, ARW, RAF, ORF, RW2, DNG, plus JPEG/PNG/TIFF als Fallback über `image`.

**API des Crates:**

```rust
pub struct RawMetadata {
    pub width: u32, pub height: u32,
    pub camera_make: String, pub camera_model: String,
    pub lens: Option<String>,
    pub iso: Option<u32>, pub shutter: Option<f32>,
    pub aperture: Option<f32>, pub focal_length: Option<f32>,
    pub captured_at: Option<OffsetDateTime>,
    pub orientation: Orientation,
    pub gps: Option<(f64, f64)>,
}

/// Nur Metadaten lesen — muss < 50 ms pro Datei brauchen, ohne Bilddaten zu dekodieren.
pub fn read_metadata(path: &Path) -> Result<RawMetadata>;

/// Eingebettete JPEG-Vorschau extrahieren, falls vorhanden. Der schnelle Weg für den Import.
pub fn extract_embedded_preview(path: &Path) -> Result<Option<DynamicImage>>;

/// Volle Dekodierung. `max_edge` = None für volle Auflösung.
pub fn decode(path: &Path, max_edge: Option<u32>) -> Result<DecodedImage>;
```

**Dekodierungs-Kette für Phase 1** — bewusst minimal und dokumentiert als provisorisch:
1. CFA-Daten laden.
2. Schwarzpunkt abziehen, auf Weißpunkt normalisieren.
3. Demosaicing: **bilinear**, plus einen „half-size"-Modus (2×2-Block → ein Pixel), der für Vorschauen benutzt wird.
4. Kamera-Weißabgleich aus den `as shot`-Multiplikatoren anwenden.
5. Kamera-RGB → sRGB über die Farbmatrix aus den Metadaten.
6. sRGB-Gammakurve.
7. Ausgabe als 16-bit RGB.

Setze über jede Stufe einen Kommentar mit dem Hinweis, dass sie in Phase 2 durch eine GPU-Stufe ersetzt wird. **Kein** Rauschfilter, **keine** Schärfung, **keine** Objektivkorrektur.

**EXIF-Orientierung** wird beim Dekodieren angewendet — das ist eine klassische Fehlerquelle. Test dafür ist Pflicht.

---

## 4. `apx-catalog` — Datenbank

**SQLite über `rusqlite` mit Bundled-Feature.** Begründung: `sqlx` erzwingt eine erreichbare Datenbank oder eine gepflegte `.sqlx`-Offline-Cache-Datei zur Compile-Zeit, was in dieser Projektphase permanent Reibung erzeugt. Wenn du `sqlx` trotzdem willst, begründe es in `DECISIONS.md` und richte `SQLX_OFFLINE` sauber ein — halbe Lösungen nicht.

Konfiguration bei jedem Verbindungsaufbau:
```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous  = NORMAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
```

**Migrationen** als nummerierte SQL-Dateien in `crates/apx-catalog/migrations/`, angewendet über `user_version`. Migration 1 legt an:

```sql
CREATE TABLE folders (
    id            TEXT PRIMARY KEY,
    path          TEXT NOT NULL UNIQUE,
    parent_id     TEXT REFERENCES folders(id) ON DELETE CASCADE,
    added_at      INTEGER NOT NULL
);

CREATE TABLE photos (
    id            TEXT PRIMARY KEY,
    folder_id     TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    filename      TEXT NOT NULL,
    file_size     INTEGER NOT NULL,
    file_mtime    INTEGER NOT NULL,
    content_hash  TEXT,                    -- xxHash der ersten+letzten 1 MB + Größe
    width         INTEGER, height INTEGER,
    orientation   INTEGER NOT NULL DEFAULT 1,
    camera_make   TEXT, camera_model TEXT, lens TEXT,
    iso           INTEGER, shutter REAL, aperture REAL, focal_length REAL,
    captured_at   INTEGER,                 -- Unix-Sekunden, UTC
    gps_lat       REAL, gps_lon REAL,
    imported_at   INTEGER NOT NULL,
    missing       INTEGER NOT NULL DEFAULT 0,
    UNIQUE (folder_id, filename)
);

CREATE INDEX idx_photos_folder   ON photos(folder_id);
CREATE INDEX idx_photos_captured ON photos(captured_at);
CREATE INDEX idx_photos_hash     ON photos(content_hash);

CREATE TABLE previews (
    photo_id      TEXT NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
    level         INTEGER NOT NULL,        -- 0=Thumb 256, 1=Standard 2048, 2=1:1
    path          TEXT NOT NULL,           -- Datei im Cache-Verzeichnis
    generated_at  INTEGER NOT NULL,
    PRIMARY KEY (photo_id, level)
);
```

Schreibe pro Tabelle ein Repository-Modul mit typisierten Funktionen. Kein SQL außerhalb von `apx-catalog`. Alle Schreibvorgänge, die mehr als eine Zeile betreffen, laufen in einer Transaktion.

---

## 5. Import

Ein `ImportJob` in `apx-app`:
1. Ordner rekursiv scannen (`walkdir`), nach unterstützten Endungen filtern.
2. Bereits bekannte Dateien anhand `(folder_id, filename, file_size, file_mtime)` überspringen.
3. Pro Datei: `read_metadata` → Zeile in `photos` schreiben.
4. Danach in einem Worker-Pool (`rayon` oder `tokio::task::spawn_blocking`, Parallelität = Anzahl physischer Kerne minus 1) Thumbnails (Level 0, 256 px lange Kante) erzeugen — bevorzugt aus der eingebetteten Vorschau, sonst per Half-Size-Dekodierung.
5. Fortschritt über Tauri-Events ans Frontend: `import:progress { done, total, current_file }`, `import:finished`, `import:error { file, message }`.
6. Abbrechbar über ein `CancellationToken`. Ein Fehler bei einer einzelnen Datei bricht den Job **nicht** ab, sondern wird gesammelt und am Ende gemeldet.

Vorschau-Cache: `<cache_dir>/previews/<erste 2 Zeichen der ID>/<id>_<level>.jpg`. Die Unterordner-Aufteilung ist wichtig, damit keine Verzeichnisse mit 100.000 Dateien entstehen.

---

## 6. Bildübertragung ans Frontend

**Nicht** über Tauri-Commands mit Base64 — das bläht den IPC auf und ruckelt. Registriere stattdessen einen **Custom-Protokoll-Handler**:

```
apx://preview/<photo_id>?level=0
apx://image/<photo_id>?max_edge=2560
```

Der Handler antwortet mit JPEG (Vorschauen) bzw. PNG oder rohem RGBA (Vollbild), setzt `Content-Type` und `Cache-Control` korrekt. Das Frontend nutzt die URL direkt in `<img>` bzw. lädt sie per `createImageBitmap`. Anfragen werden dedupliziert: zwei gleichzeitige Anfragen für dasselbe Bild lösen nur eine Dekodierung aus.

Beim Bildwechsel wird eine noch laufende Dekodierung des vorherigen Bildes abgebrochen.

---

## 7. Frontend

**Aufbau (bewusst schmal in Phase 1):**
- Kopfzeile: App-Name, Button „Ordner importieren", Fortschrittsanzeige.
- Linke Spalte: Ordnerbaum, Anzahl Fotos pro Ordner.
- Mitte: Viewer.
- Unten: Filmstreifen mit Thumbnails, virtualisiert (`@tanstack/react-virtual`) — muss mit 50.000 Einträgen flüssig scrollen.

**Viewer:**
- `<canvas>` mit 2D-Kontext, `ImageBitmap` als Quelle. Kein WebGL in Phase 1 — das kommt in Phase 2 zusammen mit der echten Pipeline. Vermerke das im Code.
- Zoom: Mausrad zum Cursor hin, Stufen „Einpassen", „Füllen", 1:1, 2:1, 4:1, 8:1, 16:1 und stufenlos dazwischen.
- Pan: Ziehen mit linker Maustaste bei Zoom > Einpassen, Leertaste-Ziehen immer.
- Doppelklick wechselt zwischen Einpassen und 1:1.
- Progressive Anzeige: sofort das Thumbnail hochskaliert zeigen, dann die höhere Auflösung einblenden, sobald sie da ist. Kein Weißblitz beim Bildwechsel.
- Bei Zoom > 100 % `imageSmoothingEnabled = false`, damit Pixel scharf bleiben.
- Rechts unten eine kleine Metadaten-Leiste: Dateiname, Kamera, Objektiv, ISO, Blende, Zeit, Brennweite, Aufnahmedatum, Auflösung.

**Tastenkürzel schon jetzt:** Pfeil links/rechts = Bild wechseln, `+`/`-` = Zoom, `0` = Einpassen, `1` = 1:1, `F` = Vollbild, `Strg/Cmd+K` = Befehlspalette (Grundgerüst, findet Ordner und Befehle).

**State:** Zustand-Store mit den Slices `catalog`, `selection`, `viewer`, `jobs`. Kein React-Context für häufig ändernde Werte.

**Theme:** dunkel, neutrale Grautöne, keine Farbstiche im Bereich rund um den Viewer — das Bild wird sonst falsch beurteilt. Umgebungsfläche `#1a1a1a` bis `#242424`, keine Akzentfarben direkt am Bildrand.

---

## 8. Tests

- `apx-raw`: pro Format eine Datei in `testdata/`, Test auf korrekte Dimensionen, Kameramodell, Orientierung. Golden-Image-Test: dekodiertes Bild gegen abgelegtes PNG, mittlere absolute Abweichung < 1/255.
- `apx-catalog`: Migration von leerer DB, Round-Trip aller Repositories, Verhalten bei doppeltem Import, Fremdschlüssel-Kaskade.
- Import: Ordner mit 3 gültigen und 1 kaputten Datei → 3 Fotos importiert, 1 Fehler gemeldet, Job abgeschlossen.
- E2E (Playwright): App starten → Ordner importieren → Thumbnails erscheinen → Bild anklicken → Viewer zeigt es → Zoom 1:1 → Neustart → Katalog ist noch da.
- CI: GitHub Actions, Matrix Windows/macOS/Linux, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `pnpm test`, Build.

`testdata/` nur mit RAWs, die frei lizenziert sind (z. B. raw.pixls.us, CC0). Herkunft und Lizenz in `THIRD_PARTY.md`.

---

## 9. Akzeptanzkriterien für Phase 1

Ich hake Phase 1 ab, wenn all das stimmt:

1. `pnpm tauri dev` startet auf allen drei Plattformen ohne Warnung.
2. Import von 500 RAWs dauert inkl. Thumbnails < 90 Sekunden, die UI bleibt dabei bedienbar und der Fortschritt läuft sichtbar.
3. Abbruch während des Imports funktioniert innerhalb von 2 Sekunden und hinterlässt einen konsistenten Katalog.
4. Bildwechsel im Filmstreifen: Thumbnail sofort, Vollbild < 800 ms bei 24 MP.
5. Zoom und Pan laufen bei 60 fps, kein Ruckeln beim Ziehen.
6. Filmstreifen mit 50.000 Thumbnails scrollt flüssig, Speicher bleibt unter 1,5 GB.
7. Ein zweiter Import desselben Ordners fügt null Duplikate hinzu.
8. Wird eine Datei außerhalb der App gelöscht, markiert die App sie beim nächsten Öffnen als `missing` und stürzt nicht ab.
9. Kein `unwrap()` außerhalb von Tests, `clippy` ohne Warnungen.
10. `FEATURES.md` für Phase 1 vollständig abgehakt, `DECISIONS.md` enthält mindestens die Entscheidungen zu Datenbank-Crate, RAW-Crate, Bildübertragung und Viewer-Technik.

---

## 10. Bekannte Fallstricke — bitte gleich richtig machen

- **EXIF-Orientierung** doppelt angewendet (einmal beim Dekodieren, einmal im Canvas) → Bilder stehen kopf. Genau einmal anwenden, Test dafür.
- **Farbmatrix ignoriert** → alles wirkt grünstichig. Die `as shot`-Multiplikatoren allein reichen nicht.
- **Base64 über IPC** → GB-weise Speicher und Ruckeln. Deshalb der Protokoll-Handler.
- **Blockierende Dekodierung im Tauri-Command** → eingefrorene UI. Alles Rechenintensive in `spawn_blocking`.
- **SQLite-Schreibzugriffe parallel** → `database is locked`. Ein einziger Writer, Leser über einen Pool.
- **`ImageBitmap` nicht freigegeben** → Speicherleck im Browser-Kontext. `.close()` beim Verdrängen aus dem Cache.
- **Zeitzonen**: EXIF-Aufnahmezeiten haben oft keine Zone. Speichere die lokale Zeit als solche und dokumentiere die Annahme, statt still UTC anzunehmen.

---

## Startbefehl

Lies `SPEC.md`. Aktualisiere dann `PLAN.md` mit einer Aufgabenliste für Phase 1 und lege `ARCHITECTURE.md`, `DECISIONS.md`, `FEATURES.md` und `THIRD_PARTY.md` an. Zeig mir die Aufgabenliste und die vier Kernentscheidungen aus Punkt 9.10 — dann warte auf mein „go", bevor du Code schreibst.

Danach arbeitest du die Aufgabenliste in dieser Reihenfolge ab: Workspace → `apx-core` → `apx-raw` → `apx-catalog` → Tauri-Shell → Import → Protokoll-Handler → Frontend-Gerüst → Viewer → Filmstreifen → Tests → CI. Nach jedem dieser Schritte ein Commit und eine Zeile Statusmeldung an mich.
