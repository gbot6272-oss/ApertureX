import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface CatalogStatus {
  catalog_path: string;
  folder_count: number;
  photo_count: number;
}

interface ImportProgress {
  done: number;
  total: number;
  current_file: string | null;
}

interface ImportFinished {
  imported: number;
  skipped: number;
  error_count: number;
  cancelled: boolean;
}

/**
 * Minimales Gerüst für die Schritte 5+6 (Tauri-Shell, Import-Job): beweist,
 * dass Frontend, Tauri-Commands, Events und apx-catalog/apx-raw korrekt
 * verdrahtet sind — inklusive Fortschrittsanzeige und Abbruch. Das
 * eigentliche Layout (Kopfzeile, Ordnerbaum, Viewer, Filmstreifen) entsteht
 * in Schritt 8 ("Frontend-Gerüst") und ersetzt diese Komponente.
 */
export default function App() {
  const [status, setStatus] = useState<CatalogStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<ImportProgress | null>(null);
  const [lastResult, setLastResult] = useState<ImportFinished | null>(null);
  const [importErrors, setImportErrors] = useState<string[]>([]);

  const refreshStatus = useCallback(() => {
    invoke<CatalogStatus>("catalog_status")
      .then(setStatus)
      .catch((err: unknown) => setError(String(err)));
  }, []);

  useEffect(() => {
    refreshStatus();
  }, [refreshStatus]);

  useEffect(() => {
    const unlistenProgress = listen<ImportProgress>("import:progress", (event) => {
      setProgress(event.payload);
    });
    const unlistenError = listen<{ file: string; message: string }>("import:error", (event) => {
      setImportErrors((prev) => [...prev, `${event.payload.file}: ${event.payload.message}`]);
    });
    const unlistenFinished = listen<ImportFinished>("import:finished", (event) => {
      setLastResult(event.payload);
      setProgress(null);
      refreshStatus();
    });

    return () => {
      unlistenProgress.then((fn) => fn());
      unlistenError.then((fn) => fn());
      unlistenFinished.then((fn) => fn());
    };
  }, [refreshStatus]);

  const handleSelectFolder = useCallback(() => {
    invoke<string | null>("select_folder")
      .then((path) => {
        if (!path) {
          return;
        }
        setImportErrors([]);
        setLastResult(null);
        return invoke("import_folder", { path });
      })
      .catch((err: unknown) => setError(String(err)));
  }, []);

  const handleCancel = useCallback(() => {
    invoke("cancel_import").catch((err: unknown) => setError(String(err)));
  }, []);

  return (
    <main
      style={{
        minHeight: "100vh",
        background: "#1a1a1a",
        color: "#e8e4dc",
        fontFamily: "system-ui, sans-serif",
        padding: "2rem",
      }}
    >
      <h1 style={{ fontWeight: 600 }}>Aperture X</h1>
      <p style={{ color: "#9a9a9a" }}>
        Phase 1 — Fundament. Vollständiges Layout folgt in Schritt 8.
      </p>

      <div style={{ display: "flex", gap: "0.75rem", marginTop: "1rem" }}>
        <button
          type="button"
          onClick={handleSelectFolder}
          disabled={progress !== null}
          style={{
            padding: "0.5rem 1rem",
            background: "#2a2a2a",
            color: "#e8e4dc",
            border: "1px solid #3a3a3a",
            borderRadius: "4px",
            cursor: progress ? "not-allowed" : "pointer",
            opacity: progress ? 0.6 : 1,
          }}
        >
          Ordner importieren
        </button>
        {progress && (
          <button
            type="button"
            onClick={handleCancel}
            style={{
              padding: "0.5rem 1rem",
              background: "#3a2020",
              color: "#e8e4dc",
              border: "1px solid #5a2f2f",
              borderRadius: "4px",
              cursor: "pointer",
            }}
          >
            Abbrechen
          </button>
        )}
      </div>

      {error && (
        <p style={{ color: "#e07a5f", marginTop: "1rem" }}>Fehler: {error}</p>
      )}

      {progress && (
        <div style={{ marginTop: "1.5rem" }}>
          <div
            style={{
              background: "#2a2a2a",
              borderRadius: "4px",
              overflow: "hidden",
              height: "8px",
              width: "min(480px, 100%)",
            }}
          >
            <div
              style={{
                background: "#e8e4dc",
                height: "100%",
                width: progress.total > 0 ? `${Math.round((progress.done / progress.total) * 100)}%` : "0%",
                transition: "width 120ms linear",
              }}
            />
          </div>
          <p style={{ color: "#9a9a9a", marginTop: "0.5rem" }}>
            {progress.done} / {progress.total}
            {progress.current_file ? ` — ${progress.current_file}` : ""}
          </p>
        </div>
      )}

      {lastResult && (
        <p style={{ marginTop: "1rem" }}>
          Import {lastResult.cancelled ? "abgebrochen" : "abgeschlossen"}: {lastResult.imported} importiert,{" "}
          {lastResult.skipped} übersprungen, {lastResult.error_count} Fehler.
        </p>
      )}

      {importErrors.length > 0 && (
        <ul style={{ color: "#e07a5f", marginTop: "0.5rem" }}>
          {importErrors.map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
      )}

      {status && (
        <dl style={{ marginTop: "1.5rem", lineHeight: 1.8 }}>
          <dt style={{ color: "#9a9a9a" }}>Katalog</dt>
          <dd>{status.catalog_path}</dd>
          <dt style={{ color: "#9a9a9a" }}>Ordner</dt>
          <dd>{status.folder_count}</dd>
          <dt style={{ color: "#9a9a9a" }}>Fotos</dt>
          <dd>{status.photo_count}</dd>
        </dl>
      )}
    </main>
  );
}
