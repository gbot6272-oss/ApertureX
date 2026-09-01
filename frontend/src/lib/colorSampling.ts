/**
 * Gemeinsame Bildpunkt-Farbauswertung für die beiden Bild-Klick-Werkzeuge
 * im Entwickeln-Panel — die Weißabgleich-Pipette (`whiteBalancePicker.ts`)
 * und der Farbmischer (`DevelopPanel.tsx`s „Region hinzufügen") teilen
 * sich den Viewer-Klick-Sampling-Code (siehe `Viewer.tsx`); diese Datei
 * hält den Teil, der beiden fehlt: aus einem RGBA8-Bytewert-Tripel den
 * Farbton in Grad zu bestimmen, den der Farbmischer für eine neu
 * aufgenommene Region braucht.
 *
 * Spiegelt `crates/apx-pipeline/src/stages/hsl_color_mixer.rs`s
 * `rgb_to_hsl`-Farbton-Berechnung (nur den `h`-Teil, `s`/`l` werden hier
 * nicht gebraucht).
 */
export function hueDegreesFromRgbByte(r: number, g: number, b: number): number {
  const rf = r / 255;
  const gf = g / 255;
  const bf = b / 255;
  const max = Math.max(rf, gf, bf);
  const min = Math.min(rf, gf, bf);
  const d = max - min;
  if (d < 1e-6) return 0;

  let h: number;
  if (max === rf) {
    h = ((gf - bf) / d) % 6;
    if (h < 0) h += 6;
  } else if (max === gf) {
    h = (bf - rf) / d + 2;
  } else {
    h = (rf - gf) / d + 4;
  }
  return h * 60;
}
