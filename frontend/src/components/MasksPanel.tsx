import { BASIC_SLIDER_SPECS, MASK_SLIDER_SPECS, readBasicField } from "../lib/edl";
import { useAppStore } from "../store";
import { DevelopSlider } from "./DevelopSlider";

/** Die für Schritt 3 sichtbaren Grundeinstellungs-Regler pro Maske — eine
 * kleine, repräsentative Auswahl (Belichtung/Kontrast) statt aller zwölf,
 * um zu belegen, dass eine Maske ihre Werkzeuge tatsächlich anwendet. Die
 * vollständige Reglerabdeckung (alle sechs Werkzeugsektionen wie bei
 * einem Preset) ist Politur für Schritt 7. */
const MASK_BASIC_SLIDER_KEYS = ["exposure_ev", "contrast"];
const MASK_BASIC_SLIDER_SPECS = BASIC_SLIDER_SPECS.filter((spec) => MASK_BASIC_SLIDER_KEYS.includes(spec.key));

/**
 * Maskenverwaltung (Phase 6 Schritt 3, siehe `DECISIONS.md` ADR-0032) —
 * Liste vorhandener Masken, Anlegen neuer Masken (Linearer/Radialer
 * Verlauf — Pinsel folgt in Schritt 4, sobald seine Viewer-Interaktion
 * existiert), Auswahl zum Bearbeiten, kleine Reglerauswahl für die
 * ausgewählte Maske. Wie `DevelopPanel` nur sichtbar, während das
 * Entwickeln-Panel offen ist.
 */
export function MasksPanel() {
  const open = useAppStore((s) => s.developPanelOpen);
  const masks = useAppStore((s) => s.developEdl.masks);
  const selectedMaskId = useAppStore((s) => s.selectedMaskId);
  const selectMask = useAppStore((s) => s.selectMask);
  const addMask = useAppStore((s) => s.addMask);
  const removeMask = useAppStore((s) => s.removeMask);
  const setMaskVisible = useAppStore((s) => s.setMaskVisible);
  const renameMask = useAppStore((s) => s.renameMask);
  const setMaskOpacity = useAppStore((s) => s.setMaskOpacity);
  const setMaskFeather = useAppStore((s) => s.setMaskFeather);
  const commitMaskDrag = useAppStore((s) => s.commitMaskDrag);
  const setMaskBasicField = useAppStore((s) => s.setMaskBasicField);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);

  if (!open) return null;

  const selectedMask = masks.find((m) => m.id === selectedMaskId) ?? null;

  function handleRename(maskId: string, currentName: string, event: React.MouseEvent) {
    event.stopPropagation();
    const name = window.prompt("Maske umbenennen", currentName);
    if (name) renameMask(maskId, name);
  }

  return (
    <aside className="flex w-64 shrink-0 flex-col gap-3 overflow-y-auto border-l border-border bg-bg-raised p-3" aria-label="Masken">
      <h2 className="text-sm font-semibold text-text-primary">Masken</h2>

      <div className="flex gap-1">
        <button
          type="button"
          onClick={() => addMask("LinearGradient")}
          disabled={!selectedPhotoId}
          className="flex-1 rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
        >
          + Linearer Verlauf
        </button>
        <button
          type="button"
          onClick={() => addMask("RadialGradient")}
          disabled={!selectedPhotoId}
          className="flex-1 rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
        >
          + Radialer Verlauf
        </button>
      </div>

      <ul className="flex flex-col gap-1">
        {masks.map((mask) => (
          <li
            key={mask.id}
            className={`flex items-center gap-1.5 rounded border px-2 py-1.5 text-sm ${
              mask.id === selectedMaskId ? "border-accent bg-accent/10" : "border-border"
            }`}
          >
            <button
              type="button"
              onClick={() => setMaskVisible(mask.id, !mask.visible)}
              aria-label={mask.visible ? `${mask.name} ausblenden` : `${mask.name} einblenden`}
              aria-pressed={mask.visible}
              className={`shrink-0 ${mask.visible ? "text-accent" : "text-text-muted"}`}
              title="Sichtbarkeit"
            >
              {mask.visible ? "👁" : "🚫"}
            </button>
            <button
              type="button"
              onClick={() => selectMask(mask.id === selectedMaskId ? null : mask.id)}
              className="min-w-0 flex-1 truncate text-left text-text-primary hover:underline"
            >
              {mask.name}
            </button>
            <span role="button" tabIndex={0} onClick={(event) => handleRename(mask.id, mask.name, event)} className="shrink-0 text-text-muted hover:text-accent" title="Umbenennen">
              ✎
            </span>
            <button
              type="button"
              onClick={() => removeMask(mask.id)}
              className="shrink-0 text-danger"
              aria-label={`${mask.name} löschen`}
            >
              ×
            </button>
          </li>
        ))}
        {masks.length === 0 && <li className="text-xs text-text-muted">Keine Masken vorhanden.</li>}
      </ul>

      {selectedMask && (
        <div className="flex flex-col gap-2 border-t border-border pt-2">
          <h3 className="text-xs font-medium text-text-secondary">{selectedMask.name}</h3>
          {MASK_SLIDER_SPECS.map((spec) => (
            <DevelopSlider
              key={spec.key}
              spec={spec}
              value={selectedMask[spec.key as "opacity" | "feather"]}
              onChange={(value) => (spec.key === "opacity" ? setMaskOpacity(selectedMask.id, value) : setMaskFeather(selectedMask.id, value))}
              onCommit={commitMaskDrag}
            />
          ))}
          <h4 className="text-xs font-medium text-text-secondary">Grundeinstellungen (Auswahl)</h4>
          {MASK_BASIC_SLIDER_SPECS.map((spec) => (
            <DevelopSlider
              key={spec.key}
              spec={spec}
              value={readBasicField(selectedMask.adjustments.basic, spec.key)}
              onChange={(value) => setMaskBasicField(selectedMask.id, spec.key, value)}
              onCommit={commitMaskDrag}
            />
          ))}
        </div>
      )}
    </aside>
  );
}
