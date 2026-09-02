import { useT } from "../lib/i18n";
import { useAppStore } from "../store";

interface StackingDialogProps {
  open: boolean;
  onClose: () => void;
}

/**
 * Fokus-/HDR-/Panorama-/Astro-Stacking (Phase 9 Schritt 8, siehe
 * `PLAN.md`, `DECISIONS.md` ADR-0035 Punkt 2) — nimmt die aktuelle
 * Mehrfachauswahl, ruft den jeweiligen Algorithmus in `apx-stacking` auf
 * (reine, deterministische Bildverarbeitung, keine externe Registrierungs-
 * Bibliothek) und importiert das Ergebnis als neues, per Stapel mit den
 * Quellfotos verknüpftes Katalogfoto (`Catalog::create_stack`, Phase 9
 * Schritt 1).
 *
 * **Bewusste Vereinfachung**: alle vier Algorithmen setzen bereits
 * pixelgenau ausgerichtete Quellbilder voraus (Fokus/HDR: Stativaufnahmen;
 * Panorama/Astro: reine Verschiebungs-Registrierung per Phasenkorrelation,
 * kein Homographie-Stitching für Freihandaufnahmen — siehe
 * `apx_stacking::panorama`s Moduldoku).
 */
export function StackingDialog({ open, onClose }: StackingDialogProps) {
  const t = useT();
  const multiSelectedIds = useAppStore((s) => s.multiSelectedIds);
  const stackingRunning = useAppStore((s) => s.stackingRunning);
  const stackingStatus = useAppStore((s) => s.stackingStatus);
  const runStackFocus = useAppStore((s) => s.runStackFocus);
  const runStackHdr = useAppStore((s) => s.runStackHdr);
  const runStackPanorama = useAppStore((s) => s.runStackPanorama);
  const runStackAstro = useAppStore((s) => s.runStackAstro);

  if (!open) return null;

  const count = multiSelectedIds.length;
  const busy = stackingRunning !== null;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-24" onClick={onClose}>
      <div
        role="dialog"
        aria-label={t("stackingDialog.title")}
        className="w-full max-w-md rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 className="mb-1 text-sm font-semibold text-text-primary">{t("stackingDialog.title")}</h2>
        <p className="mb-3 text-xs text-text-muted">
          {t("stackingDialog.selectedCount", { count, noun: count === 1 ? t("stackingDialog.photoSingular") : t("stackingDialog.photoPlural") })}
        </p>

        <div className="flex flex-col gap-2">
          <button
            type="button"
            onClick={() => void runStackFocus()}
            disabled={busy || count < 2}
            title={t("stackingDialog.focusTitle")}
            className="rounded border border-border px-3 py-1.5 text-left text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
          >
            {t("stackingDialog.focus")}
          </button>
          <button
            type="button"
            onClick={() => void runStackHdr()}
            disabled={busy || count < 2}
            title={t("stackingDialog.hdrTitle")}
            className="rounded border border-border px-3 py-1.5 text-left text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
          >
            {t("stackingDialog.hdr")}
          </button>
          <button
            type="button"
            onClick={() => void runStackPanorama()}
            disabled={busy || count < 2}
            title={t("stackingDialog.panoramaTitle")}
            className="rounded border border-border px-3 py-1.5 text-left text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
          >
            {t("stackingDialog.panorama")}
          </button>
          <button
            type="button"
            onClick={() => void runStackAstro()}
            disabled={busy || count < 3}
            title={t("stackingDialog.astroTitle")}
            className="rounded border border-border px-3 py-1.5 text-left text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
          >
            {t("stackingDialog.astro")}
          </button>
        </div>

        {busy && <p className="mt-3 text-xs text-text-muted">{t("stackingDialog.running")}</p>}
        {stackingStatus && !busy && <p className="mt-3 text-xs text-text-secondary">{stackingStatus}</p>}

        <div className="mt-3 flex justify-end">
          <button type="button" onClick={onClose} className="rounded border border-border px-3 py-1 text-xs text-text-secondary hover:bg-bg-panel">
            {t("stackingDialog.close")}
          </button>
        </div>
      </div>
    </div>
  );
}
