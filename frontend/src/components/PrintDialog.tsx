import { useState } from "react";

import { useT } from "../lib/i18n";
import type { IccProfileChoice, PicturePackageTemplate, PrintFit, PrintLayoutKind, PrintLayoutOptions } from "../lib/tauri";
import { pickSaveFilePath } from "../lib/tauri";
import { useAppStore } from "../store";

interface PrintDialogProps {
  open: boolean;
  photoIds: string[];
  onClose: () => void;
}

/**
 * Druckdialog-Grundgerüst (Phase 8 Schritt 3, siehe `DECISIONS.md`
 * ADR-0034 und `apx_export::print`s Moduldoku). Setzt `photoIds` (eines je
 * Layout-Zelle) zu einer gemeinsamen Druckseite zusammen und speichert sie
 * als JPEG — kein System-Druckdialog-Zugriff in dieser Phase.
 */
export function PrintDialog({ open, photoIds, onClose }: PrintDialogProps) {
  const t = useT();
  const printRunning = useAppStore((s) => s.printRunning);
  const printError = useAppStore((s) => s.printError);
  const printLastOutcome = useAppStore((s) => s.printLastOutcome);
  const printPhotos = useAppStore((s) => s.printPhotos);

  const LAYOUT_LABELS: Record<PrintLayoutKind, string> = {
    single: t("printDialog.layoutSingle"),
    contact_sheet: t("printDialog.layoutContactSheet"),
    custom_grid: t("printDialog.layoutCustomGrid"),
    picture_package: t("printDialog.layoutPicturePackage"),
  };

  const PACKAGE_LABELS: Record<PicturePackageTemplate, string> = {
    one_large_two_small: t("printDialog.packageOneLargeTwoSmall"),
    four_equal: t("printDialog.packageFourEqual"),
    eight_wallet: t("printDialog.packageEightWallet"),
  };

  const [layout, setLayout] = useState<PrintLayoutKind>("single");
  const [cols, setCols] = useState(2);
  const [rows, setRows] = useState(2);
  const [packageTemplate, setPackageTemplate] = useState<PicturePackageTemplate>("four_equal");
  const [pageWidthIn, setPageWidthIn] = useState(8.5);
  const [pageHeightIn, setPageHeightIn] = useState(11);
  const [dpi, setDpi] = useState(300);
  const [marginIn, setMarginIn] = useState(0.25);
  const [gapIn, setGapIn] = useState(0.1);
  const [fit, setFit] = useState<PrintFit>("contain");
  const [iccProfile, setIccProfile] = useState<IccProfileChoice>("srgb");
  const [sharpenAmount, setSharpenAmount] = useState(0.5);

  if (!open) return null;

  async function handlePrint() {
    const destPath = await pickSaveFilePath(t("printDialog.jpegFilterName"), ["jpg"], "Druckseite.jpg");
    if (!destPath) return;
    const options: PrintLayoutOptions = {
      layout,
      cols: layout === "contact_sheet" || layout === "custom_grid" ? cols : undefined,
      rows: layout === "contact_sheet" || layout === "custom_grid" ? rows : undefined,
      picturePackageTemplate: layout === "picture_package" ? packageTemplate : undefined,
      pageWidthIn,
      pageHeightIn,
      dpi,
      marginIn,
      gapIn,
      fit,
      sharpenAmount: sharpenAmount > 0 ? sharpenAmount : undefined,
      iccProfile: iccProfile !== "srgb" ? iccProfile : undefined,
    };
    await printPhotos(photoIds, destPath, options);
  }

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-16" onClick={onClose}>
      <div
        onClick={(e) => e.stopPropagation()}
        className="max-h-[85vh] w-full max-w-md overflow-y-auto rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
      >
        <h2 className="mb-1 text-sm font-semibold text-text-primary">{t("printDialog.title")}</h2>
        <p className="mb-3 text-xs text-text-muted">
          {t("printDialog.photoCount", { count: photoIds.length, plural: photoIds.length === 1 ? "" : "s" })}
        </p>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          {t("printDialog.layout")}
          <select value={layout} onChange={(e) => setLayout(e.target.value as PrintLayoutKind)} className="rounded border border-border bg-bg-panel px-2 py-1 text-sm">
            {(Object.keys(LAYOUT_LABELS) as PrintLayoutKind[]).map((key) => (
              <option key={key} value={key}>
                {LAYOUT_LABELS[key]}
              </option>
            ))}
          </select>
        </label>

        {(layout === "contact_sheet" || layout === "custom_grid") && (
          <div className="mb-3 flex gap-2">
            <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
              {t("printDialog.columns")}
              <input type="number" min={1} value={cols} onChange={(e) => setCols(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
            </label>
            <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
              {t("printDialog.rows")}
              <input type="number" min={1} value={rows} onChange={(e) => setRows(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
            </label>
          </div>
        )}

        {layout === "picture_package" && (
          <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
            {t("printDialog.template")}
            <select value={packageTemplate} onChange={(e) => setPackageTemplate(e.target.value as PicturePackageTemplate)} className="rounded border border-border bg-bg-panel px-2 py-1 text-sm">
              {(Object.keys(PACKAGE_LABELS) as PicturePackageTemplate[]).map((key) => (
                <option key={key} value={key}>
                  {PACKAGE_LABELS[key]}
                </option>
              ))}
            </select>
          </label>
        )}

        <div className="mb-3 flex gap-2">
          <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
            {t("printDialog.widthIn")}
            <input type="number" min={1} step={0.5} value={pageWidthIn} onChange={(e) => setPageWidthIn(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
          </label>
          <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
            {t("printDialog.heightIn")}
            <input type="number" min={1} step={0.5} value={pageHeightIn} onChange={(e) => setPageHeightIn(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
          </label>
          <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
            DPI
            <input type="number" min={72} value={dpi} onChange={(e) => setDpi(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
          </label>
        </div>

        <div className="mb-3 flex gap-2">
          <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
            {t("printDialog.marginIn")}
            <input type="number" min={0} step={0.05} value={marginIn} onChange={(e) => setMarginIn(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
          </label>
          <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
            {t("printDialog.gapIn")}
            <input type="number" min={0} step={0.05} value={gapIn} onChange={(e) => setGapIn(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
          </label>
        </div>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          {t("printDialog.zoom")}
          <select value={fit} onChange={(e) => setFit(e.target.value as PrintFit)} className="rounded border border-border bg-bg-panel px-2 py-1 text-sm">
            <option value="contain">{t("printDialog.fitContain")}</option>
            <option value="cover">{t("printDialog.fitCover")}</option>
          </select>
        </label>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          {t("printDialog.colorSpace")}
          <select value={iccProfile} onChange={(e) => setIccProfile(e.target.value as IccProfileChoice)} className="rounded border border-border bg-bg-panel px-2 py-1 text-sm">
            <option value="srgb">sRGB</option>
            <option value="adobe_rgb">Adobe RGB</option>
            <option value="pro_photo_rgb">ProPhoto RGB</option>
            <option value="display_p3">Display P3</option>
          </select>
        </label>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          {t("printDialog.printSharpening", { value: sharpenAmount === 0 ? t("printDialog.off") : sharpenAmount.toFixed(1) })}
          <input type="range" min={0} max={2} step={0.1} value={sharpenAmount} onChange={(e) => setSharpenAmount(Number(e.target.value))} />
        </label>

        {printError && <p className="mb-2 text-xs text-danger">{t("printDialog.error", { message: printError })}</p>}
        {!printRunning && printLastOutcome && <p className="mb-2 text-xs text-text-secondary">{t("printDialog.savedOutcome", { path: printLastOutcome.path })}</p>}

        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded border border-border px-3 py-1 text-xs hover:border-accent">
            {t("printDialog.close")}
          </button>
          <button
            type="button"
            onClick={() => void handlePrint()}
            disabled={photoIds.length === 0 || printRunning}
            className="rounded border border-accent bg-accent/10 px-3 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
          >
            {printRunning ? t("printDialog.generating") : t("printDialog.saveAsJpeg")}
          </button>
        </div>
      </div>
    </div>
  );
}
