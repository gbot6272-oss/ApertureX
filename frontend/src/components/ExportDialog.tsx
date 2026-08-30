import { useState } from "react";

import type { ExportFormat, ExportPhotoOptions, IccProfileChoice, WatermarkPosition } from "../lib/tauri";
import { pickFilePath, selectFolderDialog } from "../lib/tauri";
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

const ICC_LABELS: Record<IccProfileChoice, string> = {
  srgb: "sRGB (keine Umwandlung)",
  adobe_rgb: "Adobe RGB",
  pro_photo_rgb: "ProPhoto RGB",
  display_p3: "Display P3",
  custom: "Eigenes ICC-Profil…",
};

const POSITION_LABELS: Record<WatermarkPosition, string> = {
  top_left: "oben links",
  top_right: "oben rechts",
  bottom_left: "unten links",
  bottom_right: "unten rechts",
  center: "Mitte",
};

type SizeMode = "original" | "edge" | "megapixels";
type WatermarkMode = "none" | "text" | "image";

/**
 * Export-Exportdialog (Phase 8 Schritt 1+2, siehe `DECISIONS.md` ADR-0034
 * und `apx_export::engine`s Moduldoku). Exportiert `photoIds` mit ihrem
 * jeweils aktuellen Bearbeitungsstand nach einem gewählten Zielordner über
 * die Backend-Export-Warteschlange (Fortschritt/Pausieren, siehe
 * `store/index.ts`s `ExportSlice`).
 */
export function ExportDialog({ open, photoIds, onClose }: ExportDialogProps) {
  const exportRunning = useAppStore((s) => s.exportRunning);
  const exportProgress = useAppStore((s) => s.exportProgress);
  const exportError = useAppStore((s) => s.exportError);
  const exportQueuePaused = useAppStore((s) => s.exportQueuePaused);
  const exportPhotos = useAppStore((s) => s.exportPhotos);
  const toggleExportQueuePause = useAppStore((s) => s.toggleExportQueuePause);

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
  const [iccProfile, setIccProfile] = useState<IccProfileChoice>("srgb");
  const [iccProfilePath, setIccProfilePath] = useState("");
  const [watermarkMode, setWatermarkMode] = useState<WatermarkMode>("none");
  const [watermarkText, setWatermarkText] = useState("");
  const [watermarkFontPath, setWatermarkFontPath] = useState("");
  const [watermarkImagePath, setWatermarkImagePath] = useState("");
  const [watermarkPosition, setWatermarkPosition] = useState<WatermarkPosition>("bottom_right");
  const [watermarkOpacity, setWatermarkOpacity] = useState(0.7);
  const [metadataMake, setMetadataMake] = useState("");
  const [metadataCopyright, setMetadataCopyright] = useState("");

  if (!open) return null;

  async function handlePickDestFolder() {
    const path = await selectFolderDialog();
    if (path) setDestFolder(path);
  }

  async function handlePickIccFile() {
    const path = await pickFilePath("ICC-Profile", ["icc", "icm"]);
    if (path) setIccProfilePath(path);
  }

  async function handlePickFontFile() {
    const path = await pickFilePath("Schriftdateien", ["ttf", "otf"]);
    if (path) setWatermarkFontPath(path);
  }

  async function handlePickWatermarkImage() {
    const path = await pickFilePath("Bilder", ["png", "jpg", "jpeg", "webp"]);
    if (path) setWatermarkImagePath(path);
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

    if (iccProfile !== "srgb") {
      options.iccProfile = iccProfile;
      if (iccProfile === "custom") options.iccProfilePath = iccProfilePath;
    }

    if (watermarkMode === "text" && watermarkText && watermarkFontPath) {
      options.watermarkText = watermarkText;
      options.watermarkFontPath = watermarkFontPath;
      options.watermarkPosition = watermarkPosition;
      options.watermarkOpacity = watermarkOpacity;
    } else if (watermarkMode === "image" && watermarkImagePath) {
      options.watermarkImagePath = watermarkImagePath;
      options.watermarkPosition = watermarkPosition;
      options.watermarkOpacity = watermarkOpacity;
    }

    if (format === "jpeg") {
      if (metadataMake) options.metadataMake = metadataMake;
      if (metadataCopyright) options.metadataCopyright = metadataCopyright;
    }

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

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          Farbraum (ICC)
          <select
            value={iccProfile}
            onChange={(e) => setIccProfile(e.target.value as IccProfileChoice)}
            className="rounded border border-border bg-bg-panel px-2 py-1 text-sm"
          >
            {(Object.keys(ICC_LABELS) as IccProfileChoice[]).map((p) => (
              <option key={p} value={p}>
                {ICC_LABELS[p]}
              </option>
            ))}
          </select>
        </label>
        {iccProfile === "custom" && (
          <div className="mb-3 flex gap-1">
            <input
              type="text"
              readOnly
              value={iccProfilePath}
              placeholder="ICC-Datei wählen…"
              className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
            />
            <button type="button" onClick={() => void handlePickIccFile()} className="shrink-0 rounded border border-border px-2 py-1 text-xs hover:border-accent">
              Wählen…
            </button>
          </div>
        )}

        <fieldset className="mb-3 flex flex-col gap-1">
          <legend className="mb-1 text-xs font-medium text-text-secondary">Wasserzeichen</legend>
          <label className="mb-1 flex flex-col gap-1 text-xs text-text-secondary">
            Wasserzeichen-Art
            <select
              value={watermarkMode}
              onChange={(e) => setWatermarkMode(e.target.value as WatermarkMode)}
              className="rounded border border-border bg-bg-panel px-2 py-1 text-xs"
            >
              <option value="none">Kein Wasserzeichen</option>
              <option value="text">Text</option>
              <option value="image">Bild</option>
            </select>
          </label>
          {watermarkMode === "text" && (
            <>
              <input
                type="text"
                value={watermarkText}
                onChange={(e) => setWatermarkText(e.target.value)}
                placeholder="Text"
                className="mb-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
              />
              <div className="mb-1 flex gap-1">
                <input
                  type="text"
                  readOnly
                  value={watermarkFontPath}
                  placeholder="Schriftdatei wählen…"
                  className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
                />
                <button type="button" onClick={() => void handlePickFontFile()} className="shrink-0 rounded border border-border px-2 py-1 text-xs hover:border-accent">
                  Wählen…
                </button>
              </div>
            </>
          )}
          {watermarkMode === "image" && (
            <div className="mb-1 flex gap-1">
              <input
                type="text"
                readOnly
                value={watermarkImagePath}
                placeholder="Bilddatei wählen…"
                className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
              />
              <button type="button" onClick={() => void handlePickWatermarkImage()} className="shrink-0 rounded border border-border px-2 py-1 text-xs hover:border-accent">
                Wählen…
              </button>
            </div>
          )}
          {watermarkMode !== "none" && (
            <>
              <select
                value={watermarkPosition}
                onChange={(e) => setWatermarkPosition(e.target.value as WatermarkPosition)}
                className="mb-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
              >
                {(Object.keys(POSITION_LABELS) as WatermarkPosition[]).map((p) => (
                  <option key={p} value={p}>
                    {POSITION_LABELS[p]}
                  </option>
                ))}
              </select>
              <label className="flex items-center gap-2 text-xs">
                Deckkraft ({Math.round(watermarkOpacity * 100)}%)
                <input type="range" min={0} max={1} step={0.05} value={watermarkOpacity} onChange={(e) => setWatermarkOpacity(Number(e.target.value))} />
              </label>
            </>
          )}
        </fieldset>

        {format === "jpeg" && (
          <fieldset className="mb-3 flex flex-col gap-1">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Metadaten (nur JPEG)</legend>
            <input
              type="text"
              value={metadataMake}
              onChange={(e) => setMetadataMake(e.target.value)}
              placeholder="Hersteller"
              className="mb-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
            />
            <input
              type="text"
              value={metadataCopyright}
              onChange={(e) => setMetadataCopyright(e.target.value)}
              placeholder="Copyright"
              className="rounded border border-border bg-bg-panel px-2 py-1 text-xs"
            />
          </fieldset>
        )}

        {exportProgress && (
          <div className="mb-2 flex items-center gap-2 text-xs text-text-secondary">
            <span>
              {exportProgress.done} / {exportProgress.total} exportiert
              {exportProgress.failed > 0 ? ` (${exportProgress.failed} fehlgeschlagen)` : ""}
            </span>
            {exportRunning && (
              <button type="button" onClick={() => void toggleExportQueuePause()} className="rounded border border-border px-2 py-0.5 text-xs hover:border-accent">
                {exportQueuePaused ? "Fortsetzen" : "Pausieren"}
              </button>
            )}
          </div>
        )}
        {exportError && <p className="mb-2 text-xs text-danger">Fehler: {exportError}</p>}
        {!exportRunning && exportProgress && exportProgress.done > 0 && (
          <p className="mb-2 text-xs text-text-secondary">{exportProgress.done - exportProgress.failed} Datei(en) geschrieben.</p>
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
