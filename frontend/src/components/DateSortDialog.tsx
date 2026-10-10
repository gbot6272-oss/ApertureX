import { AlertTriangle, CalendarClock, FolderTree, Loader2 } from "lucide-react";
import { useEffect, useState } from "react";

import {
  applyDateSort,
  previewDateSort,
  type DateSortOutcome,
  type DateSortPlanDto,
  type DateSortResultDto,
} from "../lib/tauri";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

/**
 * Nach Aufnahmedatum einsortieren (Phase 34 F8, siehe `DECISIONS.md`
 * ADR-0070 und `apx-app`s `date_sort`).
 *
 * **Warum das nicht der Import macht.** Der Import legt Fotos dort ab,
 * wo sie herkommen — und das war auf der Speicherkarte ein einziges
 * Verzeichnis. Wer Jahre so importiert hat, hat einen Ordner mit ein
 * paar tausend Dateien. Hier wird daraus ein Datumsbaum, nachträglich.
 *
 * **Vorschau zuerst, Anwenden als zweiter Schritt** — wie beim
 * Ordner-Abgleich (Phase 33 F2). Das ist hier noch wichtiger, denn
 * dieser Schritt verschiebt echte Dateien auf der Platte. Die Vorschau
 * ist deshalb nicht überspringbar: „Einsortieren" ist erst anklickbar,
 * wenn ein Plan da ist.
 *
 * **Vier Ausgänge, nicht zwei.** Neben „wird verschoben" und „liegt
 * schon richtig" gibt es die beiden, die Aufmerksamkeit brauchen:
 * Fotos ohne Aufnahmedatum (die bleiben liegen, solange man den
 * Rückfall auf die Dateizeit nicht ausdrücklich einschaltet) und
 * Namenskonflikte — zwei Fotos vom selben Tag mit demselben
 * Dateinamen. Die werden gemeldet und nicht angefasst; ohne diese
 * Prüfung überschriebe das zweite das erste.
 */

interface DateSortDialogProps {
  open: boolean;
  onClose: () => void;
}

const OUTCOME_LABEL: Record<DateSortOutcome, string> = {
  move: "Wird verschoben",
  already: "Liegt schon richtig",
  no_date: "Kein Aufnahmedatum",
  collision: "Namenskonflikt",
};

const OUTCOME_HINT: Record<DateSortOutcome, string> = {
  move: "Datei und Katalogeintrag wandern in den Datumsordner.",
  already: "Bleibt, wo sie ist.",
  no_date:
    "Weder EXIF-Aufnahmedatum noch Rückfall aktiv. Bleibt liegen — „Aufnahmedaten nachlesen“ in der Kalender-Ansicht holt fehlende Daten oft noch aus der Datei.",
  collision:
    "Im selben Zielordner landete schon ein Foto dieses Namens. Bleibt liegen — erst umbenennen (Stapel-Umbenennung), dann erneut einsortieren.",
};

/** Reihenfolge nach Dringlichkeit: erst was passiert, dann was hakt. */
const OUTCOME_ORDER: DateSortOutcome[] = ["move", "collision", "no_date", "already"];

/** Wie viele Zeilen je Abschnitt gezeigt werden. Bei ein paar tausend
 * Fotos hilft die vollständige Liste niemandem — die Zahl schon. */
const ROWS_PER_SECTION = 40;

export function DateSortDialog({ open, onClose }: DateSortDialogProps) {
  const selectedFolderId = useAppStore((s) => s.selectedFolderId);
  const folders = useAppStore((s) => s.folders);

  const [root, setRoot] = useState("");
  const [pattern, setPattern] = useState("{year}/{year}-{month}-{day}");
  const [useMtimeFallback, setUseMtimeFallback] = useState(false);
  const [wholeCatalog, setWholeCatalog] = useState(false);
  const [plan, setPlan] = useState<DateSortPlanDto | null>(null);
  const [result, setResult] = useState<DateSortResultDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [running, setRunning] = useState(false);

  const selectedFolder = folders.find((folder) => folder.id === selectedFolderId) ?? null;

  useEffect(() => {
    if (!open) return;
    setPlan(null);
    setResult(null);
    setError(null);
    // Der gewählte Ordner ist die Vorgabe fürs Ziel: der übliche Fall
    // ist, einen vollen Ordner in sich selbst aufzuräumen.
    setRoot(selectedFolder?.path ?? "");
  }, [open, selectedFolder?.path]);

  function options() {
    return {
      folderId: wholeCatalog ? null : selectedFolderId,
      root: root.trim() === "" ? null : root.trim(),
      pattern: pattern.trim() === "" ? null : pattern.trim(),
      useMtimeFallback,
    };
  }

  async function runPreview() {
    setLoading(true);
    setError(null);
    setResult(null);
    try {
      setPlan(await previewDateSort(options()));
    } catch (err) {
      setPlan(null);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  async function runApply() {
    setRunning(true);
    setError(null);
    try {
      const applied = await applyDateSort(options());
      setResult(applied);
      // Nach dem Verschieben ist der alte Plan Geschichte — neu rechnen,
      // damit die Liste nicht behauptet, es sei noch etwas zu tun.
      setPlan(await previewDateSort(options()));
    } catch (err) {
      setError(String(err));
    } finally {
      setRunning(false);
    }
  }

  const counts: Record<DateSortOutcome, number> = {
    move: plan?.move_count ?? 0,
    already: plan?.already_count ?? 0,
    no_date: plan?.no_date_count ?? 0,
    collision: plan?.collision_count ?? 0,
  };

  return (
    <Dialog
      open={open}
      onClose={onClose}
      label="Nach Aufnahmedatum einsortieren"
      className="flex max-h-[85vh] w-[48rem] max-w-[92vw] flex-col"
    >
      <div className="flex min-h-0 flex-col gap-3 p-4">
        <div>
          <h2 className="flex items-center gap-1.5 text-sm font-semibold text-text-primary">
            <CalendarClock aria-hidden="true" className="size-4" />
            Nach Aufnahmedatum einsortieren
          </h2>
          <p className="mt-0.5 text-xs text-text-muted" data-testid="date-sort-summary">
            {plan
              ? `${counts.move} zu verschieben · ${counts.already} schon richtig · ${counts.no_date} ohne Datum · ${counts.collision} Namenskonflikte`
              : "Erst ansehen, was passieren würde — dann anwenden."}
          </p>
        </div>

        <div className="flex flex-col gap-2 rounded border border-border bg-bg-raised p-2">
          <label className="flex items-center gap-1.5 text-xs text-text-secondary">
            <input
              type="checkbox"
              checked={wholeCatalog}
              onChange={(event) => setWholeCatalog(event.target.checked)}
              aria-label="Den ganzen Katalog einsortieren"
            />
            Ganzen Katalog statt nur {selectedFolder ? `„${selectedFolder.path}“` : "des gewählten Ordners"}
          </label>

          <label className="flex flex-col gap-1 text-xs text-text-secondary">
            Zielordner
            <input
              type="text"
              value={root}
              onChange={(event) => setRoot(event.target.value)}
              placeholder="/Fotos"
              aria-label="Zielordner"
              className="rounded border border-border bg-bg-panel px-2 py-1 font-mono text-[11px] text-text-primary"
            />
          </label>

          <label className="flex flex-col gap-1 text-xs text-text-secondary">
            Ordnermuster
            <input
              type="text"
              value={pattern}
              onChange={(event) => setPattern(event.target.value)}
              aria-label="Ordnermuster"
              className="rounded border border-border bg-bg-panel px-2 py-1 font-mono text-[11px] text-text-primary"
            />
            <span className="text-[11px] text-text-muted">
              <code>{"{year}"}</code>, <code>{"{month}"}</code>, <code>{"{day}"}</code>; <code>/</code> trennt die Ebenen.
            </span>
          </label>

          <label className="flex items-start gap-1.5 text-xs text-text-secondary">
            <input
              type="checkbox"
              checked={useMtimeFallback}
              onChange={(event) => setUseMtimeFallback(event.target.checked)}
              aria-label="Fotos ohne Aufnahmedatum nach der Dateizeit einsortieren"
              className="mt-0.5"
            />
            <span>
              Ohne Aufnahmedatum die Dateizeit nehmen
              <span className="block text-[11px] text-text-muted">
                Aus, weil die Dateizeit sagt, wann zuletzt geschrieben wurde — ein Kopiervorgang setzt sie auf heute.
              </span>
            </span>
          </label>
        </div>

        {error && (
          <p className="flex items-start gap-2 rounded border border-danger/40 bg-danger/10 px-2 py-1.5 text-xs text-danger" role="alert">
            <AlertTriangle aria-hidden="true" className="mt-0.5 size-3.5 shrink-0" />
            {error}
          </p>
        )}

        {result && (
          <div
            className="rounded border border-success/40 bg-success/10 px-2 py-1.5 text-xs text-success"
            data-testid="date-sort-result"
          >
            {result.moved} {result.moved === 1 ? "Foto" : "Fotos"} einsortiert · {result.folders_created}{" "}
            {result.folders_created === 1 ? "Ordner" : "Ordner"} angelegt
            {result.failures.length > 0 && (
              <ul className="mt-1 flex flex-col gap-0.5 text-danger">
                {result.failures.map((failure) => (
                  <li key={failure}>{failure}</li>
                ))}
              </ul>
            )}
          </div>
        )}

        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={() => void runPreview()}
            disabled={loading || (!wholeCatalog && !selectedFolderId)}
            data-testid="date-sort-preview"
            className="apx-btn-liquid rounded border border-border px-2 py-1 text-xs text-text-secondary disabled:opacity-40"
          >
            {loading ? "Rechnet…" : "Vorschau"}
          </button>
          <button
            type="button"
            onClick={() => void runApply()}
            disabled={running || counts.move === 0}
            data-testid="date-sort-apply"
            className="apx-btn-liquid ml-auto rounded border border-accent/60 px-3 py-1 text-xs text-accent disabled:opacity-40"
          >
            {running ? (
              <span className="flex items-center gap-1.5">
                <Loader2 aria-hidden="true" className="size-3.5 animate-spin" />
                Sortiert ein…
              </span>
            ) : (
              "Einsortieren"
            )}
          </button>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto" data-testid="date-sort-list">
          {!plan && !loading && (
            <p className="text-xs text-text-muted">
              Noch keine Vorschau. Es wird nichts verschoben, bevor sie da ist.
            </p>
          )}

          <div className="flex flex-col gap-3">
            {OUTCOME_ORDER.filter((outcome) => counts[outcome] > 0).map((outcome) => {
              const rows = (plan?.entries ?? []).filter((entry) => entry.outcome === outcome);
              return (
                <section key={outcome}>
                  <h3 className="mb-1 flex items-center gap-1.5 text-xs font-semibold text-text-secondary">
                    <FolderTree aria-hidden="true" className="size-3.5" />
                    {OUTCOME_LABEL[outcome]} <span className="font-normal text-text-muted">({counts[outcome]})</span>
                  </h3>
                  <p className="mb-1 text-[11px] text-text-muted">{OUTCOME_HINT[outcome]}</p>
                  <ul className="flex flex-col gap-0.5 rounded border border-border bg-bg-raised p-1.5">
                    {rows.slice(0, ROWS_PER_SECTION).map((entry) => (
                      <li key={entry.photo_id} className="truncate font-mono text-[11px] text-text-primary">
                        {entry.filename}
                        {entry.target_dir && (
                          <span className="ml-2 text-text-muted">→ {entry.target_dir}</span>
                        )}
                        {entry.used_mtime && (
                          <span className="ml-2 text-[10px] text-text-muted">(Dateizeit)</span>
                        )}
                      </li>
                    ))}
                    {rows.length > ROWS_PER_SECTION && (
                      <li className="text-[11px] text-text-muted">
                        … und {rows.length - ROWS_PER_SECTION} weitere
                      </li>
                    )}
                  </ul>
                </section>
              );
            })}
          </div>
        </div>

        <p className="text-[11px] text-text-muted">
          Verschoben werden echte Dateien. Der Katalog wird erst umgehängt, wenn die Datei am neuen Ort liegt — bricht es
          dazwischen ab, meldet der Ordner-Abgleich sie als fehlend, statt sie zu verlieren.
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
