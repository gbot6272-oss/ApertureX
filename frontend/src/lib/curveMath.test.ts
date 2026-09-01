import { describe, expect, it } from "vitest";

import { evaluateCurveChannel, evaluateParametricCurve, evaluatePointsCurve } from "./curveMath";

describe("evaluatePointsCurve", () => {
  it("is the identity for the neutral two-point curve", () => {
    const identity = [
      { input: 0, output: 0 },
      { input: 1, output: 1 },
    ];
    for (const x of [0, 0.25, 0.5, 0.75, 1]) {
      expect(evaluatePointsCurve(identity, x)).toBeCloseTo(x);
    }
  });

  it("raises the midpoint toward its control point", () => {
    const points = [
      { input: 0, output: 0 },
      { input: 0.5, output: 0.7 },
      { input: 1, output: 1 },
    ];
    expect(evaluatePointsCurve(points, 0.5)).toBeCloseTo(0.7, 1);
  });

  it("stays monotonic for a steep local rise (Fritsch-Carlson, no overshoot)", () => {
    const points = [
      { input: 0, output: 0 },
      { input: 0.3, output: 0.3 },
      { input: 0.35, output: 0.9 },
      { input: 1, output: 1 },
    ];
    let previous = -Infinity;
    for (let i = 0; i <= 100; i++) {
      const value = evaluatePointsCurve(points, i / 100);
      expect(value).toBeGreaterThanOrEqual(previous - 1e-6);
      previous = value;
    }
  });

  it("clamps flat beyond the outermost control points", () => {
    const points = [
      { input: 0.2, output: 0.3 },
      { input: 0.8, output: 0.6 },
    ];
    expect(evaluatePointsCurve(points, 0)).toBeCloseTo(0.3);
    expect(evaluatePointsCurve(points, 1)).toBeCloseTo(0.6);
  });
});

describe("evaluateParametricCurve", () => {
  it("is the identity when all four regions are neutral", () => {
    for (const x of [0, 0.3, 0.6, 1]) {
      expect(evaluateParametricCurve(0, 0, 0, 0, x)).toBeCloseTo(x);
    }
  });

  it("lifts the shadow region more than the highlight region for a positive shadows value", () => {
    const shadowDelta = evaluateParametricCurve(50, 0, 0, 0, 0.05) - 0.05;
    const highlightDelta = evaluateParametricCurve(50, 0, 0, 0, 0.95) - 0.95;
    expect(shadowDelta).toBeGreaterThan(highlightDelta);
  });
});

describe("evaluateCurveChannel", () => {
  it("dispatches to the points evaluator", () => {
    const channel = { kind: "Points" as const, points: [{ input: 0, output: 0 }, { input: 1, output: 1 }] };
    expect(evaluateCurveChannel(channel, 0.4)).toBeCloseTo(0.4);
  });

  it("dispatches to the parametric evaluator", () => {
    const channel = { kind: "Parametric" as const, shadows: 0, darks: 0, lights: 0, highlights: 0 };
    expect(evaluateCurveChannel(channel, 0.4)).toBeCloseTo(0.4);
  });
});
