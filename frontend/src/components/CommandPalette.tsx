import { useEffect, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import { type AiFeatureStatus, useCommandRegistry } from "../lib/commandRegistry";
import { folderLabel } from "../lib/format";
import { useT } from "../lib/i18n";
import { selectActivePhotos, useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

interface PaletteEntry {
  id: string;
  label: string;
  hint?: string;
  run: () => void;
}

/**
 * Vollständige Befehlspalette (Phase 10 Schritt 4, seit Phase 18 Schritt 3
 * lokalisiert und registergespeist, siehe `DECISIONS.md` ADR-0046).
 * Vier Quellen durchsuchbarer Einträge: `useCommandRegistry()` (dieselbe
 * Quelle wie das Overflow-Menü der Kopfleiste — inklusive der neuen
 * "KI-Funktionen"-Kategorie mit Modellstatus-Badge), alle Presets (wendet
 * das Preset auf das aktuelle Entwickeln-Foto an), alle Fotos des aktuell
 * gewählten Ordners (wählt das Foto aus), sowie die bereits vorhandenen
 * Ordner-Einträge.
 */
export function CommandPalette({ open, onClose }: { open: boolean; onClose: () => void }) {
  const t = useT();
  const [query, setQuery] = useState("");
  const folders = useAppStore((s) => s.folders);
  const selectFolder = useAppStore((s) => s.selectFolder);
  const cancelImport = useAppStore((s) => s.cancelImport);
  const importRunning = useAppStore((s) => s.importRunning);
  const requestCommand = useAppStore((s) => s.requestCommand);
  const presets = useAppStore((s) => s.presets);
  const applyPreset = useAppStore((s) => s.applyPreset);
  const photos = useAppStore(useShallow(selectActivePhotos));
  const selectPhoto = useAppStore((s) => s.selectPhoto);
  const commands = useCommandRegistry();

  useEffect(() => {
    if (open) setQuery("");
  }, [open]);

  const aiStatusLabel: Record<AiFeatureStatus, string> = {
    ready: t("commands.ai.status.ready"),
    "download-needed": t("commands.ai.status.downloadNeeded"),
    "not-available": t("commands.ai.status.notAvailable"),
  };

  const entries = useMemo<PaletteEntry[]>(() => {
    const commandEntries: PaletteEntry[] = importRunning ? [{ id: "cmd:cancel-import", label: t("header.cancelImport"), run: () => void cancelImport() }] : [];

    const functionEntries: PaletteEntry[] = [
      { id: "fn:import", label: t("header.importFolder"), run: () => requestCommand("import") },
      ...commands
        .filter((entry) => !entry.disabled)
        .map((entry) => ({
          id: entry.id,
          label: entry.label,
          hint: entry.aiStatus ? aiStatusLabel[entry.aiStatus] : undefined,
          run: entry.run,
        })),
    ];

    const presetEntries: PaletteEntry[] = presets.map((preset) => ({
      id: `preset:${preset.id}`,
      label: preset.name,
      hint: "Preset anwenden",
      run: () => void applyPreset(preset.id),
    }));

    const photoEntries: PaletteEntry[] = photos.map((photo) => ({
      id: `photo:${photo.id}`,
      label: photo.filename,
      hint: "Foto auswählen",
      run: () => selectPhoto(photo.id),
    }));

    const folderEntries: PaletteEntry[] = folders.map((folder) => ({
      id: `folder:${folder.id}`,
      label: folderLabel(folder.path),
      hint: folder.path,
      run: () => selectFolder(folder.id),
    }));

    return [...commandEntries, ...functionEntries, ...presetEntries, ...photoEntries, ...folderEntries];
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [folders, importRunning, selectFolder, cancelImport, requestCommand, commands, presets, applyPreset, photos, selectPhoto, t]);

  const filtered = entries.filter((entry) => entry.label.toLowerCase().includes(query.toLowerCase()));

  return (
    <Dialog open={open} onClose={onClose} label="Befehlspalette" className="max-w-lg">
      <input
        autoFocus
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Escape") onClose();
          if (event.key === "Enter" && filtered[0]) {
            filtered[0].run();
            onClose();
          }
        }}
        placeholder="Befehl, Preset, Foto oder Ordner suchen…"
        className="w-full border-b border-border bg-transparent px-4 py-3 text-sm outline-none placeholder:text-text-muted"
      />
      <ul className="max-h-80 overflow-y-auto p-1">
        {filtered.length === 0 && <li className="px-3 py-2 text-sm text-text-muted">Keine Treffer.</li>}
        {filtered.slice(0, 50).map((entry) => (
          <li key={entry.id}>
            <button
              type="button"
              onClick={() => {
                entry.run();
                onClose();
              }}
              className="flex w-full items-center justify-between gap-2 rounded px-3 py-1.5 text-left text-sm hover:bg-bg-panel"
            >
              <span className="truncate">{entry.label}</span>
              {entry.hint && <span className="shrink-0 text-xs text-text-muted">{entry.hint}</span>}
            </button>
          </li>
        ))}
      </ul>
    </Dialog>
  );
}
