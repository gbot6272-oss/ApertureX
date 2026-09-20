import { describe, expect, it } from "vitest";

import { groupPhotosByStack, stacksInView } from "./stackGrouping";
import type { PhotoDto, StackDto } from "./tauri";

function photo(id: string): PhotoDto {
  return { id, filename: `${id}.CR3` } as PhotoDto;
}

function stack(id: string, photoIds: string[], cover: string | null = null): StackDto {
  return { id, name: null, cover_photo_id: cover, photo_ids: photoIds };
}

const A = photo("a");
const B = photo("b");
const C = photo("c");
const D = photo("d");

describe("groupPhotosByStack", () => {
  it("lässt Fotos ohne Stapel unverändert durch", () => {
    const entries = groupPhotosByStack([A, B], [], []);
    expect(entries.map((e) => e.photo.id)).toEqual(["a", "b"]);
    expect(entries.every((e) => e.stackId === undefined)).toBe(true);
  });

  it("fasst einen eingeklappten Stapel zu einer Kachel zusammen", () => {
    const entries = groupPhotosByStack([A, B, C], [stack("s1", ["a", "b"])], []);
    expect(entries.map((e) => e.photo.id)).toEqual(["a", "c"]);
    expect(entries[0]!.stackId).toBe("s1");
    expect(entries[0]!.stackSize).toBe(2);
  });

  it("zeigt aufgeklappt wieder alle Mitglieder", () => {
    const entries = groupPhotosByStack([A, B, C], [stack("s1", ["a", "b"])], ["s1"]);
    expect(entries.map((e) => e.photo.id)).toEqual(["a", "b", "c"]);
    expect(entries.every((e) => e.stackId === undefined)).toBe(true);
  });

  it("behält die Position des ersten sichtbaren Mitglieds", () => {
    // Das Einklappen darf das Raster nicht umsortieren.
    const entries = groupPhotosByStack([A, B, C, D], [stack("s1", ["b", "d"])], []);
    expect(entries.map((e) => e.photo.id)).toEqual(["a", "b", "c"]);
  });

  it("nimmt das hinterlegte Deckblatt", () => {
    const entries = groupPhotosByStack([A, B], [stack("s1", ["a", "b"], "b")], []);
    expect(entries[0]!.photo.id).toBe("b");
  });

  it("weicht aufs erste sichtbare Mitglied aus, wenn das Deckblatt weggefiltert ist", () => {
    // Ein Deckblatt zu zeigen, das im aufgeklappten Zustand gar nicht da
    // ist, wäre schlimmer als ein anderes Bild.
    const entries = groupPhotosByStack([A, B], [stack("s1", ["a", "b", "z"], "z")], []);
    expect(entries[0]!.photo.id).toBe("a");
  });

  it("zählt nur die sichtbaren Mitglieder", () => {
    // Die volle Zahl wäre ein Versprechen auf Fotos, die der Filter
    // gerade ausgeschlossen hat.
    const entries = groupPhotosByStack([A, B], [stack("s1", ["a", "b", "x", "y"])], []);
    expect(entries[0]!.stackSize).toBe(2);
  });

  it("zeigt einen Stapel mit nur einem sichtbaren Foto als gewöhnliche Kachel", () => {
    const entries = groupPhotosByStack([A], [stack("s1", ["a", "b"])], []);
    expect(entries).toHaveLength(1);
    expect(entries[0]!.stackId).toBeUndefined();
  });

  it("ordnet ein Foto in zwei Stapeln dem ersten zu", () => {
    const entries = groupPhotosByStack(
      [A, B, C],
      [stack("s1", ["a", "b"]), stack("s2", ["b", "c"])],
      [],
    );
    // b gehört zu s1; c bleibt für s2 allein übrig und ist damit kein
    // Stapel mehr.
    expect(entries.map((e) => e.photo.id)).toEqual(["a", "c"]);
    expect(entries[0]!.stackId).toBe("s1");
    expect(entries[1]!.stackId).toBeUndefined();
  });

  it("kommt mit einem leeren Raster zurecht", () => {
    expect(groupPhotosByStack([], [stack("s1", ["a"])], [])).toEqual([]);
  });

  it("kommt mit einem Stapel zurecht, von dem nichts sichtbar ist", () => {
    const entries = groupPhotosByStack([A], [stack("s1", ["x", "y"])], []);
    expect(entries.map((e) => e.photo.id)).toEqual(["a"]);
  });
});

describe("stacksInView", () => {
  it("meldet nur Stapel mit mindestens zwei sichtbaren Fotos", () => {
    const stacks = [stack("s1", ["a", "b"]), stack("s2", ["c", "x"]), stack("s3", ["y", "z"])];
    expect(stacksInView([A, B, C], stacks)).toEqual(["s1"]);
  });

  it("ist bei leerem Raster leer", () => {
    expect(stacksInView([], [stack("s1", ["a", "b"])])).toEqual([]);
  });
});
