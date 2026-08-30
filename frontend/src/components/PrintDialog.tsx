import { useState } from "react";

import type { IccProfileChoice, PicturePackageTemplate, PrintFit, PrintLayoutKind, PrintLayoutOptions } from "../lib/tauri";
import { pickSaveFilePath } from "../lib/tauri";
import { useAppStore } from "../store";

interface PrintDialogProps {
  open: boolean;
  photoIds: string[];
  onClose: () => void;
}

const LAYOUT_LABELS: Record<PrintLayoutKind, string> = {
  single: "Einzelbild",
  contact_sheet: "Kontaktbogen",
  custom_grid: "Benutzerdefiniertes Raster",
  picture_package: "Bilderpaket",
};

const PACKAGE_LABELS: Record<PicturePackageTemplate, string> = {
  one_large_two_small: "1 groß + 2 klein",
  four_equal: "4 gleich groß",
  eight_wallet: "8 Wallet-Format",
};

/**
 * Druckdialog-Grundgerüst (Phase 8 Schritt 3, siehe `DECISIONS.md`
 * ADR-0034 und `apx_export::print`s Moduldoku). Setzt `photoIds` (eines je
 * Layout-Zelle) zu einer gemeinsamen Druckseite zusammen und speichert sie
 * als JPEG — kein System-Druckdialog-Zugriff in dieser Phase.
 */
export function PrintDialog({ open, photoIds, onClose }: PrintDialogProps) {
  const printRunning = useAppStore((s) => s.printRunning);
  const printError = useAppStore((s) => s.printError);
  const printLastOutcome = useAppStore((s) => s.printLastOutcome);
  const printPhotos = useAppStore((s) => s.printPhotos);

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
    const destPath = await pickSaveFilePath("JPEG-Bild", ["jpg"], "Druckseite.jpg");
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
        <h2 className="mb-1 text-sm font-semibold text-text-primary">Drucken</h2>
        <p className="mb-3 text-xs text-text-muted">
          {photoIds.length} Foto{photoIds.length === 1 ? "" : "s"} — wird als druckfertige JPEG-Seite gespeichert
        </p>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          Layout
          <select value={layout} onChange={(e) => setLayout(e.target.value as PrintLayoutKind)} className="rounded border border-border bg-bg-panel px-2 py-1 text-sm">
            {(Object.keys(LAYOUT_LABELS) as PrintLayoutKind[]).map((l) => (
              <option key={l} value={l}>
                {LAYOUT_LABELS[l]}
              </option>
            ))}
          </select>
        </label>

        {(layout === "contact_sheet" || layout === "custom_grid") && (
          <div className="mb-3 flex gap-2">
            <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
              Spalten
              <input type="number" min={1} value={cols} onChange={(e) => setCols(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
            </label>
            <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
              Zeilen
              <input type="number" min={1} value={rows} onChange={(e) => setRows(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
            </label>
          </div>
        )}

        {layout === "picture_package" && (
          <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
            Vorlage
            <select value={packageTemplate} onChange={(e) => setPackageTemplate(e.target.value as PicturePackageTemplate)} className="rounded border border-border bg-bg-panel px-2 py-1 text-sm">
              {(Object.keys(PACKAGE_LABELS) as PicturePackageTemplate[]).map((p) => (
                <option key={p} value={p}>
                  {PACKAGE_LABELS[p]}
                </option>
              ))}
            </select>
          </label>
        )}

        <div className="mb-3 flex gap-2">
          <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
            Breite (Zoll)
            <input type="number" min={1} step={0.5} value={pageWidthIn} onChange={(e) => setPageWidthIn(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
          </label>
          <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
            Höhe (Zoll)
            <input type="number" min={1} step={0.5} value={pageHeightIn} onChange={(e) => setPageHeightIn(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
          </label>
          <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
            DPI
            <input type="number" min={72} value={dpi} onChange={(e) => setDpi(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
          </label>
        </div>

        <div className="mb-3 flex gap-2">
          <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
            Rand (Zoll)
            <input type="number" min={0} step={0.05} value={marginIn} onChange={(e) => setMarginIn(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
          </label>
          <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
            Zellabstand (Zoll)
            <input type="number" min={0} step={0.05} value={gapIn} onChange={(e) => setGapIn(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
          </label>
        </div>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          Zoom
          <select value={fit} onChange={(e) => setFit(e.target.value as PrintFit)} className="rounded border border-border bg-bg-panel px-2 py-1 text-sm">
            <option value="contain">Ganzes Bild einpassen</option>
            <option value="cover">Zelle füllen (beschneiden)</option>
          </select>
        </label>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          Farbraum (ICC)
          <select value={iccProfile} onChange={(e) => setIccProfile(e.target.value as IccProfileChoice)} className="rounded border border-border bg-bg-panel px-2 py-1 text-sm">
            <option value="srgb">sRGB</option>
            <option value="adobe_rgb">Adobe RGB</option>
            <option value="pro_photo_rgb">ProPhoto RGB</option>
            <option value="display_p3">Display P3</option>
          </select>
        </label>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          Druckschärfung ({sharpenAmount === 0 ? "aus" : sharpenAmount.toFixed(1)})
          <input type="range" min={0} max={2} step={0.1} value={sharpenAmount} onChange={(e) => setSharpenAmount(Number(e.target.value))} />
        </label>

        {printError && <p className="mb-2 text-xs text-danger">Fehler: {printError}</p>}
        {!printRunning && printLastOutcome && <p className="mb-2 text-xs text-text-secondary">Gespeichert: {printLastOutcome.path}</p>}

        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded border border-border px-3 py-1 text-xs hover:border-accent">
            Schließen
          </button>
          <button
            type="button"
            onClick={() => void handlePrint()}
            disabled={photoIds.length === 0 || printRunning}
            className="rounded border border-accent bg-accent/10 px-3 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
          >
            {printRunning ? "Erzeuge Seite…" : "Als JPEG speichern"}
          </button>
        </div>
      </div>
    </div>
  );
}
