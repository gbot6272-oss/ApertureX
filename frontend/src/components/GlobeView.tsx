import { useCallback, useEffect, useRef, useState } from "react";

import worldLandRings from "../assets/world-land-110m.json";
import { limbDarkening, projectLatLon, rotateSphere, latLonToUnitSphere, type GlobeRotation } from "../lib/geoProjection";
import { binPointsIntoGrid, heatScaleColor, normalizeHeatIntensity, rgbaCss, type GeoPoint } from "../lib/photoHeatmap";
import type { PhotoDto } from "../lib/tauri";

/** `[lon, lat]`-Ringe (Länderumrisse, 110m-Natural-Earth-Auflösung, in
 * `world-land-110m.json` aus dem npm-Paket `world-atlas` einmalig
 * vorab zu einem flachen Ringe-Array konvertiert — siehe
 * `DECISIONS.md` ADR-0044 für die genaue Herkunft/Lizenz). */
const LAND_RINGS = worldLandRings as [number, number][][];

const AUTO_ROTATE_DEG_PER_SEC = 4;
const DRAG_SENSITIVITY = 0.35;
const HEAT_CELL_SIZE_DEG = 6;

/** Einmal pro Zeichnung gelesene Theme-Farben statt hartkodierter Hex-
 * Werte — folgt automatisch Dark-/Hell-/Kontrastmodus und der
 * benutzerdefinierten Akzentfarbe (Phase 10 Schritt 7), siehe
 * `index.css`s `@theme`-Tokens. */
function readThemeColors(el: HTMLElement) {
  const style = getComputedStyle(el);
  const read = (name: string, fallback: string) => style.getPropertyValue(name).trim() || fallback;
  return {
    bgBase: read("--color-bg-base", "#1a1a1a"),
    bgRaised: read("--color-bg-raised", "#202020"),
    border: read("--color-border", "#333333"),
    accent: read("--color-accent", "#5b9bd5"),
    danger: read("--color-danger", "#e07a5f"),
    textMuted: read("--color-text-muted", "#6a6a6a"),
  };
}

function hexToRgba(hex: string, alpha: number): string {
  const clean = hex.replace("#", "");
  const r = parseInt(clean.slice(0, 2), 16);
  const g = parseInt(clean.slice(2, 4), 16);
  const b = parseInt(clean.slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

interface GlobeViewProps {
  photos: PhotoDto[];
  onEnterMap: () => void;
}

/**
 * Foto-Globus (siehe `DECISIONS.md` ADR-0044) — die "kleinste Ebene"
 * der Kartenansicht (`MapView.tsx`): eine selbst gerenderte, dreh-
 * bare 3D-Kugel (orthographische Projektion, `lib/geoProjection.ts`)
 * statt einer flachen Weltkarte, mit einer Foto-Dichte-Heatmap auf der
 * Oberfläche (dieselbe Logik wie die flache Karte, `lib/
 * photoHeatmap.ts`) — keine einzelnen Pins hier (die gibt es erst auf
 * der flachen Karte bei sehr großem Zoom, siehe `MapView.tsx`).
 *
 * Reines Canvas 2D (kein WebGL/Three.js) — eine orthographische
 * Kugelprojektion ist reine 2D-Zeichenarbeit mit vorab projizierten
 * Punkten, für eine UI-Panel-Größe performant genug und ohne die
 * Bundle-Größe/Komplexität einer 3D-Engine.
 */
export function GlobeView({ photos, onEnterMap }: GlobeViewProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const rotationRef = useRef<GlobeRotation>({ yaw: 20, pitch: -15 });
  const draggingRef = useRef<{ pointerId: number; lastX: number; lastY: number } | null>(null);
  const [reducedMotion, setReducedMotion] = useState(
    () => typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  );

  useEffect(() => {
    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    const onChange = () => setReducedMotion(media.matches);
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, []);

  const heatCells = useCallback(() => {
    const points: GeoPoint[] = photos
      .filter((p) => p.gps_lat !== null && p.gps_lon !== null)
      .map((p) => ({ lat: p.gps_lat as number, lon: p.gps_lon as number }));
    return binPointsIntoGrid(points, HEAT_CELL_SIZE_DEG);
  }, [photos]);

  useEffect(() => {
    const canvas = canvasRef.current;
    const container = containerRef.current;
    if (!canvas || !container) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let raf = 0;
    let lastFrameTime: number | null = null;
    let width = 0;
    let height = 0;
    const dpr = Math.max(1, window.devicePixelRatio || 1);

    function resize() {
      if (!canvas || !container) return;
      width = container.clientWidth;
      height = container.clientHeight;
      canvas.width = Math.round(width * dpr);
      canvas.height = Math.round(height * dpr);
      canvas.style.width = `${width}px`;
      canvas.style.height = `${height}px`;
    }
    resize();
    const resizeObserver = new ResizeObserver(resize);
    resizeObserver.observe(container);

    function draw() {
      if (!ctx || !canvas) return;
      const colors = readThemeColors(canvas);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, width, height);

      const cx = width / 2;
      const cy = height / 2;
      const radius = Math.min(width, height) * 0.42;
      const rotation = rotationRef.current;

      // Sanftes Glühen hinter der Kugel — Akzentfarbe, sehr transparent
      // (dieselbe "techy Glow"-Ästhetik wie Fokusringe/aktive Zustände
      // sonst im UI, hier nur großflächiger).
      const glow = ctx.createRadialGradient(cx, cy, radius * 0.6, cx, cy, radius * 1.6);
      glow.addColorStop(0, hexToRgba(colors.accent, 0.12));
      glow.addColorStop(1, hexToRgba(colors.accent, 0));
      ctx.fillStyle = glow;
      ctx.fillRect(0, 0, width, height);

      // Kugelkörper: radialer Verlauf für einen dezenten
      // Beleuchtungs-Eindruck (heller zur Bildmitte/oben-links, dunkler
      // zum Rand) statt einer flachen Füllung.
      const sphereGradient = ctx.createRadialGradient(
        cx - radius * 0.3,
        cy - radius * 0.3,
        radius * 0.1,
        cx,
        cy,
        radius,
      );
      sphereGradient.addColorStop(0, colors.bgRaised);
      sphereGradient.addColorStop(1, colors.bgBase);
      ctx.fillStyle = sphereGradient;
      ctx.beginPath();
      ctx.arc(cx, cy, radius, 0, Math.PI * 2);
      ctx.fill();

      // Kugelrand: dünner Akzent-Ring statt einer harten Kante.
      ctx.strokeStyle = hexToRgba(colors.accent, 0.5);
      ctx.lineWidth = 1;
      ctx.stroke();

      ctx.save();
      ctx.beginPath();
      ctx.arc(cx, cy, radius, 0, Math.PI * 2);
      ctx.clip();

      // Gradnetz (Meridiane/Breitenkreise alle 30°) — liest sich sofort
      // als "Globus", auch bevor Landmasse/Heatmap dazukommen.
      ctx.strokeStyle = hexToRgba(colors.border, 0.9);
      ctx.lineWidth = 0.75;
      for (let lon = -180; lon < 180; lon += 30) {
        ctx.beginPath();
        let started = false;
        for (let lat = -90; lat <= 90; lat += 3) {
          const p = projectLatLon(lat, lon, rotation, radius);
          if (!p.visible) {
            started = false;
            continue;
          }
          if (!started) {
            ctx.moveTo(cx + p.x, cy + p.y);
            started = true;
          } else {
            ctx.lineTo(cx + p.x, cy + p.y);
          }
        }
        ctx.stroke();
      }
      for (let lat = -60; lat <= 60; lat += 30) {
        ctx.beginPath();
        let started = false;
        for (let lon = -180; lon <= 180; lon += 3) {
          const p = projectLatLon(lat, lon, rotation, radius);
          if (!p.visible) {
            started = false;
            continue;
          }
          if (!started) {
            ctx.moveTo(cx + p.x, cy + p.y);
            started = true;
          } else {
            ctx.lineTo(cx + p.x, cy + p.y);
          }
        }
        ctx.stroke();
      }

      // Landmasse: pro Ring zusammenhängende sichtbare Läufe als
      // gefüllte Pfade zeichnen (siehe Moduldoku unten für die
      // bewusste Vereinfachung ggü. echtem Horizont-Clipping).
      ctx.fillStyle = hexToRgba(colors.textMuted, 0.9);
      for (const ring of LAND_RINGS) {
        let run: { x: number; y: number }[] = [];
        const flushRun = () => {
          if (run.length >= 3) {
            ctx.beginPath();
            ctx.moveTo(cx + run[0]!.x, cy + run[0]!.y);
            for (let i = 1; i < run.length; i++) ctx.lineTo(cx + run[i]!.x, cy + run[i]!.y);
            ctx.closePath();
            ctx.fill();
          }
          run = [];
        };
        for (const [lon, lat] of ring) {
          const p = projectLatLon(lat, lon, rotation, radius);
          if (p.visible) {
            run.push({ x: p.x, y: p.y });
          } else {
            flushRun();
          }
        }
        flushRun();
      }

      // Foto-Dichte-Heatmap: additive Glow-Blobs, Farbe von kühl
      // (wenig Fotos) über die Akzentfarbe bis zur Warnfarbe (viele
      // Fotos an einem Ort) — siehe `lib/photoHeatmap.ts`s Moduldoku
      // für die bewusste Ableitung aus den Theme-Farben statt einer
      // generischen Regenbogen-Skala.
      const cells = heatCells();
      const maxCount = cells.reduce((max, c) => Math.max(max, c.count), 0);
      ctx.globalCompositeOperation = "lighter";
      for (const cell of cells) {
        const rotated = rotateSphere(latLonToUnitSphere(cell.lat, cell.lon), rotation);
        const darkening = limbDarkening(rotated.z);
        if (darkening <= 0) continue;
        const p = projectLatLon(cell.lat, cell.lon, rotation, radius);
        const intensity = normalizeHeatIntensity(cell.count, maxCount);
        const blobRadius = radius * (0.05 + intensity * 0.08);
        const color = heatScaleColor(intensity, colors.accent, colors.accent, colors.danger);
        const alpha = (0.15 + intensity * 0.55) * darkening;
        const blobGradient = ctx.createRadialGradient(cx + p.x, cy + p.y, 0, cx + p.x, cy + p.y, blobRadius);
        blobGradient.addColorStop(0, rgbaCss(color, alpha));
        blobGradient.addColorStop(1, rgbaCss(color, 0));
        ctx.fillStyle = blobGradient;
        ctx.beginPath();
        ctx.arc(cx + p.x, cy + p.y, blobRadius, 0, Math.PI * 2);
        ctx.fill();
      }
      ctx.globalCompositeOperation = "source-over";

      ctx.restore();
    }

    function tick(time: number) {
      if (!draggingRef.current && !reducedMotion) {
        const dt = lastFrameTime === null ? 0 : (time - lastFrameTime) / 1000;
        rotationRef.current = { ...rotationRef.current, yaw: rotationRef.current.yaw + AUTO_ROTATE_DEG_PER_SEC * dt };
      }
      lastFrameTime = time;
      draw();
      raf = requestAnimationFrame(tick);
    }
    raf = requestAnimationFrame(tick);

    return () => {
      cancelAnimationFrame(raf);
      resizeObserver.disconnect();
    };
  }, [heatCells, reducedMotion]);

  function handlePointerDown(event: React.PointerEvent<HTMLCanvasElement>) {
    (event.target as HTMLCanvasElement).setPointerCapture(event.pointerId);
    draggingRef.current = { pointerId: event.pointerId, lastX: event.clientX, lastY: event.clientY };
  }

  function handlePointerMove(event: React.PointerEvent<HTMLCanvasElement>) {
    const drag = draggingRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    const dx = event.clientX - drag.lastX;
    const dy = event.clientY - drag.lastY;
    drag.lastX = event.clientX;
    drag.lastY = event.clientY;
    const next = rotationRef.current;
    rotationRef.current = {
      yaw: next.yaw + dx * DRAG_SENSITIVITY,
      pitch: Math.min(80, Math.max(-80, next.pitch - dy * DRAG_SENSITIVITY)),
    };
  }

  function handlePointerUp(event: React.PointerEvent<HTMLCanvasElement>) {
    if (draggingRef.current?.pointerId === event.pointerId) draggingRef.current = null;
  }

  const geotaggedCount = photos.filter((p) => p.gps_lat !== null && p.gps_lon !== null).length;

  return (
    <div ref={containerRef} className="relative flex-1 overflow-hidden bg-bg-base">
      <canvas
        ref={canvasRef}
        className="h-full w-full cursor-grab touch-none active:cursor-grabbing"
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onPointerLeave={handlePointerUp}
        role="img"
        aria-label={`Foto-Globus mit ${geotaggedCount} geotaggten Fotos — ziehen zum Drehen`}
      />
      <div className="pointer-events-none absolute inset-x-0 top-3 flex justify-center">
        <span className="rounded border border-border bg-bg-raised/80 px-3 py-1 text-xs text-text-secondary backdrop-blur-sm">
          {geotaggedCount} Foto{geotaggedCount === 1 ? "" : "s"} mit GPS · ziehen zum Drehen
        </span>
      </div>
      <div className="absolute bottom-3 right-3">
        <button
          type="button"
          onClick={onEnterMap}
          className="rounded border border-accent bg-bg-raised px-3 py-1.5 text-xs font-medium text-accent shadow hover:bg-accent/10"
        >
          Zur Karte →
        </button>
      </div>
    </div>
  );
}
