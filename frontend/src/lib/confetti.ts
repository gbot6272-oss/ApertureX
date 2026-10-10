/**
 * Konfetti-Simulation (Geburtstagsausgabe, siehe `DECISIONS.md`
 * ADR-0071).
 *
 * Reine Rechnung: kein DOM, kein Canvas, kein `requestAnimationFrame`.
 * Das Zeichnen steht in `components/ConfettiLayer.tsx`. Dieselbe
 * Trennung wie bei `lib/histogram.ts`/`HistogramCanvas` und bei den
 * Rust-Modulen dieser App — eine Physik, die man ohne Browser prüfen
 * kann, ist eine Physik, die man prüft.
 *
 * **Warum eine eigene Simulation und keine Bibliothek.** Für Konfetti
 * gibt es fertige npm-Pakete. Sie bringen aber ihr eigenes
 * Canvas-Management, ihre eigene rAF-Schleife und ihre eigene
 * Lizenzzeile mit — für ungefähr sechzig Zeilen Euler-Integration.
 * Dieselbe Linie wie `lib/i18n.ts`s eigene Begründung: keine
 * Abhängigkeit für etwas, das kleiner ist als ihre Anbindung.
 *
 * **Die drei Werte, die es glaubwürdig machen:**
 * - *Schwerkraft* allein ergibt Regen. Konfetti fällt langsamer als
 *   ein Stein, weil Papier Luftwiderstand hat — deshalb ist
 *   [`DRAG`] in jedem Schritt auf die Geschwindigkeit angewandt, und
 *   deshalb erreichen die Partikel eine Endgeschwindigkeit statt
 *   immer schneller zu werden.
 * - *Eigendrehung* macht aus einem fallenden Punkt ein fallendes
 *   Plättchen. Jedes Teilchen dreht sich mit eigener Rate; die
 *   Zeichenschicht staucht seine Breite mit `cos(spin)`, wodurch es
 *   von der Seite kurz verschwindet — der Effekt, der echtes Konfetti
 *   flattern lässt.
 * - *Lebensdauer* begrenzt alles. Ohne sie läuft die Schleife für
 *   immer; mit ihr räumt sich jeder Ausbruch von selbst ab, und
 *   [`stepParticles`] gibt eine leere Liste zurück, an der die
 *   Zeichenschicht erkennt, dass sie die Schleife beenden darf.
 */

/** Pixel pro Sekunde². Positiv = nach unten (Bildschirmkoordinaten). */
const GRAVITY = 1400;

/**
 * Anteil der Geschwindigkeit, der pro Sekunde erhalten bleibt.
 *
 * `0.26` heißt: nach einer Sekunde ohne Antrieb sind 26 % der
 * Geschwindigkeit übrig. Daraus ergibt sich eine Endgeschwindigkeit von
 * ungefähr `GRAVITY / -ln(DRAG)` ≈ 1040 px/s — schnell genug, dass es
 * fällt, langsam genug, dass man die Farben sieht.
 */
const DRAG = 0.26;

/** Höchstzahl gleichzeitig lebender Teilchen, app-weit.
 *
 * Nicht verhandelbar: das hier läuft in einem Fotoeditor, der
 * gleichzeitig RAW-Dateien dekodiert. Jeder Ausbruch wird gegen dieses
 * Budget gekappt (siehe [`spawnBurst`]s `existing`-Parameter), statt
 * sich bei schnellen Klicks aufzuaddieren. 220 Plättchen kosten pro
 * Bild ungefähr 220 `fillRect`-Aufrufe auf einem Compositor-Layer —
 * unterhalb jeder messbaren Auswirkung auf die Bedienung, selbst auf
 * einem schwachen Laptop. */
export const MAX_PARTICLES = 220;

export interface ConfettiParticle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  /** Drehwinkel in Radiant. */
  spin: number;
  /** Drehgeschwindigkeit in Radiant pro Sekunde. */
  spinRate: number;
  /** Halbe Kantenlänge in Pixeln. */
  size: number;
  color: string;
  /** Verbleibende Lebensdauer in Sekunden. */
  life: number;
  /** Lebensdauer beim Entstehen — die Zeichenschicht blendet daran aus. */
  maxLife: number;
}

export interface BurstOptions {
  /** Ursprung in Pixeln (Bildschirmkoordinaten). */
  x: number;
  y: number;
  /** Mittlere Abschussrichtung in Radiant. `-Math.PI / 2` = nach oben. */
  angle: number;
  /** Halber Öffnungswinkel des Kegels in Radiant. */
  spread: number;
  /** Mittlere Anfangsgeschwindigkeit in Pixeln pro Sekunde. */
  velocity: number;
  count: number;
  colors: readonly string[];
  /** Mittlere Lebensdauer in Sekunden. */
  life?: number;
}

/** Zufallszahlenquelle — als Parameter, damit Tests eine feste Folge
 * einsetzen können. Standard ist `Math.random`. */
export type RandomSource = () => number;

function between(random: RandomSource, min: number, max: number): number {
  return min + random() * (max - min);
}

/**
 * Erzeugt einen Ausbruch.
 *
 * `existing` ist die Zahl der bereits lebenden Teilchen: der Ausbruch
 * wird so gekappt, dass [`MAX_PARTICLES`] app-weit nie überschritten
 * wird. Ist das Budget erschöpft, kommt eine leere Liste zurück — ein
 * nicht gezeigter Ausbruch ist allemal besser als eine ruckelnde
 * Oberfläche.
 */
export function spawnBurst(
  options: BurstOptions,
  existing = 0,
  random: RandomSource = Math.random,
): ConfettiParticle[] {
  const budget = Math.max(0, MAX_PARTICLES - existing);
  const count = Math.min(Math.max(0, Math.floor(options.count)), budget);
  if (count === 0 || options.colors.length === 0) return [];

  const meanLife = options.life ?? 2.6;
  const particles: ConfettiParticle[] = [];
  for (let i = 0; i < count; i += 1) {
    const angle = options.angle + between(random, -options.spread, options.spread);
    // Die Wurzel verteilt die Geschwindigkeiten zum oberen Ende hin —
    // ein gleichverteilter Betrag ergibt eine sichtbar leere Mitte im
    // Kegel, weil die langsamen Teilchen alle unten zusammenliegen.
    const speed = options.velocity * Math.sqrt(between(random, 0.25, 1));
    const life = meanLife * between(random, 0.7, 1.25);
    particles.push({
      x: options.x,
      y: options.y,
      vx: Math.cos(angle) * speed,
      vy: Math.sin(angle) * speed,
      spin: between(random, 0, Math.PI * 2),
      spinRate: between(random, -14, 14),
      size: between(random, 3.5, 7.5),
      color: options.colors[Math.floor(random() * options.colors.length)] ?? options.colors[0]!,
      life,
      maxLife: life,
    });
  }
  return particles;
}

/**
 * Ein Zeitschritt für alle Teilchen. Gibt die noch lebenden zurück.
 *
 * `dt` wird auf 50 ms gedeckelt: kommt die Seite aus dem Hintergrund
 * zurück, liefert `requestAnimationFrame` einen Sprung von mehreren
 * Sekunden, und ein ungedeckelter Schritt würde alle Teilchen in einem
 * Bild quer über den Bildschirm schleudern. Lieber eine kurze
 * Zeitlupe als ein Sprung.
 */
export function stepParticles(particles: readonly ConfettiParticle[], dt: number): ConfettiParticle[] {
  const step = Math.min(Math.max(dt, 0), 0.05);
  if (step === 0) return particles.slice();

  // Exponentieller Abfall, bildratenunabhängig: `DRAG^step` statt
  // `DRAG * step`. Nur so fällt Konfetti bei 144 Hz genauso schnell
  // wie bei 60 Hz — ein linearer Faktor pro Bild macht die Physik von
  // der Bildrate abhängig, und das sieht man.
  const damping = Math.pow(DRAG, step);
  const alive: ConfettiParticle[] = [];
  for (const p of particles) {
    const life = p.life - step;
    if (life <= 0) continue;
    const vy = (p.vy + GRAVITY * step) * damping;
    const vx = p.vx * damping;
    alive.push({
      ...p,
      x: p.x + vx * step,
      y: p.y + vy * step,
      vx,
      vy,
      spin: p.spin + p.spinRate * step,
      life,
    });
  }
  return alive;
}

/** Deckkraft eines Teilchens: volle Deckkraft, dann über das letzte
 * Drittel seiner Lebensdauer ausblenden. Ein hartes Verschwinden
 * mitten im Bild sieht nach Fehler aus, nicht nach Konfetti. */
export function particleOpacity(particle: ConfettiParticle): number {
  const fadeStart = particle.maxLife / 3;
  if (particle.life >= fadeStart) return 1;
  return Math.max(0, particle.life / fadeStart);
}

/**
 * Die beiden Kanonen-Ausbrüche einer Begrüßung: unten links und unten
 * rechts, schräg zur Mitte. Zwei gegenläufige Kegel statt eines
 * zentralen Regens — eine Kanone schießt, sie regnet nicht.
 */
export function cannonBursts(
  width: number,
  height: number,
  colors: readonly string[],
): BurstOptions[] {
  const perSide = Math.floor(MAX_PARTICLES / 2);
  return [
    {
      x: width * 0.06,
      y: height * 0.96,
      // 60° über der Waagerechten, zur Mitte hin.
      angle: -Math.PI / 3,
      spread: Math.PI / 9,
      velocity: Math.max(height, 620) * 1.35,
      count: perSide,
      colors,
      life: 3.1,
    },
    {
      x: width * 0.94,
      y: height * 0.96,
      angle: -Math.PI + Math.PI / 3,
      spread: Math.PI / 9,
      velocity: Math.max(height, 620) * 1.35,
      count: perSide,
      colors,
      life: 3.1,
    },
  ];
}

/** Die Deko-Farben aus den CSS-Tokens (`--party-1` … `--party-6`,
 * siehe `index.css`s Geburtstags-Block) — damit es nur eine Palette
 * gibt und nicht eine im CSS und eine zweite, irgendwann abweichende
 * im JavaScript. Fehlt ein Token (anderes Theme aktiv), bleibt ein
 * warmer Satz als Rückfall. */
export function partyColors(root: HTMLElement = document.documentElement): string[] {
  const style = getComputedStyle(root);
  const read = (n: number) => style.getPropertyValue(`--party-${n}`).trim();
  const tokens = [1, 2, 3, 4, 5, 6].map(read).filter((value) => value.length > 0);
  return tokens.length > 0 ? tokens : ["#f0a93c", "#e876a0", "#966cd6", "#6ec2d6", "#8ec98a", "#f2e6c8"];
}
