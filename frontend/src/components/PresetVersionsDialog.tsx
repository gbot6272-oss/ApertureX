import { useEffect, useState } from "react";

import { buildPresetEdlSubset, diffEdlSubsets, parseEdlSubset, serializeEdlSubset } from "../lib/presets";
import type { PresetSectionKey } from "../lib/presets";
import { addPresetVersion, listPresetVersions } from "../lib/tauri";
import type { PresetVersionDto } from "../lib/tauri";
import { useAppStore } from "../store";

interface PresetVersionsDialogProps {
  presetId: string | null;
  presetName: string;
  onClose: () => void;
}

function formatValue(value: unknown): string {
  if (value === undefined) return "(nicht gesetzt)";
  return JSON.stringify(value);
}

/**
 * Versionierung + Diff-Ansicht (Phase 5 Schritt 8, `PLAN.md`: „Jede
 * erneute Speicherung über ein bestehendes Preset legt eine neue
 * `preset_versions`-Zeile an … kleine Diff-Ansicht: zwei Versionen wählen,
 * Liste der Felder mit unterschiedlichem Wert"). „Aktuellen Stand als neue
 * Version speichern" übernimmt bewusst dieselben Sektionen wie die
 * bisher aktuellste Version (nicht die aktuelle `SavePresetDialog`-Auswahl
 * — die gibt es hier gar nicht) — eine erneute Speicherung eines
 * bestehenden Presets aktualisiert, was es bereits abdeckte, statt neue
 * Sektionen implizit hinzuzufügen.
 */
export function PresetVersionsDialog({ presetId, presetName, onClose }: PresetVersionsDialogProps) {
  const developEdl = useAppStore((s) => s.developEdl);
  const [versions, setVersions] = useState<PresetVersionDto[]>([]);
  const [versionAId, setVersionAId] = useState<string>("");
  const [versionBId, setVersionBId] = useState<string>("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!presetId) return;
    void refresh(presetId);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- lädt bewusst nur beim Öffnen (presetId-Wechsel) neu.
  }, [presetId]);

  async function refresh(id: string) {
    const list = await listPresetVersions(id);
    setVersions(list);
    if (list.length > 0) {
      const first = list[0];
      const last = list[list.length - 1];
      if (first) setVersionAId((current) => (list.some((v) => v.id === current) ? current : first.id));
      if (last) setVersionBId((current) => (list.some((v) => v.id === current) ? current : last.id));
    }
  }

  async function handleSaveNewVersion() {
    if (!presetId) return;
    setSaving(true);
    try {
      const latest = versions[versions.length - 1];
      const sections = latest ? (Object.keys(parseEdlSubset(latest.edl_subset_json)) as PresetSectionKey[]) : [];
      const subset = buildPresetEdlSubset(developEdl, sections);
      await addPresetVersion(presetId, serializeEdlSubset(subset));
      await refresh(presetId);
    } finally {
      setSaving(false);
    }
  }

  if (!presetId) return null;

  const versionA = versions.find((v) => v.id === versionAId);
  const versionB = versions.find((v) => v.id === versionBId);
  const diff = versionA && versionB ? diffEdlSubsets(parseEdlSubset(versionA.edl_subset_json), parseEdlSubset(versionB.edl_subset_json)) : [];

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-24" onClick={onClose}>
      <div
        role="dialog"
        aria-label={`Versionen: ${presetName}`}
        className="w-full max-w-lg rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 className="mb-3 text-sm font-semibold text-text-primary">Versionen: {presetName}</h2>

        <button
          type="button"
          onClick={() => void handleSaveNewVersion()}
          disabled={saving}
          className="mb-3 rounded bg-accent px-3 py-1 text-xs text-white disabled:cursor-not-allowed disabled:opacity-40"
        >
          Aktuellen Stand als neue Version speichern
        </button>

        {versions.length === 0 ? (
          <p className="text-xs text-text-muted">Noch keine Versionen geladen.</p>
        ) : (
          <>
            <div className="mb-3 flex gap-2 text-xs">
              <label className="flex flex-1 flex-col gap-1 text-text-secondary">
                Version A
                <select
                  aria-label="Version A"
                  value={versionAId}
                  onChange={(event) => setVersionAId(event.target.value)}
                  className="rounded border border-border bg-bg-panel px-2 py-1"
                >
                  {versions.map((version) => (
                    <option key={version.id} value={version.id}>
                      #{version.sequence} — {new Date(version.created_at).toLocaleString()}
                    </option>
                  ))}
                </select>
              </label>
              <label className="flex flex-1 flex-col gap-1 text-text-secondary">
                Version B
                <select
                  aria-label="Version B"
                  value={versionBId}
                  onChange={(event) => setVersionBId(event.target.value)}
                  className="rounded border border-border bg-bg-panel px-2 py-1"
                >
                  {versions.map((version) => (
                    <option key={version.id} value={version.id}>
                      #{version.sequence} — {new Date(version.created_at).toLocaleString()}
                    </option>
                  ))}
                </select>
              </label>
            </div>

            <div className="max-h-64 overflow-y-auto rounded border border-border">
              {diff.length === 0 ? (
                <p className="p-2 text-xs text-text-muted">Keine Unterschiede zwischen den gewählten Versionen.</p>
              ) : (
                <table className="w-full text-left text-xs">
                  <thead>
                    <tr className="border-b border-border text-text-secondary">
                      <th className="p-1.5">Feld</th>
                      <th className="p-1.5">A</th>
                      <th className="p-1.5">B</th>
                    </tr>
                  </thead>
                  <tbody>
                    {diff.map((entry) => (
                      <tr key={entry.path} className="border-b border-border last:border-0">
                        <td className="p-1.5 font-mono text-text-primary">{entry.path}</td>
                        <td className="p-1.5 text-text-secondary">{formatValue(entry.a)}</td>
                        <td className="p-1.5 text-text-secondary">{formatValue(entry.b)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          </>
        )}

        <div className="mt-3 flex justify-end">
          <button type="button" onClick={onClose} className="rounded border border-border px-3 py-1 text-xs text-text-secondary hover:bg-bg-panel">
            Schließen
          </button>
        </div>
      </div>
    </div>
  );
}
