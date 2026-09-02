import { useState } from "react";

import type { FilterCriteriaDto } from "../lib/tauri";
import { useAppStore } from "../store";

interface BatchConsoleDialogProps {
  open: boolean;
  onClose: () => void;
}

type ActionKind = "SetRating" | "SetColorLabel" | "AddKeyword";

/**
 * Stapelverarbeitungs-Konsole (Phase 11 Schritt 9, siehe `PLAN.md`/
 * `DECISIONS.md` ADR-0038 — die in ADR-0036 explizit benannte Lücke aus
 * Phase 9). Eine Regel = eine {@link FilterCriteriaDto}-Auswahl
 * (dieselben Felder wie das normale Filter-Panel, siehe `FilterBar.tsx`)
 * + eine Aktion (Bewertung setzen/Schlagwort hinzufügen/Farbmarkierung
 * setzen). Trockenlauf (Vorschau) zeigt die betroffenen Fotos, ohne zu
 * schreiben; Ausführen schreibt und journalisiert; danach steht ein
 * Rückgängig-Knopf für genau diesen einen Stapel zur Verfügung — echtes
 * Batch-Undo, siehe `apx_catalog::repository::batch`s Moduldoku.
 */
export function BatchConsoleDialog({ open, onClose }: BatchConsoleDialogProps) {
  const batchPreview = useAppStore((s) => s.batchPreview);
  const batchPreviewLoading = useAppStore((s) => s.batchPreviewLoading);
  const batchApplying = useAppStore((s) => s.batchApplying);
  const batchLastId = useAppStore((s) => s.batchLastId);
  const batchLastUndoCount = useAppStore((s) => s.batchLastUndoCount);
  const previewBatchRule = useAppStore((s) => s.previewBatchRule);
  const applyBatchRule = useAppStore((s) => s.applyBatchRule);
  const undoLastBatchOperation = useAppStore((s) => s.undoLastBatchOperation);

  const [ratingAtLeast, setRatingAtLeast] = useState("");
  const [colorLabelFilter, setColorLabelFilter] = useState("");
  const [cameraModel, setCameraModel] = useState("");

  const [actionKind, setActionKind] = useState<ActionKind>("SetRating");
  const [ratingValue, setRatingValue] = useState(5);
  const [colorLabelValue, setColorLabelValue] = useState("");
  const [keywordName, setKeywordName] = useState("");

  if (!open) return null;

  function buildCriteria(): FilterCriteriaDto {
    const criteria: FilterCriteriaDto = {};
    if (ratingAtLeast.trim()) criteria.rating_at_least = Number(ratingAtLeast);
    if (colorLabelFilter.trim()) criteria.color_label = colorLabelFilter.trim();
    if (cameraModel.trim()) criteria.camera_model = cameraModel.trim();
    return criteria;
  }

  function buildAction() {
    if (actionKind === "SetRating") return { kind: "SetRating" as const, rating: ratingValue };
    if (actionKind === "SetColorLabel") {
      return { kind: "SetColorLabel" as const, color_label: colorLabelValue.trim() || null };
    }
    return { kind: "AddKeyword" as const, name: keywordName.trim() };
  }

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-16" onClick={onClose}>
      <div
        onClick={(e) => e.stopPropagation()}
        className="max-h-[85vh] w-full max-w-2xl overflow-y-auto rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
      >
        <h2 className="mb-1 text-sm font-semibold text-text-primary">Stapelverarbeitungs-Konsole</h2>
        <p className="mb-3 text-xs text-text-muted">Regel definieren, Trockenlauf prüfen, ausführen, bei Bedarf rückgängig machen.</p>

        <fieldset className="mb-3 flex flex-col gap-2 border-b border-border pb-3">
          <legend className="text-xs font-medium text-text-secondary">Auswahl (Filter)</legend>
          <div className="flex gap-2">
            <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
              Bewertung mindestens
              <input
                type="number"
                min={0}
                max={5}
                value={ratingAtLeast}
                onChange={(e) => setRatingAtLeast(e.target.value)}
                className="rounded border border-border bg-bg-panel px-2 py-1"
              />
            </label>
            <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
              Farbmarkierung
              <input
                type="text"
                value={colorLabelFilter}
                onChange={(e) => setColorLabelFilter(e.target.value)}
                placeholder="(beliebig)"
                className="rounded border border-border bg-bg-panel px-2 py-1"
              />
            </label>
            <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
              Kameramodell
              <input
                type="text"
                value={cameraModel}
                onChange={(e) => setCameraModel(e.target.value)}
                placeholder="(beliebig)"
                className="rounded border border-border bg-bg-panel px-2 py-1"
              />
            </label>
          </div>
        </fieldset>

        <fieldset className="mb-3 flex flex-col gap-2 border-b border-border pb-3">
          <legend className="text-xs font-medium text-text-secondary">Aktion</legend>
          <select
            aria-label="Aktion"
            value={actionKind}
            onChange={(e) => setActionKind(e.target.value as ActionKind)}
            className="rounded border border-border bg-bg-panel px-2 py-1 text-xs"
          >
            <option value="SetRating">Bewertung setzen</option>
            <option value="SetColorLabel">Farbmarkierung setzen</option>
            <option value="AddKeyword">Schlagwort hinzufügen</option>
          </select>
          {actionKind === "SetRating" && (
            <label className="flex flex-col gap-1 text-xs text-text-secondary">
              Neue Bewertung
              <input
                type="number"
                min={0}
                max={5}
                value={ratingValue}
                onChange={(e) => setRatingValue(Number(e.target.value))}
                className="w-20 rounded border border-border bg-bg-panel px-2 py-1"
              />
            </label>
          )}
          {actionKind === "SetColorLabel" && (
            <label className="flex flex-col gap-1 text-xs text-text-secondary">
              Neue Farbmarkierung (leer = entfernen)
              <input
                type="text"
                value={colorLabelValue}
                onChange={(e) => setColorLabelValue(e.target.value)}
                className="rounded border border-border bg-bg-panel px-2 py-1"
              />
            </label>
          )}
          {actionKind === "AddKeyword" && (
            <label className="flex flex-col gap-1 text-xs text-text-secondary">
              Schlagwort
              <input
                type="text"
                value={keywordName}
                onChange={(e) => setKeywordName(e.target.value)}
                className="rounded border border-border bg-bg-panel px-2 py-1"
              />
            </label>
          )}
        </fieldset>

        <div className="mb-3 flex gap-2">
          <button
            type="button"
            onClick={() => void previewBatchRule(buildCriteria())}
            disabled={batchPreviewLoading}
            className="rounded border border-border px-2 py-1 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-50"
          >
            {batchPreviewLoading ? "Prüft…" : "Trockenlauf (Vorschau)"}
          </button>
          <button
            type="button"
            onClick={() => void applyBatchRule(buildCriteria(), buildAction())}
            disabled={batchApplying || (actionKind === "AddKeyword" && !keywordName.trim())}
            className="rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
          >
            {batchApplying ? "Wendet an…" : "Ausführen"}
          </button>
          {batchLastId && (
            <button
              type="button"
              onClick={() => void undoLastBatchOperation()}
              className="rounded border border-danger px-2 py-1 text-xs text-danger hover:bg-danger/10"
            >
              Rückgängig
            </button>
          )}
        </div>

        {batchLastUndoCount !== null && (
          <p className="mb-2 text-xs text-text-muted">{batchLastUndoCount} Änderung(en) rückgängig gemacht.</p>
        )}

        <p className="mb-1 text-xs text-text-muted">{batchPreview.length} Foto(s) betroffen</p>
        <ul className="flex flex-col gap-0.5 text-xs">
          {batchPreview.map((photo) => (
            <li key={photo.id} className="truncate">
              {photo.filename}
            </li>
          ))}
        </ul>

        <div className="mt-3 flex justify-end">
          <button type="button" onClick={onClose} className="rounded border border-border px-3 py-1 text-xs hover:border-accent">
            Schließen
          </button>
        </div>
      </div>
    </div>
  );
}
