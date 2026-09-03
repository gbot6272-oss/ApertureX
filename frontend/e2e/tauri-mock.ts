import type { Page } from "@playwright/test";

/**
 * Simulierte `window.__TAURI_INTERNALS__`-Brücke für die Playwright-Tests.
 *
 * Siehe `DECISIONS.md` ADR-0010: Diese Tests laufen gegen den
 * Produktions-Build im normalen Chromium, nicht gegen die kompilierte
 * native App (das bräuchte `tauri-driver` + WebdriverIO). Damit das
 * Frontend trotzdem lauffähig ist, wird hier exakt der Teil der
 * `@tauri-apps/api`-Laufzeit nachgebaut, den `src/lib/tauri.ts`,
 * `src/lib/media.ts` und `src/hooks/useImportEvents.ts` tatsächlich
 * benutzen:
 *
 * - `invoke(cmd, args)` — ruft für die App-eigenen Commands
 *   (`select_folder`, `catalog_status`, `list_folders`,
 *   `list_photos_in_folder`, `import_folder`, `cancel_import`) fest
 *   hinterlegte, zur Laufzeit änderbare Fixture-Daten ab.
 * - `invoke('plugin:event|listen'/'plugin:event|unlisten', ...)` — das ist
 *   es, was `@tauri-apps/api/event`s `listen()` unter der Haube aufruft
 *   (siehe `node_modules/@tauri-apps/api/event.js`); hier nachgebaut,
 *   damit `useImportEvents` echte Listener registriert.
 * - `transformCallback(fn, once)` — registriert `fn` unter einer
 *   numerischen ID, exakt wie `core.js` es von den echten Tauri-Internals
 *   erwartet.
 * - `convertFileSrc(path, protocol)` — liefert statt einer echten
 *   `apx://…`-URL (die im Browser nicht aufgelöst werden kann) eine
 *   normale Same-Origin-URL, die der Test über `page.route()` mit einem
 *   winzigen, echten PNG beantwortet — so durchläuft `useImageBitmap`
 *   (fetch → blob → `createImageBitmap`) den echten Code, nicht nur eine
 *   Attrappe.
 *
 * Alles läuft als `page.addInitScript`, also im Browser-Kontext — die
 * Funktion darf deshalb nichts aus dem Node-Scope schließen.
 */
function installBridge(initialFixtures: Record<string, unknown>): void {
  interface CallbackEntry {
    fn: (arg: unknown) => void;
    once: boolean;
  }

  const w = window as unknown as Record<string, unknown>;

  w.__mockFixtures = {
    selectFolderResult: null as string | null,
    catalogStatus: { catalog_path: "mock-catalog.sqlite3", folder_count: 0, photo_count: 0 },
    folders: [] as unknown[],
    photosByFolder: {} as Record<string, unknown[]>,
    // `.apx`-Import/-Export (Phase 5 Schritt 10) — die echten Commands
    // öffnen einen nativen Datei-Dialog im Backend; hier stattdessen fest
    // hinterlegte Ergebnisse, per Fixture steuerbar (siehe
    // `exportPresetToApxFile`/`importPresetFromApxFile`-Tests).
    exportApxPathResult: "/mock/export.apx" as string | null,
    // Adobe .lrtemplate-Export (Phase 11 Schritt 8).
    exportLrtemplatePathResult: "/mock/export.lrtemplate" as string | null,
    importApxFile: null as { name: string; tags: string[]; conditions_json: string; edl_subset_json: string } | null,
    // Kollaborationsmodus (Phase 9 Schritt 10) — dieselbe Fixture-Strategie
    // wie beim `.apx`-Dialog oben: der eigentliche `content_hash`-Abgleich
    // (`Catalog::find_photo_by_content_hash`/`diff_share_edit`) ist bereits
    // in `apx-catalog`s Rust-Unit-Tests abgedeckt — hier steuert die
    // Fixture direkt das Ergebnis, das `import_catalog_share` zurückgäbe.
    exportSharePathResult: "/mock/export.apxs" as string | null,
    importShareResult: null as {
      name: string;
      unmatched: Array<{ filename: string; content_hash: string }>;
      unchanged: string[];
      conflicts: Array<{
        photo_id: string;
        filename: string;
        incoming_edl_json: string;
        prefer_incoming: boolean;
        local_edited_at: string;
        incoming_edited_at: string;
      }>;
    } | null,
    // Tethered Shooting (Phase 9 Schritt 11) — `tetherCameraResult`
    // simuliert `tether_connect`s Ergebnis (Standard: eine "verbundene"
    // simulierte Kamera, wie ein Build ohne `tethering`-Feature sie
    // liefert), `tetherCapturePhoto` das neu "aufgenommene" Foto, das
    // `tether_capture` zurückgäbe (per Fixture überschreibbar).
    tetherCameraResult: { model: "Simulierte Kamera", port: "usb:mock,000", simulated: true } as {
      model: string;
      port: string;
      simulated: boolean;
    } | null,
    tetherCapturePhoto: null as Record<string, unknown> | null,
    // Direktimport von Speicherkarte/Kamera (Phase 13 Schritt 2) — leere
    // Listen als Testdefault (kein simuliertes Gerät angeschlossen), per
    // Fixture überschreibbar.
    removableVolumes: [] as Array<{ mount_point: string; name: string; has_dcim: boolean }>,
    cameraFiles: [] as Array<{ folder: string; name: string }>,
    cameraImportPhoto: null as Record<string, unknown> | null,
    // KI-Funktionen (Phase 7, siehe DECISIONS.md ADR-0033) — feste,
    // per Fixture überschreibbare Antworten statt einer echten
    // Bildanalyse (die läuft nur im echten Backend).
    aiMaskAlpha: {
      width: 4,
      height: 4,
      // 16 Bytes, alle 200 — Base64 von `Uint8Array(16).fill(200)`.
      alpha_base64: "yMjIyMjIyMjIyMjIyMjIyA==",
    },
    repairSourceSuggestion: { x: 0.2, y: 0.2 },
    sensorSpots: [{ x: 0.4, y: 0.4, radius: 0.03, strength: 0.7 }] as Array<{ x: number; y: number; radius: number; strength: number }>,
    anthropicApiKey: null as string | null,
    // Phase 13 Schritt 1 (ADR-0040): `null` = kein Modell heruntergeladen
    // (Testdefault) — ein Test, der den heruntergeladenen Zustand prüfen
    // will, überschreibt das per `setMockFixtures`.
    inpaintingModelPath: null as string | null,
    // Phase 10 Schritt 1/9: `onboarding_seen: true` als Testdefault, damit
    // das automatische Onboarding-Overlay (App.tsx) nicht in jedem
    // e2e-Test ungefragt aufpoppt — ein Test, der das Onboarding gezielt
    // prüfen will, überschreibt das per `setMockFixtures`.
    uiSettings: {
      theme: "dark" as "dark" | "light",
      accent_color: null as string | null,
      locale: "de",
      ui_scale_percent: 100,
      high_contrast: false,
      reduced_motion: false,
      onboarding_seen: true,
    },
    // Beobachteter Ordner (Phase 12 Schritt 7) — derselbe Aus-Default wie
    // im echten Backend (`apx_core::settings::WatchedFolderSettings`).
    watchedFolderSettings: {
      path: null as string | null,
      enabled: false,
      poll_seconds: 30,
    },
    presetGeneratorSubsetJson: JSON.stringify({ basic: { exposure_ev: 0.6, contrast: 15 } }),
    referenceImageDialogCancelled: false,
    presetVariationCount: 3,
    tagSuggestions: ["Himmel", "Landschaft"] as string[],
    // Export-Engine (Phase 8 Schritt 1, ADR-0034) — echtes Rendern/Kodieren
    // läuft nur im Backend; hier ein fest hinterlegtes Ergebnis pro Aufruf.
    exportPhotoOutcome: { path: "/mock/export/output.jpg", width: 100, height: 80, byte_size: 12345 } as {
      path: string;
      width: number;
      height: number;
      byte_size: number;
    },
    exportPhotoShouldFail: false,
    // `pick_file_path` (ICC-/Schriftdatei/-Wasserzeichenbild-Auswahl) —
    // `null` simuliert einen abgebrochenen Dialog.
    pickFilePathResult: null as string | null,
    // Drucken (Phase 8 Schritt 3) — Speichern-unter-Dialog + Ergebnis.
    pickSaveFilePathResult: null as string | null,
    printOutcome: { path: "/mock/print/Druckseite.jpg", width: 2550, height: 3300, byte_size: 654321 } as {
      path: string;
      width: number;
      height: number;
      byte_size: number;
    },
    printPhotoShouldFail: false,
    // Diashow (Phase 8 Schritt 4) — Video-Export über ein simuliertes
    // System-`ffmpeg`. `ffmpegAvailable` steuert, ob der Export-Knopf im
    // Dialog überhaupt aktivierbar ist.
    ffmpegAvailable: true,
    slideshowVideoOutcome: { path: "/mock/slideshow/Diashow.mp4", frame_count: 250, duration_seconds: 10 } as {
      path: string;
      frame_count: number;
      duration_seconds: number;
    },
    slideshowVideoShouldFail: false,
    // Buch (Phase 8 Schritt 5) — PDF-Export.
    bookOutcome: { path: "/mock/book/Fotobuch.pdf", page_count: 2, byte_size: 123456 } as {
      path: string;
      page_count: number;
      byte_size: number;
    },
    bookExportShouldFail: false,
    // Web (Phase 8 Schritt 6) — HTML-Galerie-Export.
    webGalleryOutcome: { dest_dir: "/mock/web/galerie", photo_count: 1, uploaded_count: null } as {
      dest_dir: string;
      photo_count: number;
      uploaded_count: number | null;
    },
    webExportShouldFail: false,
    // Karte (Phase 8 Schritt 7).
    geocodedLocation: { name: "Berlin", admin1: "Berlin", country_code: "DE", distance_km: 1.2 } as {
      name: string;
      admin1: string;
      country_code: string;
      distance_km: number;
    },
    gpxTrackPoints: [] as Array<{ lat: number; lon: number; elevation: number | null; time: string | null }>,
    // Vorlagen (Phase 8 Schritt 8).
    exportTemplateFilePathResult: "/mock/templates/Vorlage.apxt" as string | null,
    importedTemplateFile: null as { kind: string; name: string; payload_json: string } | null,
    // Perceptual-Hash-Duplikaterkennung (Phase 9 Schritt 1).
    perceptualDuplicateGroups: [] as unknown[][],
    // Personenansicht (Phase 11 Schritt 5).
    peopleGroups: [] as unknown[][],
    // Adobe-XMP-Sidecar (Phase 9 Schritt 2).
    exportedXmpSidecarPath: "/mock/photos/IMG_0001.xmp" as string,
    xmpImportApplies: true as boolean,
    // Statistik/Vorschau-Cache (Phase 9 Schritt 3).
    catalogStatistics: {
      total_photos: 0,
      total_file_size: 0,
      earliest_captured_at: null,
      latest_captured_at: null,
      rating_distribution: [],
      top_camera_models: [],
      top_lenses: [],
    } as unknown,
    previewCacheStats: { file_count: 0, total_bytes: 0 } as { file_count: number; total_bytes: number },
    // Entrauschung/Hochskalierung (Phase 9 Schritt 6).
    denoisedPhotoPath: "/mock/photos/IMG_0001_entrauscht.png" as string,
    upscaledPhotoPath: "/mock/photos/IMG_0001_hochskaliert.png" as string,
    // DNG-Konvertierung (Phase 11 Schritt 1).
    convertedDngPath: "/mock/photos/IMG_0001.dng" as string,
    ...initialFixtures,
  };
  w.__mockInvokeLog = [] as Array<{ cmd: string; args: unknown }>;

  let nextCallbackId = 1;
  let nextListenerId = 1;
  const callbacks: Record<number, CallbackEntry> = {};
  const listenersByEvent: Record<string, number[]> = {};

  // Bibliothek (ab Phase 3, siehe `crates/apx-app/src/commands.rs`s
  // gleichnamige Commands): einfache In-Memory-Nachbildung, kein SQL —
  // reicht aus, um die Frontend-Logik (Store, Komponenten) ohne echtes
  // Backend zu prüfen.
  interface MockPhoto {
    id: string;
    filename: string;
    camera_model?: string | null;
    rating: number;
    flag: number;
    color_label: string | null;
    [key: string]: unknown;
  }
  interface MockCollection {
    id: string;
    name: string;
    folder_id: string | null;
    is_smart: boolean;
    smart_criteria_json: string | null;
  }
  const collections: MockCollection[] = [];
  const collectionPhotoIds: Record<string, string[]> = {};

  // Bibliotheks-Backlog (Phase 9 Schritt 1) — einfache In-Memory-
  // Nachbildung von Sammlungssätzen/Stapeln/virtuellen Kopien/
  // Farbmarkierungs-Definitionen.
  interface MockCollectionFolder {
    id: string;
    name: string;
    parent_id: string | null;
    position: number;
  }
  const collectionFolders: MockCollectionFolder[] = [];
  let nextCollectionFolderId = 1;

  interface MockStack {
    id: string;
    name: string | null;
    cover_photo_id: string | null;
    photo_ids: string[];
  }
  const stacks: MockStack[] = [];
  let nextStackId = 1;

  const virtualCopiesBySource: Record<string, string[]> = {};
  let nextVirtualCopyId = 1;
  let nextStackResultId = 1;

  interface MockColorLabelDefinition {
    name: string;
    display_name: string;
    hex: string;
    position: number;
  }
  const colorLabelDefinitions: MockColorLabelDefinition[] = [
    { name: "red", display_name: "Rot", hex: "#e53e3e", position: 0 },
    { name: "yellow", display_name: "Gelb", hex: "#d69e2e", position: 1 },
    { name: "green", display_name: "Grün", hex: "#38a169", position: 2 },
    { name: "blue", display_name: "Blau", hex: "#3182ce", position: 3 },
    { name: "purple", display_name: "Lila", hex: "#805ad5", position: 4 },
  ];
  const photoKeywords: Record<string, { id: string; name: string }[]> = {};
  let nextCollectionId = 1;
  let nextKeywordId = 1;

  // Stapelverarbeitungs-Konsole (Phase 11 Schritt 9, siehe DECISIONS.md
  // ADR-0038) — ein Journal fürs Undo, dasselbe Prinzip wie
  // `batch_operation_items` auf der echten Seite: pro angewandtem Stapel
  // wird für jede tatsächlich geänderte Eigenschaft der alte Wert notiert.
  interface MockBatchItem {
    photoId: string;
    field: "rating" | "color_label" | "keyword";
    oldValue: number | string | null;
    newValue: number | string | null;
  }
  const batchOperations: Record<string, MockBatchItem[]> = {};
  let nextBatchId = 1;

  // Schlagworthierarchie/Tag-Regeln/Metadaten (Phase 9 Schritt 2, siehe
  // DECISIONS.md ADR-0035) — `keywordMeta` hält parent_id/synonyms separat
  // von `photoKeywords`, weil dasselbe Schlagwort an mehreren Fotos hängen
  // kann und die Hierarchie unabhängig von einzelnen Foto-Verknüpfungen ist.
  const keywordMeta: Record<string, { parent_id: string | null; synonyms: string[] }> = {};
  function keywordMetaFor(id: string) {
    return keywordMeta[id] ?? { parent_id: null, synonyms: [] };
  }
  interface MockTagRule {
    id: string;
    name: string;
    keyword_id: string;
    conditions_json: string;
    enabled: boolean;
  }
  const tagRules: MockTagRule[] = [];
  let nextTagRuleId = 1;

  // Presets (ab Phase 5, siehe `DECISIONS.md` ADR-0031) — wie bei
  // Sammlungen: einfache In-Memory-Nachbildung von `apx-catalog`s
  // preset_folders/presets/preset_versions.
  interface MockPresetFolder {
    id: string;
    name: string;
    parent_id: string | null;
    position: number;
  }
  interface MockPreset {
    id: string;
    folder_id: string | null;
    name: string;
    is_favorite: boolean;
    tags: string[];
    conditions_json: string;
    created_at: string;
  }
  interface MockPresetVersion {
    id: string;
    preset_id: string;
    sequence: number;
    edl_subset_json: string;
    created_at: string;
  }
  interface MockImportPreset {
    name: string;
    mode: { mode: "AddInPlace" } | { mode: "Copy"; target_dir: string } | { mode: "Move"; target_dir: string };
    rename_pattern: string | null;
  }
  // Seedbar über `installTauriMock`s Fixtures (siehe `folders`/
  // `photosByFolder` oben) — Presets starten in einem echten Katalog zwar
  // immer leer, aber Tests für Umbenennen/Favorisieren/Löschen brauchen
  // ein bereits vorhandenes Preset, ohne den (erst in Schritt 4 gebauten)
  // Erstellen-Dialog durchklicken zu müssen.
  const presetFolders: MockPresetFolder[] = [...((initialFixtures.presetFolders as MockPresetFolder[] | undefined) ?? [])];
  const presets: MockPreset[] = [...((initialFixtures.presets as MockPreset[] | undefined) ?? [])];
  const presetVersions: Record<string, MockPresetVersion[]> = {
    ...((initialFixtures.presetVersions as Record<string, MockPresetVersion[]> | undefined) ?? {}),
  };
  let nextPresetFolderId = 1;
  let nextPresetId = 1;
  let nextPresetVersionId = 1;

  // Import-Presets (Phase 3 Backend, Phase 5 Schritt 9 Frontend) — analog
  // zu `presetFolders`/`presets` oben über Fixtures seedbar.
  const importPresets: MockImportPreset[] = [...((initialFixtures.importPresets as MockImportPreset[] | undefined) ?? [])];

  // Export-Warteschlange (Phase 8 Schritt 2) — vereinfachte In-Memory-
  // Simulation: ein eingereihter Auftrag wird sofort als abgeschlossen
  // markiert, außer die Warteschlange ist pausiert (dann bleibt er
  // "pending", bis `resume_export_queue` ihn nachträglich abschließt) —
  // genug, um Fortschritt/Pausieren/Fehler in e2e-Tests zu beobachten,
  // ohne einen echten Hintergrund-Worker nachzubauen.
  let nextExportJobId = 1;

  // Vorlagen (Phase 8 Schritt 8) — einfache In-Memory-Nachbildung von
  // `apx-catalog`s generischer `templates`-Tabelle.
  interface MockTemplate {
    id: string;
    kind: string;
    name: string;
    payload_json: string;
    created_at: string;
  }
  const templates: MockTemplate[] = [];
  let nextTemplateId = 1;
  let exportQueuePaused = Boolean(initialFixtures.exportQueueStartsPaused);
  const exportQueueJobs: Array<{ id: number; status: "pending" | "done" | "failed" }> = [];

  function findPreset(presetId: string): MockPreset | undefined {
    return presets.find((p) => p.id === presetId);
  }

  function allPhotos(): MockPhoto[] {
    const fixtures = w.__mockFixtures as { photosByFolder: Record<string, MockPhoto[]> };
    return Object.values(fixtures.photosByFolder).flat();
  }

  function findPhoto(photoId: string): MockPhoto | undefined {
    return allPhotos().find((p) => p.id === photoId);
  }

  /** Mock-Gegenstück zu `apx-app::commands::import_stack_result_photo`
   * (Phase 9 Schritt 8) — legt ein synthetisches Ergebnisfoto im selben
   * Ordner wie das erste Quellfoto an und verknüpft es per Stapel mit
   * allen Quellfotos. Reale Breite/Höhe/Datei-Schreiben/Hash entfallen
   * im Browser-Mock (siehe `installTauriMock`s Moduldoku für die
   * grundsätzliche Grenze simulierter Tauri-IPC). */
  function importStackResultPhoto(photoIds: string[], suffix: string): { photo_id: string; stack_id: string; width: number; height: number } {
    const first = findPhoto(photoIds[0]!);
    if (!first) throw new Error(`Test-Stub: Foto '${photoIds[0]}' nicht gefunden`);
    const width = (first.width as number | undefined) ?? 100;
    const height = (first.height as number | undefined) ?? 100;
    const resultId = `${first.id}-${suffix}${nextStackResultId++}`;
    const result: MockPhoto = { ...first, id: resultId, filename: `${first.filename}_${suffix}.png`, width, height };
    const fixtures = w.__mockFixtures as { photosByFolder: Record<string, MockPhoto[]> };
    for (const folderId of Object.keys(fixtures.photosByFolder)) {
      if (fixtures.photosByFolder[folderId].some((p) => p.id === first.id)) {
        fixtures.photosByFolder[folderId] = [...fixtures.photosByFolder[folderId], result];
        break;
      }
    }
    const stackId = `stack-${nextStackId++}`;
    stacks.push({ id: stackId, name: suffix, cover_photo_id: resultId, photo_ids: [resultId, ...photoIds] });
    return { photo_id: resultId, stack_id: stackId, width, height };
  }

  /** Gemeinsame Kriterien-Prüfung für `filter_photos`/`search_and_filter_photos`
   * (Schritt 8.4) — spiegelt `crates/apx-catalog/src/repository/search.rs`s
   * `build_filter_clause`. */
  function matchesFilterCriteria(
    photo: MockPhoto,
    criteria: { rating_at_least?: number; flag?: number; color_label?: string; camera_model?: string },
  ): boolean {
    if (criteria.rating_at_least !== undefined && photo.rating < criteria.rating_at_least) return false;
    if (criteria.flag !== undefined && photo.flag !== criteria.flag) return false;
    if (criteria.color_label !== undefined && photo.color_label !== criteria.color_label) return false;
    if (criteria.camera_model !== undefined && photo.camera_model !== criteria.camera_model) return false;
    return true;
  }

  /** Flache Kopie statt der intern weiterverwendeten Foto-Referenz —
   * genau wie bei `list_photo_keywords`/`list_collections` oben: echtes
   * Tauri-IPC serialisiert bei jedem Aufruf frisch, hier ist es sonst
   * dieselbe Objektreferenz, die Zustand/Immer beim Einlagern in den
   * Store einfriert. Ohne diese Kopie würde `set_photo_rating`/
   * `set_photo_flag`/`set_photo_color_label`s direkte Eigenschafts-
   * zuweisung auf dem eingefrorenen Original in der (nicht strikten)
   * `addInitScript`-Umgebung *lautlos* wirkungslos bleiben (kein Fehler,
   * aber auch keine Änderung) — ein reines Mock-Artefakt, das reale
   * Tauri-IPC nicht hat.
   */
  function clonePhoto(photo: MockPhoto): MockPhoto {
    return { ...photo };
  }

  // Simuliertes `edit_history`/`edit_current` fürs Entwickeln-Panel (ab
  // Phase 2, siehe `crates/apx-catalog`s `repository::edits` und
  // `crates/apx-app/src/commands.rs`) — dieselbe lineare
  // Verlaufs-Semantik (neue Bearbeitung nach Rückgängig verwirft die
  // "Zukunft"), nur im Browser statt in SQLite.
  interface EditHistoryState {
    entries: Array<{ edl_json: string; created_at: string; label: string | null }>;
    currentIndex: number; // -1 = neutral (noch nie bearbeitet / bis zum Anfang zurück)
  }
  const editHistories: Record<string, EditHistoryState> = {};
  let historyCounter = Date.now();

  function historyPositionAt(history: EditHistoryState): unknown {
    if (history.currentIndex < 0) return { kind: "Neutral" };
    const entry = history.entries[history.currentIndex];
    return entry ? { kind: "At", edl_json: entry.edl_json } : { kind: "Neutral" };
  }

  // Simulierte `snapshots`-Tabelle (Phase 6 Schritt 8, siehe
  // `crates/apx-catalog/src/repository/snapshots.rs`) — anders als
  // `editHistories` oben unabhängig vom linearen Verlauf, nie
  // automatisch beschnitten.
  interface MockSnapshot {
    id: string;
    name: string;
    edl_json: string;
    created_at: string;
  }
  const snapshotsByPhoto: Record<string, MockSnapshot[]> = {};
  let nextSnapshotId = 1;

  w.__mockEmit = (eventName: string, payload: unknown) => {
    const ids = listenersByEvent[eventName] ?? [];
    for (const id of [...ids]) {
      const entry = callbacks[id];
      if (!entry) continue;
      entry.fn({ event: eventName, id, payload });
      if (entry.once) delete callbacks[id];
    }
  };

  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener(eventName: string, listenerId: number) {
      // Der Listener-Rückgabewert von 'plugin:event|listen' ist hier die
      // Handler-ID selbst (siehe invoke() unten) — genau die entfernen
      // wir wieder, analog zur echten Tauri-Runtime.
      listenersByEvent[eventName] = (listenersByEvent[eventName] ?? []).filter((id) => id !== listenerId);
    },
  };

  async function invoke(cmd: string, args: Record<string, unknown> = {}): Promise<unknown> {
    (w.__mockInvokeLog as unknown[]).push({ cmd, args });
    const fixtures = w.__mockFixtures as {
      selectFolderResult: string | null;
      catalogStatus: unknown;
      folders: unknown[];
      photosByFolder: Record<string, unknown[]>;
      exportApxPathResult: string | null;
      exportLrtemplatePathResult: string | null;
      importApxFile: { name: string; tags: string[]; conditions_json: string; edl_subset_json: string } | null;
      exportSharePathResult: string | null;
      importShareResult: {
        name: string;
        unmatched: Array<{ filename: string; content_hash: string }>;
        unchanged: string[];
        conflicts: Array<{
          photo_id: string;
          filename: string;
          incoming_edl_json: string;
          prefer_incoming: boolean;
          local_edited_at: string;
          incoming_edited_at: string;
        }>;
      } | null;
      tetherCameraResult: { model: string; port: string; simulated: boolean } | null;
      tetherCapturePhoto: Record<string, unknown> | null;
      removableVolumes: Array<{ mount_point: string; name: string; has_dcim: boolean }>;
      cameraFiles: Array<{ folder: string; name: string }>;
      cameraImportPhoto: Record<string, unknown> | null;
      aiMaskAlpha: { width: number; height: number; alpha_base64: string };
      repairSourceSuggestion: { x: number; y: number };
      sensorSpots: Array<{ x: number; y: number; radius: number; strength: number }>;
      anthropicApiKey: string | null;
      inpaintingModelPath: string | null;
      uiSettings: {
        theme: "dark" | "light";
        accent_color: string | null;
        locale: string;
        ui_scale_percent: number;
        high_contrast: boolean;
        reduced_motion: boolean;
        onboarding_seen: boolean;
      };
      watchedFolderSettings: {
        path: string | null;
        enabled: boolean;
        poll_seconds: number;
      };
      presetGeneratorSubsetJson: string;
      referenceImageDialogCancelled: boolean;
      presetVariationCount: number;
      tagSuggestions: string[];
      exportPhotoOutcome: { path: string; width: number; height: number; byte_size: number };
      exportPhotoShouldFail: boolean;
      pickFilePathResult: string | null;
      pickSaveFilePathResult: string | null;
      printOutcome: { path: string; width: number; height: number; byte_size: number };
      printPhotoShouldFail: boolean;
      ffmpegAvailable: boolean;
      slideshowVideoOutcome: { path: string; frame_count: number; duration_seconds: number };
      slideshowVideoShouldFail: boolean;
      bookOutcome: { path: string; page_count: number; byte_size: number };
      bookExportShouldFail: boolean;
      webGalleryOutcome: { dest_dir: string; photo_count: number; uploaded_count: number | null };
      webExportShouldFail: boolean;
      geocodedLocation: { name: string; admin1: string; country_code: string; distance_km: number };
      gpxTrackPoints: Array<{ lat: number; lon: number; elevation: number | null; time: string | null }>;
      exportTemplateFilePathResult: string | null;
      importedTemplateFile: { kind: string; name: string; payload_json: string } | null;
      perceptualDuplicateGroups: unknown[][];
      peopleGroups: unknown[][];
    };

    switch (cmd) {
      case "plugin:event|listen": {
        const handlerId = args.handler as number;
        const eventName = args.event as string;
        listenersByEvent[eventName] = listenersByEvent[eventName] ?? [];
        listenersByEvent[eventName].push(handlerId);
        return nextListenerId++;
      }
      case "plugin:event|unlisten": {
        (w.__TAURI_EVENT_PLUGIN_INTERNALS__ as { unregisterListener: (e: string, id: number) => void }).unregisterListener(
          args.event as string,
          args.eventId as number,
        );
        return null;
      }
      case "select_folder":
        return fixtures.selectFolderResult;
      case "catalog_status":
        return fixtures.catalogStatus;
      case "list_folders":
        return fixtures.folders;
      case "list_photos_in_folder":
        return (fixtures.photosByFolder[args.folderId as string] ?? []).map((p) => clonePhoto(p as MockPhoto));
      case "import_folder":
        return null;
      case "import_folder_with_mode":
        return null;
      case "cancel_import":
        return null;
      case "list_import_presets":
        return importPresets.map((p) => ({ ...p }));
      case "save_import_preset": {
        const preset = args.preset as MockImportPreset;
        const index = importPresets.findIndex((p) => p.name === preset.name);
        if (index >= 0) importPresets[index] = preset;
        else importPresets.push(preset);
        return importPresets.map((p) => ({ ...p }));
      }
      case "delete_import_preset": {
        const name = args.name as string;
        const index = importPresets.findIndex((p) => p.name === name);
        if (index >= 0) importPresets.splice(index, 1);
        return importPresets.map((p) => ({ ...p }));
      }
      case "relink_folder": {
        // Nur ein No-Op-Stub — kein Playwright-Test übt Schritt 5s
        // Ordner-Relink-Fluss aus (reine Backend-/Vitest-Abdeckung dort),
        // aber der Command muss bekannt sein, damit ein versehentlicher
        // Aufruf nicht mit "unbekannter invoke-Befehl" abbricht.
        return null;
      }
      case "apply_develop_edit": {
        const photoId = args.photoId as string;
        const edlJson = args.edlJson as string;
        const label = (args.label as string | undefined) ?? null;
        const history = (editHistories[photoId] ??= { entries: [], currentIndex: -1 });
        // Verwirft eine zuvor per Rückgängig erreichte "Zukunft" — siehe
        // `DECISIONS.md` ADR-0014, exakt wie das echte Backend.
        history.entries = history.entries.slice(0, history.currentIndex + 1);
        // `historyCounter` statt `Date.now()` allein, damit die
        // Zeitstempel auch bei mehreren Aufrufen innerhalb derselben
        // Millisekunde (in e2e-Läufen üblich) streng monoton steigen —
        // die Zeitleiste (Phase 9 Schritt 7) sortiert/positioniert
        // danach.
        historyCounter += 1;
        history.entries.push({ edl_json: edlJson, created_at: new Date(historyCounter).toISOString(), label });
        history.currentIndex = history.entries.length - 1;
        return null;
      }
      case "current_develop_edit": {
        const history = editHistories[args.photoId as string];
        return history ? historyPositionAt(history) : { kind: "Neutral" };
      }
      case "undo_develop_edit": {
        const history = editHistories[args.photoId as string];
        if (!history || history.currentIndex < 0) return null;
        history.currentIndex -= 1;
        return historyPositionAt(history);
      }
      case "redo_develop_edit": {
        const history = editHistories[args.photoId as string];
        if (!history || history.currentIndex + 1 >= history.entries.length) return null;
        history.currentIndex += 1;
        return historyPositionAt(history);
      }
      case "list_develop_history": {
        const history = editHistories[args.photoId as string];
        if (!history) return [];
        return history.entries.map((entry, index) => ({
          sequence: index,
          label: entry.label,
          edl_json: entry.edl_json,
          created_at: entry.created_at,
        }));
      }
      case "goto_develop_edit": {
        const history = editHistories[args.photoId as string];
        const sequence = args.sequence as number;
        if (!history || sequence < 0 || sequence >= history.entries.length) return null;
        history.currentIndex = sequence;
        return historyPositionAt(history);
      }

      // ---- Schnappschüsse (Phase 6 Schritt 8) -------------------------------
      case "create_snapshot": {
        const photoId = args.photoId as string;
        const list = (snapshotsByPhoto[photoId] ??= []);
        list.push({
          id: `snapshot-${nextSnapshotId++}`,
          name: args.name as string,
          edl_json: args.edlJson as string,
          created_at: new Date().toISOString(),
        });
        return null;
      }
      case "list_snapshots": {
        return (snapshotsByPhoto[args.photoId as string] ?? []).map((s) => ({ ...s }));
      }
      case "rename_snapshot": {
        for (const list of Object.values(snapshotsByPhoto)) {
          const snapshot = list.find((s) => s.id === args.snapshotId);
          if (snapshot) snapshot.name = args.name as string;
        }
        return null;
      }
      case "delete_snapshot": {
        for (const [photoId, list] of Object.entries(snapshotsByPhoto)) {
          snapshotsByPhoto[photoId] = list.filter((s) => s.id !== args.snapshotId);
        }
        return null;
      }

      // ---- Bibliothek: Bewertung/Flagge/Farbe (ab Phase 3) -----------------
      case "set_photo_rating": {
        const photo = findPhoto(args.photoId as string);
        if (photo) photo.rating = args.rating as number;
        return null;
      }
      case "set_photo_flag": {
        const photo = findPhoto(args.photoId as string);
        if (photo) photo.flag = args.flag as number;
        return null;
      }
      case "set_photo_color_label": {
        const photo = findPhoto(args.photoId as string);
        if (photo) photo.color_label = args.colorLabel as string | null;
        return null;
      }

      // ---- Bibliothek: Schlagworte (ab Phase 3) ----------------------------
      case "add_photo_keyword": {
        const photoId = args.photoId as string;
        const name = args.name as string;
        const list = (photoKeywords[photoId] ??= []);
        const existing = list.find((k) => k.name === name);
        if (existing) return existing.id;
        const id = `kw-${nextKeywordId++}`;
        list.push({ id, name });
        return id;
      }
      case "remove_photo_keyword": {
        const photoId = args.photoId as string;
        const keywordId = args.keywordId as string;
        photoKeywords[photoId] = (photoKeywords[photoId] ?? []).filter((k) => k.id !== keywordId);
        return null;
      }
      case "list_photo_keywords":
        // Kopie zurückgeben, nicht die intern weiterverwendete Liste
        // selbst — sonst friert das Frontend (Zustand/Immer) sie beim
        // Einlagern in den Store ein, und ein späteres `.push()` hier im
        // Mock schlägt fehl (echtes Tauri-IPC serialisiert ohnehin immer
        // frisch, diese Referenz-Teilung ist ein reines Mock-Artefakt).
        return (photoKeywords[args.photoId as string] ?? []).map((k) => ({ ...k, ...keywordMetaFor(k.id) }));
      case "list_all_keywords": {
        const seen = new Map<string, { id: string; name: string }>();
        for (const list of Object.values(photoKeywords)) {
          for (const keyword of list) seen.set(keyword.id, keyword);
        }
        return [...seen.values()].map((k) => ({ ...k, ...keywordMetaFor(k.id) }));
      }

      // ---- Bibliothek: Schlagworthierarchie/Tag-Regeln/Metadaten (ab Phase 9
      // Schritt 2, siehe DECISIONS.md ADR-0035) --------------------------------
      case "set_keyword_parent": {
        const keywordId = args.keywordId as string;
        keywordMeta[keywordId] = { ...keywordMetaFor(keywordId), parent_id: (args.parentId as string | null) ?? null };
        return null;
      }
      case "set_keyword_synonyms": {
        const keywordId = args.keywordId as string;
        keywordMeta[keywordId] = { ...keywordMetaFor(keywordId), synonyms: [...(args.synonyms as string[])] };
        return null;
      }
      case "delete_keyword": {
        const keywordId = args.keywordId as string;
        for (const list of Object.values(photoKeywords)) {
          const index = list.findIndex((k) => k.id === keywordId);
          if (index >= 0) list.splice(index, 1);
        }
        delete keywordMeta[keywordId];
        for (const rule of [...tagRules]) {
          if (rule.keyword_id === keywordId) tagRules.splice(tagRules.indexOf(rule), 1);
        }
        return null;
      }
      case "create_tag_rule": {
        const id = `rule-${nextTagRuleId++}`;
        tagRules.push({
          id,
          name: args.name as string,
          keyword_id: args.keywordId as string,
          conditions_json: args.conditionsJson as string,
          enabled: true,
        });
        return id;
      }
      case "set_tag_rule_enabled": {
        const rule = tagRules.find((r) => r.id === (args.tagRuleId as string));
        if (rule) rule.enabled = args.enabled as boolean;
        return null;
      }
      case "delete_tag_rule": {
        const index = tagRules.findIndex((r) => r.id === (args.tagRuleId as string));
        if (index >= 0) tagRules.splice(index, 1);
        return null;
      }
      case "list_tag_rules":
        return [...tagRules];
      case "set_photo_metadata": {
        const photo = findPhoto(args.photoId as string);
        if (photo) {
          photo.title = args.title as string | null;
          photo.caption = args.caption as string | null;
          photo.copyright = args.copyright as string | null;
          photo.creator = args.creator as string | null;
        }
        return null;
      }
      case "export_xmp_sidecar":
        return fixtures.exportedXmpSidecarPath;
      case "import_xmp_develop_settings":
        return null;
      case "import_xmp_sidecar_from_file":
        return fixtures.xmpImportApplies;
      case "get_photo": {
        const photo = findPhoto(args.photoId as string);
        if (!photo) throw new Error(`Test-Stub: Foto "${args.photoId as string}" nicht gefunden`);
        return clonePhoto(photo);
      }
      case "catalog_statistics":
        return fixtures.catalogStatistics;
      case "preview_cache_stats":
        return fixtures.previewCacheStats;
      case "clear_preview_cache":
        fixtures.previewCacheStats = { file_count: 0, total_bytes: 0 };
        return null;
      case "generate_smart_previews":
        return (args.photoIds as string[]).length;
      case "denoise_photo":
        return fixtures.denoisedPhotoPath;
      case "upscale_photo":
        return fixtures.upscaledPhotoPath;
      case "convert_photo_to_dng":
        return fixtures.convertedDngPath;

      // ---- Bibliothek: Sammlungen (ab Phase 3, Sammlungssätze/intelligente -
      // Sammlungen ab Phase 9 Schritt 1) --------------------------------------
      case "create_collection": {
        const id = `col-${nextCollectionId++}`;
        collections.push({ id, name: args.name as string, folder_id: (args.folderId as string | null) ?? null, is_smart: false, smart_criteria_json: null });
        collectionPhotoIds[id] = [];
        return id;
      }
      case "create_smart_collection": {
        const id = `col-${nextCollectionId++}`;
        collections.push({
          id,
          name: args.name as string,
          folder_id: (args.folderId as string | null) ?? null,
          is_smart: true,
          smart_criteria_json: JSON.stringify(args.criteria),
        });
        collectionPhotoIds[id] = [];
        return id;
      }
      case "move_collection_to_folder": {
        const collection = collections.find((c) => c.id === (args.collectionId as string));
        if (collection) collection.folder_id = (args.folderId as string | null) ?? null;
        return null;
      }
      case "list_collections":
        // Kopie, aus demselben Grund wie bei `list_photo_keywords` oben.
        return [...collections];
      case "create_collection_folder": {
        const id = `col-folder-${nextCollectionFolderId++}`;
        collectionFolders.push({ id, name: args.name as string, parent_id: (args.parentId as string | null) ?? null, position: collectionFolders.length });
        return id;
      }
      case "rename_collection_folder": {
        const folder = collectionFolders.find((f) => f.id === (args.folderId as string));
        if (folder) folder.name = args.name as string;
        return null;
      }
      case "delete_collection_folder": {
        const index = collectionFolders.findIndex((f) => f.id === (args.folderId as string));
        if (index >= 0) collectionFolders.splice(index, 1);
        for (const c of collections) if (c.folder_id === (args.folderId as string)) c.folder_id = null;
        return null;
      }
      case "list_collection_folders":
        return [...collectionFolders];
      case "add_to_collection": {
        const collectionId = args.collectionId as string;
        const photoId = args.photoId as string;
        const ids = (collectionPhotoIds[collectionId] ??= []);
        if (!ids.includes(photoId)) ids.push(photoId);
        return null;
      }
      case "remove_from_collection": {
        const collectionId = args.collectionId as string;
        const photoId = args.photoId as string;
        collectionPhotoIds[collectionId] = (collectionPhotoIds[collectionId] ?? []).filter((id) => id !== photoId);
        return null;
      }
      case "list_photos_in_collection": {
        const ids = collectionPhotoIds[args.collectionId as string] ?? [];
        return ids
          .map((id) => findPhoto(id))
          .filter((p): p is MockPhoto => p !== undefined)
          .map(clonePhoto);
      }

      // ---- Bibliothek: virtuelle Kopien (Phase 9 Schritt 1) -----------------
      case "create_virtual_copy": {
        const source = findPhoto(args.photoId as string);
        if (!source) throw new Error(`Test-Stub: Foto '${args.photoId as string}' nicht gefunden`);
        const copyId = `${source.id}-vc${nextVirtualCopyId++}`;
        const copy: MockPhoto = { ...source, id: copyId };
        const fixtures = w.__mockFixtures as { photosByFolder: Record<string, MockPhoto[]> };
        for (const folderId of Object.keys(fixtures.photosByFolder)) {
          if (fixtures.photosByFolder[folderId].some((p) => p.id === source.id)) {
            fixtures.photosByFolder[folderId] = [...fixtures.photosByFolder[folderId], copy];
            break;
          }
        }
        (virtualCopiesBySource[source.id] ??= []).push(copyId);
        return clonePhoto(copy);
      }
      case "list_virtual_copies": {
        const ids = virtualCopiesBySource[args.photoId as string] ?? [];
        return ids.map((id) => findPhoto(id)).filter((p): p is MockPhoto => p !== undefined).map(clonePhoto);
      }

      // ---- Fortgeschrittenes: Stacking (Phase 9 Schritt 8) ------------------
      case "stack_focus":
        return importStackResultPhoto(args.photoIds as string[], "fokus_stack");
      case "stack_hdr":
        return importStackResultPhoto(args.photoIds as string[], "hdr");
      case "stack_panorama":
        return importStackResultPhoto(args.photoIds as string[], "panorama");
      case "stack_astro":
        return importStackResultPhoto(args.photoIds as string[], "astro_stack");

      // ---- Fortgeschrittenes: Skript-API + Plugin-System (Phase 9 Schritt 9) --
      case "run_develop_script": {
        const photoId = args.photoId as string;
        const history = (editHistories[photoId] ??= { entries: [], currentIndex: -1 });
        const previousEdlJson = history.currentIndex >= 0 ? history.entries[history.currentIndex].edl_json : "{}";
        history.entries = history.entries.slice(0, history.currentIndex + 1);
        historyCounter += 1;
        history.entries.push({ edl_json: previousEdlJson, created_at: new Date(historyCounter).toISOString(), label: "Skript" });
        history.currentIndex = history.entries.length - 1;
        return null;
      }
      case "run_plugin_custom_effect": {
        const photoId = args.photoId as string;
        return `/mock/derived/${photoId}-plugin.png`;
      }

      // ---- Bibliothek: Stapel (Phase 9 Schritt 1) ---------------------------
      case "create_stack": {
        const id = `stack-${nextStackId++}`;
        const photoIds = args.photoIds as string[];
        stacks.push({ id, name: (args.name as string | null) ?? null, cover_photo_id: photoIds[0] ?? null, photo_ids: photoIds });
        return id;
      }
      case "delete_stack": {
        const index = stacks.findIndex((s) => s.id === (args.stackId as string));
        if (index >= 0) stacks.splice(index, 1);
        return null;
      }
      case "set_stack_cover": {
        const stack = stacks.find((s) => s.id === (args.stackId as string));
        if (stack) stack.cover_photo_id = args.coverPhotoId as string;
        return null;
      }
      case "list_stacks":
        return [...stacks];
      case "auto_stack_by_time": {
        const photoIds = args.photoIds as string[];
        if (photoIds.length < 2) return [];
        const id = `stack-${nextStackId++}`;
        stacks.push({ id, name: null, cover_photo_id: photoIds[0] ?? null, photo_ids: photoIds });
        return [id];
      }

      // ---- Bibliothek: erweiterbare Farbmarkierungen (Phase 9 Schritt 1) ---
      case "list_color_label_definitions":
        return [...colorLabelDefinitions];
      case "create_color_label_definition": {
        colorLabelDefinitions.push({
          name: args.name as string,
          display_name: args.displayName as string,
          hex: args.hex as string,
          position: colorLabelDefinitions.length,
        });
        return null;
      }
      case "delete_color_label_definition": {
        const index = colorLabelDefinitions.findIndex((d) => d.name === (args.name as string));
        if (index >= 0) colorLabelDefinitions.splice(index, 1);
        return null;
      }

      // ---- Bibliothek: Perceptual-Hash-Duplikaterkennung (Phase 9 Schritt 1) -
      case "list_perceptual_duplicate_groups":
        return fixtures.perceptualDuplicateGroups;
      case "list_people_groups":
        return fixtures.peopleGroups;

      // ---- Bibliothek: Suche/Filter (ab Phase 3) ---------------------------
      case "search_photos": {
        const query = (args.query as string).toLowerCase();
        return allPhotos()
          .filter((p) => p.filename.toLowerCase().includes(query))
          .map(clonePhoto);
      }
      case "filter_photos": {
        const criteria = args.criteria as {
          rating_at_least?: number;
          flag?: number;
          color_label?: string;
          camera_model?: string;
        };
        return allPhotos()
          .filter((p) => matchesFilterCriteria(p, criteria))
          .map(clonePhoto);
      }
      case "search_and_filter_photos": {
        const query = (args.query as string | null)?.trim().toLowerCase();
        const criteria = args.criteria as {
          rating_at_least?: number;
          flag?: number;
          color_label?: string;
          camera_model?: string;
        };
        return allPhotos()
          .filter((p) => (!query ? true : p.filename.toLowerCase().includes(query)))
          .filter((p) => matchesFilterCriteria(p, criteria))
          .map(clonePhoto);
      }

      // ---- Stapelverarbeitungs-Konsole (Phase 11 Schritt 9) ------------------
      case "preview_batch_rule": {
        const criteria = args.criteria as {
          rating_at_least?: number;
          flag?: number;
          color_label?: string;
          camera_model?: string;
        };
        return allPhotos()
          .filter((p) => matchesFilterCriteria(p, criteria))
          .map(clonePhoto);
      }
      case "apply_batch_rule": {
        const criteria = args.criteria as {
          rating_at_least?: number;
          flag?: number;
          color_label?: string;
          camera_model?: string;
        };
        const action = args.action as
          | { kind: "SetRating"; rating: number }
          | { kind: "SetColorLabel"; color_label: string | null }
          | { kind: "AddKeyword"; name: string };
        const matched = allPhotos().filter((p) => matchesFilterCriteria(p, criteria));
        const items: MockBatchItem[] = [];
        for (const photo of matched) {
          if (action.kind === "SetRating") {
            if (photo.rating !== action.rating) {
              items.push({ photoId: photo.id, field: "rating", oldValue: photo.rating, newValue: action.rating });
              photo.rating = action.rating;
            }
          } else if (action.kind === "SetColorLabel") {
            if ((photo.color_label ?? null) !== (action.color_label ?? null)) {
              items.push({ photoId: photo.id, field: "color_label", oldValue: photo.color_label ?? null, newValue: action.color_label ?? null });
              photo.color_label = action.color_label;
            }
          } else {
            const list = (photoKeywords[photo.id] ??= []);
            if (!list.some((k) => k.name === action.name)) {
              const id = `kw-${nextKeywordId++}`;
              list.push({ id, name: action.name });
              items.push({ photoId: photo.id, field: "keyword", oldValue: null, newValue: id });
            }
          }
        }
        const batchId = `batch-${nextBatchId++}`;
        batchOperations[batchId] = items;
        return batchId;
      }
      case "undo_batch_operation": {
        const batchId = args.batchId as string;
        const items = batchOperations[batchId];
        if (!items) return 0;
        for (const item of items) {
          const photo = findPhoto(item.photoId);
          if (item.field === "rating") {
            if (photo) photo.rating = item.oldValue as number;
          } else if (item.field === "color_label") {
            if (photo) photo.color_label = item.oldValue as string | null;
          } else {
            const list = photoKeywords[item.photoId] ?? [];
            photoKeywords[item.photoId] = list.filter((k) => k.id !== item.newValue);
          }
        }
        delete batchOperations[batchId];
        return items.length;
      }

      // ---- Bibliothek: Duplikaterkennung (ab Phase 3, Schritt 8.2) ---------
      case "list_duplicate_photo_groups": {
        const byHash = new Map<string, MockPhoto[]>();
        for (const p of allPhotos()) {
          const hash = p.content_hash as string | null | undefined;
          if (!hash) continue;
          const group = byHash.get(hash) ?? [];
          group.push(p);
          byHash.set(hash, group);
        }
        return [...byHash.values()].filter((group) => group.length > 1).map((group) => group.map(clonePhoto));
      }

      // ---- Presets (ab Phase 5) ---------------------------------------------
      case "create_preset_folder": {
        const id = `preset-folder-${nextPresetFolderId++}`;
        const parentId = (args.parentId as string | null) ?? null;
        const siblingCount = presetFolders.filter((f) => f.parent_id === parentId).length;
        presetFolders.push({ id, name: args.name as string, parent_id: parentId, position: siblingCount });
        return id;
      }
      case "rename_preset_folder": {
        const folder = presetFolders.find((f) => f.id === args.folderId);
        if (folder) folder.name = args.name as string;
        return null;
      }
      case "delete_preset_folder": {
        const folderId = args.folderId as string;
        const toDelete = new Set([folderId]);
        // Kaskadierendes Löschen verschachtelter Unterordner, wie
        // `ON DELETE CASCADE` bei echten `preset_folders`.
        let grew = true;
        while (grew) {
          grew = false;
          for (const f of presetFolders) {
            if (f.parent_id && toDelete.has(f.parent_id) && !toDelete.has(f.id)) {
              toDelete.add(f.id);
              grew = true;
            }
          }
        }
        for (let i = presetFolders.length - 1; i >= 0; i -= 1) {
          if (toDelete.has(presetFolders[i].id)) presetFolders.splice(i, 1);
        }
        // Presets rutschen an die Wurzel statt gelöscht zu werden (siehe
        // `repository::presets::delete_folder`s Moduldoku).
        for (const preset of presets) {
          if (preset.folder_id && toDelete.has(preset.folder_id)) preset.folder_id = null;
        }
        return null;
      }
      case "list_preset_folders":
        return presetFolders.map((f) => ({ ...f }));
      case "create_preset": {
        const presetId = `preset-${nextPresetId++}`;
        const versionId = `preset-version-${nextPresetVersionId++}`;
        const now = new Date().toISOString();
        presets.push({
          id: presetId,
          folder_id: (args.folderId as string | null) ?? null,
          name: args.name as string,
          is_favorite: false,
          tags: [...(args.tags as string[])],
          conditions_json: args.conditionsJson as string,
          created_at: now,
        });
        presetVersions[presetId] = [
          { id: versionId, preset_id: presetId, sequence: 1, edl_subset_json: args.edlSubsetJson as string, created_at: now },
        ];
        return { preset_id: presetId, version_id: versionId };
      }
      case "update_preset_metadata": {
        const preset = findPreset(args.presetId as string);
        if (preset) {
          preset.folder_id = (args.folderId as string | null) ?? null;
          preset.name = args.name as string;
          preset.tags = [...(args.tags as string[])];
          preset.conditions_json = args.conditionsJson as string;
        }
        return null;
      }
      case "set_preset_favorite": {
        const preset = findPreset(args.presetId as string);
        if (preset) preset.is_favorite = args.isFavorite as boolean;
        return null;
      }
      case "delete_preset": {
        const presetId = args.presetId as string;
        const index = presets.findIndex((p) => p.id === presetId);
        if (index >= 0) presets.splice(index, 1);
        delete presetVersions[presetId];
        return null;
      }
      case "list_presets":
        return presets.map((p) => ({ ...p, tags: [...p.tags] }));
      case "add_preset_version": {
        const presetId = args.presetId as string;
        const versions = (presetVersions[presetId] ??= []);
        const versionId = `preset-version-${nextPresetVersionId++}`;
        const nextSequence = versions.reduce((max, v) => Math.max(max, v.sequence), 0) + 1;
        versions.push({
          id: versionId,
          preset_id: presetId,
          sequence: nextSequence,
          edl_subset_json: args.edlSubsetJson as string,
          created_at: new Date().toISOString(),
        });
        return versionId;
      }
      case "list_preset_versions":
        return [...(presetVersions[args.presetId as string] ?? [])];
      case "latest_preset_version": {
        const versions = presetVersions[args.presetId as string] ?? [];
        const latest = versions[versions.length - 1];
        if (!latest) throw new Error(`Test-Stub: keine Version für Preset "${String(args.presetId)}"`);
        return { ...latest };
      }
      // `.apx`-Dateidialoge lassen sich ohne echtes Betriebssystemfenster
      // nicht sinnvoll nachbilden — beide Commands liefern stattdessen ein
      // per Fixture steuerbares Ergebnis (Default: `exportApxPathResult`
      // ein fester Mock-Pfad, `importApxFile` `null` = abgebrochener
      // Dialog). Die eigentliche Serialisierungs-/Parsing-Logik ist
      // bereits in `apx-app`s `commands.rs`-Tests abgedeckt — hier geht es
      // nur um die Frontend-Anbindung (Preset-Liste aktualisiert sich nach
      // einem erfolgreichen Import).
      case "export_preset_to_apx_file":
        return fixtures.exportApxPathResult;
      case "export_preset_to_lrtemplate_file":
        return fixtures.exportLrtemplatePathResult;
      case "import_preset_from_apx_file": {
        const file = fixtures.importApxFile;
        if (!file) return null;
        const presetId = `preset-${nextPresetId++}`;
        const versionId = `preset-version-${nextPresetVersionId++}`;
        const now = new Date().toISOString();
        const preset = {
          id: presetId,
          folder_id: (args.folderId as string | null) ?? null,
          name: file.name,
          is_favorite: false,
          tags: [...file.tags],
          conditions_json: file.conditions_json,
          created_at: now,
        };
        presets.push(preset);
        presetVersions[presetId] = [{ id: versionId, preset_id: presetId, sequence: 1, edl_subset_json: file.edl_subset_json, created_at: now }];
        return { ...preset, tags: [...preset.tags] };
      }

      // ---- Fortgeschrittenes: Kollaborationsmodus (Phase 9 Schritt 10) ----
      case "export_catalog_share":
        return fixtures.exportSharePathResult;
      case "import_catalog_share":
        return fixtures.importShareResult;
      case "resolve_share_conflict":
        return null;

      // ---- Fortgeschrittenes: Tethered Shooting (Phase 9 Schritt 11) -----
      case "tether_connect":
        return fixtures.tetherCameraResult;
      case "tether_capture":
        return fixtures.tetherCapturePhoto;
      case "list_removable_volumes":
        return fixtures.removableVolumes;
      case "list_camera_files":
        return fixtures.cameraFiles;
      case "import_from_camera":
        fixtures.cameraFiles = fixtures.cameraFiles.filter(
          (f) => !(f.folder === args.folder && f.name === args.name),
        );
        return fixtures.cameraImportPhoto;

      // ---- KI-Funktionen (Phase 7, siehe DECISIONS.md ADR-0033) ----------
      case "generate_ai_mask":
        return { kind: args.kind, ...fixtures.aiMaskAlpha };
      case "suggest_repair_source":
        return fixtures.repairSourceSuggestion;
      case "detect_sensor_spots":
        return fixtures.sensorSpots;
      case "get_ai_settings":
        return {
          anthropic_api_key: fixtures.anthropicApiKey,
          inpainting_model_path: fixtures.inpaintingModelPath,
        };
      case "download_inpainting_model":
        fixtures.inpaintingModelPath = "/mock/models/lama_fp32.onnx";
        return fixtures.inpaintingModelPath;
      case "clear_inpainting_model_path":
        fixtures.inpaintingModelPath = null;
        return null;
      case "run_ai_inpaint": {
        const w = 4;
        const h = 4;
        // Reines Grau, 4x4 — reicht als Platzhalter-Ergebnis für e2e-Tests,
        // die nur den Anwenden-Fluss (Strich bekommt ein `ai_fill`) prüfen,
        // nicht die tatsächliche Bildqualität.
        const pixels = new Uint8Array(w * h * 3).fill(128);
        let binary = "";
        for (const byte of pixels) binary += String.fromCharCode(byte);
        return {
          x: args.x,
          y: args.y,
          width: args.width,
          height: args.height,
          bitmap_width: w,
          bitmap_height: h,
          pixels_base64: btoa(binary),
        };
      }
      case "get_ui_settings":
        return fixtures.uiSettings;
      case "set_ui_settings":
        fixtures.uiSettings = args.settings as typeof fixtures.uiSettings;
        return null;
      case "get_watched_folder_settings":
        return fixtures.watchedFolderSettings;
      case "set_watched_folder_settings":
        fixtures.watchedFolderSettings = args.settings as typeof fixtures.watchedFolderSettings;
        return null;
      case "set_anthropic_api_key": {
        const key = args.apiKey as string | null;
        fixtures.anthropicApiKey = key && key.trim() !== "" ? key : null;
        return null;
      }
      case "generate_preset_from_llm":
        return fixtures.presetGeneratorSubsetJson;
      case "build_preset_prompt_text":
        return `Du bist ein Farbbearbeitungs-Assistent für Aperture X.\n\nGewünschter Bildlook: ${args.description as string}`;
      case "import_preset_json": {
        // Toleriert einen Markdown-Codeblock ums JSON, genau wie das
        // echte Backend (`apx_ai::preset_generator::extract_json_object`)
        // — Claude fügt oft trotz gegenteiliger Anweisung einen an.
        const raw = (args.json as string).trim();
        const withoutPrefix = (raw.startsWith("```") ? raw.replace(/^```(json)?/, "") : raw).trim();
        const candidate = (withoutPrefix.endsWith("```") ? withoutPrefix.slice(0, -3) : withoutPrefix).trim();
        const parsed = JSON.parse(candidate) as Record<string, unknown>;
        const allowedKeys = [
          "basic",
          "curves",
          "hsl",
          "color_mixer",
          "color_grading",
          "details",
          "lens_corrections",
          "effects",
          "calibration",
          "geometry",
        ];
        for (const key of Object.keys(parsed)) {
          if (!allowedKeys.includes(key)) {
            throw new Error(`Test-Stub: unbekannte Sektion '${key}'`);
          }
        }
        return JSON.stringify(parsed);
      }
      case "generate_preset_from_reference":
        return fixtures.referenceImageDialogCancelled ? null : fixtures.presetGeneratorSubsetJson;
      case "generate_preset_variations": {
        const base = JSON.parse(args.edlSubsetJson as string) as { basic?: { exposure_ev?: number } };
        const count = args.count as number;
        return Array.from({ length: count }, (_, index) =>
          JSON.stringify({ basic: { ...base.basic, exposure_ev: (base.basic?.exposure_ev ?? 0) + index * 0.1 } }),
        );
      }
      case "learn_preset_from_photos":
        return fixtures.presetGeneratorSubsetJson;
      case "suggest_tags":
        return fixtures.tagSuggestions;

      case "export_photo": {
        if (fixtures.exportPhotoShouldFail) {
          throw new Error("Test-Stub: Export fehlgeschlagen");
        }
        return fixtures.exportPhotoOutcome;
      }

      case "enqueue_export_photo": {
        const id = nextExportJobId++;
        const status: "pending" | "done" | "failed" = exportQueuePaused ? "pending" : fixtures.exportPhotoShouldFail ? "failed" : "done";
        exportQueueJobs.push({ id, status });
        return id;
      }
      case "export_queue_progress": {
        const total = exportQueueJobs.length;
        const done = exportQueueJobs.filter((j) => j.status !== "pending").length;
        const failed = exportQueueJobs.filter((j) => j.status === "failed").length;
        return { done, total, failed, paused: exportQueuePaused };
      }
      case "pause_export_queue":
        exportQueuePaused = true;
        return null;
      case "resume_export_queue": {
        exportQueuePaused = false;
        for (const job of exportQueueJobs) {
          if (job.status === "pending") job.status = fixtures.exportPhotoShouldFail ? "failed" : "done";
        }
        return null;
      }
      case "cancel_export_job": {
        const job = exportQueueJobs.find((j) => j.id === (args.jobId as number));
        if (!job || job.status !== "pending") return false;
        job.status = "failed";
        return true;
      }
      case "clear_finished_export_jobs": {
        const stillPending = exportQueueJobs.filter((j) => j.status === "pending");
        exportQueueJobs.splice(0, exportQueueJobs.length, ...stillPending);
        return null;
      }
      case "pick_file_path":
        return fixtures.pickFilePathResult;
      case "pick_save_file_path":
        return fixtures.pickSaveFilePathResult;
      case "print_photos": {
        if (fixtures.printPhotoShouldFail) {
          throw new Error("Test-Stub: Drucken fehlgeschlagen");
        }
        return fixtures.printOutcome;
      }
      case "check_ffmpeg_available":
        return fixtures.ffmpegAvailable;
      case "export_slideshow_video": {
        if (fixtures.slideshowVideoShouldFail) {
          throw new Error("Test-Stub: Video-Export fehlgeschlagen");
        }
        return fixtures.slideshowVideoOutcome;
      }
      case "export_book_pdf": {
        if (fixtures.bookExportShouldFail) {
          throw new Error("Test-Stub: Buch-Export fehlgeschlagen");
        }
        return fixtures.bookOutcome;
      }
      case "export_web_gallery": {
        if (fixtures.webExportShouldFail) {
          throw new Error("Test-Stub: Web-Export fehlgeschlagen");
        }
        return fixtures.webGalleryOutcome;
      }

      // ---- Karte (Phase 8 Schritt 7) ---------------------------------
      case "list_geotagged_photos":
        return allPhotos()
          .filter((p) => p.gps_lat !== undefined && p.gps_lat !== null)
          .map((p) => clonePhoto(p));
      case "reverse_geocode_location":
        return fixtures.geocodedLocation;
      case "import_gpx_track":
        return fixtures.gpxTrackPoints;
      case "set_photo_gps": {
        const photo = findPhoto(args.photoId as string);
        if (!photo) throw new Error(`Test-Stub: Foto '${args.photoId as string}' nicht gefunden`);
        photo.gps_lat = args.lat as number | null;
        photo.gps_lon = args.lon as number | null;
        return null;
      }

      // ---- Vorlagen (Phase 8 Schritt 8) -------------------------------
      case "save_template": {
        const id = `template-${nextTemplateId++}`;
        templates.push({
          id,
          kind: args.kind as string,
          name: args.name as string,
          payload_json: args.payloadJson as string,
          created_at: new Date().toISOString(),
        });
        return id;
      }
      case "list_templates":
        return templates
          .filter((t) => t.kind === (args.kind as string))
          .slice()
          .sort((a, b) => a.name.localeCompare(b.name));
      case "delete_template": {
        const index = templates.findIndex((t) => t.id === (args.templateId as string));
        if (index >= 0) templates.splice(index, 1);
        return null;
      }
      case "export_template_to_file":
        return fixtures.exportTemplateFilePathResult;
      case "import_template_from_file": {
        const file = fixtures.importedTemplateFile;
        if (!file) return null;
        const id = `template-${nextTemplateId++}`;
        const created: MockTemplate = { id, kind: file.kind, name: file.name, payload_json: file.payload_json, created_at: new Date().toISOString() };
        templates.push(created);
        return created;
      }

      case "import_dcp_profile":
        // Kein Fixture-Feld nötig — der Dialog wird in Tests nie
        // ausgelöst (kein e2e-Test deckt Phase 13 Schritt 3 bisher ab);
        // liefert konsistent "abgebrochen" statt eines unbekannten-
        // Befehl-Fehlers, falls doch einmal geklickt wird.
        return null;

      default:
        throw new Error(`Test-Stub: unbekannter invoke-Befehl "${cmd}"`);
    }
  }

  w.__TAURI_INTERNALS__ = {
    convertFileSrc: (path: string, protocol = "asset") =>
      `${window.location.origin}/__apx_mock_image__?src=${encodeURIComponent(`${protocol}:${path}`)}`,
    invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
    transformCallback: (callback: (arg: unknown) => void, once = false) => {
      const id = nextCallbackId++;
      callbacks[id] = { fn: callback, once };
      return id;
    },
    unregisterCallback: (id: number) => {
      delete callbacks[id];
    },
  };
}

// Ein 2×2-PNG (transparent) als Antwort auf jede `__apx_mock_image__`-
// Anfrage — reicht aus, damit `createImageBitmap()` einen echten Treffer
// landet, ohne eine Bilddatei ins Repo aufzunehmen.
const TINY_PNG_BASE64 =
  "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFUlEQVR42mNk+A8EDEwMo4ImDgYAAKn9BPz2j1CmAAAAAElFTkSuQmCC";

/**
 * Installiert die simulierte Tauri-Brücke für die gegebene Seite.
 *
 * `initialFixtures` wird schon *vor* dem ersten Laden der Seite gesetzt
 * (über `page.addInitScript`s Argument) — nötig für Tests, die nicht den
 * kompletten Import-Ablauf nachspielen, sondern direkt mit vorbefülltem
 * Katalogzustand starten wollen (z. B. der Virtualisierungs-Test), da
 * `App.tsx` `refreshFolders()` bereits beim Mounten aufruft, also noch
 * bevor ein nachträgliches `setMockFixtures()` greifen könnte.
 */
export async function installTauriMock(page: Page, initialFixtures: Record<string, unknown> = {}): Promise<void> {
  await page.route("**/__apx_mock_image__*", async (route) => {
    const src = new URL(route.request().url()).searchParams.get("src") ?? "";
    if (src.startsWith("apx:develop/")) {
      // Entwickeln-Route (ab Phase 2): kein Bildformat, sondern ein
      // 8-Byte-Breite/Höhe-Header + rohes RGBA8 (siehe
      // `crates/apx-app/src/protocol/mod.rs`, `DECISIONS.md` ADR-0016) —
      // hier ein einheitlich warm-orange gefärbtes, undurchsichtiges
      // 2×2-Bild (bewusst *nicht* neutral-grau, siehe `develop-flow.spec.ts`s
      // Weißabgleich-Pipette-Test, Phase 4 Schritt 3 — der braucht einen
      // echten Farbstich, um eine tatsächliche Korrektur zu erzeugen).
      const width = 2;
      const height = 2;
      const header = Buffer.alloc(8);
      header.writeUInt32LE(width, 0);
      header.writeUInt32LE(height, 4);
      const pixels = Buffer.alloc(width * height * 4);
      for (let i = 0; i < pixels.length; i += 4) {
        pixels[i] = 180;
        pixels[i + 1] = 140;
        pixels[i + 2] = 100;
        pixels[i + 3] = 255;
      }
      await route.fulfill({
        status: 200,
        contentType: "application/x-apx-develop-rgba8",
        body: Buffer.concat([header, pixels]),
      });
      return;
    }

    await route.fulfill({
      status: 200,
      contentType: "image/png",
      body: Buffer.from(TINY_PNG_BASE64, "base64"),
    });
  });
  await page.addInitScript(installBridge, initialFixtures);
}

/** Ersetzt (per `Object.assign`) einen Teil der Invoke-Fixture-Daten. */
export async function setMockFixtures(page: Page, partial: Record<string, unknown>): Promise<void> {
  await page.evaluate((data) => {
    Object.assign((window as unknown as Record<string, unknown>).__mockFixtures as object, data);
  }, partial);
}

/** Simuliert ein vom Backend emittiertes Tauri-Event (siehe `useImportEvents`). */
export async function emitMockEvent(page: Page, eventName: string, payload: unknown): Promise<void> {
  await page.evaluate(
    ([name, data]) => {
      (window as unknown as { __mockEmit: (n: string, p: unknown) => void }).__mockEmit(name, data);
    },
    [eventName, payload] as const,
  );
}

/** Liest das Protokoll aller bisherigen `invoke()`-Aufrufe aus (zum Assertieren). */
export async function getMockInvokeLog(page: Page): Promise<Array<{ cmd: string; args: unknown }>> {
  return page.evaluate(() => (window as unknown as Record<string, unknown>).__mockInvokeLog as Array<{ cmd: string; args: unknown }>);
}
