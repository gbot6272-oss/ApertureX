/** Letztes Pfadsegment als Anzeigename für einen Ordner. */
export function folderLabel(path: string): string {
  const parts = path.split(/[/\\]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

/** Belichtungszeit als "2s" oder "1/125s" statt Dezimalsekunden. */
export function formatShutter(seconds: number): string {
  if (seconds >= 1) return `${seconds}s`;
  return `1/${Math.round(1 / seconds)}s`;
}

/** Dateigröße als "1.2 MB" statt Rohbytes — von `StatsCacheDialog.tsx`
 * und `CatalogDialog.tsx` (Phase 13 Schritt 6) gemeinsam genutzt. */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes;
  let unitIndex = -1;
  do {
    value /= 1024;
    unitIndex += 1;
  } while (value >= 1024 && unitIndex < units.length - 1);
  return `${value.toFixed(1)} ${units[unitIndex]}`;
}
