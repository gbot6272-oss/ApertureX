import { useEffect, useState } from "react";

import { buildSlideItems, type SlideshowSettings, type SlideshowTransition, type TitleCardSettings } from "../lib/slideshow";
import { pickFilePath, pickSaveFilePath, type SlideshowVideoOptions } from "../lib/tauri";
import { useAppStore } from "../store";
import { SlideshowPlayer } from "./SlideshowPlayer";

interface SlideshowDialogProps {
  open: boolean;
  photoIds: string[];
  onClose: () => void;
}

type Resolution = "1280x720" | "1920x1080" | "3840x2160";
const RESOLUTIONS: Record<Resolution, { width: number; height: number }> = {
  "1280x720": { width: 1280, height: 720 },
  "1920x1080": { width: 1920, height: 1080 },
  "3840x2160": { width: 3840, height: 2160 },
};

function hexToRgb(hex: string): [number, number, number] {
  const match = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
  if (!match) return [0, 0, 0];
  // Drei Fanggruppen im Muster oben, also bei einem Treffer immer gefüllt.
  return [parseInt(match[1]!, 16), parseInt(match[2]!, 16), parseInt(match[3]!, 16)];
}

/**
 * Diashow-Dialog (Phase 8 Schritt 4, siehe `PLAN.md`/`DECISIONS.md`
 * ADR-0034). Übergänge/Ken-Burns-Effekt/Intro-Outro-Screens/Musik-
 * Synchronisation sind Einstellungen für die Live-Wiedergabe
 * (`SlideshowPlayer.tsx`, „Abspielen") — derselbe Folienaufbau
 * (`buildSlideItems`) steuert außerdem, was der optionale MP4-Export
 * (`apx_export::video`) erzeugt, auch wenn beide unabhängig laufen (Canvas
 * vs. `ffmpeg`).
 */
export function SlideshowDialog({ open, photoIds, onClose }: SlideshowDialogProps) {
  const ffmpegAvailable = useAppStore((s) => s.ffmpegAvailable);
  const checkFfmpegAvailability = useAppStore((s) => s.checkFfmpegAvailability);
  const videoExportRunning = useAppStore((s) => s.videoExportRunning);
  const videoExportError = useAppStore((s) => s.videoExportError);
  const videoExportOutcome = useAppStore((s) => s.videoExportOutcome);
  const exportSlideshowVideo = useAppStore((s) => s.exportSlideshowVideo);

  const [slideSeconds, setSlideSeconds] = useState(4);
  const [kenBurns, setKenBurns] = useState(true);
  const [transition, setTransition] = useState<SlideshowTransition>("cross_fade");
  const [transitionSeconds, setTransitionSeconds] = useState(1);

  const [introEnabled, setIntroEnabled] = useState(false);
  const [introText, setIntroText] = useState("");
  const [introSeconds, setIntroSeconds] = useState(3);
  const [introBackground, setIntroBackground] = useState("#000000");
  const [introTextColor, setIntroTextColor] = useState("#ffffff");

  const [outroEnabled, setOutroEnabled] = useState(false);
  const [outroText, setOutroText] = useState("");
  const [outroSeconds, setOutroSeconds] = useState(3);
  const [outroBackground, setOutroBackground] = useState("#000000");
  const [outroTextColor, setOutroTextColor] = useState("#ffffff");

  const [fontPath, setFontPath] = useState("");
  const [musicPath, setMusicPath] = useState("");
  const [resolution, setResolution] = useState<Resolution>("1920x1080");
  const [fps, setFps] = useState(25);

  const [playerOpen, setPlayerOpen] = useState(false);

  useEffect(() => {
    if (open && ffmpegAvailable === null) void checkFfmpegAvailability();
  }, [open, ffmpegAvailable, checkFfmpegAvailability]);

  if (!open) return null;

  const needsFont = (introEnabled && introText.length > 0) || (outroEnabled && outroText.length > 0);

  function titleCard(enabled: boolean, text: string, seconds: number, background: string, textColor: string): TitleCardSettings | undefined {
    if (!enabled) return undefined;
    return { text, holdSeconds: seconds, backgroundRgb: hexToRgb(background), textColor: hexToRgb(textColor) };
  }

  const settings: SlideshowSettings = {
    slideSeconds,
    kenBurns,
    transition,
    transitionSeconds,
    intro: titleCard(introEnabled, introText, introSeconds, introBackground, introTextColor),
    outro: titleCard(outroEnabled, outroText, outroSeconds, outroBackground, outroTextColor),
  };
  const slides = buildSlideItems(photoIds, settings);

  async function handlePickFont() {
    const path = await pickFilePath("Schriftdateien", ["ttf", "otf"]);
    if (path) setFontPath(path);
  }

  async function handlePickMusic() {
    const path = await pickFilePath("Audiodateien", ["mp3", "wav", "ogg", "flac", "m4a", "aac"]);
    if (path) setMusicPath(path);
  }

  async function handleExportVideo() {
    const destPath = await pickSaveFilePath("MP4-Video", ["mp4"], "Diashow.mp4");
    if (!destPath) return;
    const { width, height } = RESOLUTIONS[resolution];
    const options: SlideshowVideoOptions = {
      slideSeconds,
      kenBurns,
      transition,
      transitionSeconds: transition === "cross_fade" ? transitionSeconds : undefined,
      width,
      height,
      fps,
      musicPath: musicPath || undefined,
    };
    if (introEnabled && introText) {
      options.intro = { text: introText, seconds: introSeconds, backgroundRgb: hexToRgb(introBackground), textColor: hexToRgb(introTextColor), fontPath: fontPath || undefined, fontSize: 48 };
    }
    if (outroEnabled && outroText) {
      options.outro = { text: outroText, seconds: outroSeconds, backgroundRgb: hexToRgb(outroBackground), textColor: hexToRgb(outroTextColor), fontPath: fontPath || undefined, fontSize: 48 };
    }
    await exportSlideshowVideo(photoIds, destPath, options);
  }

  return (
    <>
      <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-16" onClick={onClose}>
        <div
          onClick={(e) => e.stopPropagation()}
          className="max-h-[85vh] w-full max-w-md overflow-y-auto rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
        >
          <h2 className="mb-1 text-sm font-semibold text-text-primary">Diashow</h2>
          <p className="mb-3 text-xs text-text-muted">
            {photoIds.length} Foto{photoIds.length === 1 ? "" : "s"}
          </p>

          <div className="mb-3 flex gap-2">
            <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
              Dauer je Foto (s)
              <input type="number" min={0.5} step={0.5} value={slideSeconds} onChange={(e) => setSlideSeconds(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
            </label>
            <label className="flex flex-1 items-end gap-2 pb-1 text-xs text-text-secondary">
              <input type="checkbox" checked={kenBurns} onChange={(e) => setKenBurns(e.target.checked)} />
              Ken-Burns-Effekt
            </label>
          </div>

          <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
            Übergang
            <select value={transition} onChange={(e) => setTransition(e.target.value as SlideshowTransition)} className="rounded border border-border bg-bg-panel px-2 py-1 text-sm">
              <option value="cut">Harter Schnitt</option>
              <option value="cross_fade">Überblendung</option>
            </select>
          </label>

          {transition === "cross_fade" && (
            <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
              Dauer der Überblendung (s)
              <input type="number" min={0.1} step={0.1} value={transitionSeconds} onChange={(e) => setTransitionSeconds(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
            </label>
          )}

          <div className="mb-3 rounded border border-border p-2">
            <label className="mb-2 flex items-center gap-2 text-xs text-text-secondary">
              <input type="checkbox" checked={introEnabled} onChange={(e) => setIntroEnabled(e.target.checked)} />
              Intro-Bildschirm
            </label>
            {introEnabled && (
              <div className="flex flex-col gap-2">
                <input type="text" placeholder="Text" value={introText} onChange={(e) => setIntroText(e.target.value)} className="rounded border border-border bg-bg-panel px-2 py-1 text-sm" />
                <div className="flex items-center gap-2">
                  <label className="flex flex-1 items-center gap-1 text-xs text-text-secondary">
                    Dauer (s)
                    <input type="number" min={0.5} step={0.5} value={introSeconds} onChange={(e) => setIntroSeconds(Number(e.target.value))} className="w-16 rounded border border-border bg-bg-panel px-1 py-0.5" />
                  </label>
                  <label className="flex items-center gap-1 text-xs text-text-secondary">
                    Hintergrund
                    <input type="color" value={introBackground} onChange={(e) => setIntroBackground(e.target.value)} />
                  </label>
                  <label className="flex items-center gap-1 text-xs text-text-secondary">
                    Text
                    <input type="color" value={introTextColor} onChange={(e) => setIntroTextColor(e.target.value)} />
                  </label>
                </div>
              </div>
            )}
          </div>

          <div className="mb-3 rounded border border-border p-2">
            <label className="mb-2 flex items-center gap-2 text-xs text-text-secondary">
              <input type="checkbox" checked={outroEnabled} onChange={(e) => setOutroEnabled(e.target.checked)} />
              Outro-Bildschirm
            </label>
            {outroEnabled && (
              <div className="flex flex-col gap-2">
                <input type="text" placeholder="Text" value={outroText} onChange={(e) => setOutroText(e.target.value)} className="rounded border border-border bg-bg-panel px-2 py-1 text-sm" />
                <div className="flex items-center gap-2">
                  <label className="flex flex-1 items-center gap-1 text-xs text-text-secondary">
                    Dauer (s)
                    <input type="number" min={0.5} step={0.5} value={outroSeconds} onChange={(e) => setOutroSeconds(Number(e.target.value))} className="w-16 rounded border border-border bg-bg-panel px-1 py-0.5" />
                  </label>
                  <label className="flex items-center gap-1 text-xs text-text-secondary">
                    Hintergrund
                    <input type="color" value={outroBackground} onChange={(e) => setOutroBackground(e.target.value)} />
                  </label>
                  <label className="flex items-center gap-1 text-xs text-text-secondary">
                    Text
                    <input type="color" value={outroTextColor} onChange={(e) => setOutroTextColor(e.target.value)} />
                  </label>
                </div>
              </div>
            )}
          </div>

          {needsFont && (
            <div className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
              <span>Schriftdatei (für Intro-/Outro-Text)</span>
              <div className="flex gap-1">
                <input type="text" readOnly value={fontPath} placeholder="Keine ausgewählt" className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-sm" />
                <button type="button" onClick={() => void handlePickFont()} className="shrink-0 rounded border border-border px-2 py-1 text-xs hover:border-accent">
                  Schriftdatei wählen…
                </button>
              </div>
            </div>
          )}

          <div className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
            <span>Musik</span>
            <div className="flex gap-1">
              <input type="text" readOnly value={musicPath} placeholder="Keine ausgewählt" className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-sm" />
              <button type="button" onClick={() => void handlePickMusic()} className="shrink-0 rounded border border-border px-2 py-1 text-xs hover:border-accent">
                Musikdatei wählen…
              </button>
            </div>
          </div>

          <div className="mb-3 flex gap-2">
            <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
              Video-Auflösung
              <select value={resolution} onChange={(e) => setResolution(e.target.value as Resolution)} className="rounded border border-border bg-bg-panel px-2 py-1 text-sm">
                <option value="1280x720">1280×720 (HD)</option>
                <option value="1920x1080">1920×1080 (Full HD)</option>
                <option value="3840x2160">3840×2160 (4K)</option>
              </select>
            </label>
            <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
              Bildrate (fps)
              <select value={fps} onChange={(e) => setFps(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1 text-sm">
                <option value={25}>25</option>
                <option value={30}>30</option>
                <option value={60}>60</option>
              </select>
            </label>
          </div>

          {ffmpegAvailable === false && (
            <p className="mb-2 text-xs text-text-muted">Video-Export nicht verfügbar — ffmpeg wurde auf diesem Rechner nicht gefunden.</p>
          )}
          {videoExportError && <p className="mb-2 text-xs text-danger">Fehler: {videoExportError}</p>}
          {!videoExportRunning && videoExportOutcome && (
            <p className="mb-2 text-xs text-text-secondary">
              Video gespeichert: {videoExportOutcome.path} ({videoExportOutcome.duration_seconds.toFixed(1)}s)
            </p>
          )}

          <div className="flex justify-end gap-2">
            <button type="button" onClick={onClose} className="rounded border border-border px-3 py-1 text-xs hover:border-accent">
              Schließen
            </button>
            <button
              type="button"
              onClick={() => setPlayerOpen(true)}
              disabled={photoIds.length === 0}
              className="rounded border border-border bg-bg-panel px-3 py-1 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-50"
            >
              Abspielen
            </button>
            <button
              type="button"
              onClick={() => void handleExportVideo()}
              disabled={photoIds.length === 0 || videoExportRunning || ffmpegAvailable !== true}
              className="rounded border border-accent bg-accent/10 px-3 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
            >
              {videoExportRunning ? "Exportiere Video…" : "Als Video exportieren (MP4)"}
            </button>
          </div>
        </div>
      </div>

      {playerOpen && (
        <SlideshowPlayer
          slides={slides}
          transition={transition}
          transitionSeconds={transitionSeconds}
          musicPath={musicPath || undefined}
          onClose={() => setPlayerOpen(false)}
        />
      )}
    </>
  );
}
