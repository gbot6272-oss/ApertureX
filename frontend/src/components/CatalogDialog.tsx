import { useEffect, useState } from "react";

import { useT } from "../lib/i18n";
import { formatBytes } from "../lib/format";
import {
  createNewCatalog,
  getActiveCatalogInfo,
  listRecentCatalogs,
  pickFilePath,
  pickSaveFilePath,
  runCatalogBackup,
  runCatalogIntegrityCheck,
  runCatalogOptimize,
  switchActiveCatalog,
  type CatalogInfoDto,
  type RecentCatalogDto,
} from "../lib/tauri";

interface CatalogDialogProps {
  open: boolean;
  onClose: () => void;
}

/**
 * Mehrere Kataloge + Katalog-Wartung (Phase 13 Schritt 6, siehe
 * `DECISIONS.md` ADR-0040-Nachtrag IV) — aktueller Katalog (Pfad/Größe),
 * Wartungsaktionen (Integritätsprüfung/Optimieren/Sichern) auf dem
 * laufenden Katalog, sowie Wechsel/Neuanlage über die "Zuletzt
 * geöffnet"-Liste. **Wechseln/Neuanlegen startet Aperture X neu** (kein
 * Hot-Swap der offenen Katalogverbindung im laufenden Prozess, siehe
 * `apx-app::commands`s Moduldoku) — die Erfolgs-Zusage der jeweiligen
 * Aktion kommt praktisch nie sichtbar an, da der Prozess kurz danach neu
 * startet; nur ein tatsächlicher *Fehler* (z. B. eine fremde,
 * nicht-Aperture-X-Datei ausgewählt) zeigt sich noch in diesem Dialog.
 */
export function CatalogDialog({ open, onClose }: CatalogDialogProps) {
  const t = useT();
  const [info, setInfo] = useState<CatalogInfoDto | null>(null);
  const [recent, setRecent] = useState<RecentCatalogDto[]>([]);
  const [integrityResult, setIntegrityResult] = useState<string[] | null>(null);
  const [integrityRunning, setIntegrityRunning] = useState(false);
  const [optimizeRunning, setOptimizeRunning] = useState(false);
  const [optimizeDone, setOptimizeDone] = useState(false);
  const [backupRunning, setBackupRunning] = useState(false);
  const [backupDonePath, setBackupDonePath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = () => {
    void getActiveCatalogInfo().then(setInfo);
    void listRecentCatalogs().then(setRecent);
  };

  useEffect(() => {
    if (!open) return;
    setIntegrityResult(null);
    setOptimizeDone(false);
    setBackupDonePath(null);
    setError(null);
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- lädt bewusst nur beim Öffnen neu, `refresh` ist keine stabile Referenz.
  }, [open]);

  if (!open) return null;

  const handleIntegrityCheck = async () => {
    setIntegrityRunning(true);
    setError(null);
    try {
      setIntegrityResult(await runCatalogIntegrityCheck());
    } catch (err) {
      setError(String(err));
    } finally {
      setIntegrityRunning(false);
    }
  };

  const handleOptimize = async () => {
    setOptimizeRunning(true);
    setOptimizeDone(false);
    setError(null);
    try {
      await runCatalogOptimize();
      setOptimizeDone(true);
      refresh(); // Größe hat sich vermutlich geändert.
    } catch (err) {
      setError(String(err));
    } finally {
      setOptimizeRunning(false);
    }
  };

  const handleBackup = async () => {
    setError(null);
    const defaultName = `Katalog-Sicherung-${new Date().toISOString().slice(0, 10)}.sqlite`;
    const destination = await pickSaveFilePath("SQLite-Katalog", ["sqlite"], defaultName);
    if (!destination) return;
    setBackupRunning(true);
    setBackupDonePath(null);
    try {
      await runCatalogBackup(destination);
      setBackupDonePath(destination);
    } catch (err) {
      setError(String(err));
    } finally {
      setBackupRunning(false);
    }
  };

  const handleNewCatalog = async () => {
    setError(null);
    const path = await pickSaveFilePath("SQLite-Katalog", ["sqlite"], "Katalog.sqlite");
    if (!path) return;
    try {
      await createNewCatalog(path);
    } catch (err) {
      setError(String(err));
    }
  };

  const handleOpenCatalog = async () => {
    setError(null);
    const path = await pickFilePath("SQLite-Katalog", ["sqlite"]);
    if (!path) return;
    try {
      await switchActiveCatalog(path);
    } catch (err) {
      setError(String(err));
    }
  };

  const handleSwitchTo = async (path: string) => {
    setError(null);
    try {
      await switchActiveCatalog(path);
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-16" onClick={onClose}>
      <div onClick={(e) => e.stopPropagation()} className="max-h-[85vh] w-full max-w-lg overflow-y-auto rounded-lg border border-border bg-bg-raised p-4 shadow-xl">
        <h2 className="mb-3 text-sm font-semibold text-text-primary">{t("catalogDialog.title")}</h2>

        {error && <p className="mb-3 rounded border border-danger px-2 py-1 text-xs text-danger">{error}</p>}

        <div className="mb-4 flex flex-col gap-1 rounded border border-border p-2 text-xs">
          <p className="font-semibold text-text-secondary">{t("catalogDialog.currentCatalog")}</p>
          {info && (
            <>
              <p className="break-all">
                <span className="text-text-secondary">{t("catalogDialog.path")}</span> {info.path}
              </p>
              {info.file_size_bytes != null && (
                <p>
                  <span className="text-text-secondary">{t("catalogDialog.size")}</span> {formatBytes(info.file_size_bytes)}
                </p>
              )}
            </>
          )}
        </div>

        <div className="mb-4 flex flex-col gap-2 rounded border border-border p-2 text-xs">
          <p className="font-semibold text-text-secondary">{t("catalogDialog.maintenance")}</p>

          <button
            type="button"
            disabled={integrityRunning}
            onClick={() => void handleIntegrityCheck()}
            className="rounded border border-border px-2 py-1 hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
          >
            {integrityRunning ? t("catalogDialog.integrityCheckRunning") : t("catalogDialog.integrityCheck")}
          </button>
          {integrityResult &&
            (integrityResult.length === 0 ? (
              <p>{t("catalogDialog.integrityCheckOk")}</p>
            ) : (
              <div>
                <p>{t("catalogDialog.integrityCheckProblems", { count: integrityResult.length })}</p>
                <ul className="list-disc pl-4">
                  {integrityResult.map((problem) => (
                    <li key={problem}>{problem}</li>
                  ))}
                </ul>
              </div>
            ))}

          <button
            type="button"
            disabled={optimizeRunning}
            onClick={() => void handleOptimize()}
            className="rounded border border-border px-2 py-1 hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
          >
            {optimizeRunning ? t("catalogDialog.optimizeRunning") : t("catalogDialog.optimize")}
          </button>
          {optimizeDone && <p>{t("catalogDialog.optimizeDone")}</p>}

          <button
            type="button"
            disabled={backupRunning}
            onClick={() => void handleBackup()}
            className="rounded border border-border px-2 py-1 hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
          >
            {backupRunning ? t("catalogDialog.backupRunning") : t("catalogDialog.backup")}
          </button>
          {backupDonePath && <p className="break-all">{t("catalogDialog.backupDone", { path: backupDonePath })}</p>}
        </div>

        <div className="flex flex-col gap-2 rounded border border-border p-2 text-xs">
          <p className="font-semibold text-text-secondary">{t("catalogDialog.switchCatalog")}</p>
          <p className="text-text-secondary">{t("catalogDialog.switchWarning")}</p>
          <div className="flex gap-2">
            <button type="button" onClick={() => void handleNewCatalog()} className="flex-1 rounded border border-border px-2 py-1 hover:border-accent">
              {t("catalogDialog.newCatalog")}
            </button>
            <button type="button" onClick={() => void handleOpenCatalog()} className="flex-1 rounded border border-border px-2 py-1 hover:border-accent">
              {t("catalogDialog.openCatalog")}
            </button>
          </div>

          <p className="mt-2 font-semibold text-text-secondary">{t("catalogDialog.recentCatalogs")}</p>
          {recent.length === 0 ? (
            <p className="text-text-secondary">{t("catalogDialog.noRecentCatalogs")}</p>
          ) : (
            <ul className="flex flex-col gap-1">
              {recent.map((entry) => (
                <li key={entry.path} className="flex items-center justify-between gap-2 rounded border border-border px-2 py-1">
                  <div className="min-w-0">
                    <p className="truncate">{entry.file_name}</p>
                    <p className="truncate text-[10px] text-text-secondary">{entry.path}</p>
                  </div>
                  {entry.is_current ? (
                    <span className="shrink-0 text-text-secondary">{t("catalogDialog.current")}</span>
                  ) : entry.exists ? (
                    <button
                      type="button"
                      onClick={() => void handleSwitchTo(entry.path)}
                      className="shrink-0 rounded border border-border px-2 py-0.5 hover:border-accent"
                    >
                      {t("catalogDialog.switchTo")}
                    </button>
                  ) : (
                    <span className="shrink-0 text-danger">{t("catalogDialog.missing")}</span>
                  )}
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}
