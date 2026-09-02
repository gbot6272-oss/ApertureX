import { useState } from "react";

import { useT } from "../lib/i18n";
import type { ExportFormat, ExportPhotoOptions, IccProfileChoice, WatermarkPosition } from "../lib/tauri";
import { pickFilePath, selectFolderDialog } from "../lib/tauri";
import { useAppStore } from "../store";

interface ExportDialogProps {
  open: boolean;
  photoIds: string[];
  onClose: () => void;
}

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
  const t = useT();
  const exportRunning = useAppStore((s) => s.exportRunning);
  const exportProgress = useAppStore((s) => s.exportProgress);
  const exportError = useAppStore((s) => s.exportError);
  const exportQueuePaused = useAppStore((s) => s.exportQueuePaused);
  const exportPhotos = useAppStore((s) => s.exportPhotos);
  const toggleExportQueuePause = useAppStore((s) => s.toggleExportQueuePause);

  const FORMAT_LABELS: Record<ExportFormat, string> = {
    jpeg: "JPEG",
    png: "PNG",
    tiff: "TIFF",
    webp: t("exportDialog.formatWebp"),
    avif: "AVIF",
    psd: t("exportDialog.formatPsd"),
    jxl: "JPEG XL",
  };

  const ICC_LABELS: Record<IccProfileChoice, string> = {
    srgb: t("exportDialog.iccSrgb"),
    adobe_rgb: "Adobe RGB",
    pro_photo_rgb: "ProPhoto RGB",
    display_p3: "Display P3",
    custom: t("exportDialog.iccCustom"),
  };

  const POSITION_LABELS: Record<WatermarkPosition, string> = {
    top_left: t("exportDialog.positionTopLeft"),
    top_right: t("exportDialog.positionTopRight"),
    bottom_left: t("exportDialog.positionBottomLeft"),
    bottom_right: t("exportDialog.positionBottomRight"),
    center: t("exportDialog.positionCenter"),
  };

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
    const path = await pickFilePath(t("exportDialog.iccFilterName"), ["icc", "icm"]);
    if (path) setIccProfilePath(path);
  }

  async function handlePickFontFile() {
    const path = await pickFilePath(t("exportDialog.fontFilterName"), ["ttf", "otf"]);
    if (path) setWatermarkFontPath(path);
  }

  async function handlePickWatermarkImage() {
    const path = await pickFilePath(t("exportDialog.imageFilterName"), ["png", "jpg", "jpeg", "webp"]);
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
  // JPEG-XL: Qualität 100 kodiert verlustfrei, darunter verlustbehaftet
  // (siehe `apx_export::format::encode_jxl`s Moduldoku) — derselbe
  // 1-100-Regler wie bei JPEG/AVIF steuert also auch hier sinnvoll etwas.
  const supportsQuality = format === "jpeg" || format === "avif" || format === "jxl";

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-16" onClick={onClose}>
      <div
        onClick={(e) => e.stopPropagation()}
        className="max-h-[85vh] w-full max-w-md overflow-y-auto rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
      >
        <h2 className="mb-1 text-sm font-semibold text-text-primary">{t("exportDialog.title")}</h2>
        <p className="mb-3 text-xs text-text-muted">
          {t("exportDialog.photoCount", { count: photoIds.length, plural: photoIds.length === 1 ? "" : "s" })}
        </p>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          {t("exportDialog.destFolder")}
          <div className="flex gap-1">
            <input
              type="text"
              readOnly
              value={destFolder}
              placeholder={t("exportDialog.chooseFolder")}
              className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-sm"
            />
            <button
              type="button"
              onClick={() => void handlePickDestFolder()}
              className="shrink-0 rounded border border-border px-2 py-1 text-xs hover:border-accent"
            >
              {t("exportDialog.choose")}
            </button>
          </div>
        </label>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          {t("exportDialog.format")}
          <select
            value={format}
            onChange={(e) => setFormat(e.target.value as ExportFormat)}
            className="rounded border border-border bg-bg-panel px-2 py-1 text-sm"
          >
            {(Object.keys(FORMAT_LABELS) as ExportFormat[]).map((key) => (
              <option key={key} value={key}>
                {FORMAT_LABELS[key]}
              </option>
            ))}
          </select>
        </label>

        {supportsQuality && (
          <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
            {t("exportDialog.quality", { value: quality })}
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
            {t("exportDialog.bitDepth16")}
          </label>
        )}

        <fieldset className="mb-3 flex flex-col gap-1">
          <legend className="mb-1 text-xs font-medium text-text-secondary">{t("exportDialog.sizeLimit")}</legend>
          <label className="flex items-center gap-2 text-xs">
            <input type="radio" checked={sizeMode === "original"} onChange={() => setSizeMode("original")} />
            {t("exportDialog.originalSize")}
          </label>
          <label className="flex items-center gap-2 text-xs">
            <input type="radio" checked={sizeMode === "edge"} onChange={() => setSizeMode("edge")} />
            {t("exportDialog.longerEdgeAtMost")}
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
            {t("exportDialog.atMost")}
            <input
              type="number"
              min={0.1}
              step={0.1}
              value={maxMegapixels}
              onChange={(e) => setMaxMegapixels(Number(e.target.value))}
              disabled={sizeMode !== "megapixels"}
              className="w-20 rounded border border-border bg-bg-panel px-1 py-0.5 disabled:opacity-50"
            />
            {t("exportDialog.megapixels")}
          </label>
        </fieldset>

        {format === "jpeg" && (
          <label className="mb-3 flex items-center gap-2 text-xs text-text-secondary">
            <input type="checkbox" checked={limitFileSize} onChange={(e) => setLimitFileSize(e.target.checked)} />
            {t("exportDialog.targetFileSizeAtMost")}
            <input
              type="number"
              min={1}
              value={maxFileSizeKb}
              onChange={(e) => setMaxFileSizeKb(Number(e.target.value))}
              disabled={!limitFileSize}
              className="w-20 rounded border border-border bg-bg-panel px-1 py-0.5 disabled:opacity-50"
            />
            {t("exportDialog.kbOverridesQuality")}
          </label>
        )}

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          {t("exportDialog.outputSharpening", { value: sharpenAmount === 0 ? t("exportDialog.off") : sharpenAmount.toFixed(1) })}
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
          {t("exportDialog.colorSpace")}
          <select
            value={iccProfile}
            onChange={(e) => setIccProfile(e.target.value as IccProfileChoice)}
            className="rounded border border-border bg-bg-panel px-2 py-1 text-sm"
          >
            {(Object.keys(ICC_LABELS) as IccProfileChoice[]).map((key) => (
              <option key={key} value={key}>
                {ICC_LABELS[key]}
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
              placeholder={t("exportDialog.chooseIccFile")}
              className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
            />
            <button type="button" onClick={() => void handlePickIccFile()} className="shrink-0 rounded border border-border px-2 py-1 text-xs hover:border-accent">
              {t("exportDialog.choose")}
            </button>
          </div>
        )}

        <fieldset className="mb-3 flex flex-col gap-1">
          <legend className="mb-1 text-xs font-medium text-text-secondary">{t("exportDialog.watermark")}</legend>
          <label className="mb-1 flex flex-col gap-1 text-xs text-text-secondary">
            {t("exportDialog.watermarkKind")}
            <select
              value={watermarkMode}
              onChange={(e) => setWatermarkMode(e.target.value as WatermarkMode)}
              className="rounded border border-border bg-bg-panel px-2 py-1 text-xs"
            >
              <option value="none">{t("exportDialog.watermarkNone")}</option>
              <option value="text">{t("exportDialog.watermarkText")}</option>
              <option value="image">{t("exportDialog.watermarkImage")}</option>
            </select>
          </label>
          {watermarkMode === "text" && (
            <>
              <input
                type="text"
                value={watermarkText}
                onChange={(e) => setWatermarkText(e.target.value)}
                placeholder={t("exportDialog.text")}
                className="mb-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
              />
              <div className="mb-1 flex gap-1">
                <input
                  type="text"
                  readOnly
                  value={watermarkFontPath}
                  placeholder={t("exportDialog.chooseFontFile")}
                  className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
                />
                <button type="button" onClick={() => void handlePickFontFile()} className="shrink-0 rounded border border-border px-2 py-1 text-xs hover:border-accent">
                  {t("exportDialog.choose")}
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
                placeholder={t("exportDialog.chooseImageFile")}
                className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
              />
              <button type="button" onClick={() => void handlePickWatermarkImage()} className="shrink-0 rounded border border-border px-2 py-1 text-xs hover:border-accent">
                {t("exportDialog.choose")}
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
                {(Object.keys(POSITION_LABELS) as WatermarkPosition[]).map((key) => (
                  <option key={key} value={key}>
                    {POSITION_LABELS[key]}
                  </option>
                ))}
              </select>
              <label className="flex items-center gap-2 text-xs">
                {t("exportDialog.opacity", { percent: Math.round(watermarkOpacity * 100) })}
                <input type="range" min={0} max={1} step={0.05} value={watermarkOpacity} onChange={(e) => setWatermarkOpacity(Number(e.target.value))} />
              </label>
            </>
          )}
        </fieldset>

        {format === "jpeg" && (
          <fieldset className="mb-3 flex flex-col gap-1">
            <legend className="mb-1 text-xs font-medium text-text-secondary">{t("exportDialog.metadataJpegOnly")}</legend>
            <input
              type="text"
              value={metadataMake}
              onChange={(e) => setMetadataMake(e.target.value)}
              placeholder={t("exportDialog.make")}
              className="mb-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
            />
            <input
              type="text"
              value={metadataCopyright}
              onChange={(e) => setMetadataCopyright(e.target.value)}
              placeholder={t("exportDialog.copyright")}
              className="rounded border border-border bg-bg-panel px-2 py-1 text-xs"
            />
          </fieldset>
        )}

        {exportProgress && (
          <div className="mb-2 flex items-center gap-2 text-xs text-text-secondary">
            <span>
              {t("exportDialog.progress", { done: exportProgress.done, total: exportProgress.total })}
              {exportProgress.failed > 0 ? ` (${t("exportDialog.failedCount", { count: exportProgress.failed })})` : ""}
            </span>
            {exportRunning && (
              <button type="button" onClick={() => void toggleExportQueuePause()} className="rounded border border-border px-2 py-0.5 text-xs hover:border-accent">
                {exportQueuePaused ? t("exportDialog.resume") : t("exportDialog.pause")}
              </button>
            )}
          </div>
        )}
        {exportError && <p className="mb-2 text-xs text-danger">{t("exportDialog.error", { message: exportError })}</p>}
        {!exportRunning && exportProgress && exportProgress.done > 0 && (
          <p className="mb-2 text-xs text-text-secondary">{t("exportDialog.filesWritten", { count: exportProgress.done - exportProgress.failed })}</p>
        )}

        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded border border-border px-3 py-1 text-xs hover:border-accent">
            {t("exportDialog.close")}
          </button>
          <button
            type="button"
            onClick={() => void handleExport()}
            disabled={!destFolder || photoIds.length === 0 || exportRunning}
            className="rounded border border-accent bg-accent/10 px-3 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
          >
            {exportRunning ? t("exportDialog.exporting") : t("exportDialog.export")}
          </button>
        </div>
      </div>
    </div>
  );
}
