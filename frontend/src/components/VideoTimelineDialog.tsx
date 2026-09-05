import { useEffect, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import {
  pickFilePath,
  type TimelineItemInput,
  type TimelinePipOverlayInput,
  type TimelineTextOverlayInput,
  type VideoTimelineOptions,
} from "../lib/tauri";
import { selectActivePhotos, useAppStore } from "../store";

interface VideoTimelineDialogProps {
  open: boolean;
  photoIds: string[];
  onClose: () => void;
}

/** Auflösungs-/Seitenverhältnis-Voreinstellungen inkl. Social-Media-
 * Formaten (Phase 17 Schritt 6, siehe `DECISIONS.md` ADR-0045) — braucht
 * KEINE Backend-Änderung: sowohl Video-Segmente
 * (`render_video_clip_segment`s `scale`+`crop`) als auch Foto-/Titel-
 * Folien (`video::cover_adjust`) skalieren bereits "cover" (wie CSS
 * `object-fit: cover`) auf ein beliebiges `width`×`height` — Hoch-/
 * Quadratformat funktioniert also mit genau denselben Rendering-Pfaden
 * wie die bisherigen 16:9-Auflösungen. */
type Resolution =
  | "1280x720"
  | "1920x1080"
  | "3840x2160"
  | "1080x1920"
  | "1080x1080";
const RESOLUTIONS: Record<
  Resolution,
  { width: number; height: number; label: string }
> = {
  "1280x720": {
    width: 1280,
    height: 720,
    label: "16:9 Querformat — 1280×720 (HD)",
  },
  "1920x1080": {
    width: 1920,
    height: 1080,
    label: "16:9 Querformat — 1920×1080 (Full HD)",
  },
  "3840x2160": {
    width: 3840,
    height: 2160,
    label: "16:9 Querformat — 3840×2160 (4K)",
  },
  "1080x1920": {
    width: 1080,
    height: 1920,
    label: "9:16 Hochformat — 1080×1920 (Reels/TikTok/Stories)",
  },
  "1080x1080": {
    width: 1080,
    height: 1080,
    label: "1:1 Quadratisch — 1080×1080 (Instagram-Feed)",
  },
};

/** Ein Eintrag in der lokalen Bearbeitungsreihenfolge — `photoId`
 * verweist auf ein Foto/Video aus der aktuell aktiven Liste
 * (`selectActivePhotos`). Zeiten in Sekunden statt Millisekunden, für
 * direktere Zahlenfelder in der Oberfläche (Umrechnung erst beim
 * Rendern, siehe `buildTimelineItems`). */
interface DraftItem {
  photoId: string;
  inSeconds: number;
  outSeconds: number;
  holdSeconds: number;
  /** Tempo-Faktor (Phase 17 Schritt 2, siehe `DECISIONS.md` ADR-0045)
   * — nur für Video-Einträge relevant, `1.0` = unverändert. */
  speed: number;
}

/** Auswahl an Voreinstellungen für den Tempo-Regler — deckt die
 * üblichen Zeitlupe-/Zeitraffer-Sprünge ab, ohne einen freien
 * Zahlen-Regler mit Rundungsfallstricken (z. B. `1.0000001`) zu
 * brauchen. */
const SPEED_PRESETS = [0.25, 0.5, 1, 2, 4] as const;

/** Übergangsarten zwischen zwei Einträgen (Phase 17 Schritt 3, siehe
 * `DECISIONS.md` ADR-0045) — Werte entsprechen
 * `apx_app::commands::parse_timeline_transition_kind`s Vertrag. Jeder
 * Übergang zwischen zwei Einträgen kann einzeln gewählt werden, statt
 * einer einzigen Einstellung für die ganze Zeitachse. */
const TRANSITION_OPTIONS: { value: string; label: string }[] = [
  { value: "cut", label: "Schnitt" },
  { value: "fade", label: "Überblendung" },
  { value: "dissolve", label: "Auflösen" },
  { value: "wipe_left", label: "Wischen ←" },
  { value: "wipe_right", label: "Wischen →" },
  { value: "slide_up", label: "Schieben ↑" },
  { value: "slide_down", label: "Schieben ↓" },
  { value: "circle_open", label: "Kreis-Blende" },
];
const DEFAULT_TRANSITION = "fade";

/** Positionswahl für Text-Overlays (Phase 17 Schritt 4) — derselbe
 * Fünf-Positionen-Vertrag wie beim bestehenden Text-Wasserzeichen des
 * Foto-Exports (`ExportDialog.tsx`), statt eines freien x/y-Reglers. */
const OVERLAY_POSITIONS: {
  value: TimelineTextOverlayInput["position"];
  label: string;
}[] = [
  { value: "top_left", label: "Oben links" },
  { value: "top_right", label: "Oben rechts" },
  { value: "center", label: "Mitte" },
  { value: "bottom_left", label: "Unten links" },
  { value: "bottom_right", label: "Unten rechts" },
];

/** Ein Text-Overlay-Entwurf — Zeiten in Sekunden ab Beginn der
 * fertigen Sequenz (nicht relativ zu einem einzelnen Eintrag). Alle
 * Overlays teilen sich eine Schriftdatei (`overlayFontPath` im
 * Dialog-Zustand) — dasselbe bewusste "eine Schriftart genügt"-Muster
 * wie Intro-/Outro-Titelkarten in `SlideshowDialog.tsx`. */
interface DraftOverlay {
  text: string;
  position: TimelineTextOverlayInput["position"];
  startSeconds: number;
  endSeconds: number;
  fontSize: number;
  color: string;
}

/** Ein Bild-in-Bild-/Split-Screen-Overlay-Entwurf (Phase 17 Schritt 7,
 * siehe `DECISIONS.md` ADR-0045) — `photoId` verweist auf ein
 * beliebiges Foto/Video aus der aktuell aktiven Liste, unabhängig von
 * `items` (eine Bild-in-Bild-Quelle muss nicht Teil der Haupt-
 * Zeitachse sein, z. B. eine Webcam-Aufnahme über einem
 * Bildschirmmitschnitt). Video-Quellen spielen bewusst immer ab ihrem
 * eigenen Anfang für die Dauer des Overlay-Fensters (kein zusätzlicher
 * Trim-Regler nur für Bild-in-Bild — bewusste Vereinfachung). */
interface DraftPip {
  photoId: string;
  startSeconds: number;
  endSeconds: number;
  position: TimelinePipOverlayInput["position"];
  scale: number;
}

function hexToRgb(hex: string): [number, number, number] {
  const match = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
  if (!match) return [255, 255, 255];
  // Drei Fanggruppen im Muster oben, also bei einem Treffer immer gefüllt.
  return [
    parseInt(match[1]!, 16),
    parseInt(match[2]!, 16),
    parseInt(match[3]!, 16),
  ];
}

function buildTimelineItems(
  items: DraftItem[],
  isVideo: (id: string) => boolean,
): TimelineItemInput[] {
  return items.map((item) => {
    if (isVideo(item.photoId)) {
      return {
        photoId: item.photoId,
        inMs: Math.round(item.inSeconds * 1000),
        outMs: Math.round(item.outSeconds * 1000),
        speed: item.speed,
      };
    }
    return { photoId: item.photoId, holdSeconds: item.holdSeconds };
  });
}

/** Effektive Länge eines Eintrags in der Ausgabe-Sequenz (Sekunden) —
 * für Video-Einträge der getrimmte Bereich geteilt durch das Tempo,
 * für Foto-/Titel-Einträge die Haltedauer. */
function draftItemDurationSeconds(item: DraftItem, isVideo: boolean): number {
  if (isVideo)
    return (
      Math.max(0, item.outSeconds - item.inSeconds) / Math.max(0.01, item.speed)
    );
  return item.holdSeconds;
}

/** Überblendungsdauer einer Lücke — `"cut"` läuft serverseitig über eine
 * für das Auge nicht wahrnehmbare Dauer (siehe `apx_export::timeline`s
 * `CUT_TRANSITION_SECONDS`), hier zur Offset-Berechnung als `0` genähert. */
function gapDurationSeconds(
  transition: string,
  transitionSeconds: number,
): number {
  return transition === "cut" ? 0 : Math.max(0, transitionSeconds);
}

/** Wo `items[targetIndex]` in der fertig verketteten Sequenz beginnt —
 * dieselbe kumulative Überlappungs-Rechnung wie
 * `apx_export::timeline::xfade_offsets`/`total_duration_after_xfade`
 * (hier in TS dupliziert, weil die Zeitachse erst nach dem Rendern
 * bekannt wäre — Untertitel-Overlays brauchen die Startposition aber
 * schon beim Anlegen im Dialog). */
function itemStartOffsetSeconds(
  items: DraftItem[],
  isVideo: (id: string) => boolean,
  gapTransitions: string[],
  transitionSeconds: number,
  targetIndex: number,
): number {
  let cumulative = 0;
  for (let i = 0; i < targetIndex; i++) {
    const gap = gapDurationSeconds(
      gapTransitions[i] ?? DEFAULT_TRANSITION,
      transitionSeconds,
    );
    cumulative +=
      draftItemDurationSeconds(items[i]!, isVideo(items[i]!.photoId)) - gap;
  }
  return Math.max(0, cumulative);
}

/**
 * Video-Zeitachse (Phase 17 Schritt 1, siehe `PLAN.md`/`DECISIONS.md`
 * ADR-0045) — kombiniert mehrere Fotos/Videos in einer selbst
 * wählbaren Reihenfolge zu einem neuen Video. Anders als die Diashow
 * (`SlideshowDialog.tsx`, reine Fotoserie) können hier echte
 * Videoclips mit eigenen In-/Out-Punkten mitgemischt werden — jeder
 * Eintrag wird serverseitig zu einem eigenen Segment gerendert und
 * dann per Übergang verkettet (siehe `apx_export::timeline`s
 * Moduldoku). Kein bearbeitbares/wiederöffenbares Projekt — wie bei
 * der Diashow wird bei jedem Öffnen neu aus der aktuellen Auswahl
 * zusammengestellt und in einem Rutsch exportiert.
 */
export function VideoTimelineDialog({
  open,
  photoIds,
  onClose,
}: VideoTimelineDialogProps) {
  const ffmpegAvailable = useAppStore((s) => s.ffmpegAvailable);
  const checkFfmpegAvailability = useAppStore((s) => s.checkFfmpegAvailability);
  const videoTimelineRunning = useAppStore((s) => s.videoTimelineRunning);
  const videoTimelineError = useAppStore((s) => s.videoTimelineError);
  const videoTimelineOutcome = useAppStore((s) => s.videoTimelineOutcome);
  const renderVideoTimeline = useAppStore((s) => s.renderVideoTimeline);
  const activePhotos = useAppStore(useShallow(selectActivePhotos));
  const aiSettings = useAppStore((s) => s.aiSettings);
  const loadAiSettings = useAppStore((s) => s.loadAiSettings);
  const whisperModelDownloading = useAppStore((s) => s.whisperModelDownloading);
  const downloadWhisperModel = useAppStore((s) => s.downloadWhisperModel);
  const clearWhisperModelPath = useAppStore((s) => s.clearWhisperModelPath);
  const videoTranscribing = useAppStore((s) => s.videoTranscribing);
  const videoTranscribeError = useAppStore((s) => s.videoTranscribeError);
  const transcribeVideoAudio = useAppStore((s) => s.transcribeVideoAudio);

  const photoById = useMemo(
    () => new Map(activePhotos.map((p) => [p.id, p])),
    [activePhotos],
  );
  const isVideo = (id: string) => photoById.get(id)?.media_kind === "video";

  const [items, setItems] = useState<DraftItem[]>([]);
  /** Ein Übergang je Lücke zwischen zwei Einträgen (`items.length - 1`
   * Stück) — kann kürzer als nötig sein (z. B. direkt nach dem
   * Entfernen eines Eintrags), fehlende Lücken fallen beim Rendern auf
   * `DEFAULT_TRANSITION` zurück (siehe `handleRender`), statt die
   * Zuordnung bei jeder Listenänderung neu zu berechnen. */
  const [gapTransitions, setGapTransitions] = useState<string[]>([]);
  const [transitionSeconds, setTransitionSeconds] = useState(1);
  const [musicPath, setMusicPath] = useState("");
  const [resolution, setResolution] = useState<Resolution>("1920x1080");
  const [fps, setFps] = useState(30);
  const [overlays, setOverlays] = useState<DraftOverlay[]>([]);
  const [overlayFontPath, setOverlayFontPath] = useState("");
  const [pips, setPips] = useState<DraftPip[]>([]);

  useEffect(() => {
    if (!open) return;
    setItems(
      photoIds.map((id) => {
        const photo = photoById.get(id);
        const durationSeconds = photo?.duration_ms
          ? photo.duration_ms / 1000
          : 5;
        return {
          photoId: id,
          inSeconds: 0,
          outSeconds: durationSeconds,
          holdSeconds: 3,
          speed: 1,
        };
      }),
    );
    setGapTransitions(
      new Array(Math.max(0, photoIds.length - 1)).fill(DEFAULT_TRANSITION),
    );
    setOverlays([]);
    setPips([]);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, photoIds]);

  useEffect(() => {
    if (open && ffmpegAvailable === null) void checkFfmpegAvailability();
  }, [open, ffmpegAvailable, checkFfmpegAvailability]);

  useEffect(() => {
    if (open && aiSettings === null) void loadAiSettings();
  }, [open, aiSettings, loadAiSettings]);

  if (!open) return null;

  function updateItem(index: number, patch: Partial<DraftItem>) {
    setItems((prev) =>
      prev.map((item, i) => (i === index ? { ...item, ...patch } : item)),
    );
  }

  function removeItem(index: number) {
    setItems((prev) => prev.filter((_, i) => i !== index));
    // Die Lücke unmittelbar vor dem entfernten Eintrag entfällt mit
    // ihm (bei Eintrag 0 gibt es keine vorherige Lücke).
    setGapTransitions((prev) =>
      prev.filter((_, i) => i !== Math.max(0, index - 1)),
    );
  }

  function setGapTransition(gapIndex: number, value: string) {
    setGapTransitions((prev) => {
      const next = [...prev];
      while (next.length <= gapIndex) next.push(DEFAULT_TRANSITION);
      next[gapIndex] = value;
      return next;
    });
  }

  function moveItem(index: number, direction: -1 | 1) {
    setItems((prev) => {
      const target = index + direction;
      if (target < 0 || target >= prev.length) return prev;
      const next = [...prev];
      const [moved] = next.splice(index, 1);
      if (!moved) return prev;
      next.splice(target, 0, moved);
      return next;
    });
  }

  async function handlePickMusic() {
    const path = await pickFilePath("Audio", [
      "mp3",
      "wav",
      "ogg",
      "flac",
      "m4a",
      "aac",
    ]);
    if (path) setMusicPath(path);
  }

  async function handlePickOverlayFont() {
    const path = await pickFilePath("Schriftdatei", ["ttf", "otf"]);
    if (path) setOverlayFontPath(path);
  }

  /** Transkribiert die Tonspur von `items[index]` (muss ein Video sein)
   * und übernimmt jeden erkannten Abschnitt als neues Text-Overlay,
   * verschoben um die Startposition des Eintrags in der Gesamt-Sequenz
   * (siehe `itemStartOffsetSeconds`). */
  async function handleTranscribeItem(index: number) {
    const item = items[index];
    if (!item) return;
    const segments = await transcribeVideoAudio(item.photoId);
    if (segments.length === 0) return;
    const offset = itemStartOffsetSeconds(
      items,
      isVideo,
      gapTransitions,
      transitionSeconds,
      index,
    );
    setOverlays((prev) => [
      ...prev,
      ...segments.map((segment) => ({
        text: segment.text,
        position: "bottom_left" as const,
        startSeconds: offset + segment.start_ms / 1000,
        endSeconds: offset + segment.end_ms / 1000,
        fontSize: 36,
        color: "#ffffff",
      })),
    ]);
  }

  function addOverlay() {
    setOverlays((prev) => [
      ...prev,
      {
        text: "",
        position: "bottom_left",
        startSeconds: 0,
        endSeconds: 3,
        fontSize: 48,
        color: "#ffffff",
      },
    ]);
  }

  function updateOverlay(index: number, patch: Partial<DraftOverlay>) {
    setOverlays((prev) =>
      prev.map((overlay, i) =>
        i === index ? { ...overlay, ...patch } : overlay,
      ),
    );
  }

  function removeOverlay(index: number) {
    setOverlays((prev) => prev.filter((_, i) => i !== index));
  }

  function addPip() {
    const firstPhotoId = activePhotos[0]?.id ?? "";
    setPips((prev) => [
      ...prev,
      {
        photoId: firstPhotoId,
        startSeconds: 0,
        endSeconds: 3,
        position: "top_right",
        scale: 0.3,
      },
    ]);
  }

  function updatePip(index: number, patch: Partial<DraftPip>) {
    setPips((prev) =>
      prev.map((pip, i) => (i === index ? { ...pip, ...patch } : pip)),
    );
  }

  function removePip(index: number) {
    setPips((prev) => prev.filter((_, i) => i !== index));
  }

  async function handleRender() {
    if (items.length === 0) return;
    const { width, height } = RESOLUTIONS[resolution];
    const timelineItems = buildTimelineItems(items, isVideo);
    const gapCount = Math.max(0, timelineItems.length - 1);
    const options: VideoTimelineOptions = {
      width,
      height,
      fps,
      transitions: Array.from(
        { length: gapCount },
        (_, i) => gapTransitions[i] ?? DEFAULT_TRANSITION,
      ),
      transitionSeconds,
      musicPath: musicPath || undefined,
      textOverlays:
        overlays.length > 0 && overlayFontPath
          ? overlays.map((overlay) => ({
              text: overlay.text,
              position: overlay.position,
              startSeconds: overlay.startSeconds,
              endSeconds: overlay.endSeconds,
              fontPath: overlayFontPath,
              fontSize: overlay.fontSize,
              colorRgb: hexToRgb(overlay.color),
            }))
          : undefined,
      pipOverlays:
        pips.length > 0
          ? pips
              .filter((pip) => pip.photoId)
              .map((pip) => {
                const windowSeconds = Math.max(
                  0.1,
                  pip.endSeconds - pip.startSeconds,
                );
                const base: TimelinePipOverlayInput = {
                  photoId: pip.photoId,
                  startSeconds: pip.startSeconds,
                  endSeconds: pip.endSeconds,
                  position: pip.position,
                  scale: pip.scale,
                };
                return isVideo(pip.photoId)
                  ? {
                      ...base,
                      inMs: 0,
                      outMs: Math.round(windowSeconds * 1000),
                    }
                  : { ...base, holdSeconds: windowSeconds };
              })
          : undefined,
    };
    await renderVideoTimeline(timelineItems, options);
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-16"
      onClick={onClose}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="max-h-[85vh] w-full max-w-lg overflow-y-auto rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
      >
        <h2 className="mb-1 text-sm font-semibold text-text-primary">
          Video-Zeitachse
        </h2>
        <p className="mb-3 text-xs text-text-muted">
          {items.length} Eintrag{items.length === 1 ? "" : "e"} — Reihenfolge,
          Zuschnitt und Übergänge festlegen und als neues Video rendern.
        </p>

        <div className="mb-3 flex flex-col gap-2">
          {items.map((item, index) => {
            const photo = photoById.get(item.photoId);
            const video = isVideo(item.photoId);
            return (
              <div key={`${item.photoId}-${index}`}>
                <div className="rounded border border-border p-2">
                  <div className="mb-1 flex items-center justify-between gap-2">
                    <span
                      className="min-w-0 flex-1 truncate text-xs text-text-primary"
                      title={photo?.filename}
                    >
                      {index + 1}. {photo?.filename ?? item.photoId}{" "}
                      {video ? "(Video)" : "(Foto)"}
                    </span>
                    <div className="flex shrink-0 gap-1">
                      <button
                        type="button"
                        onClick={() => moveItem(index, -1)}
                        disabled={index === 0}
                        className="rounded border border-border px-1.5 py-0.5 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
                      >
                        ↑
                      </button>
                      <button
                        type="button"
                        onClick={() => moveItem(index, 1)}
                        disabled={index === items.length - 1}
                        className="rounded border border-border px-1.5 py-0.5 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
                      >
                        ↓
                      </button>
                      <button
                        type="button"
                        onClick={() => removeItem(index)}
                        className="rounded border border-border px-1.5 py-0.5 text-xs text-danger hover:border-danger"
                      >
                        Entfernen
                      </button>
                    </div>
                  </div>
                  {video ? (
                    <div className="flex gap-2">
                      <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
                        Start (s)
                        <input
                          type="number"
                          min={0}
                          step={0.1}
                          value={item.inSeconds}
                          onChange={(e) =>
                            updateItem(index, {
                              inSeconds: Number(e.target.value),
                            })
                          }
                          className="rounded border border-border bg-bg-panel px-2 py-1"
                        />
                      </label>
                      <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
                        Ende (s)
                        <input
                          type="number"
                          min={0}
                          step={0.1}
                          value={item.outSeconds}
                          onChange={(e) =>
                            updateItem(index, {
                              outSeconds: Number(e.target.value),
                            })
                          }
                          className="rounded border border-border bg-bg-panel px-2 py-1"
                        />
                      </label>
                      <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
                        Tempo
                        <select
                          value={item.speed}
                          onChange={(e) =>
                            updateItem(index, { speed: Number(e.target.value) })
                          }
                          className="rounded border border-border bg-bg-panel px-2 py-1"
                        >
                          {SPEED_PRESETS.map((speed) => (
                            <option key={speed} value={speed}>
                              {speed === 1 ? "Normal" : `${speed}×`}
                            </option>
                          ))}
                        </select>
                      </label>
                    </div>
                  ) : null}
                  {video && aiSettings?.whisper_model_path && (
                    <button
                      type="button"
                      onClick={() => void handleTranscribeItem(index)}
                      disabled={videoTranscribing}
                      className="mt-2 rounded border border-border px-2 py-0.5 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      {videoTranscribing
                        ? "Transkribiert…"
                        : "🎙️ Untertitel automatisch generieren"}
                    </button>
                  )}
                  {!video && (
                    <label className="flex flex-col gap-1 text-xs text-text-secondary">
                      Haltedauer (s)
                      <input
                        type="number"
                        min={0.5}
                        step={0.5}
                        value={item.holdSeconds}
                        onChange={(e) =>
                          updateItem(index, {
                            holdSeconds: Number(e.target.value),
                          })
                        }
                        className="w-24 rounded border border-border bg-bg-panel px-2 py-1"
                      />
                    </label>
                  )}
                </div>
                {index < items.length - 1 && (
                  <div className="my-1 flex items-center gap-2 pl-2 text-xs text-text-muted">
                    <span>↓ Übergang</span>
                    <select
                      value={gapTransitions[index] ?? DEFAULT_TRANSITION}
                      onChange={(e) => setGapTransition(index, e.target.value)}
                      className="rounded border border-border bg-bg-panel px-2 py-0.5 text-xs"
                    >
                      {TRANSITION_OPTIONS.map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label}
                        </option>
                      ))}
                    </select>
                  </div>
                )}
              </div>
            );
          })}
          {items.length === 0 && (
            <p className="text-xs text-text-muted">
              Keine Einträge — Dialog schließen und Fotos/Videos auswählen.
            </p>
          )}
        </div>

        <div className="mb-3 rounded border border-border p-2">
          <div className="mb-2 flex items-center justify-between">
            <span className="text-xs font-semibold text-text-secondary">
              Text-Overlays
            </span>
            <button
              type="button"
              onClick={addOverlay}
              className="rounded border border-border px-2 py-0.5 text-xs hover:border-accent"
            >
              + Hinzufügen
            </button>
          </div>

          <p className="mb-2 rounded border border-border px-2 py-1 text-xs text-text-secondary">
            {aiSettings === null ? (
              "Lädt…"
            ) : !aiSettings.subtitles_feature_compiled ? (
              "Diese Build wurde ohne automatische Untertitel kompiliert."
            ) : aiSettings.whisper_model_path ? (
              <>
                Untertitel-Modell installiert.{" "}
                <button
                  type="button"
                  onClick={() => void clearWhisperModelPath()}
                  className="text-text-muted underline hover:text-danger"
                >
                  Entfernen
                </button>
              </>
            ) : (
              <>
                Kein Untertitel-Modell installiert — Whisper base.en (MIT, ~142
                MB, lokal, kein Cloud-Aufruf).{" "}
                <button
                  type="button"
                  disabled={whisperModelDownloading}
                  onClick={() => void downloadWhisperModel()}
                  className="text-accent underline disabled:cursor-not-allowed disabled:opacity-40"
                >
                  {whisperModelDownloading ? "Lädt herunter…" : "Herunterladen"}
                </button>
              </>
            )}
          </p>
          {videoTranscribeError && (
            <p className="mb-2 text-xs text-danger">
              Fehler: {videoTranscribeError}
            </p>
          )}

          {overlays.length > 0 && (
            <div className="mb-2 flex flex-col gap-1 text-xs text-text-secondary">
              <span>Schriftdatei (für alle Overlays)</span>
              <div className="flex gap-1">
                <input
                  type="text"
                  readOnly
                  value={overlayFontPath}
                  placeholder="Keine ausgewählt"
                  className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-sm"
                />
                <button
                  type="button"
                  onClick={() => void handlePickOverlayFont()}
                  className="shrink-0 rounded border border-border px-2 py-1 text-xs hover:border-accent"
                >
                  Datei wählen…
                </button>
              </div>
              {!overlayFontPath && (
                <span className="text-danger">
                  Ohne Schriftdatei werden Overlays beim Rendern ignoriert.
                </span>
              )}
            </div>
          )}
          <div className="flex flex-col gap-2">
            {overlays.map((overlay, index) => (
              <div key={index} className="rounded border border-border p-2">
                <div className="mb-1 flex items-center gap-2">
                  <input
                    type="text"
                    placeholder="Text"
                    value={overlay.text}
                    onChange={(e) =>
                      updateOverlay(index, { text: e.target.value })
                    }
                    className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-sm"
                  />
                  <button
                    type="button"
                    onClick={() => removeOverlay(index)}
                    className="shrink-0 rounded border border-border px-1.5 py-0.5 text-xs text-danger hover:border-danger"
                  >
                    Entfernen
                  </button>
                </div>
                <div className="flex flex-wrap gap-2">
                  <label className="flex flex-col gap-1 text-xs text-text-secondary">
                    Start (s)
                    <input
                      type="number"
                      min={0}
                      step={0.1}
                      value={overlay.startSeconds}
                      onChange={(e) =>
                        updateOverlay(index, {
                          startSeconds: Number(e.target.value),
                        })
                      }
                      className="w-20 rounded border border-border bg-bg-panel px-2 py-1"
                    />
                  </label>
                  <label className="flex flex-col gap-1 text-xs text-text-secondary">
                    Ende (s)
                    <input
                      type="number"
                      min={0}
                      step={0.1}
                      value={overlay.endSeconds}
                      onChange={(e) =>
                        updateOverlay(index, {
                          endSeconds: Number(e.target.value),
                        })
                      }
                      className="w-20 rounded border border-border bg-bg-panel px-2 py-1"
                    />
                  </label>
                  <label className="flex flex-col gap-1 text-xs text-text-secondary">
                    Position
                    <select
                      value={overlay.position}
                      onChange={(e) =>
                        updateOverlay(index, {
                          position: e.target
                            .value as TimelineTextOverlayInput["position"],
                        })
                      }
                      className="rounded border border-border bg-bg-panel px-2 py-1"
                    >
                      {OVERLAY_POSITIONS.map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="flex flex-col gap-1 text-xs text-text-secondary">
                    Größe
                    <input
                      type="number"
                      min={8}
                      step={2}
                      value={overlay.fontSize}
                      onChange={(e) =>
                        updateOverlay(index, {
                          fontSize: Number(e.target.value),
                        })
                      }
                      className="w-16 rounded border border-border bg-bg-panel px-2 py-1"
                    />
                  </label>
                  <label className="flex flex-col gap-1 text-xs text-text-secondary">
                    Farbe
                    <input
                      type="color"
                      value={overlay.color}
                      onChange={(e) =>
                        updateOverlay(index, { color: e.target.value })
                      }
                    />
                  </label>
                </div>
              </div>
            ))}
            {overlays.length === 0 && (
              <p className="text-xs text-text-muted">Kein Overlay.</p>
            )}
          </div>
        </div>

        <div className="mb-3 rounded border border-border p-2">
          <div className="mb-2 flex items-center justify-between">
            <span className="text-xs font-semibold text-text-secondary">
              Bild-in-Bild / Split-Screen
            </span>
            <button
              type="button"
              onClick={addPip}
              disabled={activePhotos.length === 0}
              className="rounded border border-border px-2 py-0.5 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
            >
              + Hinzufügen
            </button>
          </div>
          <p className="mb-2 text-xs text-text-muted">
            Eine zweite Quelle klein über der Zeitachse einblenden — für echtes
            Split-Screen zwei Einblendungen mit je 50% Größe an
            gegenüberliegenden Positionen anlegen.
          </p>
          <div className="flex flex-col gap-2">
            {pips.map((pip, index) => (
              <div key={index} className="rounded border border-border p-2">
                <div className="mb-1 flex items-center gap-2">
                  <select
                    value={pip.photoId}
                    onChange={(e) =>
                      updatePip(index, { photoId: e.target.value })
                    }
                    className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-sm"
                  >
                    {activePhotos.map((photo) => (
                      <option key={photo.id} value={photo.id}>
                        {photo.filename}{" "}
                        {photo.media_kind === "video" ? "(Video)" : "(Foto)"}
                      </option>
                    ))}
                  </select>
                  <button
                    type="button"
                    onClick={() => removePip(index)}
                    className="shrink-0 rounded border border-border px-1.5 py-0.5 text-xs text-danger hover:border-danger"
                  >
                    Entfernen
                  </button>
                </div>
                <div className="flex flex-wrap gap-2">
                  <label className="flex flex-col gap-1 text-xs text-text-secondary">
                    Start (s)
                    <input
                      type="number"
                      min={0}
                      step={0.1}
                      value={pip.startSeconds}
                      onChange={(e) =>
                        updatePip(index, {
                          startSeconds: Number(e.target.value),
                        })
                      }
                      className="w-20 rounded border border-border bg-bg-panel px-2 py-1"
                    />
                  </label>
                  <label className="flex flex-col gap-1 text-xs text-text-secondary">
                    Ende (s)
                    <input
                      type="number"
                      min={0}
                      step={0.1}
                      value={pip.endSeconds}
                      onChange={(e) =>
                        updatePip(index, { endSeconds: Number(e.target.value) })
                      }
                      className="w-20 rounded border border-border bg-bg-panel px-2 py-1"
                    />
                  </label>
                  <label className="flex flex-col gap-1 text-xs text-text-secondary">
                    Position
                    <select
                      value={pip.position}
                      onChange={(e) =>
                        updatePip(index, {
                          position: e.target
                            .value as TimelinePipOverlayInput["position"],
                        })
                      }
                      className="rounded border border-border bg-bg-panel px-2 py-1"
                    >
                      {OVERLAY_POSITIONS.map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="flex flex-col gap-1 text-xs text-text-secondary">
                    Größe
                    <select
                      value={pip.scale}
                      onChange={(e) =>
                        updatePip(index, { scale: Number(e.target.value) })
                      }
                      className="rounded border border-border bg-bg-panel px-2 py-1"
                    >
                      <option value={0.2}>20%</option>
                      <option value={0.3}>30%</option>
                      <option value={0.5}>50% (Split-Screen)</option>
                    </select>
                  </label>
                </div>
              </div>
            ))}
            {pips.length === 0 && (
              <p className="text-xs text-text-muted">Keine Einblendung.</p>
            )}
          </div>
        </div>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          Überblendungsdauer (s) — gilt für jeden nicht-Schnitt-Übergang oben
          <input
            type="number"
            min={0.1}
            step={0.1}
            value={transitionSeconds}
            onChange={(e) => setTransitionSeconds(Number(e.target.value))}
            className="rounded border border-border bg-bg-panel px-2 py-1"
          />
        </label>

        <div className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          <span>Musik (optional)</span>
          <div className="flex gap-1">
            <input
              type="text"
              readOnly
              value={musicPath}
              placeholder="Keine ausgewählt"
              className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-sm"
            />
            <button
              type="button"
              onClick={() => void handlePickMusic()}
              className="shrink-0 rounded border border-border px-2 py-1 text-xs hover:border-accent"
            >
              Datei wählen…
            </button>
          </div>
        </div>

        <div className="mb-3 flex gap-2">
          <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
            Auflösung
            <select
              value={resolution}
              onChange={(e) => setResolution(e.target.value as Resolution)}
              className="rounded border border-border bg-bg-panel px-2 py-1 text-sm"
            >
              {(Object.keys(RESOLUTIONS) as Resolution[]).map((key) => (
                <option key={key} value={key}>
                  {RESOLUTIONS[key].label}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
            Bildrate
            <select
              value={fps}
              onChange={(e) => setFps(Number(e.target.value))}
              className="rounded border border-border bg-bg-panel px-2 py-1 text-sm"
            >
              <option value={25}>25</option>
              <option value={30}>30</option>
              <option value={60}>60</option>
            </select>
          </label>
        </div>

        {ffmpegAvailable === false && (
          <p className="mb-2 text-xs text-text-muted">
            ffmpeg wurde nicht gefunden — Video-Export ist deaktiviert.
          </p>
        )}
        {videoTimelineError && (
          <p className="mb-2 text-xs text-danger">
            Fehler: {videoTimelineError}
          </p>
        )}
        {!videoTimelineRunning && videoTimelineOutcome && (
          <p className="mb-2 text-xs text-text-secondary">
            Gespeichert als „{videoTimelineOutcome.filename}“.
          </p>
        )}

        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded border border-border px-3 py-1 text-xs hover:border-accent"
          >
            Schließen
          </button>
          <button
            type="button"
            onClick={() => void handleRender()}
            disabled={
              items.length === 0 ||
              videoTimelineRunning ||
              ffmpegAvailable !== true
            }
            className="rounded border border-accent bg-accent/10 px-3 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
          >
            {videoTimelineRunning ? "Rendert…" : "Zeitachse rendern"}
          </button>
        </div>
      </div>
    </div>
  );
}
