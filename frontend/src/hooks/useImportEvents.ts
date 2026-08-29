import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

import { useAppStore } from "../store";

interface ImportProgressEvent {
  done: number;
  total: number;
  current_file: string | null;
}

interface ImportErrorEvent {
  file: string;
  message: string;
}

interface ImportFinishedEvent {
  imported: number;
  skipped: number;
  error_count: number;
  cancelled: boolean;
}

/** Verbindet die Tauri-IPC-Events des Import-Jobs (siehe
 * `crates/apx-app/src/import/mod.rs`) mit dem `jobs`-Slice. */
export function useImportEvents(): void {
  const setImportProgress = useAppStore((s) => s.setImportProgress);
  const addImportError = useAppStore((s) => s.addImportError);
  const finishImport = useAppStore((s) => s.finishImport);

  useEffect(() => {
    const unlistenProgress = listen<ImportProgressEvent>("import:progress", (event) => {
      setImportProgress({
        done: event.payload.done,
        total: event.payload.total,
        currentFile: event.payload.current_file,
      });
    });
    const unlistenError = listen<ImportErrorEvent>("import:error", (event) => {
      addImportError(`${event.payload.file}: ${event.payload.message}`);
    });
    const unlistenFinished = listen<ImportFinishedEvent>("import:finished", (event) => {
      finishImport({
        imported: event.payload.imported,
        skipped: event.payload.skipped,
        errorCount: event.payload.error_count,
        cancelled: event.payload.cancelled,
      });
    });

    return () => {
      void unlistenProgress.then((fn) => fn());
      void unlistenError.then((fn) => fn());
      void unlistenFinished.then((fn) => fn());
    };
  }, [setImportProgress, addImportError, finishImport]);
}
