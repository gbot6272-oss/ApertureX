import { describe, expect, it } from "vitest";

import {
  FRAME_BUDGET_MS,
  MIN_SAMPLES,
  SEVERE_FRAME_MS,
  applyEffectLevel,
  decideLevel,
  median,
  probeFrameTimes,
  resolveLevel,
  type AppliedEffectLevel,
} from "./effects";

/** Eine Messreihe aus lauter gleichen Werten — kurz und unmissverständlich. */
function flat(ms: number, count = MIN_SAMPLES): number[] {
  return Array.from({ length: count }, () => ms);
}

describe("median", () => {
  it("nimmt bei gerader Anzahl das Mittel der beiden mittleren Werte", () => {
    expect(median([1, 2, 3, 4])).toBe(2.5);
  });

  it("ist gegen einen einzelnen Ausreißer unempfindlich", () => {
    // Genau dafür wird der Median und nicht der Mittelwert verwendet:
    // ein einzelner Aussetzer durch die Speicherbereinigung darf die
    // ganze Oberfläche nicht herunterstufen.
    const withSpike = [...flat(10, 19), 900];
    expect(median(withSpike)).toBe(10);
    expect(decideLevel(withSpike, "full")).toBe("full");
  });

  it("liefert 0 für eine leere Reihe statt NaN", () => {
    expect(median([])).toBe(0);
  });
});

describe("decideLevel", () => {
  it("entscheidet unter zu wenigen Messwerten gar nicht", () => {
    // Lieber keine Entscheidung als eine aus Rauschen: die ersten
    // Bilder nach dem Start sind von Layout und Schriftladen verfälscht.
    expect(decideLevel(flat(100, MIN_SAMPLES - 1), "full")).toBe("full");
  });

  it("lässt die Stufe stehen, solange das Budget gehalten wird", () => {
    expect(decideLevel(flat(FRAME_BUDGET_MS), "full")).toBe("full");
    expect(decideLevel(flat(16.7), "full")).toBe("full");
  });

  it("stuft bei überschrittenem Budget um genau eine Stufe herunter", () => {
    expect(decideLevel(flat(FRAME_BUDGET_MS + 1), "full")).toBe("reduced");
    expect(decideLevel(flat(FRAME_BUDGET_MS + 1), "reduced")).toBe("plain");
  });

  it("springt bei zäher Darstellung sofort auf die schnellste Stufe", () => {
    // 40 ms sind 25 Bilder je Sekunde. Sich von dort in Einzelschritten
    // herunterzuarbeiten hieße, den Nutzer zweimal warten zu lassen,
    // bevor es hilft.
    expect(decideLevel(flat(SEVERE_FRAME_MS), "full")).toBe("plain");
  });

  it("geht nie unter die schnellste Stufe", () => {
    expect(decideLevel(flat(500), "plain")).toBe("plain");
  });

  it("stuft nie wieder hinauf", () => {
    // Eine Automatik, die bei jeder ruhigen Sekunde zurückschaltet und
    // beim nächsten Ballon wieder herunter, flackert — und ein
    // flackerndes Fenster ist schlimmer als ein schlichtes.
    expect(decideLevel(flat(5), "plain")).toBe("plain");
    expect(decideLevel(flat(5), "reduced")).toBe("reduced");
  });
});

describe("zwei Messrunden", () => {
  it("arbeiten sich von „voll“ bis „schlicht“ durch, wenn eine Stufe nicht reicht", () => {
    // Der Fall, für den die zweite Runde existiert: eine Bildzeit knapp
    // über dem Budget stuft nur um eine Stufe herunter. Reicht die
    // nicht, bliebe das Gerät ohne Nachmessung dauerhaft zu langsam —
    // und der Nutzer säße vor einer zähen App, die ihm gerade erst
    // versichert hat, sie habe sich angepasst.
    const zuLangsam = flat(FRAME_BUDGET_MS + 6);
    const runde1 = decideLevel(zuLangsam, "full");
    expect(runde1).toBe("reduced");
    const runde2 = decideLevel(zuLangsam, runde1);
    expect(runde2).toBe("plain");
  });

  it("brechen ab, sobald eine Stufe reicht", () => {
    // Die Schleife in `App.tsx` endet, wenn `decideLevel` dieselbe
    // Stufe zurückgibt — ohne das würde sie bis „schlicht“ durchlaufen,
    // auch auf einem Gerät, dem „reduziert“ genügt.
    const runde1 = decideLevel(flat(FRAME_BUDGET_MS + 1), "full");
    expect(runde1).toBe("reduced");
    expect(decideLevel(flat(12), runde1)).toBe(runde1);
  });
});

describe("resolveLevel", () => {
  it("nimmt bei `auto` das Messergebnis", () => {
    expect(resolveLevel("auto", "plain")).toBe("plain");
  });

  it("lässt eine fest gewählte Stufe von der Messung unberührt", () => {
    // Wer „Voll" wählt, hat verstanden, was das kostet, und soll nicht
    // heimlich überstimmt werden.
    expect(resolveLevel("full", "plain")).toBe("full");
    expect(resolveLevel("plain", "full")).toBe("plain");
  });
});

describe("applyEffectLevel", () => {
  it("schreibt die Stufe dorthin, wo das Stylesheet sie liest", () => {
    const root = document.createElement("div");
    applyEffectLevel("reduced", root);
    expect(root.getAttribute("data-effects")).toBe("reduced");
  });
});

describe("probeFrameTimes", () => {
  it("verwirft die ersten beiden Bilder und liefert genau die angeforderte Anzahl", async () => {
    // Das erste Bild enthält die Zeit seit dem letzten fremden Bild,
    // das zweite oft noch das Einhängen der Last — beide sagen nichts
    // über die laufende Darstellung.
    let clock = 0;
    const times = await probeFrameTimes(5, {
      requestAnimationFrame: (callback) => {
        clock += 10;
        queueMicrotask(() => callback(clock));
        return 0;
      },
      now: () => clock,
    });
    expect(times).toHaveLength(5);
    expect(times).toEqual([10, 10, 10, 10, 10]);
  });

  it("hängt die Last vor der Messung ein und danach wieder aus", async () => {
    // Ohne bewegte Last misst die Messung den Ruhezustand, in dem alles
    // zwischengespeichert ist — dann sieht jedes Gerät schnell aus,
    // auch das, auf dem es ruckelt.
    const events: string[] = [];
    let clock = 0;
    await probeFrameTimes(3, {
      requestAnimationFrame: (callback) => {
        clock += 10;
        queueMicrotask(() => callback(clock));
        return 0;
      },
      now: () => clock,
      mountLoad: () => {
        events.push("ein");
        return () => events.push("aus");
      },
    });
    expect(events).toEqual(["ein", "aus"]);
  });

  it("erkennt eine zähe Darstellung als solche", async () => {
    let clock = 0;
    const times = await probeFrameTimes(MIN_SAMPLES, {
      requestAnimationFrame: (callback) => {
        clock += 50;
        queueMicrotask(() => callback(clock));
        return 0;
      },
      now: () => clock,
    });
    const level: AppliedEffectLevel = decideLevel(times, "full");
    expect(level).toBe("plain");
  });
});
