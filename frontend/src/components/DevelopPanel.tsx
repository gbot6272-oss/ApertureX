import { useEffect } from "react";

import { BASIC_SLIDER_SPECS, readBasicField } from "../lib/edl";
import { useAppStore } from "../store";
import { DevelopSlider } from "./DevelopSlider";

const WHITE_BALANCE_KEYS = new Set(["temp_shift_kelvin", "tint_shift"]);

/**
 * Das Entwickeln-Panel: die sieben Grundeinstellungs-Regler (Weißabgleich
 * zählt als einer, hat aber zwei Zahlenwerte — siehe `SPEC.md` §5) plus
 * Rückgängig/Wiederholen. Nur sichtbar, wenn `developPanelOpen` (siehe
 * `store/index.ts`, umgeschaltet über den "Entwickeln"-Knopf in
 * `Header.tsx`).
 *
 * Undo/Redo laufen direkt über `apx-catalog`s `edit_history` (siehe
 * `crates/apx-app/src/commands.rs`), nicht über eine separate
 * Frontend-Bibliothek wie ursprünglich in ADR-0018 vorgesehen — siehe die
 * Korrektur-Notiz dort: ein zusätzliches, lose synchronisiertes
 * Verlaufssystem im Frontend hätte keinen erkennbaren Vorteil gegenüber
 * der ohnehin schon vollständig getesteten Backend-Historie gebracht,
 * und Tauris IPC ist lokal schnell genug, um pro Undo/Redo-Klick einen
 * Roundtrip zu rechtfertigen.
 */
export function DevelopPanel() {
  const open = useAppStore((s) => s.developPanelOpen);
  const basic = useAppStore((s) => s.developBasic);
  const setBasicField = useAppStore((s) => s.setBasicField);
  const commitDevelopEdit = useAppStore((s) => s.commitDevelopEdit);
  const undoDevelop = useAppStore((s) => s.undoDevelop);
  const redoDevelop = useAppStore((s) => s.redoDevelop);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);

  useEffect(() => {
    if (!open) return;

    function handleKeyDown(event: KeyboardEvent) {
      const target = event.target as HTMLElement | null;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) return;
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "z") return;

      event.preventDefault();
      if (event.shiftKey) {
        void redoDevelop();
      } else {
        void undoDevelop();
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, undoDevelop, redoDevelop]);

  if (!open) return null;

  const whiteBalanceSpecs = BASIC_SLIDER_SPECS.filter((spec) => WHITE_BALANCE_KEYS.has(spec.key));
  const toneSpecs = BASIC_SLIDER_SPECS.filter((spec) => !WHITE_BALANCE_KEYS.has(spec.key));

  return (
    <aside className="flex w-72 shrink-0 flex-col gap-4 overflow-y-auto border-l border-border bg-bg-raised p-3" aria-label="Entwickeln">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-text-primary">Entwickeln</h2>
        <div className="flex gap-1">
          <button
            type="button"
            onClick={() => void undoDevelop()}
            disabled={!selectedPhotoId}
            aria-label="Rückgängig"
            title="Rückgängig (Strg/Cmd+Z)"
            className="rounded px-2 py-1 text-xs hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
          >
            ↶
          </button>
          <button
            type="button"
            onClick={() => void redoDevelop()}
            disabled={!selectedPhotoId}
            aria-label="Wiederholen"
            title="Wiederholen (Strg/Cmd+Umschalt+Z)"
            className="rounded px-2 py-1 text-xs hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
          >
            ↷
          </button>
        </div>
      </div>

      {!selectedPhotoId && <p className="text-xs text-text-muted">Kein Foto ausgewählt.</p>}

      {selectedPhotoId && (
        <>
          <fieldset className="flex flex-col gap-3">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Weißabgleich</legend>
            {whiteBalanceSpecs.map((spec) => (
              <DevelopSlider
                key={spec.key}
                spec={spec}
                value={readBasicField(basic, spec.key)}
                onChange={(value) => setBasicField(spec.key, value)}
                onCommit={() => void commitDevelopEdit()}
              />
            ))}
          </fieldset>

          <div className="flex flex-col gap-3">
            {toneSpecs.map((spec) => (
              <DevelopSlider
                key={spec.key}
                spec={spec}
                value={readBasicField(basic, spec.key)}
                onChange={(value) => setBasicField(spec.key, value)}
                onCommit={() => void commitDevelopEdit()}
              />
            ))}
          </div>
        </>
      )}
    </aside>
  );
}
