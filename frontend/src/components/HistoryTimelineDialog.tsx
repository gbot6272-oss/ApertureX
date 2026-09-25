import { useEffect, useState } from "react";

import { listDevelopHistory } from "../lib/tauri";
import type { EditHistoryEntryDto } from "../lib/tauri";
import { parseEdlEnvelopeJson } from "../lib/edl";
import { diffEdlPayloads } from "../lib/edlDiff";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

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
 * Der Verlaufs-Vergleich darunter zeigt jedes geänderte Feld: zwei
 * Verlaufsschritte wählen, `diffEdlPayloads` (Phase 34 F6) listet die
 * Unterschiede mit lesbarer Beschriftung.
 *
 * **Die frühere Einschränkung ist aufgehoben.** Bis Phase 34 verglich
 * diese Ansicht nur den Sektionsumfang des Presets-Systems
 * (`PRESET_SECTION_KEYS`) — ohne Reparatur, Masken, Behandlung,
 * SW-Mixer, die am Bild bedienten Werkzeuge, Rahmen, Himmelsaustausch
 * und Verflüssigen. Das war für Presets die richtige Auswahl (nicht
 * alles lässt sich sinnvoll auf ein anderes Foto übertragen), hier aber
 * die falsche: wer wissen will, was ein Schritt geändert hat, meint
 * alles, was er geändert haben könnte. Ein Schritt, der nur eine Maske
 * verschoben hat, wurde vorher als „keine Unterschiede" gemeldet.
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
  // Phase 34 F6: verglichen wird jetzt das GANZE EDL, nicht mehr nur
  // die preset-faehigen Abschnitte (`PRESET_SECTION_KEYS`). Die alte
  // Auswahl war die der Presets und passte hier nie richtig: Masken,
  // Reparaturstriche, die am Bild bedienten Werkzeuge, Rahmen,
  // Himmelsaustausch und Verfluessigen sind bewusst NICHT
  // preset-faehig — und waren damit im Verlaufsvergleich unsichtbar.
  // Wer wissen will, was ein Schritt geaendert hat, meint aber alles,
  // was er geaendert haben koennte.
  const diff = edlA && edlB ? diffEdlPayloads(edlA, edlB) : [];

  return (
    <Dialog open={open} onClose={toggleHistoryDialog} label="Verlauf" className="max-w-2xl p-4">
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
                      <td className="p-1.5 text-text-primary">
                        {entry.label}
                        {entry.label !== entry.path && (
                          // Der Pfad bleibt daneben stehen: die
                          // Beschriftung sagt WAS, der Pfad sagt WO —
                          // bei gleichnamigen Reglern in mehreren
                          // Gruppen (etwa "Staerke") ist das der
                          // Unterschied zwischen Hinweis und Ratespiel.
                          <span className="ml-2 font-mono text-[10px] text-text-muted">{entry.path}</span>
                        )}
                      </td>
                      <td className="p-1.5 text-text-secondary">{entry.before}</td>
                      <td className="p-1.5 text-text-secondary">{entry.after}</td>
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
    </Dialog>
  );
}
