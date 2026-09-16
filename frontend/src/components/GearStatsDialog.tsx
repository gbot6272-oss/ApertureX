import { Aperture, Camera, Gauge, RefreshCw, Ruler, Timer } from "lucide-react";
import { useEffect, useState } from "react";

import type { DistributionBucketDto } from "../lib/tauri";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

/**
 * Ausrüstungs- und Belichtungs-Statistik (Phase 32 F5).
 *
 * **Warum nicht „Aufnahme-Statistik".** Im selben Überlauf-Menü steht
 * bereits „Statistik…" (Katalogumfang, Phase 9 Schritt 3). Zwei
 * Einträge, die beide auf „Statistik" enden, sind für Auge und
 * Screenreader kaum zu unterscheiden — der Name sagt jetzt, worum es
 * geht: Ausrüstung und Belichtung.
 *
 * Der bestehende Statistik-Dialog (`StatsCacheDialog.tsx`, Phase 9
 * Schritt 3) beantwortet „wie groß ist der Katalog": Anzahl, Größe,
 * Zeitraum, Bewertungsverteilung — und nebenbei die acht häufigsten
 * Kameras als Textliste. Dieser hier beantwortet eine andere Frage:
 * **womit fotografiere ich eigentlich?** Vollständige Kamera- und
 * Objektivlisten, dazu die Verteilung über Brennweite, Blende, ISO und
 * Belichtungszeit.
 *
 * **Warum das nützlich ist und nicht nur hübsch:** wer sieht, dass 80 %
 * seiner Bilder zwischen 24 und 35 mm entstehen, weiß, welches Objektiv
 * er wirklich braucht — und welches seit zwei Jahren nur mitgetragen
 * wird.
 *
 * **Klick auf eine Kamera filtert den Katalog.** `camera_model` ist eines
 * der vier Felder, die `FilterCriteriaDto` kennt — der Sprung von der
 * Statistik zu den Fotos ist damit echt und nicht nur angedeutet.
 * Objektive sind bewusst **nicht** klickbar: ein Objektivfilter existiert
 * im Backend nicht, und ein Knopf, der nichts tut, wäre schlimmer als
 * gar keiner.
 */

interface GearStatsDialogProps {
  open: boolean;
  onClose: () => void;
}

type Unit = "count" | "share";

export function GearStatsDialog({ open, onClose }: GearStatsDialogProps) {
  const stats = useAppStore((s) => s.gearStatistics);
  const loading = useAppStore((s) => s.gearStatisticsLoading);
  const refresh = useAppStore((s) => s.refreshGearStatistics);
  const setLibraryFilterChip = useAppStore((s) => s.setLibraryFilterChip);
  const setCenterView = useAppStore((s) => s.setCenterView);
  const libraryFilter = useAppStore((s) => s.libraryFilter);

  const [unit, setUnit] = useState<Unit>("count");
  const [showAllCameras, setShowAllCameras] = useState(false);
  const [showAllLenses, setShowAllLenses] = useState(false);

  useEffect(() => {
    if (open) void refresh();
  }, [open, refresh]);

  function filterByCamera(model: string) {
    void setLibraryFilterChip({ camera_model: model });
    setCenterView("grid");
    onClose();
  }

  const cameras = stats?.cameras ?? [];
  const lenses = stats?.lenses ?? [];
  const visibleCameras = showAllCameras ? cameras : cameras.slice(0, 6);
  const visibleLenses = showAllLenses ? lenses : lenses.slice(0, 6);

  return (
    <Dialog open={open} onClose={onClose} label="Ausrüstung und Belichtung" className="flex max-h-[85vh] w-[52rem] max-w-[94vw] flex-col">
      <div className="flex min-h-0 flex-col gap-3 overflow-y-auto p-4">
        <div className="flex flex-wrap items-center gap-2">
          <h2 className="text-sm font-semibold text-text-primary">Ausrüstung und Belichtung</h2>
          <span className="text-xs text-text-muted" data-testid="gear-total">
            {stats ? `${stats.total} Aufnahmen ausgewertet` : "—"}
          </span>
          <span className="flex-1" />
          <div role="group" aria-label="Einheit" className="flex rounded border border-border p-0.5">
            <button
              type="button"
              onClick={() => setUnit("count")}
              aria-pressed={unit === "count"}
              className={`rounded px-2 py-0.5 text-xs transition-colors duration-[var(--duration-fast)] ${
                unit === "count" ? "bg-accent/10 text-accent" : "text-text-secondary"
              }`}
            >
              Anzahl
            </button>
            <button
              type="button"
              onClick={() => setUnit("share")}
              aria-pressed={unit === "share"}
              className={`rounded px-2 py-0.5 text-xs transition-colors duration-[var(--duration-fast)] ${
                unit === "share" ? "bg-accent/10 text-accent" : "text-text-secondary"
              }`}
            >
              Anteil
            </button>
          </div>
          <button
            type="button"
            onClick={() => void refresh()}
            disabled={loading}
            aria-label="Statistik neu berechnen"
            className="apx-btn-liquid rounded border border-border p-1.5 text-text-secondary disabled:opacity-40"
          >
            <RefreshCw aria-hidden="true" className={`size-3.5 ${loading ? "animate-spin" : ""}`} />
          </button>
        </div>

        {!stats && <p className="text-xs text-text-muted">{loading ? "Wird berechnet…" : "Noch keine Daten."}</p>}

        {stats && stats.total === 0 && (
          <p className="text-xs text-text-muted">Der Katalog ist leer — es gibt noch nichts auszuwerten.</p>
        )}

        {stats && stats.total > 0 && (
          <>
            <div className="grid gap-3 md:grid-cols-2">
              <section aria-label="Kameras" className="rounded-xl border border-border bg-bg-panel p-3">
                <h3 className="mb-2 flex items-center gap-1.5 text-xs font-semibold text-text-secondary">
                  <Camera aria-hidden="true" className="size-3.5" />
                  Kameras ({cameras.length})
                </h3>
                <ul className="flex flex-col gap-1" data-testid="gear-cameras">
                  {visibleCameras.map(([name, count]) => (
                    <li key={name}>
                      <button
                        type="button"
                        onClick={() => filterByCamera(name)}
                        aria-label={`Nur Fotos von ${name} zeigen`}
                        aria-pressed={libraryFilter.camera_model === name}
                        className="apx-btn-liquid w-full rounded px-1 py-0.5 text-left transition-colors duration-[var(--duration-fast)] hover:bg-accent/10"
                      >
                        <BarRow label={name} count={count} total={stats.total} unit={unit} />
                      </button>
                    </li>
                  ))}
                  {cameras.length === 0 && <li className="text-xs text-text-muted">Keine Kameraangaben im EXIF.</li>}
                </ul>
                {cameras.length > 6 && (
                  <button
                    type="button"
                    onClick={() => setShowAllCameras((current) => !current)}
                    className="mt-1 text-[11px] text-accent"
                  >
                    {showAllCameras ? "weniger zeigen" : `alle ${cameras.length} zeigen`}
                  </button>
                )}
              </section>

              <section aria-label="Objektive" className="rounded-xl border border-border bg-bg-panel p-3">
                <h3 className="mb-2 flex items-center gap-1.5 text-xs font-semibold text-text-secondary">
                  <Aperture aria-hidden="true" className="size-3.5" />
                  Objektive ({lenses.length})
                </h3>
                <ul className="flex flex-col gap-1" data-testid="gear-lenses">
                  {visibleLenses.map(([name, count]) => (
                    <li key={name} className="px-1 py-0.5">
                      <BarRow label={name} count={count} total={stats.total} unit={unit} />
                    </li>
                  ))}
                  {lenses.length === 0 && <li className="text-xs text-text-muted">Keine Objektivangaben im EXIF.</li>}
                </ul>
                {lenses.length > 6 && (
                  <button
                    type="button"
                    onClick={() => setShowAllLenses((current) => !current)}
                    className="mt-1 text-[11px] text-accent"
                  >
                    {showAllLenses ? "weniger zeigen" : `alle ${lenses.length} zeigen`}
                  </button>
                )}
              </section>
            </div>

            <div className="grid gap-3 md:grid-cols-2">
              <Distribution title="Brennweite" icon={<Ruler aria-hidden="true" className="size-3.5" />} buckets={stats.focal_lengths} total={stats.total} unit={unit} testId="gear-focal" />
              <Distribution title="Blende" icon={<Aperture aria-hidden="true" className="size-3.5" />} buckets={stats.apertures} total={stats.total} unit={unit} testId="gear-aperture" />
              <Distribution title="ISO" icon={<Gauge aria-hidden="true" className="size-3.5" />} buckets={stats.isos} total={stats.total} unit={unit} testId="gear-iso" />
              <Distribution title="Belichtungszeit" icon={<Timer aria-hidden="true" className="size-3.5" />} buckets={stats.shutters} total={stats.total} unit={unit} testId="gear-shutter" />
            </div>

            <p className="text-[11px] text-text-muted">
              Brennweiten sind die Werte aus dem EXIF, nicht auf Kleinbild umgerechnet — ohne verlässliche Sensorgröße wäre der
              Crop-Faktor geraten.
            </p>
          </>
        )}

        <div className="flex justify-end">
          <button type="button" onClick={onClose} className="apx-btn-liquid rounded border border-border px-3 py-1 text-xs text-text-secondary">
            Schließen
          </button>
        </div>
      </div>
    </Dialog>
  );
}

interface BarRowProps {
  label: string;
  count: number;
  total: number;
  unit: Unit;
  muted?: boolean;
}

/** Eine Zeile mit Beschriftung, Balken und Wert. Der Balken misst sich
 * am Gesamtbestand, nicht am größten Balken — sonst sähe eine Kamera mit
 * 3 von 10 000 Fotos genauso „voll" aus wie eine mit 9 000. */
function BarRow({ label, count, total, unit, muted }: BarRowProps) {
  const share = total > 0 ? (count / total) * 100 : 0;
  return (
    <span className="flex items-center gap-2 text-xs">
      <span className={`w-36 shrink-0 truncate ${muted ? "text-text-muted italic" : "text-text-primary"}`} title={label}>
        {label}
      </span>
      <span className="h-2 flex-1 overflow-hidden rounded-full bg-bg-raised">
        <span
          className={`block h-full rounded-full ${muted ? "bg-text-muted/40" : "bg-accent"}`}
          style={{ width: `${Math.max(share, count > 0 ? 1.5 : 0)}%` }}
        />
      </span>
      <span className="w-14 shrink-0 text-right tabular-nums text-text-secondary">
        {unit === "count" ? count : `${share.toFixed(1)} %`}
      </span>
    </span>
  );
}

interface DistributionProps {
  title: string;
  icon: React.ReactNode;
  buckets: DistributionBucketDto[];
  total: number;
  unit: Unit;
  testId: string;
}

function Distribution({ title, icon, buckets, total, unit, testId }: DistributionProps) {
  return (
    <section aria-label={title} data-testid={testId} className="rounded-xl border border-border bg-bg-panel p-3">
      <h3 className="mb-2 flex items-center gap-1.5 text-xs font-semibold text-text-secondary">
        {icon}
        {title}
      </h3>
      <ul className="flex flex-col gap-1">
        {buckets.map((bucket) => (
          <li key={bucket.label}>
            <BarRow label={bucket.label} count={bucket.count} total={total} unit={unit} muted={bucket.missing} />
          </li>
        ))}
      </ul>
    </section>
  );
}
