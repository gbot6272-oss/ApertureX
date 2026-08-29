import { create } from "zustand";
import { immer } from "zustand/middleware/immer";

import { buildEdlEnvelopeJson, NEUTRAL_BASIC_ADJUSTMENTS, parseEdlEnvelopeJson, writeBasicField } from "../lib/edl";
import type { BasicAdjustments } from "../lib/edl";
import * as api from "../lib/tauri";
import type {
  CatalogStatusDto,
  CollectionDto,
  FilterCriteriaDto,
  FolderDto,
  HistoryPositionDto,
  KeywordDto,
  PhotoDto,
} from "../lib/tauri";

/** Wandelt eine `HistoryPositionDto` (siehe `lib/tauri.ts`) in
 * `BasicAdjustments` um — `Neutral` bedeutet "wie aufgenommen", ein
 * unlesbares `edl_json` fällt (mit einer Konsolen-Warnung) ebenfalls auf
 * neutral zurück statt abzustürzen. */
function basicFromHistoryPosition(position: HistoryPositionDto): BasicAdjustments {
  if (position.kind === "Neutral") return NEUTRAL_BASIC_ADJUSTMENTS;
  const parsed = parseEdlEnvelopeJson(position.edl_json);
  if (!parsed) {
    console.error("Unlesbares EDL vom Backend erhalten, falle auf neutral zurück:", position.edl_json);
    return NEUTRAL_BASIC_ADJUSTMENTS;
  }
  return parsed;
}

/** In welcher Reihenfolge Foto-ID `photoId` in der aktuell angezeigten
 * Liste gemeint ist, wenn ein Bereich per Umschalt-Klick markiert wird —
 * siehe [`selectActivePhotos`]. */
export type SelectionMode = "replace" | "toggle" | "range";

/** Liest aus einem Klick-Event, welcher Auswahlmodus gemeint ist (Strg/Cmd
 * = einzelnes Umschalten, Umschalt = Bereich, sonst Ersetzen) — von Raster
 * und Filmstreifen gemeinsam genutzt (siehe `DECISIONS.md` ADR-0024). */
export function resolveSelectionMode(event: { ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }): SelectionMode {
  if (event.shiftKey) return "range";
  if (event.ctrlKey || event.metaKey) return "toggle";
  return "replace";
}

/** Patcht ein Foto an jeder Stelle, wo es aktuell im Zustand zwischen-
 * gespeichert sein könnte (Ordner-Cache, Sammlungs-Cache, Such-/Filter-
 * Ergebnis) — hält Raster/Filmstreifen nach einer Bewertungs-/Flaggen-/
 * Farb-Änderung sofort konsistent, ohne jedes Mal neu vom Backend zu
 * laden. Muss innerhalb eines Immer-`set()`-Producers aufgerufen werden. */
function patchPhotoEverywhere(state: AppStore, photoId: string, patch: Partial<PhotoDto>) {
  for (const list of Object.values(state.photosByFolder)) {
    const target = list.find((p) => p.id === photoId);
    if (target) Object.assign(target, patch);
  }
  for (const list of Object.values(state.collectionPhotos)) {
    const target = list.find((p) => p.id === photoId);
    if (target) Object.assign(target, patch);
  }
  if (state.libraryResults) {
    const target = state.libraryResults.find((p) => p.id === photoId);
    if (target) Object.assign(target, patch);
  }
}

/** Die aktuell anzuzeigende Fotoliste — Suchergebnisse (falls eine Suche/
 * ein Filter aktiv ist) haben Vorrang vor einer ausgewählten Sammlung, die
 * wiederum Vorrang vor einem ausgewählten Ordner hat. Raster und
 * Filmstreifen lesen beide über diese eine Funktion (siehe `DECISIONS.md`
 * ADR-0024: geteilter Fotoliste-Zustand statt eigener Parallel-Logik). */
export function selectActivePhotos(state: AppStore): PhotoDto[] {
  if (state.libraryResults !== null) return state.libraryResults;
  if (state.selectedCollectionId) return state.collectionPhotos[state.selectedCollectionId] ?? [];
  if (state.selectedFolderId) return state.photosByFolder[state.selectedFolderId] ?? [];
  return [];
}

/**
 * Zustand-Store mit den in `PHASE1_PROMPT.md` Abschnitt 7 geforderten
 * Slices: `catalog`, `selection`, `viewer`, `jobs`. Alle vier leben in
 * einer Datei statt in getrennten Modulen — bei vier kleinen Slices
 * vermeidet das die sonst nötigen zirkulären Typ-Importe des
 * Zustand-"Slices-Patterns", ohne die Struktur zu verlieren (jeder
 * Abschnitt unten ist unabhängig lesbar).
 *
 * Undo/Redo-Middleware (laut SPEC.md Abschnitt 1 für State vorgesehen)
 * kommt erst mit den Entwickeln-Reglern in Phase 2 — Phase 1 hat noch
 * keine Bearbeitungs-Historie, die man rückgängig machen könnte.
 */

// ---- Catalog-Slice ---------------------------------------------------

interface CatalogSlice {
  folders: FolderDto[];
  photosByFolder: Record<string, PhotoDto[]>;
  catalogStatus: CatalogStatusDto | null;
  catalogError: string | null;
  refreshFolders: () => Promise<void>;
  refreshCatalogStatus: () => Promise<void>;
  loadPhotosForFolder: (folderId: string) => Promise<void>;
  /** Verknüpft einen als fehlend markierten Ordner mit `newPath` neu und
   * lädt Ordnerliste sowie (falls gerade geöffnet) dessen Fotos danach
   * neu — siehe `FolderDto.missing`. */
  relinkFolder: (folderId: string, newPath: string) => Promise<void>;
}

// ---- Selection-Slice ---------------------------------------------------

interface SelectionSlice {
  selectedFolderId: string | null;
  selectedPhotoId: string | null;
  selectFolder: (folderId: string | null) => void;
  selectPhoto: (photoId: string | null) => void;
  /** Wählt das nächste/vorherige Foto im aktuell selektierten Ordner. */
  stepSelection: (direction: 1 | -1) => void;
}

// ---- Viewer-Slice ---------------------------------------------------

export type FitMode = "fit" | "fill" | "manual";

interface ViewerSlice {
  zoom: number; // 1.0 = 100 %
  fitMode: FitMode;
  panX: number;
  panY: number;
  setZoom: (zoom: number, fitMode?: FitMode) => void;
  setPan: (x: number, y: number) => void;
  resetView: () => void;
}

// ---- Jobs-Slice (Import) ---------------------------------------------------

interface ImportProgressState {
  done: number;
  total: number;
  currentFile: string | null;
}

interface ImportResultState {
  imported: number;
  skipped: number;
  errorCount: number;
  cancelled: boolean;
}

interface JobsSlice {
  importRunning: boolean;
  importProgress: ImportProgressState | null;
  importResult: ImportResultState | null;
  importErrors: string[];
  startImport: (path: string) => Promise<void>;
  cancelImport: () => Promise<void>;
  setImportProgress: (progress: ImportProgressState) => void;
  addImportError: (line: string) => void;
  finishImport: (result: ImportResultState) => void;
}

// ---- Develop-Slice (ab Phase 2) ---------------------------------------

interface DevelopSlice {
  developPanelOpen: boolean;
  /** Der aktuell im Panel gezeigte (u. U. noch nicht committete)
   * Bearbeitungszustand — laufend über `setBasicField` verändert,
   * während gezogen wird; nur `commitDevelopEdit()` schreibt ihn dauerhaft
   * in den Katalog (siehe `crates/apx-catalog`s `edit_history`, ADR-0014). */
  developBasic: BasicAdjustments;
  /** Zu welchem Foto `developBasic` gehört — verhindert, dass beim
   * schnellen Fotowechsel ein veralteter Zustand kurz sichtbar bleibt. */
  developPhotoId: string | null;
  toggleDevelopPanel: () => void;
  /** Lädt den zuletzt gespeicherten Bearbeitungszustand für `photoId`
   * (oder neutral, falls noch nie bearbeitet) — aufgerufen beim Öffnen
   * des Panels und bei jedem Fotowechsel, während es offen ist. */
  loadDevelopStateForPhoto: (photoId: string) => Promise<void>;
  /** Setzt ein einzelnes Feld (Regler-Zwischenwert beim Ziehen) — siehe
   * `lib/edl.ts`s `SliderSpec.key` für gültige Schlüssel. */
  setBasicField: (key: string, value: number) => void;
  /** Schreibt `developBasic` als neuen Verlaufs-Schritt (siehe
   * `PLAN.md` Phase 2 Schritt 5/6: ausgelöst beim Loslassen eines
   * Reglers, nicht bei jedem Zwischenwert). */
  commitDevelopEdit: (label?: string) => Promise<void>;
  undoDevelop: () => Promise<void>;
  redoDevelop: () => Promise<void>;
  /** Zuletzt gemessene Ende-zu-Ende-Antwortzeit der `develop/...`-Route
   * in Millisekunden (siehe `hooks/useDevelopRender`) — nur zur
   * Beobachtung/ehrlichen Dokumentation des 16-ms-Ziels (`PLAN.md` Phase
   * 2 Schritt 7), keine Business-Logik hängt daran. */
  developLastLatencyMs: number | null;
  setDevelopLatencyMs: (ms: number) => void;
}

// ---- Library-Slice (ab Phase 3: Raster, Bewertung/Flagge/Farbe,
// Sammlungen, Suche/Filter, Metadaten-Panel) --------------------------------

interface LibrarySlice {
  /** Was in der Mitte statt des Viewers gezeigt wird — `"grid"` ist das
   * neue Raster (Schritt 6), `"viewer"` der bisherige Einzelbild-Viewer. */
  centerView: "viewer" | "grid";
  toggleCenterView: () => void;

  /** Mehrfachauswahl fürs Stapel-Bearbeiten (Bewertung/Flagge/Sammlung-
   * Hinzufügen) — geteilt zwischen Raster und Filmstreifen. Enthält
   * `selectedPhotoId`, sobald eines gesetzt ist. */
  multiSelectedIds: string[];
  togglePhotoSelection: (photoId: string, mode: SelectionMode) => void;

  metadataPanelOpen: boolean;
  toggleMetadataPanel: () => void;

  photoKeywords: Record<string, KeywordDto[]>;
  loadKeywordsForPhoto: (photoId: string) => Promise<void>;
  addKeywordToPhoto: (photoId: string, name: string) => Promise<void>;
  removeKeywordFromPhoto: (photoId: string, keywordId: string) => Promise<void>;

  /** Setzt Bewertung/Flagge/Farbe. Ist `photoId` Teil einer Mehrfach-
   * auswahl mit mehr als einem Eintrag, wirkt die Änderung auf die
   * gesamte Auswahl (Stapel-Bearbeitung) — sonst nur auf `photoId`. */
  setPhotoRating: (photoId: string, rating: number) => Promise<void>;
  setPhotoFlag: (photoId: string, flag: number) => Promise<void>;
  setPhotoColorLabel: (photoId: string, colorLabel: string | null) => Promise<void>;

  collections: CollectionDto[];
  selectedCollectionId: string | null;
  collectionPhotos: Record<string, PhotoDto[]>;
  refreshCollections: () => Promise<void>;
  createCollection: (name: string) => Promise<void>;
  selectCollection: (collectionId: string | null) => void;
  loadPhotosForCollection: (collectionId: string) => Promise<void>;
  /** Fügt die aktuelle Mehrfachauswahl (oder, falls leer, das fokussierte
   * Foto) zu `collectionId` hinzu. */
  addSelectionToCollection: (collectionId: string) => Promise<void>;

  /** Freitextsuche (FTS5 über Dateiname/Kamera/Objektiv) und
   * Attributfilter sind laut `PLAN.md` Phase 3 Schritt 6 bewusst
   * alternativ statt kombiniert — das Setzen des einen leert das andere. */
  libraryQuery: string;
  libraryFilter: FilterCriteriaDto;
  /** `null` = keine Suche/kein Filter aktiv, Raster/Filmstreifen zeigen
   * den ausgewählten Ordner/die Sammlung normal (siehe
   * [`selectActivePhotos`]). */
  libraryResults: PhotoDto[] | null;
  setLibraryQuery: (query: string) => void;
  runLibrarySearch: () => Promise<void>;
  /** Setzt oder entfernt (bei `undefined`) einen Filter-Chip und wendet
   * das Ergebnis sofort an. */
  setLibraryFilterChip: (patch: FilterCriteriaDto) => Promise<void>;
  clearLibraryFilters: () => void;
}

export type AppStore = CatalogSlice & SelectionSlice & ViewerSlice & JobsSlice & DevelopSlice & LibrarySlice;

export const useAppStore = create<AppStore>()(
  immer((set, get) => ({
    // Catalog
    folders: [],
    photosByFolder: {},
    catalogStatus: null,
    catalogError: null,

    refreshFolders: async () => {
      try {
        const folders = await api.listFolders();
        set((state) => {
          state.folders = folders;
          state.catalogError = null;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    refreshCatalogStatus: async () => {
      try {
        const status = await api.getCatalogStatus();
        set((state) => {
          state.catalogStatus = status;
          state.catalogError = null;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    loadPhotosForFolder: async (folderId) => {
      try {
        const photos = await api.listPhotosInFolder(folderId);
        set((state) => {
          state.photosByFolder[folderId] = photos;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    relinkFolder: async (folderId, newPath) => {
      try {
        await api.relinkFolder(folderId, newPath);
        await get().refreshFolders();
        if (get().photosByFolder[folderId]) {
          await get().loadPhotosForFolder(folderId);
        }
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    // Selection
    selectedFolderId: null,
    selectedPhotoId: null,

    selectFolder: (folderId) => {
      set((state) => {
        state.selectedFolderId = folderId;
        state.selectedCollectionId = null;
        state.selectedPhotoId = null;
        state.multiSelectedIds = [];
        state.libraryQuery = "";
        state.libraryResults = null;
      });
      if (folderId) {
        void get().loadPhotosForFolder(folderId);
      }
    },

    selectPhoto: (photoId) => {
      set((state) => {
        state.selectedPhotoId = photoId;
        state.multiSelectedIds = photoId ? [photoId] : [];
      });
      get().resetView();
      // Läuft das Entwickeln-Panel bereits, muss es beim Fotowechsel den
      // Bearbeitungszustand des *neuen* Fotos laden statt den alten kurz
      // weiter anzuzeigen.
      if (get().developPanelOpen) {
        if (photoId) {
          void get().loadDevelopStateForPhoto(photoId);
        } else {
          set((state) => {
            state.developBasic = NEUTRAL_BASIC_ADJUSTMENTS;
            state.developPhotoId = null;
          });
        }
      }
      if (get().metadataPanelOpen && photoId) {
        void get().loadKeywordsForPhoto(photoId);
      }
    },

    stepSelection: (direction) => {
      const state = get();
      const photos = selectActivePhotos(state);
      if (photos.length === 0) return;

      const currentIndex = photos.findIndex((p) => p.id === state.selectedPhotoId);
      const nextIndex = currentIndex === -1 ? 0 : (currentIndex + direction + photos.length) % photos.length;
      const next = photos[nextIndex];
      if (next) {
        get().selectPhoto(next.id);
      }
    },

    // Viewer
    zoom: 1,
    fitMode: "fit",
    panX: 0,
    panY: 0,

    setZoom: (zoom, fitMode = "manual") => {
      set((state) => {
        state.zoom = zoom;
        state.fitMode = fitMode;
      });
    },

    setPan: (x, y) => {
      set((state) => {
        state.panX = x;
        state.panY = y;
      });
    },

    resetView: () => {
      set((state) => {
        state.zoom = 1;
        state.fitMode = "fit";
        state.panX = 0;
        state.panY = 0;
      });
    },

    // Jobs (Import)
    importRunning: false,
    importProgress: null,
    importResult: null,
    importErrors: [],

    startImport: async (path) => {
      set((state) => {
        state.importRunning = true;
        state.importProgress = { done: 0, total: 0, currentFile: null };
        state.importResult = null;
        state.importErrors = [];
      });
      try {
        await api.importFolder(path);
      } catch (err) {
        set((state) => {
          state.importRunning = false;
          state.importErrors.push(String(err));
        });
      }
    },

    cancelImport: async () => {
      await api.cancelImport();
    },

    setImportProgress: (progress) => {
      set((state) => {
        state.importProgress = progress;
      });
    },

    addImportError: (line) => {
      set((state) => {
        state.importErrors.push(line);
      });
    },

    finishImport: (result) => {
      set((state) => {
        state.importRunning = false;
        state.importResult = result;
        state.importProgress = null;
      });
      void get().refreshFolders();
      void get().refreshCatalogStatus();
      const { selectedFolderId } = get();
      if (selectedFolderId) {
        void get().loadPhotosForFolder(selectedFolderId);
      }
    },

    // Develop
    developPanelOpen: false,
    developBasic: NEUTRAL_BASIC_ADJUSTMENTS,
    developPhotoId: null,

    toggleDevelopPanel: () => {
      const willOpen = !get().developPanelOpen;
      set((state) => {
        state.developPanelOpen = willOpen;
      });
      const { selectedPhotoId } = get();
      if (willOpen && selectedPhotoId) {
        void get().loadDevelopStateForPhoto(selectedPhotoId);
      }
    },

    loadDevelopStateForPhoto: async (photoId) => {
      try {
        const position = await api.currentDevelopEdit(photoId);
        set((state) => {
          state.developBasic = basicFromHistoryPosition(position);
          state.developPhotoId = photoId;
        });
      } catch (err) {
        console.error("Bearbeitungszustand konnte nicht geladen werden:", err);
        set((state) => {
          state.developBasic = NEUTRAL_BASIC_ADJUSTMENTS;
          state.developPhotoId = photoId;
        });
      }
    },

    setBasicField: (key, value) => {
      set((state) => {
        writeBasicField(state.developBasic, key, value);
      });
    },

    commitDevelopEdit: async (label) => {
      const { developPhotoId, developBasic } = get();
      if (!developPhotoId) return;
      try {
        await api.applyDevelopEdit(developPhotoId, buildEdlEnvelopeJson(developBasic), label);
      } catch (err) {
        console.error("Bearbeitung konnte nicht gespeichert werden:", err);
      }
    },

    undoDevelop: async () => {
      const { developPhotoId } = get();
      if (!developPhotoId) return;
      const position = await api.undoDevelopEdit(developPhotoId).catch((err: unknown) => {
        console.error("Rückgängig fehlgeschlagen:", err);
        return null;
      });
      if (!position) return;
      set((state) => {
        state.developBasic = basicFromHistoryPosition(position);
      });
    },

    redoDevelop: async () => {
      const { developPhotoId } = get();
      if (!developPhotoId) return;
      const position = await api.redoDevelopEdit(developPhotoId).catch((err: unknown) => {
        console.error("Wiederholen fehlgeschlagen:", err);
        return null;
      });
      if (!position) return;
      set((state) => {
        state.developBasic = basicFromHistoryPosition(position);
      });
    },

    developLastLatencyMs: null,

    setDevelopLatencyMs: (ms) => {
      set((state) => {
        state.developLastLatencyMs = ms;
      });
    },

    // Library (ab Phase 3)
    centerView: "viewer",

    toggleCenterView: () => {
      set((state) => {
        state.centerView = state.centerView === "viewer" ? "grid" : "viewer";
      });
    },

    multiSelectedIds: [],

    togglePhotoSelection: (photoId, mode) => {
      if (mode === "toggle") {
        const wasSelected = get().multiSelectedIds.includes(photoId);
        set((state) => {
          state.multiSelectedIds = wasSelected
            ? state.multiSelectedIds.filter((id) => id !== photoId)
            : [...state.multiSelectedIds, photoId];
          state.selectedPhotoId = photoId;
        });
        get().resetView();
        if (get().developPanelOpen) void get().loadDevelopStateForPhoto(photoId);
        if (get().metadataPanelOpen) void get().loadKeywordsForPhoto(photoId);
        return;
      }
      if (mode === "range") {
        const photos = selectActivePhotos(get());
        const anchorId = get().selectedPhotoId;
        const anchorIndex = photos.findIndex((p) => p.id === anchorId);
        const targetIndex = photos.findIndex((p) => p.id === photoId);
        if (anchorIndex === -1 || targetIndex === -1) {
          get().selectPhoto(photoId);
          return;
        }
        const [start, end] = anchorIndex < targetIndex ? [anchorIndex, targetIndex] : [targetIndex, anchorIndex];
        const range = photos.slice(start, end + 1).map((p) => p.id);
        set((state) => {
          state.multiSelectedIds = range;
        });
        return;
      }
      get().selectPhoto(photoId);
    },

    metadataPanelOpen: false,

    toggleMetadataPanel: () => {
      const willOpen = !get().metadataPanelOpen;
      set((state) => {
        state.metadataPanelOpen = willOpen;
      });
      const { selectedPhotoId } = get();
      if (willOpen && selectedPhotoId) {
        void get().loadKeywordsForPhoto(selectedPhotoId);
      }
    },

    photoKeywords: {},

    loadKeywordsForPhoto: async (photoId) => {
      try {
        const keywords = await api.listPhotoKeywords(photoId);
        set((state) => {
          state.photoKeywords[photoId] = keywords;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    addKeywordToPhoto: async (photoId, name) => {
      const trimmed = name.trim();
      if (!trimmed) return;
      try {
        await api.addPhotoKeyword(photoId, trimmed);
        await get().loadKeywordsForPhoto(photoId);
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    removeKeywordFromPhoto: async (photoId, keywordId) => {
      try {
        await api.removePhotoKeyword(photoId, keywordId);
        await get().loadKeywordsForPhoto(photoId);
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    setPhotoRating: async (photoId, rating) => {
      const { multiSelectedIds } = get();
      const targets = multiSelectedIds.includes(photoId) && multiSelectedIds.length > 1 ? multiSelectedIds : [photoId];
      try {
        await Promise.all(targets.map((id) => api.setPhotoRating(id, rating)));
        set((state) => {
          for (const id of targets) patchPhotoEverywhere(state, id, { rating });
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    setPhotoFlag: async (photoId, flag) => {
      const { multiSelectedIds } = get();
      const targets = multiSelectedIds.includes(photoId) && multiSelectedIds.length > 1 ? multiSelectedIds : [photoId];
      try {
        await Promise.all(targets.map((id) => api.setPhotoFlag(id, flag)));
        set((state) => {
          for (const id of targets) patchPhotoEverywhere(state, id, { flag });
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    setPhotoColorLabel: async (photoId, colorLabel) => {
      const { multiSelectedIds } = get();
      const targets = multiSelectedIds.includes(photoId) && multiSelectedIds.length > 1 ? multiSelectedIds : [photoId];
      try {
        await Promise.all(targets.map((id) => api.setPhotoColorLabel(id, colorLabel)));
        set((state) => {
          for (const id of targets) patchPhotoEverywhere(state, id, { color_label: colorLabel });
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    collections: [],
    selectedCollectionId: null,
    collectionPhotos: {},

    refreshCollections: async () => {
      try {
        const collections = await api.listCollections();
        set((state) => {
          state.collections = collections;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    createCollection: async (name) => {
      const trimmed = name.trim();
      if (!trimmed) return;
      try {
        await api.createCollection(trimmed);
        await get().refreshCollections();
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    selectCollection: (collectionId) => {
      set((state) => {
        state.selectedCollectionId = collectionId;
        if (collectionId) state.selectedFolderId = null;
        state.selectedPhotoId = null;
        state.multiSelectedIds = [];
        state.libraryQuery = "";
        state.libraryResults = null;
      });
      if (collectionId) {
        void get().loadPhotosForCollection(collectionId);
      }
    },

    loadPhotosForCollection: async (collectionId) => {
      try {
        const photos = await api.listPhotosInCollection(collectionId);
        set((state) => {
          state.collectionPhotos[collectionId] = photos;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    addSelectionToCollection: async (collectionId) => {
      const { multiSelectedIds, selectedPhotoId } = get();
      const targets = multiSelectedIds.length > 0 ? multiSelectedIds : selectedPhotoId ? [selectedPhotoId] : [];
      if (targets.length === 0) return;
      try {
        await Promise.all(targets.map((id) => api.addToCollection(collectionId, id)));
        if (get().collectionPhotos[collectionId]) {
          await get().loadPhotosForCollection(collectionId);
        }
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    libraryQuery: "",
    libraryFilter: {},
    libraryResults: null,

    setLibraryQuery: (query) => {
      set((state) => {
        state.libraryQuery = query;
      });
    },

    runLibrarySearch: async () => {
      const trimmed = get().libraryQuery.trim();
      if (!trimmed) {
        set((state) => {
          state.libraryResults = null;
        });
        return;
      }
      try {
        const results = await api.searchPhotos(trimmed);
        set((state) => {
          state.libraryFilter = {};
          state.libraryResults = results;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    setLibraryFilterChip: async (patch) => {
      const nextFilter: FilterCriteriaDto = { ...get().libraryFilter, ...patch };
      // Ein `undefined`-Wert im Patch soll das Attribut wieder entfernen
      // (Chip abwählen), nicht das Feld auf `undefined` überschreiben.
      for (const key of Object.keys(patch) as (keyof FilterCriteriaDto)[]) {
        if (patch[key] === undefined) delete nextFilter[key];
      }
      const hasAnyFilter = Object.keys(nextFilter).length > 0;
      set((state) => {
        state.libraryFilter = nextFilter;
        state.libraryQuery = "";
      });
      if (!hasAnyFilter) {
        set((state) => {
          state.libraryResults = null;
        });
        return;
      }
      try {
        const results = await api.filterPhotos(nextFilter);
        set((state) => {
          state.libraryResults = results;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    clearLibraryFilters: () => {
      set((state) => {
        state.libraryQuery = "";
        state.libraryFilter = {};
        state.libraryResults = null;
      });
    },
  })),
);

// Store im Debug-Build am `window` verfügbar machen — üblich für
// Zustand-Projekte, praktisch zum manuellen Nachstellen von Zuständen in
// der Browser-Konsole (z. B. große Foto-Listen zum Testen der
// Filmstreifen-Virtualisierung). `import.meta.env.DEV` wird von Vite zur
// Build-Zeit ausgewertet — im Produktions-Build entfällt der Codepfad
// komplett (Dead-Code-Elimination), landet also nicht im ausgelieferten
// Bundle.
if (import.meta.env.DEV) {
  (window as unknown as { __appStore: typeof useAppStore }).__appStore = useAppStore;
}
