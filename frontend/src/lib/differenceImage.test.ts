import { describe, expect, it } from "vitest";

import { DEFAULT_AMPLIFY, differenceAmount, differenceImage } from "./differenceImage";

function pixels(...rgb: [number, number, number][]): Uint8ClampedArray {
  return new Uint8ClampedArray(rgb.flatMap(([r, g, b]) => [r, g, b, 255]));
}

describe("differenceImage", () => {
  it("liefert fuer identische Bilder ein schwarzes Ergebnis", () => {
    const img = pixels([10, 200, 90], [0, 0, 0]);
    const diff = differenceImage(img, img, { amplify: DEFAULT_AMPLIFY, mode: "channels" })!;
    for (let i = 0; i < diff.length; i += 4) {
      expect([diff[i], diff[i + 1], diff[i + 2]]).toEqual([0, 0, 0]);
    }
  });

  it("verstaerkt kleine Unterschiede in den sichtbaren Bereich", () => {
    const a = pixels([100, 100, 100]);
    const b = pixels([102, 100, 100]);
    const ungestreckt = differenceImage(a, b, { amplify: 1, mode: "channels" })!;
    const gestreckt = differenceImage(a, b, { amplify: 20, mode: "channels" })!;
    expect(ungestreckt[0]).toBe(2);
    expect(gestreckt[0]).toBe(40);
  });

  it("schneidet bei 255 ab statt umzulaufen", () => {
    const diff = differenceImage(pixels([0, 0, 0]), pixels([255, 255, 255]), {
      amplify: 10,
      mode: "channels",
    })!;
    expect(diff[0]).toBe(255);
  });

  it("ist symmetrisch — die Reihenfolge der Bilder aendert nichts", () => {
    const a = pixels([30, 60, 90]);
    const b = pixels([90, 60, 30]);
    const options = { amplify: 4, mode: "channels" as const };
    expect([...differenceImage(a, b, options)!]).toEqual([...differenceImage(b, a, options)!]);
  });

  it("macht einen reinen Farbstich nur kanalweise sichtbar", () => {
    // Nur Rot verschoben: `luma` gewichtet Rot mit 0.2126, der
    // Unterschied faellt dort deutlich schwaecher aus als im roten
    // Kanal selbst.
    const a = pixels([100, 100, 100]);
    const b = pixels([140, 100, 100]);
    const kanal = differenceImage(a, b, { amplify: 1, mode: "channels" })!;
    const hell = differenceImage(a, b, { amplify: 1, mode: "luma" })!;
    expect(kanal[0]).toBe(40);
    expect(hell[0]).toBe(9); // 40 * 0.2126 gerundet
  });

  it("zeigt eine Helligkeitsaenderung in beiden Betriebsarten", () => {
    const a = pixels([100, 100, 100]);
    const b = pixels([120, 120, 120]);
    expect(differenceImage(a, b, { amplify: 1, mode: "luma" })![0]).toBe(20);
    expect(differenceImage(a, b, { amplify: 1, mode: "channels" })![0]).toBe(20);
  });

  it("macht das Ergebnis immer undurchsichtig", () => {
    const a = new Uint8ClampedArray([10, 10, 10, 0]);
    const b = new Uint8ClampedArray([20, 20, 20, 0]);
    expect(differenceImage(a, b, { amplify: 1, mode: "channels" })![3]).toBe(255);
  });

  it("weist verschieden grosse Bilder ab, statt sie zurechtzuschneiden", () => {
    expect(differenceImage(pixels([0, 0, 0]), pixels([0, 0, 0], [0, 0, 0]), {
      amplify: 1,
      mode: "channels",
    })).toBeNull();
  });
});

describe("differenceAmount", () => {
  it("ist 0 fuer identische und 1 fuer maximal verschiedene Bilder", () => {
    const schwarz = pixels([0, 0, 0]);
    const weiss = pixels([255, 255, 255]);
    expect(differenceAmount(schwarz, schwarz)).toBe(0);
    expect(differenceAmount(schwarz, weiss)).toBe(1);
  });

  it("haengt NICHT von der Verstaerkung ab — sie ist eine Sichthilfe", () => {
    // Die Funktion nimmt die Verstaerkung gar nicht entgegen; dieser
    // Test haelt die Entscheidung fest, damit sie nicht spaeter
    // "der Bequemlichkeit halber" hineinwandert.
    const a = pixels([100, 100, 100]);
    const b = pixels([110, 100, 100]);
    expect(differenceAmount(a, b)).toBeCloseTo(10 / (3 * 255), 6);
  });

  it("liefert null fuer leere oder ungleich grosse Puffer", () => {
    expect(differenceAmount(new Uint8ClampedArray(), new Uint8ClampedArray())).toBeNull();
    expect(differenceAmount(pixels([0, 0, 0]), pixels([0, 0, 0], [1, 1, 1]))).toBeNull();
  });
});
