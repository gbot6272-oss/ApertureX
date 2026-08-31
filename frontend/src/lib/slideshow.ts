/**
 * Reine Zeitachsen-/Ken-Burns-Mathematik für die Diashow-Live-Wiedergabe
 * (Phase 8 Schritt 4, `PLAN.md`: „Übergänge/Ken-Burns-Effekt/Intro-Outro-
 * Screens: reine Frontend-Canvas-Wiedergabe"). Getrennt von
 * `SlideshowPlayer.tsx`, damit die Zeitachsen-Logik unabhängig von
 * React/Canvas nachvollziehbar bleibt (wie `viewerMath.ts`).
 *
 * **Bewusste Doppelung mit `apx_export::video`:** derselbe Ken-Burns-
 * Interpolationsformel und dieselbe Zwei-Übergangsarten-Regel (harter
 * Schnitt/Überblendung zweier eingefrorener Endzustände) existieren auch
 * in Rust (`crates/apx-export/src/video.rs`) für den Video-Export — die
 * Live-Vorschau läuft zeitbasiert (`requestAnimationFrame`), der Export
 * bildbasiert (feste Bildrate), eine gemeinsame Implementierung über die
 * Sprachgrenze hinweg wäre unverhältnismäßig aufwendig. Beide Formeln sind
 * absichtlich identisch gehalten, damit Vorschau und exportiertes Video
 * sichtbar gleich aussehen — ändert sich eine Seite, muss die andere von
 * Hand nachgezogen werden.
 */

export interface KenBurnsSpec {
  zoomStart: number;
  zoomEnd: number;
  panStart: [number, number];
  panEnd: [number, number];
}

export const STATIC_KEN_BURNS: KenBurnsSpec = {
  zoomStart: 1,
  zoomEnd: 1,
  panStart: [0.5, 0.5],
  panEnd: [0.5, 0.5],
};

const KEN_BURNS_TARGETS: [number, number][] = [
  [0.3, 0.3],
  [0.7, 0.3],
  [0.3, 0.7],
  [0.7, 0.7],
  [0.5, 0.4],
];
const KEN_BURNS_MAX_ZOOM = 1.25;

/** Deterministisches Ken-Burns-Muster je Folienindex — siehe
 * `apx_export::video::default_ken_burns`s Moduldoku (identische Formel). */
export function defaultKenBurns(index: number, enabled: boolean): KenBurnsSpec {
  if (!enabled) return STATIC_KEN_BURNS;
  // Modulo eines nichtleeren, festen Arrays — immer im gültigen Bereich.
  const target = KEN_BURNS_TARGETS[index % KEN_BURNS_TARGETS.length]!;
  return index % 2 === 0
    ? { zoomStart: 1, zoomEnd: KEN_BURNS_MAX_ZOOM, panStart: [0.5, 0.5], panEnd: target }
    : { zoomStart: KEN_BURNS_MAX_ZOOM, zoomEnd: 1, panStart: target, panEnd: [0.5, 0.5] };
}

export interface CropRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Der normierte Bildausschnitt (`0..=1`) bei `progress` (`0`=Start,
 * `1`=Ende, linear interpoliert, außerhalb geklemmt). */
export function kenBurnsCropRect(spec: KenBurnsSpec, progress: number): CropRect {
  const t = Math.min(1, Math.max(0, progress));
  const zoom = Math.max(1, spec.zoomStart + (spec.zoomEnd - spec.zoomStart) * t);
  const cx = spec.panStart[0] + (spec.panEnd[0] - spec.panStart[0]) * t;
  const cy = spec.panStart[1] + (spec.panEnd[1] - spec.panStart[1]) * t;
  const w = 1 / zoom;
  const h = 1 / zoom;
  const x = Math.min(Math.max(cx - w / 2, 0), Math.max(1 - w, 0));
  const y = Math.min(Math.max(cy - h / 2, 0), Math.max(1 - h, 0));
  return { x, y, w, h };
}

export interface SourceRect {
  sx: number;
  sy: number;
  sw: number;
  sh: number;
}

/** Passt `crop` (normierter Ken-Burns-Ausschnitt) zusätzlich an das
 * Seitenverhältnis der Zielfläche an ("cover" — wie CSS `object-fit:
 * cover") und gibt ihn in Quellbild-Pixelkoordinaten zurück, fertig für
 * `CanvasRenderingContext2D.drawImage`. Ohne das würde z. B. ein
 * hochformatiges Foto auf eine querformatige Leinwand verzerrt gestreckt
 * statt beschnitten. Dieselbe Formel (nur in Pixel- statt normierten
 * Koordinaten) wie `apx_export::video`s `cover_adjust` — siehe Moduldoku
 * zur bewussten Doppelung. */
export function coverAdjustedSourceRect(
  crop: CropRect,
  imageWidth: number,
  imageHeight: number,
  targetWidth: number,
  targetHeight: number,
): SourceRect {
  if (imageWidth <= 0 || imageHeight <= 0 || targetWidth <= 0 || targetHeight <= 0 || crop.w <= 0 || crop.h <= 0) {
    return { sx: crop.x * imageWidth, sy: crop.y * imageHeight, sw: crop.w * imageWidth, sh: crop.h * imageHeight };
  }
  const cropWpx = crop.w * imageWidth;
  const cropHpx = crop.h * imageHeight;
  const cropAspect = cropWpx / cropHpx;
  const targetAspect = targetWidth / targetHeight;

  let sx = crop.x * imageWidth;
  let sy = crop.y * imageHeight;
  let sw = cropWpx;
  let sh = cropHpx;

  if (cropAspect > targetAspect) {
    const newW = cropHpx * targetAspect;
    sx += (cropWpx - newW) / 2;
    sw = newW;
  } else {
    const newH = cropWpx / targetAspect;
    sy += (cropHpx - newH) / 2;
    sh = newH;
  }
  return { sx, sy, sw, sh };
}

export type SlideshowTransition = "cut" | "cross_fade";

export type SlideKind = "photo" | "title";

export interface PhotoSlideItem {
  kind: "photo";
  photoId: string;
  holdSeconds: number;
  kenBurns: KenBurnsSpec;
}

export interface TitleSlideItem {
  kind: "title";
  text: string;
  holdSeconds: number;
  backgroundRgb: [number, number, number];
  textColor: [number, number, number];
}

export type SlideItem = PhotoSlideItem | TitleSlideItem;

export type TimelineSegment =
  | { type: "hold"; slideIndex: number; startTime: number; endTime: number }
  | { type: "transition"; fromIndex: number; toIndex: number; startTime: number; endTime: number };

const MIN_HOLD_SECONDS = 0.1;

/** Baut die vollständige Zeitachse für `slides` — jede Folie mindestens
 * `MIN_HOLD_SECONDS` lang, zwischen zwei Folien optional
 * `transitionSeconds` Überblendungszeit (siehe Moduldoku: Überblendung
 * mischt die beiden eingefrorenen Endzustände, kein Weiterzoomen während
 * der Überblendung selbst). Leer bei leeren `slides`. */
export function buildTimeline(
  slides: SlideItem[],
  transition: SlideshowTransition,
  transitionSeconds: number,
): TimelineSegment[] {
  const segments: TimelineSegment[] = [];
  const transitionSpan = transition === "cross_fade" ? Math.max(0, transitionSeconds) : 0;
  let t = 0;
  slides.forEach((slide, index) => {
    const hold = Math.max(MIN_HOLD_SECONDS, slide.holdSeconds);
    segments.push({ type: "hold", slideIndex: index, startTime: t, endTime: t + hold });
    t += hold;
    if (transitionSpan > 0 && index + 1 < slides.length) {
      segments.push({ type: "transition", fromIndex: index, toIndex: index + 1, startTime: t, endTime: t + transitionSpan });
      t += transitionSpan;
    }
  });
  return segments;
}

export function timelineDuration(segments: TimelineSegment[]): number {
  return segments.length === 0 ? 0 : segments[segments.length - 1]!.endTime;
}

/** Das Zeitachsen-Segment, das `time` Sekunden nach dem Start abdeckt —
 * `time` wird auf `[0, Gesamtlänge)` geklemmt, `null` bei leerer
 * Zeitachse. */
export function segmentAtTime(segments: TimelineSegment[], time: number): TimelineSegment | null {
  if (segments.length === 0) return null;
  const duration = timelineDuration(segments);
  const clamped = Math.min(Math.max(time, 0), Math.max(duration - 1e-6, 0));
  return segments.find((s) => clamped >= s.startTime && clamped < s.endTime) ?? segments[segments.length - 1]!;
}

/** Fortschritt (`0..=1`) einer Halte-Folie innerhalb ihres eigenen
 * Segments — treibt sowohl den Ken-Burns-Effekt als auch (für
 * Titelkarten, die keinen Effekt haben) einfach die Anzeige. */
export function segmentProgress(segment: TimelineSegment, time: number): number {
  const span = segment.endTime - segment.startTime;
  if (span <= 0) return 1;
  return Math.min(1, Math.max(0, (time - segment.startTime) / span));
}

/** Einstellungen für eine Titelkarte (Intro/Outro) — siehe
 * `SlideshowDialog.tsx`. */
export type TitleCardSettings = Omit<TitleSlideItem, "kind">;

export interface SlideshowSettings {
  slideSeconds: number;
  kenBurns: boolean;
  transition: SlideshowTransition;
  transitionSeconds: number;
  intro?: TitleCardSettings;
  outro?: TitleCardSettings;
}

/** Baut die vereinheitlichte Folienliste (Intro, dann ein Eintrag je Foto
 * mit deterministischem Ken-Burns-Muster, dann Outro) aus `photoIds` und
 * `settings` — gemeinsam von `SlideshowPlayer` (Live-Wiedergabe) und
 * `SlideshowDialog` (Vorschau-Aufruf) genutzt, damit beide garantiert
 * dieselbe Folge sehen. */
export function buildSlideItems(photoIds: string[], settings: SlideshowSettings): SlideItem[] {
  const items: SlideItem[] = [];
  if (settings.intro) items.push({ kind: "title", ...settings.intro });
  photoIds.forEach((photoId, index) => {
    items.push({
      kind: "photo",
      photoId,
      holdSeconds: settings.slideSeconds,
      kenBurns: defaultKenBurns(index, settings.kenBurns),
    });
  });
  if (settings.outro) items.push({ kind: "title", ...settings.outro });
  return items;
}
