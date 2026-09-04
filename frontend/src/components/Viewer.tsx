import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useDevelopPreviewThumbnail, useDevelopRender } from "../hooks/useDevelopRender";
import { useElementSize } from "../hooks/useElementSize";
import { useImageBitmap } from "../hooks/useImageBitmap";
import { computeAutoTone } from "../lib/autoTone";
import { hueDegreesFromRgbByte } from "../lib/colorSampling";
import { buildEdlEnvelopeJson, CURVE_CHANNEL_TABS, nearestHslBand, visibleMasks, type CurvesAdjustment, type HslAdjustment } from "../lib/edl";
import { formatShutter } from "../lib/format";
import { buildClippingOverlay } from "../lib/histogram";
import { computeMaskPinPosition } from "../lib/maskPins";
import { matchesBinding } from "../lib/keybindings";
import { imageUrl, previewUrl } from "../lib/media";
import { mergeEdlSubset } from "../lib/presets";
import { applyFrequencyView } from "../lib/frequencySeparation";
import { applyPaperWhite, type SoftProofSettings } from "../lib/softProof";
import { clampZoom, computeBaseScale, imageOrigin, nextZoomStep, panForZoomAtCursor } from "../lib/viewerMath";
import { QuadRenderer } from "../lib/webgl";
import { useAppStore } from "../store";
import { BeforeAfterView } from "./BeforeAfterView";
import { ContentAwareMoveOverlay } from "./ContentAwareMoveOverlay";
import { CropOverlay } from "./CropOverlay";
import { DevelopAnalysisPanel } from "./DevelopAnalysisPanel";
import { MaskColorOverlay } from "./MaskColorOverlay";
import { MaskOverlay } from "./MaskOverlay";
import { ReferenceView } from "./ReferenceView";
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
  const infoOverlayVisible = useAppStore((s) => s.infoOverlayVisible);
  const toggleInfoOverlay = useAppStore((s) => s.toggleInfoOverlay);
  const maskOverlayVisible = useAppStore((s) => s.maskOverlayVisible);
  const toggleMaskOverlay = useAppStore((s) => s.toggleMaskOverlay);
  const maskGroups = useAppStore((s) => s.developEdl.mask_groups);
  const applyAutoTone = useAppStore((s) => s.applyAutoTone);
  const developPanelOpen = useAppStore((s) => s.developPanelOpen);
  const developEdl = useAppStore((s) => s.developEdl);
  const beforeAfterMode = useAppStore((s) => s.beforeAfterMode);
  const referenceViewActive = useAppStore((s) => s.referenceViewActive);
  const softProofActive = useAppStore((s) => s.softProofActive);
  const softProofProfile = useAppStore((s) => s.softProofProfile);
  const softProofCustomIccPath = useAppStore((s) => s.softProofCustomIccPath);
  const softProofIntent = useAppStore((s) => s.softProofIntent);
  const softProofGamutWarning = useAppStore((s) => s.softProofGamutWarning);
  const softProofPaperWhite = useAppStore((s) => s.softProofPaperWhite);
  const frequencyViewMode = useAppStore((s) => s.frequencyViewMode);
  const hoverPresetSubset = useAppStore((s) => s.hoverPresetSubset);
  const wbPickerActive = useAppStore((s) => s.wbPickerActive);
  const pickWhiteBalanceAt = useAppStore((s) => s.pickWhiteBalanceAt);
  const colorMixerPickerActive = useAppStore((s) => s.colorMixerPickerActive);
  const addColorMixerRegionAt = useAppStore((s) => s.addColorMixerRegionAt);
  const maskColorRangePickerActive = useAppStore((s) => s.maskColorRangePickerActive);
  const setMaskColorRangeTargetAt = useAppStore((s) => s.setMaskColorRangeTargetAt);
  const maskColorMixerPickerActive = useAppStore((s) => s.maskColorMixerPickerActive);
  const addMaskColorMixerRegionAt = useAppStore((s) => s.addMaskColorMixerRegionAt);
  const aiMaskClickPickerActive = useAppStore((s) => s.aiMaskClickPickerActive);
  const addAiMask = useAppStore((s) => s.addAiMask);
  const virtualApertureFocusPickerActive = useAppStore((s) => s.virtualApertureFocusPickerActive);
  const setVirtualApertureFocusPoint = useAppStore((s) => s.setVirtualApertureFocusPoint);
  const tatMode = useAppStore((s) => s.tatMode);
  const tatCurveChannel = useAppStore((s) => s.tatCurveChannel);
  const setTatMode = useAppStore((s) => s.setTatMode);
  const setTatCurveChannel = useAppStore((s) => s.setTatCurveChannel);
  const setCurveChannel = useAppStore((s) => s.setCurveChannel);
  const setHslBandField = useAppStore((s) => s.setHslBandField);
  const pickerActive =
    wbPickerActive ||
    colorMixerPickerActive ||
    maskColorRangePickerActive ||
    maskColorMixerPickerActive ||
    aiMaskClickPickerActive ||
    virtualApertureFocusPickerActive;
  const geometryCropActive = useAppStore((s) => s.geometryCropActive);
  const setGeometryCrop = useAppStore((s) => s.setGeometryCrop);
  const contentAwareMoveActive = useAppStore((s) => s.contentAwareMoveActive);
  const contentAwareMoveRect = useAppStore((s) => s.contentAwareMoveRect);
  const contentAwareMoveLoading = useAppStore((s) => s.contentAwareMoveLoading);
  const setContentAwareMoveRect = useAppStore((s) => s.setContentAwareMoveRect);
  const commitContentAwareMove = useAppStore((s) => s.commitContentAwareMove);
  const repairActive = useAppStore((s) => s.repairActive);
  const repairStrokes = useAppStore((s) => s.developEdl.repair);
  const repairPendingSource = useAppStore((s) => s.repairPendingSource);
  const repairDraftMode = useAppStore((s) => s.repairDraftMode);
  const autoSourceModeActive = useAppStore((s) => s.autoSourceModeActive);
  const suggestRepairSourceForTarget = useAppStore((s) => s.suggestRepairSourceForTarget);
  const sensorSpotCandidates = useAppStore((s) => s.sensorSpotCandidates);
  const setRepairSourcePoint = useAppStore((s) => s.setRepairSourcePoint);
  const addRepairStroke = useAppStore((s) => s.addRepairStroke);
  const selectedMaskId = useAppStore((s) => s.selectedMaskId);
  const selectedMask = useAppStore((s) => s.developEdl.masks.find((m) => m.id === selectedMaskId) ?? null);
  const selectMask = useAppStore((s) => s.selectMask);
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
  const developMaxEdge = photo && containerSize.width > 0 ? targetFullEdge : undefined;
  const developFrame = useDevelopRender(developPhotoId, developEdlJson, developMaxEdge);

  // Echter Soft-Proof (Phase 12 Schritt 6, siehe `DECISIONS.md`
  // ADR-0039-Nachtrag II): eine **separate** zweite Anfrage über dieselbe
  // Route, statt `developFrame` selbst zu ersetzen — Farbaufnehmer (TAT,
  // Weißabgleich-Pipette, Maskenfarbbereich), das Clipping-Overlay und
  // der Histogramm-Aufbau lesen `developFrame.pixels` direkt und müssen
  // dabei immer den echten, nicht soft-proof-verfälschten Entwickeln-
  // Zustand sehen — nur der tatsächlich gezeichnete Canvas-Inhalt soll
  // die Proof-Simulation zeigen.
  const softProofSettings: SoftProofSettings | null = softProofActive
    ? {
        profile: softProofProfile,
        customIccPath: softProofCustomIccPath,
        intent: softProofIntent,
        gamutWarning: softProofGamutWarning,
        paperWhite: softProofPaperWhite,
      }
    : null;
  const softProofFrame = useDevelopPreviewThumbnail(softProofActive ? developPhotoId : null, developEdlJson, developMaxEdge, softProofSettings);

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
      // Soft-Proof (Phase 6 Schritt 10, seit Phase 12 Schritt 6 ein
      // echter serverseitiger ICC-Transform statt einer Näherung, siehe
      // `lib/softProof.ts`s Moduldoku) betrifft nur die live entwickelte
      // Vorschau, nie das rohe Vorschau-/Vollbild-Bitmap (das läuft ja
      // nicht über die `develop/...`-Route). Solange `softProofFrame`
      // noch nicht nachgeladen ist (asynchron, eigene Anfrage), bleibt
      // die unveränderte `developFrame`-Vorschau stehen, statt kurz
      // etwas Falsches oder Leeres zu zeigen.
      const proofed = softProofActive ? softProofFrame : null;
      const basePixels = proofed ? (softProofPaperWhite ? applyPaperWhite(proofed) : proofed.pixels) : developFrame.pixels;
      // Frequenztrennungs-Ansichtsmodus (Phase 14 Schritt 2, siehe
      // `DECISIONS.md` ADR-0041) — reine Anzeige-Transformation über den
      // bereits gerenderten Puffer, verändert `developEdl` nicht.
      const pixels = applyFrequencyView(basePixels, developFrame.width, developFrame.height, frequencyViewMode);
      renderer.uploadRgba8(developFrame.width, developFrame.height, pixels);
    } else if (activeBitmap && drawSource === activeBitmap) {
      renderer.uploadImageBitmap(activeBitmap);
    }

    const origin = imageOrigin(cssWidth, cssHeight, imgW, imgH, effectiveScale, { x: panX, y: panY });
    // Über 100 % Zoom scharfe Pixelkanten statt weichgezeichneter
    // Vergrößerung (PHASE1_PROMPT.md Abschnitt 7).
    renderer.setSmoothing(effectiveScale <= 1);
    renderer.draw(cssWidth, cssHeight, dpr, origin, imgW * effectiveScale, imgH * effectiveScale);
  }, [
    drawSource,
    developFrame,
    activeBitmap,
    containerSize.width,
    containerSize.height,
    dpr,
    effectiveScale,
    imgW,
    imgH,
    panX,
    panY,
    softProofActive,
    softProofFrame,
    softProofPaperWhite,
    frequencyViewMode,
  ]);

  // ---- Maus: Zoom zum Cursor, Pan per Ziehen ---------------------------

  const dragState = useRef<{ startX: number; startY: number; startPanX: number; startPanY: number } | null>(null);
  const [spaceHeld, setSpaceHeld] = useState(false);

  // ---- Zielgerichtetes Anpassungswerkzeug (TAT, Phase 11 Schritt 6,
  // siehe DECISIONS.md ADR-0038) — Klick+Zug im Bild steuert direkt am
  // Bildpunkt den in `tatMode`/`tatCurveChannel` gewählten Regler.
  // Eigener Ref statt `dragState` (der ist fürs Schwenken reserviert) —
  // `canPan` schließt TAT-Ziehen aus, beide können also nie gleichzeitig
  // aktiv sein.
  const tatDragState = useRef<
    | { kind: "curve"; channel: keyof CurvesAdjustment; pointIndex: number; startOutput: number; startClientY: number }
    | { kind: "hsl"; band: keyof HslAdjustment; startLuminance: number; startClientY: number }
    | null
  >(null);

  // ---- Entwickeln-Analysewerkzeuge (Phase 9 Schritt 4) ------------------
  const [pointerSample, setPointerSample] = useState<{ r: number; g: number; b: number } | null>(null);
  const [clippingOverlayEnabled, setClippingOverlayEnabled] = useState(false);
  const clipCanvasRef = useRef<HTMLCanvasElement>(null);

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
  const tatActive = tatMode !== "off";
  const canPan = !repairActive && !selectedMaskIsBrush && !tatActive && (spaceHeld || effectiveScale > fitScale + 1e-6);

  // TAT-Schwellwert für "neuen Kurvenpunkt statt vorhandenen verschieben"
  // (Eingabewert-Abstand, 0..1) — siehe Store-Moduldoku.
  const TAT_NEW_POINT_THRESHOLD = 0.04;

  const handleMouseDown = useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      if (tatActive && event.button === 0 && developFrame && imgW > 0 && imgH > 0) {
        const rect = event.currentTarget.getBoundingClientRect();
        const cursor = { x: event.clientX - rect.left, y: event.clientY - rect.top };
        const origin = imageOrigin(containerSize.width, containerSize.height, imgW, imgH, effectiveScale, { x: panX, y: panY });
        const imageX = (cursor.x - origin.x) / effectiveScale;
        const imageY = (cursor.y - origin.y) / effectiveScale;
        if (imageX < 0 || imageY < 0 || imageX >= imgW || imageY >= imgH) return;

        const sampleX = Math.min(developFrame.width - 1, Math.floor((imageX / imgW) * developFrame.width));
        const sampleY = Math.min(developFrame.height - 1, Math.floor((imageY / imgH) * developFrame.height));
        const index = (sampleY * developFrame.width + sampleX) * 4;
        const r = developFrame.pixels[index] ?? 0;
        const g = developFrame.pixels[index + 1] ?? 0;
        const b = developFrame.pixels[index + 2] ?? 0;

        if (tatMode === "curve") {
          const channel = developEdl.curves[tatCurveChannel];
          const points = channel.kind === "Points" ? channel.points : [{ input: 0, output: 0 }, { input: 1, output: 1 }];
          const inputValue = (r / 255 + g / 255 + b / 255) / 3;

          let nearestIndex = 0;
          let nearestDistance = Infinity;
          points.forEach((p, i) => {
            const distance = Math.abs(p.input - inputValue);
            if (distance < nearestDistance) {
              nearestDistance = distance;
              nearestIndex = i;
            }
          });

          let workingPoints = points;
          let pointIndex = nearestIndex;
          if (nearestDistance > TAT_NEW_POINT_THRESHOLD) {
            const newPoint = { input: inputValue, output: inputValue };
            workingPoints = [...points, newPoint].sort((a, b2) => a.input - b2.input);
            pointIndex = workingPoints.indexOf(newPoint);
            setCurveChannel(tatCurveChannel, { kind: "Points", points: workingPoints });
          } else if (channel.kind !== "Points") {
            // Bestehender Punkt nah genug, aber der Kanal war bislang
            // parametrisch — auf Punkte umstellen, sonst gäbe es keine
            // Punkte zum Verschieben (siehe Store-Moduldoku).
            setCurveChannel(tatCurveChannel, { kind: "Points", points: workingPoints });
          }

          tatDragState.current = {
            kind: "curve",
            channel: tatCurveChannel,
            pointIndex,
            startOutput: workingPoints[pointIndex]?.output ?? inputValue,
            startClientY: event.clientY,
          };
        } else if (tatMode === "hsl") {
          const band = nearestHslBand(hueDegreesFromRgbByte(r, g, b));
          tatDragState.current = {
            kind: "hsl",
            band,
            startLuminance: developEdl.hsl[band].luminance,
            startClientY: event.clientY,
          };
        }
        return;
      }

      if (pickerActive || event.button !== 0 || !canPan) return;
      dragState.current = { startX: event.clientX, startY: event.clientY, startPanX: panX, startPanY: panY };
    },
    [
      tatActive,
      tatMode,
      tatCurveChannel,
      developFrame,
      imgW,
      imgH,
      containerSize.width,
      containerSize.height,
      effectiveScale,
      panX,
      panY,
      developEdl.curves,
      developEdl.hsl,
      setCurveChannel,
      pickerActive,
      canPan,
    ],
  );

  // Punktfarbmesser (Phase 9 Schritt 4): läuft unabhängig vom Ziehen mit,
  // solange das Entwickeln-Panel offen ist und ein `developFrame`
  // vorliegt — anders als die Weißabgleich-/Farbmischer-Pipetten oben
  // (`handleImageClick`, ausgelöst per Klick) braucht der Punktfarbmesser
  // keinen eigenen Werkzeug-Modus, er zeigt einfach live an, was gerade
  // unter dem Mauszeiger liegt (dieselbe Lightroom-Konvention).
  const handleMouseMove = useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      const drag = dragState.current;
      if (drag) {
        setPan(drag.startPanX + (event.clientX - drag.startX), drag.startPanY + (event.clientY - drag.startY));
      }

      const tatDrag = tatDragState.current;
      if (tatDrag && imgH > 0) {
        // Vertikaler Zug skaliert mit der Bildhöhe *auf dem Bildschirm*
        // (Lightroom-Konvention, siehe PLAN.md Phase 11 Schritt 6): die
        // volle sichtbare Bildhöhe zu ziehen deckt den vollen Regler-
        // Bereich ab, unabhängig vom aktuellen Zoom.
        const pixelHeight = imgH * effectiveScale;
        const deltaFraction = (event.clientY - tatDrag.startClientY) / pixelHeight;
        if (tatDrag.kind === "curve") {
          const channel = developEdl.curves[tatDrag.channel];
          if (channel.kind === "Points") {
            const clampedOutput = Math.min(1, Math.max(0, tatDrag.startOutput - deltaFraction));
            const newPoints = channel.points.map((p, i) => (i === tatDrag.pointIndex ? { ...p, output: clampedOutput } : p));
            setCurveChannel(tatDrag.channel, { kind: "Points", points: newPoints });
          }
        } else {
          const clampedLuminance = Math.min(100, Math.max(-100, tatDrag.startLuminance - deltaFraction * 200));
          setHslBandField(tatDrag.band, "luminance", clampedLuminance);
        }
      }

      if (!developPanelOpen || !developFrame || imgW <= 0 || imgH <= 0) {
        if (pointerSample) setPointerSample(null);
        return;
      }
      const rect = event.currentTarget.getBoundingClientRect();
      const cursor = { x: event.clientX - rect.left, y: event.clientY - rect.top };
      const origin = imageOrigin(containerSize.width, containerSize.height, imgW, imgH, effectiveScale, { x: panX, y: panY });
      const imageX = (cursor.x - origin.x) / effectiveScale;
      const imageY = (cursor.y - origin.y) / effectiveScale;
      if (imageX < 0 || imageY < 0 || imageX >= imgW || imageY >= imgH) {
        if (pointerSample) setPointerSample(null);
        return;
      }
      const sampleX = Math.min(developFrame.width - 1, Math.floor((imageX / imgW) * developFrame.width));
      const sampleY = Math.min(developFrame.height - 1, Math.floor((imageY / imgH) * developFrame.height));
      const index = (sampleY * developFrame.width + sampleX) * 4;
      setPointerSample({
        r: developFrame.pixels[index] ?? 0,
        g: developFrame.pixels[index + 1] ?? 0,
        b: developFrame.pixels[index + 2] ?? 0,
      });
    },
    [
      setPan,
      developPanelOpen,
      developFrame,
      imgW,
      imgH,
      containerSize.width,
      containerSize.height,
      effectiveScale,
      panX,
      panY,
      pointerSample,
      developEdl.curves,
      developEdl.hsl,
      setCurveChannel,
      setHslBandField,
    ],
  );

  const endDrag = useCallback(() => {
    dragState.current = null;
    if (tatDragState.current) {
      tatDragState.current = null;
      void commitDevelopEdit();
    }
  }, [commitDevelopEdit]);

  const handleMouseLeave = useCallback(() => {
    endDrag();
    setPointerSample(null);
  }, [endDrag]);

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
      } else if (aiMaskClickPickerActive) {
        // "Objekte"-KI-Maske (Phase 7 Schritt 2) — braucht anders als die
        // übrigen Bild-Klick-Werkzeuge oben keine Farbe, sondern nur die
        // normierte Klickposition als Startpunkt fürs Region-Growing.
        void addAiMask("ClickRegion", { x: imageX / imgW, y: imageY / imgH });
      } else if (virtualApertureFocusPickerActive) {
        // Fokuspunkt der "Virtuellen Blende" (Phase 14 Schritt 8) —
        // genau wie bei `ClickRegion` oben nur die normierte
        // Klickposition, keine Farbe.
        setVirtualApertureFocusPoint(imageX / imgW, imageY / imgH);
      }
    },
    [
      pickerActive,
      wbPickerActive,
      colorMixerPickerActive,
      maskColorRangePickerActive,
      maskColorMixerPickerActive,
      aiMaskClickPickerActive,
      addAiMask,
      virtualApertureFocusPickerActive,
      setVirtualApertureFocusPoint,
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

  // ---- Clipping-Overlay + Navigator-Viewport (Phase 9 Schritt 4) --------
  // Eigenes, zweites Canvas statt in `QuadRenderer`s WebGL-Zeichenpfad
  // einzugreifen — die Maske ist ohnehin größtenteils transparent
  // (`buildClippingOverlay`), ein simples 2D-`putImageData` + CSS-Skalierung
  // (analog zum WebGL-Canvas darunter) reicht, ohne den bestehenden,
  // bereits komplexen Rendering-Pfad anzufassen.
  useEffect(() => {
    const canvas = clipCanvasRef.current;
    if (!canvas) return;
    if (!clippingOverlayEnabled || !developFrame) {
      canvas.width = 0;
      canvas.height = 0;
      return;
    }
    canvas.width = developFrame.width;
    canvas.height = developFrame.height;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const overlay = buildClippingOverlay(developFrame.pixels, developFrame.width, developFrame.height);
    // `overlay` ist immer über `new Uint8ClampedArray(n)` angelegt (siehe
    // `buildClippingOverlay`), landet also nie auf einem `SharedArrayBuffer`
    // — der Cast räumt nur eine zu strenge TS-Typisierung von `ImageData`
    // aus dem Weg (`ArrayBufferLike` schließt `SharedArrayBuffer` mit ein).
    ctx.putImageData(new ImageData(overlay as Uint8ClampedArray<ArrayBuffer>, developFrame.width, developFrame.height), 0, 0);
  }, [clippingOverlayEnabled, developFrame]);

  const clipOverlayOrigin = imageOrigin(containerSize.width, containerSize.height, imgW, imgH, effectiveScale, { x: panX, y: panY });

  // Normierter (0..1) sichtbarer Bildausschnitt für die Navigator-
  // Miniaturansicht — Umkehrung von `imageOrigin`: Container-Bildschirm-
  // Ecken zurück in Bildkoordinaten, dann auf 0..1 begrenzt.
  const navigatorViewport = useMemo(() => {
    if (imgW <= 0 || imgH <= 0 || effectiveScale <= 0) return null;
    const origin = imageOrigin(containerSize.width, containerSize.height, imgW, imgH, effectiveScale, { x: panX, y: panY });
    const x0 = Math.max(0, (0 - origin.x) / effectiveScale / imgW);
    const y0 = Math.max(0, (0 - origin.y) / effectiveScale / imgH);
    const x1 = Math.min(1, (containerSize.width - origin.x) / effectiveScale / imgW);
    const y1 = Math.min(1, (containerSize.height - origin.y) / effectiveScale / imgH);
    return { x: x0, y: y0, width: Math.max(0, x1 - x0), height: Math.max(0, y1 - y0) };
  }, [imgW, imgH, effectiveScale, containerSize.width, containerSize.height, panX, panY]);

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
      } else if (matchesBinding(event, "zoom-fit")) {
        resetView();
      } else if (matchesBinding(event, "zoom-100")) {
        setZoom(1, "manual");
        setPan(0, 0);
      } else if (event.key === "i" || event.key === "I") {
        // Info-Overlay ein-/ausblenden (Phase 9 Schritt 5) — dieselbe
        // Taste wie in Lightroom Classic.
        toggleInfoOverlay();
      } else if (event.key === "o" || event.key === "O") {
        // Masken-Farbüberlagerung ein-/ausblenden (Phase 12 Schritt 1,
        // siehe DECISIONS.md ADR-0039) — dieselbe Taste wie in Lightroom.
        toggleMaskOverlay();
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
  }, [effectiveScale, fitScale, setZoom, setPan, resetView, toggleInfoOverlay, toggleMaskOverlay]);

  return (
    <main
      ref={containerRef}
      className="relative flex flex-1 items-center justify-center overflow-hidden bg-bg-base"
      onWheel={handleWheel}
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={endDrag}
      onMouseLeave={handleMouseLeave}
      onClick={handleImageClick}
      onDoubleClick={handleDoubleClick}
      style={{ cursor: pickerActive || tatActive ? "crosshair" : canPan ? (dragState.current ? "grabbing" : "grab") : "default" }}
    >
      {!photo && <p className="pointer-events-none text-sm text-text-muted">Kein Foto ausgewählt.</p>}

      <canvas ref={canvasRef} className="pointer-events-none absolute inset-0" />

      {/* Offline-Kennzeichnung (Phase 11 Schritt 4, siehe `DECISIONS.md`
          ADR-0038): `photo.missing` kommt von der bestehenden Abgleich-
          Logik (`reconcile.rs`), die es setzt, sobald die Originaldatei
          nicht mehr gefunden wird — rendert trotzdem etwas (`drawSource`
          nicht leer), kann das nur das Smart-Preview-Fallback in
          `resolve_source_path` gewesen sein. Kein eigenes Backend-Signal
          nötig, siehe dessen Moduldoku. */}
      {photo?.missing && drawSource && (
        <div className="pointer-events-none absolute left-3 top-3 rounded bg-bg-raised/90 px-2 py-1 text-xs font-medium text-accent backdrop-blur">
          Offline (Smart Preview)
        </div>
      )}

      {/* Zielgerichtetes Anpassungswerkzeug (TAT, Phase 11 Schritt 6,
          siehe DECISIONS.md ADR-0038) — nur sichtbar, während das
          Entwickeln-Panel offen ist (die Ziel-Regler leben dort). */}
      {developPanelOpen && (
        <div
          role="group"
          aria-label="Zielgerichtetes Anpassungswerkzeug"
          className="absolute right-3 top-3 flex items-center gap-1 rounded bg-bg-raised/90 p-1 text-xs backdrop-blur"
          onClick={(event) => event.stopPropagation()}
          onMouseDown={(event) => event.stopPropagation()}
        >
          <button
            type="button"
            onClick={() => setTatMode(tatMode === "curve" ? "off" : "curve")}
            aria-pressed={tatMode === "curve"}
            className={`rounded border px-2 py-1 ${tatMode === "curve" ? "border-accent bg-accent/10 text-accent" : "border-border hover:border-accent"}`}
          >
            TAT: Kurve
          </button>
          {tatMode === "curve" && (
            <select
              aria-label="TAT-Kurvenkanal"
              value={tatCurveChannel}
              onChange={(event) => setTatCurveChannel(event.target.value as keyof CurvesAdjustment)}
              className="rounded border border-border bg-bg-panel px-1 py-1"
            >
              {CURVE_CHANNEL_TABS.map((tab) => (
                <option key={tab.key} value={tab.key}>
                  {tab.label}
                </option>
              ))}
            </select>
          )}
          <button
            type="button"
            onClick={() => setTatMode(tatMode === "hsl" ? "off" : "hsl")}
            aria-pressed={tatMode === "hsl"}
            className={`rounded border px-2 py-1 ${tatMode === "hsl" ? "border-accent bg-accent/10 text-accent" : "border-border hover:border-accent"}`}
          >
            TAT: HSL
          </button>
        </div>
      )}

      {clippingOverlayEnabled && developFrame && imgW > 0 && imgH > 0 && (
        <canvas
          ref={clipCanvasRef}
          className="pointer-events-none absolute"
          style={{
            left: clipOverlayOrigin.x,
            top: clipOverlayOrigin.y,
            width: imgW * effectiveScale,
            height: imgH * effectiveScale,
            imageRendering: effectiveScale > 1 ? "pixelated" : "auto",
          }}
        />
      )}

      {photo && referenceViewActive ? (
        <ReferenceView
          workingPhotoId={developPhotoId}
          workingEdlJson={developEdlJson}
          maxEdge={containerSize.width > 0 ? targetFullEdge : undefined}
        />
      ) : (
        photo &&
        beforeAfterMode !== "none" && (
          <BeforeAfterView photoId={developPhotoId} afterEdlJson={developEdlJson} maxEdge={containerSize.width > 0 ? targetFullEdge : undefined} />
        )
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

      {photo && contentAwareMoveActive && imgW > 0 && imgH > 0 && (
        <ContentAwareMoveOverlay
          imageLeft={imageOrigin(containerSize.width, containerSize.height, imgW, imgH, effectiveScale, { x: panX, y: panY }).x}
          imageTop={imageOrigin(containerSize.width, containerSize.height, imgW, imgH, effectiveScale, { x: panX, y: panY }).y}
          imageWidth={imgW * effectiveScale}
          imageHeight={imgH * effectiveScale}
          rect={contentAwareMoveRect}
          loading={contentAwareMoveLoading}
          onRectDrawn={setContentAwareMoveRect}
          onMoveCommitted={(destX, destY) => void commitContentAwareMove(destX, destY)}
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
          skipSourceStep={repairDraftMode === "ContentAwareFill"}
          autoSourceModeActive={autoSourceModeActive}
          onSuggestSource={(point) => void suggestRepairSourceForTarget(point.x, point.y)}
          spotCandidates={sensorSpotCandidates}
        />
      )}

      {/* Masken-Farbüberlagerung (Phase 12 Schritt 1, siehe `DECISIONS.md`
          ADR-0039) — zeigt alle sichtbaren Masken eingefärbt an, unabhängig
          von der gerade zur Bearbeitung ausgewählten. Wird vor dem
          Ziehgriff-Overlay unten gerendert, damit dessen Griffe/Linien
          weiterhin sichtbar über der Einfärbung liegen. */}
      {photo && developPanelOpen && maskOverlayVisible && imgW > 0 && imgH > 0 && (
        <MaskColorOverlay
          imageLeft={imageOrigin(containerSize.width, containerSize.height, imgW, imgH, effectiveScale, { x: panX, y: panY }).x}
          imageTop={imageOrigin(containerSize.width, containerSize.height, imgW, imgH, effectiveScale, { x: panX, y: panY }).y}
          imageWidth={imgW * effectiveScale}
          imageHeight={imgH * effectiveScale}
          masks={visibleMasks(developEdl.masks, maskGroups)}
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

      {/* Bearbeitungs-Pins (Phase 9 Schritt 5): ein anklickbarer Marker je
          Maske mit eindeutiger räumlicher Position — fokussiert die
          zugehörige Maske im `MasksPanel`, reine Anzeige-Überlagerung über
          die bestehende Maskengeometrie. Nur solange nicht gerade eine
          andere Bildwerkzeug-Interaktion läuft (sonst würden sich Klicks
          überschneiden). */}
      {photo &&
        developPanelOpen &&
        !repairActive &&
        !geometryCropActive &&
        !pickerActive &&
        imgW > 0 &&
        imgH > 0 &&
        developEdl.masks.map((mask) => {
          const position = computeMaskPinPosition(mask);
          if (!position) return null;
          const origin = imageOrigin(containerSize.width, containerSize.height, imgW, imgH, effectiveScale, { x: panX, y: panY });
          return (
            <button
              key={mask.id}
              type="button"
              onClick={(event) => {
                event.stopPropagation();
                selectMask(mask.id);
              }}
              title={mask.name}
              aria-label={`Bearbeitungs-Pin: ${mask.name}`}
              className={`absolute h-4 w-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 ${
                mask.id === selectedMaskId ? "border-accent bg-accent/60" : "border-text-primary/70 bg-bg-raised/70"
              }`}
              style={{ left: origin.x + position.x * imgW * effectiveScale, top: origin.y + position.y * imgH * effectiveScale }}
            />
          );
        })}

      {photo && infoOverlayVisible && (
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

      {photo && developPanelOpen && developFrame && (
        <DevelopAnalysisPanel
          frame={developFrame}
          pointerSample={pointerSample}
          clippingOverlayEnabled={clippingOverlayEnabled}
          onToggleClippingOverlay={() => setClippingOverlayEnabled((v) => !v)}
          viewport={navigatorViewport}
          thumbnailUrl={previewUrl(photo.id, 0)}
          onAutoTone={(histogram) => applyAutoTone(computeAutoTone(histogram))}
        />
      )}
    </main>
  );
}
