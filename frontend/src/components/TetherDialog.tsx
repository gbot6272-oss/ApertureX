import { useEffect, useState } from "react";

import { useT } from "../lib/i18n";
import { useAppStore } from "../store";

interface TetherDialogProps {
  open: boolean;
  onClose: () => void;
}

/**
 * Tethered Shooting (Phase 9 Schritt 11, siehe `PLAN.md`, `DECISIONS.md`
 * ADR-0035 Punkt 5) — Kamera verbinden, auslösen, automatisch über ein
 * gewähltes Import-Preset (Phase 3/5) katalogisieren.
 *
 * **Ehrlich begrenzt**: Ohne das Cargo-Feature `tethering` (Standard auf
 * macOS/Windows-CI, siehe `THIRD_PARTY.md` — `libgphoto2` fehlt dort) läuft
 * ausschließlich eine Simulation (`apx_tether::FakeBackend`) — die
 * verbundene Kamera zeigt das explizit an, statt echte Hardware
 * vorzutäuschen.
 */
export function TetherDialog({ open, onClose }: TetherDialogProps) {
  const t = useT();
  const [presetName, setPresetName] = useState<string>("");

  const tetherConnecting = useAppStore((s) => s.tetherConnecting);
  const tetherCamera = useAppStore((s) => s.tetherCamera);
  const tetherCapturing = useAppStore((s) => s.tetherCapturing);
  const tetherStatus = useAppStore((s) => s.tetherStatus);
  const connectTetherCamera = useAppStore((s) => s.connectTetherCamera);
  const captureTetherPhoto = useAppStore((s) => s.captureTetherPhoto);
  const importPresets = useAppStore((s) => s.importPresets);
  const refreshImportPresets = useAppStore((s) => s.refreshImportPresets);
  const cameraFiles = useAppStore((s) => s.cameraFiles);
  const cameraFilesLoading = useAppStore((s) => s.cameraFilesLoading);
  const listCameraFilesAction = useAppStore((s) => s.listCameraFilesAction);
  const importCameraFile = useAppStore((s) => s.importCameraFile);

  useEffect(() => {
    if (open) void refreshImportPresets();
  }, [open, refreshImportPresets]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-24" onClick={onClose}>
      <div
        role="dialog"
        aria-label={t("tetherDialog.title")}
        className="w-full max-w-md rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 className="mb-1 text-sm font-semibold text-text-primary">{t("tetherDialog.title")}</h2>

        {!tetherCamera ? (
          <div className="flex flex-col gap-2">
            <p className="text-xs text-text-muted">{t("tetherDialog.noCamera")}</p>
            <div className="flex justify-end">
              <button
                type="button"
                onClick={() => void connectTetherCamera()}
                disabled={tetherConnecting}
                className="rounded border border-border px-3 py-1.5 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
              >
                {t("tetherDialog.connect")}
              </button>
            </div>
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            <p className="text-xs text-text-secondary">
              {tetherCamera.model} ({tetherCamera.port})
              {tetherCamera.simulated && (
                <span className="ml-1 rounded bg-bg-panel px-1 py-0.5 text-[10px] text-text-muted">
                  {t("tetherDialog.simulation")}
                </span>
              )}
            </p>

            <label className="text-xs text-text-secondary">
              {t("tetherDialog.importPreset")}
              <select
                value={presetName}
                onChange={(event) => setPresetName(event.target.value)}
                className="mt-1 w-full rounded border border-border bg-bg-panel px-2 py-1 text-xs text-text-primary"
              >
                <option value="">{t("tetherDialog.noPreset")}</option>
                {importPresets.map((preset) => (
                  <option key={preset.name} value={preset.name}>
                    {preset.name}
                  </option>
                ))}
              </select>
            </label>

            <div className="flex justify-end">
              <button
                type="button"
                onClick={() => void captureTetherPhoto(presetName || undefined)}
                disabled={tetherCapturing}
                className="rounded border border-border px-3 py-1.5 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
              >
                {t("tetherDialog.trigger")}
              </button>
            </div>

            {/* Direktimport bereits vorhandener Aufnahmen (Phase 13
                Schritt 2) — im Unterschied zum Auslösen oben: listet
                Dateien, die schon auf der Kamera liegen. */}
            <div className="mt-2 border-t border-border pt-2">
              <div className="flex items-center justify-between">
                <span className="text-xs font-medium text-text-secondary">{t("tetherDialog.existingFiles")}</span>
                <button
                  type="button"
                  onClick={() => void listCameraFilesAction()}
                  disabled={cameraFilesLoading}
                  className="rounded border border-border px-2 py-1 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
                >
                  {cameraFilesLoading ? t("tetherDialog.listing") : t("tetherDialog.listFiles")}
                </button>
              </div>
              {cameraFiles.length > 0 && (
                <ul className="mt-1 flex max-h-40 flex-col gap-1 overflow-y-auto text-xs text-text-secondary">
                  {cameraFiles.map((file) => (
                    <li
                      key={`${file.folder}/${file.name}`}
                      className="flex items-center justify-between rounded border border-border px-2 py-1"
                    >
                      <span className="truncate" title={`${file.folder}/${file.name}`}>
                        {file.name}
                      </span>
                      <button
                        type="button"
                        disabled={tetherCapturing}
                        onClick={() => void importCameraFile(file, presetName || undefined)}
                        className="shrink-0 text-accent underline disabled:cursor-not-allowed disabled:opacity-40"
                      >
                        {t("tetherDialog.importFile")}
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </div>
        )}

        {(tetherConnecting || tetherCapturing) && <p className="mt-2 text-xs text-text-muted">{t("tetherDialog.running")}</p>}
        {tetherStatus && !tetherConnecting && !tetherCapturing && (
          <p className="mt-2 text-xs text-text-secondary">{tetherStatus}</p>
        )}

        <div className="mt-3 flex justify-end">
          <button
            type="button"
            onClick={onClose}
            className="rounded border border-border px-3 py-1 text-xs text-text-secondary hover:bg-bg-panel"
          >
            {t("tetherDialog.close")}
          </button>
        </div>
      </div>
    </div>
  );
}
