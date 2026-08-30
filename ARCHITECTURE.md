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

- **Phase 6:** Maskensystem — vollständig gebaut, siehe § 10 „Architektur Phase 6".
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

## 10. Architektur Phase 6 — Masken und lokale Anpassungen

Beschreibt den tatsächlich gebauten Stand (siehe `DECISIONS.md` ADR-0032 für die Scope-Entscheidungen samt der beim Bauen entdeckten Korrekturen — insbesondere Punkt 4 zur Pipeline-Platzierung und Punkt 6 zu Schnappschüssen/Soft-Proof).

**EDL-Schema v3** (`crates/apx-pipeline/src/edl/v3.rs`, `EDL_SCHEMA_VERSION = 3`): fügt `masks: Vec<Mask>` und `mask_groups: Vec<MaskGroup>` zu `EdlV2` hinzu (`v2_to_v3`/`from_v2`: beide starten leer, alle bestehenden Felder unverändert übernommen; `from_envelope` probiert v3 zuerst, danach v2, danach v1). Eine `Mask` besteht aus mehreren kombinierbaren `MaskComponent`s (`geometry: MaskGeometry` — Brush/LinearGradient/RadialGradient/ColorRange/LuminanceRange — plus `combine: MaskCombine` [Add/Subtract/Intersect] und `invert`) statt nur einem Geometrietyp, dazu `adjustments: MaskAdjustments` (Grundeinstellungen/Kurven/HSL/Farbmischer/Color Grading/Details — bewusst ohne Objektivkorrekturen/Effekte/Kalibrierung/Geometrie/Reparatur, siehe ADR-0032 Punkt 2), `opacity`/`feather`/`invert`/`blend_mode: BlendMode`/`group_id`/`visible`/`overlay_color`. `frontend/src/lib/edl.ts` spiegelt dieselbe Struktur (`Mask`/`MaskGeometry`/`MaskComponent`/`MaskAdjustments`/`BlendMode`/`MaskGroup`-Typen + Neutral-Konstanten/Builder).

**Ebenenmodell statt Fused-Pass** (ADR-0032 Punkt 4, `crates/apx-pipeline/src/stages/masks.rs`): jede Maske ist ein eigener, sequenzieller Pipeline-Durchlauf — Maskenalpha berechnen (je Geometrietyp eine analytische Funktion: `linear_gradient_alpha`/`radial_gradient_alpha`/`brush_alpha`/`color_range_alpha`/`luminance_range_alpha`), die Komponenten *derselben* Maske kombinieren (`MaskCombine::Add` = Maximum, `Subtract` = `c·(1-a)`, `Intersect` = `c·a`), die sechs Masken-Werkzeuge auf eine Bildkopie anwenden (dieselben Stufenfunktionen wie die globale Pipeline — `basic_fused`/`hsl_color_mixer`/`color_grading`/`details`/eine neue `curves::apply_linear_rgb`, siehe unten), dann alpha-gewichtet mit dem gewählten `BlendMode` zurückmischen. `develop.rs` hängt die gesamte Maskenstufe **direkt nach `effects`, vor der Farbraum-Konvertierung** ein — eine Korrektur gegenüber der ursprünglichen Planung „nach der Phase-4-Pipeline": Kurven laufen global erst *nach* der Farbraum-Konvertierung auf dem fertigen RGBA8-Puffer, während die übrigen fünf Masken-Werkzeuge im linearen Arbeitsraum *davor* laufen; da eine Maske alle sechs in einem Durchlauf anwendet, kann sie nicht an zwei Pipeline-Stellen zugleich sitzen — die gesamte Stufe bleibt deshalb im linearen Arbeitsraum, mit einer eigenen linearen Kurven-Variante statt einer verlustreichen zweiten Farbraum-Konvertierung pro Maske. `apply_all(pixels, width, height, wb_coeffs, masks, groups)` filtert über `visible_masks(masks, groups)` — **Nachtrag (Schritt 7, beim Bauen entdeckt):** dieser Aufruf fehlte ursprünglich (`apply_all` prüfte nur `mask.visible`, nie die Gruppensichtbarkeit), Gruppen-Ausblenden hatte dadurch bis zur Korrektur keinen Pipeline-Effekt.

**Vollständige Ebenen-Mischmodi** (`blend_pixel`, ersetzt das frühere `blend_channel`): Normal/Multiply/SoftLight sind per-Kanal separierbar; Color/Luminosity brauchen Ganz-Pixel-Verarbeitung über `luminosity`/`set_luminosity`/`clip_color` — dieselbe „SetLum"/„ClipColor"-Rezeptur wie der Photoshop-/W3C-Compositing-Standard. **Bewusste Vereinfachung:** die Formeln setzen einen ungefähr `0..=1`-Wertebereich voraus; `clip_color` faltet überschreitende lineare Lichter zurück in den Bereich statt sie unverändert durchzureichen (kein HDR-korrektes Verhalten versucht).

**Alle Masken-Bausteine bleiben CPU-only** (kein GPU-Dispatch für die Maskenstufe) — eine bewusste, durchgehend dokumentierte Zurückstellung; siehe „Performance-Nachmessung" unten für die empirische Prüfung, ob das im 16-ms-Budget tatsächlich ein Problem ist.

**Frontend-Maskenverwaltung** (`MasksPanel.tsx`): Liste mit Drag-&-Drop-Sortierung (die Anzeige-Reihenfolge ist zugleich die Pipeline-Anwendungsreihenfolge), Umbenennen/Sichtbarkeit/Duplizieren/„auf anderes Foto übertragen" (reuse desselben `current_develop_edit`/`apply_develop_edit`-Musters wie überall sonst für „ein anderes Foto lesen/schreiben"), Maskengruppen, wiederverwendbare Bausteine (bewusst nur clientseitig/session-lokal statt über die Presets-Katalog-Infrastruktur aus Phase 5 — ein katalogseitiges Pendant wäre dieselbe Größenordnung an Aufwand). Volle Sechs-Sektionen-Reglerabdeckung pro Maske über exakte Wiederverwendung derselben `DevelopSlider`/`CurveEditor`/`ColorWheel`-Komponenten wie `DevelopPanel.tsx`, gerichtet auf `mask.adjustments` statt `developEdl` — drei kleine UI-Konstanten (`CURVE_CHANNEL_TABS`/`COLOR_GRADING_WHEEL_TABS`/`DetailsSliderKey`) wurden dafür von `DevelopPanel.tsx` nach `lib/edl.ts` verschoben und exportiert. `selectedMaskComponentIndex` (Store) bestimmt, welche Komponente der Viewer/Pinsel/Farbklick gerade bearbeitet.

**Workflow-Punkte** (ADR-0028-Zusage, acht Punkte, alle gebaut):
- **Schnappschüsse**: eine eigene `snapshots`-Tabelle (Migration `0005_snapshots.sql`) mit unabhängiger EDL-Kopie je Schnappschuss — eine Korrektur gegenüber der ursprünglichen Planung „ein Schnappschuss ist ein benannter Verweis auf einen `edit_history`-Stand": `repository/edits.rs::commit` löscht jede „Zukunft" hart nach einem Rückgängig (ADR-0014), ein Verweis darauf könnte also verschwinden. Kein eigener Restore-Weg nötig — Anwenden ist derselbe `apply_develop_edit`-Aufruf wie jeder andere EDL-Stand.
- **Vorher/Nachher** (vier Ansichten): `BeforeAfterView.tsx`, Canvas-2D/`putImageData` statt des WebGL2-`QuadRenderer`s (keine Zoom/Pan-Transformation nötig), gespeist über denselben `useDevelopPreviewThumbnail`-Hook wie die Preset-Live-Vorschau aus Phase 5.
- **Kopieren/Einfügen/Vorherige übernehmen/Synchronisieren/Auto-Sync**: reuse desselben `PresetEdlSubset`/`buildPresetEdlSubset`/`mergeEdlSubset`-Mechanismus aus Phase 5, direkt auf `developEdl` statt einem gespeicherten Preset angewendet. „Vorherige" = das Foto, das unmittelbar vor dem aktuellen im Entwickeln-Modul offen war (`lastDevelopPhotoId`, in `loadDevelopStateForPhoto` gepflegt). „Synchronisieren" reuse desselben `targets`-Filtermusters wie `setPhotoRating`/`setPhotoFlag`, hier auf die *übrigen* markierten Fotos zugeschnitten. Auto-Sync überträgt bewusst immer alle Sektionen (keine granulare Auswahl im Auto-Fall).
- **Referenzansicht**: `ReferenceView.tsx`, zwei unabhängige `QuadRenderer`-Instanzen mit rein lokalem (nicht Store-gehaltenem) Zoom/Pan-Zustand je Bildhälfte — reuse derselben Geometrie-Helfer aus `lib/viewerMath.ts` wie der Haupt-Viewer, aber ohne dessen Overlay-/Werkzeug-Maschinerie. Das Referenzfoto wird statisch mit seinem letzten committeten Stand gezeigt.
- **Soft-Proof**: vollständig als rein clientseitige Nachbearbeitung des bereits gerenderten RGBA8-Vorschau-Puffers (`lib/softProof.ts`), keine Backend-/Pipeline-Änderung — `apx-pipeline` kennt bis heute nur eine feste Kamera→sRGB-Matrix plus Gammakurve, kein echtes ICC-Profil-Laden. Drei simulierte Zielprofile über einen Sättigungs-Kompressions-Faktor, Renderpriorität über einen Sättigungs-Schwellenwert, Farbumfangswarnung/Papierweiß als zwei weitere Overlay-/Kompressionsschritte auf demselben Puffer.

### Datenfluss Phase 6: Maskenstufe im Rendering

```
develop::render_rgba8 (Phase-4-Pipeline unverändert bis "effects")
  -> masks::apply_all(pixels, w, h, wb_coeffs, edl.masks, edl.mask_groups)
       für jede sichtbare Maske (visible_masks filtert nach mask.visible UND Gruppensichtbarkeit):
         components kombinieren -> alpha
         Bildkopie mit mask.adjustments bearbeiten (dieselben Stufenfunktionen wie global)
         alpha-gewichtet per mask.blend_mode zurückmischen (blend_pixel)
  -> color::linear_camera_rgb_to_srgb_rgba8   (wie Phase 4, RGBA8 ab hier)
  -> curves (linear für Masken bereits erledigt; global weiterhin nach der Konvertierung)
  -> geometry
  -> RenderedImage
```

**Performance-Nachmessung** (ADR-0032 Punkt 4 nannte dies als offenes Risiko): mit mehreren gleichzeitig aktiven, komplexen Masken (je ein eigener sequenzieller Durchlauf durch alle sechs Werkzeuge) steigt die Rechenzeit linear mit der Maskenzahl — anders als der einmalige Fused-Pass der globalen Pipeline. Die tatsächliche Messung samt Ergebnis steht in `PLAN.md` Schritt 11.

## 11. Architektur Phase 7 — KI-Funktionen

Beschreibt den tatsächlich gebauten Stand (siehe `DECISIONS.md` ADR-0033 für die Scope-Entscheidungen — insbesondere Punkt 1/2 zur bewussten Absage an echte ONNX-Modellinferenz und Punkt 4 zu den Reparatur-Erweiterungen).

**Neues Crate `apx-ai`** (`crates/apx-ai`, hängt von `apx-core`/`apx-raw`/`apx-pipeline`/`apx-catalog` ab, dazu `reqwest` [rustls-tls] für den LLM-Client): bündelt alle fünf KI-Funktionsbereiche aus `SPEC.md` §5 als eigenständige Module, jedes mit seinem eigenen `apx-ai::AiError` (`Analysis`/`MissingApiKey`/`LlmRequest`/`LlmResponseUnparsable`), umgewandelt in `AppError::Ai` für die Tauri-Command-Grenze — dieselbe Konvention wie `apx_pipeline::PipelineError`/`AppError::pipeline`. `color.rs`/`blur.rs` sind gemeinsame, reine Bausteine (YCbCr/Luminanz/Sättigung, ein dreifacher Box-Filter als Gauß-Approximation); das bilineare Alpha-Resampling für KI-Masken lebt dagegen bewusst in `apx_core::raster` statt in `apx-ai` — `apx-pipeline` braucht es beim Rendern (`stages/masks.rs::ai_generated_alpha`), dürfte aber nicht von `apx-ai` abhängen (Zyklus), und `apx-core` steht schon unter beiden.

**Die fünf KI-Masken sind klassische, deterministische Bildverarbeitungsheuristiken, keine echten neuronalen Netze** (`apx-ai::segmentation`, ADR-0033 Punkt 1/2 begründet ausführlich, warum ein „Bring-your-own-ONNX-Modell"-Pfad ohne verifizierbare Gewichte nur eine ungetestete Hülle wäre): Motiv per Center-Surround-Saliency (Kontrast eines Pixels gegen seine weit weichgezeichnete Umgebung, sättigungsgewichtet), Himmel per Farbton-/Helligkeits-/Positions-Heuristik, Hintergrund als Komplement des Motivs, Objekte per farbtoleranzbasiertem Region-Growing ab einem Klickpunkt, Personen per Hautton-Erkennung im YCbCr-Raum (eine einzelne Region, keine Einzelteile). Jede liefert eine `Vec<u8>`-Alpha-Bitmap bei fester `ANALYSIS_MAX_EDGE = 512`-Auflösung — `MaskGeometry::AiGenerated { ai_kind: AiMaskKind, width, height, alpha }` (EDL-Schema unverändert v3, additive Variante) speichert sie direkt im EDL, `stages/masks.rs::ai_generated_alpha` skaliert beim Rendern per `bilinear_resize_u8` auf die tatsächliche Zielauflösung hoch — dieselbe Ebenenmodell-Pipeline wie jede andere Maskengeometrie aus Phase 6, keine Sonderbehandlung nötig.

**Reparatur-Erweiterungen** trennen scharf zwischen einmaligen Analyse-Befehlen und dem render-zeitlichen Pipeline-Pfad (ADR-0033 Punkt 4): Auto-Quellenfindung (`apx-ai::repair_analysis::suggest_source_point`, normierte Kreuzkorrelation über einen festen Ring von Kandidatenpositionen) und Sensorflecken-Visualisierung (`detect_spots`, Blob-Erkennung per lokaler Kontrast-Anomalie gegen ein weichgezeichnetes Referenzbild) laufen als gewöhnliche Tauri-Commands auf Abruf. Inhaltsbasiertes Füllen (`RepairMode::ContentAwareFill`) dagegen bleibt in `apx_pipeline::stages::repair` — es läuft bei *jedem* Rendering, nicht nur einmal auf Knopfdruck, und ist deshalb kein `apx-ai`-Analysebefehl, sondern ein vierter Pipeline-Modus neben Klonen/Reparieren. Umsetzung: vereinfachtes PatchMatch (Nächster-Nachbar-Vorbelegung als Startzustand, damit auch ein Loch größer als der Patch-Radius eine gültige Vergleichsbasis hat, dann Zufallsinitialisierung, Propagation von Nachbar-Versätzen, Zufallssuche mit schrumpfendem Radius) statt eines vollständigen Multi-Skalen-PatchMatch — **beim Bauen entdeckte Korrektur:** eine erste Fassung ohne die Nächster-Nachbar-Vorbelegung ließ Pixel tief im Inneren eines großen Lochs komplett unverändert (jede Patch-Vergleichsposition lag selbst im Loch, die Kosten blieben für jeden Kandidaten `f32::MAX`) — behoben, indem `patch_distance` jetzt gegen ein fortlaufend aktualisiertes Arbeitsbild vergleicht statt gegen die rohen, maskierten Originalpixel.

**Preset-Generator** (`apx-ai::preset_generator`) bündelt vier unabhängige Erzeugungsarten hinter einer gemeinsamen Darstellung — einer `serde_json::Value`-Teilmenge mit genau den zehn `PresetSectionKey`s aus `frontend/src/lib/presets.ts`:
- **LLM-Modus** ruft die Anthropic-Messages-API direkt per `reqwest`-JSON auf (kein offizielles Rust-SDK vorhanden) und bittet das Modell per System-Prompt, eine EDL-Teilmenge als reines JSON-Objekt zurückzugeben. **Serverseitige Validierung statt Vertrauen in die Modellantwort:** die Antwort wird per `merge_json_patch` (rekursiver Merge im Stil von JSON Merge Patch, RFC 7396) auf ein neutrales `EdlV3::neutral()` gemergt und vollständig als `EdlV3` deserialisiert — schlägt das fehl (halluziniertes Feld, falscher Typ, unbekannte Sektion), wird der ganze Aufruf abgelehnt statt ein kaputtes Preset durchzureichen. **Beim Bauen entdeckte Korrektur:** eine erste Fassung ersetzte jede oberste Sektion (z. B. `"basic"`) beim Mergen komplett durch die Modellantwort statt rekursiv Feld für Feld — ein knappes `{"basic": {"exposure_ev": 0.5}}` ohne die übrigen elf `BasicAdjustments`-Felder scheiterte dadurch an der Deserialisierung, obwohl der Wunsch eindeutig war; der rekursive Merge behoben lässt jedes nicht genannte Unterfeld beim neutralen Wert. Der Anthropic-API-Schlüssel liegt im Klartext in `apx-core::Settings::ai` (dieselbe Vertrauensgrenze wie z. B. der zuletzt geöffnete Katalogpfad — ein lokales, nicht synchronisiertes Profil), zwei neue Tauri-Commands (`get_ai_settings`/`set_anthropic_api_key`) lesen/schreiben es.
- **Manueller LLM-Modus ohne API-Schlüssel:** `standalone_prompt_text` fügt denselben System-Prompt mit der Beschreibung zu einer einzigen Nachricht zusammen (die Chat-Oberfläche der Claude-App kennt kein separates `system`-Feld) — der Nutzer kopiert sie in claude.ai, kopiert die Antwort zurück, `parse_and_validate_pasted_json` prüft sie mit derselben serverseitigen Validierung wie der API-Pfad, nur ohne den Netzwerk-Aufruf selbst. Zwei zusätzliche Tauri-Commands (`build_preset_prompt_text`/`import_preset_json`) — reine Text-/JSON-Verarbeitung, kein API-Schlüssel, keine Einstellungen nötig.
- **Referenzbild-Modus** braucht kein LLM: Koordinatenabstieg über die sechs tonwertbezogenen Grundeinstellungs-Parameter (Belichtung/Kontrast/Lichter/Tiefen/Weiß/Schwarz), der die Distanz zwischen der simulierten Luminanzverteilung des aktuellen Fotos und der eines beliebigen Referenzbilds minimiert. **Beim Bauen entdeckte Korrektur:** ein roher Bin-für-Bin-Histogrammvergleich ist für schmale/einfarbige Verteilungen nicht monoton in der Verschiebung (zwei nicht überlappende Spitzen ergäben unabhängig von ihrem Abstand denselben Wert, der Abstieg fände nie eine Verbesserung) — durch den Wechsel auf die Distanz der *Kumulativsummen* (die diskrete Earth-Mover's-/Wasserstein-1-Distanz) behoben, die stetig mit der tatsächlichen Annäherung sinkt.
- **Variationen-Generator**: deterministisch geseedeter xorshift32-PRNG (dieselbe Konstruktion wie `repair.rs`s PatchMatch-Zufallszahlen) stört jeden numerischen Blattwert eines Basis-Presets um einen kleinen, vom Wertbetrag abhängigen Betrag; derselbe `seed` liefert reproduzierbar dieselben Varianten (Kontaktbogen-Vorschau im Frontend darf nicht bei jedem Neu-Rendern flackern).
- **Preset aus Bearbeitung lernen**: mittelt numerische Blattwerte über die *aktuell committeten* EDL-Teilmengen mehrerer ausgewählter Fotos (arithmetisches Mittel je Pfad); strukturierte Listen (Kurvenpunkte, Farbmischer-Regionen, Objektivkorrektur-Hilfslinien) werden unverändert vom ersten Foto übernommen statt sinnvoll zusammengeführt — dieselbe Einschränkung, die `frontend/src/lib/presets.ts::interpolateValue` für die Preset-Stärke bereits für nicht-skalare Werte dokumentiert.

Alle vier Erzeugungsarten liefern nur eine EDL-Teilmenge zurück, committen aber nichts selbst — das Frontend zeigt sie als Vorschlag (`presetGeneratorPreview`), „Auf aktuelles Foto anwenden" mischt ihn wie ein gewöhnliches Preset in `developEdl` (`mergeEdlSubset`), und der Nutzer sichert ihn danach über den bereits bestehenden „Preset speichern"-Knopf aus Phase 5 — der Generator braucht dafür keine eigene Speicher-Logik.

**Auto-Tagging** (`apx-ai::tagging::suggest_tags`): kombiniert Flächenanteile der Himmel-/Personen-KI-Masken-Heuristiken (oberhalb eines festen Schwellenwerts) mit groben EXIF-Faustregeln (hoher ISO → „Wenig Licht", große Blendenöffnung → „Freistellung", Brennweite → „Tele"/„Weitwinkel") zu einer Vorschlagsliste — reine Vorschläge, schreibt nichts in den Katalog; das Frontend übernimmt ausgewählte Vorschläge über das bestehende `add_photo_keyword` aus Phase 3.

**Tauri-Command-Schicht bleibt reine Verdrahtung** (`crates/apx-app/src/commands.rs`, neuer Abschnitt „KI-Funktionen"): jeder neue Command dekodiert wie `compute_develop` über `TileCache::get_or_decode` (Analyse-Commands nutzen einheitlich `apx_ai::segmentation::ANALYSIS_MAX_EDGE` als Zielauflösung, nicht die volle Vorschau-Auflösung) und reicht direkt an die passende `apx-ai`-Funktion durch; keine Geschäftslogik wandert hierher.

### Datenfluss Phase 7: KI-Maske erzeugen

```
Frontend-Klick (fünf Knöpfe, "Objekte" braucht zusätzlich einen Bildklick)
  -> generate_ai_mask(photo_id, kind, click_x?, click_y?, tolerance?)
       TileCache::get_or_decode (ANALYSIS_MAX_EDGE-Auflösung)
       -> apx_ai::segmentation::generate(kind, pixels, w, h, click, tolerance)
       -> AiMaskAlphaDto { kind, width, height, alpha_base64 }
  <- Frontend: Base64 dekodieren, MaskGeometry::AiGenerated einer neuen Maske zuweisen, sofort committen
  -> beim nächsten Rendern: stages/masks.rs::ai_generated_alpha (bilineares Hochskalieren) wie jede andere Maskengeometrie
```

**Nicht in Phase 7** (bewusst zurückgestellt, siehe `PLAN.md`): echte ONNX-Runtime-Modellinferenz, Einzelregionen der Personen-Maske (Augen/Brauen/Lippen/Zähne/Haare/Kleidung einzeln wählbar), Tiefenbereich-Masken (weiterhin ohne Phasenzuordnung, siehe ADR-0032 Punkt 3).
