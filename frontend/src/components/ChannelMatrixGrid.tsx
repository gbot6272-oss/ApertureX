/**
 * Das 3×3-Gitter für die Kanalmatrix (Phase 30 Punkt 8, siehe
 * `DECISIONS.md` ADR-0060).
 *
 * Phase 28 hat die Matrix eingeführt, aber nur vier Ein-Klick-Vorgaben
 * dafür gebaut — die neun Zahlen waren überhaupt nicht erreichbar.
 * Hier stehen sie mit Zeilen- und Spaltenköpfen („Ausgabe Rot bekommt
 * so viel Eingabe Grün"), damit erkennbar ist, was eine Zelle bedeutet;
 * ein nacktes Zahlenraster wäre kaum lesbar.
 *
 * Der Streifen darunter zeigt sechs Testfarben durch die aktuelle
 * Matrix — dieselbe Rechnung wie in `stages::interactive`s
 * Schwesterstufe, nur im Kleinen und ohne Umweg übers Rendern.
 */

const CHANNEL_LABELS = ["Rot", "Grün", "Blau"] as const;

/** Sechs Testfarben, an denen sich eine Kanalmatrix ablesen lässt:
 * Primär-, Sekundärfarben und ein Grau. */
const SWATCHES: readonly [number, number, number][] = [
  [0.9, 0.15, 0.15],
  [0.15, 0.75, 0.25],
  [0.2, 0.35, 0.95],
  [0.95, 0.8, 0.2],
  [0.6, 0.3, 0.8],
  [0.55, 0.55, 0.55],
];

export interface ChannelMatrixGridProps {
  matrix: readonly number[];
  onChange: (index: number, value: number) => void;
  onCommit: () => void;
}

function applyMatrix(matrix: readonly number[], rgb: readonly [number, number, number]): string {
  const out = [0, 1, 2].map((row) => {
    const value =
      (matrix[row * 3] ?? 0) * rgb[0] +
      (matrix[row * 3 + 1] ?? 0) * rgb[1] +
      (matrix[row * 3 + 2] ?? 0) * rgb[2];
    return Math.round(Math.min(1, Math.max(0, value)) * 255);
  });
  return `rgb(${out.join(" ")})`;
}

export function ChannelMatrixGrid({ matrix, onChange, onCommit }: ChannelMatrixGridProps) {
  return (
    <div className="flex flex-col gap-2" data-testid="channel-matrix-grid">
      <div className="grid grid-cols-[auto_repeat(3,1fr)] gap-1 text-[11px]">
        <span />
        {CHANNEL_LABELS.map((label) => (
          <span key={label} className="text-center text-text-muted">
            aus {label}
          </span>
        ))}
        {CHANNEL_LABELS.map((rowLabel, row) => (
          <>
            <span key={`${rowLabel}-head`} className="self-center pr-1 text-text-muted">
              {rowLabel} →
            </span>
            {CHANNEL_LABELS.map((colLabel, col) => {
              const index = row * 3 + col;
              return (
                <input
                  key={`${rowLabel}-${colLabel}`}
                  type="number"
                  step={0.05}
                  min={-2}
                  max={2}
                  aria-label={`${rowLabel} aus ${colLabel}`}
                  value={Number((matrix[index] ?? 0).toFixed(2))}
                  onChange={(event) => onChange(index, Number(event.target.value))}
                  onBlur={onCommit}
                  className="w-full rounded border border-border bg-bg-raised px-1 py-0.5 text-center text-xs text-text-primary outline-none transition-[border-color] duration-[var(--duration-fast)] focus:border-accent"
                />
              );
            })}
          </>
        ))}
      </div>
      <div className="flex gap-1" aria-label="Vorschau der Matrix an Testfarben">
        {SWATCHES.map((rgb, index) => (
          <div key={index} className="flex flex-1 flex-col">
            <span
              className="h-4 rounded-t"
              style={{ backgroundColor: `rgb(${rgb.map((c) => Math.round(c * 255)).join(" ")})` }}
            />
            <span className="h-4 rounded-b" style={{ backgroundColor: applyMatrix(matrix, rgb) }} />
          </div>
        ))}
      </div>
    </div>
  );
}
