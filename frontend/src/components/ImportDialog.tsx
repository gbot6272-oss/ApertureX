import { useEffect, useState } from "react";

import { RENAME_PATTERN_TOKENS, previewRenamePattern } from "../lib/renamePattern";
import { importPresetModeToImportModeDto, selectFolderDialog } from "../lib/tauri";
import type { ImportPresetDto, ImportPresetModeDto } from "../lib/tauri";
import { useAppStore } from "../store";

interface ImportDialogProps {
  open: boolean;
  sourcePath: string;
  onClose: () => void;
}

type ModeChoice = "AddInPlace" | "Copy" | "Move";

function modeChoiceToPresetMode(choice: ModeChoice, targetDir: string): ImportPresetModeDto {
  if (choice === "AddInPlace") return { mode: "AddInPlace" };
  return { mode: choice, target_dir: targetDir };
}

/**
 * Erweiterter Import-Dialog (Phase 5 Schritt 9, `DECISIONS.md` ADR-0031
 * Punkt 7 — Frontend-Anbindung des seit Phase 3 im Backend bestehenden,
 * bis dahin ungenutzten Kopieren-/Verschieben-Modus samt
 * Umbenennungsmuster und Presets). Additiv zum einfachen
 * „Ordner importieren"-Knopf in `Header.tsx` (der weiterhin unverändert
 * sofort mit `import_folder`/Hinzufügen-an-Ort-und-Stelle importiert) —
 * dieser Dialog öffnet sich über einen separaten „Import mit Vorlage…"-
 * Knopf und lässt Modus, Zielordner und Umbenennungsmuster wählen sowie
 * als benanntes Preset speichern/wiederverwenden.
 */
export function ImportDialog({ open, sourcePath, onClose }: ImportDialogProps) {
  const importPresets = useAppStore((s) => s.importPresets);
  const refreshImportPresets = useAppStore((s) => s.refreshImportPresets);
  const saveImportPresetEntry = useAppStore((s) => s.saveImportPresetEntry);
  const deleteImportPresetEntry = useAppStore((s) => s.deleteImportPresetEntry);
  const startImportWithMode = useAppStore((s) => s.startImportWithMode);

  const [modeChoice, setModeChoice] = useState<ModeChoice>("AddInPlace");
  const [targetDir, setTargetDir] = useState("");
  const [renamePattern, setRenamePattern] = useState("");
  const [presetName, setPresetName] = useState("");
  const [selectedPresetName, setSelectedPresetName] = useState("");

  useEffect(() => {
    if (!open) return;
    void refreshImportPresets();
  }, [open, refreshImportPresets]);

  if (!open) return null;

  function insertToken(token: string) {
    setRenamePattern((previous) => `${previous}${token}`);
  }

  async function handlePickTargetDir() {
    const path = await selectFolderDialog();
    if (path) setTargetDir(path);
  }

  function applyPreset(name: string) {
    setSelectedPresetName(name);
    const preset = importPresets.find((p) => p.name === name);
    if (!preset) return;
    setModeChoice(preset.mode.mode);
    setTargetDir(preset.mode.mode === "AddInPlace" ? "" : preset.mode.target_dir);
    setRenamePattern(preset.rename_pattern ?? "");
  }

  async function handleSavePreset() {
    const trimmed = presetName.trim();
    if (!trimmed) return;
    const preset: ImportPresetDto = {
      name: trimmed,
      mode: modeChoiceToPresetMode(modeChoice, targetDir),
      rename_pattern: renamePattern.trim() ? renamePattern.trim() : null,
    };
    await saveImportPresetEntry(preset);
    setPresetName("");
  }

  async function handleImport() {
    const mode = importPresetModeToImportModeDto(modeChoiceToPresetMode(modeChoice, targetDir));
    await startImportWithMode(sourcePath, mode, renamePattern.trim() ? renamePattern.trim() : null);
    onClose();
  }

  const canImport = modeChoice === "AddInPlace" || targetDir.trim().length > 0;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-24" onClick={onClose}>
      <div
        role="dialog"
        aria-label="Import mit Vorlage"
        className="w-full max-w-md rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 className="mb-1 text-sm font-semibold text-text-primary">Import mit Vorlage</h2>
        <p className="mb-3 truncate text-xs text-text-muted" title={sourcePath}>
          Quelle: {sourcePath}
        </p>

        {importPresets.length > 0 && (
          <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
            Gespeichertes Preset anwenden
            <div className="flex gap-1">
              <select
                aria-label="Import-Preset"
                value={selectedPresetName}
                onChange={(event) => applyPreset(event.target.value)}
                className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-sm"
              >
                <option value="">— auswählen —</option>
                {importPresets.map((preset) => (
                  <option key={preset.name} value={preset.name}>
                    {preset.name}
                  </option>
                ))}
              </select>
              {selectedPresetName && (
                <button
                  type="button"
                  onClick={() => void deleteImportPresetEntry(selectedPresetName).then(() => setSelectedPresetName(""))}
                  className="shrink-0 rounded border border-border px-2 py-1 text-xs text-danger hover:bg-danger/10"
                  aria-label={`Preset ${selectedPresetName} löschen`}
                >
                  Löschen
                </button>
              )}
            </div>
          </label>
        )}

        <fieldset className="mb-3 flex flex-col gap-1">
          <legend className="mb-1 text-xs font-medium text-text-secondary">Modus</legend>
          {(["AddInPlace", "Copy", "Move"] as const).map((choice) => (
            <label key={choice} className="flex items-center gap-2 text-xs text-text-secondary">
              <input type="radio" name="import-mode" checked={modeChoice === choice} onChange={() => setModeChoice(choice)} />
              {choice === "AddInPlace" ? "An Ort und Stelle hinzufügen" : choice === "Copy" ? "Kopieren nach…" : "Verschieben nach…"}
            </label>
          ))}
        </fieldset>

        {modeChoice !== "AddInPlace" && (
          <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
            Zielordner
            <div className="flex gap-1">
              <input
                type="text"
                readOnly
                value={targetDir}
                aria-label="Zielordner"
                className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-sm"
              />
              <button
                type="button"
                onClick={() => void handlePickTargetDir()}
                className="shrink-0 rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel"
              >
                Wählen…
              </button>
            </div>
          </label>
        )}

        <label className="mb-1 flex flex-col gap-1 text-xs text-text-secondary">
          Umbenennungsmuster (optional)
          <input
            type="text"
            value={renamePattern}
            onChange={(event) => setRenamePattern(event.target.value)}
            placeholder="{date}_{seq}_{camera}"
            aria-label="Umbenennungsmuster"
            className="rounded border border-border bg-bg-panel px-2 py-1 text-sm font-mono text-text-primary"
          />
        </label>
        <div className="mb-1 flex flex-wrap gap-1">
          {RENAME_PATTERN_TOKENS.map(({ token, label }) => (
            <button
              key={token}
              type="button"
              onClick={() => insertToken(token)}
              title={label}
              className="rounded border border-border px-1.5 py-0.5 font-mono text-xs text-text-secondary hover:bg-bg-panel"
            >
              {token}
            </button>
          ))}
        </div>
        {renamePattern.trim() && (
          <p className="mb-3 text-xs text-text-muted">
            Vorschau: <span className="font-mono text-text-secondary">{previewRenamePattern(renamePattern)}</span>
          </p>
        )}

        <div className="mb-4 flex items-end gap-1 border-t border-border pt-3">
          <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
            Als Preset speichern
            <input
              type="text"
              value={presetName}
              onChange={(event) => setPresetName(event.target.value)}
              placeholder="Preset-Name"
              aria-label="Preset-Name"
              className="rounded border border-border bg-bg-panel px-2 py-1 text-sm text-text-primary"
            />
          </label>
          <button
            type="button"
            onClick={() => void handleSavePreset()}
            disabled={!presetName.trim()}
            className="shrink-0 rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
          >
            Speichern
          </button>
        </div>

        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded border border-border px-3 py-1 text-xs text-text-secondary hover:bg-bg-panel">
            Abbrechen
          </button>
          <button
            type="button"
            onClick={() => void handleImport()}
            disabled={!canImport}
            className="rounded bg-accent px-3 py-1 text-xs text-white disabled:cursor-not-allowed disabled:opacity-40"
          >
            Importieren
          </button>
        </div>
      </div>
    </div>
  );
}
