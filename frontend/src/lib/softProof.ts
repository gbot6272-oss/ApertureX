import type { DevelopFrame } from "../hooks/useDevelopRender";

/**
 * Soft-Proof-Vorschau (Phase 6 Schritt 10, `SPEC.md` §3.4/§7: "Soft-Proof
 * mit Zielprofil, Renderpriorität, Farbumfangswarnung,
 * Papierweiß-Simulation").
 *
 * **Bewusste Vereinfachung** (`DECISIONS.md` ADR-0032 Punkt 6: "kein
 * vollständiges ICC-Farbmanagement-Subsystem"): `apx-pipeline` kennt bis
 * heute nur eine feste Kamera→sRGB-Matrix plus Gammakurve (siehe
 * `crates/apx-pipeline/src/color/mod.rs`), kein echtes ICC-Profil-Laden
 * oder 3D-Gamut-Mapping. Statt dafür ein eigenes Backend-Subsystem zu
 * bauen, ist Soft-Proof hier eine **rein clientseitige Nachbearbeitung**
 * des bereits über die bestehende `develop/...`-Route gerenderten RGBA8-
 * Vorschau-Puffers (derselbe `DevelopFrame` wie der normale Viewer) —
 * betrifft also nie den echten Export/die Datenbank, nur die Anzeige.
 *
 * Die vier Bausteine sind entsprechend Näherungen, keine farbmetrisch
 * korrekten Transformationen:
 * - **Zielprofil**: statt eines geladenen ICC-Profils eine feste,
 *   handgepflegte Liste von drei simulierten Zielen (`SoftProofProfile`),
 *   jedes nur durch einen einzigen "Sättigung Richtung Grauwert"-Faktor
 *   beschrieben — dieselbe Art Vereinfachung wie die kleine eingebaute
 *   Kameraprofil-Liste aus Phase 4 Schritt 7 (kein DCP-Import).
 * - **Renderpriorität**: "Wahrnehmungsorientiert" wendet die
 *   Profilkompression gleichmäßig auf jeden Pixel an; "Relativ
 *   farbmetrisch" lässt Pixel unterhalb eines Sättigungs-Schwellenwerts
 *   unangetastet und komprimiert nur die (per Definition) "aus dem
 *   Zielfarbraum" fallenden — kein echtes 3D-Gamut-Mapping, aber zwei
 *   sichtbar unterschiedliche, in sich konsistente Ergebnisse.
 * - **Farbumfangswarnung**: dieselben als "außerhalb" erkannten Pixel
 *   werden statt komprimiert in einer festen Warnfarbe eingefärbt
 *   (Magenta, wie Lightroom/Photoshop).
 * - **Papierweiß-Simulation**: eine lineare Tonwert-Bereichskompression
 *   von `[0, 255]` auf `[floor, 255-floor]` — eine grobe Annäherung an
 *   ein Ausgabemedium, dessen Weiß nie perfekt 255 und dessen Schwarz nie
 *   perfekt 0 erreicht.
 */

export type SoftProofProfile = "srgb" | "print_sim" | "grayscale_sim";
export type SoftProofIntent = "perceptual" | "relative_colorimetric";

export interface SoftProofSettings {
  profile: SoftProofProfile;
  intent: SoftProofIntent;
  gamutWarning: boolean;
  paperWhite: boolean;
}

export const SOFT_PROOF_PROFILE_LABELS: Record<SoftProofProfile, string> = {
  srgb: "sRGB (unverändert)",
  print_sim: "Druck (simuliert)",
  grayscale_sim: "Graustufen-Druck (simuliert)",
};

export const SOFT_PROOF_INTENT_LABELS: Record<SoftProofIntent, string> = {
  perceptual: "Wahrnehmungsorientiert",
  relative_colorimetric: "Relativ farbmetrisch",
};

/** Wie stark jedes simulierte Zielprofil die Sättigung Richtung Grauwert
 * zurücknimmt (0 = vollständig grau, 1 = unverändert wie sRGB). */
const PROFILE_DESATURATION_FACTOR: Record<SoftProofProfile, number> = {
  srgb: 1,
  print_sim: 0.7,
  grayscale_sim: 0,
};

/** Magenta — dieselbe Warnfarbe, die Lightroom/Photoshop für
 * Farbumfangswarnungen verwenden. */
const GAMUT_WARNING_RGB: readonly [number, number, number] = [255, 0, 255];

/** Jenseits dieser Sättigung (0..1) gilt ein Pixel unter "Relativ
 * farbmetrisch" als außerhalb des simulierten Zielfarbraums. Willkürlich
 * gewählt (kein echtes Gamut-Volumen bekannt), aber niedrig genug, um bei
 * kräftig gesättigten Bildinhalten sichtbare Effekte zu erzeugen. */
const RELATIVE_COLORIMETRIC_SATURATION_THRESHOLD = 0.35;

/** Tonwert-Bodenwert der Papierweiß-Simulation — siehe Moduldoku. */
const PAPER_WHITE_FLOOR = 12;

// Rec.601-Luminanzgewichte — dieselben wie `stages/masks.rs::luminance_range_alpha`.
function luminance(r: number, g: number, b: number): number {
  return 0.299 * r + 0.587 * g + 0.114 * b;
}

function saturationOf(r: number, g: number, b: number): number {
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  if (max === 0) return 0;
  return (max - min) / max;
}

function desaturateTowardsGray(r: number, g: number, b: number, factor: number): [number, number, number] {
  const gray = luminance(r, g, b);
  return [gray + (r - gray) * factor, gray + (g - gray) * factor, gray + (b - gray) * factor];
}

function applyPaperWhite(value: number): number {
  const range = 255 - 2 * PAPER_WHITE_FLOOR;
  return PAPER_WHITE_FLOOR + (value / 255) * range;
}

function clampByte(value: number): number {
  return Math.max(0, Math.min(255, Math.round(value)));
}

/** Wendet die Soft-Proof-Einstellungen auf einen kompletten RGBA8-Frame
 * an und gibt einen **neuen** Puffer zurück — `frame.pixels` bleibt
 * unangetastet (derselbe Frame wird auch ohne Soft-Proof zum Zeichnen
 * verwendet, siehe `Viewer.tsx`). */
export function applySoftProof(frame: DevelopFrame, settings: SoftProofSettings): Uint8Array {
  const src = frame.pixels;
  const out = new Uint8Array(src.length);
  const factor = PROFILE_DESATURATION_FACTOR[settings.profile];

  for (let i = 0; i + 3 < src.length; i += 4) {
    const r = src[i]!;
    const g = src[i + 1]!;
    const b = src[i + 2]!;
    const a = src[i + 3]!;

    const outOfGamut = settings.profile !== "srgb" && saturationOf(r / 255, g / 255, b / 255) > RELATIVE_COLORIMETRIC_SATURATION_THRESHOLD;

    let [pr, pg, pb] =
      settings.intent === "perceptual"
        ? desaturateTowardsGray(r, g, b, factor)
        : outOfGamut
          ? desaturateTowardsGray(r, g, b, factor)
          : ([r, g, b] as [number, number, number]);

    if (settings.gamutWarning && outOfGamut) {
      [pr, pg, pb] = GAMUT_WARNING_RGB;
    }

    if (settings.paperWhite) {
      pr = applyPaperWhite(pr);
      pg = applyPaperWhite(pg);
      pb = applyPaperWhite(pb);
    }

    out[i] = clampByte(pr);
    out[i + 1] = clampByte(pg);
    out[i + 2] = clampByte(pb);
    out[i + 3] = a;
  }

  return out;
}
