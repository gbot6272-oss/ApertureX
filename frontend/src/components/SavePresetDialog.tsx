import { useState } from "react";

import { PRESET_SECTION_KEYS, PRESET_SECTION_LABELS } from "../lib/presets";
import type { PresetSectionKey } from "../lib/presets";
import { useAppStore } from "../store";

interface SavePresetDialogProps {
  open: boolean;
  onClose: () => void;
}

/**
 * Preset-Speichern-Dialog (Phase 5 Schritt 4, `SPEC.md` §3.5: „Beim
 * Speichern zeigt ein Dialog jede einzelne Einstellungsgruppe mit
 * Checkbox"). Reparatur ist bewusst keine wählbare Sektion (siehe
 * `lib/presets.ts`s `PresetSectionKey`-Moduldoku).
 */
export function SavePresetDialog({ open, onClose }: SavePresetDialogProps) {
  const presetFolders = useAppStore((s) => s.presetFolders);
  const savePresetFromCurrentEdl = useAppStore((s) => s.savePresetFromCurrentEdl);

  const [name, setName] = useState("");
  const [folderId, setFolderId] = useState<string>("");
  const [tagsText, setTagsText] = useState("");
  const [selectedSections, setSelectedSections] = useState<Set<PresetSectionKey>>(new Set(PRESET_SECTION_KEYS));

  if (!open) return null;

  function toggleSection(key: PresetSectionKey) {
    setSelectedSections((previous) => {
      const next = new Set(previous);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  function reset() {
    setName("");
    setFolderId("");
    setTagsText("");
    setSelectedSections(new Set(PRESET_SECTION_KEYS));
  }

  async function handleSave() {
    const tags = tagsText
      .split(",")
      .map((tag) => tag.trim())
      .filter((tag) => tag.length > 0);
    await savePresetFromCurrentEdl(name, folderId || null, tags, [...selectedSections]);
    reset();
    onClose();
  }

  const canSave = name.trim().length > 0 && selectedSections.size > 0;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-24" onClick={onClose}>
      <div
        role="dialog"
        aria-label="Preset speichern"
        className="w-full max-w-sm rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 className="mb-3 text-sm font-semibold text-text-primary">Preset speichern</h2>

        <label className="mb-2 flex flex-col gap-1 text-xs text-text-secondary">
          Name
          <input
            autoFocus
            type="text"
            value={name}
            onChange={(event) => setName(event.target.value)}
            className="rounded border border-border bg-bg-panel px-2 py-1 text-sm text-text-primary"
          />
        </label>

        <label className="mb-2 flex flex-col gap-1 text-xs text-text-secondary">
          Ordner
          <select
            value={folderId}
            onChange={(event) => setFolderId(event.target.value)}
            className="rounded border border-border bg-bg-panel px-2 py-1 text-sm"
          >
            <option value="">Wurzel</option>
            {presetFolders.map((folder) => (
              <option key={folder.id} value={folder.id}>
                {folder.name}
              </option>
            ))}
          </select>
        </label>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          Tags (durch Komma getrennt)
          <input
            type="text"
            value={tagsText}
            onChange={(event) => setTagsText(event.target.value)}
            placeholder="warm, film"
            className="rounded border border-border bg-bg-panel px-2 py-1 text-sm text-text-primary"
          />
        </label>

        <fieldset className="mb-3 flex flex-col gap-1">
          <legend className="mb-1 text-xs font-medium text-text-secondary">Einstellungsgruppen</legend>
          {PRESET_SECTION_KEYS.map((key) => (
            <label key={key} className="flex items-center gap-2 text-xs text-text-secondary">
              <input type="checkbox" checked={selectedSections.has(key)} onChange={() => toggleSection(key)} />
              {PRESET_SECTION_LABELS[key]}
            </label>
          ))}
        </fieldset>

        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded border border-border px-3 py-1 text-xs text-text-secondary hover:bg-bg-panel">
            Abbrechen
          </button>
          <button
            type="button"
            onClick={() => void handleSave()}
            disabled={!canSave}
            className="rounded bg-accent px-3 py-1 text-xs text-white disabled:cursor-not-allowed disabled:opacity-40"
          >
            Speichern
          </button>
        </div>
      </div>
    </div>
  );
}
