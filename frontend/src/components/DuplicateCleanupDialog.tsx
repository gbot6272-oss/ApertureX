import { AlertTriangle, Copy, Loader2 } from "lucide-react";
import { useState } from "react";

import {
  applyDuplicateCleanup,
  planDuplicateCleanup,
  type DuplicateCleanupPlanDto,
  type DuplicateMode,
  type KeeperReason,
} from "../lib/tauri";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

/**
 * Duplikat-Assistent (Phase 34 F9, siehe `DECISIONS.md` ADR-0070 und
 * `apx-app`s `duplicate_keeper`).
 *
 * **Was bisher fehlte.** Duplikatgruppen findet die App seit Phase 9,
 * und im Organisieren-Dialog stand sogar ein Vorschlag — als Sternchen
 * hinter einem Dateinamen. Danach war man allein: jede Gruppe einzeln
 * öffnen, von Hand aussortieren. Bei dreißig Gruppen macht das niemand.
 *
 * **Der Vorschlag sagt jetzt, warum.** „Ist mit Pick markiert" ist eine
 * andere Aussage als „hat die größere Datei", und nur die erste
 * verdient blindes Vertrauen. Die Begründung steht deshalb an der
 * Gruppe, nicht im Handbuch.
 *
 * **Und er bleibt ein Vorschlag.** Jede Gruppe lässt sich umstellen
 * (anderes Foto behalten) oder ganz überspringen. Was am Ende weggeht,
 * geht in den Papierkorb (Phase 33 F1), nicht ins Nirwana — mit dem
 * Grund „Duplikat", damit später nachvollziehbar ist, warum.
 */

interface DuplicateCleanupDialogProps {
  open: boolean;
  onClose: () => void;
}

const REASON_LABEL: Record<KeeperReason, string> = {
  picked: "ist mit Pick markiert",
  higher_rating: "hat die höhere Bewertung",
  has_edits: "ist bereits bearbeitet",
  raw: "ist das RAW, nicht der Ableger",
  higher_resolution: "hat die höhere Auflösung",
  larger_file: "hat die größere Datei",
  indistinguishable: "keine Unterschiede — die erste Version bleibt",
};

export function DuplicateCleanupDialog({ open, onClose }: DuplicateCleanupDialogProps) {
  const refreshFolders = useAppStore((s) => s.refreshFolders);
  const selectedFolderId = useAppStore((s) => s.selectedFolderId);
  const loadPhotosForFolder = useAppStore((s) => s.loadPhotosForFolder);

  const [mode, setMode] = useState<DuplicateMode>("exact");
  const [maxDistance, setMaxDistance] = useState(6);
  const [plan, setPlan] = useState<DuplicateCleanupPlanDto | null>(null);
  /** Je Gruppe: welches Foto bleibt. Leer = der Vorschlag gilt. */
  const [keeperOverride, setKeeperOverride] = useState<Record<number, string>>({});
  /** Gruppen, die übersprungen werden — aus ihnen geht nichts weg. */
  const [skipped, setSkipped] = useState<Record<number, boolean>>({});
  const [loading, setLoading] = useState(false);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [trashed, setTrashed] = useState<number | null>(null);

  async function runPlan() {
    setLoading(true);
    setError(null);
    setTrashed(null);
    setKeeperOverride({});
    setSkipped({});
    try {
      setPlan(await planDuplicateCleanup(mode, maxDistance));
    } catch (err) {
      setPlan(null);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  function keeperOf(index: number): string {
    return keeperOverride[index] ?? plan?.groups[index]?.keeper_id ?? "";
  }

  function discardIds(): string[] {
    if (!plan) return [];
    return plan.groups.flatMap((group, index) => {
      if (skipped[index]) return [];
      const keeper = keeperOf(index);
      return group.photos.map((candidate) => candidate.photo.id).filter((id) => id !== keeper);
    });
  }

  const toDiscard = discardIds();

  async function runApply() {
    setRunning(true);
    setError(null);
    try {
      const count = await applyDuplicateCleanup(mode, maxDistance, toDiscard);
      setTrashed(count);
      // Die weggeworfenen Fotos verschwinden aus jeder Liste — Ordner
      // und Raster müssen das erfahren, sonst zeigen sie Karteileichen.
      await refreshFolders();
      if (selectedFolderId) await loadPhotosForFolder(selectedFolderId);
      setPlan(await planDuplicateCleanup(mode, maxDistance));
      setKeeperOverride({});
      setSkipped({});
    } catch (err) {
      setError(String(err));
    } finally {
      setRunning(false);
    }
  }

  return (
    <Dialog
      open={open}
      onClose={onClose}
      label="Duplikate aufräumen"
      className="flex max-h-[85vh] w-[50rem] max-w-[92vw] flex-col"
    >
      <div className="flex min-h-0 flex-col gap-3 p-4">
        <div>
          <h2 className="flex items-center gap-1.5 text-sm font-semibold text-text-primary">
            <Copy aria-hidden="true" className="size-4" />
            Duplikate aufräumen
          </h2>
          <p className="mt-0.5 text-xs text-text-muted" data-testid="duplicate-cleanup-summary">
            {plan
              ? `${plan.groups.length} ${plan.groups.length === 1 ? "Gruppe" : "Gruppen"} · ${toDiscard.length} würden in den Papierkorb`
              : "Erst suchen — es wird nichts weggeworfen, was niemand gesehen hat."}
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <label className="flex items-center gap-1.5 text-xs text-text-secondary">
            Suche
            <select
              value={mode}
              onChange={(event) => setMode(event.target.value as DuplicateMode)}
              aria-label="Art der Duplikatsuche"
              className="rounded border border-border bg-bg-panel px-2 py-1 text-xs"
            >
              <option value="exact">byte-identisch</option>
              <option value="similar">ähnlich</option>
            </select>
          </label>
          {mode === "similar" && (
            <label className="flex items-center gap-1.5 text-xs text-text-secondary">
              Ähnlichkeit
              <input
                type="number"
                min={0}
                max={64}
                value={maxDistance}
                onChange={(event) => setMaxDistance(Number(event.target.value))}
                aria-label="Ähnlichkeitsschwelle"
                className="w-16 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
              />
            </label>
          )}
          <button
            type="button"
            onClick={() => void runPlan()}
            disabled={loading}
            data-testid="duplicate-cleanup-scan"
            className="apx-btn-liquid rounded border border-border px-2 py-1 text-xs text-text-secondary disabled:opacity-40"
          >
            {loading ? "Sucht…" : "Duplikate suchen"}
          </button>
          <button
            type="button"
            onClick={() => void runApply()}
            disabled={running || toDiscard.length === 0}
            data-testid="duplicate-cleanup-apply"
            className="apx-btn-liquid ml-auto rounded border border-accent/60 px-3 py-1 text-xs text-accent disabled:opacity-40"
          >
            {running ? (
              <span className="flex items-center gap-1.5">
                <Loader2 aria-hidden="true" className="size-3.5 animate-spin" />
                Räumt auf…
              </span>
            ) : (
              `${toDiscard.length} in den Papierkorb`
            )}
          </button>
        </div>

        {error && (
          <p className="flex items-start gap-2 rounded border border-danger/40 bg-danger/10 px-2 py-1.5 text-xs text-danger" role="alert">
            <AlertTriangle aria-hidden="true" className="mt-0.5 size-3.5 shrink-0" />
            {error}
          </p>
        )}

        {trashed !== null && (
          <p
            className="rounded border border-success/40 bg-success/10 px-2 py-1.5 text-xs text-success"
            data-testid="duplicate-cleanup-result"
          >
            {trashed} {trashed === 1 ? "Foto" : "Fotos"} in den Papierkorb gelegt — im Papierkorb-Dialog jederzeit zurückholbar.
          </p>
        )}

        <div className="min-h-0 flex-1 overflow-y-auto" data-testid="duplicate-cleanup-list">
          {plan && plan.groups.length === 0 && !loading && (
            <p className="text-xs text-text-muted">Keine Duplikatgruppen gefunden.</p>
          )}
          {!plan && !loading && (
            <p className="text-xs text-text-muted">Noch nicht gesucht.</p>
          )}

          <ul className="flex flex-col gap-2">
            {(plan?.groups ?? []).map((group, index) => {
              const keeper = keeperOf(index);
              const isSkipped = Boolean(skipped[index]);
              return (
                <li
                  key={group.photos.map((candidate) => candidate.photo.id).join("|")}
                  className={`rounded border border-border p-2 ${isSkipped ? "opacity-50" : ""}`}
                >
                  <div className="mb-1 flex items-center justify-between gap-2 text-[11px] text-text-muted">
                    <span>
                      Gruppe {index + 1} · {group.photos.length} Versionen · behalten, weil es{" "}
                      <span className="text-text-secondary">{REASON_LABEL[group.reason]}</span>
                    </span>
                    <label className="flex items-center gap-1 whitespace-nowrap">
                      <input
                        type="checkbox"
                        checked={isSkipped}
                        onChange={(event) =>
                          setSkipped((current) => ({ ...current, [index]: event.target.checked }))
                        }
                        aria-label={`Gruppe ${index + 1} überspringen`}
                      />
                      überspringen
                    </label>
                  </div>
                  <ul className="flex flex-col gap-0.5">
                    {group.photos.map((candidate) => (
                      <li key={candidate.photo.id} className="flex items-center gap-2 text-xs">
                        <label className="flex flex-1 items-center gap-1.5 truncate">
                          <input
                            type="radio"
                            name={`keeper-${index}`}
                            checked={keeper === candidate.photo.id}
                            disabled={isSkipped}
                            onChange={() =>
                              setKeeperOverride((current) => ({ ...current, [index]: candidate.photo.id }))
                            }
                            aria-label={`${candidate.photo.filename} behalten`}
                          />
                          <span className="truncate font-mono text-[11px] text-text-primary">
                            {candidate.photo.filename}
                          </span>
                        </label>
                        <span className="whitespace-nowrap text-[10px] text-text-muted">
                          {candidate.photo.width ?? "?"}×{candidate.photo.height ?? "?"}
                          {candidate.photo.rating > 0 ? ` · ${candidate.photo.rating}★` : ""}
                          {candidate.has_edits ? " · bearbeitet" : ""}
                        </span>
                        <span
                          className={`w-24 shrink-0 text-right text-[10px] ${
                            isSkipped
                              ? "text-text-muted"
                              : keeper === candidate.photo.id
                                ? "text-success"
                                : "text-danger"
                          }`}
                        >
                          {isSkipped ? "bleibt" : keeper === candidate.photo.id ? "bleibt" : "Papierkorb"}
                        </span>
                      </li>
                    ))}
                  </ul>
                </li>
              );
            })}
          </ul>
        </div>

        <p className="text-[11px] text-text-muted">
          Aus keiner Gruppe kann jede Version verschwinden — das prüft das Backend noch einmal nach, bevor es etwas anfasst.
        </p>

        <div className="flex justify-end">
          <button
            type="button"
            onClick={onClose}
            className="apx-btn-liquid rounded border border-border px-3 py-1 text-xs text-text-secondary"
          >
            Schließen
          </button>
        </div>
      </div>
    </Dialog>
  );
}
