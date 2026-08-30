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

Analog zu §2/§5s Datenfluss-Abschnitten, hier für den Such-/Filter-Pfad
(seit Schritt 8, `DECISIONS.md` ADR-0027, kombinierbar statt alternativ):

Frontend `FilterBar`-Suchfeld/-Chips (Bewertung/Flagge/Farbe/Kameramodell)
→ Store `runLibrarySearchAndFilter` (liest den aktuellen Suchtext *und*
alle gesetzten Filter-Chips gemeinsam, keiner leert mehr den anderen) →
Tauri-Command `search_and_filter_photos` → `apx-catalog::Catalog::search_and_filter_photos`:
ohne Suchtext identisch zu `filter_photos` (dynamisch UND-verknüpfte
`WHERE`-Klausel aus den gesetzten Kriterien, leeres `FilterCriteria`
liefert alle Fotos); mit Suchtext zusätzlich `photos_fts MATCH ?1`
UND-verknüpft mit denselben Kriterien-Klauseln, sortiert nach FTS5-
Relevanz (`rank`) statt nach Dateiname → Ergebnis ersetzt `libraryResults`
im Store → `selectActivePhotos` liefert es (sortiert nach dem aktuell
gewählten Sortierfeld, siehe unten) statt des ausgewählten Ordners/der
Sammlung an Raster und Filmstreifen. Die separaten `search_photos`/
`filter_photos`-Commands bleiben additiv bestehen, werden vom Frontend
aber nicht mehr direkt aufgerufen.

**Duplikaterkennung (Schritt 8.2):** jede beim Import gestagte Datei
bekommt einen per Streaming berechneten SHA-256-Hash in
`photos.content_hash` (`import::compute_content_hash`). Am Ende von
`run_with_mode` gruppiert `Catalog::list_duplicate_photo_groups()` alle
Fotos mit identischem Hash; die Gesamtzahl betroffener Fotos geht als
`ImportFinishedPayload.duplicate_count` ans Frontend. Der „Duplikate
anzeigen"-Knopf in `FilterBar.tsx` ruft denselben Command direkt auf und
setzt die abgeflachten Gruppen als `libraryResults` — reine Anzeige über
denselben `selectActivePhotos`-Lesepfad, kein separater Anzeigemodus.

**Sortierung (Schritt 8.3):** bewusst client-seitig statt eines weiteren
`ORDER BY`-Parameters im Backend — `selectActivePhotos` wendet
`lib/sortPhotos.ts`s reine `sortPhotos`-Funktion als letzten Schritt auf
das Ergebnis von Ordner/Sammlung/Suche-Filter/Duplikatanzeige an, gesteuert
über `librarySortField`/`librarySortDirection` im Store.

Alle vier Pfade (Ordner, Sammlung, Suche/Filter, Duplikatanzeige) laufen
über dieselbe `libraryResults`-Zustandsvariable und denselben
`selectActivePhotos`-Lesepfad — kein separater „Suchergebnis-Modus" in
Raster/Filmstreifen nötig.

## 7. Platzhalter für spätere Phasen

Diese Abschnitte werden erst gefüllt, wenn die jeweilige Phase beginnt — hier nur benannt, damit die Zielarchitektur nicht aus dem Blick gerät:

- **Phase 6:** Maskensystem (siehe `DECISIONS.md` ADR-0032) — Ebenenmodell statt Fused-Pass: jede Maske ist ein eigener Pipeline-Durchlauf nach der Phase-4-Pipeline, der Maskenalpha berechnet, die ton-/farb-/detailbezogenen Werkzeuge auf eine Bildkopie anwendet und alpha-gewichtet mit ihrem Ebenen-Mischmodus zurückmischt. Neues EDL-Schema v3 (`masks: Vec<Mask>`). Plus die in `DECISIONS.md` ADR-0028 auf diese Phase versprochenen acht Workflow-Punkte (Schnappschüsse, Vorher/Nachher, Copy/Paste-Einstellungen, Vorherige übernehmen, Sync, Auto-Sync, Referenzansicht, Soft-Proof). Tiefenbereich-Masken bleiben ohne Phasenzuordnung zurückgestellt (kein Tiefendaten-Zulieferer); KI-Masken sowie die Reparatur-Erweiterungen (Auto-Quellenfindung, Content-Aware-Fill, Sensorflecken-Visualisierung) wandern nach Phase 7 (ADR-0032 Punkt 3/8); der Bibliotheks-Backlog aus §3.1 (Sammlungssätze, Stapel, virtuelle Kopien, Schlagworthierarchie, Metadaten-Presets/EXIF-IPTC-XMP-Editor, Vergleichsansicht, Filter-Presets, Smart Previews u. a.) wandert nach Phase 9 (ADR-0032 Punkt 7).
- **Phase 7:** `apx-ai` — ONNX-Runtime-Integration (Motiv-/Himmel-/Personen-Segmentierung für KI-Masken, siehe ADR-0032 Punkt 3), LLM-Client für Preset-Generator (siehe `DECISIONS.md` ADR-0031 Punkt 1: Referenzbild-Modus/Variationen-Generator/Preset-aus-Bearbeitung-Lernen sind dieselbe CV-/Optimierungs-Kategorie wie die ADR-0028-zurückgestellten Punkte und wandern deshalb ebenfalls hierher), sowie die Reparatur-Erweiterungen aus ADR-0032 Punkt 8 (dieselbe PatchMatch-artige CV-Kategorie).
- **Phase 8–9:** Export-Engine, Ausgabe-Module, Node-Editor, Stacking, Tethering, Skript-API/Plugins — auch Grundlage für die auf diese Phasen verschobenen Export-/Wasserzeichen-/Metadaten-/Layout-/Workflow-Templates und den Template-Marktplatz (ADR-0031 Punkt 5) sowie Adobe-`.xmp`/`.lrtemplate`-Interop (ADR-0031 Punkt 3).

## 9. Architektur Phase 5 — Preset- und Template-System

Beschreibt den tatsächlich gebauten Stand (siehe `DECISIONS.md` ADR-0031 für die Scope-Entscheidungen). Phase 5 fügt bewusst **kein** neues `apx-presets`-Crate hinzu (ADR-0031 Punkt 6): ein Preset ist reine Katalogdaten — eine benannte, versionierte EDL-*Teilmenge* — ohne neue Pixel-Verarbeitungslogik, analog zu `edit_history.edl_json`. Die gesamte Merge-/Skalierungs-/Bedingungslogik lebt im Frontend (`frontend/src/lib/presets.ts`); `apx-catalog`/`apx-app` reichen die EDL-Teilmenge nur als opaken JSON-String durch, exakt wie schon `edit_history` seit Phase 2.

**`apx-catalog`-Erweiterung** (additive Migration `0004_presets.sql`): `preset_folders` (Baum über `parent_id`, analog zu `folders`), `presets` (Metadaten: Name/Ordner/Favorit/Tags/`conditions_json`), `preset_versions` (1:n zu `presets`, `sequence` + `edl_subset_json` + `created_at`, keine Version wird je überschrieben — jede erneute Speicherung eines bestehenden Presets legt eine neue Zeile an, „aktuellste Version" ist eine Abfrage nach `MAX(sequence)`). Drei neue ID-Typen (`PresetFolderId`/`PresetId`/`PresetVersionId`) über das bestehende `define_id_type!`-Makro. `repository::presets`-Modul (neu) folgt dem „ein Modul pro Tabellen-Gruppe"-Muster; `Catalog`-Fassade bekommt ~13 neue Methoden, alle auf einfachen sequenziellen `Connection::execute`-Aufrufen ohne explizite Transaktion (Präzedenzfall: `repository::edits::commit`).

**`apx-app`-Commands** (`commands.rs`, ~14 neue `#[tauri::command]`-Funktionen): `PresetFolderDto`/`PresetDto`/`PresetVersionDto` als dünne DTO-Schicht über die Katalog-Structs; `ApxPresetFile { schema_version, name, tags, conditions, edl_subset }` fürs eigene `.apx`-Exportformat — bewusst mit *eingebetteten* `serde_json::Value`-Feldern für `conditions`/`edl_subset` (menschenlesbar beim Öffnen der Datei) statt der intern genutzten, nochmals als String verschachtelten `conditions_json`/`edl_subset_json`-Repräsentation; die Umwandlung zwischen beiden Formen passiert genau an der Export-/Import-Commandgrenze.

**Frontend-Datenmodell** (`lib/presets.ts`): `PresetSectionKey = Exclude<keyof EdlPayload, "repair">` (Reparatur ist nie Teil eines Presets — bildspezifische Klon-/Reparatur-Striche sind kein übertragbarer „Look"). `PresetEdlSubset = Partial<Pick<EdlPayload, PresetSectionKey>>`.

### Datenfluss Phase 5: Speichern → Anwenden → Stärke → Stapel

```
SavePresetDialog: ausgewählte Sektionen + Bedingungsregeln
  -> buildPresetEdlSubset(developEdl, sections)   (extrahiert nur die angehakten Sektionen)
  -> create_preset(folder, name, tags, conditions_json, edl_subset_json)
       -> preset_versions-Zeile #1 (erste Version)

PresetsPanel: Klick auf einen Preset-Namen
  -> applyPreset(presetId)
       -> latest_preset_version(presetId)                     (aktuellste EDL-Teilmenge)
       -> applyConditionsToSubset(subset, conditions, photoMeta)   (siehe unten)
       -> mergeEdlSubset(developEdl, subset)    (jede Sektion wird als Ganzes ersetzt, nie feldweise gemischt)
       -> commitDevelopEdit(..., { preservePresetStrengthContext: true })
       -> presetStrengthContext = { baseEdl, subset, strength: 100 }   (für den nachträglichen Stärke-Regler)

Stärke-Regler (0-200 %, solange kein anderer Edit dazwischenliegt):
  setPresetStrength(prozent)
       -> scalePresetEdlSubset(context.subset, prozent)   (skaliert numerische Blattwerte relativ zur
                                                             jeweiligen Neutralstellung; Arrays/Enums/Strings
                                                             unskaliert — dieselbe Einschränkung wie bei
                                                             Lightroom, siehe presets.ts-Moduldoku)
       -> mergeEdlSubset(context.baseEdl, scaled)          (immer NEU aus dem unveränderten baseEdl-Snapshot
                                                             berechnet, nie inkrementell — reproduzierbar bei
                                                             wiederholtem Hin-und-her-Schieben)
       -> nur developEdl aktualisiert (Live-Vorschau via useDevelopRender), commitPresetStrength() persistiert
          erst beim Loslassen

Preset-Stapel: applyPresetStack() wendet jedes Preset im Stapel sequenziell an (jedes bei 100 %, spätere
  Einträge überschreiben gemeinsame Sektionen früherer), committet einmal am Ende.
```

`commitDevelopEdit`s Signatur bekam dafür einen einzigen neuen optionalen Parameter (`{ preservePresetStrengthContext?: boolean }`) statt jeden der ~40 bestehenden Aufrufer anzufassen oder eine Generation-Zähler-Infrastruktur einzuführen — jeder *andere* Aufruf (Standardwert) löscht `presetStrengthContext` automatisch, was „Stärke bleibt änderbar, bis ein anderer Edit dazwischenkommt" (`SPEC.md` §3.5) genau abbildet.

**Bedingte Presets (vereinfacht, ADR-0031 Punkt 4):** `PresetCondition { field, op, value, section }` — feste Feldliste (ISO/Blende/Brennweite/Kameramodell/Objektiv, alle bereits in `photos`), Operatoren `>`/`<`/`=`/„enthält", UND-verknüpft, kein UI-Builder für ODER/Verschachtelung. `section: null` bedeutet „gilt fürs ganze Preset" (ein Fehlschlag verhindert das Anwenden komplett); eine gesetzte Sektion grenzt einen Fehlschlag auf genau diese Sektion ein. `evaluateCondition`/`applyConditionsToSubset` laufen sowohl in `applyPreset`/`applyPresetStack` als auch in der Hover-Vorschau und dem `PresetThumbnail` — dieselbe Auswertung überall, damit die Vorschau nie etwas zeigt, was `applyPreset` tatsächlich nicht anwenden würde. Ein fehlendes Metadatum am aktuellen Foto gilt konservativ als nicht erfüllt.

**Live-Vorschau** (Hover + Thumbnail, `SPEC.md` §3.5): rein visuell, ändert `developEdl` nie. `Viewer.tsx` berechnet `renderedEdl = hoverPresetSubset ? mergeEdlSubset(developEdl, hoverPresetSubset) : developEdl` nur für den ans Rendering übergebenen EDL-JSON. `PresetThumbnail.tsx` rendert bei niedriger Auflösung über einen eigenen Hook `useDevelopPreviewThumbnail`, der sich mit dem Haupt-Viewer-Hook `useDevelopRender` die rAF-debounced Fetch-/Parse-/Abort-Logik über `useDevelopFrameInternal` teilt, aber bewusst den globalen Latenz-Indikator (`developLastLatencyMs`) nicht berührt.

**Versionierung + Diff** (`PresetVersionsDialog.tsx`): „Aktuellen Stand als neue Version speichern" übernimmt dieselben Sektionen wie die bisher aktuellste Version (keine implizite Sektions-Erweiterung) und ruft `add_preset_version` auf. `diffEdlSubsets` vergleicht zwei EDL-Teilmengen feldweise (rekursiv in verschachtelte Objekte, Arrays als atomarer Wert — dieselbe Konvention wie `interpolateValue`s Umgang mit nicht-skalaren Preset-Bestandteilen) und listet jeden abweichenden Blattwert mit Pfad.

**Import-/Umbenennungs-Templates (vorgezogen aus Phase 3, ADR-0031 Punkt 7):** `import_folder_with_mode`/`list_import_presets`/`save_import_preset`/`delete_import_preset` existierten seit Phase 3 im Backend, hatten aber bis Phase 5 Schritt 9 keine Frontend-Anbindung. Neuer `ImportDialog.tsx`, additiv über einen zweiten „Import mit Vorlage…"-Knopf neben dem unveränderten einfachen „Ordner importieren" erreichbar. `lib/renamePattern.ts` bildet `crates/apx-app/src/import/rename.rs`s Token-Ersetzung (`{date}`/`{seq}`/`{camera}`/`{original}`) rein clientseitig für die Live-Vorschau nach — der eigentliche Import läuft weiterhin ausschließlich im Backend.

## 8. Architektur Phase 4 — Entwickeln vollständig

Beschreibt den tatsächlich gebauten Stand (siehe `DECISIONS.md` ADR-0028 bis ADR-0030 für die Scope-Entscheidungen samt der Korrekturen, die sich beim tatsächlichen Bauen ergaben — insbesondere ADR-0030, die die unten unter „Drei Dispatch-Formen" beschriebene Zuordnung gegenüber der ursprünglichen Vorab-Planung präzisiert).

**EDL-Schema v2** (`crates/apx-pipeline/src/edl/v2.rs`, `EDL_SCHEMA_VERSION = 2`): `EdlV1` bleibt unverändert bestehen (historische `edit_history`-Zeilen bleiben lesbar, `migrate.rs::from_envelope` versucht Version 2 zuerst und hebt `schema_version == 1` per `v1_to_v2` an, alles Neue auf neutral/leer). `EdlV2` fügt der um fünf Regler erweiterten `basic: BasicAdjustments` (Textur/Klarheit/Dunst entfernen/Dynamik/Sättigung) zehn weitere Unterstrukturen hinzu — je eine pro Werkzeugkategorie aus `SPEC.md` §5s Phase-4-Satz: `curves`, `hsl`, `color_mixer`, `color_grading`, `details`, `lens_corrections`, `effects`, `calibration`, `geometry`, `repair: Vec<RepairStroke>` (Liste statt Skalar — ein Stroke pro Klon-/Reparatur-Pinselzug, siehe unten). `frontend/src/lib/edl.ts` spiegelt dieselbe Struktur 1:1 (verschachtelte Sub-Objekte je Werkzeug, eigene Neutral-Konstanten und Builder-Helfer je Sektion, analog zum bestehenden `white_balance`-Unterobjekt aus Phase 2).

**Drei GPU-Dispatch-Formen** (`gpu/dispatch.rs::run_compute_f32` unverändert als gemeinsame Grundlage für alle drei, siehe ADR-0029 für die Entscheidung, sie schrittweise statt vorab zu bauen):
1. **1:1, positions-bewusst** (Breite/Höhe als zusätzliche `Params`-Uniform-Felder, aber jeder Invocation liest/schreibt nur seinen eigenen Pixel): der erweiterte `basic_fused`-Pass (Grundeinstellungs-Ergänzung, Kurven-Vorstufe entfällt — siehe unten), HSL/Farbmischer, Color Grading, Kalibrierung, Objektivkorrekturen (der geometrische Warp braucht zwar Nachbarschafts-*Lesezugriff* auf den Eingabepuffer, bleibt aber 1:1 in der Puffergröße, siehe Punkt 3 unten für die Abgrenzung), Effekte (Vignette/Körnung).
2. **Nachbarschafts-Zugriff** (2D-Dispatch, liest umliegende Pixel aus demselben Eingabepuffer): Textur/Klarheit (`local_contrast`, aus Phase 2 wiederverwendet), Details (Unschärfe-Referenz fürs Unsharp-Masking + lokale Mittelung für Rauschreduzierung), Reparatur (versetzter/gefilterter Lesezugriff für Klonen/Reparieren).
3. **Größenverändernd** (Ausgabepuffer ≠ Eingabepuffer): entgegen der ursprünglichen Vorab-Planung (die diese Form sowohl für Objektivkorrekturen als auch Crop/Geometrie vorsah) wird sie tatsächlich **nur von Geometrie** gebraucht — Objektivkorrekturens Warp bleibt bewusst größenerhaltend (Randpixel geklemmt statt automatisch zugeschnitten, siehe ADR-0030) und läuft deshalb als Form 1. Geometrie (Drehung + Zuschnitt) läuft zudem bewusst **CPU-only** (kein GPU-Dispatch, `stages/geometry.rs`, analog zu `curves.rs`) als allerletzter Schritt in `render_rgba8` — ein GPU-Rundtrip lohnt sich für einen pro Regler-Tick nur einmal laufenden Schritt auf dem bereits herunterskalierten Vorschaubild nicht.

**Vollständige Pipeline-Reihenfolge** (`develop::render_rgba8`, jede Stufe mit „Regelfall überspringen"-Kurzschluss, wenn ihr EDL-Anteil neutral/leer ist):

```
linear.pixels (Kamera-RGB, f32)
  -> repair            (Klon-/Reparatur-Striche, sequenziell je Strich, vor allem anderen — Flecken-
                         entfernung soll auf unveränderten Sensordaten passieren)
  -> calibration       (Prozessversion/Schattentönung/Primärfarben/Kameraprofil, vor Weißabgleich)
  -> basic_fused       (die zwölf Grundeinstellungs-Regler inkl. Weißabgleich-Gains)
  -> local_contrast    (Textur/Klarheit, Nachbarschafts-Dispatch)
  -> details           (Schärfung + Rauschreduzierung, Nachbarschafts-Dispatch)
  -> hsl_color_mixer   (HSL-Bänder + Farbmischer-Regionen)
  -> color_grading     (vier Farbräder)
  -> lens_corrections  (CA/Vignette/Verzeichnung/Upright/manuelle Transformation, ein kombinierter
                         geometrischer Warp, größenerhaltend)
  -> effects           (nachträgliche Vignettierung + Körnung)
  -> color::linear_camera_rgb_to_srgb_rgba8   (feste Matrix + Gammakurve + u8-Quantisierung — ab hier RGBA8)
  -> curves            (CPU-LUT auf dem fertigen u8-Puffer, siehe ADR-0029: Sequenzierungsfrage
                         zugunsten eines einfachen CPU-Nachschritts statt eines WGSL-Farbraum-Shaders
                         entschieden)
  -> geometry          (Drehung + Zuschnitt, CPU-only, ändert als einziger Schritt die Ausgabegröße)
  -> RenderedImage { width, height, pixels }
```

`RenderedImage` (statt eines nackten `Vec<u8>`) macht diese Größenänderung im Typsystem sichtbar — `apx-app::protocol::compute_develop` rahmt entsprechend `rendered.width`/`.height` (statt `linear.width`/`.height`) in den 8-Byte-Wire-Header; das Wire-Format selbst ist unverändert (war schon immer breiten-/höhen-präfixiert).

**Frontend-Widgets** (alle neu, `frontend/src/components/`): `CurveEditor.tsx` (Canvas, ziehbare Punkte, monotone kubische Spline + parametrischer Modus), `ColorWheel.tsx` (2D-Puck auf Farbton/Sättigungs-Scheibe, 4× für Color Grading), `CropOverlay.tsx` (Ziehgriffe + Rasterüberlagerungen), `RepairOverlay.tsx` (Quellpunkt-Klick + Zielpfad-Ziehen, rein clientseitige Live-Vorschau). Alle teilen sich, wo sinnvoll, Code mit bestehenden Mustern statt eigener Parallel-Logik: Farbmischer-Bildklick und Weißabgleich-Pipette teilen sich `lib/colorSampling.ts`; jedes neue `role="slider"`-Element mit eigenem `onKeyDown` braucht `event.stopPropagation()`, sobald es in ein anderes `role="slider"`-Element verschachtelt ist (zweimal in dieser Phase als echter Bug gefunden — `CropOverlay.tsx`s Ecken-Ziehgriffe und, aus früheren Phasen bekannt, `App.tsx`s globaler Tastatur-Listener).
