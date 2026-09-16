/**
 * Geometrie der Kompositionsraster im Zuschnitt-Werkzeug (Phase 32,
 * siehe `DECISIONS.md`).
 *
 * Die Raster gab es seit Phase 4 Schritt 11, aber mit drei
 * Einschränkungen, die der damalige Code in seinen eigenen Kommentaren
 * selbst benannt hat:
 *
 * 1. Die „Spirale" war gar keine — verschachtelte Rechtecke statt der
 *    logarithmischen Kurve. Bei einem Raster, dessen ganzer Sinn die
 *    Kurve ist, hilft das Kästchen-Gerüst allein nicht: man legt die
 *    Spirale an, um zu sehen, ob das Auge dem Schwung zum Motiv folgt.
 * 2. Es gab keine Ausrichtung. Spirale und Dreiecke sitzen fest in
 *    einer Ecke — in jedem ernsthaften Editor blättert man durch vier
 *    Drehungen mal zwei Spiegelungen, weil das Motiv in jeder Ecke
 *    liegen kann. Ohne das ist das Raster in der Hälfte der Fälle
 *    unbrauchbar.
 * 3. Die Diagonalmethode war auf zwei Ecke-zu-Ecke-Linien verkürzt.
 *    Die echte Methode nach Edwin Westhoff zieht VIER Linien: von
 *    jeder Ecke eine 45°-Linie, deren Schnittpunkte die
 *    Platzierungspunkte ergeben.
 *
 * Alle Koordinaten laufen in einem 0..100-Quadrat und werden vom SVG
 * per `preserveAspectRatio="none"` auf das Zuschnitt-Rechteck gezogen.
 */

/** Vier Drehungen mal zwei Spiegelungen — die acht Lagen, die ein
 * Fotograf durchblättert. */
export const GRID_ORIENTATIONS = [0, 1, 2, 3, 4, 5, 6, 7] as const;
export type GridOrientation = (typeof GRID_ORIENTATIONS)[number];

export interface Point {
  x: number;
  y: number;
}

export interface Line {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

/**
 * Dreht/spiegelt einen Punkt im 0..100-Quadrat.
 *
 * Lagen 0–3 sind Drehungen um 90°, Lagen 4–7 dieselben Drehungen mit
 * zusätzlicher Spiegelung an der senkrechten Achse. Gespiegelt wird VOR
 * dem Drehen, damit „einmal weiter" immer dieselbe anschauliche
 * Bewegung ist, statt bei jeder zweiten Lage die Richtung zu wechseln.
 */
export function orientPoint(point: Point, orientation: GridOrientation): Point {
  const mirrored = orientation >= 4 ? { x: 100 - point.x, y: point.y } : point;
  const quarterTurns = orientation % 4;
  let { x, y } = mirrored;
  for (let turn = 0; turn < quarterTurns; turn += 1) {
    const previousX = x;
    x = 100 - y;
    y = previousX;
  }
  return { x, y };
}

export function orientLine(line: Line, orientation: GridOrientation): Line {
  const start = orientPoint({ x: line.x1, y: line.y1 }, orientation);
  const end = orientPoint({ x: line.x2, y: line.y2 }, orientation);
  return { x1: start.x, y1: start.y, x2: end.x, y2: end.y };
}

/** Drittelraster — lageunabhängig, weil punktsymmetrisch. */
export function thirdsLines(): Line[] {
  return [
    { x1: 100 / 3, y1: 0, x2: 100 / 3, y2: 100 },
    { x1: 200 / 3, y1: 0, x2: 200 / 3, y2: 100 },
    { x1: 0, y1: 100 / 3, x2: 100, y2: 100 / 3 },
    { x1: 0, y1: 200 / 3, x2: 100, y2: 200 / 3 },
  ];
}

/** Goldener Schnitt — wie das Drittelraster, nur bei 38,2 / 61,8 %. */
export function goldenRatioLines(): Line[] {
  const a = 38.196601125;
  const b = 100 - a;
  return [
    { x1: a, y1: 0, x2: a, y2: 100 },
    { x1: b, y1: 0, x2: b, y2: 100 },
    { x1: 0, y1: a, x2: 100, y2: a },
    { x1: 0, y1: b, x2: 100, y2: b },
  ];
}

/**
 * Die echte Diagonalmethode: von jeder der vier Ecken eine Linie unter
 * 45°. In einem nicht-quadratischen Bild enden sie an der jeweils
 * gegenüberliegenden Kante, nicht in der gegenüberliegenden Ecke —
 * genau daraus entstehen die vier Schnittpunkte, auf die man Motive
 * legt. Im 0..100-Quadrat sind das die Ecke-zu-Ecke- und die vier
 * Halbdiagonalen.
 */
export function diagonalMethodLines(): Line[] {
  return [
    { x1: 0, y1: 0, x2: 100, y2: 100 },
    { x1: 100, y1: 0, x2: 0, y2: 100 },
    { x1: 0, y1: 0, x2: 50, y2: 100 },
    { x1: 100, y1: 0, x2: 50, y2: 100 },
    { x1: 0, y1: 100, x2: 50, y2: 0 },
    { x1: 100, y1: 100, x2: 50, y2: 0 },
  ];
}

/** Goldenes Dreieck: eine Hauptdiagonale und zwei Lote darauf. */
export function triangleLines(): Line[] {
  return [
    { x1: 0, y1: 0, x2: 100, y2: 100 },
    { x1: 0, y1: 100, x2: 50, y2: 50 },
    { x1: 100, y1: 0, x2: 50, y2: 50 },
  ];
}

/**
 * Die Stützrechtecke der Goldenen Spirale.
 *
 * Konstruktion: vom aktuellen Rechteck wird abwechselnd links, oben,
 * rechts, unten ein Streifen der Breite `1 − 1/φ` abgeschnitten; im
 * Rest geht es weiter. Auf ein Rechteck im Goldenen Schnitt gezogen
 * ist jeder dieser Streifen genau ein Quadrat — das ist die übliche
 * normierte Konstruktion, und weil das Overlay ohnehin mit
 * `preserveAspectRatio="none"` auf den Zuschnitt gezerrt wird, ist sie
 * hier die richtige.
 *
 * Ein erster Entwurf schnitt stattdessen `min(w, h)` als echtes
 * Quadrat ab. In einem quadratischen Koordinatenraum nimmt dieser
 * Schnitt aber gleich das ganze Bild, und der Ausweg darüber hinaus
 * erzeugte Rechtecke AUSSERHALB des Bildes — ein eigener Test hat das
 * sofort aufgedeckt (`x + w` lag bei 161,8 statt höchstens 100).
 */
export function spiralSquares(
  steps = 8,
): Array<{ x: number; y: number; w: number; h: number; corner: number }> {
  const squares: Array<{ x: number; y: number; w: number; h: number; corner: number }> = [];
  const cut = 0.381966011250105; // 1 − 1/φ

  let x = 0;
  let y = 0;
  let w = 100;
  let h = 100;

  for (let step = 0; step < steps; step += 1) {
    if (w <= 0.01 || h <= 0.01) break;
    const corner = step % 4;
    if (corner === 0) {
      const slice = w * cut;
      squares.push({ x, y, w: slice, h, corner });
      x += slice;
      w -= slice;
    } else if (corner === 1) {
      const slice = h * cut;
      squares.push({ x, y, w, h: slice, corner });
      y += slice;
      h -= slice;
    } else if (corner === 2) {
      const slice = w * cut;
      squares.push({ x: x + w - slice, y, w: slice, h, corner });
      w -= slice;
    } else {
      const slice = h * cut;
      squares.push({ x, y: y + h - slice, w, h: slice, corner });
      h -= slice;
    }
  }
  return squares;
}

/**
 * Die Goldene Spirale als SVG-Pfad — echte Viertelbögen statt eines
 * Rechteck-Gerüsts.
 *
 * Je Stützrechteck ein Bogen von Ecke zu Ecke, mit den Rechteckseiten
 * als Radien. In der normierten Fläche ist das ein elliptischer Bogen;
 * auf einen Zuschnitt im Goldenen Schnitt gezogen wird daraus die
 * bekannte Fibonacci-Spirale. Genau diese Kurve fehlte bisher — das
 * Kästchengerüst allein hilft nicht, denn man legt die Spirale an, um
 * zu sehen, ob das Auge dem Schwung zum Motiv folgt.
 */
export function goldenSpiralPath(orientation: GridOrientation, steps = 8): string {
  const squares = spiralSquares(steps);
  const parts: string[] = [];

  squares.forEach((square, index) => {
    // Start- und Endecke des Bogens, je nach Schnittrichtung.
    const byCorner: Record<number, [Point, Point]> = {
      0: [
        { x: square.x + square.w, y: square.y },
        { x: square.x, y: square.y + square.h },
      ],
      1: [
        { x: square.x + square.w, y: square.y + square.h },
        { x: square.x, y: square.y },
      ],
      2: [
        { x: square.x, y: square.y + square.h },
        { x: square.x + square.w, y: square.y },
      ],
      3: [
        { x: square.x, y: square.y },
        { x: square.x + square.w, y: square.y + square.h },
      ],
    };
    const [rawFrom, rawTo] = byCorner[square.corner] ?? byCorner[0]!;
    const from = orientPoint(rawFrom, orientation);
    const to = orientPoint(rawTo, orientation);

    // Radien mitdrehen: bei einer Vierteldrehung tauschen Breite und
    // Höhe die Rollen, sonst wäre der Bogen in der gedrehten Lage
    // verzerrt.
    const swapped = orientation % 2 === 1;
    const rx = swapped ? square.h : square.w;
    const ry = swapped ? square.w : square.h;

    if (index === 0) parts.push(`M ${from.x.toFixed(3)} ${from.y.toFixed(3)}`);
    // `sweep-flag` kippt mit der Spiegelung — sonst liefe die Spirale
    // gespiegelt in die falsche Richtung.
    const sweep = orientation >= 4 ? 0 : 1;
    parts.push(
      `A ${rx.toFixed(3)} ${ry.toFixed(3)} 0 0 ${sweep} ${to.x.toFixed(3)} ${to.y.toFixed(3)}`,
    );
  });

  return parts.join(" ");
}

/** Menschlich lesbare Lagebezeichnung für die Schaltfläche. */
export function orientationLabel(orientation: GridOrientation): string {
  const turns = (orientation % 4) * 90;
  return orientation >= 4 ? `${turns}° gespiegelt` : `${turns}°`;
}
