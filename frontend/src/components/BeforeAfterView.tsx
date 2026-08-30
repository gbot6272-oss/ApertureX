import { useEffect, useRef } from "react";

import { buildEdlEnvelopeJson, neutralEdlPayload } from "../lib/edl";
import { useDevelopPreviewThumbnail, type DevelopFrame } from "../hooks/useDevelopRender";
import { useAppStore } from "../store";

/** „Vorher" ist immer das neutrale EDL (wie aufgenommen) — ein einziges
 * konstantes JSON, unabhängig vom gerade offenen Foto oder dessen
 * Bearbeitungsstand (siehe `SPEC.md` §3.4 „Vorher/Nachher"). */
const NEUTRAL_EDL_JSON = buildEdlEnvelopeJson(neutralEdlPayload());

/** Zeichnet einen rohen RGBA8-`DevelopFrame` über `putImageData` — bewusst
 * einfacher Canvas-2D-Pfad statt des WebGL2-`QuadRenderer`s aus
 * `lib/webgl.ts` (der für den Haupt-Viewer wegen dessen Zoom/Pan-
 * Transformation gebraucht wird): die Vorher/Nachher-Ansicht braucht kein
 * Zoomen/Schwenken, nur eine unverzerrte Anzeige, und `object-fit:
 * contain` übernimmt dieselbe Größenanpassung, die dort manuell berechnet
 * werden muss. */
function FrameCanvas({ frame, className, style }: { frame: DevelopFrame | null; className: string; style?: React.CSSProperties }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !frame) return;
    canvas.width = frame.width;
    canvas.height = frame.height;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const imageData = new ImageData(new Uint8ClampedArray(frame.pixels), frame.width, frame.height);
    ctx.putImageData(imageData, 0, 0);
  }, [frame]);

  return <canvas ref={canvasRef} className={className} style={style} />;
}

const CANVAS_CLASS = "h-full w-full object-contain";

interface BeforeAfterViewProps {
  photoId: string | null;
  /** Das aktuelle EDL („Nachher") als bereits gebautes Envelope-JSON —
   * derselbe Wert, den auch der normale Live-Viewer rendert. */
  afterEdlJson: string | null;
  maxEdge: number | undefined;
}

/**
 * Vorher/Nachher-Ansicht (Phase 6 Schritt 8, `SPEC.md` §3.4 „in vier
 * Ansichten") — ersetzt den normalen Viewer-Inhalt vollständig, solange
 * `beforeAfterMode !== "none"` ist (siehe `Viewer.tsx`).
 *
 * **Vier Modi:** „Links/Rechts" und „Oben/Unten" zeigen zwei vollständige,
 * unabhängige Bilder nebeneinander bzw. übereinander. „Geteilt" und
 * „Geteilt vertikal" zeigen eine einzelne gemeinsame Bildfläche, in der
 * beide Zustände per `clip-path` je zur Hälfte sichtbar sind (Vorher
 * links/oben, Nachher rechts/unten) — dieselbe Bildposition auf beiden
 * Seiten der Trennlinie, weil beide Canvases dieselbe intrinsische Größe
 * (Vorher/Nachher-Frame haben identische Breite/Höhe, nur das EDL
 * unterscheidet sich) und dieselbe `object-fit: contain`-Skalierung im
 * selben Container erhalten.
 *
 * **Bewusste Vereinfachung:** die Trennlinie der geteilten Modi sitzt
 * fest bei 50 % — kein ziehbarer Regler. `SPEC.md` nennt nur die vier
 * Ansichten selbst, keinen ziehbaren Trennbalken; das wäre reine Politur
 * für einen späteren Schritt, falls gewünscht.
 */
export function BeforeAfterView({ photoId, afterEdlJson, maxEdge }: BeforeAfterViewProps) {
  const mode = useAppStore((s) => s.beforeAfterMode);
  const beforeFrame = useDevelopPreviewThumbnail(photoId, photoId ? NEUTRAL_EDL_JSON : null, maxEdge);
  const afterFrame = useDevelopPreviewThumbnail(photoId, afterEdlJson, maxEdge);

  if (mode === "none") return null;

  if (mode === "sideBySide" || mode === "stacked") {
    return (
      <div className={`absolute inset-0 flex ${mode === "sideBySide" ? "flex-row" : "flex-col"} gap-px bg-border`} aria-label="Vorher/Nachher">
        <div className="relative flex flex-1 items-center justify-center overflow-hidden bg-bg-base">
          <FrameCanvas frame={beforeFrame} className={CANVAS_CLASS} />
          <span className="pointer-events-none absolute left-2 top-2 rounded bg-bg-raised/80 px-1.5 py-0.5 text-xs text-text-secondary">Vorher</span>
        </div>
        <div className="relative flex flex-1 items-center justify-center overflow-hidden bg-bg-base">
          <FrameCanvas frame={afterFrame} className={CANVAS_CLASS} />
          <span className="pointer-events-none absolute left-2 top-2 rounded bg-bg-raised/80 px-1.5 py-0.5 text-xs text-text-secondary">Nachher</span>
        </div>
      </div>
    );
  }

  // "splitVertical"/"splitHorizontal": eine gemeinsame Fläche, je zur
  // Hälfte per clip-path sichtbar.
  const isVerticalSplit = mode === "splitVertical";
  return (
    <div className="absolute inset-0 flex items-center justify-center overflow-hidden bg-bg-base" aria-label="Vorher/Nachher">
      <div className="relative h-full w-full">
        <FrameCanvas
          frame={beforeFrame}
          className={`absolute inset-0 ${CANVAS_CLASS}`}
          style={{ clipPath: isVerticalSplit ? "inset(0 50% 0 0)" : "inset(0 0 50% 0)" }}
        />
        <FrameCanvas
          frame={afterFrame}
          className={`absolute inset-0 ${CANVAS_CLASS}`}
          style={{ clipPath: isVerticalSplit ? "inset(0 0 0 50%)" : "inset(50% 0 0 0)" }}
        />
        <div
          className="pointer-events-none absolute bg-accent"
          style={isVerticalSplit ? { left: "50%", top: 0, bottom: 0, width: 1 } : { top: "50%", left: 0, right: 0, height: 1 }}
        />
        <span className="pointer-events-none absolute left-2 top-2 rounded bg-bg-raised/80 px-1.5 py-0.5 text-xs text-text-secondary">Vorher</span>
        <span className="pointer-events-none absolute right-2 bottom-2 rounded bg-bg-raised/80 px-1.5 py-0.5 text-xs text-text-secondary">Nachher</span>
      </div>
    </div>
  );
}
