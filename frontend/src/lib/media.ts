import { convertFileSrc } from "@tauri-apps/api/core";

/**
 * Baut die URL für den `apx://`-Custom-Protokoll-Handler (siehe
 * `crates/apx-app/src/protocol` und `DECISIONS.md` ADR-0009).
 *
 * Wichtig: `convertFileSrc` kodiert den *gesamten* übergebenen String als
 * ein einziges Pfadsegment — deshalb wird hier kein echter Query-String
 * gebaut (`?level=0`), sondern alles in einem `/`-getrennten Segment
 * übergeben, das der Rust-Handler selbst wieder aufteilt. Das
 * funktioniert identisch auf macOS, Linux und Windows, ohne dass das
 * Frontend die Plattform kennen müsste.
 */

export type PreviewLevel = 0 | 1 | 2; // 0=Thumbnail, 1=Standard, 2=Full

export function previewUrl(photoId: string, level: PreviewLevel = 0): string {
  return convertFileSrc(`preview/${photoId}/${level}`, "apx");
}

export function imageUrl(photoId: string, maxEdge?: number): string {
  const param = maxEdge === undefined ? "full" : String(maxEdge);
  return convertFileSrc(`image/${photoId}/${param}`, "apx");
}
