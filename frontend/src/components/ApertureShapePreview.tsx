import { useEffect, useRef } from "react";

/**
 * Live-Vorschau der Blendenform (Phase 30 Punkt 9, siehe
 * `DECISIONS.md` ADR-0060).
 *
 * Phase 28 hat der Virtuellen Blende Lamellenzahl, Drehung, anamorphe
 * Streckung und Wirbel gegeben — aber keine Rückmeldung, was diese
 * Zahlen bedeuten. Vier Regler ohne sichtbares Ergebnis sind Blindflug;
 * dieses Canvas zeigt genau den Kern, mit dem die Stufe gleich faltet.
 *
 * **Es ist bewusst dieselbe Formel wie in Rust** (`polygon_radius` und
 * `bokeh_kernel` in `stages::virtual_aperture`), einmal nachgezeichnet
 * statt geschätzt — eine hübsche, aber falsche Vorschau wäre
 * schlimmer als gar keine.
 */

export interface ApertureShapePreviewProps {
  blades: number;
  rotationDeg: number;
  anamorphic: number;
  /** Nur zur Anzeige: der Wirbel hängt von der Lage im Bild ab, die
   * Vorschau zeigt deshalb die Drehung an einer Beispielstelle. */
  swirl: number;
  size?: number;
}

/** Randradius eines regelmäßigen n-Ecks mit Inkreisradius 1 — dieselbe
 * Rechnung wie `polygon_radius` in `stages::virtual_aperture`. */
function polygonRadius(theta: number, blades: number, rotationRad: number): number {
  if (blades < 3) return 1;
  const sector = (Math.PI * 2) / blades;
  const a = (((theta + rotationRad) % sector) + sector) % sector - sector / 2;
  return Math.cos(sector / 2) / Math.max(1e-3, Math.cos(a));
}

export function ApertureShapePreview({
  blades,
  rotationDeg,
  anamorphic,
  swirl,
  size = 72,
}: ApertureShapePreviewProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;

    // Gleiche `devicePixelRatio`-Behandlung wie bei den übrigen
    // Canvas-Anzeigen des Projekts — sonst ist die Vorschau bei
    // skalierter Oberfläche unscharf.
    const dpr = Math.max(1, window.devicePixelRatio || 1);
    canvas.width = size * dpr;
    canvas.height = size * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, size, size);

    const center = size / 2;
    const radius = size * 0.38;
    const rotation = (rotationDeg * Math.PI) / 180;
    const stretch = 1 + Math.min(1, Math.max(0, anamorphic));
    const swirlRad = Math.min(1, Math.max(0, swirl)) * 0.9;

    ctx.beginPath();
    const steps = 180;
    for (let i = 0; i <= steps; i += 1) {
      const theta = (i / steps) * Math.PI * 2;
      const r = radius * polygonRadius(theta, blades, rotation);
      // Flächenerhaltend gestreckt, exakt wie `bokeh_kernel`.
      let x = (Math.cos(theta) * r) / stretch;
      let y = Math.sin(theta) * r * stretch;
      if (swirlRad > 0) {
        const c = Math.cos(swirlRad);
        const s = Math.sin(swirlRad);
        [x, y] = [x * c - y * s, x * s + y * c];
      }
      if (i === 0) ctx.moveTo(center + x, center + y);
      else ctx.lineTo(center + x, center + y);
    }
    ctx.closePath();

    const gradient = ctx.createRadialGradient(center, center, 0, center, center, radius * 1.4);
    gradient.addColorStop(0, "rgb(255 250 235 / 95%)");
    gradient.addColorStop(1, "rgb(255 235 190 / 35%)");
    ctx.fillStyle = gradient;
    ctx.fill();
    ctx.strokeStyle = "rgb(255 255 255 / 55%)";
    ctx.lineWidth = 1;
    ctx.stroke();
  }, [blades, rotationDeg, anamorphic, swirl, size]);

  return (
    <div className="flex items-center gap-3">
      <canvas
        ref={canvasRef}
        data-testid="aperture-shape-preview"
        aria-label="Vorschau der Blendenform"
        role="img"
        style={{ width: size, height: size }}
        className="rounded bg-bg-base"
      />
      <p className="text-xs text-text-muted">
        {blades < 3 ? "Runde Blende" : `${blades} Lamellen`}
        {anamorphic > 0 ? ", anamorph" : ""}
        {swirl > 0 ? ", Wirbel" : ""}
      </p>
    </div>
  );
}
