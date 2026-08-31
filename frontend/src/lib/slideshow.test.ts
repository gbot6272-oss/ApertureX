import { describe, expect, it } from "vitest";

import {
  buildSlideItems,
  buildTimeline,
  coverAdjustedSourceRect,
  defaultKenBurns,
  kenBurnsCropRect,
  segmentAtTime,
  segmentProgress,
  STATIC_KEN_BURNS,
  timelineDuration,
  type PhotoSlideItem,
  type SlideshowSettings,
  type TitleSlideItem,
} from "./slideshow";

function photo(holdSeconds: number): PhotoSlideItem {
  return { kind: "photo", photoId: "p", holdSeconds, kenBurns: STATIC_KEN_BURNS };
}

function title(text: string, holdSeconds: number): TitleSlideItem {
  return { kind: "title", text, holdSeconds, backgroundRgb: [0, 0, 0], textColor: [255, 255, 255] };
}

describe("kenBurnsCropRect", () => {
  it("deckt bei statischem Effekt immer das ganze Bild ab", () => {
    for (const progress of [0, 0.3, 1]) {
      expect(kenBurnsCropRect(STATIC_KEN_BURNS, progress)).toEqual({ x: 0, y: 0, w: 1, h: 1 });
    }
  });

  it("schrumpft den Ausschnitt beim Heranzoomen", () => {
    const spec = { zoomStart: 1, zoomEnd: 2, panStart: [0.5, 0.5] as [number, number], panEnd: [0.5, 0.5] as [number, number] };
    const start = kenBurnsCropRect(spec, 0);
    const end = kenBurnsCropRect(spec, 1);
    expect(start.w).toBe(1);
    expect(end.w).toBeCloseTo(0.5);
    expect(end.h).toBeCloseTo(0.5);
  });

  it("verlaesst nie die Bildgrenzen, selbst bei extremem Schwenk", () => {
    const spec = { zoomStart: 3, zoomEnd: 3, panStart: [0, 0] as [number, number], panEnd: [1, 1] as [number, number] };
    for (const progress of [0, 0.25, 0.5, 0.75, 1]) {
      const rect = kenBurnsCropRect(spec, progress);
      expect(rect.x).toBeGreaterThanOrEqual(0);
      expect(rect.y).toBeGreaterThanOrEqual(0);
      expect(rect.x + rect.w).toBeLessThanOrEqual(1 + 1e-6);
      expect(rect.y + rect.h).toBeLessThanOrEqual(1 + 1e-6);
    }
  });
});

describe("defaultKenBurns", () => {
  it("liefert den statischen Effekt, wenn abgeschaltet", () => {
    expect(defaultKenBurns(0, false)).toEqual(STATIC_KEN_BURNS);
    expect(defaultKenBurns(3, false)).toEqual(STATIC_KEN_BURNS);
  });

  it("wechselt die Zoomrichtung je Index", () => {
    const even = defaultKenBurns(0, true);
    const odd = defaultKenBurns(1, true);
    expect(even.zoomStart).toBeLessThan(even.zoomEnd);
    expect(odd.zoomStart).toBeGreaterThan(odd.zoomEnd);
  });
});

describe("coverAdjustedSourceRect", () => {
  it("ist ein No-Op, wenn Seitenverhaeltnisse bereits uebereinstimmen", () => {
    const rect = coverAdjustedSourceRect({ x: 0.1, y: 0.2, w: 0.5, h: 0.5 }, 100, 100, 200, 200);
    expect(rect).toEqual({ sx: 10, sy: 20, sw: 50, sh: 50 });
  });

  it("beschneidet ein zu breites Bild zentriert fuer eine hochformatige Zielflaeche", () => {
    // Ganzes 200x100-Bild (2:1) auf eine 1:1-Zielflaeche.
    const rect = coverAdjustedSourceRect({ x: 0, y: 0, w: 1, h: 1 }, 200, 100, 100, 100);
    expect(rect.sy).toBeCloseTo(0);
    expect(rect.sh).toBeCloseTo(100);
    expect(rect.sw).toBeCloseTo(100); // 100px von 200px Breite bleiben
    expect(rect.sx).toBeCloseTo(50); // zentriert
  });

  it("beschneidet ein zu hohes Bild zentriert fuer eine querformatige Zielflaeche", () => {
    const rect = coverAdjustedSourceRect({ x: 0, y: 0, w: 1, h: 1 }, 100, 200, 100, 100);
    expect(rect.sx).toBeCloseTo(0);
    expect(rect.sw).toBeCloseTo(100);
    expect(rect.sh).toBeCloseTo(100);
    expect(rect.sy).toBeCloseTo(50);
  });
});

describe("buildTimeline", () => {
  it("ist leer ohne Folien", () => {
    expect(buildTimeline([], "cut", 1)).toEqual([]);
  });

  it("harter Schnitt erzeugt nur Halte-Segmente ohne Ueberblendung", () => {
    const segments = buildTimeline([photo(2), photo(3)], "cut", 1);
    expect(segments).toHaveLength(2);
    expect(segments.every((s) => s.type === "hold")).toBe(true);
    expect(timelineDuration(segments)).toBeCloseTo(5);
  });

  it("Ueberblendung fuegt ein Uebergangssegment zwischen den Folien ein", () => {
    const segments = buildTimeline([photo(2), photo(2)], "cross_fade", 1);
    expect(segments).toHaveLength(3);
    expect(segments[1]!.type).toBe("transition");
    expect(timelineDuration(segments)).toBeCloseTo(5); // 2 + 1 + 2
  });

  it("die letzte Folie bekommt keinen nachfolgenden Uebergang", () => {
    const segments = buildTimeline([photo(1)], "cross_fade", 1);
    expect(segments).toHaveLength(1);
    expect(segments[0]!.type).toBe("hold");
  });

  it("sehr kurze Haltezeiten werden auf ein Mindestmass angehoben", () => {
    const segments = buildTimeline([photo(0.001)], "cut", 0);
    expect(timelineDuration(segments)).toBeGreaterThan(0);
  });

  it("Titelkarten reihen sich wie normale Folien in die Zeitachse ein", () => {
    const segments = buildTimeline([title("Intro", 2), photo(3)], "cut", 0);
    expect(segments).toHaveLength(2);
    expect(timelineDuration(segments)).toBeCloseTo(5);
  });
});

describe("buildSlideItems", () => {
  const baseSettings: SlideshowSettings = {
    slideSeconds: 3,
    kenBurns: true,
    transition: "cut",
    transitionSeconds: 1,
  };

  it("baut einen Eintrag je Foto ohne Intro/Outro", () => {
    const items = buildSlideItems(["a", "b"], baseSettings);
    expect(items).toHaveLength(2);
    expect(items.every((item) => item.kind === "photo")).toBe(true);
  });

  it("stellt Intro voran und haengt Outro an", () => {
    const settings: SlideshowSettings = {
      ...baseSettings,
      intro: { text: "Willkommen", holdSeconds: 2, backgroundRgb: [0, 0, 0], textColor: [255, 255, 255] },
      outro: { text: "Ende", holdSeconds: 2, backgroundRgb: [0, 0, 0], textColor: [255, 255, 255] },
    };
    const items = buildSlideItems(["a"], settings);
    expect(items).toHaveLength(3);
    expect(items[0]).toMatchObject({ kind: "title", text: "Willkommen" });
    expect(items[1]).toMatchObject({ kind: "photo", photoId: "a" });
    expect(items[2]).toMatchObject({ kind: "title", text: "Ende" });
  });

  it("vergibt jedem Foto ein deterministisches Ken-Burns-Muster nach Index", () => {
    const items = buildSlideItems(["a", "b"], baseSettings) as PhotoSlideItem[];
    expect(items[0]!.kenBurns).toEqual(defaultKenBurns(0, true));
    expect(items[1]!.kenBurns).toEqual(defaultKenBurns(1, true));
  });

  it("Ken-Burns abgeschaltet ergibt fuer alle Fotos den statischen Effekt", () => {
    const items = buildSlideItems(["a", "b"], { ...baseSettings, kenBurns: false }) as PhotoSlideItem[];
    expect(items[0]!.kenBurns).toEqual(STATIC_KEN_BURNS);
    expect(items[1]!.kenBurns).toEqual(STATIC_KEN_BURNS);
  });
});

describe("segmentAtTime / segmentProgress", () => {
  it("findet das richtige Segment zu einem Zeitpunkt", () => {
    const segments = buildTimeline([photo(2), photo(2)], "cut", 0);
    expect(segmentAtTime(segments, 0.5)).toEqual(segments[0]);
    expect(segmentAtTime(segments, 2.5)).toEqual(segments[1]);
  });

  it("klemmt Zeitpunkte ausserhalb der Zeitachse auf das letzte Segment", () => {
    const segments = buildTimeline([photo(2)], "cut", 0);
    expect(segmentAtTime(segments, 100)).toEqual(segments[0]);
    expect(segmentAtTime(segments, -5)).toEqual(segments[0]);
  });

  it("liefert null fuer eine leere Zeitachse", () => {
    expect(segmentAtTime([], 0)).toBeNull();
  });

  it("berechnet den Fortschritt innerhalb eines Segments linear", () => {
    const segments = buildTimeline([photo(2)], "cut", 0);
    expect(segmentProgress(segments[0]!, 0)).toBeCloseTo(0);
    expect(segmentProgress(segments[0]!, 1)).toBeCloseTo(0.5);
    expect(segmentProgress(segments[0]!, 2)).toBeCloseTo(1);
  });
});
