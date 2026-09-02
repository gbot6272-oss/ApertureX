import { useEffect } from "react";

import { useLocale, useT } from "../lib/i18n";
import { useAppStore } from "../store";

interface StatsCacheDialogProps {
  open: boolean;
  onClose: () => void;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes;
  let unitIndex = -1;
  do {
    value /= 1024;
    unitIndex += 1;
  } while (value >= 1024 && unitIndex < units.length - 1);
  return `${value.toFixed(1)} ${units[unitIndex]}`;
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

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-16" onClick={onClose}>
      <div onClick={(e) => e.stopPropagation()} className="max-h-[85vh] w-full max-w-md overflow-y-auto rounded-lg border border-border bg-bg-raised p-4 shadow-xl">
        <h2 className="mb-3 text-sm font-semibold text-text-primary">{t("statsCacheDialog.title")}</h2>

        {stats && (
          <div className="mb-4 flex flex-col gap-2 text-xs">
            <p>
              <span className="text-text-secondary">{t("statsCacheDialog.totalPhotos")}</span> {stats.total_photos.toLocaleString(intlLocale)}
            </p>
            <p>
              <span className="text-text-secondary">{t("statsCacheDialog.totalSize")}</span> {formatBytes(stats.total_file_size)}
            </p>
            {stats.earliest_captured_at && stats.latest_captured_at && (
              <p>
                <span className="text-text-secondary">{t("statsCacheDialog.dateRange")}</span> {new Date(stats.earliest_captured_at).toLocaleDateString(intlLocale)} –{" "}
                {new Date(stats.latest_captured_at).toLocaleDateString(intlLocale)}
              </p>
            )}
            {stats.top_camera_models.length > 0 && (
              <div>
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
              <div>
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
    </div>
  );
}
