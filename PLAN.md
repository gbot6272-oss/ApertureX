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

## Abgeschlossene Phase: Phase 4 — Entwickeln vollständig

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

- [x] 9. Objektivkorrekturen
  - [x] Mini-Profilformat: `crates/apx-pipeline/lens_profiles/*.json` (3 Beispielprofile, per `include_str!` zur Kompilierzeit eingebettet) + neues `crates/apx-pipeline/src/lens_profiles.rs` (`find_profile` per ID, `match_profile_for_lens_string` per Case-insensitive-Substring-Abgleich gegen EXIF-Objektiv-/Kamerastrings)
  - [x] Neues `crates/apx-pipeline/src/stages/lens_corrections.rs`/`.wgsl`: CA (radiale Kanalverschiebung), Vignette-Korrektur (radiale Aufhellung), Verzeichnung (Ein-Koeffizienten-Radialmodell), manuelle Transformation (Versatz/Skalierung/Seitenverhältnis/Rotation/Scherung als Perspektive-Näherung) — alle zu einer inversen Abbildung mit bilinearer Abtastung kombiniert, läuft nach Color Grading, noch vor der Farbraum-Konvertierung
  - [x] Scope-Präzisierung nachträglich als ADR-0030 dokumentiert (siehe `DECISIONS.md`): Ausgabegröße bleibt unverändert (Randpixel geklemmt, kein Zuschneiden — Schritt 2s größenverändernde Dispatch-Form damit für Schritt 9 nicht nötig, bleibt für Schritt 11 reserviert), Perspektive/Upright „Auto"/„Level"/„Vertical"/„Full" bleiben wirkungslose Platzhalter (echte Kantenerkennung ist eine CV-Aufgabe außerhalb des Stacks), „Guided" mittelt die ersten zwei Hilfslinien zu einer einfachen Dreh-Korrektur
  - [x] `CalibrationAdjustment`/`LensCorrectionAdjustment` bekommen ein `pub const NEUTRAL` (vorher nur `neutral()`-Fns) für die „kein zusätzlicher Durchlauf"-Optimierung in `develop.rs`
  - [x] Frontend: Objektivprofil-Dropdown, Auto-CA-Checkbox + 2 CA-Regler, Vignette-/Verzeichnungs-Regler, Perspektive/Upright-Dropdown, Hilfslinien-Zahlenfelder (statt Viewer-Klick-Interaktion, siehe ADR-0030) im Guided-Modus, 7 Regler für die manuelle Transformation
  - Verifiziert: `cargo fmt/clippy/test --workspace` (292 Rust-Tests), `tsc -b`, `vitest run` (100 Tests), `playwright test` (28/28), `vite build` — alles lokal grün

- [x] 10. Effekte
  - [x] Neues `crates/apx-pipeline/src/stages/effects.rs`/`.wgsl`, läuft nach den Objektivkorrekturen, noch vor der Farbraum-Konvertierung — positions-bewusst, aber ohne echten Nachbarschafts-Zugriff (beide Effekte sind reine Funktionen der Pixelposition)
  - [x] Nachträgliche Vignettierung: `roundness` blendet zwischen bildseitenverhältnis-passender Ellipse (`0`) und Kreis (`100`, negative Werte wirken wie `0`), `midpoint`/`feather` steuern eine `smoothstep`-Übergangszone, `highlights` schützt helle Pixel proportional zu ihrer Luminanz
  - [x] Körnung mit stabilem Pro-Pixel-Seed: deterministischer Ganzzahl-Hash aus der (auf `grain_size` heruntergerechneten) Pixelposition — reine Funktion der Position ohne Zeit-/Aufruf-Anteil, daher automatisch flackerfrei über beliebig viele Re-Renders; `roughness` verzerrt die Rauschverteilung über eine Potenzfunktion
  - [x] Frontend: 5 Vignettierung-Regler + 3 Körnung-Regler
  - Verifiziert: `cargo fmt/clippy/test --workspace` (302 Rust-Tests), `tsc -b`, `vitest run` (100 Tests), `playwright test` (29/29), `vite build` — alles lokal grün

- [x] 11. Geometrie (Crop/Rotation)
  - [x] Neues `crates/apx-pipeline/src/stages/geometry.rs` — bewusst CPU-only (kein GPU-Dispatch, analog zu `curves.rs`), als allerletzter Schritt in `develop::render_rgba8` verdrahtet: bilinear abgetastete Drehung um den Bildmittelpunkt (Randpixel geklemmt) gefolgt von pixel-genauer Zuschnitt-Rechteck-Extraktion (kein Resampling)
  - [x] `render_rgba8` liefert jetzt ein `RenderedImage { width, height, pixels }` statt eines nackten `Vec<u8>` — der einzige Schritt, der die Ausgabegröße ändert; `apx-app`s `compute_develop` rahmt entsprechend `rendered.width`/`.height` statt `linear.width`/`.height` (Wire-Format selbst unverändert, war schon immer breiten-/höhen-präfixiert)
  - [x] Scope-Präzisierung nachträglich als ADR-0030 ergänzt: Auto-Ausrichtung bleibt dokumentierter No-op-Platzhalter in dieser Stufe (EXIF-Ausrichtung läuft bereits in `apx-raw` vor der EDL-Pipeline)
  - [x] Neues `frontend/src/components/CropOverlay.tsx`: vier Ecken-Ziehgriffe (Zeigen+Tastatur, seitenverhältnis-gebunden wenn gesetzt) + Verschieben durch Ziehen im Inneren, 5 Rasterüberlagerungen als SVG-Linien (Drittel/Goldener Schnitt exakt, Diagonalen/Dreiecke/Spirale vereinfacht, siehe Moduldoku) — in `Viewer.tsx` über der angezeigten Bildfläche positioniert (`imageOrigin`/`effectiveScale`, dieselbe Zoom/Pan-Geometrie wie die WB-Pipette)
  - [x] Echter Bug gefunden und behoben: ein Tastendruck auf einem Ecken-Ziehgriff blubberte zum umschließenden Rechteck-Div hoch (beide `role="slider"` mit eigenem `onKeyDown`) — dessen "move"-Handler überschrieb mit veraltetem `crop`-Stand die gerade vorgenommene Größenänderung; behoben mit `stopPropagation()`
  - [x] Frontend: Winkel-Regler, Seitenverhältnis-/Raster-Dropdowns, Auto-Ausrichtung-Checkbox, "Freistellen"-Umschaltknopf
  - Verifiziert: `cargo fmt/clippy/test --workspace` (308 Rust-Tests), `tsc -b`, `vitest run` (100 Tests), `playwright test` (31/31), `vite build` — alles lokal grün

- [x] 12. Reparatur (Klonen/Reparieren)
  - [x] Neues `crates/apx-pipeline/src/stages/repair.rs`/`.wgsl`, läuft als allererster Schritt in `develop::render_rgba8` (vor Kalibrierung, direkt auf `linear.pixels`) — jeder `RepairStroke` wird als eigener sequenzieller Durchlauf angewendet statt als ein gemeinsamer Fused-Pass (unterschiedlich lange Pfade passen nicht in einen festen Gesamtstrich-Uniform-Puffer), Punktzahl je Strich auf `MAX_PATH_POINTS = 32` gedeckelt, beliebig viele Striche bleiben möglich
  - [x] `RepairParams`s `path`-Array nutzt ein auf 16 Byte aufgefülltes `PathPoint` (wie `hsl_color_mixer.rs`s `BandParams`/`RegionParams`) — WGSL verlangt für Arrays im `uniform`-Adressraum eine auf 16 Byte ausgerichtete Element-Schrittweite, ein rohes `[f32; 2]` hätte Rust- und Shader-Seite auseinanderlaufen lassen
  - [x] Klonen: bilinear abgetasteter, um einen festen Versatz verschobener Lesezugriff, `smoothstep`-weichgezeichnet am Rand von `radius`+`feather`; Reparieren: vereinfachtes Tiefpass/Hochpass-Überblenden (Tiefpass von der Quelle, Hochpass vom Ziel) statt echten Poisson-Blendings, siehe `repair.rs`s Moduldoku
  - [x] Frontend: neues `components/RepairOverlay.tsx` (erster Klick setzt den Quellpunkt, Ziehen malt den Zielpfad, SVG-Vorschau rein clientseitig — der Pipeline-Effekt committet erst beim Loslassen mit ausgedünntem Pfad), Store-Erweiterung (`repairActive`, `repairDraft*`, `repairPendingSource`, `addRepairStroke`, `removeRepairStroke`), DevelopPanel-Sektion mit Modus-Auswahl, Radius/Weiche-Kante/Deckkraft-Reglern und entfernbarer Strichliste
  - Verifiziert: `cargo fmt/clippy/test --workspace` (314 Rust-Tests), `tsc -b`, `vitest run` (100 Tests), `playwright test` (32/32), `vite build` — alles lokal grün

- [x] 13. Dokumentation, Tests, Abnahme
  - [x] `ARCHITECTURE.md`: Phase-4-Platzhalter in §7 durch echte Architekturbeschreibung in neuem §8 ersetzt (EDL v2, die drei Dispatch-Formen inkl. der ADR-0030-Korrektur gegenüber der Vorab-Planung, vollständige Pipeline-Reihenfolge, Frontend-Widgets)
  - [x] `FEATURES.md`: alle ~35 Phase-4-Zeilen in §3.2 auf Fertig (mit „Fertig (abweichend, siehe ADR-00XX)" für jede dokumentierte Vereinfachung); Reparatur-Zeile nachgetragen (war zuvor „Nicht begonnen“ stehen geblieben), Sensorflecken-Visualisierung auf Phase 6 umgetaggt (ADR-0028-Nachtrag)
  - [x] Volle Verifikation: `cargo fmt/clippy/test --workspace` (315 Rust-Tests), `tsc -b`, `vitest run` (100 Tests), `playwright test` (32/32), `vite build` — alles lokal grün
  - [x] 16-ms-Performance-Nachmessung (neuer Test `render_rgba8_timing_with_all_phase4_stages_active`, alle zehn Werkzeugkategorien gleichzeitig auf einen deutlich von neutral abweichenden Wert gesetzt statt nur des Phase-2-Grundeinstellungs-Kerns): ~2,3 s (GPU, `llvmpipe`-Software-Rasterisierer dieser Sandbox) bzw. ~1,3 s (CPU-Fallback, rayon) bei 2048×1365 — deutlich über dem 16-ms-Ziel im *Alle-Regler-gleichzeitig-Extremfall*. Das ist erwartbar und kein Regressions-Fund: der „Regelfall überspringen"-Kurzschluss (siehe `develop.rs`s Moduldoku) sorgt dafür, dass ein *typischer* Regler-Tick (ein bis zwei Werkzeuge gleichzeitig aktiv, der Rest neutral) weiterhin nur die tatsächlich betroffenen Stufen durchläuft — die ursprüngliche Phase-2-Kern-Messung bleibt dafür repräsentativ (~250–480 ms auf `llvmpipe`, siehe `render_rgba8_timing_on_synthetic_standard_edge_image`). Auf echter GPU-Hardware (kein Software-Rasterisierer) wären beide Werte gemäß `ARCHITECTURE.md`s Dispatch-Kostenmodell deutlich niedriger; eine scharfe Zahl dafür fehlt dieser Sandbox mangels echter Fenster-/IPC-/Compositing-Umgebung (dieselbe Einschränkung wie schon bei Phase 2 Schritt 7 dokumentiert).
  - [x] Commit+Push, CI-Check auf allen drei Plattformen, ehrlicher Abschlussbericht

### Nicht in Phase 4 (bewusst zurückgestellt)
Siehe ADR-0028 (plus Nachtrag): Workflow-Punkte (Schnappschüsse, Vorher/Nachher, Copy/Paste-Einstellungen, Sync, Auto-Sync, Referenzansicht, Soft-Proof), echter Adobe-Profil-Import (Objektivprofile + DCP-Kameraprofile), Auto-Quellenfindung, inhaltsbasiertes Füllen und Sensorflecken-Visualisierung für die Reparatur-Funktion — alle auf Phase 6 verschoben. Phase 5 ist laut `SPEC.md` §5 das Preset-/Template-System (§3.5), nicht die oben genannten Workflow-Punkte — siehe `ARCHITECTURE.md` §7.

## Abgeschlossene Phase: Phase 5 — Preset- und Template-System

`SPEC.md` §5 nennt wörtlich nur „Preset- und Template-System"; §3.5 (der volle Feature-Katalog) reicht deutlich weiter als in dieser Phase sinnvoll baubar — siehe `DECISIONS.md` ADR-0031 für die Scope-Präzisierung (Preset-Grundlagen + vereinfachte bedingte Presets + vorgezogene Import-/Umbenennungs-Templates jetzt; KI-Generator auf Phase 7, Adobe-Interop und der übrige Templates-Unterabschnitt auf spätere Phasen verschoben; kein eigenes `apx-presets`-Crate, siehe ADR-0031 Punkt 6).

**Architektur-Grundsatz:** ein Preset ist reine Katalogdaten — Name, Ordner, Favorit, Tags, Bedingungsregeln, und eine EDL-*Teilmenge* als opakes JSON (analog zu `edit_history.edl_json`, siehe `ARCHITECTURE.md` §5). `apx-catalog` muss den EDL-Teilmengen-Inhalt nie verstehen; das Zusammenführen in `developEdl` (inkl. Stärke-Skalierung, Stapel-Anwendung, Bedingungsauswertung) passiert ausschließlich im Frontend, vor dem bereits bestehenden `commitDevelopEdit()`.

- [ ] 0. Scope festzurren
  - [x] `DECISIONS.md`: neues ADR-0031 (die Scope-Entscheidungen oben)
  - [x] `FEATURES.md` §3.5 umgetaggt (Preset-Grundlagen bleiben Phase 5, KI-Generator→Phase 7, Adobe-Interop→Phase 6, Templates-Unterabschnitt→größtenteils Phase 8 außer Import-/Umbenennungs-Templates→Phase 5)
  - [x] `ARCHITECTURE.md` §7s Phase-5-Zeile präzisiert (kein `apx-presets`-Crate)
  - [ ] `PLAN.md`: dieser Abschnitt

- [ ] 1. Datenmodell: `apx-catalog`-Migration + Repository
  - [ ] Neue `apx_core`-ID-Typen: `PresetFolderId`, `PresetId`, `PresetVersionId` (via `define_id_type!`)
  - [ ] Neue Migration `0004_presets.sql`: `preset_folders` (id, name, parent_id, position — Baum wie `folders`), `presets` (id, folder_id, name, is_favorite, tags als JSON-Array-TEXT, condition_rules als JSON-TEXT, created_at), `preset_versions` (id, preset_id, sequence, edl_subset_json, created_at — jede Speicherung eine neue Version, wie `edit_history`)
  - [ ] `repository::presets.rs`: CRUD für Ordner/Presets/Versionen, `list_tree`/`list_by_folder`/`search_by_name_or_tag`, `create_version`/`list_versions`/`latest_version`
  - [ ] `models.rs`: `PresetFolder`, `Preset`, `PresetVersion` Structs
  - [ ] Tests: Baum-Hierarchie (wie `folders.rs`s Kaskaden-Test), Versions-Sequenz, Tag-Suche

- [ ] 2. `apx-app`-Commands + DTOs
  - [ ] `create_preset_folder`/`rename_preset_folder`/`delete_preset_folder`/`list_preset_folders`
  - [ ] `create_preset` (Name, Ordner, EDL-Teilmengen-JSON, Tags, Bedingungsregeln) → legt Preset + erste Version an
  - [ ] `update_preset` (überschreibt Metadaten und/oder legt neue Version an), `delete_preset`, `list_presets`, `toggle_preset_favorite`
  - [ ] `list_preset_versions`/`get_preset_version` (für Diff-Ansicht)
  - [ ] `export_preset_to_apx_file`/`import_preset_from_apx_file` (Tauri-Dateidialog, eigenes `.apx`-JSON-Format: `{schema_version, name, tags, condition_rules, edl_subset}`)

- [ ] 3. Frontend-Grundgerüst: Datenmodell + Presets-Panel
  - [ ] `frontend/src/lib/presets.ts`: TS-Typen (`PresetFolder`, `Preset`, `PresetVersion`, `EdlSubset` = `Partial<EdlPayload>`-artiges Objekt mit einem `included: Set<EdlSectionKey>`-Begleitfeld), `.apx`-Schema-Typ
  - [ ] Store-Slice `presets`: Ordnerbaum + Presetliste laden, Auswahl, Suche/Tag-Filter
  - [ ] Neues `components/PresetsPanel.tsx` (Ordnerbaum + Liste, analog zum bestehenden Sammlungen-Muster aus Phase 3) als neuer Tab/Abschnitt neben dem Entwickeln-Panel

- [ ] 4. Preset speichern
  - [ ] Neues `components/SavePresetDialog.tsx`: Checkbox je Einstellungsgruppe (die zehn Phase-4-Sektionen, Reparatur ausgenommen — bildspezifische Striche sind kein „Look"), Name/Ordner/Tags-Eingabe
  - [ ] Extrahiert die ausgewählten Sektionen aus `developEdl` in ein `EdlSubset`-Objekt, ruft `create_preset` auf

- [ ] 5. Preset anwenden + Stärke + Stapel
  - [ ] Anwenden: mischt `EdlSubset`-Felder in `developEdl`, numerische Felder bei Stärke ≠ 100 % linear zur jeweiligen Neutral-Konstante hin skaliert, kategoriale Felder (Enums/Strings) unskaliert übernommen
  - [ ] Stärke-Regler (0–200 %) bleibt nachträglich änderbar, solange seit dem Anwenden kein anderer Edit committet wurde (Store merkt sich `lastAppliedPreset { presetId, strength, baseEdl }`, jeder andere Setter löscht diesen Zustand)
  - [ ] Preset-Stapel: kleine geordnete Liste ausgewählter Presets, sequenziell angewendet, Reihenfolge per Drag oder Auf/Ab-Knöpfen änderbar

- [ ] 6. Live-Vorschau
  - [ ] Hover über einen Preset-Eintrag rendert ihn testweise in den Viewer (`useDevelopRender` mit einem temporären, nicht committeten EDL), verlässt die Maus den Eintrag ohne Klick, kehrt die Vorschau zum vorherigen Zustand zurück
  - [ ] Thumbnail je Preset-Eintrag in der Liste (kleine Vorschau-Auflösung, gleicher Renderpfad)

- [x] 7. Bedingte Presets (vereinfacht, siehe ADR-0031 Punkt 4)
  - [x] Feste Feldliste (ISO, Blende, Brennweite, Kameramodell, Objektiv — bereits in `photos` vorhanden), Operatoren (`>`, `<`, `=`, „enthält"), UND-verknüpft
  - [x] Kleiner Regel-Editor in `SavePresetDialog.tsx`; Auswertung beim Anwenden gegen die Metadaten des aktuellen Fotos. Jede Regel trägt zusätzlich eine optionale Sektion: „Ganzes Preset" (schlägt sie fehl, wird das komplette Preset nicht angewendet) oder eine einzelne Sektion (schlägt sie fehl, wird nur diese Sektion ausgeschlossen, der Rest des Presets bleibt wirksam) — Erweiterung ggü. der ursprünglichen Formulierung, die nur den sektionsbezogenen Fall nannte, aber der praxisnähere Fall „ganzes Preset nur unter Bedingung X" (z. B. „nur für Teleobjektive") ist mindestens genauso häufig und ließ sich ohne Mehraufwand mitbauen.

- [x] 8. Versionierung + Diff-Ansicht
  - [x] Neuer `PresetVersionsDialog.tsx`: „Aktuellen Stand als neue Version speichern" legt eine neue `preset_versions`-Zeile an (alte bleiben erhalten, `add_preset_version` war bereits seit Schritt 2 im Backend vorhanden, aber ungenutzt) — übernimmt dieselben Sektionen wie die bisher aktuellste Version
  - [x] Kleine Diff-Ansicht: zwei Versionen per Dropdown wählen, `lib/presets.ts`s `diffEdlSubsets` listet jedes abweichende Blattfeld (rekursiv in verschachtelte Objekte, Arrays als atomarer Wert — dieselbe Konvention wie `interpolateValue`)

- [x] 9. Import-Templates + Umbenennungs-Templates (vorgezogen aus Phase 3, siehe ADR-0031 Punkt 7)
  - [x] Neuer `ImportDialog.tsx` (geöffnet über einen zusätzlichen „Import mit Vorlage…"-Knopf, additiv zum unveränderten einfachen „Ordner importieren"-Knopf) bindet `import_folder_with_mode` sowie `list_import_presets`/`save_import_preset`/`delete_import_preset` ans Frontend an — diese Commands existierten seit Phase 3 im Backend, hatten aber bis jetzt **keine** Frontend-Anbindung (die `FEATURES.md`-Zeilen „Import mit Kopieren/Verschieben/Hinzufügen" und „Import-Presets" waren entsprechend vorschnell auf Fertig markiert; korrigiert in Schritt 10)
  - [x] Token-Editor für `rename_pattern` (Knöpfe für `{date}`/`{seq}`/`{camera}`/`{original}`, Live-Vorschau eines Beispieldateinamens über `lib/renamePattern.ts`, das dieselbe Ersetzungslogik wie `crates/apx-app/src/import/rename.rs` rein clientseitig für die Anzeige nachbildet)

- [x] 10. Dokumentation, Tests, Abnahme
  - [x] `ARCHITECTURE.md`: neues §9 „Architektur Phase 5" (Datenfluss Speichern/Anwenden/Stärke/Stapel/Bedingungen/Versionierung/Import-Templates, analog zu §5/§6/§8); §7s Phase-5-Platzhalter entfernt, Phase-7/8–9-Zeilen um die dorthin verschobenen ADR-0031-Punkte ergänzt
  - [x] `FEATURES.md`: alle jetzt gebauten §3.5-Zeilen auf Fertig; zusätzlich drei vorschnell aus Phase 3 auf Fertig markierte Zeilen korrigiert („Import mit Kopieren/Verschieben/Hinzufügen", „Import-Presets", „Automatisches Umbenennen mit Token-System" — Backend existierte seit Phase 3, Frontend fehlte bis Schritt 9 komplett) sowie eine falsch getaggte Zeile („Import mit DNG-Konvertierung": Phase 5 → Phase 8, ADR-0025 tippte noch auf „Phase 5 (Export/Publish)", bevor `SPEC.md` §5 Phase 5 als Preset-System festlegte)
  - [x] **Nachgezogene Lücke:** `.apx`-Export/-Import hatte seit Schritt 2 fertige Backend-Commands (`export_preset_to_apx_file`/`import_preset_from_apx_file`) und sogar bereits Wrapper-Funktionen in `lib/tauri.ts`, aber **keine** UI — in keinem Schritt 3–9 verdrahtet (Lücke, kein bewusster Scope-Schnitt). In Schritt 10 nachgeholt: Export-Knopf je Preset-Zeile, Import-Knopf im Panel-Kopf (`store/index.ts`s `exportPresetAsApxFile`/`importPresetFromApxFile`)
  - [x] Volle Verifikation (`cargo fmt/clippy/test`, `tsc -b`, `vitest`, `playwright`, `vite build`), Commit+Push, CI-Check, ehrlicher Abschlussbericht

### Nicht in Phase 5 (bewusst zurückgestellt)
Siehe ADR-0031: Preset-Generator (KI: LLM-Anfrage, Referenzbild-Modus, Variationen-Generator, Preset-aus-Bearbeitung-Lernen) → Phase 7; Adobe-`.xmp`/`.lrtemplate`-Import/-Export → spätere Phase; Export-/Wasserzeichen-/Metadaten-/Layout-/Workflow-Templates + Template-Marktplatz → Phase 8–9 (setzen die dort erst gebaute Export-Engine voraus).

## Aktuelle Phase: Phase 6 — Masken und lokale Anpassungen

`SPEC.md` §5 nennt wörtlich nur „Masken und lokale Anpassungen. Pinsel,
Verläufe, Bereichsmasken, Maskenkombination, Ebenen-Mischmodi." — siehe
`DECISIONS.md` ADR-0032 für die Scope-Präzisierung: Maskensystem-Kern
(ohne Tiefenbereich/KI-Masken) plus die acht in ADR-0028 explizit für
diese Phase versprochenen Workflow-Punkte; der Bibliotheks-Backlog aus
§3.1 (keine ADR hatte ihn je Phase 6 zugesagt) wandert nach Phase 9, die
Reparatur-Erweiterungen und KI-Masken nach Phase 7.

**Architektur-Grundsatz (ADR-0032 Punkt 4):** Ebenenmodell statt
Fused-Pass — die Phase-4-Pipeline (`render_rgba8`) bleibt unverändert die
Grundlage; Masken laufen danach als neue, letzte Stufengruppe, jede Maske
sequenziell: Maskenalpha berechnen → mit vorangehenden Masken derselben
Gruppe kombinieren → die Maskenwerkzeuge (Grundeinstellungen, Kurven,
HSL, Farbmischer, Color Grading, Details — siehe ADR-0032 Punkt 2) auf
eine Bildkopie anwenden → alpha-gewichtet mit dem gewählten
Ebenen-Mischmodus zurückmischen.

- [ ] 0. Scope festzurren
  - [x] `DECISIONS.md`: neues ADR-0032
  - [x] `FEATURES.md` §3.1/§3.3/§3.4 umgetaggt (Bibliotheks-Backlog → Phase 9, Reparatur-Erweiterungen/KI-Masken → Phase 7, Tiefenbereich zurückgestellt, Adobe-Interop-Zeile korrigiert auf Phase 8–9)
  - [x] `ARCHITECTURE.md` §7s Phase-6/7-Zeilen präzisiert
  - [x] `PLAN.md`: dieser Abschnitt

- [x] 1. Datenmodell: EDL-Schema v3
  - [x] `crates/apx-pipeline/src/edl/v3.rs`: `Mask` (id, name, `components: Vec<MaskComponent>` — jede Komponente eigene Geometrie + `MaskCombine` + `invert`, `adjustments: MaskAdjustments` mit Grundeinstellungen/Kurven/HSL/Farbmischer/Color Grading/Details, opacity, feather, invert, blend_mode, group_id, visible, overlay_color), `MaskGeometry`-Enum (Brush/LinearGradient/RadialGradient/ColorRange/LuminanceRange), `BlendMode`-Enum (Normal/Multiply/SoftLight/Color/Luminosity), `MaskGroup`; `EdlV3 { ..EdlV2-Felder, masks: Vec<Mask>, mask_groups: Vec<MaskGroup> }` — feingranularer als ursprünglich geplant: eine Maske besteht aus mehreren kombinierbaren Komponenten (`SPEC.md` §5 „Maskenkombination" als eigener Punkt, nicht nur ein Maskentyp pro Maske)
  - [x] `migrate.rs`: `v2_to_v3`/`from_v2` (masks/mask_groups starten leer), `from_envelope` probiert v3 zuerst, danach v2, danach v1 (alle drei Versionen bleiben lesbar)
  - [x] Tests: v1→v3- und v2→v3-Upgrade-Rundreise, Mask-JSON-Roundtrip mit mehreren Komponententypen
  - [x] `frontend/src/lib/edl.ts`: gespiegelte TS-Typen (`Mask`, `MaskGeometry`, `MaskComponent`, `MaskAdjustments`, `BlendMode`, `MaskGroup`) + Neutral-Konstanten + `newBrushMask`/`emptyBrushGeometry`-Builder; `PresetSectionKey` (Phase 5) schließt `masks`/`mask_groups` mit aus (dieselbe Begründung wie bei `repair`)

- [x] 2. Pipeline-Architektur: Maskenalpha-Grundgerüst + Anwenden + Zurückmischen
  - [x] Neues `stages/masks.rs`: Maskenalpha-Berechnung **für alle fünf Geometrietypen bereits vollständig implementiert** (nicht nur ein Grundgerüst) — CPU-only in diesem Schritt, siehe Modul-Nachtrag unten für die Begründung; GPU-Beschleunigung + Frontend-Interaktion kommen erst in Schritt 3–5, je Geometriegruppe
  - [x] Kombinationslogik (`MaskCombine::Add` = Vereinigung/Maximum, `Subtract` = `c·(1-a)`, `Intersect` = `c·a`) über die Komponenten *derselben* Maske
  - [x] Zurückmischen: alpha-gewichtete Interpolation zwischen unverändertem und maskiert-bearbeitetem Bildzustand — `BlendMode` ist bereits vollständig als Enum da, aber nur `Normal` ist tatsächlich implementiert; die übrigen vier Modi fallen bis Schritt 6 auf denselben linearen Mix zurück (dokumentiert in `masks.rs`, kein `todo!()`)
  - [x] `develop.rs`: Masken-Stufengruppe **direkt nach `effects`, vor der Farbraum-Konvertierung** eingehängt (Korrektur ggü. der ursprünglichen Formulierung „nach der Phase-4-Pipeline" — siehe Nachtrag unten), pro Maske die bestehenden Stufenfunktionen (`basic_fused`/`local_contrast`/`details`/`hsl_color_mixer`/`color_grading`/neue `curves::apply_linear_rgb`) mit den Masken-EDL-Werten statt der globalen wiederverwendet
  - [x] Test mit einer Test-Maske (voll deckend, ganzes Bild — ein `RadialGradient` mit Radius 10 und Feather 0) bestätigt: Ergebnis identisch zu einer globalen Anwendung derselben Werte
  - **Nachtrag (während des Bauens entdeckt):** die ursprüngliche Formulierung „Masken laufen nach der Phase-4-Pipeline" war ungenau — Kurven laufen in der globalen Pipeline erst *nach* der Farbraum-Konvertierung auf dem fertigen RGBA8-Puffer (`curves::apply_rgba8`), während Grundeinstellungen/HSL/Color Grading/Details im linearen Arbeitsraum *davor* laufen. Da eine Maske alle sechs Werkzeuge in einem einzigen Durchlauf anwendet, kann sie nicht an zwei verschiedenen Pipeline-Stellen zugleich sitzen. Entscheidung: die gesamte Maskenstufe läuft im linearen Arbeitsraum (nach `effects`, vor der Konvertierung); Masken-Kurven bekommen dafür eine neue `curves::apply_linear_rgb`-Funktion, die dieselbe LUT auf dem linearen Wert statt dem display-referred Tonwert anwendet — eine bewusste, dokumentierte Vereinfachung (siehe `masks.rs`s Moduldoku), die eine verlustreiche zweite Farbraum-Konvertierung pro Maske vermeidet.

- [x] 3. Maskentyp Linearer Verlauf + Radialer Verlauf
  - [x] Analytische Alpha-Funktion (Position relativ zu Start/Ende bzw. Mittelpunkt/Radien) — bereits vollständig in Schritt 2 gebaut (`stages/masks.rs::linear_gradient_alpha`/`radial_gradient_alpha`), CPU-only, GPU-Parität bleibt zurückgestellt bis Schritt 11s Performance-Messung (siehe Schritt 2)
  - [x] Viewer-Overlay: ziehbare Kontrollpunkte — neues `MaskOverlay.tsx` (analog zu `CropOverlay.tsx`: Ziehgriffe mit Pointer-Capture + Tastatur-Feinsteuerung, `role="slider"`)
  - **Bewusste Vereinfachung:** der Radialverlauf-Ziehgriff steuert nur einen einzelnen, gemeinsamen Radius (kreisförmig, `radius_x == radius_y`) statt unabhängiger Ellipsen-Achsen + Rotation — das Datenmodell (`MaskGeometry::RadialGradient`) hat bereits eigene Felder dafür, eigene Achsen-/Rotations-Ziehgriffe kommen erst bei Bedarf in einem späteren Schritt (siehe `MaskOverlay.tsx`s Moduldoku)
  - **Vorgezogen aus Schritt 7:** die Masken-Grundlagen (`MasksSlice` im Store, `MasksPanel.tsx` mit Liste/Anlegen/Sichtbarkeit/Umbenennen/Löschen, ein kleiner Ausschnitt der Pro-Maske-Regler — Deckkraft/Weichzeichnung + Belichtung/Kontrast) mussten hier bereits gebaut werden, weil eine Ziehgriff-Maske ohne Verwaltung nicht sichtbar/testbar ist. Schritt 7 bleibt für die tatsächliche Politur: Gruppen, Duplizieren, Foto-Übertragung, wiederverwendbare Bausteine, Drag-&-Drop-Sortierung, und die volle Sechs-Sektionen-Reglerabdeckung (aktuell nur zwei von zwölf Grundeinstellungs-Reglern)
  - [x] Tests: `frontend/e2e/masks-flow.spec.ts` (5 Tests: Anlegen+Liste+Commit, Ziehgriff-Tastatursteuerung je Verlaufstyp, Sichtbarkeit/Umbenennen/Löschen, maskeneigener Regler wirkt nur auf die Maske) — volle Kette (`tsc -b`, `vitest run` 149/149, `playwright test` 50/50, `vite build`) grün

- [x] 4. Maskentyp Pinsel
  - [x] Stempel-Akkumulation ähnlich `stages/repair.rs`s Pfad-Ansatz, aber als eigenständige weiche Maske (kein Klon-Versatz) — bereits vollständig in Schritt 2 gebaut (`stages/masks.rs::brush_alpha`/`distance_to_stroke`), CPU-only, GPU-Parität bleibt wie bei den Verlaufstypen zurückgestellt bis Schritt 11
  - [x] Viewer-Pinsel-Interaktion (Radius/Weichzeichnung je Strich als Entwurfsregler im `MasksPanel.tsx`, analog zu `DevelopPanel.tsx`s `repairDraft*`-Feldern; Klicken+Ziehen im Bild malt einen `BrushStroke`, `MaskOverlay.tsx` zeigt bestehende Striche + Live-Vorschau des aktuellen Strichs, analog zu `RepairOverlay.tsx`)
  - **Bewusste Vereinfachung ggü. der ursprünglichen Formulierung:** kein Deckkraft-Regler pro Strich (Deckkraft existiert bereits pro *Maske*, siehe Schritt 3) und kein Hinzufügen-/Subtrahieren-Umschalter direkt beim Malen (jede neue Maske startet mit einer einzelnen `Add`-Komponente, siehe `newMask`) — echte Maskenkombination mit mehreren Komponenten unterschiedlicher `MaskCombine`-Modi ist genau Schritt 6s Thema und würde hier vorgezogen unnötig Komplexität in die Pinsel-UI ziehen, bevor die Mehrfach-Komponenten-Verwaltung existiert
  - [x] Tests: `frontend/e2e/masks-flow.spec.ts` (1 weiterer Test: Ziehvorgang im Bild malt einen Strich und committet, Entfernen committet erneut) — volle Kette (`tsc -b`, `vitest run` 149/149, `playwright test` 51/51, `vite build`) grün; keine Rust-Änderungen in diesem Schritt (Backend war bereits in Schritt 2 vollständig)

- [x] 5. Maskentyp Farbbereich + Luminanzbereich
  - [x] Pro-Pixel-Klassifikation — bereits vollständig in Schritt 2 gebaut (`stages/masks.rs::color_range_alpha`/`luminance_range_alpha`), CPU-only, GPU-Parität bleibt wie bei den anderen Geometrietypen zurückgestellt bis Schritt 11
  - [x] Farbbereich: Bildklick nimmt Referenzfarbe auf (`maskColorRangePickerActive`/`toggleMaskColorRangePicker`/`setMaskColorRangeTargetAt` in `store/index.ts`), teilt den Viewer-Sampling-Code mit der Farbmischer-/WB-Pipette-Infrastruktur aus Phase 4 (`Viewer.tsx`s `handleImageClick`); Toleranz-/Weichzeichnung-Regler in `MasksPanel.tsx`
  - [x] Luminanzbereich: Regler für untere/obere Grenze + Weichzeichnung in `MasksPanel.tsx`, direkt auf `updateMaskGeometry`/`commitMaskDrag` (kein Bildklick nötig)
  - **Bewusste Vereinfachung:** `masks.rs`s `ColorRange` vergleicht im linearen Arbeitsraum (siehe dessen Moduldoku aus Schritt 2), der Bildklick liefert aber den bereits gerenderten, display-referred Vorschau-Frame (RGBA8-Byte, `/255` normiert) — dieselbe Näherung, die die Weißabgleich-Pipette/der Farbmischer seit Phase 4 verwenden, hier auf einen dritten Aufrufer ausgedehnt
  - [x] Tests: `frontend/e2e/masks-flow.spec.ts` (2 weitere Tests: Farbbereich-Bildklick + Toleranz-Commit, Luminanzbereich-Reglerwerte) — volle Kette (`tsc -b`, `vitest run` 149/149, `playwright test` 53/53, `vite build`) grün; keine Rust-Änderungen in diesem Schritt

- [x] 6. Maskenkombination + vollständige Ebenen-Mischmodi
  - [x] Alle in `SPEC.md` genannten Mischmodi (Multiplizieren, Weiches Licht, Farbe, Luminanz) im Zurückmisch-Schritt aus Schritt 2 ergänzt (`stages/masks.rs::blend_pixel` + `soft_light_channel`/`luminosity`/`set_luminosity`/`clip_color`, nach dem Photoshop-/W3C-Compositing-Standardverfahren „SetLum"/„ClipColor") — GPU-Parität bleibt wie bei allen Masken-Bausteinen zurückgestellt bis Schritt 11 (kein GPU-Pfad existiert für die Maskenstufe insgesamt, siehe Schritt 2)
  - **Bewusste Vereinfachung:** die Blend-Formeln setzen einen ungefähr `0.0..=1.0`-Wertebereich voraus (Standardverfahren für Ebenen-Mischmodi); im linearen Arbeitsraum können helle Lichter das überschreiten — `clip_color` faltet das zurück statt es unverändert durchzureichen (dokumentiert in `masks.rs`s Moduldoku)
  - [x] Mehrere Komponenten je Maske (`SPEC.md` §5 „Maskenkombination" als eigener Punkt): `MasksPanel.tsx` bekommt eine „Komponenten"-Liste je Maske (Geometrietyp, Verrechnung Hinzufügen/Subtrahieren/Schneiden, Invertieren, Entfernen) + „+ Komponente: …"-Knöpfe je der fünf Geometrietypen; `selectedMaskComponentIndex` (neu im Store) bestimmt, welche Komponente der Viewer gerade bearbeitet (Ziehgriffe/Pinsel-Malen/Farbklick betreffen immer die aktive Komponente, nicht mehr fest `components[0]`)
  - [x] Mischmodus-Auswahl (`BLEND_MODE_OPTIONS`, bereits seit Schritt 1 im EDL vorhanden) in `MasksPanel.tsx` verdrahtet (`setMaskBlendMode`)
  - [x] Tests: 8 neue Rust-Unit-Tests für `blend_pixel`/`soft_light_channel`/`luminosity`/`set_luminosity` (Normal/Multiply/SoftLight/Color/Luminosity je mit einer nachprüfbaren mathematischen Eigenschaft, plus Clipping) + 1 neuer e2e-Test (zweite Komponente hinzufügen, Verrechnung+Invertieren committen, Mischmodus committen, Komponente entfernen) — volle Kette grün: `cargo fmt --check`/`clippy -D warnings -D clippy::unwrap_used`/`cargo test --workspace` (alle Crates), `tsc -b`, `vitest run` 149/149, `playwright test` 54/54, `vite build`

- [x] 7. Frontend: Maskenverwaltung + Pro-Maske-Regler
  - [x] `MasksPanel.tsx`: Liste mit Drag-&-Drop-Sortierung (native HTML5-DnD, `reorderMask` — die Reihenfolge ist zugleich die Anwendungsreihenfolge, Umsortieren kann das Ergebnis also tatsächlich ändern, nicht nur die Anzeige), Umbenennen, Sichtbarkeit, Duplizieren (`duplicateMask`, tiefe Kopie mit „(Kopie)"-Suffix), auf anderes Foto übertragen (`transferMaskToPhoto`, lädt/speichert über denselben `current_develop_edit`/`apply_develop_edit`-Pfad wie `loadDevelopStateForPhoto`), als wiederverwendbarer Baustein speichern (`maskBuildingBlocks`)
  - [x] Maskengruppen (`SPEC.md` §3.3): anlegen/umbenennen/entfernen/Sichtbarkeit — **Nachtrag (beim Bauen entdeckt):** `stages/masks.rs::apply_all` prüfte bisher nur `mask.visible`, nicht die Gruppensichtbarkeit — `visible_masks` (bereits seit Schritt 2 vorbereitet) war nie eingehängt. In diesem Schritt korrigiert (`apply_all` nimmt jetzt `groups: &[MaskGroup]` entgegen und filtert darüber), sonst hätte die neue Gruppen-UI ausblendbare Gruppen versprochen, ohne dass es einen Effekt gehabt hätte
  - [x] Pro-Maske-Reglerabschnitt: **volle Sechs-Sektionen-Abdeckung** (Grundeinstellungen alle zwölf statt zwei, Kurven, HSL, Farbmischer, Color Grading, Details) — reuse derselben `DevelopSlider`/`CurveEditor`/`ColorWheel`-Komponenten aus Phase 4, auf `mask.adjustments` statt `developEdl` gerichtet; `CURVE_CHANNEL_TABS`/`COLOR_GRADING_WHEEL_TABS`/`DetailsSliderKey` aus `DevelopPanel.tsx` nach `lib/edl.ts` verschoben und exportiert (dieselbe Wiederverwendung wie das bereits dort liegende `HSL_BAND_TABS`), damit beide Panels sie teilen statt zu duplizieren
  - **Bewusste Vereinfachung ggü. der ursprünglichen Formulierung:** Überlagerungsfarbe (`overlay_color`, bereits im EDL seit Schritt 1) bekommt in diesem Schritt keine UI — sie steuert nur eine Masken-Overlay-Voransicht im Viewer, die es noch gar nicht gibt (die aktuellen Overlays zeigen Ziehgriffe/Pfade, keine Flächenfarbe); ohne diese Voransicht wäre ein Farbwähler nur toter Zustand. Bausteine sind bewusst nur clientseitig für diese Sitzung gehalten statt über die Presets-Katalog-Infrastruktur aus Phase 5 (Ordner/Versionen/SQLite) — ein katalogseitiges Pendant wäre dieselbe Größenordnung an Aufwand wie das gesamte Presets-System und hätte diesen ohnehin schon großen Schritt gesprengt; bei echtem Bedarf ein eigener späterer Schritt
  - [x] Tests: 1 neuer Rust-Unit-Test (`a_mask_in_an_invisible_group_is_excluded...`) + 7 neue e2e-Tests (Sechs-Sektionen-Regler HSL/Details/ColorGrading, Farbmischer-Bildklick, Gruppen anlegen/zuordnen/ausblenden/entfernen, Duplizieren, Baustein speichern+anwenden, auf anderes Foto übertragen) — volle Kette grün: `cargo fmt --check`/`clippy -D warnings -D clippy::unwrap_used`/`cargo test --workspace`, `tsc -b`, `vitest run` 149/149, `playwright test` 60/60, `vite build`

- [x] 8. Workflow: Schnappschüsse + Vorher/Nachher
  - [x] Schnappschüsse: benannte, klickbare EDL-Zwischenstände zusätzlich zum linearen Verlauf
  - **Korrektur gegenüber der ursprünglichen Plan-Formulierung** ("kein neues Backend-Konzept nötig — ein Schnappschuss ist ein benannter Verweis auf einen bestehenden Verlaufs-Stand"): beim Nachlesen von `repository/edits.rs::commit` zeigte sich, dass `edit_history`-Zeilen *nicht* stabil sind — ein Commit nach einem Rückgängig löscht jede „Zukunft" hart (ADR-0014). Ein Verweis auf eine solche Zeile könnte also verschwinden, sobald man über einen Schnappschuss hinaus weiterbearbeitet — genau das Gegenteil von „zusätzlich zum linearen Verlauf" (PLAN.md, ursprüngliche Formulierung). Deshalb doch ein neues, aber sehr kleines Konzept: eine eigene `snapshots`-Tabelle (`migrations/0005_snapshots.sql`) mit einer eigenen EDL-Kopie je Schnappschuss, unabhängig vom linearen Verlauf — `apx-catalog::repository::snapshots` (5 Tests, inkl. eines Tests, der genau das ursprünglich übersehene Szenario abdeckt), `apx-app`-Commands (`create_snapshot`/`list_snapshots`/`rename_snapshot`/`delete_snapshot`, kein eigener „restore" nötig — reuse von `apply_develop_edit`), Frontend (`DevelopPanel.tsx`-Abschnitt „Schnappschüsse")
  - [x] Vorher/Nachher in vier Ansichten (Links/Rechts, Oben/Unten, Geteilt, Geteilt vertikal) im Viewer — neues `BeforeAfterView.tsx`, „Vorher" ist das neutrale EDL, „Nachher" der aktuelle `developEdl`-Stand, beide über `useDevelopPreviewThumbnail` gerendert (derselbe Mechanismus wie die Preset-Live-Vorschau aus Phase 5) und per Canvas-2D/`putImageData` gezeichnet (bewusst kein WebGL — keine Zoom/Pan-Transformation nötig)
  - **Bewusste Vereinfachung:** die Trennlinie der geteilten Modi sitzt fest bei 50 % (kein ziehbarer Regler) — `SPEC.md` nennt nur die vier Ansichten, keinen ziehbaren Trennbalken
  - **Auslegungsentscheidung:** `SPEC.md` §3.4 nennt nur "links/rechts, geteilt, oben/unten, geteilt vertikal" ohne weitere Erklärung — ausgelegt als Lightroom-analog: Links/Rechts und Oben/Unten zeigen zwei vollständige Bilder, „Geteilt"/„Geteilt vertikal" sind eine einzelne per Trennlinie geteilte Fläche (vertikale bzw. horizontale Trennlinie)
  - [x] Tests: 5 neue Rust-Unit-Tests (`repository::snapshots`) + 2 neue e2e-Tests (neues `workflow-flow.spec.ts`: Schnappschuss speichern/wiederherstellen/umbenennen/löschen; Vorher/Nachher-Modi schalten korrekt um) — volle Kette grün: `cargo fmt/clippy/test` (Workspace), `tsc -b`, `vitest run` 149/149, `playwright test` 62/62, `vite build`

- [x] 9. Workflow: Copy/Paste + Vorherige übernehmen + Sync + Auto-Sync
  - [x] Einstellungen kopieren/einfügen mit granularer Sektionsauswahl (derselbe `PresetEdlSubset`-Mechanismus wie Presets, direkt aus `developEdl` statt einem gespeicherten Preset — `copiedEdlSubset`/`copyDevelopSettings`/`pasteDevelopSettings` im Store, Checkbox-Liste über `PRESET_SECTION_KEYS`/`PRESET_SECTION_LABELS` aus Phase 5 wiederverwendet)
  - [x] Vorherige übernehmen: `lastDevelopPhotoId` merkt sich beim Öffnen/Wechseln im Entwickeln-Modul (`loadDevelopStateForPhoto`) das zuvor offene Foto; „Vorherige übernehmen" lädt dessen letzten committeten Stand über `current_develop_edit` und schreibt ihn per `apply_develop_edit` auf das aktuelle Foto
  - **Auslegungsentscheidung:** „Vorherige" ist ausgelegt als „das Foto, das unmittelbar vor dem aktuellen im Entwickeln-Modul offen war" (nicht z. B. „vorheriges Foto in der Ordnerreihenfolge") — deckt sich mit der Lightroom-Bedeutung von „Vorherige Einstellungen synchronisieren"
  - [x] Synchronisieren über die aktuelle Mehrfachauswahl (`syncSettingsToSelection`, reuse desselben `targets`-Filtermusters wie `setPhotoRating`/`setPhotoFlag`, hier auf „die *übrigen* markierten Fotos" statt „alle markierten" zugeschnitten, da synchronisiert vom aktiven Foto *auf* die anderen wird), Auto-Sync-Modus (`autoSyncActive`/`toggleAutoSync`, in `commitDevelopEdit` eingehängt)
  - **Bewusste Vereinfachung:** Auto-Sync überträgt bei jedem Commit immer alle zehn EDL-Sektionen (`PRESET_SECTION_KEYS`), nicht die im UI granular abwählbare Teilmenge, die für den manuellen Sync-Knopf gilt — ein Auto-Sync mit derselben granularen Auswahl bräuchte einen zweiten, unabhängig gepflegten Sektions-Zustand nur für den Auto-Fall; für den erwarteten Anwendungsfall („alles an mehrere ausgewählte Fotos gleichzeitig anpassen") ist „immer alles" die einfachere und naheliegendere Vorgabe
  - [x] Tests: 4 neue e2e-Tests in `workflow-flow.spec.ts` (Kopieren+Einfügen einer Sektionsteilmenge, Vorherige übernehmen inkl. deaktiviert/aktiviert-Zustand des Knopfs, Synchronisieren auf die übrige Mehrfachauswahl per Strg-Klick-Mehrfachauswahl, Auto-Sync ohne Knopfklick) — volle Kette grün: `cargo fmt/clippy/test` (Workspace, keine Rust-Änderungen in diesem Schritt), `tsc -b`, `vitest run` 149/149, `playwright test` 66/66, `vite build`

- [x] 10. Workflow: Referenzansicht + Soft-Proof
  - [x] Referenzansicht (Referenzbild links, Arbeitsbild rechts, unabhängiger Zoom/Pan) — neues `ReferenceView.tsx`, ersetzt den Viewer-Inhalt vollständig, solange `referenceViewActive` gesetzt ist (analog zu `BeforeAfterView`s Einhängung, mit Vorrang davor: beide sind gegenseitig exklusiv). Jede Bildhälfte hat einen eigenen `QuadRenderer` (`lib/webgl.ts`, dieselbe Textur-Hochlade-/Zeichenlogik wie `Viewer.tsx`) mit **rein lokalem** Zoom/Pan-Zustand statt des globalen `zoom`/`panX`/`panY` — reuse derselben reinen Geometrie-Helfer aus `lib/viewerMath.ts` (`imageOrigin`/`computeBaseScale`/`panForZoomAtCursor`/`nextZoomStep`), aber ohne `Viewer.tsx`s Overlay-/Werkzeug-Maschinerie (Masken/Crop/Reparatur/Pipetten). Das Referenzfoto ist frei wählbar (`DevelopPanel.tsx`-Dropdown, dasselbe Auswahlmuster wie `transferMaskToPhoto`s Zielfoto-Select) und wird **statisch** mit seinem zuletzt committeten Stand gezeigt (`current_develop_edit`, wie `applyPreviousSettings` es bereits für ein anderes Foto tut — `edlFromHistoryPosition` dafür aus `store/index.ts` exportiert)
  - [x] Soft-Proof (Zielprofil, Renderpriorität, Farbumfangswarnung, Papierweiß-Simulation) — vollständig als **rein clientseitige Nachbearbeitung** des bereits über die bestehende `develop/...`-Route gerenderten RGBA8-Vorschau-Puffers (`lib/softProof.ts::applySoftProof`, in `Viewer.tsx` vor dem `uploadRgba8`-Aufruf eingehängt): keine Backend-/Pipeline-Änderung nötig, betrifft nie den echten Export
  - **Präzisierung ggü. ADR-0032 Punkt 6** ("kein vollständiges ICC-Farbmanagement-Subsystem"): die genaue Architektur — rein clientseitig, ohne jede Backend-Beteiligung — war dort noch offen; hier festgelegt, weil `apx-pipeline` bis heute nur eine feste Kamera→sRGB-Matrix + Gammakurve kennt (kein ICC-Profil-Laden), ein neues Backend-Subsystem dafür allein für eine Anzeige-Vorschau unverhältnismäßig wäre
  - **Bewusste Vereinfachung (Soft-Proof):** drei simulierte Zielprofile (`SoftProofProfile`: sRGB unverändert, „Druck (simuliert)", „Graustufen-Druck (simuliert)"), jedes nur durch einen einzigen Sättigungs-Kompressions-Faktor beschrieben — kein echtes 3D-Gamut-Mapping. Renderpriorität ist als zwei tatsächlich unterschiedliche, aber ebenfalls angenäherte Kompressionsstrategien umgesetzt (wahrnehmungsorientiert = gleichmäßig auf alle Pixel, relativ farbmetrisch = nur auf Pixel oberhalb eines Sättigungs-Schwellenwerts). Farbumfangswarnung nutzt dieselbe Schwellenwert-Erkennung, um betroffene Pixel stattdessen magenta einzufärben. Papierweiß-Simulation ist eine lineare Tonwert-Bereichskompression `[0,255] → [12,243]`
  - [x] Tests: 6 neue Vitest-Unit-Tests (`lib/softProof.test.ts`: Identität bei sRGB, Graustufen-Umwandlung, Sättigungs-Schwellenwert unter „Relativ farbmetrisch", Farbumfangswarnung, Papierweiß-Kompression, Eingabepuffer bleibt unverändert) + 2 neue e2e-Tests in `workflow-flow.spec.ts` (Referenzansicht anzeigen/ausblenden inkl. deaktiviertem Knopf ohne gewähltes Referenzfoto, Soft-Proof-Regler schalten sich frei ohne zu committen) — volle Kette grün: `cargo fmt/clippy/test` (Workspace, keine Rust-Änderungen in diesem Schritt), `tsc -b`, `vitest run` 155/155, `playwright test` 68/68, `vite build`

- [x] 11. Dokumentation, Tests, Abnahme
  - [x] `ARCHITECTURE.md`: neues Kapitel „Architektur Phase 6" (§ 10) — Ebenenmodell inkl. der beim Bauen entdeckten Pipeline-Platzierungs-Korrektur und des Gruppen-Sichtbarkeits-Nachtrags, EDL-Schema v3, alle acht Workflow-Punkte, Datenfluss-Diagramm; Phase-6-Platzhalter in § 7 durch einen kurzen Verweis ersetzt
  - [x] `FEATURES.md`: alle jetzt gebauten §3.3/§3.4-Zeilen auf Fertig, mit ehrlichen Teil-Einschränkungs-/Abweichungs-Vermerken je bewusster Vereinfachung (Radialverlauf-Ziehgriff nur ein Radius, Überlagerungsfarbe ohne UI, Bausteine session-lokal, Vorher/Nachher-Trennlinie fest bei 50 %, Auto-Sync ohne granulare Auswahl, Soft-Proof-Näherungen)
  - [x] Performance-Nachmessung (neuer Test `render_rgba8_timing_with_several_masks_active`, fünf gleichzeitig sichtbare Masken — alle fünf Geometrietypen, alle sechs Masken-Werkzeuge je Maske nicht-neutral, drei der teureren Ganz-Pixel-Mischmodi vertreten): ~4,9 s (GPU, `llvmpipe`-Software-Rasterisierer dieser Sandbox — die Maskenstufe hat aber ohnehin keinen eigenen GPU-Pfad, läuft also unabhängig von `ctx` komplett CPU-seitig) bzw. ~4,0 s (CPU-Fallback) bei 2048×1365. **Bestätigt das in ADR-0032 Punkt 4 benannte Risiko:** das Ebenenmodell skaliert linear mit der Maskenzahl (fünf sequenzielle Durchläufe durch je sechs Werkzeuge statt eines einzigen Fused-Passes) und liegt im „viele/komplexe Masken gleichzeitig"-Extremfall weit über dem 16-ms-Ziel — deutlicher noch als die bereits in Phase 4 Schritt 13 gemessene „alle Werkzeuge gleichzeitig"-Grundpipeline (~2,3 s GPU/~1,3 s CPU). Kein Regressions-Fund, sondern die erwartete Konsequenz der in Schritt 2 bewusst getroffenen Architekturentscheidung; die naheliegende Abhilfe (GPU-Beschleunigung der Maskenstufe, in Schritt 2/3 explizit zurückgestellt) bleibt ein Ausbau für eine spätere Phase, sobald echte Performance-Beschwerden mit mehreren aktiven Masken auftreten — für den *typischen* Fall (ein bis drei einfache Masken) ist die Verlangsamung proportional kleiner
  - [x] Volle Verifikation: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used`, `cargo test --workspace`, `tsc -b`, `vitest run` 155/155, `playwright test` 68/68, `vite build` — alles grün; Commit+Push, CI-Check (siehe Chat-Abschlussbericht)

### Nicht in Phase 6 (bewusst zurückgestellt)
Siehe ADR-0032: Tiefenbereich-Masken (kein Tiefendaten-Zulieferer, ohne Phasenzuordnung); KI-Masken (Motiv/Himmel/Hintergrund/Objekte/Personen) → Phase 7; Reparatur-Erweiterungen (Auto-Quellenfindung, inhaltsbasiertes Füllen, Sensorflecken-Visualisierung) → Phase 7; Bibliotheks-Backlog (Sammlungssätze, Stapel, virtuelle Kopien, erweiterbare Farbmarkierungen, Schlagworthierarchie, Metadaten-Presets/EXIF-IPTC-XMP-Editor, Vergleichs-/Übersichtsansicht, Filter-Presets, Schnellentwicklung im Raster, Vorschau-Cache/Smart Previews) → Phase 9.

## Aktuelle Phase: Phase 7 — KI-Funktionen

`SPEC.md` §5 nennt wörtlich „KI-Funktionen. Motiv-/Himmel-/Personen-
Segmentierung (ONNX-Runtime, Modelle lokal), Preset-Generator per LLM,
Referenzbild-Matching, Auto-Tagging." Siehe `DECISIONS.md` ADR-0033 für
die Scope-Präzisierung: echte ONNX-Runtime-Modellinferenz ist in dieser
Umgebung nicht seriös umsetzbar (kein legitimer Weg, Modellgewichte zu
beschaffen/mitzuliefern, kein bestätigter Zugriff auf vorkompilierte
ONNX-Runtime-Binaries) — die fünf KI-Masken werden stattdessen über
echte, deterministische, klassische Bildverarbeitungsheuristiken gebaut
(Saliency/Farbheuristik/Region-Growing/Hautton-Erkennung), jede eine
genuine statt vorgetäuschte Fähigkeit. Der LLM-Client für den
Preset-Generator ist dagegen ein echter Anthropic-Messages-API-Client.
Neues `apx-ai`-Crate bündelt alle Bausteine dieser Phase (Reparatur-
Erweiterungen, die schon ADR-0032 Punkt 8 hierher vorgemerkt hatte,
eingeschlossen — außer dem render-zeitlichen Content-Aware-Fill, das in
`apx-pipeline::stages::repair` bleibt).

**Testdisziplin dieser Phase (Nutzerauftrag):** anders als in Phase 2–6
wird die volle Playwright-/Vitest-Testabdeckung **nicht** nach jedem
Schritt neu geschrieben/ausgeführt, sondern erst gebündelt in Schritt 6
(Dokumentation, Tests, Abnahme) — Zwischenschritte halten `cargo fmt`/
`clippy`/`test` sowie `tsc -b` grün (schnell, fängt grobe Fehler früh),
verzichten aber auf neue e2e-Spezifikationen bis zum Schluss. Am Ende
müssen `cargo test --workspace`, `tsc -b`, `vitest run`, `playwright
test` und `vite build` alle vollständig grün sein — keine Ausnahme.

- [ ] 0. Scope festzurren
  - [x] `DECISIONS.md`: neues ADR-0033
  - [x] `FEATURES.md`: Auto-Tagging-Zeile nachgetragen (§3.1, Phase 7 — fehlte bislang komplett)
  - [ ] `ARCHITECTURE.md` §7s Phase-7-Platzhalter wird in Schritt 6 durch ein volles Kapitel ersetzt
  - [x] `PLAN.md`: dieser Abschnitt

- [x] 1. `apx-ai`-Crate-Grundgerüst + gemeinsame Bildanalyse-Bausteine
  - [x] Neues Crate `crates/apx-ai`, Workspace-Mitglied, Abhängigkeiten `apx-core`/`apx-raw`/`apx-pipeline`/`apx-catalog`/`reqwest` (rustls-tls)/`tokio`/`base64`/`serde`/`serde_json`/`rayon`; eigener `AiError`-Fehlertyp (analog `PipelineError`) + neuer `AppError::Ai`
  - [x] Gemeinsame Hilfsfunktionen: `color.rs` (Luminanz/YCbCr/Sättigung), `blur.rs` (dreifacher Box-Filter als Gauß-Approximation) — beide neu in `apx-ai`; bilineares Alpha-Resampling (`bilinear_resize_u8`/`fit_within`) bewusst in `apx_core::raster` statt in `apx-ai` (vermeidet einen Abhängigkeitszyklus `apx-pipeline` → `apx-ai`, siehe dessen Moduldoku)
  - [x] `apx-pipeline/src/edl/v3.rs`: `MaskGeometry::AiGenerated { ai_kind: AiMaskKind, width, height, alpha: Vec<u8> }` (Feld heißt `ai_kind` statt `kind` — Kollision mit dem internen Serde-Tag) + `AiMaskKind`-Enum (Subject/Sky/Background/ClickRegion/Person); `stages/masks.rs::ai_generated_alpha` skaliert per `apx_core::raster::bilinear_resize_u8` auf die Renderauflösung hoch
  - [x] **Vorgezogen aus Schritt 2/3:** alle fünf KI-Masken-Heuristiken (`apx-ai::segmentation`) und beide Reparatur-Analyse-Algorithmen (`apx-ai::repair_analysis::suggest_source_point`/`detect_spots`) bereits vollständig implementiert und unit-getestet (29 Tests) — nur die Tauri-Command-/Frontend-Verdrahtung bleibt für Schritt 2/3
  - [x] Inhaltsbasiertes Füllen (`RepairMode::ContentAwareFill` in `apx-pipeline::stages::repair.rs`) ebenfalls vorgezogen: vereinfachtes PatchMatch (Nächster-Nachbar-Vorbelegung + Zufallsinit + Propagation + Zufallssuche, deterministischer xorshift32-PRNG), 4 neue Rust-Tests
  - [x] Volle Kette grün: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used`, `cargo test --workspace` (203 Rust-Tests total, 29 davon neu in `apx-ai`) — Frontend unverändert in diesem Schritt

- [x] 2. Die fünf KI-Masken (Tauri-/Frontend-Verdrahtung — Algorithmen siehe Schritt 1)
  - [x] Tauri-Command `generate_ai_mask(photo_id, kind, click_x?, click_y?, tolerance?) -> AiMaskAlphaDto` (Base64-kodierte Alpha-Bitmap + Breite/Höhe), nutzt `TileCache::get_or_decode` auf `apx_ai::segmentation::ANALYSIS_MAX_EDGE` wie `compute_develop`
  - [x] Frontend-Datenmodell: `MaskGeometry::AiGenerated`/`AiMaskKind` in `lib/edl.ts` ergänzt (spiegelt die Rust-Seite aus Schritt 1)
  - [x] Frontend-UI: `MasksPanel.tsx` hat einen „KI-Maske hinzufügen"-Abschnitt (fünf Knöpfe, „Objekte…" aktiviert einen Bild-Klick-Picker in `Viewer.tsx`, analog zu den bestehenden Picker-Mustern für Weißabgleich/Farbmischer/Farbbereich) — `store/index.ts::addAiMask` dekodiert Base64→`number[]` (`lib/edl.ts::base64ToByteArray`), legt eine neue Maske mit `AiGenerated`-Geometrie an und committet sofort

- [x] 3. Reparatur-Erweiterungen (Tauri-/Frontend-Verdrahtung — Algorithmen siehe Schritt 1)
  - [x] Je ein Tauri-Command für Auto-Quellenfindung (`suggest_repair_source`) und Sensorflecken-Visualisierung (`detect_sensor_spots`)
  - [x] Inhaltsbasiertes Füllen als dritter Reparatur-Modus im Frontend auswählbar: `RepairMode` um `"ContentAwareFill"` erweitert, `RepairOverlay.tsx` überspringt für diesen Modus den Quellpunkt-Schritt (`skipSourceStep`-Prop), `store/index.ts::addRepairStroke` erlaubt das Committen ohne `repairPendingSource`, `DevelopPanel.tsx` mit drittem Radio-Knopf + angepasstem Hinweistext
  - [x] Frontend-UI für Auto-Quellenfindung: Checkbox „Quelle automatisch vorschlagen" in `DevelopPanel.tsx` — solange aktiv, löst der erste Klick in `RepairOverlay.tsx` (der sonst direkt den Quellpunkt setzt) stattdessen `suggestRepairSourceForTarget` an dieser Position aus (`autoSourceModeActive`/`onSuggestSource`-Prop)
  - [x] Frontend-UI für Sensorflecken-Visualisierung: Knopf „Sensorflecken suchen" + Fundliste mit „Reparieren"-Knopf je Fund (übernimmt als `ContentAwareFill`-Strich) in `DevelopPanel.tsx`, orange gestrichelte Marker im `RepairOverlay.tsx`
  - Volle Kette grün: `cargo fmt/clippy/test` (Workspace, weiterhin 203 Rust-Tests — reine Verdrahtung, keine neue Rust-Logik in diesem Teilschritt), `tsc --noEmit`, `vitest run` (155 Frontend-Tests)

- [x] 4. Preset-Generator
  - [x] `apx-core::Settings`: neues `AiSettings { anthropic_api_key: Option<String> }`-Feld + Frontend-Einstellungen-UI zum Hinterlegen — bewusst kein eigener globaler Einstellungsbildschirm (existiert noch nicht im Frontend), stattdessen ein `<details>`-Abschnitt direkt im KI-Preset-Generator (`PresetsPanel.tsx`), wo der Schlüssel gebraucht wird
  - [x] LLM-Anfrage: `apx-ai::preset_generator::generate_from_llm` — echter Anthropic-Messages-API-Aufruf per rohem `reqwest`-JSON (kein offizielles Rust-SDK), System-Prompt beschreibt das EDL-Sektionsschema, erwartet ein reines JSON-Objekt als Antwort; serverseitige Validierung mergt die Antwort **rekursiv** (`merge_json_patch`, JSON-Merge-Patch-Stil) auf ein neutrales `EdlV3` und deserialisiert es vollständig — ein halluziniertes/falsch geformtes Feld lässt den Aufruf fehlschlagen statt ein kaputtes Preset durchzureichen. **Beim Testen entdeckte Korrektur:** ein anfänglicher flacher Merge ersetzte jede genannte Sektion komplett statt Feld für Feld, wodurch ein knappes `{"basic": {"exposure_ev": 0.5}}` an den fehlenden übrigen `BasicAdjustments`-Feldern scheiterte — behoben, Systemprompt entsprechend angepasst (nicht genannte Unterfelder bleiben neutral)
  - [x] **Nachtrag (auf Nutzerwunsch):** manueller Modus ohne API-Schlüssel — `standalone_prompt_text`/`parse_and_validate_pasted_json` plus zwei Tauri-Commands (`build_preset_prompt_text`/`import_preset_json`) und zwei neue Knöpfe in `PresetsPanel.tsx` („Prompt für Claude-App" kopiert in die Zwischenablage, „Antwort aus der Claude-App einfügen" validiert eine von Hand zurückkopierte JSON-Antwort) — dieselbe serverseitige Validierung wie der API-Pfad, aber ohne Netzwerk-Aufruf/Kosten; nutzt die kostenlose Claude-App (claude.ai) statt der kostenpflichtigen API
  - [x] Referenzbild-Modus: Koordinatenabstieg über sechs tonwertbezogene Grundeinstellungs-Parameter (Belichtung/Kontrast/Lichter/Tiefen/Weiß/Schwarz), Histogramm-Distanz als Zielfunktion (Kumulativsummen/Earth-Mover's statt rohem Bin-Vergleich — sonst kein Fortschrittssignal bei schmalen Verteilungen, beim Testen entdeckt), kein LLM
  - [x] Variationen-Generator: deterministisch geseedeter xorshift32-PRNG stört jeden numerischen Blattwert eines Basis-Presets (Kontaktbogen-Vorschau im Frontend, mehrere Vorschläge gleichzeitig auswählbar)
  - [x] Preset aus Bearbeitung lernen: Mittelwertbildung committeter EDL-Werte mehrerer ausgewählter Fotos je Sektion (`apx-ai::preset_generator::average_subsets`, arithmetisches Mittel numerischer Blattwerte)
  - [x] Frontend: neuer `AiPresetGeneratorSection`-Abschnitt in `PresetsPanel.tsx` für alle vier Generator-Modi — liefert nur eine EDL-Teilmengen-Vorschau, „Auf aktuelles Foto anwenden" mischt sie in `developEdl` (wie ein normales Preset), Sichern läuft über den bereits bestehenden „Preset speichern"-Knopf aus Phase 5 statt einer eigenen Speicher-Logik

- [x] 5. Auto-Tagging
  - [x] `apx-ai::tagging`: regelbasierte Schlagwort-Vorschläge aus Segmentierungs-Heuristiken (Himmel-/Personen-Flächenanteil) + EXIF (ISO/Blende/Brennweite) — reuse der bestehenden `photo_keywords`-Infrastruktur aus Phase 3
  - [x] Tauri-Command `suggest_tags(photo_id) -> Vec<String>`, Frontend-Knopf „Tag-Vorschläge" im Metadaten-Panel — jeder Vorschlag ein eigener Knopf, der ihn per bestehendem `addKeywordToPhoto` übernimmt und aus der Vorschlagsliste entfernt

- [x] 6. Dokumentation, Tests, Abnahme
  - [x] `ARCHITECTURE.md`: neues Kapitel „11. Architektur Phase 7 — KI-Funktionen" (Crate-Übersicht, alle fünf Funktionsbereiche, Datenfluss-Diagramm KI-Maske erzeugen, plus die beiden beim Bauen entdeckten Korrekturen: PatchMatch-Nächster-Nachbar-Vorbelegung, Kumulativsummen-Histogrammdistanz)
  - [x] `FEATURES.md`: alle Phase-7-Zeilen in §3.1/§3.2/§3.3/§3.5 auf Fertig (abweichend, mit Verweis auf ADR-0033) — inkl. der bereits in Schritt 1 vorgezogenen Auto-Tagging-Zeile, die zuvor als „Nicht begonnen" stehen geblieben war
  - [x] Volle, gebündelte Testabdeckung: 218 Rust-Tests workspace-weit (29 in `apx-ai` aus Schritt 1 plus 19 neue in `preset_generator`/`tagging`), neue Playwright-Spezifikation `e2e/ai-flow.spec.ts` (6 Szenarien: alle fünf KI-Masken-Auslöser, Auto-Quellenfindung, Sensorflecken-Reparatur, Auto-Tagging-Übernahme, Preset-Generator-LLM-Modus) — volle Kette grün: `cargo fmt/clippy(-D warnings -D unwrap_used)/test`, `tsc --noEmit`, `vitest run` (155 Tests), `playwright test` (74 Tests, alle bestehenden weiterhin grün)
  - [x] Commit+Push, ehrlicher Abschlussbericht (inkl. aller ADR-0033-Vereinfachungen: keine echte ONNX-Inferenz, Personen-Maske nur eine Hautton-Region statt Einzelteile, vereinfachtes PatchMatch, Kumulativsummen-Histogrammdistanz statt echtem Gradientenverfahren, Referenzbild-Modus nur sechs Tonwertregler, Lernen mittelt nur Skalare)

### Nicht in Phase 7 (bewusst zurückgestellt)
Tiefenbereich-Masken (siehe ADR-0032 Punkt 3, weiterhin ohne Phasenzuordnung); echte ONNX-Runtime-Modellinferenz (siehe ADR-0033 Punkt 1 — ein „Bring-your-own-Model"-Pfad wäre ohne verifizierbares Modell nur eine ungetestete Hülle); Einzelregionen der Personen-Maske (Augen/Brauen/Lippen/Zähne/Haare/Kleidung einzeln wählbar).

## Aktuelle Phase: Phase 8 — Export und Ausgabe-Module

`SPEC.md` §5 nennt wörtlich „Export und Ausgabe-Module. Export-Engine,
Warteschlange, Wasserzeichen, dann Drucken, Diashow, Buch, Web, Karte."
Siehe `DECISIONS.md` ADR-0034 für die Scope-Präzisierung: anders als
Phase 6/7 ist dies kein einzelnes fachliches Thema, sondern sechs
weitgehend unabhängige Ausgabe-Module, die nur die Export-Engine als
gemeinsamen Unterbau teilen — die Schrittfolge unten übernimmt deshalb
`SPEC.md`s eigene Reihenfolge (Export-Engine zuerst, dann
Drucken/Diashow/Buch/Web/Karte), Templates erst ganz am Ende (sie
setzen alle anderen Module voraus, siehe ADR-0031 Punkt 5).

**Reale vs. zurückgestellte Fähigkeiten (ADR-0034):** WebP-/AVIF-Export,
echtes ICC-Farbmanagement (`lcms2`, mit echtem Verdrahtungsgrund diesmal
— eine exportierte Datei muss ihr Profil korrekt tragen, anders als
Phase 6s simulierter Soft-Proof), PDF-Export (`printpdf`), FTP/SFTP-
Upload (`suppaftp`/`russh`) und Reverse Geocoding (`reverse_geocoder`,
vollständig offline) sind **echt** umsetzbar, keine Simulation. PSD-/
HEIF-/JPEG-XL-Export bleiben zurückgestellt (keine tragfähige
Rust-Bibliothek bzw. Lizenz-/Beschaffungsmauer wie bei ONNX in
ADR-0033). Video-Export ruft ein System-`ffmpeg` auf statt eines
mitgelieferten Binaries (Lizenz-/Bundling-Aufwand pro Plattform) — echt,
wenn vorhanden, sonst eine klare Fehlermeldung statt einer leeren
Funktion. Die Kartenansicht selbst (Kachel-Bilder) ist die einzige
Ausnahme von „offline zuerst" in dieser Phase.

- [x] 0. Scope festzurren
  - [x] `DECISIONS.md`: neues ADR-0034 (Machbarkeitsprüfung sechs Module: Export-Engine/Drucken/Diashow/Buch/Web/Karte)
  - [x] `FEATURES.md`: bereits vollständig und korrekt auf Phase 8 getaggt (inkl. Export-Warteschlange-Zeile) — keine Korrektur nötig
  - [ ] `ARCHITECTURE.md` §7s Phase-8-Platzhalter wird im letzten Schritt durch ein volles Kapitel ersetzt
  - [x] `PLAN.md`: dieser Abschnitt

- [x] 1. Export-Engine-Grundgerüst + Formate
  - [x] Neues Crate `crates/apx-export` (Workspace-Mitglied), hängt von `apx-core`/`apx-raw`/`apx-pipeline`/`apx-catalog` ab; rendert über den bestehenden `apx_pipeline::develop::render_rgba8`-Pfad (`engine::render_and_encode`/`export_to_file`), kein zweiter Rendering-Codepfad
  - [x] Formate: JPEG/PNG/TIFF (`image`-Crate, wie bisher), WebP (verlustfrei über `image-webp`) + AVIF (verlustbehaftet über `ravif`/`rav1e`) neu, beide zusätzliche `image`-Features statt eigener Abhängigkeiten (`format.rs`) — AVIF-Dekodieren ist damit *nicht* möglich (bräuchte das separate `avif-native`-Feature mit einer C-Systembibliothek), nur Kodieren; als Test stattdessen die ISOBMFF-`ftyp`-Signatur geprüft
  - [x] Bit-Tiefe 8/16 (`format.rs::BitDepth`), Größenbegrenzung Kante/Megapixel (`resize.rs::SizeConstraint`) + Zieldateigröße per iterativer JPEG-Qualitätssuche (`resize::fit_jpeg_to_max_bytes`), Ausgabeschärfung nach Medium (`sharpen.rs`, Unsharp-Masking mit Bildschirm-/Matt-/Hochglanz-Voreinstellungen) — **16-Bit ist eine lineare Streckung des fertigen 8-Bit-Werts** (`v * 257`), keine echte Präzisionssteigerung, da `render_rgba8` durchgehend 8-Bit quantisiert (siehe `format.rs`s Moduldoku); nur für PNG/TIFF
  - [x] Import mit DNG-Konvertierung: `dng`-Bibliothek evaluiert — ihr öffentliches API ist reiner Lesezugriff, kein Schreibpfad für eigene DNG-Dateien; damit in dieser Umgebung nicht umsetzbar, zurückgestellt (siehe `engine.rs`s Moduldoku, `FEATURES.md`)
  - [x] Tauri-Command `export_photo` + Frontend-Exportdialog-Grundgerüst (`ExportDialog.tsx`: Zielordner, Format, Qualität, 16-Bit, Größenbegrenzung, Zieldateigröße, Ausgabeschärfung; „Exportieren…"-Knopf in `Header.tsx`, exportiert Mehrfachauswahl sequenziell)
  - **Abweichung von der ursprünglichen Schritt-Planung (Disk-Vorsicht):** `apx-export`s `Cargo.toml` deklariert vorerst nur `image` (webp/avif) — `lcms2`/`ab_glyph` (Schritt 2), `printpdf` (Schritt 5), `suppaftp`/`russh` (Schritt 6), `reverse_geocoder`/`quick-xml` (Schritt 7) werden erst im jeweiligen Schritt ergänzt statt alle auf einmal: ein Testlauf in dieser Sandbox mit allen auf einmal deklarierten Abhängigkeiten hat das feste Plattenkontingent der Umgebung tatsächlich erschöpft (`printpdf`→`azul-layout` zieht einen sehr großen Font-/Layout-Baum nach sich) — kein architektonischer Rückschritt, nur eine andere Reihenfolge des Hinzufügens

- [x] 2. Farbräume/ICC, Wasserzeichen, Export-Warteschlange, Metadaten-Filter
  - [x] `lcms2` (Feature `static`) zurück als echte Abhängigkeit (`icc.rs`) — vier gebündelte Standardprofile (sRGB/Adobe RGB/ProPhoto RGB/Display P3, aus offiziellen Chromatizitätswerten aufgebaut statt als `.icc`-Dateien mitgeliefert) + Dateiauswahl für „eigenes ICC"; Phase 6s simulierter Soft-Proof bleibt unverändert — **bewusste Vereinfachung:** ProPhoto/Display-P3 nutzen eine reine Potenzgammakurve statt ihrer echten stückweisen Übertragungsfunktion (Unterschied nur in den untersten Tonwerten, für Exportzwecke vernachlässigbar)
  - [x] Wasserzeichen (`watermark.rs`): Bild-Overlay (dekodierte RGBA8-Pixel, alpha-gewichtet in eine Bildecke komponiert) und Text-Overlay (echte Glyph-Rasterisierung über `ab_glyph`) — **Text-Wasserzeichen brauchen eine vom Nutzer gewählte `.ttf`/`.otf`-Datei**, keine eingebettete Schriftart (spart eine Binärdatei + deren Lizenzeintrag für eine reine Zusatzfunktion)
  - [x] Metadaten-Filter (`metadata.rs`): echter minimaler EXIF-Writer (flaches IFD0, Make/Model/DateTime/Copyright/Artist als ASCII-Tags) als APP1-Segment direkt nach dem JPEG-SOI-Marker eingefügt — **nur für JPEG** (kein Encoder der übrigen vier Formate unterstützt Metadaten-Schreiben), **GPS/`DateTimeOriginal`-Sub-IFD zurückgestellt** (IFD0-`DateTime` statt vollem Exif-Sub-IFD, siehe Moduldoku)
  - [x] Export-Warteschlange (`queue.rs` + `apx-app`s `export_queue_worker`): echte Fortschritts-/Pausier-/Prioritäts-Logik (reine, threading-freie `ExportQueue<T>`-Struktur, von einem einzelnen Hintergrund-Worker abgearbeitet, der `spawn_blocking` für die eigentliche Render-/Kodierarbeit nutzt — dieselbe Grundidee wie der Import-Job) — **vereinfacht:** Abfragen (150ms serverseitig, 250ms Frontend-Polling) statt einer Weck-Benachrichtigung/Tauri-Events, keine Persistenz der Warteschlange über App-Neustarts hinweg

- [x] 3. Drucken
  - [x] Layout-Geometrie (Einzelbild/Kontaktbogen/Bilderpaket/benutzerdefiniertes Raster): neues `apx-export::print`-Modul, alle vier Layouts auf eine gemeinsame `PrintSlot`-Liste + `compose_page`-Funktion reduziert (`grid_slots` für Einzelbild/Kontaktbogen/Raster — Einzelbild ist der Sonderfall Spalten=Zeilen=1 —, `picture_package_slots` für Bilderpaket); Randeinstellungen (Rand/Zellabstand in Zoll), Zoom (Einpassen/Füllen-Beschneiden über `FitMode`) — **Bilderpaket nutzt drei feste, handgepflegte Vorlagen** (1 groß+2 klein/4 gleich/8 Wallet) statt echtem Bin-Packing beliebiger Formatkombinationen — bewusste Vereinfachung, siehe Moduldoku
  - [x] Druckschärfung/Farbmanagement über die Export-Engine aus Schritt 1/2 wiederverwendet: `engine::render_to_pixels` (aus `render_and_encode` extrahiert) liefert die rohen RGBA8-Pixel je Foto vor der Kodierung, `print_photos` wendet darauf denselben Schärfe-/ICC-Pfad an wie ein Einzelexport, komponiert dann alle Zellen zu einer Seite und kodiert erst diese fertige Seite als JPEG — kein zweiter Render-Pfad
  - [x] Druckauflösung als DPI-Parameter (bestimmt zusammen mit den Seitenmaßen in Zoll die Pixelgröße der Ausgabeseite, `compose_page`)
  - [x] „Speichern als JPEG" (Druck-Layout als Exportziel statt Einzelbild): neuer Tauri-Command `print_photos` + `pick_save_file_path` (Speichern-unter-Dialog statt Zielordner-Auswahl, da genau eine Ausgabedatei entsteht) — kein System-Druckertreiber-Zugriff in dieser Phase, Ausgabe ist eine druckfertige Datei
  - [x] Neues `PrintDialog.tsx` (Layout-/Vorlagenwahl, Seitenmaße/DPI/Rand/Zellabstand, Zoom, ICC-Profil, Druckschärfung-Regler) + „Drucken…"-Knopf in `Header.tsx`, teilt die Fotoauswahl-Quelle mit dem Exportdialog
  - [x] Tests: 10 neue Rust-Unit-Tests in `print.rs` (Slot-Geometrie je Layout, `compose_page`-Pixelmaße/-Hintergrund/-Zellplatzierung, Fit-Modi), 8 neue Playwright-e2e-Tests in `print-flow.spec.ts` (Knopf-Aktivierung, Dialog-Abbruch, alle drei parametrisierten Layouts inkl. Vorlagenwahl, ICC/Zoom/Schärfung-Übergabe, Fehlerfall)

- [x] 4. Diashow
  - [x] Übergänge/Ken-Burns-Effekt/Intro-Outro-Screens: reine Frontend-Canvas-Wiedergabe (`lib/slideshow.ts` — Zeitachsen-/Ken-Burns-Mathematik, rein und unit-getestet — plus `SlideshowPlayer.tsx`, eine `requestAnimationFrame`-Schleife, die pro Frame den aktuellen Zeitachsen-Abschnitt bestimmt und Fotos/Titelkarten auf ein `<canvas>` zeichnet) — **bewusste Vereinfachung:** nur zwei Übergangsarten (harter Schnitt/Überblendung zweier eingefrorener Ken-Burns-Endzustände statt eines während der Überblendung weiterlaufenden Effekts), kein Wipe/Slide, siehe `apx_export::video`s Moduldoku
  - [x] Musik-Synchronisation: Tauri-Webview-`<audio>`-Element (kein Rust-Audio-Crate) — die lokale Audiodatei (Nutzerauswahl über `pick_file_path`) wird über eine neue `apx://music/<pfad>`-Protokollroute roh ausgeliefert (`protocol::route::ImageRequest::Music`, `frontend/src/lib/media.ts::musicUrl`), Start/Stop laufen synchron mit der `requestAnimationFrame`-Schleife
  - [x] Video-Export (MP4): neues `apx_export::video`-Modul — `ffmpeg_available()` prüft `ffmpeg -version` beim Öffnen des Diashow-Dialogs (`check_ffmpeg_available`-Command, deaktiviert sonst den Export-Knopf mit Hinweistext); vorhanden → `export_slideshow_video` rendert Frame für Frame (Ken-Burns-Zuschnitt + Seitenverhältnis-Korrektur `cover_adjust`, Überblendungs-Alpha-Mischung, Titelkarten über `render_title_card` — wiederverwendet `watermark::apply_text_watermark`s Glyph-Rasterisierung statt eines zweiten Textpfads) und pippt sie roh (RGBA8) in einen gespawnten `ffmpeg`-Prozess (`-f rawvideo` über `stdin`, H.264/MP4, `-shortest` bei Musik — die Video-Länge bleibt maßgeblich, kürzere Musik beendet das Video mit) — kein Bundling eines eigenen `ffmpeg`-Binaries (Lizenz-/Bündelungsaufwand je Plattform, wie ADR-0034 es für Schritt 1 schon für DNG/PDF/FTP/Geocoding festhält); fehlt `ffmpeg`, liefert der Command eine klare Fehlermeldung statt eines stillen Fehlschlags
  - [x] Neues `SlideshowDialog.tsx` (Dauer je Foto, Ken-Burns-Umschalter, Übergang/-dauer, Intro-/Outro-Textkarten mit Hintergrund-/Textfarbe, Schriftdatei-/Musikauswahl, Video-Auflösung/Bildrate) + „Diashow…"-Knopf in `Header.tsx`, teilt die Fotoauswahl-Quelle mit Export-/Druckdialog; „Abspielen" öffnet `SlideshowPlayer.tsx` mit derselben Folienliste (`buildSlideItems`), die auch der Video-Export verwendet
  - [x] Tests: 25 neue Rust-Unit-Tests in `apx-export::video` (Ken-Burns-Interpolation/-Grenzen, deterministisches Muster, Zuschnitt, Seitenverhältnis-Korrektur, Titelkarten, Frame-Plan bei Schnitt/Überblendung, Frame-Rendering/-Mischung, `ffmpeg`-Fehlerpfad) + 5 neue Rust-Unit-Tests in `apx-app::protocol` (Musikrouten-Parsing/-Auslieferung, MIME-Erkennung), 22 neue Vitest-Tests in `lib/slideshow.test.ts` (Ken-Burns-Formel, Seitenverhältnis-Korrektur, Zeitachsen-Aufbau, Folienliste), 11 neue Playwright-e2e-Tests in `slideshow-flow.spec.ts` (Knopf-Aktivierung, fehlendes ffmpeg, Live-Wiedergabe öffnet eine Leinwand, Übergangsdauer-Sichtbarkeit, Dialog-Abbruch, Video-Export mit Einstellungen/Ken-Burns-Umschalter/Intro-Schriftdatei/Musik, Fehlerfall)

- [x] 5. Buch
  - [x] Neues `apx-export::book`-Modul — Seitenvorlagen (Randlos/Zwei-Fotos/2×2-Raster/Foto-mit-Bildunterschrift/Titelseite) wiederverwenden `print::PrintSlot`/`print::compose_page`/`print::grid_slots` unverändert (eine Buchseite ist geometrisch dieselbe Zellen-auf-Seite-Aufgabe wie eine Druckseite) — **bewusste Vereinfachung wie bei den Bilderpaket-Vorlagen:** fünf feste Layouts statt einer frei konfigurierbaren Slot-Engine, keine „Text-Stile" im Sinne wählbarer Schriftfamilien/-schnitte (eine Schriftdatei, wie bei Diashow-Titelkarten)
  - [x] `auto_fill_pages`: verteilt die Fotoauswahl reihum auf Seiten gemäß Slotanzahl der Vorlage — „automatische Befüllung"; Bildunterschriften bei „Foto mit Bildunterschrift" sind automatisch der Dateiname (keine manuelle Texteingabe pro Seite nötig, hält den Dialog schlank)
  - [x] Textfelder (Titelseite/Bildunterschriften) über zwei neue, aus `apply_text_watermark`/`apply_image_watermark` herausgezogene Funktionen `watermark::apply_text_at`/`apply_image_at` (freier Pixel-Ursprung statt der vier festen Wasserzeichen-Ecken) — kein zweiter Textrasterisierungs-Pfad
  - [x] PDF-Export über `printpdf` (reines Rust) — **bewusst ohne dessen Standard-Features** (`html`→`azul-layout`, `images`→eigener Bilddecoder): jede Buchseite ist bereits ein fertig komponiertes RGBA8-Bild, `book::build_pdf` bettet es direkt als `printpdf::RawImage` ein, ohne Zwischenkodierung und ohne `printpdf`s Text-/HTML-Engine — hält den Abhängigkeitsbaum klein (1m17s statt der in Schritt 1 dokumentierten Aufblähung mit Standard-Features)
  - [x] Druckerei-Presets als reine Parametersätze (`PrintShopPreset`: Beschnitt/Auflösung/Hintergrund, drei feste Presets) — keine anbieterspezifische Validierung
  - [x] Tauri-Command `export_book_pdf` + neues `BookDialog.tsx` (Seitenvorlage, Seitenmaße, Druckerei-Preset, optionale Titelseite mit Schriftdatei-Auswahl) + „Buch…"-Knopf in `Header.tsx`, teilt die Fotoauswahl-Quelle mit den übrigen Export-Dialogen
  - [x] Tests (bewusst schlank statt erschöpfend, siehe Nutzerwunsch): 7 neue Rust-Unit-Tests in `apx-export::book` (Slotanzahl je Vorlage, automatische Befüllung inkl. Titelseiten-Sonderfall, Seiten-Pixelmaße, Bildunterschrift-ohne-Schriftdatei-Fehler, PDF-Signatur/leere-Seitenliste), 3 neue Playwright-e2e-Tests in `book-flow.spec.ts` (Knopf-Aktivierung, Export mit Einstellungen, Fehlerfall) statt der bei Schritt 3/4 üblichen zweistelligen Testzahl

- [x] 6. Web
  - [x] Neues `apx-export::web`-Modul — `generate_gallery_html` baut eine einzelne statische HTML-Datei (reines Rust-String-Templating, kein Template-Engine-Crate) mit eingebettetem CSS für drei Themes (Hell/Dunkel/Minimal); Fotos werden wie bei Druck/Buch über `engine::render_to_pixels` gerendert und als JPEG-Miniaturbilder neben die HTML-Datei geschrieben — kein zweiter Rendering-Pfad
  - [x] Upload via FTP/SFTP: `suppaftp` (echtes FTP/FTPS, synchron) + `russh`/`russh-sftp` (echtes SFTP, reines Rust, keine OpenSSL-/libssh2-Systemabhängigkeit) — beide laden den erzeugten Ordner rekursiv hoch; SFTP nimmt zur Host-Key-Prüfung bewusst jeden Server-Schlüssel an (`AcceptAnyHostKey`, wie ein `ssh -o StrictHostKeyChecking=no`) statt eines Known-Hosts-Abgleichs, siehe Moduldoku
  - [x] Tauri-Command `export_web_gallery` (`async fn`, da der SFTP-Upload asynchron läuft — auf Tauris eigenem Tokio-Runtime, kein verschachtelter Runtime) + neues `WebDialog.tsx` (Titel, Theme, optionaler FTP/SFTP-Direkt-Upload mit Zugangsdaten) + „Web…"-Knopf in `Header.tsx`, teilt die Fotoauswahl-Quelle mit den übrigen Export-Dialogen; Zielordner über den bestehenden `select_folder`-Dialog (Ausgabe ist ein Ordner mit mehreren Dateien, kein Einzeldokument wie bei Buch/Drucken)
  - [x] Tests (bewusst schlank wie Schritt 5): 6 neue Rust-Unit-Tests in `apx-export::web` (HTML-Escaping, Titel-/Foto-Einbettung, leere Fotoliste, Datei-Schreibvorgang, FTP-/SFTP-Fehlerpfad bei unerreichbarem Server), 3 neue Playwright-e2e-Tests in `web-flow.spec.ts` (Knopf-Aktivierung, Export mit Einstellungen, Fehlerfall)

- [ ] 7. Karte
  - GPS aus EXIF (Erweiterung des bestehenden Metadaten-Pfads, `kamadak-exif`/`rawler` lesen GPS-Tags bereits mit), Kartenansicht im Frontend (Leaflet.js + OpenStreetMap-Kacheln — einzige Netzwerk-Abhängigkeit dieser Phase)
  - Reverse Geocoding vollständig offline (`reverse_geocoder`, GeoNames-Datensatz gebündelt)
  - GPX-Tracklog-Import (`quick-xml`), Fotos per Drag auf Karte setzen, Reiserouten-Ansicht (GPS-getaggte Fotos nach Aufnahmezeit sortiert)

- [ ] 8. Templates (setzen alle Module aus Schritt 1–7 voraus)
  - Export-/Wasserzeichen-/Metadaten-Templates (Parametersätze für die Export-Engine aus Schritt 1/2)
  - Layout-Templates (Druck/Buch/Diashow/Web — je ein vorkonfiguriertes Layout je Modul)
  - Workflow-Templates (Import→Filter→Preset→Export als ein Klick)
  - Template-Marktplatz-Struktur (lokales Repo-Format, Manifest, Installation — kein Online-Marktplatz-Hosting in dieser Phase)

- [ ] 9. Dokumentation, Tests, Abnahme
  - `ARCHITECTURE.md`: neues Kapitel „Architektur Phase 8" (Export-Engine als Unterbau, alle sechs Module, Datenfluss-Diagramm Export)
  - `FEATURES.md`: alle jetzt gebauten Phase-8-Zeilen auf Fertig (abweichend, mit Verweis auf ADR-0034 wo zutreffend), PSD-/HEIF-/JPEG-XL-Export bleiben „Nicht begonnen"
  - Volle Testabdeckung: Rust-Unit-Tests je neuem Modul, neue Playwright-e2e-Spezifikationen für die Frontend-Flows, volle Kette grün (`cargo fmt/clippy/test`, `tsc --noEmit`, `vitest run`, `playwright test`)
  - Commit+Push, ehrlicher Abschlussbericht (inkl. aller ADR-0034-Vereinfachungen)

### Nicht in Phase 8 (bewusst zurückgestellt)
PSD-/HEIF-/JPEG-XL-Export (siehe ADR-0034 Punkt 1 — keine tragfähige Rust-Bibliothek bzw. Lizenz-/Beschaffungsmauer); System-Druckdialog-/Druckertreiber-Integration (Ausgabe bleibt eine druckfertige Datei); Online-Template-Marktplatz-Hosting (nur die lokale Repo-Struktur); Adobe `.xmp`/`.lrtemplate`-Interop (weiterhin auf „eine spätere Phase" verschoben, siehe ADR-0031 Punkt 3).

## Backlog-Ergänzung für Phase 9 (auf Nutzerwunsch, außerhalb der Reihe)

Nachträglich aufgenommen (nicht aus `SPEC.md`, sondern direkt vom Nutzer
anhand eines Lightroom-Classic-Screenshots angefragt: Histogramm-Panel
plus Basic-Panel-Nachbarschaft) — elf UI-nahe Entwickeln-/Anzeige-
Fähigkeiten, die Lightroom hat und die bei uns bisher nirgends vorkamen,
weder in `SPEC.md` noch in `FEATURES.md`. Volle Liste mit technischen
Kurznotizen steht in `FEATURES.md` §3.2, neuer Unterabschnitt
„Histogramm, Zielwerkzeuge & KI-Verbesserung" (11 neue Zeilen, alle
`Status: Nicht begonnen`):

1. Live-Histogramm (RGB + Luminanz)
2. Clipping-Warnungen (Lichter/Tiefen-Dreiecke + Bildüberlagerung)
3. Punktfarbmesser (RGB-Wert unter dem Mauszeiger)
4. Zielgerichtetes Anpassungswerkzeug (TAT) für Kurven/HSL
5. Schwarzweiß-Umwandlung mit eigenem 8-Kanal-Mixer
6. Auto-Ton / Auto-Weißabgleich per Ein-Klick
7. Navigator-Miniaturansicht beim Zoomen
8. KI-Entrauschung über die volle Bildfläche
9. KI-Hochskalierung / Detailverbesserung
10. Info-Overlay im Vollbild-Modus
11. Bearbeitungs-Pins auf dem Bild für lokale Masken

**Vorläufig auf Phase 9 getaggt** (SPEC.md §5s „Fortgeschrittenes" ist
ohnehin schon der Sammelpunkt für nachträglich verschobene Punkte, siehe
ADR-0032) — das ist nur eine Einordnung, keine Zusage zur Umsetzungsart.
Anders als bei den übrigen Phase-8-Schritten oben bekommt dieser Block
**bewusst noch keine** Schritt-0-Scope-Präzisierung/ADR: Phase 8 ist noch
nicht abgeschlossen, Phase 9 ist noch nicht die aktuelle Phase (siehe
Kopfzeile dieser Datei: „hier steht nur der Arbeitsplan für die aktuell
offene Phase im Detail"). Zwei Punkte (8/9, KI-Entrauschung/-Hochskalierung)
brauchen wahrscheinlich ein echtes neuronales Modell — dasselbe
ONNX-Beschaffungsproblem, das in ADR-0033 bereits für Phase 7 dokumentiert
ist (keine testbare Modell-Datei in dieser Sandbox verfügbar); ob es bis
zum Start von Phase 9 eine tragfähige Lösung gibt, ist offen und wird
dann neu geprüft, nicht hier vorweggenommen. Die übrigen neun Punkte sind
reine UI-/Analyse-Erweiterungen ohne bekannte Blocker.
