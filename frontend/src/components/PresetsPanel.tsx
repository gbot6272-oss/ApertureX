import { useEffect, useState } from "react";

import { buildChildrenByParent } from "../lib/folderTree";
import { useT } from "../lib/i18n";
import { buildPresetEdlSubset, PRESET_SECTION_KEYS } from "../lib/presets";
import type { PresetDto, PresetFolderDto } from "../lib/tauri";
import { selectPresetConditionMeta, useAppStore } from "../store";
import { PaletteFrame } from "./PaletteFrame";
import { PresetThumbnail } from "./PresetThumbnail";
import { PresetVersionsDialog } from "./PresetVersionsDialog";

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
  onOpenVersions: (presetId: string, presetName: string) => void;
}

function PresetRow({ preset, folders, onOpenVersions }: PresetRowProps) {
  const setPresetFavorite = useAppStore((s) => s.setPresetFavorite);
  const renamePreset = useAppStore((s) => s.renamePreset);
  const movePresetToFolder = useAppStore((s) => s.movePresetToFolder);
  const deletePreset = useAppStore((s) => s.deletePreset);
  const applyPreset = useAppStore((s) => s.applyPreset);
  const addPresetToStack = useAppStore((s) => s.addPresetToStack);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const developEdl = useAppStore((s) => s.developEdl);
  const previewPresetHover = useAppStore((s) => s.previewPresetHover);
  const clearPresetHoverPreview = useAppStore((s) => s.clearPresetHoverPreview);
  const photoMeta = useAppStore(selectPresetConditionMeta);
  const exportPresetAsApxFile = useAppStore((s) => s.exportPresetAsApxFile);
  const exportPresetAsLrtemplateFile = useAppStore((s) => s.exportPresetAsLrtemplateFile);

  function handleRename(event: React.MouseEvent) {
    event.stopPropagation();
    const name = window.prompt("Preset umbenennen", preset.name);
    if (name) void renamePreset(preset.id, name);
  }

  return (
    <li
      className="flex flex-col gap-1 rounded border border-border px-2 py-1.5 text-sm"
      onMouseEnter={() => selectedPhotoId && void previewPresetHover(preset.id)}
      onMouseLeave={clearPresetHoverPreview}
    >
      <div className="flex items-center gap-1.5">
        <PresetThumbnail
          presetId={preset.id}
          presetName={preset.name}
          currentEdl={developEdl}
          photoId={selectedPhotoId}
          conditionsJson={preset.conditions_json}
          photoMeta={photoMeta}
        />
        <button
          type="button"
          onClick={() => setPresetFavorite(preset.id, !preset.is_favorite)}
          aria-label={preset.is_favorite ? `${preset.name} aus Favoriten entfernen` : `${preset.name} zu Favoriten hinzufügen`}
          aria-pressed={preset.is_favorite}
          className={`shrink-0 ${preset.is_favorite ? "text-accent" : "text-text-muted hover:text-accent"}`}
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
      </div>
      <div className="flex items-center justify-between gap-1.5 text-xs">
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
        <button
          type="button"
          onClick={() => onOpenVersions(preset.id, preset.name)}
          className="shrink-0 text-text-muted hover:text-accent"
          title="Versionen"
          aria-label={`${preset.name}: Versionen`}
        >
          🕐
        </button>
        <button
          type="button"
          onClick={() => void exportPresetAsApxFile(preset.id)}
          className="shrink-0 text-text-muted hover:text-accent"
          title="Als .apx exportieren"
          aria-label={`${preset.name} als .apx exportieren`}
        >
          ⬇
        </button>
        <button
          type="button"
          onClick={() => void exportPresetAsLrtemplateFile(preset.id)}
          className="shrink-0 text-text-muted hover:text-accent"
          title="Als Adobe .lrtemplate exportieren (nur Basic + HSL, siehe DECISIONS.md ADR-0038)"
          aria-label={`${preset.name} als .lrtemplate exportieren`}
        >
          ⬇LR
        </button>
        <select
          aria-label={`${preset.name}: Ordner`}
          value={preset.folder_id ?? ""}
          onChange={(event) => void movePresetToFolder(preset.id, event.target.value || null)}
          className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-1 py-0.5 text-xs"
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
          className="shrink-0 text-danger underline"
          aria-label={`${preset.name} löschen`}
        >
          Löschen
        </button>
      </div>
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
 * KI-Preset-Generator (Phase 7 Schritt 4, siehe `DECISIONS.md` ADR-0033)
 * — vier unabhängige Erzeugungsarten (LLM-Freitext, Referenzbild,
 * Variationen, Lernen aus mehreren Fotos), jede liefert eine EDL-
 * Teilmenge in `presetGeneratorPreview`. Der erzeugte Vorschlag ist
 * bewusst noch kein Preset: „Auf aktuelles Foto anwenden" mischt ihn nur
 * in `developEdl` (wie das Anwenden eines bestehenden Presets) — der
 * Nutzer sichert ihn danach über den bestehenden „Preset speichern"-
 * Knopf (`DevelopPanel.tsx`), ohne dass der Generator eine eigene
 * Speicher-Logik bräuchte.
 *
 * **Bewusste Vereinfachungen** (siehe `apx-ai::preset_generator`s
 * Moduldoku): Referenzbild-Modus vergleicht nur die sieben Tonwertregler,
 * „Lernen" mittelt nur numerische Werte (Kurvenpunkte/Farbmischer-
 * Regionen werden vom ersten ausgewählten Foto übernommen).
 */
function AiPresetGeneratorSection() {
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const multiSelectedIds = useAppStore((s) => s.multiSelectedIds);
  const developEdl = useAppStore((s) => s.developEdl);
  const aiSettings = useAppStore((s) => s.aiSettings);
  const loadAiSettings = useAppStore((s) => s.loadAiSettings);
  const saveAnthropicApiKey = useAppStore((s) => s.saveAnthropicApiKey);
  const presetGeneratorLoading = useAppStore((s) => s.presetGeneratorLoading);
  const presetGeneratorPreview = useAppStore((s) => s.presetGeneratorPreview);
  const presetGeneratorSelectedIndex = useAppStore((s) => s.presetGeneratorSelectedIndex);
  const generatePresetFromDescription = useAppStore((s) => s.generatePresetFromDescription);
  const copyPresetPromptForClaudeApp = useAppStore((s) => s.copyPresetPromptForClaudeApp);
  const importPresetFromPastedJson = useAppStore((s) => s.importPresetFromPastedJson);
  const generatePresetFromReferenceImage = useAppStore((s) => s.generatePresetFromReferenceImage);
  const generatePresetVariationsFromBase = useAppStore((s) => s.generatePresetVariationsFromBase);
  const learnPresetFromSelectedPhotos = useAppStore((s) => s.learnPresetFromSelectedPhotos);
  const selectPresetGeneratorPreview = useAppStore((s) => s.selectPresetGeneratorPreview);
  const applyPresetGeneratorPreview = useAppStore((s) => s.applyPresetGeneratorPreview);
  const clearPresetGeneratorPreview = useAppStore((s) => s.clearPresetGeneratorPreview);

  const [description, setDescription] = useState("");
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [pastedJson, setPastedJson] = useState("");
  const [promptCopied, setPromptCopied] = useState(false);

  useEffect(() => {
    void loadAiSettings();
    // Nur beim ersten Einblenden laden — `loadAiSettings` ist stabil
    // (Zustand-Aktion), ein erneuter Aufruf bei jedem Tastenanschlag im
    // Schlüsselfeld wäre unnötig.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    setApiKeyInput(aiSettings?.anthropic_api_key ?? "");
  }, [aiSettings]);

  const hasApiKey = Boolean(aiSettings?.anthropic_api_key);

  function handleVariations() {
    const base = buildPresetEdlSubset(developEdl, PRESET_SECTION_KEYS);
    const seed = Math.floor(Math.random() * 1_000_000);
    void generatePresetVariationsFromBase(base, 4, seed);
  }

  async function handleCopyPrompt() {
    await copyPresetPromptForClaudeApp(description);
    setPromptCopied(true);
    setTimeout(() => setPromptCopied(false), 2000);
  }

  async function handleImportPastedJson() {
    if (!pastedJson.trim()) return;
    await importPresetFromPastedJson(pastedJson);
    setPastedJson("");
  }

  return (
    <div className="flex flex-col gap-2 border-t border-border pt-2">
      <h3 className="text-xs font-medium text-text-secondary">KI-Preset-Generator</h3>

      <details className="text-xs text-text-secondary">
        <summary className="cursor-pointer select-none">Anthropic-API-Schlüssel {hasApiKey ? "(hinterlegt)" : "(fehlt)"}</summary>
        <div className="mt-1 flex gap-1">
          <input
            type="password"
            value={apiKeyInput}
            onChange={(event) => setApiKeyInput(event.target.value)}
            placeholder="sk-ant-…"
            aria-label="Anthropic-API-Schlüssel"
            className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
          />
          <button
            type="button"
            onClick={() => void saveAnthropicApiKey(apiKeyInput)}
            className="shrink-0 rounded border border-border px-2 py-1 text-xs hover:border-accent"
          >
            Speichern
          </button>
        </div>
      </details>

      <div className="flex flex-col gap-1">
        <label className="text-xs text-text-secondary" htmlFor="ai-preset-description">
          Beschreibung (LLM-Modus)
        </label>
        <textarea
          id="ai-preset-description"
          value={description}
          onChange={(event) => setDescription(event.target.value)}
          rows={2}
          placeholder="z. B. warmer, kontrastreicher Filmlook"
          className="w-full resize-none rounded border border-border bg-bg-panel px-2 py-1 text-xs"
        />
        <div className="flex gap-1">
          <button
            type="button"
            disabled={!hasApiKey || !description.trim() || presetGeneratorLoading}
            onClick={() => void generatePresetFromDescription(description)}
            title={hasApiKey ? undefined : "Erst einen Anthropic-API-Schlüssel hinterlegen"}
            className="flex-1 rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
          >
            {presetGeneratorLoading ? "Erzeuge…" : "Aus Beschreibung erzeugen"}
          </button>
          <button
            type="button"
            disabled={!description.trim()}
            onClick={() => void handleCopyPrompt()}
            title="Prompt in die Zwischenablage kopieren, zum Einfügen in die Claude-App (claude.ai) — kein API-Schlüssel nötig"
            className="flex-1 rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
          >
            {promptCopied ? "Kopiert!" : "Prompt für Claude-App"}
          </button>
        </div>

        <details className="text-xs text-text-secondary">
          <summary className="cursor-pointer select-none">Antwort aus der Claude-App einfügen (kein API-Schlüssel nötig)</summary>
          <div className="mt-1 flex flex-col gap-1">
            <p className="text-text-muted">
              "Prompt für Claude-App" oben, in <span className="whitespace-nowrap">claude.ai</span> einfügen, Antwort hier zurück einfügen.
            </p>
            <textarea
              value={pastedJson}
              onChange={(event) => setPastedJson(event.target.value)}
              rows={3}
              placeholder='{"basic": {"exposure_ev": 0.5}}'
              aria-label="Aus der Claude-App eingefügte JSON-Antwort"
              className="w-full resize-none rounded border border-border bg-bg-panel px-2 py-1 font-mono text-xs"
            />
            <button
              type="button"
              disabled={!pastedJson.trim() || presetGeneratorLoading}
              onClick={() => void handleImportPastedJson()}
              className="self-start rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
            >
              Übernehmen
            </button>
          </div>
        </details>
      </div>

      <div className="grid grid-cols-2 gap-1">
        <button
          type="button"
          disabled={!selectedPhotoId || presetGeneratorLoading}
          onClick={() => void generatePresetFromReferenceImage()}
          className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
        >
          Referenzbild…
        </button>
        <button
          type="button"
          disabled={presetGeneratorLoading}
          onClick={handleVariations}
          className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
        >
          Variationen
        </button>
        <button
          type="button"
          disabled={multiSelectedIds.length < 2 || presetGeneratorLoading}
          onClick={() => void learnPresetFromSelectedPhotos(multiSelectedIds, [...PRESET_SECTION_KEYS])}
          title={multiSelectedIds.length < 2 ? "Mindestens zwei Fotos in der Filmstreifen-Mehrfachauswahl nötig" : undefined}
          className="col-span-2 rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
        >
          Aus {multiSelectedIds.length} ausgewählten Fotos lernen
        </button>
      </div>

      {presetGeneratorPreview.length > 0 && (
        <div className="flex flex-col gap-1 rounded border border-accent/40 bg-accent/5 p-2">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-text-secondary">Vorschlag{presetGeneratorPreview.length > 1 ? "e" : ""}</span>
            <button type="button" onClick={clearPresetGeneratorPreview} className="text-xs text-text-muted hover:text-danger">
              Verwerfen
            </button>
          </div>
          {presetGeneratorPreview.length > 1 && (
            <div className="flex flex-wrap gap-1">
              {presetGeneratorPreview.map((_, index) => (
                <button
                  key={index}
                  type="button"
                  onClick={() => selectPresetGeneratorPreview(index)}
                  aria-pressed={index === presetGeneratorSelectedIndex}
                  className={`rounded border px-2 py-1 text-xs ${
                    index === presetGeneratorSelectedIndex ? "border-accent bg-accent/20 text-accent" : "border-border text-text-secondary"
                  }`}
                >
                  {index + 1}
                </button>
              ))}
            </div>
          )}
          <button
            type="button"
            disabled={!selectedPhotoId}
            onClick={applyPresetGeneratorPreview}
            className="rounded bg-accent px-2 py-1 text-xs text-white disabled:cursor-not-allowed disabled:opacity-40"
          >
            Auf aktuelles Foto anwenden
          </button>
          <p className="text-xs text-text-muted">Danach über „Preset speichern" im Entwickeln-Panel als echtes Preset sichern.</p>
        </div>
      )}
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
  const t = useT();
  const open = useAppStore((s) => s.developPanelOpen);
  const presetFolders = useAppStore((s) => s.presetFolders);
  const presets = useAppStore((s) => s.presets);
  const selectedPresetFolderId = useAppStore((s) => s.selectedPresetFolderId);
  const selectPresetFolder = useAppStore((s) => s.selectPresetFolder);
  const refreshPresetFolders = useAppStore((s) => s.refreshPresetFolders);
  const refreshPresets = useAppStore((s) => s.refreshPresets);
  const createPresetFolder = useAppStore((s) => s.createPresetFolder);
  const importPresetFromApxFile = useAppStore((s) => s.importPresetFromApxFile);
  const [newFolderName, setNewFolderName] = useState("");
  const [versionsDialog, setVersionsDialog] = useState<{ presetId: string; presetName: string } | null>(null);

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
    <PaletteFrame id="presets" side="left" defaultWidth={256} label={t("presets.heading")} className="gap-3 border-r border-border bg-bg-raised p-3">
      <h2 className="text-sm font-semibold text-text-primary">{t("presets.heading")}</h2>

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

      <button
        type="button"
        onClick={() => void importPresetFromApxFile(selectedPresetFolderId)}
        className="rounded border border-border px-2 py-1 text-left text-xs text-text-secondary hover:bg-bg-panel"
      >
        .apx importieren…
      </button>

      <ul className="flex flex-col gap-1">
        {visiblePresets.map((preset) => (
          <PresetRow
            key={preset.id}
            preset={preset}
            folders={presetFolders}
            onOpenVersions={(presetId, presetName) => setVersionsDialog({ presetId, presetName })}
          />
        ))}
        {visiblePresets.length === 0 && <li className="text-xs text-text-muted">Keine Presets in diesem Ordner.</li>}
      </ul>

      <PresetStackSection />

      <AiPresetGeneratorSection />

      <PresetVersionsDialog
        presetId={versionsDialog?.presetId ?? null}
        presetName={versionsDialog?.presetName ?? ""}
        onClose={() => setVersionsDialog(null)}
      />
    </PaletteFrame>
  );
}
