import { useCallback, useEffect, useRef, useState } from "react";

import { videoUrl } from "../lib/media";
import { useAppStore } from "../store";

function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, "0")}`;
}

/**
 * Video-Wiedergabe (Phase 16 Schritt 5, siehe `DECISIONS.md` ADR-0043) —
 * ersetzt `Viewer` für Fotos mit `media_kind === "video"` als
 * Einzel-Element-Ansicht (siehe `App.tsx`s Fallback-Zweig). Eigene,
 * einfache Zeitleiste statt der nativen `<video controls>`-Steuerung: ein
 * anklickbarer Fortschrittsbalken statt eines Browser-Scrubbers, damit
 * Schritt 6 (Trimmen) dort Anfang-/Ende-Ziehpunkte ergänzen kann, ohne
 * mit einer nativen Steuerleiste zu kollidieren — dasselbe Muster wie
 * `LiquifyOverlay`/`RepairOverlay`: ein eigenes, anklickbares
 * SVG-/Div-Overlay statt eines Browser-Standardelements.
 */
export function VideoPlayer() {
  const selectedFolderId = useAppStore((s) => s.selectedFolderId);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const photos = useAppStore((s) => (selectedFolderId ? s.photosByFolder[selectedFolderId] : undefined));
  const photo = photos?.find((p) => p.id === selectedPhotoId);

  const videoRef = useRef<HTMLVideoElement>(null);
  const timelineRef = useRef<HTMLDivElement>(null);
  const [playing, setPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);

  // Beim Fotowechsel Wiedergabezustand zurücksetzen — sonst zeigt die
  // Zeitleiste kurz den Stand des vorherigen Videos an, bevor die neuen
  // Metadaten geladen sind.
  useEffect(() => {
    setPlaying(false);
    setCurrentTime(0);
    setDuration(0);
  }, [selectedPhotoId]);

  const togglePlay = useCallback(() => {
    const video = videoRef.current;
    if (!video) return;
    if (video.paused) void video.play();
    else video.pause();
  }, []);

  const handleTimelineClick = useCallback((event: React.MouseEvent<HTMLDivElement>) => {
    const video = videoRef.current;
    const timeline = timelineRef.current;
    if (!video || !timeline || duration <= 0) return;
    const rect = timeline.getBoundingClientRect();
    const fraction = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
    video.currentTime = fraction * duration;
  }, [duration]);

  if (!photo) return null;

  const progress = duration > 0 ? currentTime / duration : 0;

  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 overflow-hidden bg-bg-base p-4">
      <video
        ref={videoRef}
        src={videoUrl(photo.id)}
        className="max-h-full max-w-full flex-1 bg-black"
        onClick={togglePlay}
        onPlay={() => setPlaying(true)}
        onPause={() => setPlaying(false)}
        onTimeUpdate={(event) => setCurrentTime(event.currentTarget.currentTime)}
        onLoadedMetadata={(event) => setDuration(event.currentTarget.duration)}
      />

      <div className="flex w-full max-w-3xl flex-col gap-1.5">
        <div
          ref={timelineRef}
          role="slider"
          aria-label="Wiedergabeposition"
          aria-valuemin={0}
          aria-valuemax={Math.round(duration)}
          aria-valuenow={Math.round(currentTime)}
          onClick={handleTimelineClick}
          className="relative h-2 w-full cursor-pointer rounded-full bg-bg-raised"
        >
          <div className="absolute inset-y-0 left-0 rounded-full bg-accent" style={{ width: `${progress * 100}%` }} />
          <div
            className="absolute top-1/2 h-3 w-3 -translate-x-1/2 -translate-y-1/2 rounded-full bg-accent shadow"
            style={{ left: `${progress * 100}%` }}
          />
        </div>

        <div className="flex items-center justify-between text-xs text-text-secondary">
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={togglePlay}
              className="rounded border border-border px-2 py-0.5 text-text-primary hover:border-accent"
            >
              {playing ? "Pause" : "Abspielen"}
            </button>
            <span>
              {formatTime(currentTime)} / {formatTime(duration)}
            </span>
          </div>
          <span className="text-text-muted">
            {photo.width && photo.height ? `${photo.width}×${photo.height}` : null}
            {photo.frame_rate ? ` · ${photo.frame_rate.toFixed(2)} fps` : null}
            {photo.video_codec ? ` · ${photo.video_codec}` : null}
            {photo.has_audio === false ? " · ohne Ton" : null}
          </span>
        </div>
      </div>
    </div>
  );
}
