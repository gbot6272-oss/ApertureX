import { useEffect, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import {
  pickFilePath,
  type TimelineItemInput,
  type VideoTimelineOptions,
} from "../lib/tauri";
import { selectActivePhotos, useAppStore } from "../store";

interface VideoTimelineDialogProps {
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, photoIds]);

  useEffect(() => {
    if (open && ffmpegAvailable === null) void checkFfmpegAvailability();
  }, [open, ffmpegAvailable, checkFfmpegAvailability]);

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
                  ) : (
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
              <option value="1280x720">1280×720 (HD)</option>
              <option value="1920x1080">1920×1080 (Full HD)</option>
              <option value="3840x2160">3840×2160 (4K)</option>
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
