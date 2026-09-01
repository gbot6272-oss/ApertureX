/**
 * Client-seitige Vorschau für das Umbenennungsmuster-Tokensystem
 * (`crates/apx-app/src/import/rename.rs::render_rename_pattern`) — reine
 * Anzeige-Hilfe für den Token-Editor in `ImportDialog.tsx`, keine
 * Ersatzimplementierung des eigentlichen Imports (der läuft weiterhin
 * ausschließlich im Backend). Bildet dieselben vier Tokens auf dieselbe
 * Weise ab (Datum `JJJJMMTT`, vierstellig nullgepolsterte Laufnummer,
 * bereinigtes Kameramodell, bereinigter Originalname); unbekannte
 * `{…}`-Platzhalter bleiben unverändert stehen, exakt wie im Backend.
 */

export const RENAME_PATTERN_TOKENS: ReadonlyArray<{ token: string; label: string }> = [
  { token: "{date}", label: "Datum (JJJJMMTT)" },
  { token: "{seq}", label: "Laufnummer" },
  { token: "{camera}", label: "Kameramodell" },
  { token: "{original}", label: "Ursprünglicher Name" },
];

export interface RenamePatternSample {
  date: Date;
  seq: number;
  camera: string | null;
  originalStem: string;
}

/** Beispielwerte für die Live-Vorschau — keine echten Fotodaten nötig,
 * das Muster wird nur illustriert. */
export const SAMPLE_RENAME_PATTERN_INPUT: RenamePatternSample = {
  date: new Date(2026, 2, 15),
  seq: 7,
  camera: "Canon EOS R5",
  originalStem: "IMG_0042",
};

function sanitizeForFilename(raw: string): string {
  return raw.replace(/[\\/:*?"<>|]/g, "_");
}

export function previewRenamePattern(pattern: string, sample: RenamePatternSample = SAMPLE_RENAME_PATTERN_INPUT): string {
  const yyyy = sample.date.getFullYear().toString().padStart(4, "0");
  const mm = (sample.date.getMonth() + 1).toString().padStart(2, "0");
  const dd = sample.date.getDate().toString().padStart(2, "0");
  const date = `${yyyy}${mm}${dd}`;
  const camera = sanitizeForFilename(sample.camera ?? "Kamera");
  const seq = sample.seq.toString().padStart(4, "0");
  const original = sanitizeForFilename(sample.originalStem);
  return pattern.replaceAll("{date}", date).replaceAll("{seq}", seq).replaceAll("{camera}", camera).replaceAll("{original}", original);
}
