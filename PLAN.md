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

## Abgeschlossene Phase: Phase 2 — Pipeline-Kern

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

- [x] 9. Dokumentation fertigstellen
  - [x] `THIRD_PARTY.md`: beim Durchsehen aufgefallen, dass `lcms2` seit Schritt 1 in `Cargo.toml` stand, aber nie tatsächlich verwendet wurde (Farbmanagement kam am Ende ohne es aus, siehe ADR-0019) — Abhängigkeit entfernt statt eine unbenutzte native C-Bibliothek weiter mitzuschleppen, Zeile aus der Tabelle raus mit Erklärungs-Notiz
  - [x] `ARCHITECTURE.md` §5: Modulaufbau-Baumdiagramm um `develop.rs`/`test_support.rs` ergänzt, `color/`s Beschreibung korrigiert (keine `lcms2`-Anbindung mehr, feste Matrix+Gamma statt „ProPhoto-Matrizen"), neuer Abschnitt „Datenfluss Phase 2: Regler → Pixel" (Regler-Tick-Pfad, Commit-Pfad, Undo/Redo-Pfad) analog zu §2s Phase-1-Datenfluss
  - [x] `FEATURES.md`: alle 7 Regler-Zeilen sowie Undo/Redo und Regler-Standardverhalten abgehakt — bei zwei Zeilen ehrlich mit Teil-Einschränkung markiert statt pauschal "Fertig": Weißabgleich fertig, aber Pipette/Kamera-Presets fehlen (zu Phase 4 verschoben, waren nie Teil von `SPEC.md` §5s Phase-2-Satz); Undo/Redo funktioniert, aber kein klickbares Verlaufs-Panel mit benannten Schritten (nur Rückgängig/Wiederholen um je einen Schritt); Regler-Standardverhalten fertig bis auf Alt-Maskenvorschau (betrifft ohnehin keinen der 7 Regler)
  - [x] Verifiziert nach der `lcms2`-Entfernung: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used`, `cargo test --workspace` (176 Tests) — alles grün

- [x] 10. Abnahme gegen Phase-2-Kriterien
  - [x] Neuer Test `apx-catalog::tests::edit_history_persists_across_reopen` (analog zu `open_on_disk_persists_across_reopen`): Katalog auf Platte anlegen, Foto + EDL committen, schließen, neu öffnen, `current_edit()` liefert dieselbe EDL — SPEC.md §7 Punkt 6 ("in der EDL serialisierbar und nach Neustart identisch reproduzierbar") jetzt mit eigenem Test statt nur durch die (rein speicherresidenten) Undo/Redo-Tests indirekt abgedeckt
  - [x] **Definition-of-Done aus `SPEC.md` §7, Punkt für Punkt, ehrlich geprüft:**
    1. *Funktioniert auf Windows/macOS/Linux* — ⚠️ **teilweise**: alle drei Plattformen sind in CI grün (`cargo test`/`cargo build`, siehe Schritt 8) und haben empirisch einen echten wgpu-Adapter (ADR-0021); die tatsächliche App als laufendes Fenster wurde auf keiner der drei Plattformen manuell gestartet/bedient — dieselbe, bereits aus Phase 1 bekannte und dort dokumentierte Einschränkung (ADR-0010: Playwright läuft gegen den Produktions-Build im Browser mit simulierter Tauri-Brücke, nicht gegen die kompilierte native App; kein `tauri-driver` in dieser Sandbox)
    2. *Test vorhanden und grün* — ✅ 177 Rust-Tests + 33 Vitest-Tests + 8 Playwright-Tests, lokal und in CI grün
    3. *Tastenkürzel vergeben und im Cheatsheet* — ⚠️ **teilweise**: Strg/Cmd+Z (Rückgängig) und Strg/Cmd+Umschalt+Z (Wiederholen) sind vergeben (mit Tooltip-Hinweis an den Panel-Knöpfen); ein Cheatsheet-Overlay existiert nicht — das ist laut `FEATURES.md` Zeile 251 explizit erst Phase-10-Scope und war auch am Ende von Phase 1 schon nicht vorhanden, also keine neue Lücke, sondern eine von Anfang an bekannte spätere Phase
    4. *In `FEATURES.md` abgehakt* — ✅ siehe Schritt 9 (zwei Zeilen ehrlich mit Teil-Einschränkung markiert)
    5. *Rückgängig/Wiederholen funktioniert* — ✅ Backend vollständig getestet (`apx-catalog::repository::edits`), Frontend-Integration über `develop-flow.spec.ts` mit echtem Zustandsabgleich verifiziert
    6. *In der EDL serialisierbar und nach Neustart identisch reproduzierbar* — ✅ neuer Test oben
    7. *Performance-Budget aus 2.4 eingehalten* — ❌ **nicht eingehalten** (ehrlich gemessen, Schritt 7): ≈181 ms (GPU-Pfad, Software-Adapter `llvmpipe`) bzw. ≈102 ms (CPU-Fallback) statt der geforderten <16 ms für ein 2048×1365-Bild, in dieser Sandbox ohne echte GPU-Hardware gemessen — auf echter GPU-Hardware unverifiziert, siehe Schritt 7s Detailanalyse (Software-Vulkan-Dispatch-Overhead als vermuteter Haupt-Engpass, nicht die Auflösung)
  - [x] Verifiziert (finaler Stand nach Schritt 10): `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used`, `cargo test --workspace` (177 Tests), `tsc -b`, `vitest run` (33 Tests), `vite build`, `playwright test` (8/8) — alles grün
  - [x] Abschlussbericht an den Nutzer (siehe Chat-Antwort dieser Sitzung) — inklusive der oben gelisteten, tatsächlich gemessenen/geprüften Zahlen statt Annahmen

### Nicht in Phase 2 (bewusst zurückgestellt)
Gradationskurve, HSL, Farbmischer, Color Grading, Details/Schärfen/Rauschen, Objektivkorrekturen, Effekte, Kalibrierung, Geometrie/Crop, Reparatur (alle → Phase 4), Presets (→ Phase 5), Masken (→ Phase 6), sowie die fünf per ADR-0011 nach Phase 4 verschobenen Regler (Textur, Klarheit, Dunst entfernen, Dynamik, Sättigung).

### Bekannte offene Punkte aus Phase 1 (unverändert)
- ADR-0007: keine echten RAW-Testdateien (Netzwerkzugriff auf raw.pixls.us blockiert) — betrifft auch Phase 2s Shader-Tests, die deshalb weiterhin auf synthetische Testmuster angewiesen sind.
- ADR-0010: Playwright testet simuliert, nicht die native App.

---

## Abgeschlossene Phase: Phase 3 — Bibliothek

Ziel (laut `SPEC.md` §5): Import, Ordner, Raster, Filmstreifen, Vorschau-Generierung, Bewertungen/Flaggen/Farben, Sammlungen, Filter, Metadaten-Panel, FTS-Suche.

Wie bei Phase 2 gibt es kein eigenes ausführliches Prompt-Dokument — die Schrittliste ist selbst erarbeitet (kein Explore-/Plan-Subagenten-Einsatz diesmal, auf ausdrücklichen Wunsch des Nutzers mit reduziertem Planungsaufwand — vollständige Herleitung samt Architektur-Entscheidungen in `DECISIONS.md` ADR-0022 bis ADR-0025).

**Wichtige Scope-Korrektur (ADR-0022):** `FEATURES.md` hatte wieder deutlich mehr Punkte (§3.1s vollständiger BIBLIOTHEK-Katalog) auf Phase 3 getaggt, als `SPEC.md` §5s Phase-3-Satz meint — analog zu ADR-0011 bei Phase 2 auf den Satz zurückgeschnitten, siehe `FEATURES.md` §3.1 für die einzelnen Umtaggungen.

### Reihenfolge

- [x] 0. Scope festzurren
  - [x] `FEATURES.md`: Über-Scope-Punkte auf spätere Phasen umgetaggt, zwei fehlende Zeilen (Volltextsuche, Metadaten-Panel) ergänzt
  - [x] `DECISIONS.md`: ADR-0022 bis ADR-0025
  - [x] Dieser Abschnitt in `PLAN.md`

- [x] 1. DB-Schema-Erweiterung (Migration `0003_library.sql`)
  - [x] `rating`/`flag`/`color_label` auf `photos`
  - [x] `keywords` + `photo_keywords`
  - [x] `collections` + `collection_photos`
  - [x] `photos_fts` (FTS5 external-content) + Sync-Trigger
  - [x] Tests: Idempotenz, alter Katalog (nur Migration 1, bzw. 1+2 mit Bestandsdaten) öffnet und zieht Migration 3 nach inkl. FTS5-Backfill-Verifikation; FK-Cascade-Tests für `keywords`/`collections` bewusst auf Schritt 2 verschoben (dort existieren echte `Catalog`-Methoden zum Einfügen von Testdaten, konsistent mit dem bestehenden `repository::edits`-Testmuster statt Rohdaten in `migrations.rs`)

- [x] 2. Repository- und `Catalog`-Erweiterungen
  - [x] `repository::{keywords, collections, search}` als neue Module; Bewertung/Flagge/Farbe bewusst in `repository::photos` belassen statt eigenem `ratings`-Modul (gleiche Tabelle, gleiches Muster wie das bestehende `set_missing` — konsistenter mit der im Datei-Kopf von `repository/mod.rs` dokumentierten Regel "ein Modul pro Tabelle" als ein eigenes Modul für drei Spalten derselben Tabelle)
  - [x] Neue `Catalog`-Methoden: `set_photo_rating`/`set_photo_flag`/`set_photo_color_label` (mit Validierung: Bewertung 0–5, Flagge -1/0/1, Farbe aus fester Palette — neue `AppError::Validation`-Variante in `apx-core`), `add_keyword`/`remove_keyword`/`list_keywords_for_photo`/`list_all_keywords`, `create_collection`/`rename_collection`/`delete_collection`/`add_photo_to_collection`/`remove_photo_from_collection`/`list_collections`/`list_photos_in_collection`, `search_photos` (FTS5 `MATCH`, nach `rank` sortiert), `filter_photos` (kombinierbarer Attributfilter, UND-verknüpft)
  - [x] `apx-core`: neue IDs `KeywordId`/`CollectionId`
  - [x] `Photo` um `rating`/`flag`/`color_label` erweitert; neue Modelle `Keyword`, `Collection`, `FilterCriteria`
  - [x] Tests je Modul (u. a. FK-Cascade für `photo_keywords`/`collection_photos`, FTS5-Sync-Trigger-Verifikation nach `UPDATE`, Positions-Reihenfolge in Sammlungen) plus ein Catalog-Integrationstest, der alle neuen Features durch die öffentliche API kombiniert
  - Verifiziert: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used`, `cargo test --workspace` — alles grün

- [x] 3. Tauri-Commands + DTOs
  - [x] `PhotoDto` um `rating`/`flag`/`color_label` erweitert; neue `KeywordDto`/`CollectionDto`/`FilterCriteriaDto`
  - [x] 16 neue Commands als reine Verdrahtung auf Schritt 2 (`set_photo_rating`/`set_photo_flag`/`set_photo_color_label`, `add_photo_keyword`/`remove_photo_keyword`/`list_photo_keywords`/`list_all_keywords`, `create_collection`/`rename_collection`/`delete_collection`/`list_collections`/`add_to_collection`/`remove_from_collection`/`list_photos_in_collection`, `search_photos`, `filter_photos`), in `main.rs`s `generate_handler!` registriert
  - Verifiziert: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used`, `cargo test --workspace` — alles grün

- [x] 4. Import-Erweiterung
  - [x] `ImportMode { AddInPlace, Copy, Move }` (`import::mode`) — `stage_file_for_mode` kopiert/verschiebt vor dem bestehenden Scan-/Metadaten-/Thumbnail-Ablauf, der danach unverändert weiterläuft; Metadaten werden immer vom Ursprungspfad gelesen (bei `Move` existiert die Quelle danach nicht mehr)
  - [x] Rename-Token-System (`import::rename`, reine Funktion ohne Dateisystemzugriff): `{date}`/`{seq}`/`{camera}`/`{original}`, unbekannte Kamera → Platzhalter, Dateiendung bleibt vom Original erhalten, verbotene Dateinamenzeichen werden bereinigt
  - [x] Import-Presets (`import::presets`, JSON via neuem `AppPaths::import_presets_file()`, analog zu `apx_core::Settings`s Lade-/Speicherschema, aber Liste statt Einzelstruktur)
  - [x] Neuer Tauri-Command `import_folder_with_mode` additiv zum bestehenden `import_folder` (bleibt Add-in-Place, unverändert für das aktuelle Frontend); `list_import_presets`/`save_import_preset`/`delete_import_preset`
  - Verifiziert: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used`, `cargo test --workspace` — alles grün (inkl. neuem End-to-End-Test: Copy-Modus mit Umbenennungsmuster kopiert in Zielordner, Quelldatei bleibt erhalten, Katalogeintrag zeigt auf neuen Ort/Namen)

- [x] 5. Ordner-Erweiterung
  - [x] Sidebar-Baumdarstellung über `parent_id` (`lib/folderTree.ts`s reine `buildChildrenByParent`, rekursive `FolderNode`-Komponente; verwaiste `parent_id`-Referenzen werden defensiv als Wurzel behandelt statt den Ordner zu verlieren)
  - [x] `relink_folder`-Command (`repository::folders::update_path` → `Catalog::relink_folder`), ruft danach `reconcile_missing` für den neuen Pfad auf, analog zum bestehenden Öffnen-Ablauf
  - [x] Ordner-fehlend-Erkennung: `FolderDto.missing` live per `path.exists()` berechnet (keine neue DB-Spalte/Migration nötig, keine Reconcile-Kaskade beim Start), Sidebar zeigt "fehlt"-Badge + "verknüpfen"-Link (öffnet den bestehenden Ordner-Dialog, ruft `relink_folder`)
  - Verifiziert: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used`, `cargo test --workspace`, `tsc -b`, `vitest run` (37 Tests), `playwright test` (8/8) — alles grün

- [x] 6. Frontend: Raster, Bewertung/Flaggen/Farben, Sammlungen, Filterleiste, Metadaten-Panel
  - [x] `GridView.tsx`: zeilenvirtualisiert (`@tanstack/react-virtual`, Spaltenzahl per `ResizeObserver`), geteilter Auswahl-/Fotoliste-Zustand mit Filmstreifen über neue `selectActivePhotos`/`togglePhotoSelection`/`resolveSelectionMode` im Store (ADR-0024) — Klick/Strg-Klick/Umschalt-Klick für Ersetzen/Umschalten/Bereichsauswahl, geteilt zwischen Raster und Filmstreifen. Zellen sind `role="button"`-`div`s statt verschachtelter `<button>`s (HTML erlaubt kein interaktives Element in einem anderen — die Bewertungs-/Flaggen-/Farb-Widgets pro Zelle sind selbst Buttons)
  - [x] Bewertungs-/Flaggen-/Farb-Widgets (`RatingFlagColor.tsx`, in Raster-Zelle kompakt und im Metadaten-Panel ausführlich) + Tastenkürzel `0`–`5` (Bewertung, erneutes Klicken auf den aktuellen Wert löscht ihn), `P`/`X` (Pick/Reject) im bestehenden globalen Tastatur-Handler in `App.tsx`
  - [x] Sammlungen-UI: neuer Abschnitt in `Sidebar.tsx` unterhalb des Ordnerbaums, Inline-Eingabe zum Anlegen, "+"-Knopf pro Sammlung fügt die aktuelle Auswahl hinzu (erscheint nur bei aktiver Auswahl)
  - [x] Filterleiste (`FilterBar.tsx`, nur im Raster sichtbar): Suchfeld (`search_photos`) und Attribut-Chips (Bewertung/Flagge/Farbe, `filter_photos`) — bewusst alternativ statt kombiniert (Setzen des einen leert das andere), beide wirken über ein gemeinsames `libraryResults`, das `selectActivePhotos` gegenüber Ordner/Sammlung priorisiert
  - [x] Metadaten-Panel (`MetadataPanel.tsx`, strukturell wie `DevelopPanel.tsx`): read-only EXIF-Felder plus editierbare Bewertung/Flagge/Farbe/Schlagworte
  - [x] Tests: Vitest für `resolveSelectionMode`/`selectActivePhotos` (`store/index.test.ts`) und `buildChildrenByParent` (bereits Schritt 5); neue Playwright-Spezifikation `library-flow.spec.ts` (Raster anzeigen, Foto bewerten, Sammlung anlegen und befüllen, nach Bewertung filtern, Filter zurücksetzen); `tauri-mock.ts` um alle neuen Commands erweitert
  - **Mock-Erkenntnis (kein App-Bug):** `tauri-mock.ts` gab Fotos/Sammlungen/Schlagworte anfangs als dieselbe Objektreferenz zurück, die es intern weiterverwendet — echtes Tauri-IPC serialisiert dagegen bei jedem Aufruf frisch. Sobald Zustand/Immer so ein Objekt in den Store einlagert, friert Immer es ein; eine spätere direkte Eigenschaftszuweisung im Mock (`photo.rating = …`) blieb dadurch in der nicht-strikten `addInitScript`-Umgebung lautlos wirkungslos (kein Fehler, aber auch keine Änderung), und `collections.push(...)` auf einem eingefrorenen Array warf sogar einen echten Fehler. Behoben, indem der Mock bei jeder Foto-/Sammlungs-/Schlagwort-Liste eine Kopie statt der Live-Referenz zurückgibt — entspricht dem tatsächlichen IPC-Verhalten und ist kein Hinweis auf einen Fehler im Produktivcode
  - **Zustand-Erkenntnis:** `selectActivePhotos` lieferte bei leerer Auswahl bei jedem Aufruf ein neues `[]`-Literal — `useAppStore(selector)`s `Object.is`-Vergleich hielt das für eine sich ständig ändernde Snapshot und löste über `useSyncExternalStore` endlose Neu-Renderings aus ("Maximum update depth exceeded"). Behoben mit `useShallow` aus `zustand/react/shallow` an allen drei Verwendungsstellen (`Filmstrip`/`GridView`/`MetadataPanel`)
  - Verifiziert: `tsc -b`, `vitest run` (45 Tests), `playwright test` (9/9, inkl. neuer Bibliotheks-Spezifikation) — alles grün. Keine Rust-Änderungen in diesem Schritt.

- [x] 7. Tests, Dokumentation, Abnahme (gebündelt)
  - [x] Vollständige Verifikation: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used`, `cargo test --workspace` (196 Testzeilen, alle grün), `tsc -b`, `vitest run` (45 Tests), `playwright test` (10/10), `vite build` — alles grün
  - [x] `ARCHITECTURE.md` (neuer Abschnitt „6. Architektur Phase 3 — Bibliothek" inkl. Datenfluss „Suche/Filter → Ergebnisliste")/`FEATURES.md` aktualisiert; `THIRD_PARTY.md` unverändert (keine neuen Abhängigkeiten in Phase 3 — geprüft per `git log` auf `Cargo.toml`/`package.json` seit Phase 2 Schritt 1)
  - [x] Bei der Abnahme zwei von ADR-0022 übersehene Über-Scope-Punkte gefunden und korrigiert (ADR-0026): „Duplikaterkennung per exaktem Hash" und „Sortierung nach beliebigem Feld" waren fälschlich auf Phase 3 getaggt, obwohl sie in `SPEC.md` §5s Phase-3-Satz nicht vorkommen — auf Phase 6 umgetaggt, nicht gebaut vorgetäuscht
  - [x] Kleine Lücke geschlossen: `FilterBar.tsx` bekam einen Kameramodell-Filter-Chip (Backend unterstützte `camera_model` in `filter_photos` bereits, es fehlte nur die UI)
  - [x] 100.000-Foto-Raster-Performance-Check (`SPEC.md` §2.4): manuell mit einem Einweg-Playwright-Test verifiziert — DOM-Zellenzahl bleibt bei Ruhe/Scroll-Mitte/Scroll-Ende zwischen 35 und 60 (nie nahe 100.000), Ersteinbindung+Wechsel ins Raster ~2,2 s in dieser Sandbox (keine echte GPU/reales Hardware-Profil), JS-Heap ~119 MB — automatisiertes CI-Regressionsäquivalent mit 5.000 Fotos in `library-flow.spec.ts` (analog zum bestehenden 50.000/5.000-Muster des Filmstreifens aus Phase 1)
  - [x] Definition-of-Done je Feature gegen `SPEC.md` §7 geprüft, ehrlicher Abschlussbericht (siehe Chat)

### Nicht in Phase 3 (bewusst zurückgestellt)
Siehe `FEATURES.md` §3.1 für die genaue Zuordnung: Gesichtserkennung, virtuelle Kopien, Stapel, Sekundäres Display (→ Phase 9 bzw. eigene spätere Ausbaustufe), Schlagwort-Hierarchie/Synonyme/Auto-Vervollständigung, intelligente Sammlungen mit Regeln, Sammlungssätze, Metadaten-Presets, Stapel-Metadatenbearbeitung, vollständiger EXIF/IPTC/XMP-Editor, Sidecar-Export, Vergleichs-/Übersichtsansicht, Vorschau-Cache-Verwaltung/Smart Previews, Filter-Presets, Schnellentwicklung im Raster (→ Phase 6), Perceptual-Hash-Duplikaterkennung, Katalog-Statistiken-Dashboard (→ Phase 9), DNG-Konvertierung (→ Phase 5).

## Schritt 8 — Nachtrag: fünf zurückgestellte Punkte nachgezogen

Nach dem Abschlussbericht zu Schritt 7 hat der Nutzer entschieden, genau
die fünf dort ehrlich benannten Lücken jetzt noch in Phase 3 zu
schließen — nicht den kompletten restlichen BIBLIOTHEK-Katalog (siehe
`DECISIONS.md` ADR-0027 für die vollständige Begründung je Punkt).

- [x] 8.1 Undo/Redo für Bibliotheks-Metadaten
  - [x] `frontend/src/lib/undoStack.ts`: reine, getestete Stack-Logik (`pushUndo`/`undo`/`redo`)
  - [x] `store/index.ts`: `libraryUndoStack`/`libraryRedoStack`, `undoLibraryAction`/`redoLibraryAction`; `setPhotoRating`/`setPhotoFlag`/`setPhotoColorLabel`/`addKeywordToPhoto`/`removeKeywordFromPhoto`/`addSelectionToCollection` erfassen den alten Zustand und pushen einen Undo-Eintrag
  - [x] `App.tsx`: Strg/Cmd+Z / Strg/Cmd+Umschalt+Z, gated auf `!developPanelOpen`
  - [x] Bewusst nicht abgedeckt: Sammlung anlegen/umbenennen/löschen

- [x] 8.2 Duplikaterkennung per exaktem Hash
  - [x] Neue direkte Abhängigkeit `sha2` (0.10.9, bereits transitiv vorhanden)
  - [x] `import_single_file` berechnet einen SHA-256-Streaming-Hash und schreibt ihn in `NewPhoto.content_hash`
  - [x] `Catalog::list_duplicate_photo_groups()`, neuer Tauri-Command, `ImportFinishedPayload.duplicate_count`
  - [x] Frontend: „Duplikate anzeigen"-Knopf in `FilterBar.tsx`, Duplikatzahl im Import-Abschlusstext

- [x] 8.3 Sortierung nach beliebigem Feld
  - [x] `frontend/src/lib/sortPhotos.ts` (client-seitig, reine Funktion, fehlende Werte immer ans Ende)
  - [x] `PhotoDto`/`store/index.ts`: neues `file_size`-Feld, `librarySortField`/`librarySortDirection`, `selectActivePhotos` sortiert als letzten Schritt
  - [x] `FilterBar.tsx`: Feld-Auswahl + Richtungs-Umschalter

- [x] 8.4 Kombinierte Suche + Filter
  - [x] `repository/search.rs`: gemeinsamer `build_filter_clause`-Baukasten, neue `search_and_filter_photos` (additiv zu `search_photos`/`filter_photos`)
  - [x] Neuer Tauri-Command `search_and_filter_photos`; `store/index.ts`s `runLibrarySearchAndFilter` kombiniert Suchtext und Filter-Chips statt sie sich gegenseitig löschen zu lassen

- [x] 8.5 Volle Ordnerbaum-Hierarchie beim Import
  - [x] `run_with_mode` berechnet eine Hierarchie-Wurzel (gewählter Ordner bei `AddInPlace`, Zielordner bei `Copy`/`Move`)
  - [x] `ensure_folder` legt rekursiv alle Elternordner bis zur Hierarchie-Wurzel an, mit defensivem Fallback

- [x] Dokumentation + Verifikation
  - [x] `DECISIONS.md` ADR-0027, `FEATURES.md` (zwei Punkte zurück auf Phase 3/Fertig, Filterleisten- und Ordnerbaum-Zeile aktualisiert, neue Undo/Redo-Zeile), `THIRD_PARTY.md` (`sha2`-Eintrag), dieser Abschnitt
  - [x] Neue Tests: Rust (`import::tests::duplicate_photos_are_detected_by_content_hash`, `nested_subfolders_form_a_multi_level_parent_chain`, `repository::photos::list_duplicate_groups_finds_matching_hashes_and_ignores_the_rest`, vier neue `repository::search`-Tests für die kombinierte Suche), Vitest (`undoStack.test.ts`, `sortPhotos.test.ts`), Playwright (`library-flow.spec.ts`: Undo/Redo-Erweiterung des ersten Tests, kombinierte Suche+Filter, Duplikatanzeige, Sortierung)
  - Verifiziert: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used`, `cargo test --workspace`, `tsc -b`, `vitest run`, `playwright test`, `vite build` — alles lokal grün

---

## Aktuelle Phase: Phase 4 — Entwickeln vollständig

Ziel (laut `SPEC.md` §5): Kurven, HSL, Farbmischer, Color Grading, Details, Objektivkorrekturen, Effekte, Kalibrierung, Crop/Geometrie, Reparatur. Mit Abstand die größte Phase bisher — 10 Werkzeugkategorien, ~35 Einzelpunkte in `FEATURES.md` §3.2, mehrere echte Architektur-Neuerungen (EDL-Schema-Erweiterung, neue GPU-Dispatch-Formen, komplett neue Frontend-Widgets).

**Scope-Präzisierung (ADR-0028):** Workflow-Punkte aus `FEATURES.md` §3.4 (Schnappschüsse, Vorher/Nachher, Sync, Soft-Proof etc.) stehen nicht im §5-Satz und wandern auf Phase 6. Objektivkorrekturen bekommen ein eigenes Mini-Profilformat statt echtem Adobe-Profil-Import; Reparatur bekommt manuelles Klonen/Reparieren ohne Auto-Quellenfindung/Content-Aware-Fill (beides auf Phase 6 verschoben). Details zu allen vier Entscheidungen in ADR-0028.

**Architektur-Grundsatzentscheidungen** (Details in ADR-0028 und den jeweiligen Schritten):
- **EDL-Schema v2** statt Erweiterung von `EdlV1` — `apx-pipeline/src/edl/migrate.rs` lehnt unbekannte Schema-Versionen bewusst hart ab (kein `#[serde(default)]`), neue Felder kommen als `EdlV2` mit explizitem `v1_to_v2`-Aufwärtspfad. Keine DB-Migration nötig (`edit_history.edl_json` ist eine opake TEXT-Spalte).
- **Drei GPU-Dispatch-Formen** statt einer: (1) positions-bewusst aber 1:1 (Vignette, Körnung — brauchen Breite/Höhe), (2) Nachbarschafts-Zugriff (Textur/Klarheit, Details, Reparatur-Klonen), (3) größenverändernd (Objektivkorrekturen-Warp, Crop/Geometrie — Ausgabe ≠ Eingabe). Crop/Geometrie wird als CPU-seitiger letzter Schritt in `render_rgba8` umgesetzt (nach der RGBA8-Quantisierung), nicht als GPU-Pass.
- **16-ms-Budget (ADR-0017-Präzedenzfall):** alle Werkzeuge im 1:1-/positions-bewussten Modell (Grundeinstellungs-Ergänzung, Kurven-LUT, HSL, Farbmischer, Color Grading, Kalibrierung, Vignette, Körnung) werden zu einem erweiterten Fused-Pass zusammengefasst statt N einzelner Dispatch-Rundtripps.
- **Kurven-Sequenzierung:** laufen laut bestehendem Code-Kommentar (`stages/contrast.rs`) nach der Farbraum-Konvertierung, auf Luminanz statt pro Kanal — Schritt 4 (nicht mehr Schritt 2, siehe ADR-0029) entscheidet anhand eines Benchmarks, ob die Farbraum-Konvertierung ins WGSL wandert oder Kurven ein schneller CPU-LUT-Nachschritt bleiben.
- **Frontend:** fast alle nötigen UI-Widgets sind komplett neu (Kurven-Editor, Farbrad, HSL-Bänder, Crop-Overlay, Checkbox, Accordion) — einzige wiederverwendbare Primitive sind Regler+Zahlenfeld, gedrückte Buttons, feste Paletten-Swatches, ein natives `<select>`.

### Reihenfolge

- [x] 0. Scope festzurren
  - [x] `DECISIONS.md` ADR-0028
  - [x] `FEATURES.md`: §3.4-Workflow-Zeilen auf Phase 6 umgetaggt, erklärende Kommentare bei Objektivkorrekturen/Kalibrierung/Reparatur/Geometrie ergänzt
  - [x] Dieser Abschnitt in `PLAN.md`

- [x] 1. EDL-Schema v2 + Migration
  - [x] `crates/apx-pipeline/src/edl/v2.rs`: alle neuen Structs (`CurvesAdjustment`, `HslAdjustment`, `ColorMixerAdjustment`, `ColorGradingAdjustment`, `DetailsAdjustment`, `LensCorrectionAdjustment`, `EffectsAdjustment`, `CalibrationAdjustment`, `GeometryAdjustment`, `Vec<RepairStroke>`), `EDL_SCHEMA_VERSION = 2`
  - [x] `migrate.rs`: `v1_to_v2`-Aufwärtspfad, `from_envelope` probiert v2 zuerst
  - [x] Tests: v1→v2-Upgrade-Rundreise, alte `edit_history`-Zeilen (v1-JSON) laden weiterhin korrekt
  - [x] `frontend/src/lib/edl.ts`: gespiegelte TS-Typen (`EdlPayload` mit allen zehn Sektionen) + Neutral-Konstanten/-Funktionen je Sektion; `store/index.ts`s `developBasic`-Zustand zu `developEdl: EdlPayload` erweitert/umbenannt
  - Übergangsstand: `render_rgba8` verarbeitet bislang nur `basic` (über `to_v1_subset`) — die neuen Felder sind bis Schritt 2 inert. Verifiziert: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used`, `cargo test --workspace` (228 Tests), `tsc -b`, `vitest run` (67 Tests), `playwright test` (13/13), `vite build` — alles lokal grün

- [x] 2. GPU-Dispatch-Erweiterung + erweiterter Fused-Pass (Scope präzisiert, siehe ADR-0029: Kurven/HSL/Farbmischer/Color-Grading/Kalibrierung/Effekte bekommen ihr eigenes Modul in ihrem eigenen Schritt statt hier vorgebaut zu werden)
  - [x] `gpu/dispatch.rs` geprüft: `run_compute_f32` trägt unverändert sowohl positions-bewusste als auch nachbarschafts-fähige Operationen (Breite/Höhe als zusätzliche `Params`-Felder, uneingeschränkter Lesezugriff im Shader) — keine Änderung nötig
  - [x] `stages/basic_fused.wgsl`/`.rs` um Dunst entfernen/Dynamik/Sättigung erweitert (12-Feld-`Params`, keine Padding nötig da bereits 48 Byte)
  - [x] Neues `stages/local_contrast.{rs,wgsl}` für Textur/Klarheit (echter 3×3-Nachbarschafts-Zugriff, in `develop.rs` nur dispatcht, wenn mindestens einer der beiden Regler ungleich neutral steht)
  - [x] `develop.rs::render_rgba8` verdrahtet beide Erweiterungen — damit sind alle zwölf Grundeinstellungs-Regler fertig
  - [x] GPU/CPU-Paritätstests je neuem Teil-Feature (Muster: `gpu_matches_cpu`) für Dunst entfernen/Dynamik/Sättigung/Textur/Klarheit
  - Verifiziert: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used`, `cargo test --workspace` (235 Tests), `tsc -b`, `vitest run` (67 Tests) — alles lokal grün

- [x] 3. Grundeinstellungen-Erweiterung (Frontend + Shader)
  - [x] `BASIC_SLIDER_SPECS` auf 13 Einträge erweitert (Textur/Klarheit/Dunst entfernen/Dynamik/Sättigung, Reihenfolge wie `SPEC.md` §3.2), in `DevelopPanel.tsx` sichtbar
  - [x] WB-Kamera-Presets: `WHITE_BALANCE_PRESETS` (7 feste Presets, kein DCP-Import, siehe ADR-0028) + Dropdown in `DevelopPanel.tsx`, setzt Temperatur/Tint absolut
  - [x] WB-Pipette: neues `lib/whiteBalancePicker.ts` (Klick-Farbwert → additive Temperatur-/Tint-Korrektur, bewusste Vereinfachung auf dem gamma-kodierten Anzeigebild statt linearem Kamera-RGB, siehe Moduldoku), Viewer-Klick-Interaktion (Crosshair-Cursor, Pan währenddessen deaktiviert), Pipette-Umschaltknopf in `DevelopPanel.tsx`
  - Verifiziert: `tsc -b`, `vitest run` (80 Tests), `playwright test` (15/15), `vite build` — alles lokal grün; Rust-Seite unverändert (Schritt 2 hat den Shader/CPU-Teil bereits geliefert)

- [x] 4. Kurven
  - [x] Sequenzierungsfrage aus Schritt 2 entschieden: Kurven laufen als CPU-LUT-Nachschritt auf dem fertigen RGBA8-Puffer, nach der Farbraum-Konvertierung — kein GPU-Dispatch nötig (Begründung in `curves.rs`s Moduldoku)
  - [x] `crates/apx-pipeline/src/stages/curves.rs`: Fritsch-Carlson-monotone-kubische-Spline für Punktkurven, vereinfachtes Gauß-gewichtetes Vier-Zonen-Modell für parametrische Kurven, feste Verkettungsreihenfolge Luminanz→RGB→R/G/B
  - [x] Neues `frontend/src/lib/curveMath.ts` (TS-Spiegel für die Editor-Vorschau) + `frontend/src/lib/edl.ts`s `PARAMETRIC_CURVE_SLIDER_SPECS`/`CURVE_PRESETS`
  - [x] Neues `frontend/src/components/CurveEditor.tsx` — SVG statt `<canvas>` (fokussierbare, per Tastatur bedienbare Punkte statt Pixel-Hit-Testing, siehe Moduldoku dort), Punkte-/Parametrisch-Umschalter, numerische Punkteingabe, Presets-Dropdown; in `DevelopPanel.tsx` mit 5 Kanal-Tabs (RGB/Rot/Grün/Blau/Luminanz)
  - Verifiziert: `cargo fmt/clippy/test --workspace` (244 Rust-Tests), `tsc -b`, `vitest run` (88 Tests), `playwright test` (19/19), `vite build` — alles lokal grün

- [x] 5. HSL + Farbmischer erweitert
  - [x] Neues `crates/apx-pipeline/src/stages/hsl_color_mixer.rs`/`.wgsl`: RGB↔HSL-Konvertierung, Gauß-gewichtete Verschiebung nach Farbton-Abstand zu 8 festen Bändern + offener Regionenliste (gekappt auf `MAX_COLOR_MIXER_REGIONS = 8`, siehe Moduldoku), läuft im linearen Arbeitsraum wie `basic_fused`, direkt davor in `develop.rs` verdrahtet
  - [x] `frontend/src/lib/colorSampling.ts` (neu): teilt die Farbton-Berechnung aus einem Bildklick mit der WB-Pipette (`Viewer.tsx`s `handleImageClick` bedient jetzt beide Werkzeuge)
  - [x] 8-Band-HSL-UI (Tabs + 3 Regler) und Farbmischer-UI (Regionen-Liste mit Klick-Aufnahme, Bandbreite/Farbton/Sättigung/Luminanz-Verschiebung je Region) in `DevelopPanel.tsx`
  - Verifiziert: `cargo fmt/clippy/test --workspace` (250 Rust-Tests), `tsc -b`, `vitest run` (93 Tests), `playwright test` (21/21), `vite build` — alles lokal grün

- [x] 6. Color Grading (Farbräder)
  - [x] Neues `crates/apx-pipeline/src/stages/color_math.rs`: aus `hsl_color_mixer.rs` extrahierte gemeinsame RGB↔HSL-Konvertierung/Gauß-Gewichtung (`pub(crate)`, private `mod color_math`), vermeidet Duplizierung zwischen den beiden Rust-Modulen — WGSL dupliziert die Helfer weiterhin je Shader-Datei, da dieses Shader-Modell keine Cross-File-Imports kennt
  - [x] Neues `crates/apx-pipeline/src/stages/color_grading.rs`/`.wgsl`: 4 Farbräder (Schatten/Mitteltöne/Lichter/Global), Gauß-gewichtete Tonwertzonen (fixe Zentren bei Luminanz 0/0,5/1 statt echter verschiebbarer Umschlagpunkte), Balance verschiebt das Gewicht zwischen Schatten-/Lichter-Zone statt deren Zentren zu bewegen (siehe Moduldoku), Überblendung steuert die Zonenbreite (`sigma`); direkt nach `hsl_color_mixer` in `develop.rs` verdrahtet
  - [x] Neues `frontend/src/components/ColorWheel.tsx`, 4× instanziiert (Schatten/Mitteltöne/Lichter/Global) — HTML/CSS-Rad (`radial-gradient`+`conic-gradient`) statt SVG/Canvas, da hier kein Pixel-Hit-Testing nötig ist; `frontend/src/lib/colorWheelMath.ts` (Pixel-Offset↔Farbton/Sättigung, mit Rundreise-Tests)
  - [x] GPU/CPU: tonwertzonen-gewichtete Farbverschiebung (`gpu_matches_cpu`-Paritätstest wie bei den übrigen Werkzeugen)
  - [x] Echter Fehler gefunden und behoben (nicht nur bei Color Grading selbst, sondern ein vorbestehender Bug, der durch dessen Tastatur-E2E-Test aufgedeckt wurde): `App.tsx`s globaler `keydown`-Handler für die Foto-Navigation (Pfeiltasten) prüfte nur auf `INPUT`/`TEXTAREA`-Tags, nicht auf `role="slider"`-Elemente ohne natives Eingabe-Tag — dadurch feuerten `ColorWheel.tsx`s und `CurveEditor.tsx`s eigene Pfeiltasten-Handler gemeinsam mit dem globalen Foto-Wechsel-Kurzbefehl, dessen asynchrones `loadDevelopStateForPhoto` die gerade vorgenommene Regler-Änderung Millisekunden später wieder überschrieb. Behoben durch eine `target.closest('[role="slider"]')`-Ausnahme im selben Guard.
  - Verifiziert: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used`, `cargo test --workspace` (261 Rust-Tests), `tsc -b`, `vitest run` (100 Tests), `playwright test` (23/23), `vite build` — alles lokal grün

- [x] 7. Kalibrierung
  - [x] Neues `crates/apx-pipeline/src/stages/calibration.rs`/`.wgsl`, läuft *vor* Weißabgleich/den Grundeinstellungen (in `develop.rs`, per `Cow<[f32]>` ohne Klon im Regelfall): drei Gauß-gewichtete Primärfarben-Bänder um 0°/120°/240° (Farbton-/Sättigungs-Verschiebung, keine echte Matrixrotation), Schattentönung als additive Grün-/Magenta-Verschiebung (gewichtet mit fester Gauß-Schatten-Zone, dieselbe Konvention wie `white_balance.rs`s Tint), Kameraprofil als globaler Sättigungs-/Kontrast-Bias aus einer kleinen handgepflegten `CAMERA_PROFILES`-Liste (kein DCP-Import) — `PrimaryColorAdjustment`/`CalibrationAdjustment` bekommen einen `pub const NEUTRAL` (vorher nur eine `neutral()`-Fn, jetzt konsistent mit den übrigen Phase-4-Adjustments)
  - [x] Prozessversion bleibt inert (nur `V1` existiert, siehe `edl/v2.rs`s Moduldoku) — dafür in der UI nur eine informative Anzeige statt eines toten Auswahl-Feldes
  - [x] Frontend: 3× Farbton-/Sättigungs-Regler (Primärfarben, eindeutige Labels `Farbton (Rot)` usw. gegen Kollision mit HSL-Bändern), Schattentönung-Regler, Kameraprofil-`<select>` (`CAMERA_PROFILE_OPTIONS` spiegelt `CAMERA_PROFILES`, committet sofort wie ein WB-Preset)
  - Verifiziert: `cargo fmt/clippy/test --workspace` (268 Rust-Tests), `tsc -b`, `vitest run` (100 Tests), `playwright test` (25/25), `vite build` — alles lokal grün

- [x] 8. Details (Schärfung + Rauschreduzierung)
  - [x] Neues `crates/apx-pipeline/src/stages/details.rs`/`.wgsl`, direkt nach Textur/Klarheit in `develop.rs` verdrahtet — erster Schritt, der Schritt 2s Nachbarschafts-Dispatch mit variablem Radius tatsächlich braucht (`sharpen_radius` rundet auf einen ganzzahligen 1–3-Pixel-Box-Filter-Radius, Rauschreduzierung nutzt einen festen 3×3-Box-Weichzeichner wie `local_contrast.rs`)
  - [x] Schärfung (Unsharp Masking je Kanal), Maskierung über eine `smoothstep`-Schwelle auf den Hochpass-Betrag, Deconvolution-Alternativmodus als Potenzfunktions-Verstärkung (bewusster Stand-in statt echter iterativer Entfaltung)
  - [x] Luminanz-/Farbrauschen: Chroma-Glättung relativ zur Luminanz, `detail` bewahrt Kanten über denselben Luminanz-Kantenwert für beide, `contrast`/`smoothness` als zusätzliche Gewichtungsfaktoren — beide bewusst gegenüber `amount` gated, damit die NEUTRAL-Default-Werte (`detail=50`, `smoothness=50`) bei `amount=0` keinen Effekt haben
  - [x] Rauschreduzierung und Schärfung laufen in einem gemeinsamen Durchlauf statt zweier sequenzieller Stufen (echtes Lightroom wendet NR vor Schärfung an) — beide Anteile unabhängig aus derselben Original-Nachbarschaft berechnet und addiert, siehe Moduldoku
  - [x] WGSL-Fallstrick gefunden und behoben (dieselbe Fehlerklasse wie die frühere `active`-Falle, hier aber Indizierung statt Namenskonflikt): naga erlaubt nur konstante Indizes in ein lokales `array<f32, 3>` — die anfängliche Für-Schleife über den Kanal-Index wurde manuell zu drei Blöcken entrollt
  - [x] Frontend: 3 Regler-Gruppen (Schärfung/Luminanzrauschen/Farbrauschen) + eine native Checkbox für den Deconvolution-Modus (erste Checkbox im Projekt — kein eigenständiges `Checkbox.tsx` gebaut, da bislang nur ein einziger Bool-Regler existiert)
  - Verifiziert: `cargo fmt/clippy/test --workspace` (275 Rust-Tests), `tsc -b`, `vitest run` (100 Tests), `playwright test` (26/26), `vite build` — alles lokal grün

- [ ] 9. Objektivkorrekturen
  - [ ] Mini-Profilformat (`crates/apx-pipeline/lens_profiles/*.json` + Lade-/Zuordnungsmodul per EXIF-Objektiv-/Kamerastring)
  - [ ] Manuelle Regler: CA, Vignette, Verzeichnung, Perspektive/Upright (Guided mit 2 Linienpaaren), manuelle Transformation
  - [ ] Geometrischer Warp als eine inverse Abbildung mit bilinearem Sampling

- [ ] 10. Effekte
  - [ ] Nachträgliche Vignettierung, Körnung mit stabilem Pro-Pixel-Seed

- [ ] 11. Geometrie (Crop/Rotation)
  - [ ] Freistellen (Presets, Rasterüberlagerungen), Winkel-Werkzeug, vereinfachte Auto-Ausrichtung (nur EXIF-Orientierung)
  - [ ] CPU-seitiger Crop+Rotate+Resample als letzter Schritt in `render_rgba8`
  - [ ] Neues `frontend/src/components/CropOverlay.tsx`

- [ ] 12. Reparatur (Klonen/Reparieren)
  - [ ] Pinsel-Interaktion (Quellpunkt, Zielpfad, Radius/Deckkraft/weiche Kante)
  - [ ] Klonen (versetzter Lesezugriff + radiale Weichzeichnung), Reparieren (vereinfachtes nahtloses Überblenden, kein echtes Poisson-Blending)
  - [ ] `repair: Vec<RepairStroke>`, je Strich einzeln entfernbar

- [ ] 13. Dokumentation, Tests, Abnahme
  - [ ] `ARCHITECTURE.md`: Phase-4-Platzhalter durch echte Architekturbeschreibung ersetzen
  - [ ] `FEATURES.md`: alle Phase-4-Zeilen auf Fertig (mit „Fertig (abweichend, siehe ADR-0028)" für die vier bewussten Vereinfachungen)
  - [ ] Volle Verifikation + 16-ms-Performance-Nachmessung
  - [ ] Commit+Push, CI-Check auf allen drei Plattformen, ehrlicher Abschlussbericht

### Nicht in Phase 4 (bewusst zurückgestellt)
Siehe ADR-0028: Workflow-Punkte (Schnappschüsse, Vorher/Nachher, Copy/Paste-Einstellungen, Sync, Auto-Sync, Referenzansicht, Soft-Proof), echter Adobe-Profil-Import (Objektivprofile + DCP-Kameraprofile), Auto-Quellenfindung und inhaltsbasiertes Füllen für die Reparatur-Funktion — alle auf Phase 6 verschoben.
