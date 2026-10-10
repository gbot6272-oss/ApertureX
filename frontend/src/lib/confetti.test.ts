import { describe, expect, it } from "vitest";

import {
  MAX_PARTICLES,
  cannonBursts,
  particleOpacity,
  partyColors,
  spawnBurst,
  stepParticles,
  type ConfettiParticle,
} from "./confetti";

/** Feste Zufallsfolge: macht jeden Test hier deterministisch, ohne die
 * Physik zu verfälschen (die Werte liegen alle im erlaubten `[0,1)`). */
function fixedRandom(values: number[]): () => number {
  let i = 0;
  return () => values[i++ % values.length]!;
}

const COLORS = ["#f0a93c", "#e876a0"] as const;

function burst(count: number, overrides: Partial<Parameters<typeof spawnBurst>[0]> = {}) {
  return spawnBurst(
    { x: 100, y: 500, angle: -Math.PI / 2, spread: 0.3, velocity: 800, count, colors: COLORS, ...overrides },
    0,
    fixedRandom([0.5, 0.25, 0.75, 0.1, 0.9, 0.4]),
  );
}

describe("spawnBurst", () => {
  it("erzeugt die gewünschte Zahl Teilchen am Ursprung", () => {
    const particles = burst(20);
    expect(particles).toHaveLength(20);
    for (const p of particles) {
      expect(p.x).toBe(100);
      expect(p.y).toBe(500);
      expect(COLORS).toContain(p.color as (typeof COLORS)[number]);
    }
  });

  it("schießt im Mittel in die angegebene Richtung", () => {
    const particles = burst(60);
    const meanVy = particles.reduce((sum, p) => sum + p.vy, 0) / particles.length;
    // `angle = -PI/2` ist nach oben, und oben ist in Bildschirm-
    // koordinaten negativ.
    expect(meanVy).toBeLessThan(0);
  });

  /** Das ist die Zusage, die den Fotoeditor schützt: schnelle Klicks
   * dürfen sich nicht aufaddieren. */
  it("kappt gegen das app-weite Budget und gibt nichts zurück, wenn es erschöpft ist", () => {
    const partial = spawnBurst(
      { x: 0, y: 0, angle: 0, spread: 0.2, velocity: 400, count: 500, colors: COLORS },
      MAX_PARTICLES - 10,
    );
    expect(partial).toHaveLength(10);

    const none = spawnBurst(
      { x: 0, y: 0, angle: 0, spread: 0.2, velocity: 400, count: 50, colors: COLORS },
      MAX_PARTICLES,
    );
    expect(none).toHaveLength(0);
  });

  it("erzeugt nichts ohne Farben oder ohne Anzahl", () => {
    expect(spawnBurst({ x: 0, y: 0, angle: 0, spread: 0.2, velocity: 400, count: 10, colors: [] })).toHaveLength(0);
    expect(burst(0)).toHaveLength(0);
  });
});

describe("stepParticles", () => {
  function one(overrides: Partial<ConfettiParticle> = {}): ConfettiParticle {
    return {
      x: 0,
      y: 0,
      vx: 0,
      vy: 0,
      spin: 0,
      spinRate: 2,
      size: 5,
      color: "#fff",
      life: 2,
      maxLife: 2,
      ...overrides,
    };
  }

  it("lässt Teilchen fallen und dreht sie", () => {
    const [p] = stepParticles([one()], 1 / 60);
    expect(p!.vy).toBeGreaterThan(0);
    expect(p!.y).toBeGreaterThan(0);
    expect(p!.spin).toBeGreaterThan(0);
  });

  it("entfernt Teilchen, deren Lebensdauer abgelaufen ist", () => {
    expect(stepParticles([one({ life: 0.01 })], 0.1)).toHaveLength(0);
  });

  /** Luftwiderstand ist der Unterschied zwischen Konfetti und
   * Kieselsteinen: ohne ihn wächst die Geschwindigkeit unbegrenzt. */
  it("erreicht eine Endgeschwindigkeit statt immer schneller zu werden", () => {
    let particles = [one({ life: 1000, maxLife: 1000 })];
    for (let i = 0; i < 600; i += 1) particles = stepParticles(particles, 1 / 60);
    const settled = particles[0]!.vy;

    for (let i = 0; i < 600; i += 1) particles = stepParticles(particles, 1 / 60);
    // Nach weiteren zehn Sekunden praktisch unverändert — das ist die
    // Endgeschwindigkeit, nicht freier Fall.
    expect(Math.abs(particles[0]!.vy - settled)).toBeLessThan(settled * 0.02);
    expect(settled).toBeLessThan(1400);
  });

  /** Bildratenunabhängigkeit: 144 Hz und 60 Hz müssen nach derselben
   * Zeit auf praktisch derselben Höhe landen. Ein linearer
   * Dämpfungsfaktor pro Bild (der naheliegende Fehler) würde hier
   * deutlich auseinanderlaufen. */
  it("fällt bei hoher Bildrate genauso weit wie bei niedriger", () => {
    let slow = [one({ life: 100, maxLife: 100 })];
    let fast = [one({ life: 100, maxLife: 100 })];
    for (let i = 0; i < 60; i += 1) slow = stepParticles(slow, 1 / 60);
    for (let i = 0; i < 144; i += 1) fast = stepParticles(fast, 1 / 144);

    const a = slow[0]!.y;
    const b = fast[0]!.y;
    expect(Math.abs(a - b)).toBeLessThan(a * 0.05);
  });

  /** Rückkehr aus dem Hintergrund-Tab: `requestAnimationFrame` liefert
   * dann einen Sprung von Sekunden. Ungedeckelt würde alles in einem
   * Bild quer über den Bildschirm geschleudert. */
  it("deckelt einen großen Zeitsprung statt die Teilchen zu schleudern", () => {
    const [jumped] = stepParticles([one({ life: 100, maxLife: 100 })], 12);
    const [stepped] = stepParticles([one({ life: 100, maxLife: 100 })], 0.05);
    expect(jumped!.y).toBeCloseTo(stepped!.y, 6);
  });

  it("gibt bei dt = 0 eine Kopie zurück, nicht dasselbe Array", () => {
    const input = [one()];
    const output = stepParticles(input, 0);
    expect(output).not.toBe(input);
    expect(output[0]).toEqual(input[0]);
  });
});

describe("particleOpacity", () => {
  it("ist am Anfang voll und blendet zum Ende aus", () => {
    const base = { x: 0, y: 0, vx: 0, vy: 0, spin: 0, spinRate: 0, size: 4, color: "#fff", maxLife: 3 };
    expect(particleOpacity({ ...base, life: 3 })).toBe(1);
    expect(particleOpacity({ ...base, life: 1 })).toBe(1);
    expect(particleOpacity({ ...base, life: 0.5 })).toBeCloseTo(0.5, 5);
    expect(particleOpacity({ ...base, life: 0 })).toBe(0);
  });
});

describe("cannonBursts", () => {
  it("stellt zwei gegenläufige Kanonen unten an die Ränder", () => {
    const [left, right] = cannonBursts(1200, 800, COLORS);
    expect(left!.x).toBeLessThan(600);
    expect(right!.x).toBeGreaterThan(600);
    expect(left!.y).toBeGreaterThan(700);
    expect(right!.y).toBeGreaterThan(700);
    // Beide schießen nach oben, aber zu verschiedenen Seiten.
    expect(Math.sin(left!.angle)).toBeLessThan(0);
    expect(Math.sin(right!.angle)).toBeLessThan(0);
    expect(Math.cos(left!.angle)).toBeGreaterThan(0);
    expect(Math.cos(right!.angle)).toBeLessThan(0);
  });

  it("überschreitet mit beiden Kanonen zusammen das Budget nicht", () => {
    const total = cannonBursts(1200, 800, COLORS).reduce((sum, b) => sum + b.count, 0);
    expect(total).toBeLessThanOrEqual(MAX_PARTICLES);
  });
});

describe("partyColors", () => {
  it("fällt auf eine warme Palette zurück, wenn keine Tokens gesetzt sind", () => {
    const colors = partyColors(document.createElement("div"));
    expect(colors.length).toBeGreaterThanOrEqual(6);
    for (const color of colors) expect(color).toMatch(/^#[0-9a-f]{6}$/i);
  });
});

/**
 * Ein Rechen-Budget, nicht ein Schönheitstest.
 *
 * Die Deko läuft in einer App, die gleichzeitig RAW-Dateien
 * dekodiert. Ein voller Ausbruch (`MAX_PARTICLES`) muss deshalb pro
 * Bild so wenig kosten, dass er im 16-ms-Budget (SPEC.md §2.4) nicht
 * auffällt. Gemessen wird bewusst sehr großzügig — die Schranke soll
 * eine Größenordnung fangen (eine versehentlich quadratische
 * Schleife), nicht Mikrosekunden auf einem geteilten CI-Rechner.
 */
describe("Rechenaufwand", () => {
  it("rechnet 600 Bilder mit dem vollen Teilchenbudget in unter 150 ms", () => {
    let particles = spawnBurst({
      x: 500,
      y: 500,
      angle: -Math.PI / 2,
      spread: 1,
      velocity: 800,
      count: MAX_PARTICLES,
      colors: COLORS,
      life: 1000,
    });
    expect(particles).toHaveLength(MAX_PARTICLES);

    const started = performance.now();
    for (let frame = 0; frame < 600; frame += 1) {
      particles = stepParticles(particles, 1 / 60);
    }
    const elapsed = performance.now() - started;

    expect(particles).toHaveLength(MAX_PARTICLES);
    expect(elapsed).toBeLessThan(150);
  });
});
