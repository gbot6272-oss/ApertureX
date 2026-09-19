import { AlertTriangle, Check, Plus, Save, Trash2, X } from "lucide-react";
import { useEffect, useState } from "react";

import type { MetadataPreset } from "../lib/tauri";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

/**
 * Metadaten-Vorgaben (Phase 33 F4).
 *
 * **Die Entscheidung, die den Dialog erklärt:** jedes der vier
 * IPTC-Felder hat einen eigenen Haken „übernehmen". Ohne den könnte ein
 * leeres Eingabefeld zweierlei heißen — „lass das Feld in Ruhe" oder
 * „leere es" —, und die Vorgabe, die Urheber und Copyright nachträgt,
 * würde beim ersten Anwenden alle einzeln geschriebenen
 * Bildunterschriften mitnehmen. Ohne Haken bleibt das Feld unberührt;
 * mit Haken und leerem Text wird es geleert.
 *
 * Gespeichert wird als gewöhnliche Vorlage der Art `"metadata"` — der
 * Katalog hat dafür längst einen Ablageort, siehe
 * `crates/apx-app/src/metadata_preset.rs`.
 */

const PLACEHOLDERS = ["{year}", "{camera}", "{lens}", "{filename}", "{stem}"];

interface FieldRowProps {
  label: string;
  value: string | null;
  onChange: (value: string | null) => void;
}

/** Ein IPTC-Feld mit seinem „übernehmen"-Haken. */
function FieldRow({ label, value, onChange }: FieldRowProps) {
  const enabled = value !== null;
  return (
    <div className="flex items-center gap-2">
      <label className="flex w-40 shrink-0 items-center gap-1.5 text-xs text-text-secondary">
        <input
          type="checkbox"
          checked={enabled}
          onChange={(event) => onChange(event.target.checked ? "" : null)}
          aria-label={`${label} übernehmen`}
        />
        {label}
      </label>
      <input
        type="text"
        value={value ?? ""}
        disabled={!enabled}
        onChange={(event) => onChange(event.target.value)}
        aria-label={label}
        placeholder={enabled ? "leer lassen = Feld leeren" : "wird nicht angefasst"}
        className="flex-1 rounded border border-border bg-bg-base px-2 py-1 text-xs text-text-primary disabled:opacity-40"
      />
    </div>
  );
}

interface MetadataPresetDialogProps {
  open: boolean;
  onClose: () => void;
}

export function MetadataPresetDialog({ open, onClose }: MetadataPresetDialogProps) {
  const presets = useAppStore((s) => s.metadataPresets);
  const draft = useAppStore((s) => s.metadataPresetDraft);
  const running = useAppStore((s) => s.metadataPresetRunning);
  const result = useAppStore((s) => s.metadataPresetResult);
  const error = useAppStore((s) => s.metadataPresetError);
  const refresh = useAppStore((s) => s.refreshMetadataPresets);
  const setDraft = useAppStore((s) => s.setMetadataPresetDraft);
  const loadDraft = useAppStore((s) => s.loadMetadataPresetDraft);
  const saveDraft = useAppStore((s) => s.saveMetadataPresetDraft);
  const deletePreset = useAppStore((s) => s.deleteMetadataPreset);
  const apply = useAppStore((s) => s.applyMetadataPresetToSelection);
  const multiSelectedIds = useAppStore((s) => s.multiSelectedIds);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);

  const [presetName, setPresetName] = useState("");
  const [keywordDraft, setKeywordDraft] = useState("");
  const [customKey, setCustomKey] = useState("");
  const [customValue, setCustomValue] = useState("");

  const selectionSize = multiSelectedIds.length > 0 ? multiSelectedIds.length : selectedPhotoId ? 1 : 0;

  useEffect(() => {
    if (open) void refresh();
  }, [open, refresh]);

  const patch = (change: Partial<MetadataPreset>) => setDraft(change);

  const addKeyword = () => {
    const value = keywordDraft.trim();
    if (!value || draft.keywords.includes(value)) {
      setKeywordDraft("");
      return;
    }
    patch({ keywords: [...draft.keywords, value] });
    setKeywordDraft("");
  };

  const addCustom = () => {
    const key = customKey.trim();
    if (!key) return;
    patch({ custom: { ...draft.custom, [key]: customValue } });
    setCustomKey("");
    setCustomValue("");
  };

  return (
    <Dialog
      open={open}
      onClose={onClose}
      label="Metadaten-Vorgaben"
      className="flex max-h-[85vh] w-[50rem] max-w-[94vw] flex-col"
    >
      <div className="flex min-h-0 flex-col gap-3 p-4">
        <div>
          <h2 className="text-sm font-semibold text-text-primary">Metadaten-Vorgaben</h2>
          <p className="mt-0.5 text-xs text-text-muted" data-testid="metadata-preset-scope">
            {selectionSize === 0
              ? "Kein Foto ausgewählt."
              : `${selectionSize} ${selectionSize === 1 ? "Foto" : "Fotos"} ausgewählt`}
          </p>
        </div>

        {error && (
          <p className="flex items-start gap-2 rounded border border-danger/40 bg-danger/10 px-2 py-1.5 text-xs text-danger" role="alert">
            <AlertTriangle aria-hidden="true" className="mt-0.5 size-3.5 shrink-0" />
            {error}
          </p>
        )}

        {result && (
          <p className="rounded border border-success/40 bg-success/10 px-2 py-1.5 text-xs text-success" data-testid="metadata-preset-result">
            {result.photos} {result.photos === 1 ? "Foto" : "Fotos"} beschriftet · {result.keywords_set} Stichwörter vergeben
            {result.keywords_removed > 0 ? ` · ${result.keywords_removed} entfernt` : ""}
          </p>
        )}

        <div className="grid min-h-0 flex-1 grid-cols-[14rem_1fr] gap-3 overflow-hidden">
          <aside className="flex min-h-0 flex-col gap-2 overflow-y-auto border-r border-border pr-3">
            <h3 className="text-xs font-semibold text-text-secondary">Gespeicherte Vorgaben</h3>
            {presets.length === 0 && (
              <p className="text-[11px] text-text-muted">
                Noch keine. Felder ausfüllen, Namen vergeben, speichern — danach steht die Vorgabe hier.
              </p>
            )}
            <ul className="flex flex-col gap-1" data-testid="metadata-preset-list">
              {presets.map((preset) => (
                <li key={preset.id} className="flex items-center gap-1">
                  <button
                    type="button"
                    onClick={() => loadDraft(preset.id)}
                    className="apx-btn-liquid min-w-0 flex-1 truncate rounded border border-border px-2 py-1 text-left text-[11px] text-text-primary"
                  >
                    {preset.name}
                  </button>
                  <button
                    type="button"
                    onClick={() => void deletePreset(preset.id)}
                    aria-label={`Vorgabe ${preset.name} löschen`}
                    className="apx-btn-liquid rounded border border-border p-1 text-text-muted"
                  >
                    <Trash2 aria-hidden="true" className="size-3" />
                  </button>
                </li>
              ))}
            </ul>

            <div className="mt-auto flex flex-col gap-1 pt-2">
              <input
                type="text"
                value={presetName}
                onChange={(event) => setPresetName(event.target.value)}
                aria-label="Name der Vorgabe"
                placeholder="Name der Vorgabe"
                className="rounded border border-border bg-bg-base px-2 py-1 text-[11px] text-text-primary"
              />
              <button
                type="button"
                onClick={() => {
                  void saveDraft(presetName);
                  setPresetName("");
                }}
                disabled={!presetName.trim()}
                // Die Filterleiste hat ebenfalls ein „Speichern" —
                // ein eigenes Label hält beide auseinander.
                aria-label="Vorgabe speichern"
                className="apx-btn-liquid flex items-center justify-center gap-1.5 rounded border border-border px-2 py-1 text-[11px] text-text-secondary disabled:opacity-40"
              >
                <Save aria-hidden="true" className="size-3" />
                Speichern
              </button>
            </div>
          </aside>

          <div className="flex min-h-0 flex-col gap-3 overflow-y-auto pr-1">
            <section className="flex flex-col gap-1.5">
              <FieldRow label="Titel" value={draft.title} onChange={(value) => patch({ title: value })} />
              <FieldRow label="Bildunterschrift" value={draft.caption} onChange={(value) => patch({ caption: value })} />
              <FieldRow label="Copyright" value={draft.copyright} onChange={(value) => patch({ copyright: value })} />
              <FieldRow label="Urheber" value={draft.creator} onChange={(value) => patch({ creator: value })} />
              <p className="text-[11px] text-text-muted">
                Ohne Haken bleibt das Feld unberührt. Mit Haken und leerem Text wird es geleert. Platzhalter:{" "}
                {PLACEHOLDERS.join(", ")} — unbekannte bleiben wörtlich stehen.
              </p>
            </section>

            <section>
              <h3 className="mb-1 text-xs font-semibold text-text-secondary">Stichwörter</h3>
              <div className="flex flex-wrap gap-1" data-testid="metadata-preset-keywords">
                {draft.keywords.map((keyword) => (
                  <span key={keyword} className="flex items-center gap-1 rounded bg-bg-raised px-1.5 py-0.5 text-[11px] text-text-primary">
                    {keyword}
                    <button
                      type="button"
                      onClick={() => patch({ keywords: draft.keywords.filter((entry) => entry !== keyword) })}
                      aria-label={`Stichwort ${keyword} entfernen`}
                      className="text-text-muted"
                    >
                      <X aria-hidden="true" className="size-3" />
                    </button>
                  </span>
                ))}
              </div>
              <div className="mt-1.5 flex items-center gap-2">
                <input
                  type="text"
                  value={keywordDraft}
                  onChange={(event) => setKeywordDraft(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.preventDefault();
                      addKeyword();
                    }
                  }}
                  aria-label="Stichwort hinzufügen"
                  placeholder="Stichwort"
                  className="w-48 rounded border border-border bg-bg-base px-2 py-1 text-xs text-text-primary"
                />
                <button
                  type="button"
                  onClick={addKeyword}
                  className="apx-btn-liquid rounded border border-border p-1 text-text-secondary"
                  aria-label="Stichwort übernehmen"
                >
                  <Plus aria-hidden="true" className="size-3.5" />
                </button>
                <label className="flex items-center gap-1.5 text-xs text-text-secondary">
                  <input
                    type="checkbox"
                    checked={draft.keyword_mode === "replace"}
                    onChange={(event) => patch({ keyword_mode: event.target.checked ? "replace" : "add" })}
                    aria-label="Vorhandene Stichwörter ersetzen"
                  />
                  Vorhandene ersetzen
                </label>
              </div>
            </section>

            <section>
              <h3 className="mb-1 text-xs font-semibold text-text-secondary">Zusatzfelder</h3>
              <ul className="flex flex-col gap-0.5" data-testid="metadata-preset-custom">
                {Object.entries(draft.custom).map(([key, value]) => (
                  <li key={key} className="flex items-center gap-2 text-[11px]">
                    <span className="w-32 shrink-0 truncate text-text-secondary">{key}</span>
                    <span className="min-w-0 flex-1 truncate text-text-primary">{value || "(leer = löschen)"}</span>
                    <button
                      type="button"
                      onClick={() => {
                        const next = { ...draft.custom };
                        delete next[key];
                        patch({ custom: next });
                      }}
                      aria-label={`Zusatzfeld ${key} entfernen`}
                      className="text-text-muted"
                    >
                      <X aria-hidden="true" className="size-3" />
                    </button>
                  </li>
                ))}
              </ul>
              <div className="mt-1.5 flex items-center gap-2">
                <input
                  type="text"
                  value={customKey}
                  onChange={(event) => setCustomKey(event.target.value)}
                  aria-label="Name des Zusatzfelds"
                  placeholder="Feldname"
                  className="w-32 rounded border border-border bg-bg-base px-2 py-1 text-xs text-text-primary"
                />
                <input
                  type="text"
                  value={customValue}
                  onChange={(event) => setCustomValue(event.target.value)}
                  aria-label="Wert des Zusatzfelds"
                  placeholder="Wert"
                  className="flex-1 rounded border border-border bg-bg-base px-2 py-1 text-xs text-text-primary"
                />
                <button
                  type="button"
                  onClick={addCustom}
                  aria-label="Zusatzfeld übernehmen"
                  className="apx-btn-liquid rounded border border-border p-1 text-text-secondary"
                >
                  <Plus aria-hidden="true" className="size-3.5" />
                </button>
              </div>
            </section>
          </div>
        </div>

        <div className="flex items-center justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="apx-btn-liquid rounded border border-border px-3 py-1 text-xs text-text-secondary"
          >
            Schließen
          </button>
          <button
            type="button"
            onClick={() => void apply()}
            disabled={running || selectionSize === 0}
            data-testid="metadata-preset-apply"
            className="apx-btn-liquid flex items-center gap-1.5 rounded border border-accent/60 px-3 py-1 text-xs text-accent disabled:opacity-40"
          >
            <Check aria-hidden="true" className="size-3.5" />
            {running ? "Wendet an…" : "Auf Auswahl anwenden"}
          </button>
        </div>
      </div>
    </Dialog>
  );
}
