import { AlertTriangle, Gauge, Loader2 } from "lucide-react";
import { useState } from "react";

// `FRAME_BUDGET_MS` bewusst von dort importiert statt hier
// zweitgeschrieben: zwei Zahlen, die dasselbe bedeuten sollen, laufen
// irgendwann auseinander.
import { FRAME_BUDGET_MS, median, probeFrameTimes } from "../lib/effects";
import { runPerformanceBenchmark, type PerformanceReportDto } from "../lib/tauri";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

/**
 * **Leistungs-Diagnose** (siehe `DECISIONS.md` ADR-0073).
 *
 * **Wofür das da ist.** „Die App ist langsam" ist eine Beobachtung,
 * keine Diagnose — und sie führt verlässlich zu stundenlanger Suche an
 * der falschen Stelle. In dieser App kommen drei völlig verschiedene
 * Ursachen in Frage, und sie fühlen sich gleich an:
 *
 * 1. **Ein Debug-Build.** Wer mit `cargo tauri dev` startet, bekommt
 *    einen. Die Bildberechnung besteht aus Rechnerei über
 *    Megapixel-Arrays, und genau die verliert ohne Optimierung ein
 *    Vielfaches. Das ist der häufigste Fall und der, den man von außen
 *    am schlechtesten sieht.
 * 2. **Eine zähe Oberfläche.** Die Glasflächen tragen Weichzeichner und
 *    SVG-Verzerrung gleichzeitig; auf einer nicht
 *    hardwarebeschleunigten Webansicht ist das allein genug.
 * 3. **Ein langsames Gerät.** Dann hilft nur, weniger zu verlangen.
 *
 * Dieser Dialog misst alle drei, statt zu raten — und sagt zu jedem
 * Befund, was dagegen hilft. Ein Messwert ohne Handlungsanweisung wäre
 * nur eine andere Art, den Nutzer allein zu lassen.
 *
 * **Warum die Messung wehtun darf.** Beide Messungen kosten spürbar
 * Zeit: die Bildberechnung rechnet wirklich zwölf Megapixel durch, die
 * Bildzeit-Messung hängt sich wirklich eine bewegte Last hinter die
 * Glasflächen. Eine billigere Messung hätte gemessen, wie schnell das
 * Gerät im Leerlauf ist — und das ist nie die Frage.
 */

interface PerformanceDialogProps {
  open: boolean;
  onClose: () => void;
}

/** Eine Zahl mit einer Nachkommastelle, ohne wissenschaftliche Notation. */
function round1(value: number): string {
  return (Math.round(value * 10) / 10).toLocaleString("de-DE");
}

export function PerformanceDialog({ open, onClose }: PerformanceDialogProps) {
  const uiSettings = useAppStore((s) => s.uiSettings);
  const saveUiSettings = useAppStore((s) => s.saveUiSettings);

  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [report, setReport] = useState<PerformanceReportDto | null>(null);
  const [frameMs, setFrameMs] = useState<number | null>(null);

  async function measure(): Promise<void> {
    setRunning(true);
    setError(null);
    setReport(null);
    setFrameMs(null);
    try {
      // Erst die Oberfläche, dann die Bildberechnung: die
      // Bildzeit-Messung braucht eine ruhige Hauptschleife, und ein
      // parallel laufender Zwölf-Megapixel-Durchlauf wäre genau das
      // Gegenteil. Umgekehrt stört die Bildzeit-Messung die
      // Bildberechnung kaum.
      const times = await probeFrameTimes(40);
      setFrameMs(median(times));
      setReport(await runPerformanceBenchmark());
    } catch (err) {
      setError(String(err));
    } finally {
      setRunning(false);
    }
  }

  const uiSlow = frameMs !== null && frameMs > FRAME_BUDGET_MS;

  return (
    <Dialog
      open={open}
      onClose={onClose}
      label="Leistung prüfen"
      className="flex max-h-[85vh] w-[40rem] max-w-[92vw] flex-col"
    >
      <div className="flex min-h-0 flex-col gap-3 overflow-y-auto p-4">
        <div>
          <h2 className="flex items-center gap-2 text-sm font-semibold text-text-primary">
            <Gauge aria-hidden="true" className="size-4" />
            Leistung prüfen
          </h2>
          <p className="mt-0.5 text-xs text-text-muted">
            Misst auf diesem Gerät, wo die Zeit hingeht: die Bildzeit der Oberfläche und die Dauer einer
            vollständigen Entwicklung. Dauert einige Sekunden und rechnet dabei wirklich — eine billigere
            Messung würde nur den Leerlauf messen.
          </p>
        </div>

        <div>
          <button
            type="button"
            onClick={() => void measure()}
            disabled={running}
            data-testid="performance-run"
            className="flex items-center gap-2 rounded border border-accent bg-accent/10 px-3 py-1.5 text-sm text-accent disabled:cursor-not-allowed disabled:opacity-50"
          >
            {running && <Loader2 aria-hidden="true" className="size-3.5 animate-spin" />}
            {running ? "Wird gemessen…" : "Messung starten"}
          </button>
        </div>

        {error && (
          <p
            className="flex items-start gap-2 rounded border border-danger/40 bg-danger/10 px-2 py-1.5 text-xs text-danger"
            role="alert"
          >
            <AlertTriangle aria-hidden="true" className="mt-0.5 size-3.5 shrink-0" />
            {error}
          </p>
        )}

        {report && (
          <div className="flex flex-col gap-3" data-testid="performance-report">
            {/* Der wichtigste Einzelbefund steht zuerst und ist der
                einzige, der als Warnung gestaltet ist: ein Debug-Build
                erklärt für sich genommen einen Faktor, gegen den jede
                andere Maßnahme verpufft. */}
            {report.debug_build && (
              <div
                className="rounded border border-danger/40 bg-danger/10 px-3 py-2 text-xs text-danger"
                role="alert"
                data-testid="performance-debug-warning"
              >
                <strong className="block text-sm">Diese App läuft als Debug-Build.</strong>
                Das ist mit Abstand die wahrscheinlichste Ursache. Die Bildberechnung besteht aus Rechnerei
                über Millionen Bildpunkte und ist ohne Optimierung um ein Vielfaches langsamer. Abhilfe:
                mit <code className="font-mono">cargo tauri build</code> bauen und die gebaute Anwendung
                starten statt <code className="font-mono">cargo tauri dev</code>.
              </div>
            )}

            <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-xs">
              <dt className="text-text-muted">Build</dt>
              <dd className="text-text-primary" data-testid="performance-build">
                {report.debug_build ? "Debug (nicht optimiert)" : "Release (optimiert)"}
              </dd>

              <dt className="text-text-muted">Kerne</dt>
              <dd className="text-text-primary">{report.threads}</dd>

              <dt className="text-text-muted">Bildzeit der Oberfläche</dt>
              <dd className={uiSlow ? "text-danger" : "text-text-primary"} data-testid="performance-frame">
                {frameMs === null ? "—" : `${round1(frameMs)} ms (${round1(1000 / Math.max(frameMs, 0.1))} Bilder/s)`}
              </dd>

              <dt className="text-text-muted">Entwickeln, neutral</dt>
              <dd className="text-text-primary">
                {report.develop_neutral_ms} ms für {round1(report.bench_megapixels)} Megapixel
              </dd>

              <dt className="text-text-muted">Entwickeln, mit Reglern</dt>
              <dd className="text-text-primary">{report.develop_loaded_ms} ms</dd>

              <dt className="text-text-muted">Durchsatz</dt>
              <dd className="text-text-primary">{round1(report.megapixels_per_second)} Megapixel/s</dd>
            </dl>

            {/* Die Oberflächen-Diagnose mit dem Schalter direkt daneben:
                wer gerade erfahren hat, dass sein Fenster 15 Bilder je
                Sekunde schafft, soll nicht erst den
                Einstellungsdialog suchen müssen. */}
            {uiSlow && (
              <div
                className="rounded border border-accent/40 bg-accent/10 px-3 py-2 text-xs text-text-primary"
                role="status"
                data-testid="performance-ui-hint"
              >
                <strong className="block text-sm">Die Oberfläche hält das Bildbudget nicht.</strong>
                Die Glasflächen tragen Weichzeichner und Verzerrung gleichzeitig. Eine sparsamere
                Effektstufe macht hier den größten Unterschied, ohne an der Bildberechnung etwas zu ändern.
                <div className="mt-2 flex flex-wrap gap-2">
                  <button
                    type="button"
                    onClick={() => void saveUiSettings({ ...uiSettings!, effect_level: "reduced" })}
                    disabled={!uiSettings}
                    className="rounded border border-border px-2 py-1 hover:border-accent"
                  >
                    Auf „Reduziert“ stellen
                  </button>
                  <button
                    type="button"
                    onClick={() => void saveUiSettings({ ...uiSettings!, effect_level: "plain" })}
                    disabled={!uiSettings}
                    className="rounded border border-border px-2 py-1 hover:border-accent"
                  >
                    Auf „Schlicht“ stellen
                  </button>
                </div>
              </div>
            )}

            {!report.debug_build && !uiSlow && (
              <p className="rounded border border-success/40 bg-success/10 px-3 py-2 text-xs text-success" role="status">
                Kein Engpass gefunden: optimierter Build, und die Oberfläche hält ihr Bildbudget.
              </p>
            )}
          </div>
        )}
      </div>
    </Dialog>
  );
}
