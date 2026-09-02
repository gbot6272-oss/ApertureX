import type { DevelopFrame } from "../hooks/useDevelopRender";
import type { IccProfileChoice } from "./tauri";

/**
 * Soft-Proof-Vorschau (Phase 6 Schritt 10, `SPEC.md` §3.4/§7: "Soft-Proof
 * mit Zielprofil, Renderpriorität, Farbumfangswarnung,
 * Papierweiß-Simulation").
 *
 * **Phase 12 Schritt 6** (siehe `DECISIONS.md` ADR-0039-Nachtrag II) hat
 * die ursprüngliche, rein clientseitige Sättigungs-Näherung (ADR-0032
 * Punkt 6: "kein vollständiges ICC-Farbmanagementsystem") durch einen
 * **echten** ICC-Soft-Proof ersetzt: `apx_export::icc::soft_proof_rgba8`
 * (`lcms2::Transform::new_proofing` mit `SOFT_PROOFING`/`GAMUT_CHECK`)
 * läuft serverseitig, als zusätzlicher Parameter derselben
 * `develop/...`-Route (`crates/apx-app/src/protocol`), die auch den
 * normalen Vorschau-Frame liefert — `Viewer.tsx` erhält den Puffer also
 * bereits fertig soft-proofed, dieselbe Transformation, die auch
 * `convert_from_srgb` beim Export nutzt (echte Zielprofile, echtes
 * Farbumfangs-Alarm-Flagging über `cmsSetAlarmCodes`, nicht mehr eine
 * handgepflegte "Sättigung Richtung Grau"-Näherung mit drei erfundenen
 * Zielen).
 *
 * **Verbleibende bewusste Vereinfachung — Papierweiß-Simulation:**
 * `lcms2` bietet keine eingebaute "Papierweiß"-Funktion (das ist in
 * echten Bildbearbeitungsprogrammen meist eine zusätzliche, getrennt vom
 * eigentlichen ICC-Proofing gerechnete Tonwert-Kompression). [`applyPaperWhite`]
 * bleibt daher weiterhin eine kleine, ehrlich beschriftete clientseitige
 * Nachbearbeitung — anders als vorher betrifft sie aber nur noch den
 * Tonwertbereich, nicht mehr Farbe/Sättigung/Gamut (das übernimmt jetzt
 * vollständig der echte Server-Transform).
 */

export type SoftProofIntent = "perceptual" | "relative_colorimetric";

/** Wiederverwendet dieselben Zielprofil-Namen wie der Export-Dialog
 * (`apx_export::icc::StandardIccProfile` + eine vom Nutzer gewählte
 * `.icc`-Datei) — derselbe echte `lcms2`-Weg bedient beide Funktionen. */
export type SoftProofProfile = IccProfileChoice;

export const SOFT_PROOF_PROFILE_LABELS: Record<SoftProofProfile, string> = {
  srgb: "sRGB (unverändert)",
  adobe_rgb: "Adobe RGB",
  pro_photo_rgb: "ProPhoto RGB",
  display_p3: "Display P3",
  custom: "Eigene ICC-Datei…",
};

export const SOFT_PROOF_INTENT_LABELS: Record<SoftProofIntent, string> = {
  perceptual: "Wahrnehmungsorientiert",
  relative_colorimetric: "Relativ farbmetrisch",
};

export interface SoftProofSettings {
  profile: SoftProofProfile;
  /** Nur bei `profile === "custom"` gelesen. */
  customIccPath: string;
  intent: SoftProofIntent;
  gamutWarning: boolean;
  paperWhite: boolean;
}

/** Tonwert-Bodenwert der Papierweiß-Simulation — siehe Moduldoku. */
const PAPER_WHITE_FLOOR = 12;

function applyPaperWhiteToChannel(value: number): number {
  const range = 255 - 2 * PAPER_WHITE_FLOOR;
  return Math.max(0, Math.min(255, Math.round(PAPER_WHITE_FLOOR + (value / 255) * range)));
}

/** Wendet ausschließlich die Papierweiß-Tonwertkompression an (Farbe/Gamut
 * sind zu diesem Zeitpunkt bereits durch den echten Server-Soft-Proof
 * korrekt, siehe Moduldoku) und gibt einen **neuen** Puffer zurück. */
export function applyPaperWhite(frame: DevelopFrame): Uint8Array {
  const src = frame.pixels;
  const out = new Uint8Array(src.length);
  for (let i = 0; i + 3 < src.length; i += 4) {
    out[i] = applyPaperWhiteToChannel(src[i]!);
    out[i + 1] = applyPaperWhiteToChannel(src[i + 1]!);
    out[i + 2] = applyPaperWhiteToChannel(src[i + 2]!);
    out[i + 3] = src[i + 3]!;
  }
  return out;
}

/** Base64url-Alphabet (kein `/`, kein Padding) — siehe
 * `crates/apx-app/src/protocol/route.rs`s Moduldoku für die Begründung
 * (ein Dateipfad im `<soft_proof>`-Segment darf die bestehende
 * "an `/` aufteilen"-URL-Struktur nicht durcheinanderbringen). */
function base64UrlEncode(text: string): string {
  const bytes = new TextEncoder().encode(text);
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replaceAll("=", "");
}

/** Baut das `<soft_proof>`-URL-Segment für `developUrl` — `"none"`, wenn
 * kein Soft-Proof aktiv ist, sonst das base64url-kodierte JSON, das
 * `crates/apx-app/src/protocol/route.rs::parse_soft_proof` erwartet. */
export function encodeSoftProofSegment(settings: SoftProofSettings | null): string {
  if (!settings) return "none";
  const payload = {
    target: settings.profile,
    custom_path: settings.profile === "custom" ? settings.customIccPath : null,
    intent: settings.intent,
    gamut_warning: settings.gamutWarning,
  };
  return base64UrlEncode(JSON.stringify(payload));
}
