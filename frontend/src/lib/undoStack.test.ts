import { describe, expect, it, vi } from "vitest";

import { emptyUndoStacks, pushUndo, redo, undo } from "./undoStack";
import type { UndoEntry } from "./undoStack";

function dummyEntry(label: string): UndoEntry & { undoCalls: number; redoCalls: number } {
  const entry = {
    label,
    undoCalls: 0,
    redoCalls: 0,
    undo: vi.fn(async () => {
      entry.undoCalls += 1;
    }),
    redo: vi.fn(async () => {
      entry.redoCalls += 1;
    }),
  };
  return entry;
}

describe("undoStack", () => {
  it("starts empty", () => {
    expect(emptyUndoStacks()).toEqual({ undoStack: [], redoStack: [] });
  });

  it("pushUndo appends to the undo stack and clears the redo stack", () => {
    const entryA = dummyEntry("a");
    const entryB = dummyEntry("b");
    let stacks = pushUndo(emptyUndoStacks(), entryA);
    stacks = { ...stacks, redoStack: [dummyEntry("stale-future")] };
    stacks = pushUndo(stacks, entryB);

    expect(stacks.undoStack).toEqual([entryA, entryB]);
    expect(stacks.redoStack).toEqual([]);
  });

  it("pushUndo does not mutate the stacks it was given", () => {
    const original = emptyUndoStacks();
    pushUndo(original, dummyEntry("a"));
    expect(original).toEqual({ undoStack: [], redoStack: [] });
  });

  it("undo runs the entry's undo callback and moves it to the redo stack", async () => {
    const entry = dummyEntry("a");
    const stacks = pushUndo(emptyUndoStacks(), entry);

    const next = await undo(stacks);

    expect(entry.undoCalls).toBe(1);
    expect(next.undoStack).toEqual([]);
    expect(next.redoStack).toEqual([entry]);
  });

  it("undo on an empty stack is a no-op", async () => {
    const stacks = emptyUndoStacks();
    const next = await undo(stacks);
    expect(next).toEqual(stacks);
  });

  it("redo runs the entry's redo callback and moves it back to the undo stack", async () => {
    const entry = dummyEntry("a");
    const afterUndo = await undo(pushUndo(emptyUndoStacks(), entry));

    const next = await redo(afterUndo);

    expect(entry.redoCalls).toBe(1);
    expect(next.undoStack).toEqual([entry]);
    expect(next.redoStack).toEqual([]);
  });

  it("redo on an empty stack is a no-op", async () => {
    const stacks = emptyUndoStacks();
    const next = await redo(stacks);
    expect(next).toEqual(stacks);
  });

  it("undo then redo restores multiple entries in the correct order", async () => {
    const entryA = dummyEntry("a");
    const entryB = dummyEntry("b");
    let stacks = pushUndo(emptyUndoStacks(), entryA);
    stacks = pushUndo(stacks, entryB);

    stacks = await undo(stacks); // undoes b
    stacks = await undo(stacks); // undoes a
    expect(stacks.undoStack).toEqual([]);
    expect(stacks.redoStack).toEqual([entryB, entryA]);

    stacks = await redo(stacks); // redoes a
    stacks = await redo(stacks); // redoes b
    expect(stacks.undoStack).toEqual([entryA, entryB]);
    expect(stacks.redoStack).toEqual([]);
  });

  it("a new action after undo discards the redo future", async () => {
    const entryA = dummyEntry("a");
    const entryB = dummyEntry("b");
    let stacks = pushUndo(emptyUndoStacks(), entryA);
    stacks = await undo(stacks);
    expect(stacks.redoStack).toEqual([entryA]);

    stacks = pushUndo(stacks, entryB);
    expect(stacks.undoStack).toEqual([entryB]);
    expect(stacks.redoStack).toEqual([]);
  });
});
