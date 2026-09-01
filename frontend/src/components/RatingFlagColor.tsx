/**
 * Kleine, wiederverwendbare Bewertungs-/Flaggen-/Farb-Widgets — sowohl in
 * Raster-Zellen (`GridView.tsx`, kompakt) als auch im Metadaten-Panel
 * (`MetadataPanel.tsx`, ausführlich) verwendet, siehe `PLAN.md` Phase 3,
 * Schritt 6. Alle drei sind reine, kontrollierte Komponenten — der
 * aufrufende Store-Zugriff (Stapel-Bearbeitung über die Mehrfachauswahl)
 * passiert außerhalb, siehe `store/index.ts`s `setPhotoRating`/
 * `setPhotoFlag`/`setPhotoColorLabel`.
 */

/** Muss mit `ALLOWED_COLOR_LABELS` in
 * `crates/apx-catalog/src/repository/photos.rs` übereinstimmen. */
export const COLOR_LABELS = ["red", "yellow", "green", "blue", "purple"] as const;

export const COLOR_SWATCH: Record<(typeof COLOR_LABELS)[number], string> = {
  red: "#e07a5f",
  yellow: "#e0c15f",
  green: "#7fb069",
  blue: "#5b9bd5",
  purple: "#9b7ed5",
};

interface RatingStarsProps {
  rating: number;
  onChange: (rating: number) => void;
  /** Kleinere Darstellung für Raster-Zellen statt des vollen Metadaten-Panels. */
  compact?: boolean;
}

export function RatingStars({ rating, onChange, compact = false }: RatingStarsProps) {
  return (
    <div className={`flex ${compact ? "gap-0" : "gap-0.5"}`} role="group" aria-label="Bewertung">
      {[1, 2, 3, 4, 5].map((value) => (
        <button
          key={value}
          type="button"
          onClick={(event) => {
            event.stopPropagation();
            // Erneutes Klicken auf den aktuellen Bewertungswert löscht ihn
            // wieder (Lightroom-Konvention) statt ihn nur neu zu setzen.
            onChange(rating === value ? 0 : value);
          }}
          aria-label={`${value} Sterne`}
          aria-pressed={rating >= value}
          className={`leading-none ${compact ? "text-[10px]" : "text-sm"} ${rating >= value ? "text-accent" : "text-text-muted hover:text-text-secondary"}`}
        >
          ★
        </button>
      ))}
    </div>
  );
}

interface FlagToggleProps {
  flag: number;
  onChange: (flag: number) => void;
  compact?: boolean;
}

export function FlagToggle({ flag, onChange, compact = false }: FlagToggleProps) {
  const size = compact ? "text-[10px] px-1" : "text-xs px-1.5 py-0.5";
  return (
    <div className="flex gap-1" role="group" aria-label="Flagge">
      <button
        type="button"
        onClick={(event) => {
          event.stopPropagation();
          onChange(flag === 1 ? 0 : 1);
        }}
        aria-label="Pick"
        aria-pressed={flag === 1}
        title="Pick (P)"
        className={`rounded ${size} ${flag === 1 ? "bg-accent/20 text-accent" : "text-text-muted hover:text-text-secondary"}`}
      >
        ✓
      </button>
      <button
        type="button"
        onClick={(event) => {
          event.stopPropagation();
          onChange(flag === -1 ? 0 : -1);
        }}
        aria-label="Reject"
        aria-pressed={flag === -1}
        title="Reject (X)"
        className={`rounded ${size} ${flag === -1 ? "bg-danger/20 text-danger" : "text-text-muted hover:text-text-secondary"}`}
      >
        ✕
      </button>
    </div>
  );
}

interface ColorLabelPickerProps {
  colorLabel: string | null;
  onChange: (colorLabel: string | null) => void;
  compact?: boolean;
}

export function ColorLabelPicker({ colorLabel, onChange, compact = false }: ColorLabelPickerProps) {
  const size = compact ? "h-2 w-2" : "h-3.5 w-3.5";
  return (
    <div className="flex gap-1" role="group" aria-label="Farbmarkierung">
      {COLOR_LABELS.map((color) => (
        <button
          key={color}
          type="button"
          onClick={(event) => {
            event.stopPropagation();
            onChange(colorLabel === color ? null : color);
          }}
          aria-label={color}
          aria-pressed={colorLabel === color}
          title={color}
          style={{ backgroundColor: COLOR_SWATCH[color] }}
          className={`rounded-full ${size} ${colorLabel === color ? "ring-2 ring-text-primary" : "opacity-60 hover:opacity-100"}`}
        />
      ))}
    </div>
  );
}
