/**
 * Abbildung einer Spiegelreflexkamera (Geburtstagsausgabe, siehe
 * `DECISIONS.md` ADR-0071) — reines Inline-SVG.
 *
 * **Warum gezeichnet und nicht fotografiert.** Die Netzwerk-Policy
 * dieser Umgebung blockiert jede externe Bildquelle (siehe ADR-0047,
 * derselbe Grund, aus dem der Start-Ladeschirm einen gezeichneten
 * Blendenring statt eines Logo-Bilds hat), und ein mitgeliefertes
 * Produktfoto wäre ein fremdes Lichtbildwerk im Paket.
 *
 * **Bewusst eine Silhouette, kein Markenzeichen.** Gezeichnet ist die
 * Bauform, die jede DSLR teilt — Gehäuse, Prismenbuckel, Objektivtubus,
 * Auslöser, Wählrad. Kein Herstellerlogo, keine Typenbezeichnung auf
 * dem Gehäuse: das Modell steht als Text daneben, wo es hingehört
 * (es ist eine Katalogangabe, keine Beschriftung des Bildes).
 */

interface CameraIllustrationProps {
  /** Kantenlänge in `rem`-Einheiten über Tailwind-Klassen von außen. */
  className?: string;
  /** Für Screenreader — die Abbildung ist sonst rein dekorativ. */
  label?: string;
}

export function CameraIllustration({ className, label }: CameraIllustrationProps) {
  return (
    <svg
      viewBox="0 0 120 84"
      className={className}
      role={label ? "img" : "presentation"}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      data-testid="camera-illustration"
    >
      {/* Der Verlauf gibt dem Gehäuse Rundung, ohne ein zweites
          Element: oben Streiflicht, unten Schatten. */}
      <defs>
        <linearGradient id="apx-cam-body" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="currentColor" stopOpacity="0.42" />
          <stop offset="55%" stopColor="currentColor" stopOpacity="0.26" />
          <stop offset="100%" stopColor="currentColor" stopOpacity="0.16" />
        </linearGradient>
        <radialGradient id="apx-cam-glass" cx="50%" cy="38%" r="62%">
          <stop offset="0%" stopColor="currentColor" stopOpacity="0.5" />
          <stop offset="70%" stopColor="currentColor" stopOpacity="0.14" />
          <stop offset="100%" stopColor="currentColor" stopOpacity="0.3" />
        </radialGradient>
      </defs>

      {/* Prismenbuckel — das Merkmal, an dem man eine Spiegelreflex
          auf einen Blick erkennt. */}
      <path d="M44 22 L52 10 L74 10 L82 22 Z" fill="url(#apx-cam-body)" />
      {/* Blitzschuh darauf. */}
      <rect x="56" y="6" width="14" height="4" rx="1" fill="currentColor" fillOpacity="0.3" />

      {/* Gehäuse. */}
      <rect x="10" y="22" width="100" height="48" rx="7" fill="url(#apx-cam-body)" />
      {/* Handgriff rechts, leicht vorgewölbt. */}
      <path d="M96 22 Q110 22 110 34 L110 58 Q110 70 98 70 L92 70 L92 22 Z" fill="currentColor" fillOpacity="0.2" />

      {/* Objektivtubus: zwei Ringe plus Frontglas. */}
      <circle cx="56" cy="46" r="21" fill="currentColor" fillOpacity="0.22" />
      <circle cx="56" cy="46" r="17" fill="currentColor" fillOpacity="0.16" />
      <circle cx="56" cy="46" r="13" fill="url(#apx-cam-glass)" />
      {/* Das kleine Glanzlicht auf dem Frontglas — ohne es sieht das
          Objektiv wie ein Loch aus, mit ihm wie Glas. */}
      <ellipse cx="50" cy="40" rx="4.5" ry="3" fill="currentColor" fillOpacity="0.45" />

      {/* Auslöser und Moduswählrad. */}
      <rect x="98" y="17" width="9" height="6" rx="3" fill="currentColor" fillOpacity="0.38" />
      <circle cx="24" cy="31" r="6" fill="currentColor" fillOpacity="0.26" />
      <circle cx="24" cy="31" r="2" fill="currentColor" fillOpacity="0.4" />

      {/* Rote Markierung am Objektivring: der Ansetzpunkt beim
          Bajonettwechsel. Ein Detail, aber genau eines, das jeder
          erkennt, der schon ein Objektiv gewechselt hat. */}
      <circle cx="56" cy="24" r="1.8" fill="var(--color-danger)" fillOpacity="0.8" />
    </svg>
  );
}
