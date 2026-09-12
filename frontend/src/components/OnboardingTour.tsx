import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import gsap from "gsap";

import { useFocusTrap } from "../lib/a11y";
import { useT, type TranslationKey } from "../lib/i18n";
import { usePrefersReducedMotion } from "../lib/motion";
import { playCue } from "../lib/sound";

interface OnboardingTourProps {
  open: boolean;
  onClose: () => void;
}

interface Rect {
  left: number;
  top: number;
  width: number;
  height: number;
}

interface TourStep {
  titleKey: TranslationKey;
  bodyKey: TranslationKey;
  /** `null` = keine Hervorhebung (zentrierte Karte, z. B. Begrüßung). */
  target: () => HTMLElement | null;
  showSamplePhoto?: boolean;
}

const SPOTLIGHT_PADDING = 10;
const TOUR_EASE = "power2.inOut";
const TOUR_DURATION = 0.5;

/** Liefert das Zielelement für einen `data-tour`-Schritt, oder `null`,
 * wenn es (noch) nicht im DOM ist — die Karte fällt dann auf eine
 * zentrierte Ansicht ohne Ausschnitt zurück statt abzustürzen. */
function byDataTour(name: string): HTMLElement | null {
  return document.querySelector<HTMLElement>(`[data-tour="${name}"]`);
}

/** Stilisiertes Beispielfoto (Phase 21, siehe `DECISIONS.md` ADR-0049)
 * — reines Inline-SVG statt eines echten Fotos: die Netzwerk-Policy
 * dieser Umgebung blockiert jede externe Bildquelle (siehe ADR-0047),
 * und vor dem ersten Import liegt ohnehin kein echtes Nutzerfoto vor.
 * Zeigt trotzdem sofort, wie eine Kachel/ein Bild in der App aussieht
 * (Berg-Silhouette + Sonne vor Himmelsverlauf), statt die Begrüßung
 * rein textlich zu lassen. */
function SampleTourPhoto() {
  return (
    <svg viewBox="0 0 240 150" className="h-auto w-full rounded-lg" role="img" aria-label="Beispielfoto">
      <defs>
        <linearGradient id="apx-tour-sky" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="#5b9bd5" />
          <stop offset="65%" stopColor="#f2c98e" />
          <stop offset="100%" stopColor="#f6e3c0" />
        </linearGradient>
      </defs>
      <rect width="240" height="150" rx="10" fill="url(#apx-tour-sky)" />
      <circle cx="176" cy="46" r="16" fill="#fff7e6" opacity="0.9" />
      <path d="M0 118 L52 66 L88 96 L128 54 L168 104 L200 82 L240 118 L240 150 L0 150 Z" fill="#232019" opacity="0.55" />
      <path d="M0 134 L70 100 L120 126 L180 96 L240 134 L240 150 L0 150 Z" fill="#141210" opacity="0.7" />
    </svg>
  );
}

/**
 * Spotlight-Einführungstour (Phase 21, siehe `DECISIONS.md` ADR-0049)
 * — ersetzt die vorherige `OnboardingDialog.tsx` (eine reine, alles auf
 * einmal zeigende Textliste). Hebt stattdessen Schritt für Schritt
 * echte, gerade sichtbare Bedienelemente hervor (Sidebar/Import/
 * Ansicht-Umschalter/Entwickeln/Suche/Filmstreifen, per
 * `data-tour="..."`-Attribut markiert), mit derselben `apx-glass`-
 * Materialoptik wie der Rest der App seit dieser Phase.
 *
 * **Technik der Aussparung:** vier Abdunkel-/Weichzeichner-Rechtecke
 * (oben/unten/links/rechts) umrahmen das Zielelement, statt eines
 * einzelnen Vollbild-Overlays mit CSS-`mask`-Ausschnitt — robuster
 * browserübergreifend und ohne Masken-/Stapelkontext-Fallstricke (ein
 * `backdrop-filter` wirkt nur innerhalb der eigenen Box eines
 * Elements, ein `mask`-Ausschnitt hätte das rechteckige Loch separat
 * nachbilden müssen). Ohne Zielelement (Begrüßung/Abschluss) haben
 * alle vier Rechtecke eine Breite/Höhe von 0 an einem zentrierten
 * Phantompunkt — dieselbe Berechnung deckt dann lückenlos den gesamten
 * Bildschirm ab, ohne einen Sonderfall im Rendering zu brauchen.
 *
 * **UX-Pflicht** (siehe `ui-ux-pro-max`-Skill-Recherche, Kategorie
 * "Onboarding"/"User Freedom"): jederzeit überspring- und zurück-
 * navigierbar, keine erzwungene lineare Tour.
 */
export function OnboardingTour({ open, onClose }: OnboardingTourProps) {
  const t = useT();
  const reducedMotion = usePrefersReducedMotion();
  const [stepIndex, setStepIndex] = useState(0);
  const [mounted, setMounted] = useState(open);
  const cardRef = useRef<HTMLDivElement>(null);
  const topRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const leftRef = useRef<HTMLDivElement>(null);
  const rightRef = useRef<HTMLDivElement>(null);
  const isFirstPositionRef = useRef(true);

  useFocusTrap(cardRef, open);

  const steps = useMemo<TourStep[]>(
    () => [
      { titleKey: "onboarding.tour.welcome.title", bodyKey: "onboarding.tour.welcome.body", target: () => null, showSamplePhoto: true },
      { titleKey: "onboarding.tour.sidebar.title", bodyKey: "onboarding.tour.sidebar.body", target: () => document.querySelector<HTMLElement>(`aside[aria-label="${t("sidebar.heading")}"]`) },
      { titleKey: "onboarding.tour.import.title", bodyKey: "onboarding.tour.import.body", target: () => byDataTour("import") },
      { titleKey: "onboarding.tour.views.title", bodyKey: "onboarding.tour.views.body", target: () => byDataTour("views") },
      { titleKey: "onboarding.tour.develop.title", bodyKey: "onboarding.tour.develop.body", target: () => byDataTour("develop") },
      { titleKey: "onboarding.tour.search.title", bodyKey: "onboarding.tour.search.body", target: () => byDataTour("search") },
      { titleKey: "onboarding.tour.filmstrip.title", bodyKey: "onboarding.tour.filmstrip.body", target: () => byDataTour("filmstrip") },
    ],
    [t],
  );

  useEffect(() => {
    if (open) {
      setMounted(true);
      setStepIndex(0);
      isFirstPositionRef.current = true;
      playCue("open");
    } else if (mounted) {
      playCue("close");
      setMounted(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `mounted` bewusst nicht in den Deps: soll nur auf `open` reagieren.
  }, [open]);

  // Positioniert die vier Rahmen-Rechtecke + die Glas-Karte für den
  // aktuellen Schritt — läuft bei Schritt-Wechsel und bei
  // Fenstergrößenänderung (Zielelemente können sich verschieben).
  useLayoutEffect(() => {
    if (!mounted) return;

    function position() {
      const step = steps[stepIndex];
      const el = step?.target() ?? null;
      const vw = window.innerWidth;
      const vh = window.innerHeight;

      let rect: Rect;
      if (el) {
        const box = el.getBoundingClientRect();
        rect = {
          left: Math.max(0, box.left - SPOTLIGHT_PADDING),
          top: Math.max(0, box.top - SPOTLIGHT_PADDING),
          width: Math.min(vw, box.width + SPOTLIGHT_PADDING * 2),
          height: Math.min(vh, box.height + SPOTLIGHT_PADDING * 2),
        };
      } else {
        // Phantom-Punkt in der Bildschirmmitte, Breite/Höhe 0 — siehe
        // Moduldoku oben: dieselbe Vier-Rechteck-Formel deckt dann ohne
        // Sonderfall den ganzen Bildschirm ab.
        rect = { left: vw / 2, top: vh / 2, width: 0, height: 0 };
      }

      const panels = {
        top: { left: 0, top: 0, width: vw, height: rect.top },
        bottom: { left: 0, top: rect.top + rect.height, width: vw, height: Math.max(0, vh - (rect.top + rect.height)) },
        left: { left: 0, top: rect.top, width: rect.left, height: rect.height },
        right: { left: rect.left + rect.width, top: rect.top, width: Math.max(0, vw - (rect.left + rect.width)), height: rect.height },
      };

      const targets = [
        [topRef.current, panels.top],
        [bottomRef.current, panels.bottom],
        [leftRef.current, panels.left],
        [rightRef.current, panels.right],
      ] as const;

      for (const [node, box] of targets) {
        if (!node) continue;
        if (isFirstPositionRef.current || reducedMotion) {
          gsap.set(node, box);
        } else {
          gsap.to(node, { ...box, duration: TOUR_DURATION, ease: TOUR_EASE });
        }
      }

      // Karte unterhalb des Ziels platzieren, wenn Platz ist, sonst
      // darüber; ohne Ziel (Begrüßung/Abschluss) zentriert.
      const card = cardRef.current;
      if (card) {
        const cardHeight = card.offsetHeight || 200;
        const cardWidth = card.offsetWidth || 320;
        let cardLeft: number;
        let cardTop: number;
        if (el) {
          cardLeft = Math.min(Math.max(8, rect.left), vw - cardWidth - 8);
          const spaceBelow = vh - (rect.top + rect.height);
          cardTop = spaceBelow > cardHeight + 24 ? rect.top + rect.height + 16 : Math.max(8, rect.top - cardHeight - 16);
        } else {
          cardLeft = vw / 2 - cardWidth / 2;
          cardTop = vh / 2 - cardHeight / 2;
        }
        if (isFirstPositionRef.current || reducedMotion) {
          gsap.set(card, { left: cardLeft, top: cardTop });
        } else {
          gsap.to(card, { left: cardLeft, top: cardTop, duration: TOUR_DURATION, ease: TOUR_EASE });
        }
      }

      isFirstPositionRef.current = false;
    }

    position();
    window.addEventListener("resize", position);
    return () => window.removeEventListener("resize", position);
  }, [mounted, stepIndex, steps, reducedMotion]);

  useEffect(() => {
    if (!open) return;
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open, onClose]);

  if (!mounted) return null;

  // `stepIndex` bleibt per Konstruktion (`Math.max`/`Math.min` bei den
  // Zurück-/Weiter-Knöpfen, `setStepIndex(0)` beim Öffnen) stets im
  // gültigen Bereich — die Absicherung hier ist reine Typsicherheit
  // (`noUncheckedIndexedAccess`), kein erwarteter Laufzeitfall.
  const step = steps[stepIndex] ?? steps[0];
  if (!step) return null;
  const isLast = stepIndex === steps.length - 1;

  return (
    <div aria-hidden={false} className="pointer-events-none fixed inset-0 z-[150]">
      <div ref={topRef} className="apx-glass-strong pointer-events-auto fixed" onClick={onClose} />
      <div ref={bottomRef} className="apx-glass-strong pointer-events-auto fixed" onClick={onClose} />
      <div ref={leftRef} className="apx-glass-strong pointer-events-auto fixed" onClick={onClose} />
      <div ref={rightRef} className="apx-glass-strong pointer-events-auto fixed" onClick={onClose} />

      <div
        ref={cardRef}
        role="dialog"
        aria-modal="true"
        aria-label={t(step.titleKey)}
        className="apx-glass-strong pointer-events-auto fixed flex w-80 flex-col gap-3 rounded-xl border border-[var(--glass-border)] p-4 shadow-xl"
      >
        {step.showSamplePhoto && (
          <div className="overflow-hidden rounded-lg">
            <SampleTourPhoto />
          </div>
        )}
        <div>
          <p className="text-xs text-text-muted">{t("onboarding.tour.stepLabel", { current: stepIndex + 1, total: steps.length })}</p>
          <h2 className="text-sm font-semibold text-text-primary">{t(step.titleKey)}</h2>
          <p className="mt-1 text-xs text-text-secondary">{t(step.bodyKey)}</p>
        </div>

        <div className="flex items-center justify-center gap-1.5" aria-hidden="true">
          {steps.map((s, i) => (
            <span
              key={s.titleKey}
              className={`h-1.5 w-1.5 rounded-full transition-colors duration-[var(--duration-fast)] ${i === stepIndex ? "bg-accent" : "bg-border"}`}
            />
          ))}
        </div>

        <div className="flex items-center justify-between gap-2">
          <button type="button" onClick={onClose} className="text-xs text-text-muted hover:text-text-secondary">
            {t("onboarding.tour.skip")}
          </button>
          <div className="flex items-center gap-2">
            {stepIndex > 0 && (
              <button
                type="button"
                onClick={() => setStepIndex((i) => Math.max(0, i - 1))}
                className="rounded border border-border px-3 py-1.5 text-xs text-text-secondary hover:border-accent hover:text-text-primary"
              >
                {t("onboarding.tour.back")}
              </button>
            )}
            <button
              type="button"
              onClick={() => (isLast ? onClose() : setStepIndex((i) => Math.min(steps.length - 1, i + 1)))}
              className="rounded border border-accent bg-accent/10 px-3 py-1.5 text-xs text-accent hover:bg-accent/20"
            >
              {t(isLast ? "onboarding.tour.finish" : "onboarding.tour.next")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
