import { create } from "zustand";
import { immer } from "zustand/middleware/immer";

import * as api from "../lib/tauri";
import type { CatalogStatusDto, FolderDto, PhotoDto } from "../lib/tauri";

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

type AppStore = CatalogSlice & SelectionSlice & ViewerSlice & JobsSlice;

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

    // Selection
    selectedFolderId: null,
    selectedPhotoId: null,

    selectFolder: (folderId) => {
      set((state) => {
        state.selectedFolderId = folderId;
        state.selectedPhotoId = null;
      });
      if (folderId) {
        void get().loadPhotosForFolder(folderId);
      }
    },

    selectPhoto: (photoId) => {
      set((state) => {
        state.selectedPhotoId = photoId;
      });
      get().resetView();
    },

    stepSelection: (direction) => {
      const { selectedFolderId, selectedPhotoId, photosByFolder } = get();
      if (!selectedFolderId) return;
      const photos = photosByFolder[selectedFolderId];
      if (!photos || photos.length === 0) return;

      const currentIndex = photos.findIndex((p) => p.id === selectedPhotoId);
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
