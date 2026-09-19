import type { PhotoDto } from "./tauri";

/**
 * Sortierung nach beliebigem Feld (Phase 3, Schritt 8.3, siehe
 * `DECISIONS.md` ADR-0027) — bewusst client-seitig statt als weiterer
 * `ORDER BY`-Parameter durch die verschiedenen Backend-Abfragen
 * durchgereicht: die komplette Fotoliste ist wegen der Virtualisierung
 * (Raster/Filmstreifen, siehe `PLAN.md` Phase 3 Schritt 7) ohnehin schon im
 * Speicher, serverseitiges Sortieren brächte hier keinen Zusatznutzen.
 */
/** `"manual"` (Phase 33 F8) ist kein Feld am Foto, sondern die
 * gespeicherte Reihenfolge einer Sammlung — [`sortPhotos`] lässt die
 * Liste dann unverändert, weil sie schon in der richtigen Reihenfolge
 * aus dem Katalog kommt (`collection_photos.position`). Nur innerhalb
 * einer Sammlung sinnvoll: ein Ordner hat die Reihenfolge des
 * Dateisystems, und eine zweite daneben zu erfinden hieße, dafür eine
 * Tabelle anzulegen, nach der niemand gefragt hat. */
export type SortField = "filename" | "captured_at" | "rating" | "file_size" | "camera_model" | "manual";
export type SortDirection = "asc" | "desc";

const FIELD_LABELS: Record<SortField, string> = {
  filename: "Dateiname",
  captured_at: "Aufnahmedatum",
  rating: "Bewertung",
  file_size: "Dateigröße",
  camera_model: "Kameramodell",
  manual: "Manuell (Sammlung)",
};

export const SORT_FIELDS: SortField[] = ["filename", "captured_at", "rating", "file_size", "camera_model", "manual"];

export function sortFieldLabel(field: SortField): string {
  return FIELD_LABELS[field];
}

/** Liest den zu vergleichenden Rohwert für `field` aus `photo` — `null`
 * (fehlender Wert) sortiert unabhängig von `direction` immer ans Ende. */
function sortKey(photo: PhotoDto, field: SortField): string | number | null {
  // `manual` erreicht diese Funktion nie — `sortPhotos` fängt es vorher
  // ab, weil es kein Feld am Foto ist.
  switch (field) {
    case "filename":
      return photo.filename;
    case "captured_at":
      return photo.captured_at;
    case "rating":
      return photo.rating;
    case "file_size":
      return photo.file_size;
    case "camera_model":
      return photo.camera_model;
    case "manual":
      // Unerreichbar (siehe oben) — aber TypeScript will den Zweig, und
      // `null` wäre hier die harmloseste Antwort: alles landete am Ende,
      // die Reihenfolge bliebe stabil.
      return null;
  }
}

/** Sortiert eine Kopie von `photos` nach `field`/`direction`. Fehlende
 * Werte (`null`) landen immer am Ende der Liste, unabhängig von
 * `direction` — ein unbewertetes Foto ist bei "Bewertung absteigend" nicht
 * plötzlich "höher" als ein 5-Sterne-Foto, nur weil `null` numerisch
 * kleiner als jede Zahl wäre. */
export function sortPhotos(photos: PhotoDto[], field: SortField, direction: SortDirection): PhotoDto[] {
  // Die manuelle Reihenfolge steht bereits in der Liste — und zwar auch
  // ihre Richtung: ein „absteigend manuell" gibt es nicht, das wäre eine
  // zweite, unsichtbare Reihenfolge über der von Hand gelegten.
  if (field === "manual") return photos;
  const sign = direction === "asc" ? 1 : -1;
  return [...photos].sort((a, b) => {
    const keyA = sortKey(a, field);
    const keyB = sortKey(b, field);
    if (keyA === null && keyB === null) return 0;
    if (keyA === null) return 1;
    if (keyB === null) return -1;
    if (keyA < keyB) return -1 * sign;
    if (keyA > keyB) return 1 * sign;
    return 0;
  });
}
