import { describe, expect, it } from "vitest";

import type { PhotoDto } from "../lib/tauri";
import { resolveSelectionMode, selectActivePhotos } from "./index";
import type { AppStore } from "./index";

function photo(id: string): PhotoDto {
  return {
    id,
    filename: `${id}.cr2`,
    width: null,
    height: null,
    camera_make: null,
    camera_model: null,
    lens: null,
    iso: null,
    aperture: null,
    shutter: null,
    focal_length: null,
    captured_at: null,
    missing: false,
    rating: 0,
    flag: 0,
    color_label: null,
  };
}

/** Nur die von `selectActivePhotos` gelesenen Felder sind für diesen Test
 * relevant — der Rest von `AppStore` wird nie aufgerufen, deshalb genügt
 * ein partieller Stub statt jedes Slice vollständig nachzubauen. */
function makeState(partial: Partial<AppStore>): AppStore {
  return partial as AppStore;
}

describe("resolveSelectionMode", () => {
  it("returns replace when no modifier is held", () => {
    expect(resolveSelectionMode({ ctrlKey: false, metaKey: false, shiftKey: false })).toBe("replace");
  });

  it("returns toggle for ctrl/cmd-click", () => {
    expect(resolveSelectionMode({ ctrlKey: true, metaKey: false, shiftKey: false })).toBe("toggle");
    expect(resolveSelectionMode({ ctrlKey: false, metaKey: true, shiftKey: false })).toBe("toggle");
  });

  it("returns range for shift-click, even combined with ctrl", () => {
    expect(resolveSelectionMode({ ctrlKey: false, metaKey: false, shiftKey: true })).toBe("range");
    expect(resolveSelectionMode({ ctrlKey: true, metaKey: false, shiftKey: true })).toBe("range");
  });
});

describe("selectActivePhotos", () => {
  it("falls back to an empty list when nothing is selected", () => {
    const state = makeState({ libraryResults: null, selectedCollectionId: null, selectedFolderId: null });
    expect(selectActivePhotos(state)).toEqual([]);
  });

  it("shows the selected folder's photos", () => {
    const state = makeState({
      libraryResults: null,
      selectedCollectionId: null,
      selectedFolderId: "f1",
      photosByFolder: { f1: [photo("a")] },
    });
    expect(selectActivePhotos(state).map((p) => p.id)).toEqual(["a"]);
  });

  it("prefers the selected collection over the selected folder", () => {
    const state = makeState({
      libraryResults: null,
      selectedCollectionId: "c1",
      collectionPhotos: { c1: [photo("b")] },
      selectedFolderId: "f1",
      photosByFolder: { f1: [photo("a")] },
    });
    expect(selectActivePhotos(state).map((p) => p.id)).toEqual(["b"]);
  });

  it("prefers active search/filter results over folder and collection", () => {
    const state = makeState({
      libraryResults: [photo("search-hit")],
      selectedCollectionId: "c1",
      collectionPhotos: { c1: [photo("b")] },
      selectedFolderId: "f1",
      photosByFolder: { f1: [photo("a")] },
    });
    expect(selectActivePhotos(state).map((p) => p.id)).toEqual(["search-hit"]);
  });

  it("treats an empty search result as a real (empty) result, not as 'inactive'", () => {
    const state = makeState({
      libraryResults: [],
      selectedFolderId: "f1",
      photosByFolder: { f1: [photo("a")] },
    });
    expect(selectActivePhotos(state)).toEqual([]);
  });
});
