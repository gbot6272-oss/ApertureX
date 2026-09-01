import { useEffect, useState } from "react";

import { buildPresetEdlSubset, diffEdlSubsets, PRESET_SECTION_KEYS } from "../lib/presets";
import { listDevelopHistory } from "../lib/tauri";
import type { EditHistoryEntryDto } from "../lib/tauri";
import { parseEdlEnvelopeJson } from "../lib/edl";
import { useAppStore } from "../store";

function formatValue(value: unknown): string {
  if (value === undefined) return "(nicht gesetzt)";
  return JSON.stringify(value);
}

/**
 * Zeitleisten-Ansicht + Verlaufs-Vergleich (Phase 9 Schritt 7, siehe
 * `PLAN.md`/`DECISIONS.md` ADR-0035) — der vollständige, ungekürzte
 * `edit_history`-Verlauf eines Fotos (anders als der Undo/Redo-
 * Einzelschritt-Mechanismus, der nur den *aktuellen* Zeiger kennt),
 * zeitlich statt als reine Liste angeordnet: die Position jedes Punkts
 * auf der horizontalen Linie ist proportional zu seinem `created_at`
 * relativ zum ersten/letzten Eintrag — ein Klick springt direkt zu
 * diesem Stand (`gotoDevelopHistory`, `apx_catalog::repository::edits::
 * goto`), ohne über Einzelschritte zu gehen.
 *
 * Der Verlaufs-Vergleich darunter ist dasselbe Diff-Muster wie
 * `PresetVersionsDialog.tsx` (Phase 5 Schritt 8): zwei Verlaufsschritte
 * wählen, `diffEdlSubsets` zeigt jedes geänderte Feld. **Bewusste
 * Vereinfachung**: derselbe Sektionsumfang wie das Presets-System
 * (`PRESET_SECTION_KEYS` — ohne Reparatur/Masken/Behandlung/SW-Mixer/
 * Node-Editor-Stufen), nicht das komplette EDL.
 */
export function HistoryTimelineDialog() {
  const open = useAppStore((s) => s.historyDialogOpen);
  const toggleHistoryDialog = useAppStore((s) => s.toggleHistoryDialog);
  const developPhotoId = useAppStore((s) => s.developPhotoId);
  const gotoDevelopHistory = useAppStore((s) => s.gotoDevelopHistory);

  const [entries, setEntries] = useState<EditHistoryEntryDto[]>([]);
  const [sequenceA, setSequenceA] = useState<number | null>(null);
  const [sequenceB, setSequenceB] = useState<number | null>(null);

  useEffect(() => {
    if (!open || !developPhotoId) return;
    void refresh(developPhotoId);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- lädt bewusst nur beim Öffnen/Fotowechsel neu.
  }, [open, developPhotoId]);

  async function refresh(photoId: string) {
    const list = await listDevelopHistory(photoId);
    setEntries(list);
    if (list.length > 0) {
      const first = list[0];
      const last = list[list.length - 1];
      if (first) setSequenceA((current) => (list.some((e) => e.sequence === current) ? current : first.sequence));
      if (last) setSequenceB((current) => (list.some((e) => e.sequence === current) ? current : last.sequence));
    }
  }

  if (!open) return null;

  const times = entries.map((entry) => new Date(entry.created_at).getTime()).filter((ms) => !Number.isNaN(ms));
  const minTime = times.length > 0 ? Math.min(...times) : 0;
  const maxTime = times.length > 0 ? Math.max(...times) : 0;
  const span = maxTime - minTime;

  function positionPercent(entry: EditHistoryEntryDto): number {
    const ms = new Date(entry.created_at).getTime();
    if (Number.isNaN(ms) || span <= 0) return 0;
    return ((ms - minTime) / span) * 100;
  }

  const entryA = entries.find((e) => e.sequence === sequenceA);
  const entryB = entries.find((e) => e.sequence === sequenceB);
  const edlA = entryA ? parseEdlEnvelopeJson(entryA.edl_json) : null;
  const edlB = entryB ? parseEdlEnvelopeJson(entryB.edl_json) : null;
  const diff = edlA && edlB ? diffEdlSubsets(buildPresetEdlSubset(edlA, PRESET_SECTION_KEYS), buildPresetEdlSubset(edlB, PRESET_SECTION_KEYS)) : [];

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-24" onClick={toggleHistoryDialog}>
      <div
        role="dialog"
        aria-label="Verlauf"
        className="w-full max-w-2xl rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 className="mb-3 text-sm font-semibold text-text-primary">Verlauf — Zeitleiste &amp; Vergleich</h2>

        {entries.length === 0 ? (
          <p className="text-xs text-text-muted">Noch keine Bearbeitungsschritte für dieses Foto.</p>
        ) : (
          <>
            <div className="mb-4" aria-label="Zeitleiste" role="group">
              <div className="relative h-8 rounded border border-border bg-bg-panel">
                {entries.map((entry) => (
                  <button
                    key={entry.sequence}
                    type="button"
                    onClick={() => void gotoDevelopHistory(entry.sequence)}
                    title={`#${entry.sequence}${entry.label ? ` — ${entry.label}` : ""} — ${new Date(entry.created_at).toLocaleString()}`}
                    aria-label={`Zu Verlaufsschritt #${entry.sequence} springen`}
                    className="absolute top-1/2 h-3 w-3 -translate-x-1/2 -translate-y-1/2 rounded-full border border-accent bg-accent/70 hover:bg-accent"
                    style={{ left: `${positionPercent(entry)}%` }}
                  />
                ))}
              </div>
              <div className="mt-1 flex justify-between text-[11px] text-text-muted">
                <span>{entries[0] ? new Date(entries[0].created_at).toLocaleString() : ""}</span>
                <span>{entries[entries.length - 1] ? new Date(entries[entries.length - 1]!.created_at).toLocaleString() : ""}</span>
              </div>
            </div>

            <div className="mb-3 flex gap-2 text-xs">
              <label className="flex flex-1 flex-col gap-1 text-text-secondary">
                Schritt A
                <select
                  aria-label="Verlaufsschritt A"
                  value={sequenceA ?? ""}
                  onChange={(event) => setSequenceA(Number(event.target.value))}
                  className="rounded border border-border bg-bg-panel px-2 py-1"
                >
                  {entries.map((entry) => (
                    <option key={entry.sequence} value={entry.sequence}>
                      #{entry.sequence}{entry.label ? ` — ${entry.label}` : ""}
                    </option>
                  ))}
                </select>
              </label>
              <label className="flex flex-1 flex-col gap-1 text-text-secondary">
                Schritt B
                <select
                  aria-label="Verlaufsschritt B"
                  value={sequenceB ?? ""}
                  onChange={(event) => setSequenceB(Number(event.target.value))}
                  className="rounded border border-border bg-bg-panel px-2 py-1"
                >
                  {entries.map((entry) => (
                    <option key={entry.sequence} value={entry.sequence}>
                      #{entry.sequence}{entry.label ? ` — ${entry.label}` : ""}
                    </option>
                  ))}
                </select>
              </label>
            </div>

            <div className="max-h-64 overflow-y-auto rounded border border-border">
              {diff.length === 0 ? (
                <p className="p-2 text-xs text-text-muted">Keine Unterschiede zwischen den gewählten Schritten.</p>
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
          <button type="button" onClick={toggleHistoryDialog} className="rounded border border-border px-3 py-1 text-xs text-text-secondary hover:bg-bg-panel">
            Schließen
          </button>
        </div>
      </div>
    </div>
  );
}
