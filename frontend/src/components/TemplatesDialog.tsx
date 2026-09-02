import { useEffect, useState } from "react";

import { useT } from "../lib/i18n";
import { exportTemplateToFile, selectFolderDialog } from "../lib/tauri";
import type { ExportFormat, TemplateDto, TemplateKind, WorkflowTemplatePayload } from "../lib/tauri";
import { useAppStore } from "../store";

interface TemplatesDialogProps {
  open: boolean;
  photoIds: string[];
  onClose: () => void;
}

/**
 * Vorlagen-Dialog (Phase 8 Schritt 8, siehe `PLAN.md`/`apx_catalog::Template`s
 * Moduldoku). Verwaltet gespeicherte Vorlagen aller Art (Export-/Layout-
 * Vorlagen für die übrigen fünf Ausgabemodule, plus Workflow-Vorlagen) —
 * anlegen, auflisten, löschen, als lokale `.apxt`-Datei exportieren/
 * importieren ("lokales Repo-Format mit Manifest" statt Online-Hosting).
 *
 * **Bewusste Vereinfachung:** Export-/Layout-Vorlagen (Export/Druck/Buch/
 * Diashow/Web) werden hier über eingefügtes JSON angelegt, nicht per
 * "Aktuelle Einstellungen speichern"-Knopf direkt im jeweiligen Dialog —
 * die generische Speicherung ist echt (dieselbe Tabelle/Commands), nur
 * die komfortable Übernahme läuft noch nicht automatisch. Workflow-
 * Vorlagen (Preset + Exportoptionen) haben dagegen ein eigenes,
 * geführtes Formular, weil sie sich direkt ausführen lassen.
 */
export function TemplatesDialog({ open, photoIds, onClose }: TemplatesDialogProps) {
  const t = useT();
  const [kind, setKind] = useState<TemplateKind>("workflow");

  const KIND_LABELS: Record<TemplateKind, string> = {
    export: t("templatesDialog.kindExport"),
    print: t("templatesDialog.kindPrint"),
    book: t("templatesDialog.kindBook"),
    slideshow: t("templatesDialog.kindSlideshow"),
    web: t("templatesDialog.kindWeb"),
    workflow: t("templatesDialog.kindWorkflow"),
    // Filter-Presets (Phase 9 Schritt 3) haben ihre eigene Verwaltung in
    // `FilterBar.tsx` — dieser Dialog (Phase 8 Schritt 8) listet sie
    // bewusst nicht mit auf, der Eintrag hier ist nur für die
    // `Record<TemplateKind, string>`-Vollständigkeit nötig.
    filter: t("templatesDialog.kindFilter"),
  };

  const templatesByKind = useAppStore((s) => s.templatesByKind);
  const refreshTemplates = useAppStore((s) => s.refreshTemplates);
  const saveTemplateAction = useAppStore((s) => s.saveTemplateAction);
  const deleteTemplateAction = useAppStore((s) => s.deleteTemplateAction);
  const importTemplateFile = useAppStore((s) => s.importTemplateFile);
  const presets = useAppStore((s) => s.presets);
  const refreshPresets = useAppStore((s) => s.refreshPresets);
  const workflowRunning = useAppStore((s) => s.workflowRunning);
  const workflowProgress = useAppStore((s) => s.workflowProgress);
  const runWorkflowTemplate = useAppStore((s) => s.runWorkflowTemplate);

  const [newName, setNewName] = useState("");
  const [newPayloadJson, setNewPayloadJson] = useState("{}");
  const [workflowPresetId, setWorkflowPresetId] = useState("");
  const [workflowFormat, setWorkflowFormat] = useState<ExportFormat>("jpeg");
  const [workflowMaxEdge, setWorkflowMaxEdge] = useState(2048);
  const [error, setError] = useState<string | null>(null);

  const templates = templatesByKind[kind] ?? [];

  useEffect(() => {
    if (!open) return;
    void refreshTemplates(kind);
    void refreshPresets();
  }, [open, kind, refreshTemplates, refreshPresets]);

  if (!open) return null;

  async function handleSaveGeneric() {
    setError(null);
    if (!newName.trim()) return;
    try {
      const payload = JSON.parse(newPayloadJson);
      await saveTemplateAction(kind, newName.trim(), payload);
      setNewName("");
    } catch {
      setError(t("templatesDialog.invalidJson"));
    }
  }

  async function handleSaveWorkflow() {
    if (!newName.trim() || !workflowPresetId) return;
    const payload: WorkflowTemplatePayload = {
      presetId: workflowPresetId,
      exportOptions: { format: workflowFormat, maxEdge: workflowMaxEdge },
    };
    await saveTemplateAction("workflow", newName.trim(), payload);
    setNewName("");
  }

  async function handleRunWorkflow(template: TemplateDto) {
    const destFolder = await selectFolderDialog();
    if (!destFolder) return;
    const payload = JSON.parse(template.payload_json) as WorkflowTemplatePayload;
    await runWorkflowTemplate(photoIds, payload, destFolder);
  }

  async function exportTemplateFileFor(template: TemplateDto) {
    await exportTemplateToFile(template.id);
  }

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-16" onClick={onClose}>
      <div
        onClick={(e) => e.stopPropagation()}
        className="max-h-[85vh] w-full max-w-lg overflow-y-auto rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
      >
        <h2 className="mb-1 text-sm font-semibold text-text-primary">{t("templatesDialog.title")}</h2>
        <p className="mb-3 text-xs text-text-muted">{t("templatesDialog.subtitle")}</p>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          {t("templatesDialog.kind")}
          <select value={kind} onChange={(e) => setKind(e.target.value as TemplateKind)} className="rounded border border-border bg-bg-panel px-2 py-1 text-sm">
            {(Object.keys(KIND_LABELS) as TemplateKind[]).map((key) => (
              <option key={key} value={key}>
                {KIND_LABELS[key]}
              </option>
            ))}
          </select>
        </label>

        <ul className="mb-3 flex flex-col gap-1">
          {templates.length === 0 && <li className="text-xs text-text-muted">{t("templatesDialog.noTemplates")}</li>}
          {templates.map((template) => (
            <li key={template.id} className="flex items-center justify-between gap-2 rounded border border-border px-2 py-1 text-xs">
              <span className="truncate">{template.name}</span>
              <div className="flex shrink-0 gap-1">
                {kind === "workflow" && (
                  <button
                    type="button"
                    onClick={() => void handleRunWorkflow(template)}
                    disabled={photoIds.length === 0 || workflowRunning}
                    className="rounded border border-accent px-1.5 py-0.5 text-accent disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {t("templatesDialog.run")}
                  </button>
                )}
                <button type="button" onClick={() => void exportTemplateFileFor(template)} className="rounded border border-border px-1.5 py-0.5 hover:border-accent">
                  {t("templatesDialog.export")}
                </button>
                <button type="button" onClick={() => void deleteTemplateAction(kind, template.id)} className="rounded border border-border px-1.5 py-0.5 hover:border-danger">
                  {t("templatesDialog.delete")}
                </button>
              </div>
            </li>
          ))}
        </ul>

        {workflowProgress && (
          <p className="mb-2 text-xs text-text-secondary">
            {t("templatesDialog.workflowProgress", { done: workflowProgress.done, total: workflowProgress.total })}
            {workflowProgress.failed > 0 ? ` (${t("templatesDialog.workflowFailed", { count: workflowProgress.failed })})` : ""}
          </p>
        )}

        <button type="button" onClick={() => void importTemplateFile()} className="mb-3 rounded border border-border px-2 py-1 text-xs hover:border-accent">
          {t("templatesDialog.importFromFile")}
        </button>

        <div className="rounded border border-border p-2">
          <p className="mb-2 text-xs font-semibold text-text-secondary">{t("templatesDialog.newTemplate")}</p>
          <label className="mb-2 flex flex-col gap-1 text-xs text-text-secondary">
            {t("templatesDialog.name")}
            <input type="text" value={newName} onChange={(e) => setNewName(e.target.value)} className="rounded border border-border bg-bg-panel px-2 py-1" />
          </label>

          {kind === "workflow" ? (
            <>
              <label className="mb-2 flex flex-col gap-1 text-xs text-text-secondary">
                {t("templatesDialog.preset")}
                <select value={workflowPresetId} onChange={(e) => setWorkflowPresetId(e.target.value)} className="rounded border border-border bg-bg-panel px-2 py-1">
                  <option value="">{t("templatesDialog.choosePreset")}</option>
                  {presets.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                    </option>
                  ))}
                </select>
              </label>
              <div className="mb-2 flex gap-2">
                <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
                  {t("templatesDialog.format")}
                  <select value={workflowFormat} onChange={(e) => setWorkflowFormat(e.target.value as ExportFormat)} className="rounded border border-border bg-bg-panel px-2 py-1">
                    <option value="jpeg">JPEG</option>
                    <option value="png">PNG</option>
                    <option value="tiff">TIFF</option>
                    <option value="webp">WebP</option>
                    <option value="avif">AVIF</option>
                  </select>
                </label>
                <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
                  {t("templatesDialog.maxEdge")}
                  <input type="number" min={1} value={workflowMaxEdge} onChange={(e) => setWorkflowMaxEdge(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
                </label>
              </div>
              <button
                type="button"
                onClick={() => void handleSaveWorkflow()}
                disabled={!newName.trim() || !workflowPresetId}
                className="w-full rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
              >
                {t("templatesDialog.saveWorkflow")}
              </button>
            </>
          ) : (
            <>
              <label className="mb-2 flex flex-col gap-1 text-xs text-text-secondary">
                {t("templatesDialog.settingsJson")}
                <textarea value={newPayloadJson} onChange={(e) => setNewPayloadJson(e.target.value)} rows={4} className="rounded border border-border bg-bg-panel px-2 py-1 font-mono text-xs" />
              </label>
              {error && <p className="mb-2 text-xs text-danger">{error}</p>}
              <button
                type="button"
                onClick={() => void handleSaveGeneric()}
                disabled={!newName.trim()}
                className="w-full rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
              >
                {t("templatesDialog.saveTemplate")}
              </button>
            </>
          )}
        </div>

        <div className="mt-3 flex justify-end">
          <button type="button" onClick={onClose} className="rounded border border-border px-3 py-1 text-xs hover:border-accent">
            {t("templatesDialog.close")}
          </button>
        </div>
      </div>
    </div>
  );
}
