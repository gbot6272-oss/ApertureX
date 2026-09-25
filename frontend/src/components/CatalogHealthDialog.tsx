import { AlertTriangle, ClipboardCheck, Loader2 } from "lucide-react";
import { useEffect, useState } from "react";

import {
  catalogHealth,
  listHealthPhotos,
  type CatalogHealthEntryDto,
  type CatalogHealthKind,
} from "../lib/tauri";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

/**
 * Katalog-Gesundheit (Phase 34 F5, siehe `DECISIONS.md` ADR-0070 und
 * `apx-catalog`s `repository::health`).
 *
 * **Wofür das da ist.** Ein über Jahre gewachsener Katalog hat blinde
 * Flecken, die niemand sucht, weil man sie nicht sieht: Fotos ohne
 * Bewertung, Schlagwortlücken, Dateien, die außerhalb der App
 * verschoben wurden. Jede Lücke ist einzeln über Filter auffindbar —
 * aber nur, wenn man auf die Idee kommt, danach zu suchen.
 *
 * **Jede Zeile führt zur Arbeit, nicht nur zur Zahl.** Ein Dashboard,
 * das „412 Fotos ohne Schlagworte" meldet und dabei stehenbleibt, hat
 * dem Nutzer nichts abgenommen. „Anzeigen" legt die Fotos in die
 * Mehrfachauswahl — dieselbe Auswahl, mit der Stapel-Bewertung,
 * Metadaten-Vorgaben und Export ohnehin arbeiten.
 *
 * **Die Reihenfolge kommt aus dem Katalog**, nicht von hier: fehlende
 * Dateien zuerst, weil sie echten Datenverlust bedeuten können, Ort und
 * Bewertung zuletzt, weil sie bloß unbequem sind. Das ist eine Aussage
 * über Dringlichkeit und gehört zur Logik, nicht zur Darstellung.
 */

interface CatalogHealthDialogProps {
  open: boolean;
  onClose: () => void;
}

/** Wie viele Fotos „Anzeigen" höchstens in die Auswahl legt. */
const SELECTION_LIMIT = 500;

const LABELS: Record<CatalogHealthKind, { title: string; hint: string }> = {
  missing: {
    title: "Datei nicht auffindbar",
    hint: "Außerhalb der App verschoben oder gelöscht — der Katalogeintrag zeigt ins Leere.",
  },
  with_open_notes: {
    title: "Offene Notizen",
    hint: "Eine Bildnotiz ist noch nicht abgehakt.",
  },
  without_capture_date: {
    title: "Ohne Aufnahmedatum",
    hint: "Fehlt im Kalender und in der Sortierung nach Aufnahmezeit.",
  },
  without_keywords: {
    title: "Ohne Schlagworte",
    hint: "Über die Suche nur noch am Dateinamen auffindbar.",
  },
  without_rating: {
    title: "Nie bewertet",
    hint: "Beim Aussortieren übersehen worden.",
  },
  without_position: {
    title: "Ohne Ort",
    hint: "Taucht auf der Karte nicht auf — ein GPX-Track kann das nachtragen.",
  },
};

export function CatalogHealthDialog({ open, onClose }: CatalogHealthDialogProps) {
  const setMultiSelection = useAppStore((s) => s.setMultiSelection);
  const selectPhoto = useAppStore((s) => s.selectPhoto);

  const [entries, setEntries] = useState<CatalogHealthEntryDto[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [picked, setPicked] = useState<{ kind: CatalogHealthKind; count: number } | null>(null);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setLoading(true);
    setError(null);
    setPicked(null);
    catalogHealth()
      .then((rows) => {
        if (!cancelled) setEntries(rows);
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(String(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open]);

  async function show(kind: CatalogHealthKind): Promise<void> {
    setError(null);
    try {
      const photos = await listHealthPhotos(kind, SELECTION_LIMIT);
      setMultiSelection(photos.map((photo) => photo.id));
      if (photos[0]) selectPhoto(photos[0].id);
      setPicked({ kind, count: photos.length });
    } catch (err) {
      setError(String(err));
    }
  }

  const clean = entries.length > 0 && entries.every((entry) => entry.count === 0);

  return (
    <Dialog
      open={open}
      onClose={onClose}
      label="Katalog-Gesundheit"
      className="flex max-h-[85vh] w-[40rem] max-w-[92vw] flex-col"
    >
      <div className="flex min-h-0 flex-col gap-3 p-4">
        <div>
          <h2 className="flex items-center gap-2 text-sm font-semibold text-text-primary">
            <ClipboardCheck aria-hidden="true" className="size-4" />
            Katalog-Gesundheit
          </h2>
          <p className="mt-0.5 text-xs text-text-muted" data-testid="catalog-health-summary">
            {loading
              ? "Wird geprüft…"
              : clean
                ? "Keine Lücken gefunden — im Katalog ist alles gepflegt."
                : "Was im Bestand noch Arbeit braucht. Über „Anzeigen“ landen die Fotos in der Auswahl."}
          </p>
        </div>

        {error && (
          <p className="flex items-start gap-2 rounded border border-danger/40 bg-danger/10 px-2 py-1.5 text-xs text-danger" role="alert">
            <AlertTriangle aria-hidden="true" className="mt-0.5 size-3.5 shrink-0" />
            {error}
          </p>
        )}

        {picked && (
          <p className="rounded border border-success/40 bg-success/10 px-2 py-1.5 text-xs text-success" role="status" data-testid="catalog-health-picked">
            {picked.count} {picked.count === 1 ? "Foto" : "Fotos"} ausgewählt — „{LABELS[picked.kind].title}“.
            {picked.count === SELECTION_LIMIT ? ` Mehr als ${SELECTION_LIMIT} Treffer; der Rest kommt beim nächsten Durchgang.` : ""}
          </p>
        )}

        <ul className="min-h-0 flex-1 overflow-y-auto" data-testid="catalog-health-list">
          {loading && <Loader2 aria-label="Wird geprüft" className="size-4 animate-spin text-text-muted" />}
          {entries.map((entry) => (
            <li
              key={entry.kind}
              className="flex items-center gap-3 border-b border-border py-2 last:border-b-0"
            >
              <div className="min-w-0 flex-1">
                <p className="text-xs font-medium text-text-primary">{LABELS[entry.kind].title}</p>
                <p className="text-[11px] text-text-muted">{LABELS[entry.kind].hint}</p>
              </div>
              <span
                className={`tabular-nums text-sm ${entry.count === 0 ? "text-text-muted" : "text-text-primary"}`}
                data-testid={`catalog-health-count-${entry.kind}`}
              >
                {entry.count}
              </span>
              <button
                type="button"
                data-testid={`catalog-health-show-${entry.kind}`}
                disabled={entry.count === 0}
                onClick={() => void show(entry.kind)}
                className="apx-btn-liquid rounded border border-border px-2 py-0.5 text-xs text-text-secondary transition-colors duration-[var(--duration-fast)] hover:border-accent hover:text-text-primary disabled:opacity-40"
              >
                Anzeigen
              </button>
            </li>
          ))}
        </ul>
      </div>
    </Dialog>
  );
}
