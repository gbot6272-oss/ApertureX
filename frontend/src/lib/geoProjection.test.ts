import { describe, expect, it } from "vitest";

import { latLonToUnitSphere, limbDarkening, orthographicProject, projectLatLon, rotateSphere } from "./geoProjection";

describe("latLonToUnitSphere", () => {
  it("places (0, 0) on the near-facing point of the unit sphere", () => {
    const p = latLonToUnitSphere(0, 0);
    expect(p.x).toBeCloseTo(0);
    expect(p.y).toBeCloseTo(0);
    expect(p.z).toBeCloseTo(1);
  });

  it("places the north pole (90, *) on the +Y axis regardless of longitude", () => {
    const p = latLonToUnitSphere(90, 47);
    expect(p.x).toBeCloseTo(0);
    expect(p.y).toBeCloseTo(1);
    expect(p.z).toBeCloseTo(0);
  });

  it("every point lies on the unit sphere", () => {
    for (const [lat, lon] of [
      [10, 20],
      [-45, 170],
      [89, -30],
    ]) {
      const p = latLonToUnitSphere(lat as number, lon as number);
      const length = Math.sqrt(p.x * p.x + p.y * p.y + p.z * p.z);
      expect(length).toBeCloseTo(1);
    }
  });
});

describe("rotateSphere", () => {
  it("a zero rotation is the identity", () => {
    const p = latLonToUnitSphere(23, -55);
    const rotated = rotateSphere(p, { yaw: 0, pitch: 0 });
    expect(rotated.x).toBeCloseTo(p.x);
    expect(rotated.y).toBeCloseTo(p.y);
    expect(rotated.z).toBeCloseTo(p.z);
  });

  it("a 90° yaw rotates the front-facing point onto the +X axis", () => {
    const front = latLonToUnitSphere(0, 0);
    const rotated = rotateSphere(front, { yaw: 90, pitch: 0 });
    expect(rotated.x).toBeCloseTo(1);
    expect(rotated.z).toBeCloseTo(0);
  });

  it("a 180° yaw hides the front-facing point on the far side", () => {
    const front = latLonToUnitSphere(0, 0);
    const rotated = rotateSphere(front, { yaw: 180, pitch: 0 });
    expect(rotated.z).toBeCloseTo(-1);
  });

  it("preserves vector length (rotation, not scaling)", () => {
    const p = latLonToUnitSphere(31, 12);
    const rotated = rotateSphere(p, { yaw: 40, pitch: -25 });
    const length = Math.sqrt(rotated.x ** 2 + rotated.y ** 2 + rotated.z ** 2);
    expect(length).toBeCloseTo(1);
  });
});

describe("orthographicProject", () => {
  it("scales x/y by radius and flips y (screen coordinates grow downward)", () => {
    const projected = orthographicProject({ x: 0.5, y: 0.5, z: 0.5 }, 100);
    expect(projected.x).toBeCloseTo(50);
    expect(projected.y).toBeCloseTo(-50);
  });

  it("marks a point with z >= 0 as visible, z < 0 as hidden", () => {
    expect(orthographicProject({ x: 0, y: 0, z: 0.01 }, 100).visible).toBe(true);
    expect(orthographicProject({ x: 0, y: 0, z: 0 }, 100).visible).toBe(true);
    expect(orthographicProject({ x: 0, y: 0, z: -0.01 }, 100).visible).toBe(false);
  });
});

describe("projectLatLon", () => {
  it("the front-facing point projects to the screen center at no rotation", () => {
    const p = projectLatLon(0, 0, { yaw: 0, pitch: 0 }, 200);
    expect(p.x).toBeCloseTo(0);
    expect(p.y).toBeCloseTo(0);
    expect(p.visible).toBe(true);
  });

  it("the antipodal point is hidden (back of the globe)", () => {
    const p = projectLatLon(0, 180, { yaw: 0, pitch: 0 }, 200);
    expect(p.visible).toBe(false);
  });
});

describe("limbDarkening", () => {
  it("is zero at or behind the horizon", () => {
    expect(limbDarkening(0)).toBe(0);
    expect(limbDarkening(-0.5)).toBe(0);
  });

  it("equals z for a front-facing point", () => {
    expect(limbDarkening(0.7)).toBeCloseTo(0.7);
  });
});
