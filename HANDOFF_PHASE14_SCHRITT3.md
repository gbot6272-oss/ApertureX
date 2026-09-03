# Unterbrechungs-Notiz: Phase 14 Schritt 3 (Mehrfachbelichtung & Layer-Blend-Modi)

Diese Datei ist ein **temporärer Zwischenstand-Vermerk**, kein Teil der
regulären Projektdokumentation (PLAN.md/DECISIONS.md) — auf ausdrücklichen
Wunsch des Nutzers angelegt, um festzuhalten, wo die Arbeit an Phase 14
Schritt 3 unterbrochen wurde, bevor es an anderer Stelle weitergeht.

## Update: zwischenzeitlich committet und gepusht

Der unten beschriebene Stand (Rust-Backend + EDL-Typen + Store-Aktionen,
**noch ohne Bedienoberfläche**) wurde inzwischen fertig implementiert (die
zuvor nur deklarierten Store-Aktionen haben jetzt echte Implementierungen,
nach demselben Muster wie `runAiOutpaint`/`clearCanvasExtension`),
durchläuft `cargo check`/`test`/`fmt`/`clippy` und `tsc -b` sauber, wurde
committet und nach `claude/phase-8-schritt-4-diashow-r3akzl` gepusht (ein
automatischer Git-Hook verlangte einen sauberen Arbeitsbaum). Die Liste
unter „Fehlt noch" ist weiterhin gültig — insbesondere Punkt 2
(`CompositeLayersPanel.tsx`/UI) ist NICHT Teil dieses Commits.

## Stand bei Unterbrechung (historisch, vor dem obigen Commit)

Mitten in der Store-Anbindung (`frontend/src/store/index.ts`) für Schritt 3.
Die zuletzt angewendete Änderung: die Interface-Deklarationen für die neuen
Store-Aktionen wurden gerade eingefügt (`addCompositeLayerFromPhoto`,
`addCompositeLayerFromTexture`, `compositeLayerLoading`,
`removeCompositeLayer`, `setCompositeLayerField`) — direkt nach
`clearCanvasExtension: () => void;`. **Die tatsächlichen Implementierungen
dieser Aktionen (der `set`/`get`-Rumpf im Store-Objekt weiter unten) fehlen
noch.**

## Bereits fertig und committet (Schritt 1 + 2 vollständig, gepusht)

- Phase 14 Schritt 1 (KI-Ausfüllen über Bildränder/Outpainting) — Commit
  `a80c547`, gepusht.
- Phase 14 Schritt 2 (Frequenztrennung für Präzisions-Retusche) — Commit
  `bab9ab3`, gepusht.

## Bereits fertig, aber NOCH NICHT committet (Schritt 3, in Arbeit)

Alles unten läuft sauber durch `cargo check`/`cargo test`/`cargo fmt`/
`cargo clippy` für die Rust-Seite; die TypeScript-Seite ist bis zu dem
Punkt konsistent, an dem unterbrochen wurde (aber noch nicht vollständig,
siehe „Fehlt noch").

### Rust (fertig, getestet, formatiert, clippy-sauber)

- `crates/apx-pipeline/src/edl/v4.rs`:
  - `StageEnabled` bekommt ein neues Feld `composite: bool` mit
    `#[serde(default = "default_true")]` (neue Hilfsfunktion
    `default_true()`) — Begründung im Code-Kommentar: ein altes
    gespeichertes `StageEnabled`-Objekt ohne dieses Feld muss als „diese
    Stufe war aktiv" gelesen werden, nicht als deaktiviert.
  - Neue Typen `CompositeLayerSource` (bitmap_width/height + `pixels:
    Vec<u8>`, interleaved RGB) und `CompositeLayer` (visible, blend_mode,
    opacity, scale, offset_x, offset_y, source).
  - `EdlV4` bekommt `composite_layers: Vec<CompositeLayer>` additiv
    (`#[serde(default)]`), `neutral()`/`from_v3()` aktualisiert.
- `crates/apx-pipeline/src/edl/mod.rs`: `CompositeLayer`/
  `CompositeLayerSource` re-exportiert.
- `crates/apx-pipeline/src/stages/masks.rs`: `blend_pixel` von privat auf
  `pub(crate)` geändert (Wiederverwendung durch `stages::composite`).
- **Neu:** `crates/apx-pipeline/src/stages/composite.rs` — `apply_all()`
  legt Ebenen sequenziell über ein RGBA8-Bild (Skalierung per
  `apx_core::raster::bilinear_resize_u8`, Blend über `masks::blend_pixel`,
  Platzierung per normiertem Mittelpunkt `offset_x`/`offset_y` + `scale`).
  5 Tests, alle grün.
- `crates/apx-pipeline/src/stages/mod.rs`: `pub mod composite;` registriert.
- `crates/apx-pipeline/src/develop.rs`: neue Stufe `composited` zwischen
  `curved` und `geometry` eingefügt (nach `curves`, vor `geometry` — im
  fertig entwickelten sRGB-RGBA8-Bild, nicht im linearen Arbeitsraum),
  gated durch `stages.composite`; Import-Liste + Test-Fixture-Literale
  (`EdlV4 { ... }` ohne `..neutral()`-Spread) um `composite_layers:
  Vec::new()` ergänzt. Neuer Integrationstest
  `a_composite_layer_reaches_the_final_render_and_can_be_disabled` — grün.
- `crates/apx-app/src/commands.rs`: neuer Command
  `prepare_composite_layer_source(photo_id: Option<String>, texture_path:
  Option<String>)` — löst **entweder** ein weiteres Katalog-Foto (per
  `decode_linear` + `apx_pipeline::color::linear_camera_rgb_to_srgb_rgba8`)
  **oder** eine vom Nutzer gewählte Textur-Datei (per `image::open` +
  eigene `downsample_rgb_image`-Hilfsfunktion, Cap bei
  `apx_ai::segmentation::ANALYSIS_MAX_EDGE`) zu einer fertigen
  `CompositeLayerSourceDto` (base64 RGB) auf. Kein neues KI-Modell nötig.
- `crates/apx-app/src/main.rs`: Command registriert.

Alle Rust-Änderungen sind uncommitted im Arbeitsverzeichnis (`git status`
zeigt sie als `M`/neue Dateien).

### TypeScript (teilweise fertig)

- `frontend/src/lib/edl.ts`: `StageEnabled.composite` (+
  `NEUTRAL_STAGE_ENABLED`), `CompositeLayerSource`/`CompositeLayer`-
  Interfaces, `STAGE_NODE_SPECS`-Eintrag „Compositing", `EdlPayload.
  composite_layers` + `neutralEdlPayload()` aktualisiert.
- `frontend/src/components/DevelopPanel.tsx`: `STAGE_ANCHOR_IDS.composite
  = "stage-composite"` ergänzt (der eigentliche `<fieldset id="stage-
  composite">`-Abschnitt mit den Reglern existiert noch NICHT).
- `frontend/src/lib/presets.ts`: `composite_layers` bewusst in
  `PRESET_SECTION_KEYS`/`PRESET_SECTION_LABELS` aufgenommen (anders als
  `repair`/`masks`, siehe Code-Kommentar dort: eine Ebenen-„Rezeptur" mit
  eigener Bitmap ist ein portabler „Look", keine bildpositions-gebundene
  Angabe) — `tsc -b` und die bestehenden Vitest-Suiten (`edl.test.ts`,
  `presets.test.ts`) liefen zu diesem Zeitpunkt sauber grün.
- `frontend/src/lib/tauri.ts`: `CompositeLayerSourceDto` +
  `prepareCompositeLayerSource(photoId, texturePath)`-Binding ergänzt.
- `frontend/src/store/index.ts`: Import von `CompositeLayer`/`BlendMode`
  ergänzt; **Interface-Deklarationen** der neuen Aktionen eingefügt
  (`addCompositeLayerFromPhoto`, `addCompositeLayerFromTexture`,
  `compositeLayerLoading`, `removeCompositeLayer`,
  `setCompositeLayerField`) — **die Implementierungen fehlen noch**, `tsc
  -b` wird an dieser Stelle mit hoher Wahrscheinlichkeit fehlschlagen
  (Interface verspricht Felder, die das Store-Objekt noch nicht erfüllt).

## Fehlt noch für Schritt 3 (nicht begonnen)

1. Store-**Implementierungen** der oben deklarierten Aktionen (analog zu
   `runAiOutpaint`/`clearCanvasExtension` als Vorbild).
2. Neue `frontend/src/components/CompositeLayersPanel.tsx` (oder ein
   `<fieldset id="stage-composite">`-Abschnitt direkt in
   `DevelopPanel.tsx`) mit: „Foto aus Katalog wählen"/„Textur-Datei
   wählen" (via `pickFilePath`), Ebenenliste mit
   sichtbar/Blend-Modus/Deckkraft/Skalierung/Position-Reglern, Entfernen-
   Knopf.
3. `tsc -b` + Vitest erneut grün bekommen (aktuell rot wegen Punkt 1).
4. `cargo fmt`/`cargo clippy` fürs Gesamt-Rust-Ergebnis erneut laufen
   lassen (nach evtl. weiteren Änderungen).
5. **PLAN.md**: Checkbox „- [ ] 3. Mehrfachbelichtung & Layer-Blend-Modi"
   auf `[x]` + Prosa-Absatz nach dem etablierten Muster der Schritte 1/2.
6. **DECISIONS.md**: neuer „ADR-0041-Nachtrag III"-Absatz (Compositing-
   Pipeline-Position nach `curves`/vor `geometry`, Wiederverwendung von
   `blend_pixel`, „kein Katalogzugriff in `apx-pipeline`"-Architekturgrund,
   `composite_layers` bewusst in Presets/Sync eingeschlossen).
7. Commit + Push nach `claude/phase-8-schritt-4-diashow-r3akzl` (mit dem
   vom Projekt vorgeschriebenen Attribution-Footer).
8. Task #62 im Tracker auf `completed` setzen, dann Task #63 (Schritt 4:
   Halation/Bloom) beginnen.

## Wiedereinstieg

Beim Fortsetzen: zuerst `git status`/`git diff` prüfen, um den exakten
Stand zu bestätigen (diese Notiz beschreibt ihn, ersetzt aber keine
eigene Prüfung), dann bei „Store-Implementierungen ergänzen" (Punkt 1
oben) weitermachen.
