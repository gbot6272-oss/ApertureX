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
    importApxFile: null as { name: string; tags: string[]; conditions_json: string; edl_subset_json: string } | null,
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
  }
  const collections: MockCollection[] = [];
  const collectionPhotoIds: Record<string, string[]> = {};
  const photoKeywords: Record<string, { id: string; name: string }[]> = {};
  let nextCollectionId = 1;
  let nextKeywordId = 1;

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
  const presetVersions: Record<string, MockPresetVersion[]> = {};
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
    entries: Array<{ edl_json: string }>;
    currentIndex: number; // -1 = neutral (noch nie bearbeitet / bis zum Anfang zurück)
  }
  const editHistories: Record<string, EditHistoryState> = {};

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
      importApxFile: { name: string; tags: string[]; conditions_json: string; edl_subset_json: string } | null;
      aiMaskAlpha: { width: number; height: number; alpha_base64: string };
      repairSourceSuggestion: { x: number; y: number };
      sensorSpots: Array<{ x: number; y: number; radius: number; strength: number }>;
      anthropicApiKey: string | null;
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
        const history = (editHistories[photoId] ??= { entries: [], currentIndex: -1 });
        // Verwirft eine zuvor per Rückgängig erreichte "Zukunft" — siehe
        // `DECISIONS.md` ADR-0014, exakt wie das echte Backend.
        history.entries = history.entries.slice(0, history.currentIndex + 1);
        history.entries.push({ edl_json: edlJson });
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
        return [...(photoKeywords[args.photoId as string] ?? [])];
      case "list_all_keywords": {
        const seen = new Map<string, { id: string; name: string }>();
        for (const list of Object.values(photoKeywords)) {
          for (const keyword of list) seen.set(keyword.id, keyword);
        }
        return [...seen.values()];
      }

      // ---- Bibliothek: Sammlungen (ab Phase 3) -----------------------------
      case "create_collection": {
        const id = `col-${nextCollectionId++}`;
        collections.push({ id, name: args.name as string });
        collectionPhotoIds[id] = [];
        return id;
      }
      case "list_collections":
        // Kopie, aus demselben Grund wie bei `list_photo_keywords` oben.
        return [...collections];
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

      // ---- KI-Funktionen (Phase 7, siehe DECISIONS.md ADR-0033) ----------
      case "generate_ai_mask":
        return { kind: args.kind, ...fixtures.aiMaskAlpha };
      case "suggest_repair_source":
        return fixtures.repairSourceSuggestion;
      case "detect_sensor_spots":
        return fixtures.sensorSpots;
      case "get_ai_settings":
        return { anthropic_api_key: fixtures.anthropicApiKey };
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
