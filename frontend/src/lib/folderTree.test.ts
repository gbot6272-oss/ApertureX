import { describe, expect, it } from "vitest";

import { buildChildrenByParent } from "./folderTree";
import type { FolderDto } from "./tauri";

function folder(id: string, parentId: string | null): FolderDto {
  return { id, path: `/${id}`, photo_count: 0, parent_id: parentId, missing: false };
}

describe("buildChildrenByParent", () => {
  it("puts folders without parent_id at the root", () => {
    const { roots, childrenOf } = buildChildrenByParent([folder("a", null), folder("b", null)]);
    expect(roots.map((f) => f.id)).toEqual(["a", "b"]);
    expect(childrenOf.size).toBe(0);
  });

  it("nests a folder under its parent", () => {
    const { roots, childrenOf } = buildChildrenByParent([folder("a", null), folder("b", "a")]);
    expect(roots.map((f) => f.id)).toEqual(["a"]);
    expect(childrenOf.get("a")?.map((f) => f.id)).toEqual(["b"]);
  });

  it("treats a folder whose parent_id points nowhere as a root instead of dropping it", () => {
    const { roots } = buildChildrenByParent([folder("orphan", "does-not-exist")]);
    expect(roots.map((f) => f.id)).toEqual(["orphan"]);
  });

  it("supports multiple levels of nesting", () => {
    const { roots, childrenOf } = buildChildrenByParent([
      folder("a", null),
      folder("b", "a"),
      folder("c", "b"),
    ]);
    expect(roots.map((f) => f.id)).toEqual(["a"]);
    expect(childrenOf.get("a")?.map((f) => f.id)).toEqual(["b"]);
    expect(childrenOf.get("b")?.map((f) => f.id)).toEqual(["c"]);
  });
});
