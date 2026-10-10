import { afterEach, describe, expect, it, vi } from "vitest";

import { burstAt, celebrate, celebrationsVisible, fireCannons, registerConfettiSink } from "./celebrate";
import type { BurstOptions } from "./confetti";

let unregister: (() => void) | null = null;

afterEach(() => {
  unregister?.();
  unregister = null;
});

function sink() {
  const calls: BurstOptions[][] = [];
  unregister = registerConfettiSink((bursts) => calls.push(bursts));
  return calls;
}

describe("celebrate", () => {
  /** Die wichtigste Eigenschaft: ohne Zeichenschicht passiert nichts
   * und es wirft nichts. Der Store ruft `celebrate()` im Erfolgsfall
   * eines Imports auf — eine Dekoration darf nie der Grund sein, dass
   * ein Import als gescheitert gilt. */
  it("tut ohne angemeldete Zeichenschicht nichts und wirft nicht", () => {
    expect(celebrationsVisible()).toBe(false);
    expect(() => celebrate()).not.toThrow();
    expect(() => burstAt(10, 10)).not.toThrow();
    expect(() => fireCannons()).not.toThrow();
  });

  it("meldet eine Zeichenschicht an und wieder ab", () => {
    const calls = sink();
    expect(celebrationsVisible()).toBe(true);

    celebrate();
    expect(calls).toHaveLength(2); // zwei Ausbrüche, links und rechts

    unregister?.();
    unregister = null;
    expect(celebrationsVisible()).toBe(false);
    celebrate();
    expect(calls).toHaveLength(2); // nichts kam dazu
  });

  it("schickt die Kanonen als leere Liste, damit die Geometrie an der Fenstergröße hängt", () => {
    const calls = sink();
    fireCannons();
    expect(calls).toEqual([[]]);
  });

  it("schießt einen Punkt-Ausbruch nach oben", () => {
    const calls = sink();
    burstAt(120, 400, 33);
    const burst = calls[0]![0]!;
    expect(burst.x).toBe(120);
    expect(burst.y).toBe(400);
    expect(burst.count).toBe(33);
    expect(Math.sin(burst.angle)).toBeLessThan(0);
    // Farben leer: die Zeichenschicht nimmt die CSS-Token-Palette,
    // damit es nur eine Farbliste gibt.
    expect(burst.colors).toEqual([]);
  });

  it("feiert aus dem unteren Drittel, links und rechts", () => {
    const calls = sink();
    vi.stubGlobal("innerWidth", 1000);
    vi.stubGlobal("innerHeight", 800);
    celebrate();
    const left = calls[0]![0]!;
    const right = calls[1]![0]!;
    expect(left.x).toBeLessThan(500);
    expect(right.x).toBeGreaterThan(500);
    expect(left.y).toBeGreaterThan(560);
    expect(right.y).toBeGreaterThan(560);
    vi.unstubAllGlobals();
  });

  /** Eine zweite Anmeldung ersetzt die erste — es gibt immer genau eine
   * Leinwand (siehe `ConfettiLayer`s Moduldoku). Die Abmeldefunktion der
   * ERSTEN darf die zweite dann nicht mit abräumen. */
  it("eine zweite Anmeldung ersetzt die erste, ohne sie beim Abmelden mitzunehmen", () => {
    const first: BurstOptions[][] = [];
    const unregisterFirst = registerConfettiSink((b) => first.push(b));
    const second: BurstOptions[][] = [];
    unregister = registerConfettiSink((b) => second.push(b));

    fireCannons();
    expect(first).toHaveLength(0);
    expect(second).toHaveLength(1);

    unregisterFirst();
    fireCannons();
    expect(second).toHaveLength(2);
  });
});
