import { useState } from "react";

import { useT } from "../lib/i18n";
import { useAppStore } from "../store";

interface ScriptPluginDialogProps {
  open: boolean;
  onClose: () => void;
}

/**
 * Skript-API (Rhai) + Plugin-System (Phase 9 Schritt 9, siehe `PLAN.md`,
 * `DECISIONS.md` ADR-0035 Punkt 3) — arbeitet auf `developPhotoId` (dem im
 * Entwickeln-Panel aktiven Foto).
 *
 * **Skript-Tab**: freies Rhai-Skript gegen eine schmale, primitiv-typisierte
 * `edl`-API (`edl.get_exposure()`/`edl.set_exposure(1.5)` etc., siehe
 * `apx_script`s Moduldoku) — committet direkt als neue Bearbeitung, wie jede
 * andere Entwickeln-Änderung.
 *
 * **Plugin-Tab**: lädt eine Plugin-`cdylib` (`.so`/`.dylib`/`.dll`) über
 * `apx-plugin-host`, der die ABI-Version hart prüft (siehe `apx-plugin-abi`s
 * Moduldoku) statt eine unpassende Tabelle zu erraten. Schreibt das Ergebnis
 * als neue PNG-Datei neben dem Original — das Katalogfoto/EDL bleibt
 * unverändert (derselbe Mechanismus wie die KI-Filter aus Schritt 6).
 */
export function ScriptPluginDialog({ open, onClose }: ScriptPluginDialogProps) {
  const t = useT();
  const [tab, setTab] = useState<"script" | "plugin">("script");
  const [scriptText, setScriptText] = useState("edl.set_exposure(edl.get_exposure() + 0.5);");
  const [pluginPath, setPluginPath] = useState("");
  const [pluginParam, setPluginParam] = useState("1.0");

  const developPhotoId = useAppStore((s) => s.developPhotoId);
  const scriptRunning = useAppStore((s) => s.scriptRunning);
  const scriptStatus = useAppStore((s) => s.scriptStatus);
  const runDevelopScriptOnCurrent = useAppStore((s) => s.runDevelopScriptOnCurrent);
  const pluginRunning = useAppStore((s) => s.pluginRunning);
  const pluginStatus = useAppStore((s) => s.pluginStatus);
  const runPluginOnCurrent = useAppStore((s) => s.runPluginOnCurrent);

  if (!open) return null;

  const noPhoto = !developPhotoId;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-24" onClick={onClose}>
      <div
        role="dialog"
        aria-label={t("scriptPluginDialog.title")}
        className="w-full max-w-lg rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 className="mb-1 text-sm font-semibold text-text-primary">{t("scriptPluginDialog.title")}</h2>
        <p className="mb-3 text-xs text-text-muted">
          {noPhoto ? t("scriptPluginDialog.noPhoto") : t("scriptPluginDialog.activePhoto")}
        </p>

        <div className="mb-3 flex gap-1 border-b border-border">
          <button
            type="button"
            onClick={() => setTab("script")}
            className={`px-3 py-1.5 text-xs ${
              tab === "script" ? "border-b-2 border-accent text-text-primary" : "text-text-muted"
            }`}
          >
            {t("scriptPluginDialog.tabScript")}
          </button>
          <button
            type="button"
            onClick={() => setTab("plugin")}
            className={`px-3 py-1.5 text-xs ${
              tab === "plugin" ? "border-b-2 border-accent text-text-primary" : "text-text-muted"
            }`}
          >
            {t("scriptPluginDialog.tabPlugin")}
          </button>
        </div>

        {tab === "script" ? (
          <div className="flex flex-col gap-2">
            <textarea
              value={scriptText}
              onChange={(event) => setScriptText(event.target.value)}
              rows={6}
              spellCheck={false}
              placeholder="edl.set_exposure(edl.get_exposure() + 0.5);"
              className="w-full rounded border border-border bg-bg-panel p-2 font-mono text-xs text-text-primary"
            />
            <p className="text-[11px] text-text-muted">{t("scriptPluginDialog.scriptScopeNote")}</p>
            <div className="flex justify-end">
              <button
                type="button"
                onClick={() => void runDevelopScriptOnCurrent(scriptText)}
                disabled={noPhoto || scriptRunning || scriptText.trim() === ""}
                className="rounded border border-border px-3 py-1.5 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
              >
                {t("scriptPluginDialog.run")}
              </button>
            </div>
            {scriptRunning && <p className="text-xs text-text-muted">{t("scriptPluginDialog.running")}</p>}
            {scriptStatus && !scriptRunning && <p className="text-xs text-text-secondary">{scriptStatus}</p>}
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            <label className="text-xs text-text-secondary">
              {t("scriptPluginDialog.pluginFile")}
              <input
                type="text"
                value={pluginPath}
                onChange={(event) => setPluginPath(event.target.value)}
                placeholder="/pfad/zu/plugin.so"
                className="mt-1 w-full rounded border border-border bg-bg-panel px-2 py-1 text-xs text-text-primary"
              />
            </label>
            <label className="text-xs text-text-secondary">
              {t("scriptPluginDialog.parameter")}
              <input
                type="number"
                step="0.1"
                value={pluginParam}
                onChange={(event) => setPluginParam(event.target.value)}
                className="mt-1 w-full rounded border border-border bg-bg-panel px-2 py-1 text-xs text-text-primary"
              />
            </label>
            <p className="text-[11px] text-text-muted">{t("scriptPluginDialog.pluginAbiNote")}</p>
            <div className="flex justify-end">
              <button
                type="button"
                onClick={() => void runPluginOnCurrent(pluginPath, Number(pluginParam) || 0)}
                disabled={noPhoto || pluginRunning || pluginPath.trim() === ""}
                className="rounded border border-border px-3 py-1.5 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
              >
                {t("scriptPluginDialog.apply")}
              </button>
            </div>
            {pluginRunning && <p className="text-xs text-text-muted">{t("scriptPluginDialog.running")}</p>}
            {pluginStatus && !pluginRunning && <p className="text-xs text-text-secondary">{pluginStatus}</p>}
          </div>
        )}

        <div className="mt-3 flex justify-end">
          <button
            type="button"
            onClick={onClose}
            className="rounded border border-border px-3 py-1 text-xs text-text-secondary hover:bg-bg-panel"
          >
            {t("scriptPluginDialog.close")}
          </button>
        </div>
      </div>
    </div>
  );
}
