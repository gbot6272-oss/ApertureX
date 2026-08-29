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

Ersetzt den bisherigen Platzhalter-Einzeiler jetzt, wo die Entscheidungen getroffen sind (siehe `DECISIONS.md` ADR-0011 bis ADR-0018 für die Begründungen).

**Modulaufbau von `apx-pipeline`:**
```
apx-pipeline/
  lib.rs            // öffentliche API: GpuContext, EdlV1, render_proxy()
  error.rs          // PipelineError (thiserror, wie apx_core::AppError)
  edl/
    mod.rs          // öffentliche Re-Exports, EDL_SCHEMA_VERSION
    v1.rs           // EdlV1-Struct (7 Regler-Felder, typisiert)
    migrate.rs       // Upgrade-Kette für künftige Schema-Versionen
  color/            // ProPhoto-Matrizen, lcms2-Anbindung (Anzeige-Transform)
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
  tile_cache.rs
```

**Abhängigkeitsrichtung:** `apx-pipeline` → `apx-core` + `apx-raw`. Nicht `apx-catalog` (siehe Diagramm/Regel oben). `apx-app` hängt zusätzlich von `apx-pipeline` ab und verdrahtet es wie die anderen drei Crates, ohne eigene Bildverarbeitungslogik.

**EDL-Speicherung:** `apx-core` bekommt einen minimalen, versionsmarkierten Umschlagtyp `EdlEnvelope { schema_version: u32, payload: serde_json::Value }`. `apx-catalog` speichert nur diesen Umschlag (als `TEXT`/JSON in einer neuen `edit_history`-Tabelle, siehe `migrations/0002_edits.sql`) und muss den `payload`-Inhalt nie verstehen. Nur `apx-pipeline` kennt die konkrete `EdlV1`-Struct und entpackt `payload` in sie. Das hält die Abhängigkeitsrichtung sauber (kein `apx-catalog` → `apx-pipeline`).

**`apx-raw`-Grenze:** Ein neuer, additiver Einstiegspunkt `apx_raw::decode_linear()` deckt die Schritte RAW-Dekodierung/Demosaicing/Normalisierung ab und hört **vor** dem bisher fest einprogrammierten Weißabgleich/Gamma auf (die bestehende `decode()`/`DecodedImage`-API für Phase-1-Aufrufer wie Vorschaubilder bleibt unverändert). `apx-pipeline` übernimmt ab dort: Weißabgleich, Belichtung/Ton, Ausgabe-Transform.

**Interaktiver Rendering-Pfad:** Ein Regler-Wechsel im Frontend löst (gebündelt/entprellt) eine Anfrage über eine neue `apx://develop/<id>/<edl_hash>/<max_edge>`-Route aus (Erweiterung des bestehenden Protokoll-Handler-Musters), die `apx-pipeline`s fusionierten Shader auf den von `apx-raw::decode_linear()` gelieferten linearen Puffer anwendet und rohe RGBA8-Bytes zurückgibt (nicht PNG — Begründung in ADR-0016). Das Frontend lädt diese Bytes direkt als WebGL2-Textur, statt sie zu dekodieren.

## 6. Platzhalter für spätere Phasen

Diese Abschnitte werden erst gefüllt, wenn die jeweilige Phase beginnt — hier nur benannt, damit die Zielarchitektur nicht aus dem Blick gerät:

- **Phase 3–4:** Erweiterung von `apx-catalog` um Sammlungen/Keywords/FTS5, volles Entwickeln-Modul als weitere Pipeline-Stufen.
- **Phase 5:** `apx-presets` — Preset-/Template-Engine, Adobe-Interop.
- **Phase 6:** Maskensystem als eigene Pipeline-Stufe(n).
- **Phase 7:** `apx-ai` — ONNX-Runtime-Integration, LLM-Client für Preset-Generator.
- **Phase 8–9:** Export-Engine, Ausgabe-Module, Node-Editor, Stacking, Tethering, Skript-API/Plugins.
