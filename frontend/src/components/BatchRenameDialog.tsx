import { AlertTriangle, ArrowRight, Check, Save, Trash2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { RENAME_PATTERN_TOKENS, previewRenamePattern } from "../lib/renamePattern";
import * as api from "../lib/tauri";
import type { RenamePlanStatus, TemplateDto } from "../lib/tauri";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

/**
 * Stapel-Umbenennung (Phase 32 F4).
 *
 * Der Import konnte Dateien seit Phase 9 nach einem Tokenmuster benennen
 * (`lib/renamePattern.ts`, `apx-app`s `import::rename`) — danach war der
 * Name für immer fest. Dieser Dialog holt dasselbe Tokensystem an die
 * bereits importierten Fotos.
 *
 * **Die Vorschau kommt aus dem Backend**, nicht aus einer
 * Frontend-Nachrechnung: `preview_batch_rename` und `apply_batch_rename`
 * benutzen in Rust dieselbe Planungsfunktion. Eine hier nachgebaute
 * Vorschau könnte vom Ergebnis abweichen — und gerade bei einer Aktion,
 * die Dateien auf der Platte anfasst, ist „was du siehst, ist was
 * passiert" der ganze Wert der Vorschau. Die reine Tipp-Hilfe unten
 * (`previewRenamePattern` an einem erfundenen Beispiel) läuft weiter
 * lokal — die braucht kein echtes Foto und soll auch bei leerer Auswahl
 * zeigen, was ein Token tut.
 */

const STATUS_LABEL: Record<RenamePlanStatus, string> = {
  planned: "wird umbenannt",
  unchanged: "unverändert",
  empty_name: "Muster ergibt keinen Namen",
  duplicate_in_batch: "doppelter Zielname — {seq} einfügen",
  collides_with_existing: "Name schon vergeben",
  virtual_copy: "virtuelle Kopie — Datei gehört dem Original",
};

const QUICK_PATTERNS = [
  { pattern: "{date}_{seq}", label: "Datum + Nummer" },
  { pattern: "{original}_{date}", label: "Name + Datum" },
  { pattern: "{camera}_{date}_{seq}", label: "Kamera + Datum + Nummer" },
];

interface BatchRenameDialogProps {
  open: boolean;
  onClose: () => void;
}

export function BatchRenameDialog({ open, onClose }: BatchRenameDialogProps) {
  const multiSelectedIds = useAppStore((s) => s.multiSelectedIds);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const preview = useAppStore((s) => s.batchRenamePreview);
  const previewLoading = useAppStore((s) => s.batchRenamePreviewLoading);
  const running = useAppStore((s) => s.batchRenameRunning);
  const error = useAppStore((s) => s.batchRenameError);
  const resultCount = useAppStore((s) => s.batchRenameResultCount);
  const loadPreview = useAppStore((s) => s.loadBatchRenamePreview);
  const runBatchRename = useAppStore((s) => s.runBatchRename);
  const clearState = useAppStore((s) => s.clearBatchRenameState);

  const [pattern, setPattern] = useState("{date}_{seq}");
  const [startSeq, setStartSeq] = useState(1);
  const [templates, setTemplates] = useState<TemplateDto[]>([]);
  const [templateName, setTemplateName] = useState("");
  const [selectedTemplateId, setSelectedTemplateId] = useState("");
  const patternInputRef = useRef<HTMLInputElement | null>(null);

  // Die Auswahl, auf die das Muster angewandt wird: die Mehrfachauswahl,
  // ersatzweise das einzelne aktive Foto.
  const photoIds = multiSelectedIds.length > 0 ? multiSelectedIds : selectedPhotoId ? [selectedPhotoId] : [];
  const photoIdsKey = photoIds.join(",");

  useEffect(() => {
    if (!open) return;
    void loadPreview(photoIdsKey ? photoIdsKey.split(",") : [], pattern, startSeq);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `photoIdsKey` steht für die Auswahl; die Array-Referenz ändert sich bei jedem Render
  }, [open, photoIdsKey, pattern, startSeq]);

  useEffect(() => {
    if (!open) return;
    void api
      .listTemplates("rename")
      .then(setTemplates)
      .catch(() => setTemplates([]));
  }, [open]);

  useEffect(() => {
    if (!open) clearState();
  }, [open, clearState]);

  /** Fügt ein Token an der Cursorposition ein statt hinten anzuhängen —
   * wer mitten im Muster steht, will es dort haben. */
  function insertToken(token: string) {
    const input = patternInputRef.current;
    if (!input) {
      setPattern((current) => current + token);
      return;
    }
    const start = input.selectionStart ?? pattern.length;
    const end = input.selectionEnd ?? pattern.length;
    const next = pattern.slice(0, start) + token + pattern.slice(end);
    setPattern(next);
    requestAnimationFrame(() => {
      input.focus();
      input.setSelectionRange(start + token.length, start + token.length);
    });
  }

  async function saveTemplate() {
    const name = templateName.trim();
    if (!name) return;
    await api.saveTemplate("rename", name, JSON.stringify({ pattern, start_seq: startSeq }));
    setTemplates(await api.listTemplates("rename"));
    setTemplateName("");
  }

  function applyTemplate(templateId: string) {
    setSelectedTemplateId(templateId);
    const template = templates.find((entry) => entry.id === templateId);
    if (!template) return;
    try {
      const payload = JSON.parse(template.payload_json) as { pattern?: string; start_seq?: number };
      if (typeof payload.pattern === "string") setPattern(payload.pattern);
      if (typeof payload.start_seq === "number") setStartSeq(payload.start_seq);
    } catch {
      // Eine von Hand verbogene Vorlage soll den Dialog nicht mitreißen.
    }
  }

  async function deleteTemplate() {
    if (!selectedTemplateId) return;
    await api.deleteTemplate(selectedTemplateId);
    setTemplates(await api.listTemplates("rename"));
    setSelectedTemplateId("");
  }

  const plannedCount = preview.filter((entry) => entry.status === "planned").length;
  const unchangedCount = preview.filter((entry) => entry.status === "unchanged").length;
  const skippedCount = preview.filter((entry) => entry.status === "virtual_copy").length;
  const blockedCount = preview.filter(
    (entry) => entry.status !== "planned" && entry.status !== "unchanged" && entry.status !== "virtual_copy",
  ).length;

  return (
    <Dialog open={open} onClose={onClose} label="Stapel-Umbenennung" className="flex max-h-[85vh] w-[46rem] max-w-[92vw] flex-col">
      <div className="flex min-h-0 flex-col gap-3 p-4">
        <div>
          <h2 className="text-sm font-semibold text-text-primary">Stapel-Umbenennung</h2>
          <p className="mt-0.5 text-xs text-text-muted" data-testid="batch-rename-scope">
            {photoIds.length} {photoIds.length === 1 ? "Foto" : "Fotos"} ausgewählt · die Dateiendung bleibt immer erhalten
          </p>
        </div>

        <div className="flex flex-wrap items-end gap-2">
          <label className="flex min-w-[18rem] flex-1 flex-col gap-1 text-xs text-text-secondary">
            Muster
            <input
              ref={patternInputRef}
              type="text"
              value={pattern}
              onChange={(event) => setPattern(event.target.value)}
              aria-label="Umbenennungsmuster"
              className="rounded border border-border bg-bg-base px-2 py-1 font-mono text-xs text-text-primary"
            />
          </label>
          <label className="flex w-28 flex-col gap-1 text-xs text-text-secondary">
            Startnummer
            <input
              type="number"
              min={1}
              value={startSeq}
              onChange={(event) => setStartSeq(Math.max(1, Number(event.target.value) || 1))}
              aria-label="Startnummer"
              className="rounded border border-border bg-bg-base px-2 py-1 text-xs text-text-primary"
            />
          </label>
        </div>

        <div className="flex flex-wrap items-center gap-1" role="group" aria-label="Token einfügen">
          {RENAME_PATTERN_TOKENS.map((token) => (
            <button
              key={token.token}
              type="button"
              onClick={() => insertToken(token.token)}
              title={token.label}
              aria-label={`Token ${token.token} einfügen`}
              className="apx-btn-liquid rounded border border-border px-2 py-0.5 font-mono text-[11px] text-text-secondary transition-colors duration-[var(--duration-fast)] hover:text-text-primary"
            >
              {token.token}
            </button>
          ))}
          <span className="mx-1 text-text-muted">·</span>
          {QUICK_PATTERNS.map((quick) => (
            <button
              key={quick.pattern}
              type="button"
              onClick={() => setPattern(quick.pattern)}
              aria-label={`Muster ${quick.label}`}
              className="apx-btn-liquid rounded border border-border px-2 py-0.5 text-[11px] text-text-secondary transition-colors duration-[var(--duration-fast)] hover:text-text-primary"
            >
              {quick.label}
            </button>
          ))}
        </div>

        <p className="text-xs text-text-muted">
          Beispiel: <span className="font-mono text-text-secondary">{previewRenamePattern(pattern) || "—"}</span>
        </p>

        <div className="flex flex-wrap items-end gap-2 rounded border border-border p-2">
          <label className="flex flex-col gap-1 text-xs text-text-secondary">
            Gespeicherte Muster
            <select
              value={selectedTemplateId}
              onChange={(event) => applyTemplate(event.target.value)}
              aria-label="Gespeichertes Muster anwenden"
              className="rounded border border-border bg-bg-base px-2 py-1 text-xs text-text-primary"
            >
              <option value="">— auswählen —</option>
              {templates.map((template) => (
                <option key={template.id} value={template.id}>
                  {template.name}
                </option>
              ))}
            </select>
          </label>
          <button
            type="button"
            onClick={() => void deleteTemplate()}
            disabled={!selectedTemplateId}
            aria-label="Gespeichertes Muster löschen"
            className="apx-btn-liquid rounded border border-border p-1.5 text-text-secondary disabled:cursor-not-allowed disabled:opacity-40"
          >
            <Trash2 aria-hidden="true" className="size-3.5" />
          </button>
          <span className="flex-1" />
          <input
            type="text"
            value={templateName}
            onChange={(event) => setTemplateName(event.target.value)}
            placeholder="Neues Muster…"
            aria-label="Name für das neue Muster"
            className="w-44 rounded border border-border bg-bg-base px-2 py-1 text-xs text-text-primary"
          />
          <button
            type="button"
            onClick={() => void saveTemplate()}
            disabled={!templateName.trim()}
            // Eigenes Label: „Speichern" allein gibt es an anderer Stelle
            // (Filter-Presets in `FilterBar.tsx`) noch einmal.
            aria-label="Muster speichern"
            className="apx-btn-liquid flex items-center gap-1.5 rounded border border-border px-2 py-1 text-xs text-text-secondary disabled:cursor-not-allowed disabled:opacity-40"
          >
            <Save aria-hidden="true" className="size-3.5" />
            Speichern
          </button>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto rounded border border-border">
          <table className="w-full text-left text-xs" data-testid="batch-rename-preview">
            <thead className="sticky top-0 bg-bg-panel text-text-muted">
              <tr>
                <th className="px-2 py-1 font-normal">Bisher</th>
                <th className="w-6" />
                <th className="px-2 py-1 font-normal">Neu</th>
                <th className="px-2 py-1 font-normal">Status</th>
              </tr>
            </thead>
            <tbody>
              {preview.map((entry) => {
                const blocked = entry.status !== "planned" && entry.status !== "unchanged";
                return (
                  <tr key={entry.photo_id} className={blocked ? "bg-danger/10" : ""}>
                    <td className="px-2 py-1 font-mono text-text-secondary">{entry.current_filename}</td>
                    <td className="text-text-muted">
                      <ArrowRight aria-hidden="true" className="size-3" />
                    </td>
                    <td className="px-2 py-1 font-mono text-text-primary">{entry.new_filename}</td>
                    <td className="px-2 py-1 text-text-muted">
                      {blocked && <AlertTriangle aria-hidden="true" className="mr-1 inline size-3 text-danger" />}
                      {STATUS_LABEL[entry.status]}
                    </td>
                  </tr>
                );
              })}
              {preview.length === 0 && (
                <tr>
                  <td colSpan={4} className="px-2 py-3 text-center text-text-muted">
                    {previewLoading ? "Vorschau wird berechnet…" : "Keine Fotos ausgewählt."}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>

        <div className="flex flex-wrap items-center gap-3">
          <span className="text-xs text-text-secondary" data-testid="batch-rename-summary">
            {plannedCount} werden umbenannt · {unchangedCount} unverändert · {skippedCount} übersprungen · {blockedCount} blockiert
          </span>
          <span className="flex-1" />
          {resultCount !== null && (
            <span className="apx-notice-in flex items-center gap-1 text-xs text-success" data-testid="batch-rename-result">
              <Check aria-hidden="true" className="size-3.5" />
              {resultCount} umbenannt
            </span>
          )}
          <button
            type="button"
            onClick={onClose}
            className="apx-btn-liquid rounded border border-border px-3 py-1 text-xs text-text-secondary"
          >
            Schließen
          </button>
          <button
            type="button"
            onClick={() => void runBatchRename(photoIds, pattern, startSeq)}
            disabled={running || plannedCount === 0 || blockedCount > 0}
            title={blockedCount > 0 ? "Erst die rot markierten Zeilen auflösen" : undefined}
            className="apx-btn-liquid rounded border border-accent bg-accent/10 px-3 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-40"
          >
            {running ? "Benennt um…" : "Umbenennen"}
          </button>
        </div>

        {error && (
          <p role="alert" className="rounded border border-danger/50 bg-danger/10 px-2 py-1 text-xs text-danger">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}
