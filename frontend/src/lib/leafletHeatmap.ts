import L from "leaflet";

import { binPointsIntoGrid, heatScaleColor, normalizeHeatIntensity, rgbaCss, type GeoPoint } from "./photoHeatmap";

/** Selbst implementierter Leaflet-Heatmap-Layer (siehe `DECISIONS.md`
 * ADR-0044) statt einer neuen Laufzeit-Abhängigkeit (z. B.
 * `leaflet.heat`) — dieselbe "Canvas-Overlay im `overlayPane`, auf
 * `moveend`/`zoomend` neu zeichnen"-Technik, die praktisch jeder
 * Leaflet-Canvas-Layer verwendet, hier mit der bereits vorhandenen
 * `lib/photoHeatmap.ts`-Rasterung/Farbskala statt einer echten
 * Kernel-Density-Schätzung — für eine Foto-Dichte-Übersicht reicht das
 * (dieselbe Begründung wie in `photoHeatmap.ts`s Moduldoku), und teilt
 * sich damit exakt dieselbe Logik/Farbskala mit dem Globus
 * (`GlobeView.tsx`). */

interface HeatLayerColors {
  cool: string;
  mid: string;
  hot: string;
}

export interface PhotoHeatLayer extends L.Layer {
  setPoints(points: GeoPoint[]): void;
}

const CELL_SIZE_DEG = 1.5;

export function createPhotoHeatLayer(points: GeoPoint[], getColors: () => HeatLayerColors): PhotoHeatLayer {
  let currentPoints = points;
  let canvas: HTMLCanvasElement | null = null;
  let map: L.Map | null = null;

  function redraw() {
    if (!map || !canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const dpr = Math.max(1, window.devicePixelRatio || 1);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    const cells = binPointsIntoGrid(currentPoints, CELL_SIZE_DEG);
    const maxCount = cells.reduce((max, c) => Math.max(max, c.count), 0);
    const colors = getColors();

    ctx.globalCompositeOperation = "lighter";
    for (const cell of cells) {
      const point = map.latLngToContainerPoint([cell.lat, cell.lon]);
      const intensity = normalizeHeatIntensity(cell.count, maxCount);
      const blobRadius = 20 + intensity * 45;
      const color = heatScaleColor(intensity, colors.cool, colors.mid, colors.hot);
      const alpha = 0.16 + intensity * 0.55;
      const gradient = ctx.createRadialGradient(point.x, point.y, 0, point.x, point.y, blobRadius);
      gradient.addColorStop(0, rgbaCss(color, alpha));
      gradient.addColorStop(1, rgbaCss(color, 0));
      ctx.fillStyle = gradient;
      ctx.beginPath();
      ctx.arc(point.x, point.y, blobRadius, 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.globalCompositeOperation = "source-over";
  }

  function reset() {
    if (!map || !canvas) return;
    const size = map.getSize();
    const topLeft = map.containerPointToLayerPoint([0, 0]);
    L.DomUtil.setPosition(canvas, topLeft);
    const dpr = Math.max(1, window.devicePixelRatio || 1);
    canvas.width = Math.round(size.x * dpr);
    canvas.height = Math.round(size.y * dpr);
    canvas.style.width = `${size.x}px`;
    canvas.style.height = `${size.y}px`;
    redraw();
  }

  const layer = new L.Layer() as PhotoHeatLayer;

  layer.onAdd = function onAdd(addedMap: L.Map) {
    map = addedMap;
    canvas = L.DomUtil.create("canvas", "apx-photo-heat-layer") as HTMLCanvasElement;
    canvas.style.position = "absolute";
    canvas.style.pointerEvents = "none";
    map.getPanes().overlayPane.appendChild(canvas);
    map.on("moveend zoomend resize", reset);
    reset();
    return this;
  };

  layer.onRemove = function onRemove(removedMap: L.Map) {
    if (canvas) L.DomUtil.remove(canvas);
    removedMap.off("moveend zoomend resize", reset);
    canvas = null;
    map = null;
    return this;
  };

  layer.setPoints = (nextPoints: GeoPoint[]) => {
    currentPoints = nextPoints;
    redraw();
  };

  return layer;
}
