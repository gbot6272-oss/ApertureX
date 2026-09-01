import type { PhotoDto } from "./tauri";

/**
 * Duplikat-Assistent (Phase 9 Schritt 1, siehe `DECISIONS.md` ADR-0032):
 * schlägt aus einer Gruppe (nahezu) identischer Fotos die "beste" Version
 * vor — höchste Auflösung zuerst, dann größere Dateigröße, dann höhere
 * Bewertung als Tiebreaker. Reine Heuristik, keine Inhaltsanalyse.
 */
export function suggestBestPhoto(photos: PhotoDto[]): PhotoDto | null {
  if (photos.length === 0) return null;
  return photos.reduce((best, candidate) => {
    const bestArea = (best.width ?? 0) * (best.height ?? 0);
    const candidateArea = (candidate.width ?? 0) * (candidate.height ?? 0);
    if (candidateArea !== bestArea) return candidateArea > bestArea ? candidate : best;
    if (candidate.file_size !== best.file_size) return candidate.file_size > best.file_size ? candidate : best;
    return candidate.rating > best.rating ? candidate : best;
  });
}
