import { OVERLAY_COLOR_HEX, radialGradientBoundaryPoints, type Mask } from "../lib/edl";

interface MaskColorOverlayProps {
  /** Position/Größe des angezeigten Bildes in Bildschirm-Pixeln, wie bei
   * `CropOverlay`/`RepairOverlay`/`MaskOverlay` bereits von `Viewer.tsx`
   * berechnet. */
  imageLeft: number;
  imageTop: number;
  imageWidth: number;
  imageHeight: number;
  /** Bereits auf sichtbare Masken gefiltert (siehe `lib/edl.ts`s
   * `visibleMasks`) — diese Komponente selbst filtert nicht. */
  masks: readonly Mask[];
}

/**
 * Masken-Farbüberlagerung (Phase 12 Schritt 1, siehe `DECISIONS.md`
 * ADR-0039, `SPEC.md` §3.3 „Maskenüberlagerung in wählbarer Farbe") — macht
 * `overlay_color` (seit Phase 6 im EDL, bis hierher nie verdrahtet)
 * tatsächlich sichtbar: jede sichtbare Maske wird zusätzlich zu
 * `MaskOverlay`s Ziehgriffen (die nur für die *ausgewählte* Maske gelten)
 * flächig in ihrer eigenen Farbe eingefärbt gerendert, per Taste „O" (wie
 * Lightroom) ein-/ausblendbar.
 *
 * **Bewusste Vereinfachung:** dieselbe Vereinfachung wie bei `MaskOverlay`s
 * Radialverlauf-Ziehgriff — die Geometrie jeder Maskenkomponente wird direkt
 * als SVG-Form (Rechteck+Gradient/Ellipse+Gradient/Pinsellinien) gerendert,
 * *nicht* durch eine echte Pipeline-Anfrage der pixelgenau zusammengesetzten
 * Alpha (`apx_pipeline::stages::masks::compose_mask_alpha`, die reale
 * Quelle der tatsächlichen Wirkung). `combine`/`invert`/`feather` einzelner
 * Komponenten fließen deshalb **nicht** in diese rein visuelle Vorschau
 * ein — nur `mask.opacity` als grobe Gesamtdeckkraft. Für die in diesem
 * Schritt unterstützten Geometrietypen (Verläufe, Pinsel) liefert das
 * trotzdem eine originalgetreue Vorschau, weil die meisten Masken nur aus
 * einer einzigen Komponente ohne Kombinationslogik bestehen.
 */
export function MaskColorOverlay({ imageLeft, imageTop, imageWidth, imageHeight, masks }: MaskColorOverlayProps) {
  if (masks.length === 0) return null;

  return (
    <svg
      className="pointer-events-none absolute overflow-visible"
      style={{ left: imageLeft, top: imageTop, width: imageWidth, height: imageHeight }}
    >
      {masks.map((mask) => {
        const hex = OVERLAY_COLOR_HEX[mask.overlay_color];
        const groupOpacity = Math.max(0, Math.min(100, mask.opacity)) / 100;
        return (
          <g key={mask.id} opacity={groupOpacity}>
            {mask.components.map((component, index) => {
              const geometry = component.geometry;
              const gradId = `mask-overlay-${mask.id}-${index}`;

              if (geometry.kind === "LinearGradient") {
                return (
                  <g key={index}>
                    <defs>
                      <linearGradient
                        id={gradId}
                        x1={`${geometry.x1 * 100}%`}
                        y1={`${geometry.y1 * 100}%`}
                        x2={`${geometry.x2 * 100}%`}
                        y2={`${geometry.y2 * 100}%`}
                      >
                        <stop offset="0%" stopColor={hex} stopOpacity={0.55} />
                        <stop offset="100%" stopColor={hex} stopOpacity={0} />
                      </linearGradient>
                    </defs>
                    <rect x="0" y="0" width="100%" height="100%" fill={`url(#${gradId})`} />
                  </g>
                );
              }

              if (geometry.kind === "RadialGradient") {
                // Phase 12 Schritt 2 (siehe DECISIONS.md ADR-0039): die
                // Randform kommt jetzt aus `radialGradientBoundaryPoints`
                // (exakt dieselbe rotierte Ellipse wie `MaskOverlay`s
                // Ziehgriff-Kontur und `masks.rs`s `radial_gradient_alpha`),
                // statt einer achsenparallelen `<ellipse>`, die `angle_degrees`
                // ignoriert hätte. Der Verlauf selbst bleibt
                // `objectBoundingBox` auf dem Polygon — bei starker Rotation
                // ist sein Zentrum/seine Ausdehnung eine Näherung an die
                // eigentliche Bounding-Box statt exakt an die Ellipse
                // angepasst, für eine reine Vorschau-Einfärbung ausreichend.
                const boundary = radialGradientBoundaryPoints(geometry);
                return (
                  <g key={index}>
                    <defs>
                      <radialGradient id={gradId}>
                        <stop offset="0%" stopColor={hex} stopOpacity={0.55} />
                        <stop offset="100%" stopColor={hex} stopOpacity={0} />
                      </radialGradient>
                    </defs>
                    <svg width="100%" height="100%" preserveAspectRatio="none" viewBox="0 0 100 100">
                      <polygon points={boundary.map((p) => `${p.x * 100},${p.y * 100}`).join(" ")} fill={`url(#${gradId})`} />
                    </svg>
                  </g>
                );
              }

              if (geometry.kind === "Brush") {
                return (
                  <svg key={index} className="overflow-visible" width="100%" height="100%" preserveAspectRatio="none" viewBox="0 0 100 100">
                    {geometry.strokes.map((stroke, strokeIndex) => (
                      <polyline
                        key={strokeIndex}
                        points={stroke.points.map((p) => `${p.x * 100},${p.y * 100}`).join(" ")}
                        fill="none"
                        stroke={hex}
                        strokeWidth={Math.max(0.4, stroke.radius * 100)}
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        opacity={0.55}
                        vectorEffect="non-scaling-stroke"
                      />
                    ))}
                  </svg>
                );
              }

              // Andere Geometrietypen (Farbbereich/Luminanzbereich/KI-
              // erzeugt/Tiefennäherung) haben keine einfache Vektorform —
              // eine flächentreue Vorschau bräuchte ohnehin die echte
              // Pipeline-Alpha, siehe Moduldoku oben. Kein Overlay statt
              // einer irreführenden Näherung.
              return null;
            })}
          </g>
        );
      })}
    </svg>
  );
}
