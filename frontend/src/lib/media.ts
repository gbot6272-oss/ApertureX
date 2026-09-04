import { convertFileSrc } from "@tauri-apps/api/core";

import { encodeSoftProofSegment, type SoftProofSettings } from "./softProof";

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

/**
 * Baut die URL für die interaktive Entwickeln-Route (ab Phase 2, siehe
 * `crates/apx-app/src/protocol/route.rs`, `DECISIONS.md` ADR-0016).
 * `edlJson` ist die vollständige `EdlEnvelope`-JSON-Serialisierung (siehe
 * `lib/edl.ts`), nicht bloß ein Hash — die Route muss auch während des
 * Ziehens eines Reglers (noch nicht committet) live rendern können.
 * Antwort ist kein Bildformat, sondern ein 8-Byte-Breite/Höhe-Header +
 * rohes RGBA8 (siehe `hooks/useDevelopRender`).
 *
 * `softProof` (Phase 12 Schritt 6) hängt bei aktivem Soft-Proof ein
 * zusätzliches, base64url-kodiertes Segment an — der Server rendert dann
 * bereits farbmanagementkorrekt vor, siehe `lib/softProof.ts`s Moduldoku.
 */
export function developUrl(photoId: string, edlJson: string, maxEdge?: number, softProof: SoftProofSettings | null = null): string {
  const param = maxEdge === undefined ? "full" : String(maxEdge);
  const softProofSegment = encodeSoftProofSegment(softProof);
  return convertFileSrc(`develop/${photoId}/${param}/${softProofSegment}/${edlJson}`, "apx");
}

/**
 * Baut die URL für eine lokale Audiodatei (Diashow-Musiksynchronisation,
 * Phase 8 Schritt 4) — `absolutePath` kommt aus dem systemeigenen
 * Datei-Auswahldialog ({@link import("./tauri").pickFilePath}), nicht aus
 * freier Nutzereingabe (siehe `apx-app`s `protocol::route::ImageRequest::
 * Music`-Doku für den Vertrauensrahmen).
 */
export function musicUrl(absolutePath: string): string {
  return convertFileSrc(`music/${absolutePath}`, "apx");
}

/**
 * Baut die URL für die Original-Videodatei eines Katalog-Assets (Phase 16
 * Schritt 5, siehe `DECISIONS.md` ADR-0043) — anders als jede andere
 * Route hier mit echter HTTP-Range-Unterstützung serverseitig
 * (`protocol::mod::handle_video_request`), damit das `<video>`-Element
 * seeken kann.
 */
export function videoUrl(photoId: string): string {
  return convertFileSrc(`video/${photoId}`, "apx");
}
