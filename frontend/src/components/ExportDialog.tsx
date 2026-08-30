import { useState } from "react";

import type { ExportFormat, ExportPhotoOptions } from "../lib/tauri";
import { selectFolderDialog } from "../lib/tauri";
import { useAppStore } from "../store";

interface ExportDialogProps {
  open: boolean;
  photoIds: string[];
  onClose: () => void;
}

const FORMAT_LABELS: Record<ExportFormat, string> = {
  jpeg: "JPEG",
  png: "PNG",
  tiff: "TIFF",
  webp: "WebP (verlustfrei)",
  avif: "AVIF",
};

type SizeMode = "original" | "edge" | "megapixels";

/**
 * Export-Exportdialog-Grundgerüst (Phase 8 Schritt 1, siehe `DECISIONS.md`
 * ADR-0034 und `apx_export::engine`s Moduldoku). Exportiert `photoIds` mit
 * ihrem jeweils aktuellen Bearbeitungsstand nach einem gewählten Zielordner.
 * Farbmanagement/Wasserzeichen/Metadaten-Filter/echte Warteschlange folgen
 * in Schritt 2.
 */
export function ExportDialog({ open, photoIds, onClose }: ExportDialogProps) {
  const exportRunning = useAppStore((s) => s.exportRunning);
  const exportProgress = useAppStore((s) => s.exportProgress);
  const exportError = useAppStore((s) => s.exportError);
  const exportLastOutcomes = useAppStore((s) => s.exportLastOutcomes);
  const exportPhotos = useAppStore((s) => s.exportPhotos);

  const [destFolder, setDestFolder] = useState("");
  const [format, setFormat] = useState<ExportFormat>("jpeg");
  const [quality, setQuality] = useState(90);
  const [bitDepth16, setBitDepth16] = useState(false);
  const [sizeMode, setSizeMode] = useState<SizeMode>("original");
  const [maxEdge, setMaxEdge] = useState(2048);
  const [maxMegapixels, setMaxMegapixels] = useState(12);
  const [limitFileSize, setLimitFileSize] = useState(false);
  const [maxFileSizeKb, setMaxFileSizeKb] = useState(500);
  const [sharpenAmount, setSharpenAmount] = useState(0);

  if (!open) return null;

  async function handlePickDestFolder() {
    const path = await selectFolderDialog();
    if (path) setDestFolder(path);
  }

  async function handleExport() {
    if (!destFolder || photoIds.length === 0) return;
    const options: ExportPhotoOptions = {
      format,
      quality,
      bitDepth16: bitDepth16 && (format === "png" || format === "tiff"),
      sharpenAmount: sharpenAmount > 0 ? sharpenAmount : undefined,
    };
    if (sizeMode === "edge") options.maxEdge = maxEdge;
    if (sizeMode === "megapixels") options.maxMegapixels = maxMegapixels;
    if (format === "jpeg" && limitFileSize) options.maxFileSizeBytes = maxFileSizeKb * 1024;
    await exportPhotos(photoIds, destFolder, options);
  }

  const supportsBitDepth16 = format === "png" || format === "tiff";
  const supportsQuality = format === "jpeg" || format === "avif";

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-16" onClick={onClose}>
      <div
        onClick={(e) => e.stopPropagation()}
        className="max-h-[85vh] w-full max-w-md overflow-y-auto rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
      >
        <h2 className="mb-1 text-sm font-semibold text-text-primary">Exportieren</h2>
        <p className="mb-3 text-xs text-text-muted">
          {photoIds.length} Foto{photoIds.length === 1 ? "" : "s"} mit aktuellem Bearbeitungsstand
        </p>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          Zielordner
          <div className="flex gap-1">
            <input
              type="text"
              readOnly
              value={destFolder}
              placeholder="Ordner wählen…"
              className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-sm"
            />
            <button
              type="button"
              onClick={() => void handlePickDestFolder()}
              className="shrink-0 rounded border border-border px-2 py-1 text-xs hover:border-accent"
            >
              Wählen…
            </button>
          </div>
        </label>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          Format
          <select
            value={format}
            onChange={(e) => setFormat(e.target.value as ExportFormat)}
            className="rounded border border-border bg-bg-panel px-2 py-1 text-sm"
          >
            {(Object.keys(FORMAT_LABELS) as ExportFormat[]).map((f) => (
              <option key={f} value={f}>
                {FORMAT_LABELS[f]}
              </option>
            ))}
          </select>
        </label>

        {supportsQuality && (
          <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
            Qualität ({quality})
            <input
              type="range"
              min={1}
              max={100}
              value={quality}
              onChange={(e) => setQuality(Number(e.target.value))}
            />
          </label>
        )}

        {supportsBitDepth16 && (
          <label className="mb-3 flex items-center gap-2 text-xs text-text-secondary">
            <input type="checkbox" checked={bitDepth16} onChange={(e) => setBitDepth16(e.target.checked)} />
            16-Bit-Ausgabe (Dateiformat-Kompatibilität, keine echte Präzisionssteigerung — siehe ADR-0034)
          </label>
        )}

        <fieldset className="mb-3 flex flex-col gap-1">
          <legend className="mb-1 text-xs font-medium text-text-secondary">Größenbegrenzung</legend>
          <label className="flex items-center gap-2 text-xs">
            <input type="radio" checked={sizeMode === "original"} onChange={() => setSizeMode("original")} />
            Originalgröße
          </label>
          <label className="flex items-center gap-2 text-xs">
            <input type="radio" checked={sizeMode === "edge"} onChange={() => setSizeMode("edge")} />
            Längere Kante höchstens
            <input
              type="number"
              min={1}
              value={maxEdge}
              onChange={(e) => setMaxEdge(Number(e.target.value))}
              disabled={sizeMode !== "edge"}
              className="w-20 rounded border border-border bg-bg-panel px-1 py-0.5 disabled:opacity-50"
            />
            px
          </label>
          <label className="flex items-center gap-2 text-xs">
            <input type="radio" checked={sizeMode === "megapixels"} onChange={() => setSizeMode("megapixels")} />
            Höchstens
            <input
              type="number"
              min={0.1}
              step={0.1}
              value={maxMegapixels}
              onChange={(e) => setMaxMegapixels(Number(e.target.value))}
              disabled={sizeMode !== "megapixels"}
              className="w-20 rounded border border-border bg-bg-panel px-1 py-0.5 disabled:opacity-50"
            />
            Megapixel
          </label>
        </fieldset>

        {format === "jpeg" && (
          <label className="mb-3 flex items-center gap-2 text-xs text-text-secondary">
            <input type="checkbox" checked={limitFileSize} onChange={(e) => setLimitFileSize(e.target.checked)} />
            Ziel-Dateigröße höchstens
            <input
              type="number"
              min={1}
              value={maxFileSizeKb}
              onChange={(e) => setMaxFileSizeKb(Number(e.target.value))}
              disabled={!limitFileSize}
              className="w-20 rounded border border-border bg-bg-panel px-1 py-0.5 disabled:opacity-50"
            />
            KB (ersetzt die Qualitätsstufe oben)
          </label>
        )}

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          Ausgabeschärfung ({sharpenAmount === 0 ? "aus" : sharpenAmount.toFixed(1)})
          <input
            type="range"
            min={0}
            max={2}
            step={0.1}
            value={sharpenAmount}
            onChange={(e) => setSharpenAmount(Number(e.target.value))}
          />
        </label>

        {exportProgress && (
          <p className="mb-2 text-xs text-text-secondary">
            {exportProgress.done} / {exportProgress.total} exportiert
          </p>
        )}
        {exportError && <p className="mb-2 text-xs text-danger">Fehler: {exportError}</p>}
        {!exportRunning && exportLastOutcomes.length > 0 && (
          <p className="mb-2 text-xs text-text-secondary">{exportLastOutcomes.length} Datei(en) geschrieben.</p>
        )}

        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded border border-border px-3 py-1 text-xs hover:border-accent">
            Schließen
          </button>
          <button
            type="button"
            onClick={() => void handleExport()}
            disabled={!destFolder || photoIds.length === 0 || exportRunning}
            className="rounded border border-accent bg-accent/10 px-3 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
          >
            {exportRunning ? "Exportiere…" : "Exportieren"}
          </button>
        </div>
      </div>
    </div>
  );
}
