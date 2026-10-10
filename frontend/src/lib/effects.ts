/**
 * **Effektstufen** — wie aufwendig die Oberfläche gezeichnet werden darf,
 * und wie das Gerät das selbst herausfindet (siehe `DECISIONS.md`
 * ADR-0073).
 *
 * **Das Problem, in Zahlen.** Die Glasflächen dieser App tragen zwei der
 * teuersten Dinge, die eine Webansicht kennt, gleichzeitig und auf
 * derselben Ebene: einen `backdrop-filter` (Weichzeichnen plus
 * Sättigung von allem, was durchscheint) und einen SVG-Verzerrungsfilter
 * aus `feTurbulence` + `feGaussianBlur` + `feDisplacementMap`. Gemessen
 * im Entwickeln-Modus bei 1600×900: **acht Flächen, alle acht mit
 * beidem, zusammen 1,08 Millionen Bildpunkte** — rund drei Viertel des
 * Fensters.
 *
 * Die Moduldoku von `ui/GlassDistortionFilter.tsx` behauptete, ein
 * geteiltes `<filter id>` genüge für beliebig viele Flächen und es gebe
 * „keine Vervielfachung der teuren SVG-Filterberechnung pro
 * Aufrufstelle". Das stimmt nicht: geteilt wird die *Definition*, die
 * *Auswertung* geschieht pro Element und über dessen eigene Fläche.
 *
 * **Warum es überhaupt auffällt.** Solange hinter einer Glasfläche
 * nichts passiert, ist das Ergebnis zwischenspeicherbar und die Kosten
 * fallen einmal an. Bewegt sich dahinter etwas, ist der
 * Zwischenspeicher bei jedem einzelnen Bild ungültig. Die
 * Geburtstags-Luftballons bewegen sich **dauerhaft** und liegen
 * **hinter allem** — sie sind damit, ohne selbst teuer zu sein, der
 * Auslöser dafür, dass acht Verzerrungsfilter sechzig Mal pro Sekunde
 * neu gerechnet werden. Auf einer nicht hardwarebeschleunigten
 * Webansicht (WebKitGTK unter Linux, der Regelfall für Tauri dort) macht
 * das die ganze Oberfläche zäh.
 *
 * **Warum gemessen und nicht geraten wird.** Ob ein Gerät das trägt,
 * hängt an Treiber, Compositor und Webansicht und lässt sich aus keiner
 * Kennzahl ableiten. Deshalb misst die Vorgabe `auto` die tatsächliche
 * Bildzeit auf dem Gerät des Nutzers und stuft herunter, wenn das
 * Budget nicht gehalten wird.
 *
 * **Die Messung muss wehtun.** Eine Messung auf einer ruhenden Seite
 * sagt nichts, weil genau dann alles zwischengespeichert ist. `probe`
 * bewegt deshalb selbst ein unsichtbares Element hinter den
 * Glasflächen — es misst den Fall, der zählt, nicht den Ruhezustand.
 */

/** Was in den Einstellungen steht. */
export type EffectSetting = "auto" | "full" | "reduced" | "plain";

/** Was am Ende wirklich gezeichnet wird. */
export type AppliedEffectLevel = "full" | "reduced" | "plain";

/** Reihenfolge von teuer nach billig — die Herunterstufung geht hier entlang. */
export const LEVELS: readonly AppliedEffectLevel[] = ["full", "reduced", "plain"];

/**
 * Obergrenze für eine Bildzeit, die sich noch flüssig anfühlt.
 *
 * 24 ms entspricht gut 41 Bildern je Sekunde. Bewusst nicht 16,7 ms
 * (also 60): ein Budget genau auf der Bildwiederholrate würde bei jeder
 * normalen Schwankung anschlagen und die Stufe hin- und herspringen
 * lassen. 24 ms trennt „läuft" von „ruckelt spürbar".
 */
export const FRAME_BUDGET_MS = 24;

/**
 * Ab hier ist nicht mehr von Ruckeln die Rede, sondern von Zähigkeit.
 * 40 ms sind 25 Bilder je Sekunde; dann wird nicht eine Stufe
 * heruntergegangen, sondern direkt auf die schnellste.
 */
export const SEVERE_FRAME_MS = 40;

/**
 * Unter so vielen Messwerten wird gar nicht entschieden. Die ersten
 * Bilder nach dem Start sind von Layout und Schriftladen verfälscht,
 * und eine Entscheidung aus drei Werten wäre Rauschen.
 */
export const MIN_SAMPLES = 20;

/** Median — robust gegen den einen Ausreißer, den jede Messung hat. */
export function median(values: readonly number[]): number {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const middle = sorted.length >> 1;
  return sorted.length % 2 === 0 ? (sorted[middle - 1]! + sorted[middle]!) / 2 : sorted[middle]!;
}

/**
 * Entscheidet aus gemessenen Bildzeiten, welche Stufe das Gerät trägt.
 *
 * Rein und ohne Seiteneffekte, damit die Schwellen prüfbar sind, statt
 * nur im laufenden Betrieb zu wirken.
 *
 * Es wird **nur herunter**gestuft, nie hinauf. Eine Automatik, die bei
 * jeder ruhigen Sekunde wieder hochschaltet und beim nächsten Ballon
 * zurückfällt, flackert — und ein flackerndes Fenster ist schlimmer als
 * ein dauerhaft schlichtes. Wer wieder mehr will, wählt die Stufe von
 * Hand; genau dafür gibt es die drei festen Werte.
 */
export function decideLevel(
  frameTimesMs: readonly number[],
  current: AppliedEffectLevel,
): AppliedEffectLevel {
  if (frameTimesMs.length < MIN_SAMPLES) return current;
  const typical = median(frameTimesMs);
  if (typical >= SEVERE_FRAME_MS) return "plain";
  if (typical <= FRAME_BUDGET_MS) return current;
  const next = LEVELS[Math.min(LEVELS.indexOf(current) + 1, LEVELS.length - 1)]!;
  return next;
}

/** Was aus Einstellung + Messung tatsächlich gezeichnet wird. */
export function resolveLevel(
  setting: EffectSetting,
  measured: AppliedEffectLevel,
): AppliedEffectLevel {
  return setting === "auto" ? measured : setting;
}

/** Schreibt die Stufe dorthin, wo `index.css` sie lesen kann. */
export function applyEffectLevel(level: AppliedEffectLevel, root: HTMLElement): void {
  root.setAttribute("data-effects", level);
}

interface ProbeDeps {
  requestAnimationFrame: (callback: (now: number) => void) => number;
  now: () => number;
  /** Wird vor der Messung eingehängt und danach wieder entfernt. */
  mountLoad?: () => () => void;
}

/**
 * Hängt ein bewegtes, unsichtbares Element hinter die Glasflächen.
 *
 * Genau das tun die Luftballons dauerhaft, und genau das macht die
 * Verzerrungsfilter teuer. Ohne diese Last misst die Messung den
 * Ruhezustand und meldet jedes Gerät als schnell.
 *
 * `z-index: 0` und `opacity: 0.01` statt `0`: eine Fläche mit Deckkraft
 * 0 darf der Compositor ganz weglassen, und dann wäre die Last wieder
 * weg. Ein Hundertstel ist unsichtbar und zählt trotzdem.
 */
function mountProbeLoad(): () => void {
  const probe = document.createElement("div");
  probe.setAttribute("aria-hidden", "true");
  probe.dataset.apxProbe = "frame-load";
  probe.style.cssText =
    "position:fixed;left:0;top:0;width:60vw;height:60vh;z-index:0;opacity:0.01;" +
    "background:linear-gradient(90deg,#fff,#000);pointer-events:none;will-change:transform";
  document.body.append(probe);
  let frame = 0;
  let raf = 0;
  const step = () => {
    frame += 1;
    probe.style.transform = `translate3d(${(frame * 7) % 400}px,0,0)`;
    raf = window.requestAnimationFrame(step);
  };
  raf = window.requestAnimationFrame(step);
  return () => {
    window.cancelAnimationFrame(raf);
    probe.remove();
  };
}

/**
 * Misst `sampleCount` Bildzeiten unter dieser Last.
 *
 * Die ersten beiden Werte fallen weg: der allererste enthält die
 * Zeit seit dem letzten fremden Bild, der zweite oft noch das
 * Einhängen der Last selbst.
 */
export function probeFrameTimes(
  sampleCount: number,
  deps: ProbeDeps = {
    requestAnimationFrame: (cb) => window.requestAnimationFrame(cb),
    now: () => performance.now(),
    mountLoad: mountProbeLoad,
  },
): Promise<number[]> {
  return new Promise((resolve) => {
    const unmount = deps.mountLoad?.() ?? (() => {});
    const times: number[] = [];
    let last = deps.now();
    const tick = (now: number) => {
      times.push(now - last);
      last = now;
      if (times.length < sampleCount + 2) {
        deps.requestAnimationFrame(tick);
      } else {
        unmount();
        resolve(times.slice(2));
      }
    };
    deps.requestAnimationFrame(tick);
  });
}
