import { useEffect, useRef, useState } from "react";

import { DEFAULT_AMPLIFY, differenceAmount, differenceImage, type DifferenceMode } from "../lib/differenceImage";
import { previewUrl } from "../lib/media";

/**
 * Zeichnet das verstärkte Differenzbild zweier Fotos (Phase 34 F3,
 * siehe `DECISIONS.md` ADR-0070 und `lib/differenceImage.ts` für die
 * Mathematik).
 *
 * **Warum beide Bilder auf dieselbe Größe gezeichnet werden.** Die
 * Differenz ist pixelweise definiert; zwei Vorschauen mit
 * unterschiedlichen Kantenlängen (Hoch-/Querformat, andere
 * Zuschnitte) hätten sonst gar keine gemeinsame Grundlage.
 * `differenceImage` lehnt ungleich große Puffer ausdrücklich ab —
 * deshalb wird hier vorher auf ein gemeinsames Raster gezeichnet. Dass
 * dabei bei unterschiedlichem Seitenverhältnis verzerrt wird, ist der
 * ehrlichere Kompromiss als ein stillschweigender Zuschnitt, der eine
 * Verschiebung als Unterschied ausgäbe.
 */

/** Kantenlänge des gemeinsamen Rasters. Klein genug, um ohne spürbare
 * Verzögerung im Hauptthread zu rechnen, groß genug für Details. */
const GRID_EDGE = 512;

interface DifferenceCanvasProps {
  photoIdA: string;
  photoIdB: string;
  amplify: number;
  mode: DifferenceMode;
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error(`Bild konnte nicht geladen werden: ${src}`));
    img.src = src;
  });
}

export function DifferenceCanvas({ photoIdA, photoIdB, amplify, mode }: DifferenceCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [amount, setAmount] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setError(null);

    async function draw(): Promise<void> {
      const canvas = canvasRef.current;
      if (!canvas) return;
      const [a, b] = await Promise.all([
        loadImage(previewUrl(photoIdA)),
        loadImage(previewUrl(photoIdB)),
      ]);
      if (cancelled) return;

      const scratch = document.createElement("canvas");
      scratch.width = GRID_EDGE;
      scratch.height = GRID_EDGE;
      const scratchCtx = scratch.getContext("2d", { willReadFrequently: true });
      const ctx = canvas.getContext("2d");
      if (!scratchCtx || !ctx) return;

      scratchCtx.drawImage(a, 0, 0, GRID_EDGE, GRID_EDGE);
      const pixelsA = scratchCtx.getImageData(0, 0, GRID_EDGE, GRID_EDGE).data;
      scratchCtx.clearRect(0, 0, GRID_EDGE, GRID_EDGE);
      scratchCtx.drawImage(b, 0, 0, GRID_EDGE, GRID_EDGE);
      const pixelsB = scratchCtx.getImageData(0, 0, GRID_EDGE, GRID_EDGE).data;

      const diff = differenceImage(pixelsA, pixelsB, { amplify, mode });
      if (!diff || cancelled) return;
      canvas.width = GRID_EDGE;
      canvas.height = GRID_EDGE;
      // Der Umweg ueber `createImageData` statt `new ImageData(diff, …)`:
      // TypeScripts `ImageDataArray` verlangt einen `ArrayBuffer`, waehrend
      // ein frisch erzeugtes `Uint8ClampedArray` als `ArrayBufferLike`
      // typisiert ist (es koennte ein `SharedArrayBuffer` sein). Kopieren
      // statt casten — der Cast waere hier eine Behauptung ueber fremden
      // Speicher.
      const target = ctx.createImageData(GRID_EDGE, GRID_EDGE);
      target.data.set(diff);
      ctx.putImageData(target, 0, 0);
      setAmount(differenceAmount(pixelsA, pixelsB));
    }

    void draw().catch((err: unknown) => {
      if (!cancelled) setError(String(err));
    });
    return () => {
      cancelled = true;
    };
  }, [photoIdA, photoIdB, amplify, mode]);

  return (
    <figure className="flex min-h-0 flex-1 flex-col items-center gap-2" data-testid="difference-canvas">
      <canvas
        ref={canvasRef}
        aria-label="Differenzbild der beiden Aufnahmen"
        className="max-h-full max-w-full rounded border border-border object-contain"
      />
      <figcaption className="text-xs text-text-muted" data-testid="difference-amount">
        {error
          ? error
          : amount === null
            ? "Wird berechnet…"
            : amount === 0
              ? "Kein messbarer Unterschied — die beiden Vorschauen sind identisch."
              : `Mittlere Abweichung ${(amount * 100).toFixed(2)} % · ${amplify}-fach verstärkt`}
      </figcaption>
    </figure>
  );
}

export { DEFAULT_AMPLIFY };
