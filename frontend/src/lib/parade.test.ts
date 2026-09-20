import { describe, expect, it } from "vitest";

import { analyzeParade, CAST_THRESHOLD, readCast, readCasts } from "./parade";
import { computeWaveform } from "./waveform";

/** Baut einen RGBA8-Puffer, in dem jeder Pixel dieselbe Farbe hat. */
function flat(width: number, height: number, rgb: [number, number, number]): Uint8Array {
  const pixels = new Uint8Array(width * height * 4);
  for (let i = 0; i < width * height; i += 1) {
    pixels[i * 4] = rgb[0];
    pixels[i * 4 + 1] = rgb[1];
    pixels[i * 4 + 2] = rgb[2];
    pixels[i * 4 + 3] = 255;
  }
  return pixels;
}

describe("analyzeParade", () => {
  it("liest bei einer einfarbigen Fläche Schwarz- und Weißpunkt auf demselben Wert", () => {
    const analysis = analyzeParade(computeWaveform(flat(16, 16, [100, 100, 100]), 16, 16));
    expect(analysis.r.black).toBe(100);
    expect(analysis.r.white).toBe(100);
  });

  it("trennt die Kanäle — ein Blaustich hebt nur den Blaukanal", () => {
    const analysis = analyzeParade(computeWaveform(flat(16, 16, [40, 40, 80]), 16, 16));
    expect(analysis.r.black).toBe(40);
    expect(analysis.g.black).toBe(40);
    expect(analysis.b.black).toBe(80);
  });

  it("lässt sich von einzelnen Ausreißern nicht den Schwarzpunkt verschieben", () => {
    // 1024 Pixel auf 100, ein einziger auf 0 — das ist unter der
    // 0,5-Prozent-Schwelle und darf den Schwarzpunkt nicht auf 0 ziehen.
    const pixels = flat(32, 32, [100, 100, 100]);
    pixels[0] = 0;
    pixels[1] = 0;
    pixels[2] = 0;
    const analysis = analyzeParade(computeWaveform(pixels, 32, 32));
    expect(analysis.r.black).toBe(100);
  });

  it("meldet für einen leeren Tonwertbereich null statt eines erfundenen Werts", () => {
    // Alles in den Tiefen — Mitten und Lichter sind leer.
    const analysis = analyzeParade(computeWaveform(flat(8, 8, [10, 10, 10]), 8, 8));
    expect(analysis.r.shadowMean).toBe(10);
    expect(analysis.r.midMean).toBeNull();
    expect(analysis.r.highlightMean).toBeNull();
  });

  it("kommt mit einem leeren Bild zurecht", () => {
    const analysis = analyzeParade(computeWaveform(new Uint8Array(0), 0, 0));
    expect(analysis.r.black).toBe(0);
    expect(analysis.r.shadowMean).toBeNull();
  });
});

describe("readCast", () => {
  it("nennt den dominierenden Kanal", () => {
    expect(readCast([40, 40, 80]).channel).toBe("b");
    expect(readCast([80, 40, 40]).channel).toBe("r");
    expect(readCast([40, 80, 40]).channel).toBe("g");
  });

  it("meldet unterhalb der Schwelle keinen Stich", () => {
    const reading = readCast([100, 100 + CAST_THRESHOLD - 1, 100]);
    expect(reading.channel).toBeNull();
    expect(reading.delta).toBe(CAST_THRESHOLD - 1);
  });

  it("meldet genau an der Schwelle einen Stich", () => {
    expect(readCast([100, 100 + CAST_THRESHOLD, 100]).channel).toBe("g");
  });

  it("meldet ohne vollständige Kanalwerte gar nichts", () => {
    // Aus zwei von drei Kanälen lässt sich kein Farbstich ablesen.
    expect(readCast([40, null, 80])).toEqual({ channel: null, delta: 0 });
  });
});

describe("readCasts", () => {
  it("findet einen Stich in den Tiefen, ohne Mitten und Lichter zu erfinden", () => {
    const casts = readCasts(analyzeParade(computeWaveform(flat(16, 16, [20, 20, 50]), 16, 16)));
    expect(casts.shadows.channel).toBe("b");
    expect(casts.shadows.delta).toBe(30);
    expect(casts.midtones.channel).toBeNull();
    expect(casts.highlights.channel).toBeNull();
  });

  it("meldet einen Gesamtstich auch dann, wenn kein einzelner Bereich eine Aussage hat", () => {
    // Genau der Fall, für den `overall` da ist: eine einfarbige Fläche
    // liegt je Kanal in einem anderen Tonwertbereich, also hat keiner
    // der drei alle Kanäle — der Stich ist trotzdem offensichtlich.
    const casts = readCasts(analyzeParade(computeWaveform(flat(16, 16, [180, 140, 100]), 16, 16)));
    expect(casts.shadows.channel).toBeNull();
    expect(casts.midtones.channel).toBeNull();
    expect(casts.highlights.channel).toBeNull();
    expect(casts.overall.channel).toBe("r");
    expect(casts.overall.delta).toBe(80);
  });

  it("meldet bei einem neutralen Bild in keinem Bereich einen Stich", () => {
    const pixels = new Uint8Array(32 * 32 * 4);
    for (let i = 0; i < 32 * 32; i += 1) {
      // Ein neutraler Verlauf über alle drei Tonwertbereiche.
      const value = Math.floor((i / (32 * 32)) * 255);
      pixels[i * 4] = value;
      pixels[i * 4 + 1] = value;
      pixels[i * 4 + 2] = value;
      pixels[i * 4 + 3] = 255;
    }
    const casts = readCasts(analyzeParade(computeWaveform(pixels, 32, 32)));
    expect(casts.shadows.channel).toBeNull();
    expect(casts.midtones.channel).toBeNull();
    expect(casts.highlights.channel).toBeNull();
    expect(casts.overall.channel).toBeNull();
  });
});
