import { describe, expect, it } from "vitest";

import { RENAME_PATTERN_TOKENS, previewRenamePattern } from "./renamePattern";

describe("previewRenamePattern", () => {
  it("replaces all known tokens with the sample values", () => {
    const result = previewRenamePattern("{date}_{seq}_{camera}_{original}", {
      date: new Date(2026, 2, 15),
      seq: 7,
      camera: "Canon EOS R5",
      originalStem: "IMG_0042",
    });
    expect(result).toBe("20260315_0007_Canon EOS R5_IMG_0042");
  });

  it("zero-pads the sequence number to four digits", () => {
    const result = previewRenamePattern("{seq}", { date: new Date(2026, 0, 1), seq: 3, camera: null, originalStem: "a" });
    expect(result.startsWith("0003")).toBe(true);
  });

  it("falls back to a placeholder camera name when missing", () => {
    const result = previewRenamePattern("{camera}", { date: new Date(2026, 0, 1), seq: 1, camera: null, originalStem: "a" });
    expect(result).toBe("Kamera");
  });

  it("sanitizes forbidden filename characters", () => {
    const result = previewRenamePattern("{camera}", { date: new Date(2026, 0, 1), seq: 1, camera: "Nikon Z9:Pro?", originalStem: "a" });
    expect(result).toBe("Nikon Z9_Pro_");
  });

  it("leaves unknown tokens untouched", () => {
    const result = previewRenamePattern("{unbekannt}", { date: new Date(2026, 0, 1), seq: 1, camera: null, originalStem: "a" });
    expect(result).toBe("{unbekannt}");
  });

  it("uses the built-in sample when no sample is passed", () => {
    expect(previewRenamePattern("{camera}")).toBe("Canon EOS R5");
  });
});

describe("RENAME_PATTERN_TOKENS", () => {
  it("lists exactly the four tokens the backend supports", () => {
    expect(RENAME_PATTERN_TOKENS.map((t) => t.token)).toEqual(["{date}", "{seq}", "{camera}", "{original}"]);
  });
});
