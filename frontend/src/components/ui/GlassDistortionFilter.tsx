/**
 * Einmalig im DOM gehaltene SVG-`<filter>`-Definition für die echte
 * "Liquid Glass"-Verzerrung (Phase 25 Nachtrag, siehe `DECISIONS.md`,
 * aktuelles ADR) — adaptiert aus dem vom Nutzer bereitgestellten
 * Referenz-Prompt (siehe `PROMPTS.md`, Prompt 1s `GlassFilter`), auf
 * gedämpftere Werte reduziert: `.apx-glass`s `backdrop-filter` allein
 * (nur `blur`+`saturate`) liefert einen sauberen, aber optisch flachen
 * Weichzeichner — echtes "Glas" braucht zusätzlich eine leichte
 * `feDisplacementMap`-Verzerrung (bricht Kanten/Licht wie eine reale
 * Glasoberfläche), hier bewusst schwächer als im Referenz-Prompt
 * (`baseFrequency`/`scale` reduziert), da die App-Chrome (Kopfzeile,
 * Paletten, Dialoge) dauerhaft sichtbar ist, nicht nur ein einzelnes
 * Marketing-Element — zu starke Verzerrung würde Text in der
 * Kopfzeile selbst schwer lesbar machen. `<svg>` bleibt unsichtbar
 * (keine eigene Breite/Höhe, `position: absolute` außerhalb des
 * normalen Layoutflusses) — nur als Verweisziel für `filter:
 * var(--glass-distortion)` in `index.css` gedacht, ein einziges
 * Exemplar genügt für beliebig viele `.apx-glass`-Flächen gleichzeitig
 * (dieselbe `<filter id>` wird per URL-Referenz wiederverwendet, keine
 * Vervielfachung der teuren SVG-Filterberechnung pro Aufrufstelle).
 */
export function GlassDistortionFilter() {
  return (
    <svg aria-hidden focusable="false" style={{ position: "absolute", width: 0, height: 0, overflow: "hidden" }}>
      <filter id="apx-glass-distortion" x="-20%" y="-20%" width="140%" height="140%" colorInterpolationFilters="sRGB">
        <feTurbulence type="fractalNoise" baseFrequency="0.008 0.012" numOctaves={1} seed={7} result="turbulence" />
        <feGaussianBlur in="turbulence" stdDeviation={2} result="softMap" />
        <feDisplacementMap in="SourceGraphic" in2="softMap" scale={18} xChannelSelector="R" yChannelSelector="G" />
      </filter>
    </svg>
  );
}
