import { useEffect, useMemo, useRef, useState } from "react";

import { imageUrl, musicUrl } from "../lib/media";
import {
  buildTimeline,
  coverAdjustedSourceRect,
  kenBurnsCropRect,
  segmentAtTime,
  segmentProgress,
  timelineDuration,
  type SlideItem,
  type SlideshowTransition,
  type TimelineSegment,
} from "../lib/slideshow";

interface SlideshowPlayerProps {
  slides: SlideItem[];
  transition: SlideshowTransition;
  transitionSeconds: number;
  /** Absoluter Pfad einer lokalen Audiodatei, `undefined` = stumm. */
  musicPath?: string;
  onClose: () => void;
}

const CANVAS_WIDTH = 1920;
const CANVAS_HEIGHT = 1080;

/**
 * Live-Wiedergabe der Diashow (Phase 8 Schritt 4, `PLAN.md`: „Übergänge/
 * Ken-Burns-Effekt/Intro-Outro-Screens: reine Frontend-Canvas-
 * Wiedergabe"). Lädt alle Fotos der Auswahl einmalig als `ImageBitmap`
 * (siehe `useImageBitmap`-Hook — hier eine eigene, auf mehrere gleichzeitig
 * gehaltene Bitmaps zugeschnittene Variante), dann treibt eine
 * `requestAnimationFrame`-Schleife Canvas-Zeichnung und `<audio>`-
 * Wiedergabe gemeinsam über dieselbe Zeitachse (`lib/slideshow.ts`) an —
 * kein Rust-Audio-Crate nötig (siehe `PLAN.md` Schritt 4).
 */
export function SlideshowPlayer({ slides, transition, transitionSeconds, musicPath, onClose }: SlideshowPlayerProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const audioRef = useRef<HTMLAudioElement>(null);
  const bitmapsRef = useRef<Map<string, ImageBitmap>>(new Map());
  const [loading, setLoading] = useState(true);
  const [finished, setFinished] = useState(false);

  const timeline = useMemo(() => buildTimeline(slides, transition, transitionSeconds), [slides, transition, transitionSeconds]);
  const duration = timelineDuration(timeline);

  // Escape schließt die Wiedergabe, wie `CommandPalette`/`Sidebar`.
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  // Alle benötigten Fotos einmalig vorab laden (siehe Moduldoku) — für die
  // in dieser Phase üblichen Diashow-Größen (einige Dutzend Fotos)
  // unproblematisch; keine seitenweise Nachladelogik.
  useEffect(() => {
    let cancelled = false;
    const photoIds = Array.from(
      new Set(slides.filter((slide): slide is Extract<SlideItem, { kind: "photo" }> => slide.kind === "photo").map((slide) => slide.photoId)),
    );
    setLoading(true);
    setFinished(false);

    void (async () => {
      const loaded = await Promise.all(
        photoIds.map(async (photoId) => {
          try {
            const response = await fetch(imageUrl(photoId, 2048));
            if (!response.ok) throw new Error(`HTTP ${response.status}`);
            const blob = await response.blob();
            const bitmap = await createImageBitmap(blob);
            return [photoId, bitmap] as const;
          } catch (err) {
            console.error("Diashow-Foto konnte nicht geladen werden:", photoId, err);
            return null;
          }
        }),
      );
      if (cancelled) {
        loaded.forEach((entry) => entry?.[1].close());
        return;
      }
      bitmapsRef.current = new Map(loaded.filter((entry): entry is readonly [string, ImageBitmap] => entry !== null));
      setLoading(false);
    })();

    return () => {
      cancelled = true;
      bitmapsRef.current.forEach((bitmap) => bitmap.close());
      bitmapsRef.current = new Map();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `slides` als Ganzes ist die eigentliche Abhängigkeit, nicht jede Foto-ID einzeln
  }, [slides]);

  useEffect(() => {
    if (loading || timeline.length === 0) return;
    const canvas = canvasRef.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;

    const drawSlideAt = (slide: SlideItem, progress: number, alpha: number) => {
      const w = canvas.width;
      const h = canvas.height;
      ctx.save();
      ctx.globalAlpha = alpha;
      if (slide.kind === "title") {
        ctx.fillStyle = `rgb(${slide.backgroundRgb.join(",")})`;
        ctx.fillRect(0, 0, w, h);
        if (slide.text) {
          ctx.fillStyle = `rgb(${slide.textColor.join(",")})`;
          ctx.font = `${Math.round(Math.min(w, h) * 0.08)}px sans-serif`;
          ctx.textAlign = "center";
          ctx.textBaseline = "middle";
          ctx.fillText(slide.text, w / 2, h / 2);
        }
      } else {
        const bitmap = bitmapsRef.current.get(slide.photoId);
        if (bitmap) {
          const crop = kenBurnsCropRect(slide.kenBurns, progress);
          const { sx, sy, sw, sh } = coverAdjustedSourceRect(crop, bitmap.width, bitmap.height, w, h);
          ctx.drawImage(bitmap, sx, sy, sw, sh, 0, 0, w, h);
        } else {
          ctx.fillStyle = "black";
          ctx.fillRect(0, 0, w, h);
        }
      }
      ctx.restore();
    };

    // Alle Indizes hier stammen aus `buildTimeline(slides, …)` selbst
    // (siehe `lib/slideshow.ts`) und sind damit immer gültige Indizes in
    // dasselbe `slides`-Array.
    const drawSegment = (segment: TimelineSegment, elapsed: number) => {
      if (segment.type === "hold") {
        drawSlideAt(slides[segment.slideIndex]!, segmentProgress(segment, elapsed), 1);
      } else {
        drawSlideAt(slides[segment.fromIndex]!, 1, 1);
        drawSlideAt(slides[segment.toIndex]!, 0, segmentProgress(segment, elapsed));
      }
    };

    const startedAt = performance.now();
    let rafId: number;
    audioRef.current?.play().catch(() => {
      // Autoplay kann von der Laufzeit blockiert werden — die Diashow
      // läuft dann stumm weiter statt abzustürzen.
    });

    const tick = () => {
      const elapsed = (performance.now() - startedAt) / 1000;
      if (elapsed >= duration) {
        // `timeline.length === 0` wurde oben bereits abgefangen (der Effekt
        // kehrt in dem Fall früher zurück) — hier also immer vorhanden.
        const last = timeline[timeline.length - 1]!;
        drawSegment(last, duration - 1e-6);
        setFinished(true);
        return;
      }
      const segment = segmentAtTime(timeline, elapsed);
      if (segment) drawSegment(segment, elapsed);
      rafId = requestAnimationFrame(tick);
    };
    rafId = requestAnimationFrame(tick);

    return () => {
      cancelAnimationFrame(rafId);
      audioRef.current?.pause();
    };
  }, [loading, timeline, duration, slides]);

  return (
    <div role="dialog" aria-label="Diashow-Wiedergabe" className="fixed inset-0 z-50 flex flex-col items-center justify-center bg-black">
      {loading && <p className="text-sm text-white">Lade Fotos…</p>}
      <canvas
        ref={canvasRef}
        width={CANVAS_WIDTH}
        height={CANVAS_HEIGHT}
        className="max-h-full max-w-full"
        style={{ display: loading ? "none" : "block" }}
      />
      {musicPath && <audio ref={audioRef} src={musicUrl(musicPath)} />}
      {finished && <p className="absolute bottom-8 text-sm text-white">Diashow beendet</p>}
      <button
        type="button"
        onClick={onClose}
        className="absolute right-4 top-4 rounded border border-white/40 px-3 py-1 text-xs text-white hover:bg-white/10"
      >
        Schließen
      </button>
    </div>
  );
}
