import { useEffect, useState } from "react";

import { buildChildrenByParent } from "../lib/folderTree";
import type { PresetDto, PresetFolderDto } from "../lib/tauri";
import { useAppStore } from "../store";

interface PresetFolderNodeProps {
  folder: PresetFolderDto;
  depth: number;
  childrenOf: Map<string, PresetFolderDto[]>;
}

function PresetFolderNode({ folder, depth, childrenOf }: PresetFolderNodeProps) {
  const selectedPresetFolderId = useAppStore((s) => s.selectedPresetFolderId);
  const selectPresetFolder = useAppStore((s) => s.selectPresetFolder);
  const renamePresetFolder = useAppStore((s) => s.renamePresetFolder);
  const deletePresetFolder = useAppStore((s) => s.deletePresetFolder);
  const children = childrenOf.get(folder.id) ?? [];

  function handleRename(event: React.MouseEvent) {
    event.stopPropagation();
    const name = window.prompt("Ordner umbenennen", folder.name);
    if (name) void renamePresetFolder(folder.id, name);
  }

  function handleDelete(event: React.MouseEvent) {
    event.stopPropagation();
    void deletePresetFolder(folder.id);
  }

  return (
    <li>
      <button
        type="button"
        onClick={() => selectPresetFolder(folder.id)}
        style={{ paddingLeft: `${0.5 + depth * 1}rem` }}
        className={`flex w-full items-center justify-between rounded px-2 py-1.5 text-left text-sm hover:bg-bg-panel ${
          folder.id === selectedPresetFolderId ? "bg-bg-panel text-text-primary" : "text-text-secondary"
        }`}
      >
        <span className="truncate">{folder.name}</span>
        <span className="ml-2 flex shrink-0 items-center gap-2 text-xs">
          <span role="button" tabIndex={0} onClick={handleRename} className="text-text-muted hover:text-accent" title="Ordner umbenennen">
            ✎
          </span>
          <span role="button" tabIndex={0} onClick={handleDelete} className="text-text-muted hover:text-danger" title="Ordner löschen">
            ×
          </span>
        </span>
      </button>
      {children.length > 0 && (
        <ul className="space-y-0.5">
          {children.map((child) => (
            <PresetFolderNode key={child.id} folder={child} depth={depth + 1} childrenOf={childrenOf} />
          ))}
        </ul>
      )}
    </li>
  );
}

interface PresetRowProps {
  preset: PresetDto;
  folders: PresetFolderDto[];
}

function PresetRow({ preset, folders }: PresetRowProps) {
  const setPresetFavorite = useAppStore((s) => s.setPresetFavorite);
  const renamePreset = useAppStore((s) => s.renamePreset);
  const movePresetToFolder = useAppStore((s) => s.movePresetToFolder);
  const deletePreset = useAppStore((s) => s.deletePreset);
  const applyPreset = useAppStore((s) => s.applyPreset);
  const addPresetToStack = useAppStore((s) => s.addPresetToStack);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);

  function handleRename(event: React.MouseEvent) {
    event.stopPropagation();
    const name = window.prompt("Preset umbenennen", preset.name);
    if (name) void renamePreset(preset.id, name);
  }

  return (
    <li className="flex items-center justify-between gap-1.5 rounded border border-border px-2 py-1.5 text-sm">
      <button
        type="button"
        onClick={() => setPresetFavorite(preset.id, !preset.is_favorite)}
        aria-label={preset.is_favorite ? `${preset.name} aus Favoriten entfernen` : `${preset.name} zu Favoriten hinzufügen`}
        aria-pressed={preset.is_favorite}
        className={preset.is_favorite ? "text-accent" : "text-text-muted hover:text-accent"}
        title="Favorit"
      >
        {preset.is_favorite ? "★" : "☆"}
      </button>
      <button
        type="button"
        onClick={() => void applyPreset(preset.id)}
        disabled={!selectedPhotoId}
        className="min-w-0 flex-1 truncate text-left text-text-primary hover:underline disabled:cursor-not-allowed disabled:opacity-40"
        title="Preset anwenden"
      >
        {preset.name}
      </button>
      <span role="button" tabIndex={0} onClick={handleRename} className="shrink-0 text-text-muted hover:text-accent" title="Umbenennen">
        ✎
      </span>
      <button
        type="button"
        onClick={() => addPresetToStack(preset.id)}
        className="shrink-0 text-text-muted hover:text-accent"
        title="Zum Preset-Stapel hinzufügen"
        aria-label={`${preset.name} zum Stapel hinzufügen`}
      >
        ➕
      </button>
      <select
        aria-label={`${preset.name}: Ordner`}
        value={preset.folder_id ?? ""}
        onChange={(event) => void movePresetToFolder(preset.id, event.target.value || null)}
        className="shrink-0 rounded border border-border bg-bg-panel px-1 py-0.5 text-xs"
      >
        <option value="">Wurzel</option>
        {folders.map((folder) => (
          <option key={folder.id} value={folder.id}>
            {folder.name}
          </option>
        ))}
      </select>
      <button
        type="button"
        onClick={() => void deletePreset(preset.id)}
        className="shrink-0 text-xs text-danger underline"
        aria-label={`${preset.name} löschen`}
      >
        Löschen
      </button>
    </li>
  );
}

/** Preset-Stapel (`SPEC.md` §3.5): eine geordnete Liste ausgewählter
 * Presets, die auf einen Klick nacheinander angewendet werden — jedes
 * bei 100 % Stärke, spätere Einträge überschreiben gemeinsame Sektionen
 * früherer. */
function PresetStackSection() {
  const presetStack = useAppStore((s) => s.presetStack);
  const presets = useAppStore((s) => s.presets);
  const removePresetFromStack = useAppStore((s) => s.removePresetFromStack);
  const movePresetInStack = useAppStore((s) => s.movePresetInStack);
  const clearPresetStack = useAppStore((s) => s.clearPresetStack);
  const applyPresetStack = useAppStore((s) => s.applyPresetStack);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);

  if (presetStack.length === 0) return null;

  return (
    <div className="flex flex-col gap-1 border-t border-border pt-2">
      <h3 className="text-xs font-medium text-text-secondary">Preset-Stapel</h3>
      <ul className="flex flex-col gap-1">
        {presetStack.map((presetId, index) => {
          const preset = presets.find((p) => p.id === presetId);
          return (
            <li key={`${presetId}-${index}`} className="flex items-center justify-between gap-1 rounded border border-border px-2 py-1 text-xs">
              <span className="min-w-0 flex-1 truncate">
                {index + 1}. {preset?.name ?? presetId}
              </span>
              <button type="button" onClick={() => movePresetInStack(index, -1)} disabled={index === 0} className="disabled:opacity-30" aria-label="Nach oben">
                ↑
              </button>
              <button
                type="button"
                onClick={() => movePresetInStack(index, 1)}
                disabled={index === presetStack.length - 1}
                className="disabled:opacity-30"
                aria-label="Nach unten"
              >
                ↓
              </button>
              <button type="button" onClick={() => removePresetFromStack(index)} className="text-danger" aria-label="Aus dem Stapel entfernen">
                ×
              </button>
            </li>
          );
        })}
      </ul>
      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => void applyPresetStack()}
          disabled={!selectedPhotoId}
          className="rounded bg-accent px-2 py-1 text-xs text-white disabled:cursor-not-allowed disabled:opacity-40"
        >
          Stapel anwenden
        </button>
        <button type="button" onClick={clearPresetStack} className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel">
          Leeren
        </button>
      </div>
    </div>
  );
}

/**
 * Presets-Grundgerüst (Phase 5 Schritt 3, siehe `DECISIONS.md` ADR-0031):
 * Ordnerbaum (analog zu `Sidebar.tsx`s Ordnerbaum) + Presetliste, gefiltert
 * auf den ausgewählten Ordner. Anlegen eines neuen Presets aus dem
 * aktuellen Entwickeln-Zustand ist `SavePresetDialog` (Schritt 4)
 * vorbehalten — dieses Panel organisiert nur bereits gespeicherte Presets.
 * Wie `DevelopPanel` nur sichtbar, während das Entwickeln-Panel offen ist.
 */
export function PresetsPanel() {
  const open = useAppStore((s) => s.developPanelOpen);
  const presetFolders = useAppStore((s) => s.presetFolders);
  const presets = useAppStore((s) => s.presets);
  const selectedPresetFolderId = useAppStore((s) => s.selectedPresetFolderId);
  const selectPresetFolder = useAppStore((s) => s.selectPresetFolder);
  const refreshPresetFolders = useAppStore((s) => s.refreshPresetFolders);
  const refreshPresets = useAppStore((s) => s.refreshPresets);
  const createPresetFolder = useAppStore((s) => s.createPresetFolder);
  const [newFolderName, setNewFolderName] = useState("");

  useEffect(() => {
    if (!open) return;
    void refreshPresetFolders();
    void refreshPresets();
  }, [open, refreshPresetFolders, refreshPresets]);

  if (!open) return null;

  const { roots, childrenOf } = buildChildrenByParent(presetFolders);
  const visiblePresets = presets.filter((preset) => preset.folder_id === selectedPresetFolderId);

  function handleCreateFolder(event: React.FormEvent) {
    event.preventDefault();
    const name = newFolderName.trim();
    if (!name) return;
    void createPresetFolder(name, selectedPresetFolderId);
    setNewFolderName("");
  }

  return (
    <aside className="flex w-64 shrink-0 flex-col gap-3 overflow-y-auto border-r border-border bg-bg-raised p-3">
      <h2 className="text-sm font-semibold text-text-primary">Presets</h2>

      <ul className="space-y-0.5">
        <li>
          <button
            type="button"
            onClick={() => selectPresetFolder(null)}
            className={`w-full rounded px-2 py-1.5 text-left text-sm hover:bg-bg-panel ${
              selectedPresetFolderId === null ? "bg-bg-panel text-text-primary" : "text-text-secondary"
            }`}
          >
            Wurzel
          </button>
        </li>
        {roots.map((folder) => (
          <PresetFolderNode key={folder.id} folder={folder} depth={1} childrenOf={childrenOf} />
        ))}
      </ul>

      <form onSubmit={handleCreateFolder} className="flex gap-1">
        <input
          type="text"
          value={newFolderName}
          onChange={(event) => setNewFolderName(event.target.value)}
          placeholder="Neuer Ordner…"
          aria-label="Neuer Preset-Ordner"
          className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
        />
        <button
          type="submit"
          aria-label="Ordner anlegen"
          className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel"
        >
          +
        </button>
      </form>

      <ul className="flex flex-col gap-1">
        {visiblePresets.map((preset) => (
          <PresetRow key={preset.id} preset={preset} folders={presetFolders} />
        ))}
        {visiblePresets.length === 0 && <li className="text-xs text-text-muted">Keine Presets in diesem Ordner.</li>}
      </ul>

      <PresetStackSection />
    </aside>
  );
}
