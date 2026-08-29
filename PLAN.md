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

- [x] 2. EDL-Datenmodell + Katalog-Migration
  - [x] `EdlEnvelope` in `apx-core` (schema_version + `serde_json::Value`-Payload, `from_json_str`/`to_json_string`)
  - [x] `EdlV1`/`BasicAdjustments`/`WhiteBalanceAdjustment` in `apx-pipeline::edl::v1` — Weißabgleich als Verschiebung relativ zum As-shot-Wert (nicht absolut), damit `NEUTRAL` kamera-unabhängig eindeutig ist; `edl::migrate::{to_envelope, from_envelope}` als einziger Umwandlungspunkt (Upgrade-Kette für künftige Schema-Versionen kommt dort rein, Aufrufer ändert sich nicht)
  - [x] `EditHistoryId` in `apx-core::ids` (gleiche `define_id_type!`-Konvention wie `PhotoId`/`FolderId`)
  - [x] `migrations/0002_edits.sql`: `edit_history` (vollständige EDL-Schnappschüsse, `UNIQUE(photo_id, sequence)`) + `edit_current` (1 Zeiger pro Foto) — additiv, `photos`/`folders`/`previews` unverändert
  - [x] `repository::edits`: `commit` (verwirft „Zukunft" bei neuer Bearbeitung nach Undo), `current`/`undo`/`redo` (linearer Verlauf, `HistoryPosition::Neutral` für „noch nie bearbeitet"/„bis zum Anfang zurück"), `list_history` — neue `Catalog`-Methoden `commit_edit`/`current_edit`/`undo_edit`/`redo_edit`/`list_edit_history`
  - [x] Tests: EDL-Roundtrip (Umschlag und `EdlV1` einzeln), unbekannte/fehlerhafte Schema-Version abgelehnt, Undo/Redo-Zustandsmaschine (inkl. „Redo nach neuer Bearbeitung verwirft verworfene Zukunft"), FK-Cascade (Foto löschen → Verlauf + Zeiger weg), Migrations-Idempotenz, alter (nur-Migration-1-)Katalog öffnet noch und zieht Migration 2 nach
  - [x] Verifiziert: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used`, `cargo test --workspace` (116 Tests) — alles grün

- [x] 3. wgpu-Gerätekontext (`apx-pipeline::gpu`)
  - [x] `GpuContext` (`Instance`/`Adapter`/`Device`/`Queue`), `Backends::all()` mit explizitem Fallback: erst bevorzugter Hardware-Adapter, dann `force_fallback_adapter: true`, erst danach `PipelineError::GpuUnavailable` — Konstruktion darf laut Test nie abstürzen (`Ok` oder `Err(GpuUnavailable)`, beides gültig)
  - [x] Gemeinsamer Dispatch-Helfer `gpu::dispatch::run_compute_f32` (Bind-Group-Layout `binding(0)`=Uniform-Parameter, `(1)`=Storage-Read-Eingabe, `(2)`=Storage-Read-Write-Ausgabe; alle Phase-2-Regler 1:1) — Shader-Kompilierfehler über `push_error_scope`/`pop_error_scope` abgefangen statt sie zu ignorieren
  - [x] Test: trivialer "Addiere Konstante"-Compute-Shader — **lief in dieser Sandbox tatsächlich auf einem echten GPU-Adapter durch** (nicht nur kompiliert), Ergebnis bit-genau gegen die CPU-Erwartung geprüft; überspringt sich selbst mit Diagnosemeldung, falls in einer Umgebung ganz ohne Adapter ausgeführt (z. B. manche CI-Runner vor Schritt 8s Verifikation)
  - [x] Verifiziert: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used`, `cargo test --workspace` (118 Tests, inkl. echtem GPU-Dispatch) — alles grün

- [x] 4. Die 7 Regler: WGSL-Shader + CPU-Fallback
  - [x] 5 Regler-Module (`white_balance`, `exposure`, `contrast`, `highlights_shadows`, `whites_blacks` — Lichter+Tiefen bzw. Weiß+Schwarz je ein Modul mit zwei Parametern, da mathematisch dieselbe tonwertzonen-gewichtete Operation), je mit eigenem `.wgsl`-Shader (`include_str!`), `#[repr(C)] #[derive(Pod, Zeroable)]`-Parameter-Struct, rayon-parallelisiertem CPU-Fallback mit identischer Mathematik und GPU-Dispatch über `gpu::dispatch::run_compute_f32`
  - [x] Fusionierter Shader `basic_fused` (alle 5 Module in einem GPU-Aufruf, für den interaktiven Vorschau-Pfad) — Abgleichstest `fused_matches_sequential_application_of_individual_stages` beweist identisches Ergebnis zu den Einzel-Shadern
  - [x] Weißabgleich als Verschiebung relativ zu `apx_raw::LinearImage::as_shot_wb_coeffs` (`[R, G, B, E]`-Konvention, viertes/Emerald-Koeffizient ignoriert wie bei der bestehenden `ColorPipeline`) statt absoluter Kelvin/Tint-Werte, `compute_gains` rechnet Kamera-Rohdaten + Nutzer-Verschiebung in die drei Kanal-Gains um
  - [x] `apx_raw::decode_linear()` additiver Einstiegspunkt (neue `LinearImage { width, height, pixels: Vec<f32>, as_shot_wb_coeffs }`) — mirrort `decode_raw()` bis vor `ColorPipeline` für RAW, normalisiert `decode()`s u16-Ausgabe mit neutralen Koeffizienten für Fallback-Formate (JPEG/PNG/TIFF); bestehender `decode()`/`DecodedImage`-Vertrag für Phase-1-Aufrufer unverändert
  - [x] Dafür genericisiert (zero-risk, bestehende Tests unverändert grün): `Orientation::apply_rgb<T>` (privat, `apply_rgb16`/`apply_rgb_f32` als Wrapper), `crop_to_active_area<T>`; neu `downsample_linear_if_needed` (f32-Pendant zu `downsample_if_needed`)
  - [x] Tests pro Modul: Hand-Erwartungswert (z. B. Belichtung +1 EV ⇒ Ausgabe = Eingabe × 2), Identität bei neutralem EDL, CPU/GPU-Abgleich (Toleranz `1e-4`, GPU-Tests laufen in dieser Sandbox tatsächlich auf echter Hardware statt sich zu überspringen), Fusion-Abgleich; `apx-raw`: 6 neue Tests für `decode_linear`s Bausteine (Orientierung/Crop/Downsample für f32)
  - [x] Bewusste Vereinfachungen dokumentiert (Modul-Doku-Kommentare): Lichter/Tiefen/Weiß/Schwarz wirken kanalweise statt luminanzbasiert (echte Tonwertzonen-Maskierung kommt mit Phase 4); Weißabgleich-Umrechnung ist eine lineare Näherung, keine physikalische Planckscher-Strahler-Rechnung (käme mit `lcms2`-Integration später)
  - [x] `tile_cache.rs` bewusst weiterhin Platzhalter — die reale Implementierung gehört an den Tauri-Aufrufort (Schritt 5), wo Cache-Schlüssel und Lebensdauer konkret feststehen, statt spekulativ vorab zu entwerfen
  - [x] `color/mod.rs` bewusst weiterhin Platzhalter — Schritt 4s Umfang war explizit „die 7 Regler"; `lcms2`/ProPhoto-Farbmanagement ist an keiner Reglerformel beteiligt und bleibt einem eigenen Schritt vorbehalten, sobald ein konkreter Aufrufer (z. B. Bildschirmprofil-Anzeige) es braucht
  - [x] Verifiziert: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used`, `cargo test --workspace` (150 Tests, inkl. aller `gpu_matches_cpu`-Tests tatsächlich auf echter GPU-Hardware) — alles grün

- [x] 5. Tauri-Anbindung: Command + Protokoll-Route
  - [x] `AppState` um `pipeline: Arc<apx_pipeline::GpuContext>` (einmal beim App-Start via `GpuContext::new_blocking()` aufgebaut, `expect()`-Ausnahmefall wie beim Katalog) und `tile_cache: Arc<apx_pipeline::tile_cache::TileCache>` erweitert
  - [x] Neue `ImageRequest::Develop { photo_id, max_edge, edl_json }`, geparst aus `develop/<id>/<max_edge_oder_'full'>/<edl_json>` — Feld/Reihenfolge gegenüber der ursprünglichen Plan-Notiz („edl_hash") bewusst korrigiert: `edl_json` trägt die volle EDL-JSON-Serialisierung, nicht nur eine Prüfsumme, weil die Route auch während des Ziehens (noch nicht committet) live rendern muss (siehe ADR-0016-Korrektur, `route.rs`-Moduldoku)
  - [x] `route::parse` auf variable Segmentanzahl umgebaut (`preview`/`image`: 3 Segmente, `develop`: 4), gemeinsame `parse_photo_id`/`parse_max_edge`-Helfer statt Duplizierung
  - [x] Neues `apx-pipeline`-Modul `develop` (`render_rgba8`): Weißabgleich-Gains → `basic_fused` (GPU mit automatischem CPU-Fallback bei Laufzeitfehler) → `color::linear_camera_rgb_to_srgb_rgba8` (feste Kamera→sRGB-Matrix + `apx_raw::srgb_gamma`) — der einzige Einstiegspunkt, den `apx-app` aufruft (reine Verdrahtung bleibt reine Verdrahtung)
  - [x] `color/mod.rs` jetzt real gefüllt (Matrix+Gamma+RGBA8-Quantisierung) statt Platzhalter — `lcms2`/ProPhoto bleiben bewusst zurückgestellt, siehe ADR-0019
  - [x] `apx_raw::LinearImage` um `cam_to_srgb: [[f32; 3]; 3]` erweitert (Einheitsmatrix für Fallback-Formate), `cam_to_srgb_matrix` von privat auf `pub(crate)` angehoben, `srgb_gamma` zusätzlich öffentlich re-exportiert (siehe ADR-0019) — additiv, bestehende `decode()`-Aufrufer unverändert
  - [x] `tile_cache.rs` jetzt real implementiert (die in Schritt 4 bewusst hierher verschobene Implementierung): kleiner, hand-gerollter LRU-Cache (Kapazität 4) für `LinearImage` pro `(photo_id, max_edge)` — ohne EDL im Schlüssel, da `decode_linear` nicht vom EDL abhängt; anders als `apx-app`s `ImageCache` hält er Einträge stark (Wiederverwendung über aufeinanderfolgende Regler-Ticks, nicht nur gleichzeitige Anfragen)
  - [x] Antwortformat: 8-Byte-Header (Breite/Höhe als `u32` little-endian) + rohes RGBA8, `Content-Type: application/x-apx-develop-rgba8` (ADR-0016)
  - [x] Neue Commands `apply_develop_edit`/`current_develop_edit`/`undo_develop_edit`/`redo_develop_edit` (validieren EDL-JSON vor dem Schreiben, delegieren an `apx-catalog`s bereits getestete `commit_edit`/`current_edit`/`undo_edit`/`redo_edit`)
  - [x] Tests: `route::parse` für das neue 4-Segment-Format (inkl. Ablehnung falscher Segmentanzahl/ungültiger Kantenlänge/leerem EDL-JSON), `compute_develop`-Integrationstest (Antwortgröße = 8 + width×height×4, Ablehnung kaputten JSONs), Cache-Schlüssel-Unterscheidungstest um `Develop` erweitert (zwei EDL-Zustände desselben Fotos → zwei Schlüssel), `TileCache`-Tests (Wiederverwendung, Schlüssel-Trennung nach Foto/Auflösung, LRU-Verdrängung), `apx-pipeline::develop`-Tests (Größe/Alpha, dunklere Ausgabe bei negativer Belichtung, GPU≈CPU), `apx-pipeline::color`-Tests (Kanaltausch durch die Matrix, Alpha immer 255)
  - [x] Verifiziert: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used`, `cargo test --workspace` (172 Tests) — alles grün

- [x] 6. Frontend: Entwickeln-Regler, Undo/Redo, WebGL2-Viewer
  - [x] `lib/edl.ts`: TS-Gegenstück zu `EdlV1`/`BasicAdjustments`/`WhiteBalanceAdjustment`, `SliderSpec`-Konfiguration für alle 8 Zahlenwerte (Weißabgleich zählt als 1 Regler mit 2 Werten), `buildEdlEnvelopeJson`/`parseEdlEnvelopeJson`, `applyArrowStep`/`clampSliderValue` (SPEC.md §4-Konvention), `readBasicField`/`writeBasicField`
  - [x] `DevelopSlice` im bestehenden Store (`store/index.ts`): `developPanelOpen`, `developBasic` (Live-Zustand), `developPhotoId`, `setBasicField` (Zwischenwert), `commitDevelopEdit` (schreibt via `apply_develop_edit`), `undoDevelop`/`redoDevelop`, `loadDevelopStateForPhoto` (lädt `current_develop_edit` beim Öffnen/Fotowechsel) — `selectPhoto` lädt den Bearbeitungszustand automatisch nach, wenn das Panel offen ist
  - [x] **Korrektur gegenüber ADR-0018 (Schritt 0):** kein `zundo` — Undo/Redo läuft direkt über die bereits vollständig getestete `edit_history` (siehe ADR-0018s Revisions-Notiz); die eigentlich für `zundo` vorgesehene Entprellung (nicht bei jedem Zwischenwert committen) übernimmt stattdessen `DevelopSlider`s `onChange`(live)/`onCommit`(Loslassen)-Trennung auf UI-Ereignis-Ebene
  - [x] `DevelopSlider.tsx` (wiederverwendbare Komponente, 8-mal benutzt statt dupliziert): Doppelklick=Zurücksetzen, Zahlenfeld-Direkteingabe, Pfeiltasten=Feinschritt, Umschalt+Pfeiltasten=Grobschritt (SPEC.md §4)
  - [x] `DevelopPanel.tsx`: Weißabgleich (2 Regler) + die 6 Ton-Regler, Rückgängig/Wiederholen-Knöpfe + Strg/Cmd+Z/Umschalt+Z, Toggle-Knopf in `Header.tsx`
  - [x] `hooks/useDevelopRender.ts`: fetch()+AbortController analog zu `useImageBitmap`, parst das 8-Byte-Breite/Höhe-Header+RGBA8-Antwortformat (siehe ADR-0016)
  - [x] `lib/webgl.ts` (`QuadRenderer`): `Viewer.tsx` auf WebGL2 umgestellt (ADR-0020) — zeigt ohne offenes Entwickeln-Panel exakt wie in Phase 1 das bestehende Vorschau-/Vollbild-`ImageBitmap`, mit offenem Panel zusätzlich den `develop`-Live-Render (RGBA8) über denselben Textur-Mechanismus; Zoom/Pan-Geometrie (`viewerMath.ts`) unverändert wiederverwendet
  - [x] Tests: Vitest `lib/edl.test.ts` (Envelope-Roundtrip, Schema-Versions-Ablehnung, Pfeiltasten-Schritte, Feld-Lesen/Schreiben) — 33 Frontend-Tests insgesamt, `tsc -b`/`vite build` grün; Playwright `develop-flow.spec.ts` (Panel öffnen/Regler sichtbar, Direkteingabe committet ans simulierte Backend, Rückgängig stellt wieder her, Doppelklick-Reset committet) — `viewer-flow.spec.ts` unverändert grün (Regressionsschutz für den WebGL2-Umbau), `e2e/tauri-mock.ts` um die vier neuen Commands + die `develop/...`-Routen-Antwort (8-Byte-Header + graues 2×2-RGBA8) erweitert

- [x] 7. Performance-Feinschliff fürs 16-ms-Ziel
  - [x] Entprellung: `useDevelopRender` schickt eine Anfrage frühestens im nächsten `requestAnimationFrame` (statt sofort bei jeder `edlJson`-Änderung) — koppelt die Anfragerate an den tatsächlichen Bild-Rhythmus des Geräts statt an eine feste Millisekundenzahl; mehrere Änderungen im selben Frame lösen nur eine Anfrage aus
  - [x] Rust-seitige Zeitmessung: `compute_develop` misst Dekodier- (`TileCache`-Treffer/Fehlschlag) und Render-Zeit getrennt und loggt sie strukturiert (`tracing::debug!`)
  - [x] Frontend-seitige Zeitmessung: `performance.now()` um `fetch()` in `useDevelopRender`, Ergebnis im Store (`developLastLatencyMs`) und sichtbar in `DevelopPanel` ("Letztes Rendering: N ms")
  - [x] Neuer Rust-Test `render_rgba8_timing_on_synthetic_standard_edge_image` (2048×1365, `--nocapture`) misst `render_rgba8` isoliert (der Teil, der bei *jedem* Regler-Tick läuft) für GPU- und CPU-Pfad
  - [x] **Ehrlich gemessene Zahlen (diese Sandbox, Software-Vulkan-Adapter `llvmpipe`, keine echte GPU-Hardware verfügbar):** GPU-Pfad ≈ 181 ms, CPU-Fallback-Pfad ≈ 102 ms für ein 2048×1365-Bild — **das 16-ms-Ziel wird auf dieser Software-Adapter-Hardware klar verfehlt**, und überraschenderweise ist der CPU-Pfad hier *schneller* als der "GPU"-Pfad: `llvmpipe` ist selbst ein CPU-basierter Software-Renderer, sodass der zusätzliche Puffer-Upload/Rücklese-Overhead über die (Software-)Vulkan-API den reinen rayon-parallelisierten CPU-Rechenweg nicht aufwiegt. Auf echter GPU-Hardware wird ein deutlich anderes (voraussichtlich zugunsten der GPU umgekehrtes) Verhältnis erwartet, konnte in dieser Sandbox aber nicht verifiziert werden — siehe `DECISIONS.md` ADR-0012/ADR-0018 zur allgemeinen Einschränkung „keine echte GPU in dieser Sandbox verfügbar". Diese Zahlen messen außerdem nur den Rust-seitigen Rechenanteil, nicht Tauri-IPC, Browser-`fetch`, Texturupload oder Neuzeichnen — eine vollständige Ende-zu-Ende-Messung in einer echten App-Fensterumgebung steht noch aus (Instrumentierung dafür ist jetzt vorhanden: `tracing::debug!` rückseitig, `developLastLatencyMs` frontseitig)
  - [x] Konsequenz aus der Messung: **keine** blinde "niedriger aufgelöste Zwischenvorschau"-Optimierung eingebaut, wie ursprünglich als möglicher Schritt-7-Ausgang vorgesehen — die Zahlen zeigen, dass der Engpass in dieser Sandbox nicht die Auflösung, sondern der Software-Vulkan-Dispatch-Overhead ist; eine Auflösungsreduktion würde das nicht beheben. Auf echter Hardware ist eine erneute Messung nötig, bevor diese Optimierung gerechtfertigt wäre (nicht vorab annehmen, siehe Schritt-7-Leitprinzip)
  - [x] Verifiziert: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used`, `cargo test --workspace` (173 Tests), `tsc -b`, `vitest run` (33 Tests), `vite build`, `playwright test` (8/8, alle Vorher-Tests weiterhin grün) — alles grün

- [x] 8. Testinfrastruktur: synthetische Daten + wgpu in CI
  - [x] Gemeinsamer Fixture-Helfer `apx-pipeline::test_support` (`#[cfg(test)]`-only): `ramp`/`gray_gradient`/`saturated_channels` — ersetzt die zuvor sieben wortgleichen `(0..300).map(|i| (i as f32) / 300.0).collect()`-Zeilen in den Regler-Testmodulen
  - [x] CI-Lücke geschlossen: `cargo clippy` in `ci.yml` prüfte bisher **ohne** `-D clippy::unwrap_used` (nur lokal verwendet) — jetzt angeglichen; `apx-app` bekam dafür zusätzlich `#![deny(clippy::unwrap_used)]` (fehlte bisher als einziges Crate, hatte aber ohnehin keine `.unwrap()`-Aufrufe außerhalb von Tests)
  - [x] `cargo test --workspace -- --nocapture` in CI (vorher ohne `--nocapture`): ein grüner Testlauf sah bisher identisch aus, egal ob `gpu_matches_cpu`-artige Tests einen echten Adapter fanden oder sich nur weich (`eprintln!` + früher Rückgabewert) übersprungen haben — ohne sichtbare Ausgabe ließ sich das nicht unterscheiden
  - [x] **Empirisch verifiziert (CI-Lauf 33236428223, Commit ca2e39f):** alle drei Rust-Runner fanden tatsächlich einen echten wgpu-Adapter — in keinem der drei Job-Logs (Linux/ubuntu-latest, macOS/macos-latest, Windows/windows-latest) taucht die "übersprungen: kein GPU-Adapter …"-Meldung oder `GpuUnavailable` auch nur ein einziges Mal auf; alle sechs `stages::*::tests::gpu_matches_cpu`-Tests liefen auf allen drei Plattformen als echte GPU-Dispatches durch (nicht nur kompiliert, nicht weich übersprungen). Damit ist das in Schritt 8 als "wichtigstes offenes Risiko" benannte Risiko empirisch ausgeräumt, nicht länger nur angenommen — GPU-Ausführungstests dürfen ab sofort als echte Pflicht gelten (nicht mehr dauerhaft "weich"), genau wie ursprünglich vorgesehen. Linux' Runner-Image bringt demnach offenbar von Haus aus einen nutzbaren Software-Vulkan-Adapter mit (kein zusätzliches `apt-get install mesa-vulkan-drivers` o. ä. war nötig); macOS nutzte vermutlich Metal, Windows vermutlich WARP/DX12 — welcher konkrete Adapter/Backend es auf macOS/Windows genau war, ist aus den Logs selbst nicht ablesbar (kein `tracing::info!`-Output ohne gesetztes `RUST_LOG`), nur dass einer gefunden wurde reicht als Bestätigung.

- [ ] 9. Dokumentation fertigstellen
  - [ ] `THIRD_PARTY.md`, `ARCHITECTURE.md`-Datenfluss-Abschnitt, `FEATURES.md` abhaken

- [ ] 10. Abnahme gegen Phase-2-Kriterien
  - [ ] Definition-of-Done je Feature, EDL-Neustart-Persistenz-Test, Performance-Zahlen, Abschlussbericht

### Nicht in Phase 2 (bewusst zurückgestellt)
Gradationskurve, HSL, Farbmischer, Color Grading, Details/Schärfen/Rauschen, Objektivkorrekturen, Effekte, Kalibrierung, Geometrie/Crop, Reparatur (alle → Phase 4), Presets (→ Phase 5), Masken (→ Phase 6), sowie die fünf per ADR-0011 nach Phase 4 verschobenen Regler (Textur, Klarheit, Dunst entfernen, Dynamik, Sättigung).

### Bekannte offene Punkte aus Phase 1 (unverändert)
- ADR-0007: keine echten RAW-Testdateien (Netzwerkzugriff auf raw.pixls.us blockiert) — betrifft auch Phase 2s Shader-Tests, die deshalb weiterhin auf synthetische Testmuster angewiesen sind.
- ADR-0010: Playwright testet simuliert, nicht die native App.
