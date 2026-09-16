import { describe, expect, it } from "vitest";

import { buildCalendar, buildMonth, countByDay, densityLevel, localDayKey } from "./calendarGrid";

describe("localDayKey", () => {
  it("schreibt Jahr, Monat und Tag zweistellig aufgefüllt", () => {
    expect(localDayKey(new Date(2024, 0, 5))).toBe("2024-01-05");
    expect(localDayKey(new Date(2024, 11, 31))).toBe("2024-12-31");
  });
});

describe("countByDay", () => {
  it("zählt mehrere Aufnahmen desselben Tages zusammen", () => {
    const day = localDayKey(new Date(2024, 5, 1));
    const counts = countByDay([
      new Date(2024, 5, 1, 9).toISOString(),
      new Date(2024, 5, 1, 18).toISOString(),
      new Date(2024, 5, 2, 9).toISOString(),
    ]);
    expect(counts.find((c) => c.day === day)?.count).toBe(2);
  });

  it("überspringt Fotos ohne Aufnahmedatum, statt ein Datum zu raten", () => {
    // Ein geratenes Datum wäre im Kalender nicht als solches erkennbar
    // und würde den Tag verfälschen, an dem wirklich fotografiert wurde.
    expect(countByDay([null, undefined, ""])).toEqual([]);
  });

  it("überspringt unlesbare Zeitstempel", () => {
    expect(countByDay(["kein Datum", "2024-13-45T99:99:99Z"])).toEqual([]);
  });

  it("gibt die Tage aufsteigend zurück", () => {
    const counts = countByDay([
      new Date(2024, 5, 9).toISOString(),
      new Date(2024, 0, 2).toISOString(),
      new Date(2024, 2, 7).toISOString(),
    ]);
    expect(counts.map((c) => c.day)).toEqual([...counts.map((c) => c.day)].sort());
  });

  it("gruppiert nach LOKALEM Kalendertag", () => {
    // Nach UTC gruppiert landete eine Abendaufnahme aus Mitteleuropa im
    // Sommer schon am Folgetag — für den Fotografen derselbe Abend.
    const evening = new Date(2024, 6, 15, 23, 30);
    expect(countByDay([evening.toISOString()])[0]!.day).toBe("2024-07-15");
  });
});

describe("buildMonth", () => {
  it("füllt auf volle Wochen auf", () => {
    const month = buildMonth(2024, 2, new Map());
    expect(month.cells.length % 7).toBe(0);
  });

  it("beginnt die Woche am Montag", () => {
    // Der 1. September 2024 war ein Sonntag: sechs Füllzellen davor.
    const month = buildMonth(2024, 9, new Map());
    const leading = month.cells.findIndex((cell) => cell.dayOfMonth === 1);
    expect(leading).toBe(6);
  });

  it("kennt die Schaltjahr-Länge des Februars", () => {
    const leap = buildMonth(2024, 2, new Map());
    const normal = buildMonth(2023, 2, new Map());
    expect(leap.cells.filter((c) => c.dayOfMonth !== null)).toHaveLength(29);
    expect(normal.cells.filter((c) => c.dayOfMonth !== null)).toHaveLength(28);
  });

  it("summiert die Aufnahmen des Monats", () => {
    const counts = new Map([
      ["2024-03-01", 3],
      ["2024-03-15", 7],
      ["2024-04-01", 100],
    ]);
    expect(buildMonth(2024, 3, counts).total).toBe(10);
  });
});

describe("buildCalendar", () => {
  it("liefert für einen leeren Katalog nichts", () => {
    expect(buildCalendar([])).toEqual([]);
  });

  it("zeigt auch die leeren Monate dazwischen", () => {
    // Ein Kalender, der von März direkt auf Juni springt, sieht aus wie
    // ein lückenloser Zeitraum. Die Pause IST oft die Information.
    const months = buildCalendar([
      { day: "2024-03-10", count: 2 },
      { day: "2024-06-04", count: 5 },
    ]);
    expect(months.map((m) => m.month)).toEqual([3, 4, 5, 6]);
    expect(months[1]!.total).toBe(0);
  });

  it("läuft über einen Jahreswechsel", () => {
    const months = buildCalendar([
      { day: "2023-11-20", count: 1 },
      { day: "2024-01-05", count: 1 },
    ]);
    expect(months).toHaveLength(3);
    expect(months[0]!.year).toBe(2023);
    expect(months[2]!.year).toBe(2024);
  });

  it("friert bei unsinnig weit auseinander liegenden Daten nicht ein", () => {
    // Ein Foto von 1899 und eines von 2999 ergäbe sonst 13 000 Monate.
    const months = buildCalendar([
      { day: "1899-01-01", count: 1 },
      { day: "2999-12-31", count: 1 },
    ]);
    expect(months.length).toBeLessThanOrEqual(1200);
  });
});

describe("densityLevel", () => {
  it("gibt leeren Tagen die Stufe null", () => {
    expect(densityLevel(0, 100)).toBe(0);
  });

  it("gibt dem stärksten Tag die höchste Stufe", () => {
    expect(densityLevel(100, 100)).toBe(4);
  });

  it("misst relativ, nicht absolut", () => {
    // Wer an einem Tag 12 Fotos macht, soll denselben Kontrast sehen
    // wie jemand mit 1200 an seinem stärksten Tag.
    expect(densityLevel(12, 12)).toBe(densityLevel(1200, 1200));
    expect(densityLevel(6, 12)).toBe(densityLevel(600, 1200));
  });

  it("kommt mit einem Höchstwert von null klar", () => {
    expect(densityLevel(5, 0)).toBe(0);
  });
});
