import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useDevelopRender } from "../hooks/useDevelopRender";
import { useElementSize } from "../hooks/useElementSize";
import { useImageBitmap } from "../hooks/useImageBitmap";
import { buildEdlEnvelopeJson } from "../lib/edl";
import { formatShutter } from "../lib/format";
import { imageUrl, previewUrl } from "../lib/media";
import { mergeEdlSubset } from "../lib/presets";
import { clampZoom, computeBaseScale, imageOrigin, nextZoomStep, panForZoomAtCursor } from "../lib/viewerMath";
import { QuadRenderer } from "../lib/webgl";
import { useAppStore } from "../store";
import { BeforeAfterView } from "./BeforeAfterView";
import { CropOverlay } from "./CropOverlay";
import { MaskOverlay } from "./MaskOverlay";
import { RepairOverlay } from "./RepairOverlay";

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
  const developPanelOpen = useAppStore((s) => s.developPanelOpen);
  const developEdl = useAppStore((s) => s.developEdl);
  const beforeAfterMode = useAppStore((s) => s.beforeAfterMode);
  const hoverPresetSubset = useAppStore((s) => s.hoverPresetSubset);
  const wbPickerActive = useAppStore((s) => s.wbPickerActive);
  const pickWhiteBalanceAt = useAppStore((s) => s.pickWhiteBalanceAt);
  const colorMixerPickerActive = useAppStore((s) => s.colorMixerPickerActive);
  const addColorMixerRegionAt = useAppStore((s) => s.addColorMixerRegionAt);
  const maskColorRangePickerActive = useAppStore((s) => s.maskColorRangePickerActive);
  const setMaskColorRangeTargetAt = useAppStore((s) => s.setMaskColorRangeTargetAt);
  const maskColorMixerPickerActive = useAppStore((s) => s.maskColorMixerPickerActive);
  const addMaskColorMixerRegionAt = useAppStore((s) => s.addMaskColorMixerRegionAt);
  const pickerActive = wbPickerActive || colorMixerPickerActive || maskColorRangePickerActive || maskColorMixerPickerActive;
  const geometryCropActive = useAppStore((s) => s.geometryCropActive);
  const setGeometryCrop = useAppStore((s) => s.setGeometryCrop);
  const repairActive = useAppStore((s) => s.repairActive);
  const repairStrokes = useAppStore((s) => s.developEdl.repair);
  const repairPendingSource = useAppStore((s) => s.repairPendingSource);
  const setRepairSourcePoint = useAppStore((s) => s.setRepairSourcePoint);
  const addRepairStroke = useAppStore((s) => s.addRepairStroke);
  const selectedMaskId = useAppStore((s) => s.selectedMaskId);
  const selectedMask = useAppStore((s) => s.developEdl.masks.find((m) => m.id === selectedMaskId) ?? null);
  const selectedMaskComponentIndex = useAppStore((s) => s.selectedMaskComponentIndex);
  const updateMaskGeometry = useAppStore((s) => s.updateMaskGeometry);
  const commitMaskDrag = useAppStore((s) => s.commitMaskDrag);
  const addMaskBrushStroke = useAppStore((s) => s.addMaskBrushStroke);
  const removeMaskBrushStroke = useAppStore((s) => s.removeMaskBrushStroke);
  const removeRepairStroke = useAppStore((s) => s.removeRepairStroke);
  const commitDevelopEdit = useAppStore((s) => s.commitDevelopEdit);

  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const rendererRef = useRef<QuadRenderer | null>(null);
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

  // Läuft das Entwickeln-Panel, wird zusätzlich live über die neue
  // `develop/...`-Route gerendert (siehe `hooks/useDevelopRender`,
  // `PLAN.md` Phase 2 Schritt 6) — bis die erste Antwort da ist, bleibt
  // `activeBitmap` als Platzhalter sichtbar (kein Weißblitz beim Öffnen
  // des Panels, analog zum Vorschau/Vollbild-Übergang oben).
  // Hover-Vorschau eines Presets (Phase 5 Schritt 6, `SPEC.md` §3.5)
  // überschreibt rein visuell, welche EDL gerendert wird — `developEdl`
  // selbst bleibt unverändert, solange nicht tatsächlich geklickt wird.
  const renderedEdl = hoverPresetSubset ? mergeEdlSubset(developEdl, hoverPresetSubset) : developEdl;
  const developEdlJson = developPanelOpen && photo ? buildEdlEnvelopeJson(renderedEdl) : null;
  const developPhotoId = developPanelOpen ? (photo?.id ?? null) : null;
  const developFrame = useDevelopRender(developPhotoId, developEdlJson, photo && containerSize.width > 0 ? targetFullEdge : undefined);

  const drawSource = developFrame ?? activeBitmap ?? null;

  // Bildmaße für die Geometrie kommen aus den Katalog-Metadaten (stabil
  // über Vorschau/Vollbild/Entwickeln hinweg) — nur als Fallback die Maße
  // der gerade verfügbaren Pixelquelle, falls die Metadaten fehlen.
  const imgW = photo?.width ?? drawSource?.width ?? 0;
  const imgH = photo?.height ?? drawSource?.height ?? 0;

  const fitScale = useMemo(() => computeBaseScale("fit", containerSize.width, containerSize.height, imgW, imgH), [containerSize.width, containerSize.height, imgW, imgH]);

  const effectiveScale = fitMode === "manual" ? zoom : computeBaseScale(fitMode, containerSize.width, containerSize.height, imgW, imgH);

  // ---- Zeichnen (WebGL2, siehe lib/webgl.ts) --------------------------
  //
  // Ein `QuadRenderer` pro `<canvas>`-Element, über dessen gesamte
  // Lebenszeit wiederverwendet — WebGL2 kann sowohl ein dekodiertes
  // `ImageBitmap` (Vorschau/Vollbild) als auch einen rohen RGBA8-Puffer
  // (Entwickeln-Route) als Textur hochladen, Canvas-2D könnte Letzteres
  // nicht ohne Umweg über `ImageData` (PLAN.md Phase 2 Schritt 6).
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    try {
      rendererRef.current = new QuadRenderer(canvas);
    } catch (err) {
      console.error("WebGL2-Renderer konnte nicht erstellt werden:", err);
      rendererRef.current = null;
    }
    return () => {
      rendererRef.current?.dispose();
      rendererRef.current = null;
    };
  }, []);

  useEffect(() => {
    const renderer = rendererRef.current;
    if (!renderer) return;

    const cssWidth = containerSize.width;
    const cssHeight = containerSize.height;

    if (!drawSource || imgW <= 0 || imgH <= 0) {
      renderer.draw(cssWidth, cssHeight, dpr, { x: 0, y: 0 }, 0, 0);
      return;
    }

    if (developFrame && drawSource === developFrame) {
      renderer.uploadRgba8(developFrame.width, developFrame.height, developFrame.pixels);
    } else if (activeBitmap && drawSource === activeBitmap) {
      renderer.uploadImageBitmap(activeBitmap);
    }

    const origin = imageOrigin(cssWidth, cssHeight, imgW, imgH, effectiveScale, { x: panX, y: panY });
    // Über 100 % Zoom scharfe Pixelkanten statt weichgezeichneter
    // Vergrößerung (PHASE1_PROMPT.md Abschnitt 7).
    renderer.setSmoothing(effectiveScale <= 1);
    renderer.draw(cssWidth, cssHeight, dpr, origin, imgW * effectiveScale, imgH * effectiveScale);
  }, [drawSource, developFrame, activeBitmap, containerSize.width, containerSize.height, dpr, effectiveScale, imgW, imgH, panX, panY]);

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

  // Solange das Reparatur-Werkzeug aktiv ist, deckt `RepairOverlay` die
  // gesamte Bildfläche mit einem eigenen `pointer-events-auto`-Div ab
  // (anders als `CropOverlay`, dessen anfassbare Fläche auf das kleine
  // Freistellungsrechteck beschränkt ist) — Ziehen soll dort malen, nicht
  // schwenken. Dieselbe Fläche deckt `MaskOverlay` für eine ausgewählte
  // Pinselmaske ab (Phase 6 Schritt 4).
  const selectedMaskIsBrush = selectedMask?.components[selectedMaskComponentIndex]?.geometry.kind === "Brush";
  const canPan = !repairActive && !selectedMaskIsBrush && (spaceHeld || effectiveScale > fitScale + 1e-6);

  const handleMouseDown = useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      if (pickerActive || event.button !== 0 || !canPan) return;
      dragState.current = { startX: event.clientX, startY: event.clientY, startPanX: panX, startPanY: panY };
    },
    [pickerActive, canPan, panX, panY],
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

  // ---- Bild-Klick-Werkzeuge: Weißabgleich-Pipette (Phase 4 Schritt 3)
  // und Farbmischer-Region-Aufnahme (Phase 4 Schritt 5) teilen sich das
  // Sampling — nur der Aufrufer (`pickWhiteBalanceAt` vs.
  // `addColorMixerRegionAt`) unterscheidet sich.

  const handleImageClick = useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      if (!pickerActive || !developFrame || imgW <= 0 || imgH <= 0) return;

      const rect = event.currentTarget.getBoundingClientRect();
      const cursor = { x: event.clientX - rect.left, y: event.clientY - rect.top };
      const origin = imageOrigin(containerSize.width, containerSize.height, imgW, imgH, effectiveScale, { x: panX, y: panY });
      const imageX = (cursor.x - origin.x) / effectiveScale;
      const imageY = (cursor.y - origin.y) / effectiveScale;
      if (imageX < 0 || imageY < 0 || imageX >= imgW || imageY >= imgH) return;

      // `developFrame` kann in einer anderen Auflösung vorliegen als
      // `imgW`/`imgH` (Katalog-Metadaten) — Bruchteil statt Pixelwert
      // übertragen, um beide Auflösungen konsistent aufeinander
      // abzubilden.
      const sampleX = Math.min(developFrame.width - 1, Math.floor((imageX / imgW) * developFrame.width));
      const sampleY = Math.min(developFrame.height - 1, Math.floor((imageY / imgH) * developFrame.height));
      const index = (sampleY * developFrame.width + sampleX) * 4;
      const r = developFrame.pixels[index] ?? 0;
      const g = developFrame.pixels[index + 1] ?? 0;
      const b = developFrame.pixels[index + 2] ?? 0;

      if (wbPickerActive) {
        pickWhiteBalanceAt(r, g, b);
      } else if (colorMixerPickerActive) {
        addColorMixerRegionAt(r, g, b);
      } else if (maskColorRangePickerActive && selectedMask) {
        setMaskColorRangeTargetAt(selectedMask.id, r, g, b);
      } else if (maskColorMixerPickerActive && selectedMask) {
        addMaskColorMixerRegionAt(selectedMask.id, r, g, b);
      }
    },
    [
      pickerActive,
      wbPickerActive,
      colorMixerPickerActive,
      maskColorRangePickerActive,
      maskColorMixerPickerActive,
      selectedMask,
      developFrame,
      imgW,
      imgH,
      containerSize.width,
      containerSize.height,
      effectiveScale,
      panX,
      panY,
      pickWhiteBalanceAt,
      addColorMixerRegionAt,
      setMaskColorRangeTargetAt,
      addMaskColorMixerRegionAt,
    ],
  );

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
      onClick={handleImageClick}
      onDoubleClick={handleDoubleClick}
      style={{ cursor: pickerActive ? "crosshair" : canPan ? (dragState.current ? "grabbing" : "grab") : "default" }}
    >
      {!photo && <p className="pointer-events-none text-sm text-text-muted">Kein Foto ausgewählt.</p>}

      <canvas ref={canvasRef} className="pointer-events-none absolute inset-0" />

      {photo && beforeAfterMode !== "none" && (
        <BeforeAfterView photoId={developPhotoId} afterEdlJson={developEdlJson} maxEdge={containerSize.width > 0 ? targetFullEdge : undefined} />
      )}

      {photo && geometryCropActive && imgW > 0 && imgH > 0 && (
        <CropOverlay
          imageLeft={imageOrigin(containerSize.width, containerSize.height, imgW, imgH, effectiveScale, { x: panX, y: panY }).x}
          imageTop={imageOrigin(containerSize.width, containerSize.height, imgW, imgH, effectiveScale, { x: panX, y: panY }).y}
          imageWidth={imgW * effectiveScale}
          imageHeight={imgH * effectiveScale}
          crop={developEdl.geometry.crop}
          overlay={developEdl.geometry.overlay}
          aspectRatio={developEdl.geometry.aspect_ratio}
          onChange={setGeometryCrop}
          onCommit={() => void commitDevelopEdit()}
        />
      )}

      {photo && repairActive && imgW > 0 && imgH > 0 && (
        <RepairOverlay
          imageLeft={imageOrigin(containerSize.width, containerSize.height, imgW, imgH, effectiveScale, { x: panX, y: panY }).x}
          imageTop={imageOrigin(containerSize.width, containerSize.height, imgW, imgH, effectiveScale, { x: panX, y: panY }).y}
          imageWidth={imgW * effectiveScale}
          imageHeight={imgH * effectiveScale}
          strokes={repairStrokes}
          pendingSource={repairPendingSource}
          onSetSource={setRepairSourcePoint}
          onPaint={addRepairStroke}
          onRemoveStroke={removeRepairStroke}
        />
      )}

      {photo &&
        selectedMask &&
        (selectedMask.components[selectedMaskComponentIndex]?.geometry.kind === "LinearGradient" ||
          selectedMask.components[selectedMaskComponentIndex]?.geometry.kind === "RadialGradient" ||
          selectedMask.components[selectedMaskComponentIndex]?.geometry.kind === "Brush") &&
        imgW > 0 &&
        imgH > 0 && (
          <MaskOverlay
            imageLeft={imageOrigin(containerSize.width, containerSize.height, imgW, imgH, effectiveScale, { x: panX, y: panY }).x}
            imageTop={imageOrigin(containerSize.width, containerSize.height, imgW, imgH, effectiveScale, { x: panX, y: panY }).y}
            imageWidth={imgW * effectiveScale}
            imageHeight={imgH * effectiveScale}
            geometry={selectedMask.components[selectedMaskComponentIndex].geometry}
            onChange={(geometry) => updateMaskGeometry(selectedMask.id, geometry)}
            onCommit={commitMaskDrag}
            onPaintBrushStroke={(points) => addMaskBrushStroke(selectedMask.id, points)}
            onRemoveBrushStroke={(index) => removeMaskBrushStroke(selectedMask.id, index)}
          />
        )}

      {photo && (
        <div className="pointer-events-none absolute right-3 bottom-3 rounded bg-bg-raised/90 px-3 py-2 text-xs text-text-secondary backdrop-blur">
          <div className="font-medium text-text-primary">
            {photo.filename}
            {photo.missing && <span className="ml-2 text-danger">Datei fehlt</span>}
          </div>
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
