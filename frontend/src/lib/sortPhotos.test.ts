import { describe, expect, it } from "vitest";

import type { PhotoDto } from "./tauri";
import { sortPhotos } from "./sortPhotos";

function photo(overrides: Partial<PhotoDto> & { id: string }): PhotoDto {
  return {
    filename: "z.cr2",
    file_size: 0,
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
    ...overrides,
  };
}

describe("sortPhotos", () => {
  it("sorts by filename ascending by default direction", () => {
    const photos = [photo({ id: "b", filename: "b.cr2" }), photo({ id: "a", filename: "a.cr2" })];
    expect(sortPhotos(photos, "filename", "asc").map((p) => p.id)).toEqual(["a", "b"]);
  });

  it("reverses order for descending direction", () => {
    const photos = [photo({ id: "a", filename: "a.cr2" }), photo({ id: "b", filename: "b.cr2" })];
    expect(sortPhotos(photos, "filename", "desc").map((p) => p.id)).toEqual(["b", "a"]);
  });

  it("sorts by rating", () => {
    const photos = [photo({ id: "low", rating: 1 }), photo({ id: "high", rating: 5 }), photo({ id: "mid", rating: 3 })];
    expect(sortPhotos(photos, "rating", "asc").map((p) => p.id)).toEqual(["low", "mid", "high"]);
    expect(sortPhotos(photos, "rating", "desc").map((p) => p.id)).toEqual(["high", "mid", "low"]);
  });

  it("sorts by file size", () => {
    const photos = [photo({ id: "big", file_size: 3000 }), photo({ id: "small", file_size: 100 })];
    expect(sortPhotos(photos, "file_size", "asc").map((p) => p.id)).toEqual(["small", "big"]);
  });

  it("sorts by captured_at", () => {
    const photos = [
      photo({ id: "later", captured_at: "2024-06-02T10:00:00Z" }),
      photo({ id: "earlier", captured_at: "2024-06-01T10:00:00Z" }),
    ];
    expect(sortPhotos(photos, "captured_at", "asc").map((p) => p.id)).toEqual(["earlier", "later"]);
  });

  it("sorts by camera_model", () => {
    const photos = [photo({ id: "z9", camera_model: "Z9" }), photo({ id: "r5", camera_model: "EOS R5" })];
    expect(sortPhotos(photos, "camera_model", "asc").map((p) => p.id)).toEqual(["r5", "z9"]);
  });

  it("always sorts missing values last, regardless of direction", () => {
    const photos = [
      photo({ id: "no-model", camera_model: null }),
      photo({ id: "has-model", camera_model: "EOS R5" }),
    ];
    expect(sortPhotos(photos, "camera_model", "asc").map((p) => p.id)).toEqual(["has-model", "no-model"]);
    expect(sortPhotos(photos, "camera_model", "desc").map((p) => p.id)).toEqual(["has-model", "no-model"]);
  });

  it("does not mutate the input array", () => {
    const photos = [photo({ id: "b", filename: "b.cr2" }), photo({ id: "a", filename: "a.cr2" })];
    const original = [...photos];
    sortPhotos(photos, "filename", "asc");
    expect(photos).toEqual(original);
  });
});
