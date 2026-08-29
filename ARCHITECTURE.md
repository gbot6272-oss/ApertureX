# ARCHITECTURE.md — Aperture X

Dieses Dokument beschreibt die Systemarchitektur, so wie sie mit Phase 1 begonnen wird, und den Zielzustand, auf den spätere Phasen aufbauen. Details, die erst in einer späteren Phase entstehen (GPU-Pipeline, Masken, Presets …), werden hier nur als Platzhalter-Abschnitt benannt und in der jeweiligen Phase ausgefüllt — nicht vorab spekulativ ausdesignt.

---

## 1. Grobstruktur

```
┌─────────────────────────────────────────────────────────────┐
│  Frontend (React 19 + TS, Vite)                              │
│  – reines UI-Rendering, keine Bildverarbeitung                │
│  – Zustand-Store (catalog/selection/viewer/jobs)               │
└───────────────┬─────────────────────────────┬─────────────────┘
                │ Tauri-Commands (Steuerung)   │ Custom-Protokoll
                │ z. B. import_folder,         │ apx://preview/…
                │ list_photos, cancel_job      │ apx://image/…
┌───────────────▼─────────────────────────────▼─────────────────┐
│  apx-app (Tauri-Binary)                                        │
│  – reine Verdrahtung: Commands, IPC-Events, Protokoll-Handler  │
│  – KEINE Geschäftslogik                                        │
└───────┬───────────────────────┬─────────────────────┬──────────┘
        │                       │                     │
┌───────▼───────┐     ┌─────────▼────────┐   ┌────────▼────────┐
│  apx-raw       │     │  apx-catalog     │   │  apx-pipeline    │
│  RAW-Decode,   │     │  SQLite, Repos,  │   │  (ab Phase 2)    │
│  Metadaten,    │     │  Migrationen     │   │  EDL, wgpu-      │
│  Vorschau       │     │                  │   │  Compute, Tile-  │
│                │     │                  │   │  Cache, lcms2    │
└───────┬────────┘     └─────────┬────────┘   └────────┬─────────┘
        │                        │                      │
        │            ┌───────────┴──────────────────────┘
        │            │  (apx-pipeline hängt zusätzlich von apx-raw ab,
        │            │   siehe Abhängigkeitsregel unten — im Diagramm
        │            │   der Übersicht halber nicht extra eingezeichnet)
        └──────────┬─┘
                    │
              ┌─────▼─────┐
              │ apx-core  │
              │ IDs, Fehler,
              │ Pfade, Settings,
              │ Logging, EdlEnvelope
              └───────────┘
```

**Abhängigkeitsregel:** Pfeile zeigen nur nach unten zu `apx-core`. `apx-raw` und `apx-catalog` kennen sich gegenseitig nicht — Verknüpfung (z. B. „Metadaten aus `apx-raw` in `apx-catalog` schreiben") passiert ausschließlich in `apx-app`. Das hält die Fach-Crates unabhängig testbar und verhindert, dass sich Datenbank- und Decode-Logik vermischen. Seit Phase 2 gilt zusätzlich: `apx-pipeline` hängt von `apx-core` **und** `apx-raw` ab (braucht dessen dekodierte Pixeldaten als Eingabe), aber ausdrücklich **nicht** von `apx-catalog` — die Pipeline transformiert Pixel anhand eines ihr übergebenen EDL-Werts, sie weiß nichts von SQLite. `apx-catalog` speichert das EDL nur als undurchsichtigen, versionsmarkierten Umschlag (`apx_core::EdlEnvelope`, siehe ADR-0013) und hängt umgekehrt nicht von `apx-pipeline` ab.

---

## 2. Datenfluss Phase 1

**Import:**
`apx-app::ImportJob` scannt einen Ordner (`walkdir`) → für jede Datei `apx-raw::read_metadata()` → Zeile via `apx-catalog`-Repository in `photos` → Worker-Pool erzeugt Thumbnail (`apx-raw::extract_embedded_preview()` oder Half-Size-Decode) → Datei landet im Preview-Cache-Verzeichnis, Pfad in `previews` → Fortschritt als Tauri-Event ans Frontend.

**Anzeige:**
Frontend fordert `apx://preview/<id>?level=0` bzw. `apx://image/<id>?max_edge=…` an → Protokoll-Handler in `apx-app` liest Cache oder ruft `apx-raw::decode()` in `spawn_blocking` auf → Antwort als JPEG/PNG/RGBA mit Cache-Headern → `<img>`/`createImageBitmap` im Viewer.

**Persistenz:**
Jeder Import-Schritt schreibt sofort in SQLite (WAL), nicht erst am Job-Ende — ein Absturz mitten im Import verliert daher höchstens die aktuell laufende Datei, nicht den gesamten Fortschritt.

---

## 3. Technologiewahl — Begründung (Kurzfassung, Details in `DECISIONS.md`)

| Entscheidung | Wahl | Warum |
|---|---|---|
| Shell | Tauri 2 | Vorgabe aus `SPEC.md`; kleiner Bundle- und Speicher-Footprint gegenüber Electron, Rust-natives Backend passt zur GPU-Pipeline späterer Phasen |
| DB-Zugriff | `rusqlite` (bundled) statt `sqlx` | Kein Compile-Zeit-DB-Zwang/Offline-Cache-Pflege in einer Phase, in der sich das Schema noch häufig ändert. Siehe ADR-0001 |
| RAW-Decode | `rawler` | Einzige Rust-Bibliothek mit breiter Formatabdeckung und aktiver Pflege ohne C-FFI; Lizenzimplikation siehe ADR-0002 |
| Bildübertragung | Custom-Protokoll-Handler statt Base64-IPC | Vermeidet Speicher-Overhead und Ruckeln bei großen Bildern, siehe „Bekannte Fallstricke" in `PHASE1_PROMPT.md` |
| Viewer Phase 1 | Canvas 2D + `ImageBitmap` | WebGL/WebGPU kommt erst in Phase 2 zusammen mit der echten GPU-Pipeline; für reines Betrachten reicht 2D-Canvas und hält Phase 1 schlank |

---

## 4. Modulgrenzen — Regeln

- **`apx-core`**: darf niemals von einem anderen Workspace-Crate importiert werden umgekehrt; enthält nur Typen/Fehler/Pfade/Settings/Logging, keine Fachlogik.
- **`apx-raw`**: reine Funktionsbibliothek (Pfad rein, Bilddaten/Metadaten raus). Kein Zugriff auf die Datenbank, kein Zugriff auf Tauri-APIs.
- **`apx-catalog`**: gesamtes SQL lebt hier, nirgendwo sonst. Öffentliche API sind typisierte Repository-Funktionen, kein rohes `Connection`-Handle nach außen.
- **`apx-app`**: verdrahtet die Crates, hält Tauri-Commands/Events/Protokoll-Handler, Job-Orchestrierung (`ImportJob`). Enthält selbst keine Bildverarbeitung und kein SQL.
- **`frontend/`**: kennt weder Dateisystempfade noch SQL noch Bildpuffer — nur Tauri-Commands, das `apx://`-Protokoll und Anzeige-/Interaktionslogik.
- **`apx-pipeline`** (ab Phase 2): reine Bildverarbeitungs-Bibliothek wie `apx-raw` — nimmt dekodierte Pixeldaten (`apx-raw::LinearImage`) und einen EDL-Wert entgegen, gibt gerenderte Pixel zurück. Kein Zugriff auf die Datenbank, kein Zugriff auf Tauri-APIs, kein Wissen darüber, wie/ob ein EDL dauerhaft gespeichert wird.

---

## 5. Architektur Phase 2 — `apx-pipeline`

Beschreibt den tatsächlich gebauten Stand (nicht mehr die Vorab-Planung aus Schritt 0 — siehe `DECISIONS.md` ADR-0011 bis ADR-0021 für die einzelnen Entscheidungen samt der Korrekturen, die sich beim tatsächlichen Bauen ergaben).

**Modulaufbau von `apx-pipeline`:**
```
apx-pipeline/
  lib.rs            // öffentliche API: GpuContext, EdlV1, develop::render_rgba8()
  error.rs          // PipelineError (thiserror, wie apx_core::AppError)
  edl/
    mod.rs          // öffentliche Re-Exports, EDL_SCHEMA_VERSION
    v1.rs           // EdlV1/BasicAdjustments/WhiteBalanceAdjustment (7 Regler-Felder, typisiert)
    migrate.rs       // Umwandlung EdlEnvelope <-> EdlV1, Upgrade-Kette für künftige Schema-Versionen
  color/            // feste Kamera->sRGB-Matrix + Gammakurve, RGBA8-Quantisierung (kein lcms2, siehe ADR-0019)
  gpu/
    mod.rs          // GpuContext (Instance/Adapter/Device/Queue)
    dispatch.rs     // gemeinsamer Puffer-hoch/Shader-ausführen/-runter-Helfer
  stages/           // je ein Modul pro Regler + ein fusionierter Shader
    white_balance.rs
    exposure.rs
    contrast.rs
    highlights_shadows.rs
    whites_blacks.rs
    basic_fused.rs
  develop.rs        // Orchestrierung: Weißabgleich-Gains -> basic_fused -> color -> RGBA8 (der einzige Einstiegspunkt, den apx-app aufruft)
  tile_cache.rs     // kleiner handgerollter LRU-Cache für das teure decode_linear-Ergebnis pro Foto+Auflösung
  test_support.rs   // #[cfg(test)]-only: gemeinsame synthetische Testmuster (ramp/gray_gradient/saturated_channels)
```

**Abhängigkeitsrichtung:** `apx-pipeline` → `apx-core` + `apx-raw`. Nicht `apx-catalog` (siehe Diagramm/Regel oben). `apx-app` hängt zusätzlich von `apx-pipeline` ab und verdrahtet es wie die anderen drei Crates, ohne eigene Bildverarbeitungslogik.

**EDL-Speicherung:** `apx-core` bekommt einen minimalen, versionsmarkierten Umschlagtyp `EdlEnvelope { schema_version: u32, payload: serde_json::Value }`. `apx-catalog` speichert nur diesen Umschlag (als `TEXT`/JSON in einer `edit_history`-Tabelle, siehe `migrations/0002_edits.sql`) und muss den `payload`-Inhalt nie verstehen. Nur `apx-pipeline` kennt die konkrete `EdlV1`-Struct und entpackt `payload` in sie (`edl::migrate::from_envelope`/`to_envelope`). Das hält die Abhängigkeitsrichtung sauber (kein `apx-catalog` → `apx-pipeline`).

**`apx-raw`-Grenze:** Ein additiver Einstiegspunkt `apx_raw::decode_linear()` deckt die Schritte RAW-Dekodierung/Demosaicing/Normalisierung ab und hört **vor** dem bisher fest einprogrammierten Weißabgleich/Gamma auf (die bestehende `decode()`/`DecodedImage`-API für Phase-1-Aufrufer wie Vorschaubilder bleibt unverändert). Er liefert ein `LinearImage` mit den As-shot-Weißabgleich-Koeffizienten **und** der festen Kamera→sRGB-Matrix (`cam_to_srgb`) — Letztere wird zwar erst von `apx-pipeline::color` angewendet, aber von `apx-raw` berechnet, weil dort bereits die Kamera-Kalibrierungsdaten vorliegen (siehe ADR-0019).

### Datenfluss Phase 2: Regler → Pixel

Analog zu §2s Phase-1-Datenfluss, hier für den neuen interaktiven Entwickeln-Pfad:

**Regler-Tick (Ziehen, noch nicht committet):**
Frontend `DevelopSlider.onChange` → Store `setBasicField` (nur In-Memory, kein IPC) → `Viewer` berechnet `edlJson` aus dem aktuellen `developBasic` (`lib/edl.ts::buildEdlEnvelopeJson`) → `useDevelopRender` schickt frühestens im nächsten `requestAnimationFrame` einen `fetch()` an `apx://develop/<id>/<max_edge>/<edl_json>` → `apx-app::protocol::compute_develop`: `apx_core::EdlEnvelope::from_json_str` + `apx_pipeline::edl::from_envelope` (Validierung) → `TileCache::get_or_decode` (Cache-Treffer bei jedem Tick nach dem ersten desselben Fotos — `apx_raw::decode_linear()` läuft nur einmal) → `apx_pipeline::develop::render_rgba8`: `white_balance::compute_gains` → `basic_fused::apply_gpu` (mit automatischem CPU-Fallback) → `color::linear_camera_rgb_to_srgb_rgba8` (feste Matrix + Gammakurve) → 8-Byte-Breite/Höhe-Header + rohes RGBA8 zurück → Frontend lädt die Bytes direkt als WebGL2-Textur (`lib/webgl.ts::QuadRenderer`), kein Dekodierschritt nötig.

**Commit (Loslassen/Blur/Doppelklick-Reset):**
`DevelopSlider.onCommit` → Store `commitDevelopEdit` → Tauri-Command `apply_develop_edit` → `apx-catalog::Catalog::commit_edit` schreibt einen neuen `edit_history`-Schnappschuss und rückt `edit_current` nach (verwirft dabei eine zuvor per Undo erreichte „Zukunft", siehe ADR-0014).

**Undo/Redo:** Store `undoDevelop`/`redoDevelop` → Tauri-Commands `undo_develop_edit`/`redo_develop_edit` → bewegen nur den `edit_current`-Zeiger (keine neue Zeile, keine gelöschte Zeile) → Antwort (`HistoryPosition`) wird zu `BasicAdjustments` entpackt und ersetzt `developBasic` direkt — kein separates Frontend-Verlaufssystem (siehe ADR-0018s Korrektur-Notiz).

## 6. Architektur Phase 3 — Bibliothek

Beschreibt den tatsächlich gebauten Stand (siehe `DECISIONS.md` ADR-0022 bis ADR-0025 für die einzelnen Entscheidungen). Phase 3 fügt keine neuen Crates hinzu — sie erweitert `apx-catalog` (Schema/Repositories), `apx-app` (Commands/Import) und `frontend/` (Raster/Sammlungen/Filter/Metadaten) innerhalb der bestehenden Modulgrenzen aus §4.

**`apx-catalog`-Erweiterung** (additive Migration `0003_library.sql`): `photos` bekommt `rating`/`flag`/`color_label` als direkte Skalarspalten (konsistent mit dem bestehenden `missing`-Muster); neue Tabellen `keywords`/`photo_keywords` (flache Schlagwort-Liste), `collections`/`collection_photos` (rein manuelle Sammlungen mit `position`-Reihenfolge); `photos_fts` als FTS5-**External-Content**-Virtualtabelle über `photos` (referenziert Originalspalten statt sie zu duplizieren) mit `INSERT`/`UPDATE`/`DELETE`-Sync-Triggern. Repository-Module folgen weiter „ein Modul pro Tabelle" (`repository::{keywords, collections, search}` neu; Bewertung/Flagge/Farbe bewusst in `repository::photos` statt eigenem Modul, da dieselbe Tabelle).

**Import-Erweiterung**: `apx-app::import::mode::ImportMode { AddInPlace, Copy, Move }` — bei Copy/Move wird die Datei vor dem unveränderten Scan-/Metadaten-/Thumbnail-Ablauf in einen Zielordner kopiert/verschoben, optional per Tokensystem (`import::rename`) umbenannt. Additiv zum bestehenden `import_folder`-Command (der weiterhin Add-in-Place-only bleibt) über einen neuen `import_folder_with_mode`-Command.

**Frontend-Erweiterung**: Raster (`GridView.tsx`) und Filmstreifen (`Filmstrip.tsx`) teilen sich Fotoliste und Mehrfachauswahl über eine gemeinsame Store-Funktion `selectActivePhotos` statt eigener Parallel-Logik (siehe ADR-0024) — diese priorisiert ein aktives Such-/Filterergebnis vor einer ausgewählten Sammlung vor einem ausgewählten Ordner. Bewertung/Flagge/Farbe wirken bei aktiver Mehrfachauswahl auf alle ausgewählten Fotos zugleich (Stapel-Bearbeitung).

### Datenfluss Phase 3: Suche/Filter → Ergebnisliste

Analog zu §2/§5s Datenfluss-Abschnitten, hier für den neuen Such-/Filter-Pfad:

**Freitextsuche:** Frontend `FilterBar`-Suchfeld → Store `runLibrarySearch` → Tauri-Command `search_photos` → `apx-catalog::Catalog::search_photos`: `SELECT … FROM photos_fts JOIN photos ON photos.rowid = photos_fts.rowid WHERE photos_fts MATCH ?1 ORDER BY rank` (FTS5-Relevanzsortierung) → Ergebnis ersetzt `libraryResults` im Store → `selectActivePhotos` liefert es statt des ausgewählten Ordners/der Sammlung an Raster und Filmstreifen.

**Attributfilter:** Frontend `FilterBar`-Chip (Bewertung/Flagge/Farbe) → Store `setLibraryFilterChip` (kombiniert alle gesetzten Chips im Store, leert dabei die Freitextsuche — beide sind laut `PLAN.md` Phase 3 Schritt 6 bewusst alternativ) → Tauri-Command `filter_photos` → `apx-catalog::Catalog::filter_photos`: baut dynamisch eine UND-verknüpfte `WHERE`-Klausel aus den gesetzten Kriterien (leeres `FilterCriteria` liefert alle Fotos) → Ergebnis ersetzt `libraryResults` wie bei der Suche.

Beide Pfade laufen über dieselbe `libraryResults`-Zustandsvariable und denselben `selectActivePhotos`-Lesepfad — kein separater „Suchergebnis-Modus" in Raster/Filmstreifen nötig.

## 7. Platzhalter für spätere Phasen

Diese Abschnitte werden erst gefüllt, wenn die jeweilige Phase beginnt — hier nur benannt, damit die Zielarchitektur nicht aus dem Blick gerät:

- **Phase 4:** Volles Entwickeln-Modul als weitere Pipeline-Stufen (über die 7 Grundregler aus Phase 2 hinaus).
- **Phase 5:** `apx-presets` — Preset-/Template-Engine, Adobe-Interop.
- **Phase 6:** Maskensystem als eigene Pipeline-Stufe(n).
- **Phase 7:** `apx-ai` — ONNX-Runtime-Integration, LLM-Client für Preset-Generator.
- **Phase 8–9:** Export-Engine, Ausgabe-Module, Node-Editor, Stacking, Tethering, Skript-API/Plugins.
