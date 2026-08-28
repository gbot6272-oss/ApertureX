import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface CatalogStatus {
  catalog_path: string;
  folder_count: number;
  photo_count: number;
}

/**
 * Minimales Gerüst für Schritt 5 (Tauri-Shell): beweist, dass Frontend,
 * Tauri-Commands und apx-catalog korrekt verdrahtet sind. Das eigentliche
 * Layout (Kopfzeile, Ordnerbaum, Viewer, Filmstreifen) entsteht in
 * Schritt 8 ("Frontend-Gerüst") und ersetzt diese Komponente.
 */
export default function App() {
  const [status, setStatus] = useState<CatalogStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refreshStatus = useCallback(() => {
    invoke<CatalogStatus>("catalog_status")
      .then(setStatus)
      .catch((err: unknown) => setError(String(err)));
  }, []);

  useEffect(() => {
    refreshStatus();
  }, [refreshStatus]);

  const handleSelectFolder = useCallback(() => {
    invoke<string | null>("select_folder")
      .then((path) => {
        if (path) {
          // Der eigentliche Import-Aufruf kommt in Schritt 6 dazu.
          console.info("Ordner ausgewählt:", path);
        }
      })
      .catch((err: unknown) => setError(String(err)));
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

      <button
        type="button"
        onClick={handleSelectFolder}
        style={{
          marginTop: "1rem",
          padding: "0.5rem 1rem",
          background: "#2a2a2a",
          color: "#e8e4dc",
          border: "1px solid #3a3a3a",
          borderRadius: "4px",
          cursor: "pointer",
        }}
      >
        Ordner importieren
      </button>

      {error && (
        <p style={{ color: "#e07a5f", marginTop: "1rem" }}>Fehler: {error}</p>
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
