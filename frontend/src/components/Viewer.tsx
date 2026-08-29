import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useElementSize } from "../hooks/useElementSize";
import { useImageBitmap } from "../hooks/useImageBitmap";
import { formatShutter } from "../lib/format";
import { imageUrl, previewUrl } from "../lib/media";
import { clampZoom, computeBaseScale, imageOrigin, nextZoomStep, panForZoomAtCursor } from "../lib/viewerMath";
import { useAppStore } from "../store";

// Zielkante für die hochauflösende Anzeige: an der Container-Größe
// orientiert (siehe PHASE1_PROMPT.md Abschnitt 6, das Beispiel
// `max_edge=2560` dort) statt immer die volle Sensorauflösung zu
// dekodieren — Kachel-Rendering für beliebigen Zoom ist erst Phase 2.
const MIN_FULL_EDGE = 1024;
const MAX_FULL_EDGE = 4096;

export function Viewer() {
  const selectedFolderId = useAppStore((s) => s.selectedFolderId);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const photos = useAppStore((s) => (selectedFolderId ? s.photosByFolder[selectedFolderId] : undefined));
  const photo = photos?.find((p) => p.id === selectedPhotoId);

  const zoom = useAppStore((s) => s.zoom);
  const fitMode = useAppStore((s) => s.fitMode);
  const panX = useAppStore((s) => s.panX);
  const panY = useAppStore((s) => s.panY);
  const setZoom = useAppStore((s) => s.setZoom);
  const setPan = useAppStore((s) => s.setPan);
  const resetView = useAppStore((s) => s.resetView);

  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerSize = useElementSize(containerRef);

  const dpr = window.devicePixelRatio || 1;
  const targetFullEdge = Math.round(Math.min(MAX_FULL_EDGE, Math.max(MIN_FULL_EDGE, Math.max(containerSize.width, containerSize.height) * dpr)));

  const thumbUrl = photo ? previewUrl(photo.id, 0) : null;
  const fullUrl = photo && containerSize.width > 0 ? imageUrl(photo.id, targetFullEdge) : null;
  const thumbBitmap = useImageBitmap(thumbUrl);
  const fullBitmap = useImageBitmap(fullUrl);

  // Sobald die hochauflösende Version da ist, wird sie gezeigt; bis
  // dahin die Vorschau hochskaliert — kein Weißblitz beim Bildwechsel
  // (siehe PHASE1_PROMPT.md Abschnitt 7).
  const activeBitmap = fullBitmap ?? thumbBitmap;

  // Bildmaße für die Geometrie kommen aus den Katalog-Metadaten (stabil
  // über Vorschau/Vollbild hinweg) — nur als Fallback die Maße des
  // gerade verfügbaren Bitmaps, falls die Metadaten fehlen.
  const imgW = photo?.width ?? activeBitmap?.width ?? 0;
  const imgH = photo?.height ?? activeBitmap?.height ?? 0;

  const fitScale = useMemo(() => computeBaseScale("fit", containerSize.width, containerSize.height, imgW, imgH), [containerSize.width, containerSize.height, imgW, imgH]);

  const effectiveScale = fitMode === "manual" ? zoom : computeBaseScale(fitMode, containerSize.width, containerSize.height, imgW, imgH);

  // ---- Zeichnen ------------------------------------------------------

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const cssWidth = containerSize.width;
    const cssHeight = containerSize.height;
    canvas.width = Math.max(1, Math.round(cssWidth * dpr));
    canvas.height = Math.max(1, Math.round(cssHeight * dpr));
    canvas.style.width = `${cssWidth}px`;
    canvas.style.height = `${cssHeight}px`;

    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, cssWidth, cssHeight);

    if (!activeBitmap || imgW <= 0 || imgH <= 0) return;

    const origin = imageOrigin(cssWidth, cssHeight, imgW, imgH, effectiveScale, { x: panX, y: panY });
    // Über 100 % Zoom scharfe Pixelkanten statt weichgezeichneter
    // Vergrößerung (PHASE1_PROMPT.md Abschnitt 7).
    ctx.imageSmoothingEnabled = effectiveScale <= 1;
    ctx.drawImage(activeBitmap, origin.x, origin.y, imgW * effectiveScale, imgH * effectiveScale);
  }, [activeBitmap, containerSize.width, containerSize.height, dpr, effectiveScale, imgW, imgH, panX, panY]);

  // ---- Maus: Zoom zum Cursor, Pan per Ziehen ---------------------------

  const dragState = useRef<{ startX: number; startY: number; startPanX: number; startPanY: number } | null>(null);
  const [spaceHeld, setSpaceHeld] = useState(false);

  const handleWheel = useCallback(
    (event: React.WheelEvent<HTMLDivElement>) => {
      if (!photo || imgW <= 0) return;
      event.preventDefault();

      const rect = event.currentTarget.getBoundingClientRect();
      const cursor = { x: event.clientX - rect.left, y: event.clientY - rect.top };

      const factor = Math.pow(1.0015, -event.deltaY);
      const newZoom = clampZoom(effectiveScale * factor);
      const newPan = panForZoomAtCursor(cursor, containerSize.width, containerSize.height, imgW, imgH, effectiveScale, newZoom, { x: panX, y: panY });

      setZoom(newZoom, "manual");
      setPan(newPan.x, newPan.y);
    },
    [photo, imgW, imgH, effectiveScale, containerSize.width, containerSize.height, panX, panY, setZoom, setPan],
  );

  const canPan = spaceHeld || effectiveScale > fitScale + 1e-6;

  const handleMouseDown = useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      if (event.button !== 0 || !canPan) return;
      dragState.current = { startX: event.clientX, startY: event.clientY, startPanX: panX, startPanY: panY };
    },
    [canPan, panX, panY],
  );

  const handleMouseMove = useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      const drag = dragState.current;
      if (!drag) return;
      setPan(drag.startPanX + (event.clientX - drag.startX), drag.startPanY + (event.clientY - drag.startY));
    },
    [setPan],
  );

  const endDrag = useCallback(() => {
    dragState.current = null;
  }, []);

  const handleDoubleClick = useCallback(() => {
    if (fitMode !== "manual") {
      setZoom(1, "manual");
      setPan(0, 0);
    } else {
      resetView();
    }
  }, [fitMode, setZoom, setPan, resetView]);

  // ---- Tastatur: +/- Zoom, 0 Einpassen, 1 1:1, Leertaste zum Ziehen ------

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      const target = event.target as HTMLElement | null;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) return;

      if (event.code === "Space") {
        setSpaceHeld(true);
      } else if (event.key === "+" || event.key === "=") {
        setZoom(nextZoomStep(effectiveScale, 1, fitScale), "manual");
      } else if (event.key === "-") {
        setZoom(nextZoomStep(effectiveScale, -1, fitScale), "manual");
      } else if (event.key === "0") {
        resetView();
      } else if (event.key === "1") {
        setZoom(1, "manual");
        setPan(0, 0);
      }
    }
    function handleKeyUp(event: KeyboardEvent) {
      if (event.code === "Space") setSpaceHeld(false);
    }

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
    };
  }, [effectiveScale, fitScale, setZoom, setPan, resetView]);

  return (
    <main
      ref={containerRef}
      className="relative flex flex-1 items-center justify-center overflow-hidden bg-bg-base"
      onWheel={handleWheel}
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={endDrag}
      onMouseLeave={endDrag}
      onDoubleClick={handleDoubleClick}
      style={{ cursor: canPan ? (dragState.current ? "grabbing" : "grab") : "default" }}
    >
      {!photo && <p className="pointer-events-none text-sm text-text-muted">Kein Foto ausgewählt.</p>}

      <canvas ref={canvasRef} className="pointer-events-none absolute inset-0" />

      {photo && (
        <div className="pointer-events-none absolute right-3 bottom-3 rounded bg-bg-raised/90 px-3 py-2 text-xs text-text-secondary backdrop-blur">
          <div className="font-medium text-text-primary">{photo.filename}</div>
          <div>
            {[photo.camera_make, photo.camera_model].filter(Boolean).join(" ")}
            {photo.lens ? ` · ${photo.lens}` : ""}
          </div>
          <div>
            {[photo.iso ? `ISO ${photo.iso}` : null, photo.aperture ? `f/${photo.aperture}` : null, photo.shutter ? formatShutter(photo.shutter) : null, photo.focal_length ? `${Math.round(photo.focal_length)}mm` : null]
              .filter(Boolean)
              .join(" · ")}
          </div>
          <div>
            {photo.captured_at ? new Date(photo.captured_at).toLocaleString() : ""}
            {photo.width && photo.height ? ` · ${photo.width} × ${photo.height}` : ""}
            {` · ${Math.round(effectiveScale * 100)} %`}
          </div>
        </div>
      )}
    </main>
  );
}
