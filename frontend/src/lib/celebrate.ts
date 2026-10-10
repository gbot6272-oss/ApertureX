import type { BurstOptions } from "./confetti";

/**
 * Die Anlässe, zu denen diese Fassung feiert (Geburtstagsausgabe, siehe
 * `DECISIONS.md` ADR-0071) — und der schmale Kanal dorthin.
 *
 * **Warum das hier steht und nicht in `ConfettiLayer.tsx`.** Die Stellen,
 * die feiern wollen, sind zum Teil im Store (`finishImport`,
 * `exportPhotos`), und der Store darf keine React-Komponente
 * importieren. Dieselbe Trennung wie bei `lib/sound.ts`: ein
 * modulweiter Kanal, den die Oberfläche bedient und jeder aufrufen
 * kann, ohne die Zeichenschicht zu kennen.
 *
 * **Ohne montierte Zeichenschicht ein folgenloser No-op.** Deko aus,
 * reduzierte Bewegung, Tests: dann hört niemand zu, und
 * [`celebrate`] tut nichts. Eine Dekoration darf nie der Grund sein,
 * dass ein Import oder ein Export scheitert — deshalb gibt es hier
 * keinen Fehlerpfad, nur Stille.
 *
 * **Sparsam ist Absicht.** Gefeiert wird bei Import- und Export-Ende,
 * also bei Dingen, die Minuten dauern und ein paar Mal am Tag
 * vorkommen. Nicht bei jedem angewendeten Preset: Konfetti, das bei
 * jedem Klick fliegt, ist nach zehn Minuten kein Fest mehr, sondern
 * ein Flackern, das man abschalten will. Wer mehr will, hat die
 * Begrüßung beim Öffnen.
 */

/** Was die Zeichenschicht entgegennimmt. Eine leere Liste bedeutet
 * „die Kanonen der Begrüßung", deren Geometrie von der Fenstergröße
 * abhängt und deshalb dort berechnet wird. */
export type ConfettiSink = (bursts: BurstOptions[]) => void;

let sink: ConfettiSink | null = null;

/** Von `ConfettiLayer` beim Montieren aufgerufen; gibt die
 * Abmelde-Funktion zurück. */
export function registerConfettiSink(next: ConfettiSink): () => void {
  sink = next;
  return () => {
    if (sink === next) sink = null;
  };
}

/** `true`, solange eine Zeichenschicht zuhört. */
export function celebrationsVisible(): boolean {
  return sink !== null;
}

/**
 * Ein Ausbruch an einer Bildschirmposition — zum Beispiel am Knopf,
 * der gerade gedrückt wurde.
 */
export function burstAt(x: number, y: number, count = 70): void {
  sink?.([
    {
      x,
      y,
      // Nach oben, weit geöffnet: ein Ausbruch an einem Knopf soll von
      // dort aufblühen, nicht in eine Richtung schießen.
      angle: -Math.PI / 2,
      spread: Math.PI / 2.4,
      velocity: 620,
      count,
      // Leer = die Zeichenschicht nimmt die CSS-Token-Palette (siehe
      // `confetti.partyColors`), damit es nur eine Farbliste gibt.
      colors: [],
      life: 2.2,
    },
  ]);
}

/** Die beiden Kanonen der Begrüßung. */
export function fireCannons(): void {
  sink?.([]);
}

/**
 * „Etwas Langes ist fertig geworden" — Import oder Export.
 *
 * Zwei Ausbrüche aus dem unteren Drittel, nach oben: von dort kommt in
 * dieser Oberfläche die Fortschrittsmeldung, und ein Ausbruch, der
 * dort beginnt, gehört sichtbar zu ihr.
 */
export function celebrate(): void {
  if (sink === null) return;
  const w = window.innerWidth;
  const h = window.innerHeight;
  burstAt(w * 0.3, h * 0.78, 55);
  burstAt(w * 0.7, h * 0.78, 55);
}
