import { useCallback, useEffect, useRef, useState } from "react";

import type { LutFilterData } from "../lib/edl";
import { videoUrl } from "../lib/media";
import { importLutCubeFile, pickFilePath } from "../lib/tauri";
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

  const videoTrimStartMs = useAppStore((s) => s.videoTrimStartMs);
  const videoTrimEndMs = useAppStore((s) => s.videoTrimEndMs);
  const setVideoTrimStart = useAppStore((s) => s.setVideoTrimStart);
  const setVideoTrimEnd = useAppStore((s) => s.setVideoTrimEnd);
  const clearVideoTrim = useAppStore((s) => s.clearVideoTrim);
  const videoTrimSaving = useAppStore((s) => s.videoTrimSaving);
  const videoTrimError = useAppStore((s) => s.videoTrimError);
  const commitVideoTrim = useAppStore((s) => s.commitVideoTrim);

  const videoSceneChanges = useAppStore((s) => s.videoSceneChanges);
  const videoSceneChangesLoading = useAppStore((s) => s.videoSceneChangesLoading);
  const videoSceneChangesError = useAppStore((s) => s.videoSceneChangesError);
  const detectVideoSceneChanges = useAppStore((s) => s.detectVideoSceneChanges);
  const clearVideoSceneChanges = useAppStore((s) => s.clearVideoSceneChanges);
  const useSceneAsVideoTrim = useAppStore((s) => s.useSceneAsVideoTrim);

  const videoAudioBusy = useAppStore((s) => s.videoAudioBusy);
  const videoAudioError = useAppStore((s) => s.videoAudioError);
  const denoiseCurrentVideoAudio = useAppStore((s) => s.denoiseCurrentVideoAudio);
  const addAudioToCurrentVideo = useAppStore((s) => s.addAudioToCurrentVideo);

  const builtinLutFilters = useAppStore((s) => s.builtinLutFilters);
  const loadBuiltinLutFilters = useAppStore((s) => s.loadBuiltinLutFilters);
  const videoLutBusy = useAppStore((s) => s.videoLutBusy);
  const videoLutError = useAppStore((s) => s.videoLutError);
  const applyLutFilterToCurrentVideo = useAppStore((s) => s.applyLutFilterToCurrentVideo);

  const similarVideoGroups = useAppStore((s) => s.similarVideoGroups);
  const similarVideosLoading = useAppStore((s) => s.similarVideosLoading);
  const similarVideosError = useAppStore((s) => s.similarVideosError);
  const findSimilarVideos = useAppStore((s) => s.findSimilarVideos);
  const jumpToVideo = useAppStore((s) => s.jumpToVideo);

  const videoRef = useRef<HTMLVideoElement>(null);
  const timelineRef = useRef<HTMLDivElement>(null);
  const [playing, setPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [denoiseStrength, setDenoiseStrength] = useState<"low" | "medium" | "high">("medium");
  const [musicPath, setMusicPath] = useState<string | null>(null);
  const [musicMode, setMusicMode] = useState<"mix" | "replace">("mix");
  const [musicVolume, setMusicVolume] = useState(1);
  const [customLut, setCustomLut] = useState<LutFilterData | null>(null);
  const [selectedLutKey, setSelectedLutKey] = useState<string>("");
  const [lutStrength, setLutStrength] = useState(1);

  // Die fünf eingebauten Filter-Looks (dieselben wie im Foto-
  // Entwickeln-Panel, `LutFilterPanel.tsx`) werden lazy beim ersten
  // Öffnen einer Video-Ansicht geladen, danach für die ganze Sitzung
  // wiederverwendet (siehe `loadBuiltinLutFilters`s Moduldoku).
  useEffect(() => {
    void loadBuiltinLutFilters();
  }, [loadBuiltinLutFilters]);

  // Beim Fotowechsel Wiedergabezustand zurücksetzen — sonst zeigt die
  // Zeitleiste kurz den Stand des vorherigen Videos an, bevor die neuen
  // Metadaten geladen sind.
  useEffect(() => {
    setPlaying(false);
    setCurrentTime(0);
    setDuration(0);
    clearVideoTrim();
    clearVideoSceneChanges();
    setMusicPath(null);
  }, [selectedPhotoId, clearVideoTrim, clearVideoSceneChanges]);

  const handleImportCustomLut = useCallback(async () => {
    const imported = await importLutCubeFile();
    if (imported) {
      setCustomLut(imported);
      setSelectedLutKey("custom");
    }
  }, []);

  const selectedLut: LutFilterData | null =
    selectedLutKey === "custom"
      ? customLut
      : selectedLutKey !== "" && builtinLutFilters
        ? (builtinLutFilters[Number(selectedLutKey)] ?? null)
        : null;

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

  // Setzt den jeweiligen Trimm-Punkt auf die aktuelle Wiedergabeposition
  // (Millisekunden, wie `trim_video`s Signatur es erwartet) — dasselbe
  // "aktuelle Position markieren"-Muster wie viele Videoschnittwerkzeuge,
  // statt eigener Zieh-Ziehpunkte auf der Zeitleiste.
  const markStart = useCallback(() => {
    setVideoTrimStart(Math.round(currentTime * 1000));
  }, [currentTime, setVideoTrimStart]);

  const markEnd = useCallback(() => {
    setVideoTrimEnd(Math.round(currentTime * 1000));
  }, [currentTime, setVideoTrimEnd]);

  const useCurrentSceneAsTrim = useCallback(() => {
    useSceneAsVideoTrim(Math.round(currentTime * 1000));
  }, [currentTime, useSceneAsVideoTrim]);

  const handlePickMusic = useCallback(async () => {
    const path = await pickFilePath("Audio", ["mp3", "wav", "ogg", "flac", "m4a", "aac"]);
    if (path) setMusicPath(path);
  }, []);

  if (!photo) return null;

  const similarGroup = similarVideoGroups?.find((group) => group.some((entry) => entry.photo.id === photo.id));
  const otherSimilarVideos = similarGroup?.filter((entry) => entry.photo.id !== photo.id) ?? [];

  const progress = duration > 0 ? currentTime / duration : 0;
  const trimStartProgress =
    videoTrimStartMs !== null && duration > 0 ? Math.min(1, Math.max(0, videoTrimStartMs / 1000 / duration)) : null;
  const trimEndProgress =
    videoTrimEndMs !== null && duration > 0 ? Math.min(1, Math.max(0, videoTrimEndMs / 1000 / duration)) : null;
  const canTrim =
    videoTrimStartMs !== null && videoTrimEndMs !== null && videoTrimEndMs > videoTrimStartMs && !videoTrimSaving;

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
          {trimStartProgress !== null && trimEndProgress !== null ? (
            <div
              className="absolute inset-y-0 rounded-full bg-accent/25"
              style={{ left: `${trimStartProgress * 100}%`, width: `${Math.max(0, trimEndProgress - trimStartProgress) * 100}%` }}
            />
          ) : null}
          {trimStartProgress !== null ? (
            <div
              aria-hidden="true"
              className="absolute top-1/2 h-3.5 w-1 -translate-x-1/2 -translate-y-1/2 rounded-full bg-green-500"
              style={{ left: `${trimStartProgress * 100}%` }}
            />
          ) : null}
          {trimEndProgress !== null ? (
            <div
              aria-hidden="true"
              className="absolute top-1/2 h-3.5 w-1 -translate-x-1/2 -translate-y-1/2 rounded-full bg-red-500"
              style={{ left: `${trimEndProgress * 100}%` }}
            />
          ) : null}
          {videoSceneChanges?.map((ms) =>
            duration > 0 ? (
              <div
                key={ms}
                aria-hidden="true"
                className="absolute inset-y-0 w-px bg-yellow-500/70"
                style={{ left: `${Math.min(1, Math.max(0, ms / 1000 / duration)) * 100}%` }}
              />
            ) : null,
          )}
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

        {/* Schneiden/Trimmen (Phase 16 Schritt 6): nicht-destruktiv — der
            Knopf legt über `commitVideoTrim` ein neues Katalog-Asset an,
            das Original bleibt unangetastet (siehe `trim_video`s Moduldoku
            in `apx_app::commands`). */}
        <div className="flex flex-wrap items-center gap-2 border-t border-border pt-2 text-xs">
          <button
            type="button"
            onClick={markStart}
            className="rounded border border-border px-2 py-0.5 text-text-primary hover:border-green-500"
          >
            Anfang markieren
          </button>
          <span className="text-text-muted">
            {videoTrimStartMs !== null ? formatTime(videoTrimStartMs / 1000) : "–"}
          </span>
          <button
            type="button"
            onClick={markEnd}
            className="rounded border border-border px-2 py-0.5 text-text-primary hover:border-red-500"
          >
            Ende markieren
          </button>
          <span className="text-text-muted">
            {videoTrimEndMs !== null ? formatTime(videoTrimEndMs / 1000) : "–"}
          </span>
          <button
            type="button"
            onClick={clearVideoTrim}
            disabled={videoTrimStartMs === null && videoTrimEndMs === null}
            className="rounded border border-border px-2 py-0.5 text-text-secondary hover:border-accent disabled:opacity-40"
          >
            Zurücksetzen
          </button>
          <button
            type="button"
            onClick={() => void commitVideoTrim()}
            disabled={!canTrim}
            className="ml-auto rounded border border-accent bg-accent/10 px-3 py-0.5 font-medium text-accent hover:bg-accent/20 disabled:cursor-not-allowed disabled:opacity-40"
          >
            {videoTrimSaving ? "Schneide…" : "Schneiden"}
          </button>
        </div>
        {videoTrimError ? <p className="text-xs text-red-500">{videoTrimError}</p> : null}

        {/* Automatisches Zuschneiden (Phase 16 Schritt 7): Szenenwechsel-
            Erkennung per ffmpegs `scdet`-Filter (gelbe Striche auf der
            Zeitleiste oben) — "Diesen Abschnitt übernehmen" belegt Start/
            Ende automatisch mit den beiden Wechseln links/rechts der
            aktuellen Wiedergabeposition vor, statt beide Punkte manuell
            markieren zu müssen. */}
        <div className="flex flex-wrap items-center gap-2 border-t border-border pt-2 text-xs">
          <button
            type="button"
            onClick={() => void detectVideoSceneChanges()}
            disabled={videoSceneChangesLoading}
            className="rounded border border-border px-2 py-0.5 text-text-primary hover:border-accent disabled:opacity-40"
          >
            {videoSceneChangesLoading ? "Erkenne Szenen…" : "Szenenwechsel erkennen"}
          </button>
          {videoSceneChanges ? (
            <>
              <span className="text-text-muted">
                {videoSceneChanges.length === 0
                  ? "keine Wechsel gefunden"
                  : `${videoSceneChanges.length} Wechsel gefunden`}
              </span>
              <button
                type="button"
                onClick={useCurrentSceneAsTrim}
                className="rounded border border-border px-2 py-0.5 text-text-primary hover:border-accent"
              >
                Diesen Abschnitt übernehmen
              </button>
            </>
          ) : null}
        </div>
        {videoSceneChangesError ? <p className="text-xs text-red-500">{videoSceneChangesError}</p> : null}

        {/* Geräuschreduktion + Musik/Sounds hinzufügen (Phase 16 Schritt 8):
            beide nicht-destruktiv — legen bei Erfolg ein neues Katalog-
            Asset an, das automatisch ausgewählt wird (siehe
            `denoise_video_audio`/`add_video_audio_track`s Moduldoku in
            `apx_app::commands`). */}
        <div className="flex flex-wrap items-center gap-2 border-t border-border pt-2 text-xs">
          <span className="text-text-secondary">Entrauschen:</span>
          <select
            value={denoiseStrength}
            onChange={(e) => setDenoiseStrength(e.target.value as "low" | "medium" | "high")}
            className="rounded border border-border bg-bg-panel px-1 py-0.5"
          >
            <option value="low">schwach</option>
            <option value="medium">mittel</option>
            <option value="high">stark</option>
          </select>
          <button
            type="button"
            onClick={() => void denoiseCurrentVideoAudio(denoiseStrength)}
            disabled={videoAudioBusy || photo.has_audio !== true}
            title={photo.has_audio !== true ? "Video hat keine Tonspur" : undefined}
            className="rounded border border-border px-2 py-0.5 text-text-primary hover:border-accent disabled:opacity-40"
          >
            {videoAudioBusy ? "Verarbeite…" : "Entrauschen"}
          </button>
        </div>

        <div className="flex flex-wrap items-center gap-2 border-t border-border pt-2 text-xs">
          <span className="text-text-secondary">Musik:</span>
          <button
            type="button"
            onClick={() => void handlePickMusic()}
            className="rounded border border-border px-2 py-0.5 text-text-primary hover:border-accent"
          >
            {musicPath ? "Andere Datei wählen" : "Datei wählen"}
          </button>
          {musicPath ? (
            <>
              <span className="max-w-[12rem] truncate text-text-muted" title={musicPath}>
                {musicPath.split(/[/\\]/).pop()}
              </span>
              <select
                value={musicMode}
                onChange={(e) => setMusicMode(e.target.value as "mix" | "replace")}
                className="rounded border border-border bg-bg-panel px-1 py-0.5"
              >
                <option value="mix">mit Ton mischen</option>
                <option value="replace">Ton ersetzen</option>
              </select>
              <label className="flex items-center gap-1 text-text-secondary">
                Lautstärke
                <input
                  type="range"
                  min={0}
                  max={2}
                  step={0.05}
                  value={musicVolume}
                  onChange={(e) => setMusicVolume(Number(e.target.value))}
                  className="w-20"
                />
                <span className="w-8 text-right text-text-muted">{Math.round(musicVolume * 100)}%</span>
              </label>
              <button
                type="button"
                onClick={() => void addAudioToCurrentVideo(musicPath, musicMode, musicVolume)}
                disabled={videoAudioBusy}
                className="ml-auto rounded border border-accent bg-accent/10 px-3 py-0.5 font-medium text-accent hover:bg-accent/20 disabled:cursor-not-allowed disabled:opacity-40"
              >
                {videoAudioBusy ? "Verarbeite…" : "Hinzufügen"}
              </button>
            </>
          ) : null}
        </div>
        {videoAudioError ? <p className="text-xs text-red-500">{videoAudioError}</p> : null}

        {/* Filter/LUT auf Video anwenden (Phase 16 Schritt 9): dieselben
            fünf eingebauten Filter-Looks wie im Foto-Entwickeln-Panel
            (`LutFilterPanel.tsx`) plus freier `.cube`-Import — bewusst
            global (keine Pinselstriche wie bei Fotos, siehe
            `apply_lut_filter_to_video`s Moduldoku). */}
        <div className="flex flex-wrap items-center gap-2 border-t border-border pt-2 text-xs">
          <span className="text-text-secondary">Filter:</span>
          <select
            value={selectedLutKey}
            onChange={(e) => setSelectedLutKey(e.target.value)}
            className="rounded border border-border bg-bg-panel px-1 py-0.5"
          >
            <option value="">– wählen –</option>
            {builtinLutFilters?.map((lut, index) => (
              <option key={lut.name} value={index}>
                {lut.name}
              </option>
            ))}
            {customLut ? <option value="custom">{customLut.name} (importiert)</option> : null}
          </select>
          <button
            type="button"
            onClick={() => void handleImportCustomLut()}
            className="rounded border border-border px-2 py-0.5 text-text-primary hover:border-accent"
          >
            .cube importieren
          </button>
          {selectedLut ? (
            <>
              <label className="flex items-center gap-1 text-text-secondary">
                Stärke
                <input
                  type="range"
                  min={0}
                  max={1}
                  step={0.05}
                  value={lutStrength}
                  onChange={(e) => setLutStrength(Number(e.target.value))}
                  className="w-20"
                />
                <span className="w-8 text-right text-text-muted">{Math.round(lutStrength * 100)}%</span>
              </label>
              <button
                type="button"
                onClick={() => void applyLutFilterToCurrentVideo(selectedLut, lutStrength)}
                disabled={videoLutBusy}
                className="ml-auto rounded border border-accent bg-accent/10 px-3 py-0.5 font-medium text-accent hover:bg-accent/20 disabled:cursor-not-allowed disabled:opacity-40"
              >
                {videoLutBusy ? "Wende an…" : "Anwenden"}
              </button>
            </>
          ) : null}
        </div>
        {videoLutError ? <p className="text-xs text-red-500">{videoLutError}</p> : null}

        {/* Ähnliche Videos finden (Phase 16 Schritt 10): läuft über den
            ganzen Katalog (nicht nur den aktuellen Ordner), siehe
            `list_similar_video_groups`s Moduldoku. Klick auf ein
            gefundenes Video springt dorthin — auch über Ordnergrenzen
            hinweg (`jumpToVideo`). */}
        <div className="flex flex-wrap items-center gap-2 border-t border-border pt-2 text-xs">
          <button
            type="button"
            onClick={() => void findSimilarVideos(10)}
            disabled={similarVideosLoading}
            className="rounded border border-border px-2 py-0.5 text-text-primary hover:border-accent disabled:opacity-40"
          >
            {similarVideosLoading ? "Suche…" : "Ähnliche Videos finden"}
          </button>
          {similarVideoGroups ? (
            otherSimilarVideos.length > 0 ? (
              <div className="flex flex-wrap items-center gap-1">
                {otherSimilarVideos.map((entry) => (
                  <button
                    key={entry.photo.id}
                    type="button"
                    onClick={() => void jumpToVideo(entry)}
                    className="rounded border border-border px-2 py-0.5 text-text-primary hover:border-accent"
                    title={entry.photo.filename}
                  >
                    {entry.photo.filename}
                  </button>
                ))}
              </div>
            ) : (
              <span className="text-text-muted">keine ähnlichen Videos gefunden</span>
            )
          ) : null}
        </div>
        {similarVideosError ? <p className="text-xs text-red-500">{similarVideosError}</p> : null}
      </div>
    </div>
  );
}
