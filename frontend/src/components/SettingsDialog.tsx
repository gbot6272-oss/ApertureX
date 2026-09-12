import { useEffect, useState } from "react";

import { useT } from "../lib/i18n";
import { SOUND_PACKS, previewPack } from "../lib/sound";
import { selectFolderDialog, type UiSettingsDto } from "../lib/tauri";
import { resetWorkspaceLayout } from "../lib/workspaceLayout";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

interface SettingsDialogProps {
  open: boolean;
  onClose: () => void;
}

type Tab = "anzeige" | "sprache" | "import" | "karte";

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
  const mapSettings = useAppStore((s) => s.mapSettings);
  const loadMapSettings = useAppStore((s) => s.loadMapSettings);
  const saveMapSettings = useAppStore((s) => s.saveMapSettings);
  const [cartoApiKeyInput, setCartoApiKeyInput] = useState("");

  useEffect(() => {
    if (open && !uiSettings) void loadUiSettings();
  }, [open, uiSettings, loadUiSettings]);

  useEffect(() => {
    if (open && !watchedFolderSettings) void loadWatchedFolderSettings();
  }, [open, watchedFolderSettings, loadWatchedFolderSettings]);

  useEffect(() => {
    if (open && !mapSettings) void loadMapSettings();
  }, [open, mapSettings, loadMapSettings]);

  useEffect(() => {
    setCartoApiKeyInput(mapSettings?.carto_api_key ?? "");
  }, [mapSettings]);

  const tabLabels: Record<Tab, string> = {
    anzeige: t("settings.tab.display"),
    sprache: t("settings.tab.language"),
    import: t("settings.tab.import"),
    karte: t("settings.tab.map"),
  };

  function handleSaveCartoApiKey() {
    void saveMapSettings({ carto_api_key: cartoApiKeyInput.trim() || null });
  }

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
    <Dialog open={open} onClose={onClose} label={t("settings.title")} className="flex max-h-[80vh] max-w-lg flex-col">
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

      {/* `key={tab}` erzwingt eine Neumontage bei jedem Reiterwechsel —
          spielt die Einblend-Animation (`apx-tab-panel-in`, Phase 24,
          siehe DECISIONS.md ADR-0052) jedes Mal frisch ab, statt nur
          beim ersten Mount. */}
      <div key={tab} className="apx-tab-panel-in flex-1 overflow-y-auto p-4">
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

            {/* UI-Sounds (Phase 19, siehe `DECISIONS.md` ADR-0047) —
                bewusst unabhängig von `reduced_motion`: Bewegung und Ton
                sind zwei getrennte Zugänglichkeits-/Geschmacksfragen. */}
            <label className="flex items-center justify-between gap-2">
              <span className="text-text-secondary">{t("settings.soundEnabled")}</span>
              <input
                type="checkbox"
                checked={uiSettings.sound_enabled}
                onChange={(event) => update({ sound_enabled: event.target.checked })}
              />
            </label>

            <label className="flex flex-col gap-1">
              <span className="text-text-secondary">{t("settings.soundVolume", { percent: uiSettings.sound_volume_percent })}</span>
              <input
                type="range"
                min={0}
                max={100}
                step={5}
                value={uiSettings.sound_volume_percent}
                onChange={(event) => update({ sound_volume_percent: Number(event.target.value) })}
              />
            </label>

            <label className="flex flex-col gap-1">
              <span className="text-text-secondary">{t("settings.soundPack")}</span>
              <select
                value={uiSettings.sound_pack}
                onChange={(event) => {
                  const pack = event.target.value;
                  update({ sound_pack: pack });
                  previewPack(pack as (typeof SOUND_PACKS)[number]["id"]);
                }}
                className="rounded border border-border bg-bg-panel px-2 py-1"
              >
                {SOUND_PACKS.map((pack) => (
                  <option key={pack.id} value={pack.id}>
                    {pack.label} — {pack.description}
                  </option>
                ))}
              </select>
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
        ) : tab === "import" ? (
          !watchedFolderSettings ? (
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
          )
        ) : (
          <div className="flex flex-col gap-4 text-xs">
            <label className="flex flex-col gap-1">
              <span className="text-text-secondary">{t("settings.mapApiKey")}</span>
              <div className="flex gap-1">
                <input
                  type="password"
                  value={cartoApiKeyInput}
                  onChange={(event) => setCartoApiKeyInput(event.target.value)}
                  placeholder={t("settings.mapApiKeyPlaceholder")}
                  className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1"
                />
                <button type="button" onClick={handleSaveCartoApiKey} className="shrink-0 rounded border border-border px-2 py-1 hover:border-accent">
                  {t("settings.mapApiKeySave")}
                </button>
              </div>
            </label>
            <p className="text-text-muted">{t("settings.mapApiKeyHint")}</p>
          </div>
        )}
      </div>
    </Dialog>
  );
}
