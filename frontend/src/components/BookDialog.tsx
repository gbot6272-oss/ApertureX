import { useState } from "react";

import { pickFilePath, pickSaveFilePath, PRINT_SHOP_PRESET_NAMES, type BookOptions, type BookPageTemplate } from "../lib/tauri";
import { useAppStore } from "../store";

interface BookDialogProps {
  open: boolean;
  photoIds: string[];
  onClose: () => void;
}

const TEMPLATE_LABELS: Record<BookPageTemplate, string> = {
  full_bleed: "Ein Foto, randlos",
  two_side_by_side: "Zwei Fotos nebeneinander",
  grid_2x2: "2×2-Raster",
  photo_with_caption: "Foto mit Bildunterschrift (Dateiname)",
};

/**
 * Buch-Dialog (Phase 8 Schritt 5, siehe `PLAN.md`/`apx_export::book`s
 * Moduldoku). Automatische Befüllung: die ausgewählten Fotos werden
 * reihum auf Seiten gemäß der gewählten Vorlage verteilt, Bilder werden
 * serverseitig gerendert und als eine mehrseitige PDF-Datei exportiert.
 */
export function BookDialog({ open, photoIds, onClose }: BookDialogProps) {
  const bookExportRunning = useAppStore((s) => s.bookExportRunning);
  const bookExportError = useAppStore((s) => s.bookExportError);
  const bookExportOutcome = useAppStore((s) => s.bookExportOutcome);
  const exportBookPdf = useAppStore((s) => s.exportBookPdf);

  const [template, setTemplate] = useState<BookPageTemplate>("grid_2x2");
  const [pageWidthIn, setPageWidthIn] = useState(8);
  const [pageHeightIn, setPageHeightIn] = useState(8);
  const [printShopPreset, setPrintShopPreset] = useState<string>(PRINT_SHOP_PRESET_NAMES[0]);
  const [title, setTitle] = useState("");
  const [fontPath, setFontPath] = useState("");

  if (!open) return null;

  const needsFont = title.length > 0 || template === "photo_with_caption";

  async function handlePickFont() {
    const path = await pickFilePath("Schriftdateien", ["ttf", "otf"]);
    if (path) setFontPath(path);
  }

  async function handleExport() {
    const destPath = await pickSaveFilePath("PDF-Fotobuch", ["pdf"], "Fotobuch.pdf");
    if (!destPath) return;
    const options: BookOptions = {
      template,
      pageWidthIn,
      pageHeightIn,
      dpi: 300,
      printShopPreset,
      title: title || undefined,
      fontPath: fontPath || undefined,
    };
    await exportBookPdf(photoIds, destPath, options);
  }

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-16" onClick={onClose}>
      <div
        onClick={(e) => e.stopPropagation()}
        className="max-h-[85vh] w-full max-w-md overflow-y-auto rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
      >
        <h2 className="mb-1 text-sm font-semibold text-text-primary">Buch</h2>
        <p className="mb-3 text-xs text-text-muted">
          {photoIds.length} Foto{photoIds.length === 1 ? "" : "s"} — wird automatisch auf Seiten verteilt
        </p>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          Seitenvorlage
          <select value={template} onChange={(e) => setTemplate(e.target.value as BookPageTemplate)} className="rounded border border-border bg-bg-panel px-2 py-1 text-sm">
            {(Object.keys(TEMPLATE_LABELS) as BookPageTemplate[]).map((t) => (
              <option key={t} value={t}>
                {TEMPLATE_LABELS[t]}
              </option>
            ))}
          </select>
        </label>

        <div className="mb-3 flex gap-2">
          <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
            Breite (Zoll)
            <input type="number" min={1} step={0.5} value={pageWidthIn} onChange={(e) => setPageWidthIn(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
          </label>
          <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
            Höhe (Zoll)
            <input type="number" min={1} step={0.5} value={pageHeightIn} onChange={(e) => setPageHeightIn(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
          </label>
        </div>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          Druckerei-Preset
          <select value={printShopPreset} onChange={(e) => setPrintShopPreset(e.target.value)} className="rounded border border-border bg-bg-panel px-2 py-1 text-sm">
            {PRINT_SHOP_PRESET_NAMES.map((name) => (
              <option key={name} value={name}>
                {name}
              </option>
            ))}
          </select>
        </label>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          Titelseite (leer = keine)
          <input type="text" value={title} onChange={(e) => setTitle(e.target.value)} placeholder="z. B. Urlaub 2026" className="rounded border border-border bg-bg-panel px-2 py-1 text-sm" />
        </label>

        {needsFont && (
          <div className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
            <span>Schriftdatei (für Titel/Bildunterschriften)</span>
            <div className="flex gap-1">
              <input type="text" readOnly value={fontPath} placeholder="Keine ausgewählt" className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-sm" />
              <button type="button" onClick={() => void handlePickFont()} className="shrink-0 rounded border border-border px-2 py-1 text-xs hover:border-accent">
                Schriftdatei wählen…
              </button>
            </div>
          </div>
        )}

        {bookExportError && <p className="mb-2 text-xs text-danger">Fehler: {bookExportError}</p>}
        {!bookExportRunning && bookExportOutcome && (
          <p className="mb-2 text-xs text-text-secondary">
            Gespeichert: {bookExportOutcome.path} ({bookExportOutcome.page_count} Seiten)
          </p>
        )}

        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded border border-border px-3 py-1 text-xs hover:border-accent">
            Schließen
          </button>
          <button
            type="button"
            onClick={() => void handleExport()}
            disabled={photoIds.length === 0 || bookExportRunning}
            className="rounded border border-accent bg-accent/10 px-3 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
          >
            {bookExportRunning ? "Erzeuge PDF…" : "Als PDF speichern"}
          </button>
        </div>
      </div>
    </div>
  );
}
