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
