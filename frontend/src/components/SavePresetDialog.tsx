import { useState } from "react";

import {
  PRESET_CONDITION_FIELD_OPTIONS,
  PRESET_CONDITION_OPERATOR_OPTIONS,
  PRESET_SECTION_KEYS,
  PRESET_SECTION_LABELS,
} from "../lib/presets";
import type { PresetLeafCondition, PresetRuleGroup, PresetRules, PresetSectionKey } from "../lib/presets";
import { conditionNode } from "../lib/ruleTree";
import { RuleTreeEditor } from "./RuleTreeEditor";
import { useAppStore } from "../store";

interface SavePresetDialogProps {
  open: boolean;
  onClose: () => void;
}

let nextRuleKey = 0;

/** Eine Regelgruppe im Editor-Zustand — `key` ist reines React-Listen-
 * Schlüsselhilfsmittel, kein Teil der gespeicherten [`PresetRuleGroup`]. */
interface DraftRule extends PresetRuleGroup {
  key: number;
}

function makeDefaultLeaf(): PresetLeafCondition {
  return { field: "iso", op: ">", value: "" };
}

function makeDraftRule(): DraftRule {
  return { key: nextRuleKey++, section: null, node: conditionNode(makeDefaultLeaf()) };
}

/**
 * Preset-Speichern-Dialog (Phase 5 Schritt 4, `SPEC.md` §3.5: „Beim
 * Speichern zeigt ein Dialog jede einzelne Einstellungsgruppe mit
 * Checkbox"). Reparatur ist bewusst keine wählbare Sektion (siehe
 * `lib/presets.ts`s `PresetSectionKey`-Moduldoku).
 *
 * Die Bedingungen (Phase 13 Schritt 7) sind eine Liste von Regelgruppen —
 * jede gattert entweder das ganze Preset (`section: null`) oder genau eine
 * Sektion, mehrere Gruppen bleiben untereinander UND-verknüpft. Innerhalb
 * einer Gruppe ist jetzt aber ein echter, beliebig verschachtelbarer
 * UND/ODER-Baum möglich ([`RuleTreeEditor`]) statt nur einer einzelnen
 * Bedingung — siehe `DECISIONS.md` ADR-0040-Nachtrag V.
 */
export function SavePresetDialog({ open, onClose }: SavePresetDialogProps) {
  const presetFolders = useAppStore((s) => s.presetFolders);
  const savePresetFromCurrentEdl = useAppStore((s) => s.savePresetFromCurrentEdl);

  const [name, setName] = useState("");
  const [folderId, setFolderId] = useState<string>("");
  const [tagsText, setTagsText] = useState("");
  const [selectedSections, setSelectedSections] = useState<Set<PresetSectionKey>>(new Set(PRESET_SECTION_KEYS));
  const [rules, setRules] = useState<DraftRule[]>([]);

  if (!open) return null;

  function toggleSection(key: PresetSectionKey) {
    setSelectedSections((previous) => {
      const next = new Set(previous);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  function addRule() {
    setRules((previous) => [...previous, makeDraftRule()]);
  }

  function updateRule(key: number, patch: Partial<Pick<DraftRule, "section" | "node">>) {
    setRules((previous) => previous.map((rule) => (rule.key === key ? { ...rule, ...patch } : rule)));
  }

  function removeRule(key: number) {
    setRules((previous) => previous.filter((rule) => rule.key !== key));
  }

  function reset() {
    setName("");
    setFolderId("");
    setTagsText("");
    setSelectedSections(new Set(PRESET_SECTION_KEYS));
    setRules([]);
  }

  async function handleSave() {
    const tags = tagsText
      .split(",")
      .map((tag) => tag.trim())
      .filter((tag) => tag.length > 0);
    const savedRules: PresetRules = rules.map(({ section, node }) => ({ section, node }));
    await savePresetFromCurrentEdl(name, folderId || null, tags, [...selectedSections], savedRules);
    reset();
    onClose();
  }

  const canSave = name.trim().length > 0 && selectedSections.size > 0;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-24" onClick={onClose}>
      <div
        role="dialog"
        aria-label="Preset speichern"
        className="max-h-[85vh] w-full max-w-sm overflow-y-auto rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
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

        <fieldset className="mb-3 flex flex-col gap-2">
          <legend className="mb-1 text-xs font-medium text-text-secondary">
            Bedingungen (optional — mehrere Regelgruppen müssen alle zutreffen, innerhalb einer Gruppe frei UND/ODER-verschachtelbar)
          </legend>
          {rules.map((rule) => (
            <div key={rule.key} className="flex flex-col gap-1 rounded border border-border p-2">
              <div className="flex items-center justify-between gap-1">
                <select
                  aria-label="Betroffene Sektion"
                  value={rule.section ?? ""}
                  onChange={(event) => updateRule(rule.key, { section: (event.target.value || null) as PresetSectionKey | null })}
                  className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-1 py-0.5 text-xs"
                >
                  <option value="">Ganzes Preset</option>
                  {PRESET_SECTION_KEYS.map((key) => (
                    <option key={key} value={key}>
                      Nur {PRESET_SECTION_LABELS[key]}
                    </option>
                  ))}
                </select>
                <button
                  type="button"
                  onClick={() => removeRule(rule.key)}
                  aria-label="Regelgruppe entfernen"
                  className="shrink-0 text-xs text-danger"
                >
                  ×
                </button>
              </div>
              <RuleTreeEditor
                node={rule.node}
                onChange={(next) => updateRule(rule.key, { node: next })}
                makeDefaultLeaf={makeDefaultLeaf}
                renderLeaf={(leaf, onLeafChange) => (
                  <>
                    <select
                      aria-label="Bedingungsfeld"
                      value={leaf.field}
                      onChange={(event) => onLeafChange({ ...leaf, field: event.target.value as PresetLeafCondition["field"] })}
                      className="min-w-0 rounded border border-border bg-bg-panel px-1 py-0.5"
                    >
                      {PRESET_CONDITION_FIELD_OPTIONS.map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label}
                        </option>
                      ))}
                    </select>
                    <select
                      aria-label="Bedingungsoperator"
                      value={leaf.op}
                      onChange={(event) => onLeafChange({ ...leaf, op: event.target.value as PresetLeafCondition["op"] })}
                      className="min-w-0 rounded border border-border bg-bg-panel px-1 py-0.5"
                    >
                      {PRESET_CONDITION_OPERATOR_OPTIONS.map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label}
                        </option>
                      ))}
                    </select>
                    <input
                      type="text"
                      aria-label="Bedingungswert"
                      value={leaf.value}
                      onChange={(event) => onLeafChange({ ...leaf, value: event.target.value })}
                      className="w-16 min-w-0 rounded border border-border bg-bg-panel px-1 py-0.5"
                    />
                  </>
                )}
              />
            </div>
          ))}
          <button
            type="button"
            onClick={addRule}
            className="self-start rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel"
          >
            + Regelgruppe
          </button>
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
