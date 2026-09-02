import { useEffect } from "react";

import { BASIC_SLIDER_SPECS } from "../lib/edl";
import { useAppStore } from "../store";
import { DevelopSlider } from "./DevelopSlider";

/** Dieselben sieben Phase-2-Basisregler wie in `DevelopPanel.tsx`s
 * Grundeinstellungen-Abschnitt — Reihenfolge wie `SPEC.md` §3.2, ohne
 * Weißabgleich (Temperatur/Tint) und die fünf Phase-4-Regler
 * (Textur/Klarheit/Dunst entfernen/Dynamik werden hier bewusst
 * weggelassen; Sättigung bleibt als siebter Regler dabei — genau die in
 * PLAN.md Phase 11 Schritt 3 benannte Liste). */
const QUICK_DEVELOP_KEYS = ["exposure_ev", "contrast", "highlights", "shadows", "whites", "blacks", "saturation"] as const;
const QUICK_DEVELOP_SPECS = BASIC_SLIDER_SPECS.filter((spec) => (QUICK_DEVELOP_KEYS as readonly string[]).includes(spec.key));

interface QuickDevelopOverlayProps {
  photoId: string;
}

/**
 * Schnellentwicklung im Raster (Phase 11 Schritt 3, siehe `DECISIONS.md`
 * ADR-0038): kompaktes Overlay mit den sieben Basisreglern für eine
 * einzelne Rasterkachel der Übersichtsansicht — lädt den aktuellen
 * Bearbeitungsstand des Fotos beim Erscheinen (Hover/Auswahl der Kachel,
 * siehe `GridView.tsx`) und committet über denselben
 * `apply_develop_edit`-Pfad wie das Entwickeln-Panel, aber bewusst über
 * einen eigenen Store-Zustand (`quickDevelopEdl`) statt `developEdl` —
 * siehe dessen Moduldoku in `store/index.ts`.
 */
export function QuickDevelopOverlay({ photoId }: QuickDevelopOverlayProps) {
  const quickDevelopPhotoId = useAppStore((s) => s.quickDevelopPhotoId);
  const quickDevelopEdl = useAppStore((s) => s.quickDevelopEdl);
  const loadQuickDevelopStateForPhoto = useAppStore((s) => s.loadQuickDevelopStateForPhoto);
  const setQuickDevelopBasicField = useAppStore((s) => s.setQuickDevelopBasicField);
  const commitQuickDevelopEdit = useAppStore((s) => s.commitQuickDevelopEdit);
  const clearQuickDevelopState = useAppStore((s) => s.clearQuickDevelopState);

  useEffect(() => {
    void loadQuickDevelopStateForPhoto(photoId);
    return () => clearQuickDevelopState();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- lädt gezielt nur bei einem Kachelwechsel neu, nicht bei jeder Store-Referenzänderung
  }, [photoId]);

  if (quickDevelopPhotoId !== photoId || !quickDevelopEdl) {
    return (
      <div
        className="absolute inset-x-1 bottom-1 rounded bg-bg-base/90 px-2 py-1 text-[10px] text-text-muted"
        onClick={(event) => event.stopPropagation()}
      >
        Lädt…
      </div>
    );
  }

  return (
    <div
      className="absolute inset-x-1 bottom-1 flex flex-col gap-1 rounded bg-bg-base/95 p-2 text-text-primary shadow-lg"
      onClick={(event) => event.stopPropagation()}
      onKeyDown={(event) => event.stopPropagation()}
      role="group"
      aria-label="Schnellentwicklung"
    >
      {QUICK_DEVELOP_SPECS.map((spec) => (
        <DevelopSlider
          key={spec.key}
          spec={spec}
          value={quickDevelopEdl.basic[spec.key as keyof typeof quickDevelopEdl.basic] as number}
          onChange={(value) => setQuickDevelopBasicField(spec.key, value)}
          onCommit={() => void commitQuickDevelopEdit()}
        />
      ))}
    </div>
  );
}
