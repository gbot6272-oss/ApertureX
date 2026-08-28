# PLAN.md — Aperture X

Dieser Plan wird vor jeder Phase aktualisiert. Abgeschlossene Punkte bleiben angehakt stehen (Historie), neue Punkte kommen für die jeweils aktuelle Phase dazu. Die vollständige Phasenübersicht (Phase 1–10) steht in `SPEC.md`, Abschnitt 5 — hier steht nur der Arbeitsplan für die **aktuell offene** Phase im Detail.

---

## Aktuelle Phase: Phase 1 — Fundament

Ziel (siehe `PHASE1_PROMPT.md`): App starten, Ordner mit RAWs importieren, Bilder in einer Liste sehen, eines auswählen, flüssig mit Zoom/Pan betrachten, nach Neustart ist alles noch da. Keine GPU-Pipeline, keine Regler, keine Bearbeitung, keine Presets, kein Export.

### Reihenfolge (verbindlich laut Prompt)

- [x] 1. Workspace-Grundgerüst
  - [x] `Cargo.toml` (Workspace-Root), `rust-toolchain.toml`
  - [x] Verzeichnisse `crates/apx-core`, `crates/apx-raw`, `crates/apx-catalog`, `crates/apx-app`, `frontend/`, `testdata/`
  - [x] Abhängigkeitsrichtung erzwingen: `apx-core` ohne Workspace-Abhängigkeiten; `apx-raw` und `apx-catalog` je nur gegen `apx-core`; `apx-app` gegen alle drei — verifiziert per `cargo check --workspace` (kompiliert), Grenzen zusätzlich in `ARCHITECTURE.md` §4 dokumentiert
  - [x] `.gitignore` (Rust-Target, Node-Modules, DB-Dateien, Cache)
  - [x] `clippy.toml` (erlaubt `unwrap()`/`expect()` in Testcode, siehe ADR-0006)

- [x] 2. `apx-core`
  - [x] `PhotoId` / `FolderId` / `CatalogId` als UUIDv7-Newtypes
  - [x] `AppError` (`thiserror`): `Io`, `Decode`, `Database`, `NotFound`, `Unsupported`, `Cancelled` (+ `InvalidId`, `Settings`)
  - [x] `AppPaths` (`directories`-Crate): Katalog-, Cache-, Log-, Settings-Pfade je Plattform (`discover()`), plus `rooted_at()` für Tests/portablen Modus
  - [x] `Settings` (Serde + TOML), Defaults bei fehlender Datei
  - [x] Logging-Setup (`tracing` + `tracing-subscriber`, Datei-Rotation + stdout im Debug-Build)
  - [x] Unit-Tests (14 Tests, alle grün; `cargo fmt --check` und `cargo clippy -D warnings -D clippy::unwrap_used` sauber)

- [x] 3. `apx-raw`
  - [x] Abhängigkeit auf `rawler` (Lizenz-Flag siehe `DECISIONS.md` ADR-0002)
  - [x] `RawMetadata`-Struct, `read_metadata()` (nutzt `dummy=true`-Dekodierung von `rawler`, überspringt die teure Pixel-Dekompression)
  - [x] `extract_embedded_preview()` (Thumbnail zuerst, Preview als Fallback; für JPEG/PNG/TIFF `None`, da dort `decode()` bereits günstig ist)
  - [x] `decode()` mit minimaler, dokumentiert-provisorischer Kette (Schwarzpunkt → Demosaic bilinear/half-size → WB → Kamera-RGB→sRGB-Matrix → Gamma → 16-bit RGB), siehe `pipeline`-Modul
  - [x] EXIF-Orientierung korrekt und genau einmal angewendet (eigener `Orientation`-Typ, Anwendung ausschließlich in `apx-raw`, dokumentiert gegen Doppelanwendung im Frontend)
  - [x] Formate: CR2, CR3, NEF, ARW, RAF, ORF, RW2, DNG, JPEG/PNG/TIFF-Fallback über `image`
  - [x] Unit-Tests (39 Tests: Orientierung, Metadaten-Parsing inkl. Zeitzonen-Fallstrick, Demosaic voll/half-size/Bayer/generisch, Farbmatrix/Gamma, Crop, Downsampling)
  - [ ] ⚠️ **Blockiert:** Echte Testdaten in `testdata/` (CC0, z. B. raw.pixls.us) für Golden-Image-Tests je Format — **Netzwerkzugriff auf raw.pixls.us ist in dieser Sandbox-Umgebung von der Egress-Policy blockiert** (verifiziert per `curl`/`WebFetch`, HTTP-403/EGRESS_BLOCKED). Die Algorithmen selbst sind über synthetische Unit-Tests abgedeckt; echte Kameradateien pro Format fehlen noch. Siehe `DECISIONS.md` ADR-0007 für Optionen.
  - [ ] Golden-Image-Tests gegen echte Testdateien (mittlere Abweichung < 1/255) — wartet auf obigen Punkt

- [ ] 4. `apx-catalog`
  - [ ] `rusqlite` (bundled), WAL/synchronous/foreign_keys/busy_timeout je Connection
  - [ ] Migrationssystem über `user_version`, Migration 1 (`folders`, `photos`, `previews` + Indizes)
  - [ ] Repository-Module pro Tabelle, kein SQL außerhalb von `apx-catalog`
  - [ ] Mehrzeilige Schreibvorgänge in Transaktionen
  - [ ] Tests: leere DB → Migration, Round-Trip, doppelter Import, FK-Kaskade

- [ ] 5. Tauri-Shell (`apx-app`)
  - [ ] Tauri-2-Projekt, Verdrahtung von `apx-core`/`apx-raw`/`apx-catalog`, keine Geschäftslogik im Crate selbst
  - [ ] Grundlegende Tauri-Commands (Ordner öffnen, Katalog laden/anlegen)

- [ ] 6. Import
  - [ ] `ImportJob`: rekursiver Scan (`walkdir`), Endungsfilter, Duplikat-Skip via `(folder_id, filename, size, mtime)`
  - [ ] Metadaten-Erfassung pro Datei → `photos`
  - [ ] Thumbnail-Erzeugung im Worker-Pool (Kerne − 1), bevorzugt eingebettete Vorschau, sonst Half-Size-Decode
  - [ ] Fortschritts-Events (`import:progress`, `import:finished`, `import:error`)
  - [ ] Abbruch via `CancellationToken`, Einzeldatei-Fehler sammeln statt Job abzubrechen
  - [ ] Vorschau-Cache-Layout `<cache>/previews/<xx>/<id>_<level>.jpg`
  - [ ] Tests: 3 gültige + 1 kaputte Datei → 3 importiert, 1 Fehler, Job „finished"

- [ ] 7. Custom-Protokoll-Handler
  - [ ] `apx://preview/<id>?level=` und `apx://image/<id>?max_edge=`
  - [ ] Korrekte `Content-Type`/`Cache-Control`, Dekodierung in `spawn_blocking`
  - [ ] Deduplizierung gleichzeitiger Anfragen
  - [ ] Abbruch laufender Dekodierung bei Bildwechsel

- [ ] 8. Frontend-Gerüst
  - [ ] Vite + React 19 + TS, Zustand-Store (`catalog`, `selection`, `viewer`, `jobs`)
  - [ ] Layout: Kopfzeile (Import-Button, Fortschritt), linke Spalte (Ordnerbaum), Mitte (Viewer), unten (Filmstreifen)
  - [ ] Dark-Theme (`#1a1a1a`–`#242424`, keine Akzentfarben am Bildrand)

- [ ] 9. Viewer
  - [ ] Canvas 2D + `ImageBitmap`, `.close()` beim Verdrängen aus dem Cache
  - [ ] Zoom (Mausrad zum Cursor, Stufen inkl. stufenlos), Pan (Drag / Leertaste)
  - [ ] Doppelklick Einpassen ↔ 1:1, `imageSmoothingEnabled = false` bei Zoom > 100 %
  - [ ] Progressive Anzeige (Thumbnail → Vollbild, kein Weißblitz)
  - [ ] Metadaten-Leiste unten rechts
  - [ ] Tastenkürzel: ←/→, +/-, 0, 1, F, Strg/Cmd+K (Grundgerüst Befehlspalette)

- [ ] 10. Filmstreifen
  - [ ] `@tanstack/react-virtual`, flüssig bei 50.000 Einträgen

- [ ] 11. Tests
  - [ ] `apx-raw`, `apx-catalog`, Import-Job (siehe oben)
  - [ ] Playwright-E2E: Start → Import → Thumbnails → Auswahl → Viewer → Zoom 1:1 → Neustart → Katalog persistent

- [ ] 12. CI
  - [ ] GitHub Actions Matrix Windows/macOS/Linux
  - [ ] `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `pnpm test`, Build

### Offene Entscheidung vor „go" (siehe `DECISIONS.md` ADR-0002)
`rawler` (und jede realistische Alternative für vollständige RAW-Formatunterstützung in Rust) ist **LGPL-2.1** lizenziert. Das kollidiert wörtlich mit der Regel „nichts mit GPL im Kern, außer du weist mich ausdrücklich darauf hin" aus `SPEC.md` Abschnitt 6. Hiermit ausdrücklich darauf hingewiesen — Entscheidung und Kompromiss stehen in `DECISIONS.md`, ich warte auf explizite Bestätigung.

### Nicht in Phase 1 (bewusst zurückgestellt)
Alles aus Phase 2–10 der `SPEC.md`: GPU-Pipeline, Regler/Entwickeln-Modul, Presets/Templates, Masken, KI-Funktionen, Export, Druck/Buch/Web/Karte, Node-Editor, Stacking, Tethering, Skript-API, Politur. Keine Vorarbeiten dafür in Phase 1, auch keine „vorbereitenden" Abstraktionen darüber hinaus, was Phase 1 selbst braucht.
