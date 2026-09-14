import { useEffect } from "react";

import { useLocale, useT } from "../lib/i18n";
import { formatBytes } from "../lib/format";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

interface StatsCacheDialogProps {
  open: boolean;
  onClose: () => void;
}

/**
 * Katalog-Statistik-Dashboard + Vorschau-Cache-Verwaltung (Phase 9
 * Schritt 3, siehe `PLAN.md`/`DECISIONS.md` ADR-0035) — reine
 * Aggregatanzeige/Cache-Aktionen, keine eigene Zustandsverwaltung über
 * einen Dialog-Tab-Mechanismus hinaus nötig (deutlich schlanker als
 * `LibraryOrganizeDialog.tsx`/`MetadataDialog.tsx`).
 */
export function StatsCacheDialog({ open, onClose }: StatsCacheDialogProps) {
  const t = useT();
  const locale = useLocale();
  const intlLocale = locale === "en" ? "en-US" : "de-DE";
  const stats = useAppStore((s) => s.catalogStatistics);
  const refreshCatalogStatistics = useAppStore((s) => s.refreshCatalogStatistics);
  const cacheStats = useAppStore((s) => s.previewCacheStats);
  const refreshPreviewCacheStats = useAppStore((s) => s.refreshPreviewCacheStats);
  const clearPreviewCache = useAppStore((s) => s.clearPreviewCache);

  useEffect(() => {
    if (!open) return;
    void refreshCatalogStatistics();
    void refreshPreviewCacheStats();
  }, [open, refreshCatalogStatistics, refreshPreviewCacheStats]);

  return (
    <Dialog open={open} onClose={onClose} label={t("statsCacheDialog.title")} className="max-w-md">
      <div className="p-4">
        <h2 className="mb-3 text-sm font-semibold text-text-primary">{t("statsCacheDialog.title")}</h2>

        {stats && (
          // Phase 25 Schritt 5 (siehe DECISIONS.md, aktuelles ADR):
          // Bento-artiges Kachelraster statt einer flachen `<p>`-Liste
          // — unterschiedlich große Karten (Gesamtzahl als große
          // "Hero"-Kachel, Kamera-/Bewertungslisten breiter, Größe/
          // Zeitraum schmal), statt einer gleichförmigen Textspalte.
          // Textinhalt je Kachel unverändert (`name: count` etc.) —
          // reine Neuanordnung, keine Bedeutungsänderung.
          <div className="mb-4 grid grid-cols-2 gap-3 text-xs">
            <div className="col-span-2 rounded-xl border border-border bg-bg-panel p-4">
              <p className="text-text-secondary">{t("statsCacheDialog.totalPhotos")}</p>
              <p className="mt-1 text-2xl font-semibold tracking-tight text-text-primary">{stats.total_photos.toLocaleString(intlLocale)}</p>
            </div>
            <div className="rounded-xl border border-border bg-bg-panel p-3">
              <p className="text-text-secondary">{t("statsCacheDialog.totalSize")}</p>
              <p className="mt-1 font-medium text-text-primary">{formatBytes(stats.total_file_size)}</p>
            </div>
            {stats.earliest_captured_at && stats.latest_captured_at && (
              <div className="rounded-xl border border-border bg-bg-panel p-3">
                <p className="text-text-secondary">{t("statsCacheDialog.dateRange")}</p>
                <p className="mt-1 font-medium text-text-primary">
                  {new Date(stats.earliest_captured_at).toLocaleDateString(intlLocale)} – {new Date(stats.latest_captured_at).toLocaleDateString(intlLocale)}
                </p>
              </div>
            )}
            {stats.top_camera_models.length > 0 && (
              <div className="col-span-2 rounded-xl border border-border bg-bg-panel p-3">
                <p className="mb-1 font-semibold text-text-secondary">{t("statsCacheDialog.cameraModels")}</p>
                <ul>
                  {stats.top_camera_models.map(([name, count]) => (
                    <li key={name}>
                      {name}: {count}
                    </li>
                  ))}
                </ul>
              </div>
            )}
            {stats.rating_distribution.some(([, count]) => count > 0) && (
              <div className="col-span-2 rounded-xl border border-border bg-bg-panel p-3">
                <p className="mb-1 font-semibold text-text-secondary">{t("statsCacheDialog.ratingDistribution")}</p>
                <ul>
                  {stats.rating_distribution.map(([rating, count]) => (
                    <li key={rating}>
                      {rating === 0 ? t("statsCacheDialog.unrated") : `${rating}★`}: {count}
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        )}

        <div className="rounded border border-border p-2 text-xs">
          <p className="mb-2 font-semibold text-text-secondary">{t("statsCacheDialog.previewCache")}</p>
          {cacheStats && (
            <p className="mb-2">
              {t("statsCacheDialog.cacheFileCount", { count: cacheStats.file_count.toLocaleString(intlLocale), size: formatBytes(cacheStats.total_bytes) })}
            </p>
          )}
          <button type="button" onClick={() => void clearPreviewCache()} className="w-full rounded border border-border px-2 py-1 hover:border-danger">
            {t("statsCacheDialog.clearCache")}
          </button>
        </div>
      </div>
    </Dialog>
  );
}
