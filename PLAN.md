# PLAN.md — Aperture X

Dieser Plan wird vor jeder Phase aktualisiert. Abgeschlossene Punkte bleiben angehakt stehen (Historie), neue Punkte kommen für die jeweils aktuelle Phase dazu. Die vollständige Phasenübersicht (Phase 1–10) steht in `SPEC.md`, Abschnitt 5 — hier steht nur der Arbeitsplan für die **aktuell offene** Phase im Detail.

---

## Abgeschlossene Phase: Phase 1 — Fundament

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

- [x] 4. `apx-catalog`
  - [x] `rusqlite` (bundled), WAL/synchronous/foreign_keys/busy_timeout je Connection (siehe ADR-0008: eine `Connection` hinter einem `Mutex` statt Pool)
  - [x] Migrationssystem über `user_version`, Migration 1 (`folders`, `photos`, `previews` + Indizes)
  - [x] Repository-Module pro Tabelle, kein SQL außerhalb von `apx-catalog`
  - [x] Mehrzeilige Schreibvorgänge in Transaktionen (`Catalog::transaction`)
  - [x] Tests: leere DB → Migration (inkl. Idempotenz, Ablehnung zu neuer Schema-Version), Round-Trip aller Repositories, doppelter Import (kein Duplikat, geänderte Datei aktualisiert statt dupliziert), FK-Kaskade, Neustart-Persistenz (Datei schließen/neu öffnen) — 26 Tests grün

- [x] 5. Tauri-Shell (`apx-app`)
  - [x] Tauri-2-Projekt (`tauri.conf.json`, `build.rs`, Capabilities, generiertes Aperture-Icon-Set), Verdrahtung von `apx-core`/`apx-raw`/`apx-catalog` über `AppState`, keine Geschäftslogik im Crate selbst
  - [x] Grundlegende Tauri-Commands: `select_folder` (nativer Ordnerdialog via `tauri-plugin-dialog`), `catalog_status` + `list_folders` (Katalog ist beim Start automatisch geladen/angelegt)
  - [x] Minimales Frontend-Gerüst (Vite + React 19 + TS), das die Commands aufruft — Smoke-Test für die IPC-Verdrahtung; vollständiges Layout folgt in Schritt 8
  - [x] Verifiziert: `cargo build -p apx-app` erfolgreich, Binary unter Xvfb gestartet — loggt korrekten Start, öffnet/migriert den Katalog, läuft stabil (kein Absturz)

- [x] 6. Import
  - [x] `ImportJob`: rekursiver Scan (`walkdir`), Endungsfilter, Duplikat-Skip via `(folder_id, filename, size, mtime)` (in `Catalog::upsert_photo`, geänderte Dateien aktualisieren statt zu duplizieren)
  - [x] Metadaten-Erfassung pro Datei → `photos`
  - [x] Thumbnail-Erzeugung im Worker-Pool (physische Kerne − 1 via `num_cpus`), bevorzugt eingebettete Vorschau, sonst (Half-Size-)Decode
  - [x] Fortschritts-Events (`import:progress`, `import:finished`, `import:error`) über eine testbare `ImportEvents`-Abstraktion (`TauriEvents` in der App, `RecordingEvents` in Tests)
  - [x] Abbruch via `CancellationToken`, Einzeldatei-Fehler sammeln statt Job abzubrechen
  - [x] Vorschau-Cache-Layout `<cache>/previews/<xx>/<id>_0.jpg` (Level 0 = Thumbnail)
  - [x] Tests: 3 gültige + 1 kaputte Datei → 3 importiert, 1 Fehler, Job „finished" (Akzeptanztest aus Abschnitt 8, mit synthetischen JPEGs statt echten RAWs — siehe ADR-0007), plus Idempotenz-Test für zweiten Import
  - [x] Nachträglich ergänzt (bei der Abnahme gegen Abschnitt-9-Akzeptanzkriterium 8 aufgefallen): `Catalog::set_photo_missing` existierte bereits, wurde aber nirgends aufgerufen — `crate::reconcile::reconcile_missing` gleicht jetzt beim Öffnen eines Ordners (`list_photos_in_folder`) den `missing`-Status jedes Fotos mit der tatsächlichen Dateisystem-Existenz ab (3 Tests: wird als missing markiert, Markierung verschwindet bei Wiederauftauchen, kein Absturz bei fehlendem Ordner) und wird im Filmstreifen (abgedunkelt + „fehlt"-Badge) und in der Viewer-Metadatenleiste („Datei fehlt") sichtbar gemacht — sonst wäre die DTO-Eigenschaft eine stillschweigend tote Fläche gewesen

- [x] 7. Custom-Protokoll-Handler
  - [x] `apx://preview/<id>/<level>` und `apx://image/<id>/<max_edge|'full'>` — Segment- statt Query-String-Format über `convertFileSrc`, siehe ADR-0009 (funktional identisch, plattformunabhängig)
  - [x] Korrekte `Content-Type` (JPEG für Preview, PNG für Vollbild — 16-Bit-Präzision erhalten) /`Cache-Control`, Dekodierung in einem eigenen OS-Thread pro Anfrage (`register_asynchronous_uri_scheme_protocol`)
  - [x] Deduplizierung gleichzeitiger Anfragen (Single-Flight-Cache über `Weak`-Referenzen, mit Nebenläufigkeits-Test)
  - [x] Abbruch laufender Dekodierung bei Bildwechsel — als Frontend-Verantwortung dokumentiert (`fetch`+`AbortController` statt `<img src>`), da echtes Abbrechen eines laufenden OS-Threads ohne kooperative Abbruchpunkte in `rawler` nicht möglich ist (siehe Modul-Doku in `protocol/mod.rs`)

- [x] 8. Frontend-Gerüst
  - [x] Vite + React 19 + TS, Zustand-Store (`catalog`, `selection`, `viewer`, `jobs`) mit Immer-Middleware
  - [x] Layout: Kopfzeile (Import-Button, Fortschritt, Abbrechen), linke Spalte (Ordnerbaum mit Fotoanzahl), Mitte (Viewer — zeigt bereits echte Vorschauen über den `apx://`-Handler), unten (Filmstreifen, noch nicht virtualisiert)
  - [x] Dark-Theme über Tailwind CSS 4 (`@theme`-Tokens `#1a1a1a`–`#242424`, keine Akzentfarben am Bildrand)
  - [x] Neuer Command `list_photos_in_folder` (Grundlage für Filmstreifen/Viewer)
  - [x] Grundlegende Tastenkürzel (←/→ Fotowechsel, Strg/Cmd+K Befehlspalette mit Ordnersuche + Kontext-Befehlen)
  - [x] Visuell verifiziert (Playwright-Screenshot gegen den Produktions-Build) und Tauri-Binary mit eingebettetem Frontend unter Xvfb stabil

- [x] 9. Viewer
  - [x] Canvas 2D + `ImageBitmap` über `fetch()`+`createImageBitmap()` (nicht `<img>`, ermöglicht `AbortController` — siehe ADR-0009); `.close()` beim Ersetzen/Unmount in `useImageBitmap`
  - [x] Zoom (Mausrad zum Cursor über `panForZoomAtCursor`, Stufen 1:1/2:1/4:1/8:1/16:1 inkl. Einpassen-Skalierung als Sprungpunkt, stufenlos per Mausrad dazwischen), Pan (Ziehen bei Zoom > Einpassen, Leertaste-Ziehen immer)
  - [x] Doppelklick Einpassen ↔ 1:1, `imageSmoothingEnabled = false` bei Zoom > 100 %
  - [x] Progressive Anzeige (Thumbnail sofort über `previewUrl(id,0)`, Vollbild über `imageUrl(id, an Containergröße×DPR angepasste Kantenlänge)`, kein Weißblitz — Geometrie bleibt stabil, da Bildmaße aus Katalog-Metadaten statt aus dem jeweils aktiven Bitmap kommen)
  - [x] Metadaten-Leiste unten rechts (Dateiname, Kamera/Objektiv, ISO/Blende/Zeit/Brennweite, Aufnahmedatum, Auflösung, aktueller Zoom)
  - [x] Tastenkürzel: ←/→ (App-Ebene), +/-/0/1/Leertaste (Viewer-Ebene, brauchen Container-/Bildmaße), F (Vollbild via Fullscreen-API), Strg/Cmd+K (Befehlspalette, aus Schritt 8)
  - [x] Visuell verifiziert (Playwright-Screenshot); reines Geometrie-Modul (`viewerMath.ts`) ohne DOM-Abhängigkeit für spätere Tests (Schritt 11)

- [x] 10. Filmstreifen
  - [x] `@tanstack/react-virtual`, flüssig bei 50.000 Einträgen — verifiziert per Playwright: 50.000 synthetische Fotos in den Store injiziert, DOM bleibt bei 22 gerenderten Zellen (vorher wie nach Scroll ans Ende), unabhängig von der Gesamtanzahl

- [x] 11. Tests
  - [x] Rust: `apx-raw` (29 Tests), `apx-catalog` (26 Tests, inkl. `open_on_disk_persists_across_reopen` für „Neustart → Katalog persistent"), `apx-core` (14 Tests), `apx-app`/Import-Job (26 Tests, inkl. `import_run_handles_three_valid_and_one_broken_file` und `import_run_is_idempotent_on_second_pass`) — zusammen 95 Tests, alle grün (`cargo test --workspace`)
  - [x] Vitest (`pnpm test`): 18 Tests für die reinen Geometrie-/Format-Module `viewerMath.ts` und `format.ts` (`jsdom`-Umgebung, kein DOM-Rendering nötig)
  - [x] Playwright-E2E (`pnpm test:e2e`, `frontend/e2e/`): Start → Ordner importieren (simulierte `import:progress`/`import:finished`-Events) → Thumbnails im Filmstreifen → Foto anklicken → Viewer zeigt Metadaten → Taste „1" → Zoom exakt 100 % — plus ein Test für `import:error` in der Fehlerleiste und ein Regressionstest für die Filmstreifen-Virtualisierung (5.000 synthetische Fotos, DOM-Knotenanzahl bleibt vor/nach Scroll klein; die 50.000er-Zahl aus Schritt 10 wurde dort bereits manuell verifiziert, hier bewusst kleiner für schnelle CI-Läufe). „Neustart → Katalog persistent" ist laut ADR-0010 kein Playwright-Fall (kein echter App-Neustart ohne `tauri-driver`), sondern durch die Rust-Persistenz-Tests oben abgedeckt.
  - [x] Eigene, wiederverwendbare `window.__TAURI_INTERNALS__`-Simulation (`frontend/e2e/tauri-mock.ts`) statt der echten nativen App — siehe ADR-0010 für die Begründung und die bewusst offen gelassene Lücke (echtes natives Klick-E2E bräuchte `tauri-driver` + WebdriverIO)

- [x] 12. CI (`.github/workflows/ci.yml`)
  - [x] Job „frontend" (nur Ubuntu, plattformunabhängig): `pnpm test` (Vitest), `pnpm build`, `pnpm exec playwright install --with-deps chromium`, `pnpm test:e2e`; HTML-Report als Artefakt bei Fehlschlag
  - [x] Job „rust" als Matrix Windows/macOS/Linux: Tauri-Systemabhängigkeiten unter Linux (`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev` u. a., siehe die Tauri-2-Voraussetzungen), Rust-Toolchain automatisch aus `rust-toolchain.toml` (1.94.1 + rustfmt/clippy) über `rustup show`, `Swatinem/rust-cache`
  - [x] `cargo fmt --all -- --check` (einmal, Linux), `cargo clippy --workspace --all-targets --all-features -- -D warnings` (alle drei Plattformen — lokal auf Linux verifiziert: keine Warnungen), `cargo test --workspace` (95 Tests), `cargo build --workspace` — Frontend wird vorher über `pnpm build` erzeugt, da `apx-app`s `build.rs` `frontend/dist` einbettet (`tauri.conf.json`s `frontendDist`), unabhängig vom `tauri build`-CLI-Hook
  - [x] Bewusst kein volles `tauri build` mit Installer-Paketen/Signierung — das ist Phase 10 (Distribution), nicht Phase 1

### Nicht in Phase 1 (bewusst zurückgestellt)
Alles aus Phase 2–10 der `SPEC.md`: GPU-Pipeline, Regler/Entwickeln-Modul, Presets/Templates, Masken, KI-Funktionen, Export, Druck/Buch/Web/Karte, Node-Editor, Stacking, Tethering, Skript-API, Politur. Keine Vorarbeiten dafür in Phase 1, auch keine „vorbereitenden" Abstraktionen darüber hinaus, was Phase 1 selbst braucht.

**Lizenzentscheidung `rawler`/LGPL-2.1 (ADR-0002) ist vom Nutzer bestätigt** — kein offener Punkt mehr.

---

## Aktuelle Phase: Phase 2 — Pipeline-Kern

Ziel (laut `SPEC.md` §5): wgpu-Setup, Shader-Framework, Farbmanagement, EDL-Datenmodell, die sieben Grundeinstellungs-Regler (Weißabgleich, Belichtung, Kontrast, Lichter, Tiefen, Weiß, Schwarz), Tile-Cache, Verlauf mit Undo/Redo. Ergebnis: interaktives Entwickeln.

Anders als Phase 1 gibt es kein eigenes, ausführliches Prompt-Dokument für Phase 2 — `SPEC.md` §5 fasst den Umfang in einem Satz zusammen. Die folgende Schrittliste ist deshalb selbst erarbeitet (vollständige Herleitung samt Architektur-Entscheidungen in `DECISIONS.md` ADR-0011 bis ADR-0018, Zusammenfassung in `ARCHITECTURE.md` §5).

**Wichtige Scope-Korrektur (ADR-0011):** `FEATURES.md` hatte fälschlich zwölf Regler als Phase 2 markiert; `SPEC.md`s Phasenplan nennt nur sieben. Textur/Klarheit/Dunst entfernen/Dynamik/Sättigung sind jetzt korrekt als Phase 4 markiert.

### Reihenfolge

- [x] 0. Scope festzurren, Dokumente vorbereiten
  - [x] `FEATURES.md`: fünf Regler-Zeilen von Phase 2 auf Phase 4 korrigiert (ADR-0011)
  - [x] `DECISIONS.md`: ADR-0011 bis ADR-0018 (Scope, Ein-Crate-Entscheidung, EDL-Format, Verlauf-Modell, apx-raw-Grenze, Transportformat, Shader-Strategie, Undo/Redo-Bibliothek)
  - [x] `ARCHITECTURE.md`: §5-Platzhalter durch echte Modulbeschreibung ersetzt, Grobstruktur-Diagramm und Modulgrenzen-Regeln um `apx-pipeline` ergänzt
  - [x] Nebenbei behoben: `DECISIONS.md` ADR-0002s Status war noch als "wartet auf Bestätigung" markiert, obwohl der Nutzer die LGPL-2.1-Ausnahme längst bestätigt hatte — korrigiert

- [x] 1. `apx-pipeline` Crate-Grundgerüst
  - [x] `Cargo.toml` (wgpu 22, bytemuck, pollster, lcms2 + Workspace-Deps), Workspace-Einbindung (`members`, `workspace.dependencies`), `#![deny(clippy::unwrap_used)]`
  - [x] Modul-Skelett (`edl/`, `color/`, `gpu/`, `stages/`, `tile_cache.rs`) — je mit Doku-Kommentar, der auf den füllenden Schritt verweist, noch ohne Inhalt
  - [x] `PipelineError` (thiserror, gleiche Form wie `apx_core::AppError`) mit `From<PipelineError> for AppError`
  - [x] Additive `AppError::Pipeline`-Variante in `apx-core` + Konstruktor `AppError::pipeline()` + Test
  - [x] `THIRD_PARTY.md`: Lizenzen für `wgpu`/`bytemuck`/`pollster`/`lcms2` per `cargo metadata` verifiziert (alle MIT/Apache-2.0/Zlib, keine GPL-Ausnahme nötig) und eingetragen
  - [x] Verifiziert: `cargo check --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used`, `cargo fmt --all -- --check`, `cargo test --workspace` (101 Tests) — alles grün

- [ ] 2. EDL-Datenmodell + Katalog-Migration
  - [ ] `EdlV1` in `apx-pipeline::edl`, `EdlEnvelope` in `apx-core`
  - [ ] `migrations/0002_edits.sql` (`edit_history`, `edit_current`), `repository::edits`
  - [ ] Tests: Roundtrip, Schema-Version-Ablehnung, FK-Cascade, Migrations-Idempotenz, alter Katalog öffnet noch

- [ ] 3. wgpu-Gerätekontext (`apx-pipeline::gpu`)
  - [ ] `GpuContext`, `Backends::all()` + `force_fallback_adapter`-Fallback
  - [ ] Gemeinsamer Dispatch-Helfer, Kopier-Shader-Roundtrip-Test

- [ ] 4. Die 7 Regler: WGSL-Shader + CPU-Fallback
  - [ ] 5 Regler-Module + fusionierter Shader
  - [ ] `apx_raw::decode_linear()` additiver Einstiegspunkt
  - [ ] Tests pro Modul (Hand-Erwartungswert, CPU/GPU-Abgleich, Identität, Fallback, Fusion-Abgleich)

- [ ] 5. Tauri-Anbindung: Command + Protokoll-Route
  - [ ] `AppState.pipeline`, `ImageRequest::Develop`, `apply_develop_edit`-Command
  - [ ] RGBA8-Rohbytes-Transport (ADR-0016)
  - [ ] Tests: Routen-Parsing, `compute_develop`-Integrationstest, Cache-Schlüssel

- [ ] 6. Frontend: Entwickeln-Regler, Undo/Redo, WebGL2-Viewer
  - [ ] `DevelopSlice` + `zundo`, `DevelopPanel.tsx` mit 7 Reglern
  - [ ] `Viewer.tsx` auf WebGL2, `useDevelopRender`-Hook
  - [ ] Tests: Vitest (Regler-Logik), Store (Undo/Redo), Playwright-E2E-Erweiterung

- [ ] 7. Performance-Feinschliff fürs 16-ms-Ziel
  - [ ] Entprellung, Latenzmessung (`tracing`-Spans + Frontend-Zeitstempel), ehrliche Dokumentation der Zahlen

- [ ] 8. Testinfrastruktur: synthetische Daten + wgpu in CI
  - [ ] Gemeinsamer Fixture-Helfer
  - [ ] CI: Mesa `llvmpipe`/`lavapipe` (Linux), WARP (Windows), Metal-Zugriff (macOS) empirisch verifizieren

- [ ] 9. Dokumentation fertigstellen
  - [ ] `THIRD_PARTY.md`, `ARCHITECTURE.md`-Datenfluss-Abschnitt, `FEATURES.md` abhaken

- [ ] 10. Abnahme gegen Phase-2-Kriterien
  - [ ] Definition-of-Done je Feature, EDL-Neustart-Persistenz-Test, Performance-Zahlen, Abschlussbericht

### Nicht in Phase 2 (bewusst zurückgestellt)
Gradationskurve, HSL, Farbmischer, Color Grading, Details/Schärfen/Rauschen, Objektivkorrekturen, Effekte, Kalibrierung, Geometrie/Crop, Reparatur (alle → Phase 4), Presets (→ Phase 5), Masken (→ Phase 6), sowie die fünf per ADR-0011 nach Phase 4 verschobenen Regler (Textur, Klarheit, Dunst entfernen, Dynamik, Sättigung).

### Bekannte offene Punkte aus Phase 1 (unverändert)
- ADR-0007: keine echten RAW-Testdateien (Netzwerkzugriff auf raw.pixls.us blockiert) — betrifft auch Phase 2s Shader-Tests, die deshalb weiterhin auf synthetische Testmuster angewiesen sind.
- ADR-0010: Playwright testet simuliert, nicht die native App.
