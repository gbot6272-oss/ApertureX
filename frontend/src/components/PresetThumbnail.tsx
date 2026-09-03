import { useEffect, useRef, useState } from "react";

import { useDevelopPreviewThumbnail } from "../hooks/useDevelopRender";
import { buildEdlEnvelopeJson, type EdlPayload } from "../lib/edl";
import { applyRulesToSubset, mergeEdlSubset, parseEdlSubset, parseRules, type PresetConditionPhotoMeta } from "../lib/presets";
import { latestPresetVersion } from "../lib/tauri";

const THUMBNAIL_MAX_EDGE = 64;

/**
 * Kleine Vorschau eines Presets, angewendet auf das aktuell geöffnete
 * Foto (`SPEC.md` §3.5: „Live-Vorschau … als Thumbnail des aktuellen
 * Bildes in der Preset-Liste"). Lädt die aktuellste Version einmal beim
 * Anzeigen (nicht reaktiv auf Preset-Änderungen — ein neu gespeichertes
 * Preset erscheint erst nach einem erneuten Mount, z. B. Ordnerwechsel),
 * rendert dann über [`useDevelopPreviewThumbnail`] bei niedriger
 * Auflösung, unabhängig vom Haupt-Viewer-Rendering.
 *
 * Wertet — wie `applyPreset`/die Hover-Vorschau — die Bedingungen des
 * Presets gegen `photoMeta` aus (`lib/presets.ts`s
 * `applyRulesToSubset`): eine fürs ganze Preset fehlgeschlagene
 * Regelgruppe zeigt das unveränderte `currentEdl` (keine Vorschau-Wirkung),
 * eine sektionsbezogene Regelgruppe blendet nur diese Sektion aus.
 */
export function PresetThumbnail({
  presetId,
  presetName,
  currentEdl,
  photoId,
  conditionsJson,
  photoMeta,
}: {
  presetId: string;
  presetName: string;
  currentEdl: EdlPayload;
  photoId: string | null;
  conditionsJson: string;
  photoMeta: PresetConditionPhotoMeta | null;
}) {
  const [edlJson, setEdlJson] = useState<string | null>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const version = await latestPresetVersion(presetId);
        if (cancelled) return;
        const rawSubset = parseEdlSubset(version.edl_subset_json);
        const rules = parseRules(conditionsJson);
        const subset = applyRulesToSubset(rawSubset, rules, photoMeta) ?? {};
        const merged = mergeEdlSubset(currentEdl, subset);
        setEdlJson(buildEdlEnvelopeJson(merged));
      } catch {
        // Kein Thumbnail statt eines Absturzes — z. B. wenn das Preset
        // zwischenzeitlich gelöscht wurde.
        setEdlJson(null);
      }
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- lädt bewusst nur einmal je presetId, siehe Moduldoku oben (currentEdl-/photoMeta-Änderungen sollen kein erneutes Nachladen der Version auslösen).
  }, [presetId]);

  const frame = useDevelopPreviewThumbnail(photoId, edlJson, THUMBNAIL_MAX_EDGE);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !frame) return;
    canvas.width = frame.width;
    canvas.height = frame.height;
    const ctx = canvas.getContext("2d");
    ctx?.putImageData(new ImageData(new Uint8ClampedArray(frame.pixels), frame.width, frame.height), 0, 0);
  }, [frame]);

  if (!photoId) return null;

  return (
    <canvas
      ref={canvasRef}
      aria-label={`Vorschau: ${presetName}`}
      className="h-8 w-8 shrink-0 rounded border border-border object-cover"
    />
  );
}
