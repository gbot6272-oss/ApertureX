import { useState } from "react";

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
        aria-label="Skript &amp; Plugins"
        className="w-full max-w-lg rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 className="mb-1 text-sm font-semibold text-text-primary">Skript &amp; Plugins</h2>
        <p className="mb-3 text-xs text-text-muted">
          {noPhoto
            ? "Kein Foto im Entwickeln-Panel aktiv."
            : "Wirkt auf das aktuell im Entwickeln-Panel geöffnete Foto."}
        </p>

        <div className="mb-3 flex gap-1 border-b border-border">
          <button
            type="button"
            onClick={() => setTab("script")}
            className={`px-3 py-1.5 text-xs ${
              tab === "script" ? "border-b-2 border-accent text-text-primary" : "text-text-muted"
            }`}
          >
            Skript
          </button>
          <button
            type="button"
            onClick={() => setTab("plugin")}
            className={`px-3 py-1.5 text-xs ${
              tab === "plugin" ? "border-b-2 border-accent text-text-primary" : "text-text-muted"
            }`}
          >
            Plugin
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
            <p className="text-[11px] text-text-muted">
              Nur die Grundeinstellungen sind erreichbar (Belichtung, Kontrast, Weiß/Schwarz,
              Lichter/Tiefen, Klarheit, Dynamik/Sättigung, Farbe/Schwarzweiß-Umschalter) — kein
              Zugriff auf Kurven/HSL/Masken/Objektivkorrekturen.
            </p>
            <div className="flex justify-end">
              <button
                type="button"
                onClick={() => void runDevelopScriptOnCurrent(scriptText)}
                disabled={noPhoto || scriptRunning || scriptText.trim() === ""}
                className="rounded border border-border px-3 py-1.5 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
              >
                Ausführen
              </button>
            </div>
            {scriptRunning && <p className="text-xs text-text-muted">Läuft…</p>}
            {scriptStatus && !scriptRunning && <p className="text-xs text-text-secondary">{scriptStatus}</p>}
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            <label className="text-xs text-text-secondary">
              Plugin-Datei (.so/.dylib/.dll)
              <input
                type="text"
                value={pluginPath}
                onChange={(event) => setPluginPath(event.target.value)}
                placeholder="/pfad/zu/plugin.so"
                className="mt-1 w-full rounded border border-border bg-bg-panel px-2 py-1 text-xs text-text-primary"
              />
            </label>
            <label className="text-xs text-text-secondary">
              Parameter
              <input
                type="number"
                step="0.1"
                value={pluginParam}
                onChange={(event) => setPluginParam(event.target.value)}
                className="mt-1 w-full rounded border border-border bg-bg-panel px-2 py-1 text-xs text-text-primary"
              />
            </label>
            <p className="text-[11px] text-text-muted">
              Die ABI-Version des Plugins wird beim Laden hart geprüft — bei Abweichung wird das
              Plugin abgelehnt statt geraten geladen. Ergebnis wird als neue Datei neben dem
              Original gespeichert, das Katalogfoto bleibt unverändert.
            </p>
            <div className="flex justify-end">
              <button
                type="button"
                onClick={() => void runPluginOnCurrent(pluginPath, Number(pluginParam) || 0)}
                disabled={noPhoto || pluginRunning || pluginPath.trim() === ""}
                className="rounded border border-border px-3 py-1.5 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
              >
                Anwenden
              </button>
            </div>
            {pluginRunning && <p className="text-xs text-text-muted">Läuft…</p>}
            {pluginStatus && !pluginRunning && <p className="text-xs text-text-secondary">{pluginStatus}</p>}
          </div>
        )}

        <div className="mt-3 flex justify-end">
          <button
            type="button"
            onClick={onClose}
            className="rounded border border-border px-3 py-1 text-xs text-text-secondary hover:bg-bg-panel"
          >
            Schließen
          </button>
        </div>
      </div>
    </div>
  );
}
