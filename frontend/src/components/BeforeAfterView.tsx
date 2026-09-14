import { useCallback, useEffect, useRef, useState } from "react";

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

  // "splitVertical"/"splitHorizontal": eine gemeinsame Fläche, per
  // clip-path geteilt — die Kante ist seit Phase 31 Schritt 6 ZIEHBAR.
  //
  // Vorher sass sie fest bei 50 % und trug sogar `pointer-events-none`.
  // Das machte den Modus für den häufigsten Fall unbrauchbar: man will
  // die Kante über die Stelle schieben, an der man gerade etwas geändert
  // hat, und die liegt selten genau in der Bildmitte.
  return (
    <SplitCompare beforeFrame={beforeFrame} afterFrame={afterFrame} vertical={mode === "splitVertical"} />
  );
}

/** Die ziehbare Vorher/Nachher-Kante. */
function SplitCompare({
  beforeFrame,
  afterFrame,
  vertical,
}: {
  // Wie bei den übrigen Zweigen dieser Datei dürfen beide Rahmen noch
  // fehlen (Vorschau lädt); `FrameCanvas` zeichnet dann nichts.
  beforeFrame: DevelopFrame | null;
  afterFrame: DevelopFrame | null;
  vertical: boolean;
}) {
  const [position, setPosition] = useState(50);
  const areaRef = useRef<HTMLDivElement>(null);
  const draggingRef = useRef(false);

  const moveTo = useCallback(
    (clientX: number, clientY: number) => {
      const rect = areaRef.current?.getBoundingClientRect();
      if (!rect || rect.width === 0 || rect.height === 0) return;
      const raw = vertical
        ? ((clientX - rect.left) / rect.width) * 100
        : ((clientY - rect.top) / rect.height) * 100;
      // Auf 2…98 statt 0…100 geklemmt: eine Kante ganz am Rand sieht aus
      // wie ein Fehler ("eine Hälfte fehlt") und lässt sich zudem kaum
      // wieder zurückgreifen.
      setPosition(Math.max(2, Math.min(98, raw)));
    },
    [vertical],
  );

  useEffect(() => {
    function onMove(event: MouseEvent) {
      if (!draggingRef.current) return;
      event.preventDefault();
      moveTo(event.clientX, event.clientY);
    }
    function onUp() {
      draggingRef.current = false;
    }
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [moveTo]);

  function onKeyDown(event: React.KeyboardEvent) {
    const back = vertical ? "ArrowLeft" : "ArrowUp";
    const forward = vertical ? "ArrowRight" : "ArrowDown";
    if (event.key !== back && event.key !== forward) return;
    event.preventDefault();
    const step = event.shiftKey ? 10 : 2;
    setPosition((previous) =>
      Math.max(2, Math.min(98, previous + (event.key === forward ? step : -step))),
    );
  }

  return (
    <div className="absolute inset-0 flex items-center justify-center overflow-hidden bg-bg-base" aria-label="Vorher/Nachher">
      <div ref={areaRef} className="relative h-full w-full">
        <FrameCanvas
          frame={beforeFrame}
          className={`absolute inset-0 ${CANVAS_CLASS}`}
          style={{ clipPath: vertical ? `inset(0 ${100 - position}% 0 0)` : `inset(0 0 ${100 - position}% 0)` }}
        />
        <FrameCanvas
          frame={afterFrame}
          className={`absolute inset-0 ${CANVAS_CLASS}`}
          style={{ clipPath: vertical ? `inset(0 0 0 ${position}%)` : `inset(${position}% 0 0 0)` }}
        />
        {/* Der Griff ist bewusst breiter als die sichtbare Linie: eine
            1px-Trefferfläche wäre mit der Maus kaum zu fassen. Die Linie
            selbst bleibt 1px, damit sie das Bild nicht zerschneidet. */}
        <div
          role="slider"
          tabIndex={0}
          // Bewusst NICHT mit "Vorher/Nachher" beginnend: `getByLabel` sucht
          // per Teilzeichenkette, und die umgebende Fläche trägt genau
          // dieses Label — ein Präfix hier machte jede Suche nach der Fläche
          // mehrdeutig (real im Testlauf aufgeschlagen).
          aria-label={vertical ? "Trennkante waagerecht verschieben" : "Trennkante senkrecht verschieben"}
          aria-orientation={vertical ? "horizontal" : "vertical"}
          aria-valuemin={2}
          aria-valuemax={98}
          aria-valuenow={Math.round(position)}
          data-testid="before-after-handle"
          onMouseDown={(event) => {
            if (event.button !== 0) return;
            draggingRef.current = true;
            moveTo(event.clientX, event.clientY);
          }}
          onKeyDown={onKeyDown}
          className={`absolute flex items-center justify-center ${
            vertical ? "top-0 bottom-0 w-6 cursor-col-resize" : "left-0 right-0 h-6 cursor-row-resize"
          }`}
          style={vertical ? { left: `calc(${position}% - 0.75rem)` } : { top: `calc(${position}% - 0.75rem)` }}
        >
          <div className={`bg-accent ${vertical ? "h-full w-px" : "h-px w-full"}`} />
          <div className="absolute size-4 rounded-full border border-accent bg-bg-raised" />
        </div>
        <span className="pointer-events-none absolute left-2 top-2 rounded bg-bg-raised/80 px-1.5 py-0.5 text-xs text-text-secondary">Vorher</span>
        <span className="pointer-events-none absolute right-2 bottom-2 rounded bg-bg-raised/80 px-1.5 py-0.5 text-xs text-text-secondary">Nachher</span>
      </div>
    </div>
  );
}
