import { useEffect, useRef, useState } from "react";

import { useFocusTrap } from "../lib/a11y";
import { useT } from "../lib/i18n";
import { selectFolderDialog, type UiSettingsDto } from "../lib/tauri";
import { resetWorkspaceLayout } from "../lib/workspaceLayout";
import { useAppStore } from "../store";

interface SettingsDialogProps {
  open: boolean;
  onClose: () => void;
}

type Tab = "anzeige" | "sprache" | "import";

const DEFAULT_ACCENT = "#5b9bd5";

/**
 * Zentraler Einstellungsdialog (Phase 10 Schritt 1) — Steuerfläche für
 * `apx_core::settings::UiSettings` (Theme/Akzentfarbe/Sprache/
 * UI-Skalierung/Kontrastmodus/reduzierte Bewegung). Das Schreiben in die
 * Einstellungsdatei ist ab diesem Schritt vollständig verdrahtet; dass die
 * einzelnen Felder tatsächlich sichtbar wirken (helles Theme, Kontrast,
 * Skalierung, Sprachumschaltung), kommt in den jeweils dafür vorgesehenen
 * späteren Schritten dieser Phase (siehe `PLAN.md` Schritte 6–8) — hier
 * werden sie schon persistiert, damit keine Reihenfolge-Abhängigkeit
 * zwischen Dialog und CSS/i18n-Anbindung entsteht.
 */
export function SettingsDialog({ open, onClose }: SettingsDialogProps) {
  const t = useT();
  const [tab, setTab] = useState<Tab>("anzeige");
  const uiSettings = useAppStore((s) => s.uiSettings);
  const loadUiSettings = useAppStore((s) => s.loadUiSettings);
  const saveUiSettings = useAppStore((s) => s.saveUiSettings);
  const watchedFolderSettings = useAppStore((s) => s.watchedFolderSettings);
  const loadWatchedFolderSettings = useAppStore((s) => s.loadWatchedFolderSettings);
  const saveWatchedFolderSettings = useAppStore((s) => s.saveWatchedFolderSettings);
  const dialogRef = useRef<HTMLDivElement>(null);
  useFocusTrap(dialogRef, open);

  useEffect(() => {
    if (open && !uiSettings) void loadUiSettings();
  }, [open, uiSettings, loadUiSettings]);

  useEffect(() => {
    if (open && !watchedFolderSettings) void loadWatchedFolderSettings();
  }, [open, watchedFolderSettings, loadWatchedFolderSettings]);

  if (!open) return null;

  const tabLabels: Record<Tab, string> = {
    anzeige: t("settings.tab.display"),
    sprache: t("settings.tab.language"),
    import: t("settings.tab.import"),
  };

  function update(patch: Partial<UiSettingsDto>) {
    if (!uiSettings) return;
    void saveUiSettings({ ...uiSettings, ...patch });
  }

  async function handlePickWatchedFolder() {
    const path = await selectFolderDialog();
    if (path && watchedFolderSettings) {
      void saveWatchedFolderSettings({ ...watchedFolderSettings, path });
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={onClose}>
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label={t("settings.title")}
        className="flex max-h-[80vh] w-full max-w-lg flex-col rounded-lg border border-border bg-bg-raised shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <h2 className="text-sm font-semibold">{t("settings.title")}</h2>
          <button type="button" onClick={onClose} className="text-text-secondary hover:text-text-primary" aria-label={t("settings.close")}>
            ✕
          </button>
        </div>

        <div className="flex border-b border-border">
          {(Object.keys(tabLabels) as Tab[]).map((tabKey) => (
            <button
              key={tabKey}
              type="button"
              onClick={() => setTab(tabKey)}
              className={`px-4 py-2 text-xs ${tab === tabKey ? "border-b-2 border-accent text-text-primary" : "text-text-secondary"}`}
            >
              {tabLabels[tabKey]}
            </button>
          ))}
        </div>

        <div className="flex-1 overflow-y-auto p-4">
          {!uiSettings ? (
            <p className="text-xs text-text-muted">{t("settings.loading")}</p>
          ) : tab === "anzeige" ? (
            <div className="flex flex-col gap-4 text-xs">
              <label className="flex flex-col gap-1">
                <span className="text-text-secondary">{t("settings.theme")}</span>
                <select
                  value={uiSettings.theme}
                  onChange={(event) => update({ theme: event.target.value as UiSettingsDto["theme"] })}
                  className="rounded border border-border bg-bg-panel px-2 py-1"
                >
                  <option value="dark">{t("settings.themeDark")}</option>
                  <option value="light">{t("settings.themeLight")}</option>
                </select>
              </label>

              <label className="flex items-center justify-between gap-2">
                <span className="text-text-secondary">{t("settings.accentColor")}</span>
                <div className="flex items-center gap-2">
                  <input
                    type="color"
                    value={uiSettings.accent_color ?? DEFAULT_ACCENT}
                    onChange={(event) => update({ accent_color: event.target.value })}
                    aria-label={t("settings.accentColor")}
                  />
                  {uiSettings.accent_color && (
                    <button type="button" onClick={() => update({ accent_color: null })} className="text-text-muted hover:text-text-primary">
                      {t("settings.accentColorReset")}
                    </button>
                  )}
                </div>
              </label>

              <label className="flex flex-col gap-1">
                <span className="text-text-secondary">{t("settings.uiScale", { percent: uiSettings.ui_scale_percent })}</span>
                <input
                  type="range"
                  min={75}
                  max={200}
                  step={5}
                  value={uiSettings.ui_scale_percent}
                  onChange={(event) => update({ ui_scale_percent: Number(event.target.value) })}
                />
              </label>

              <label className="flex items-center justify-between gap-2">
                <span className="text-text-secondary">{t("settings.highContrast")}</span>
                <input
                  type="checkbox"
                  checked={uiSettings.high_contrast}
                  onChange={(event) => update({ high_contrast: event.target.checked })}
                />
              </label>

              <label className="flex items-center justify-between gap-2">
                <span className="text-text-secondary">{t("settings.reducedMotion")}</span>
                <input
                  type="checkbox"
                  checked={uiSettings.reduced_motion}
                  onChange={(event) => update({ reduced_motion: event.target.checked })}
                />
              </label>

              <button
                type="button"
                onClick={() => resetWorkspaceLayout()}
                title={t("settings.resetWorkspaceTitle")}
                className="self-start rounded border border-border px-3 py-1 text-text-secondary hover:border-accent hover:text-text-primary"
              >
                {t("settings.resetWorkspace")}
              </button>
            </div>
          ) : tab === "sprache" ? (
            <div className="flex flex-col gap-4 text-xs">
              <label className="flex flex-col gap-1">
                <span className="text-text-secondary">{t("settings.language")}</span>
                <select
                  value={uiSettings.locale}
                  onChange={(event) => update({ locale: event.target.value })}
                  className="rounded border border-border bg-bg-panel px-2 py-1"
                >
                  <option value="de">{t("settings.languageDe")}</option>
                  <option value="en">{t("settings.languageEn")}</option>
                </select>
              </label>
            </div>
          ) : !watchedFolderSettings ? (
            <p className="text-xs text-text-muted">{t("settings.loading")}</p>
          ) : (
            <div className="flex flex-col gap-4 text-xs">
              <label className="flex items-center justify-between gap-2">
                <span className="text-text-secondary">{t("settings.watchedFolderEnabled")}</span>
                <input
                  type="checkbox"
                  checked={watchedFolderSettings.enabled}
                  onChange={(event) => void saveWatchedFolderSettings({ ...watchedFolderSettings, enabled: event.target.checked })}
                />
              </label>

              <label className="flex flex-col gap-1">
                <span className="text-text-secondary">{t("settings.watchedFolderPath")}</span>
                <div className="flex gap-1">
                  <input
                    type="text"
                    readOnly
                    value={watchedFolderSettings.path ?? ""}
                    placeholder={t("settings.watchedFolderChoose")}
                    className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1"
                  />
                  <button type="button" onClick={() => void handlePickWatchedFolder()} className="shrink-0 rounded border border-border px-2 py-1 hover:border-accent">
                    {t("settings.watchedFolderChoose")}
                  </button>
                </div>
              </label>

              <label className="flex flex-col gap-1">
                <span className="text-text-secondary">{t("settings.watchedFolderPollSeconds", { seconds: watchedFolderSettings.poll_seconds })}</span>
                <input
                  type="range"
                  min={5}
                  max={300}
                  step={5}
                  value={watchedFolderSettings.poll_seconds}
                  onChange={(event) => void saveWatchedFolderSettings({ ...watchedFolderSettings, poll_seconds: Number(event.target.value) })}
                />
              </label>

              <p className="text-text-muted">{t("settings.watchedFolderHint")}</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
