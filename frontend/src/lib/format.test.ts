import { describe, expect, it } from "vitest";

import { folderLabel, formatShutter } from "./format";

describe("folderLabel", () => {
  it("nimmt das letzte Segment eines Unix-Pfads", () => {
    expect(folderLabel("/home/user/Fotos/2024")).toBe("2024");
  });

  it("nimmt das letzte Segment eines Windows-Pfads", () => {
    expect(folderLabel("C:\\Users\\me\\Fotos\\2024")).toBe("2024");
  });

  it("ignoriert einen abschliessenden Trenner", () => {
    expect(folderLabel("/home/user/Fotos/2024/")).toBe("2024");
  });

  it("gibt den ganzen Pfad zurueck, wenn kein Trenner vorkommt", () => {
    expect(folderLabel("Fotos")).toBe("Fotos");
  });
});

describe("formatShutter", () => {
  it("zeigt lange Zeiten als Sekunden", () => {
    expect(formatShutter(2)).toBe("2s");
  });

  it("zeigt kurze Zeiten als Bruch", () => {
    expect(formatShutter(1 / 125)).toBe("1/125s");
  });

  it("rundet den Nenner bei nicht ganzzahligen Kehrwerten", () => {
    expect(formatShutter(0.01)).toBe("1/100s");
  });
});
