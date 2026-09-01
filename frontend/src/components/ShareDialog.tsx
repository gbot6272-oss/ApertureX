import { useState } from "react";

import { useAppStore } from "../store";

interface ShareDialogProps {
  open: boolean;
  onClose: () => void;
}

/**
 * Kollaborationsmodus (Phase 9 Schritt 10, siehe `PLAN.md`,
 * `DECISIONS.md` ADR-0035 Punkt 4) — asynchroner
 * Export→Weitergabe→Import→Konfliktauflösung-Ablauf über `.apxs`-Dateien.
 *
 * **Export-Tab**: schreibt die aktuellen Bearbeitungsstände der
 * Mehrfachauswahl als `.apxs`-Datei (keine Pixel-Bytes, nur EDL +
 * `content_hash`-Matching-Schlüssel).
 *
 * **Import-Tab**: liest eine `.apxs`-Datei, gleicht jedes Foto per
 * `content_hash` gegen den lokalen Katalog ab — unveränderte Stände und
 * nicht zuordenbare Fotos werden nur angezeigt, Konflikte (abweichender
 * EDL-Inhalt) brauchen eine manuelle Entscheidung je Foto (meins
 * behalten/übernehmen/als virtuelle Kopie behalten). Nichts wird beim
 * bloßen Einlesen der Datei committet.
 *
 * **Ehrlich begrenzt**: kein Echtzeit-Mehrbenutzer-Modus (kein
 * Live-Cursor/keine Präsenz/kein CRDT) — dieser Dialog deckt nur den
 * asynchronen Datei-Austausch ab.
 */
export function ShareDialog({ open, onClose }: ShareDialogProps) {
  const [tab, setTab] = useState<"export" | "import">("export");
  const [shareName, setShareName] = useState("Freigabe");

  const multiSelectedIds = useAppStore((s) => s.multiSelectedIds);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const shareRunning = useAppStore((s) => s.shareRunning);
  const shareExportStatus = useAppStore((s) => s.shareExportStatus);
  const shareImportResult = useAppStore((s) => s.shareImportResult);
  const exportSelectionAsShare = useAppStore((s) => s.exportSelectionAsShare);
  const importShareFile = useAppStore((s) => s.importShareFile);
  const resolveShareConflictAction = useAppStore((s) => s.resolveShareConflictAction);

  if (!open) return null;

  const exportIds = multiSelectedIds.length > 0 ? multiSelectedIds : selectedPhotoId ? [selectedPhotoId] : [];

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-24" onClick={onClose}>
      <div
        role="dialog"
        aria-label="Kollaboration"
        className="w-full max-w-lg rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 className="mb-1 text-sm font-semibold text-text-primary">Kollaboration</h2>
        <p className="mb-3 text-xs text-text-muted">
          Asynchroner Austausch über .apxs-Dateien — keine Pixel-Bytes, kein Echtzeit-Modus.
        </p>

        <div className="mb-3 flex gap-1 border-b border-border">
          <button
            type="button"
            onClick={() => setTab("export")}
            className={`px-3 py-1.5 text-xs ${
              tab === "export" ? "border-b-2 border-accent text-text-primary" : "text-text-muted"
            }`}
          >
            Exportieren
          </button>
          <button
            type="button"
            onClick={() => setTab("import")}
            className={`px-3 py-1.5 text-xs ${
              tab === "import" ? "border-b-2 border-accent text-text-primary" : "text-text-muted"
            }`}
          >
            Importieren
          </button>
        </div>

        {tab === "export" ? (
          <div className="flex flex-col gap-2">
            <p className="text-xs text-text-muted">
              {exportIds.length} {exportIds.length === 1 ? "Foto" : "Fotos"} ausgewählt.
            </p>
            <label className="text-xs text-text-secondary">
              Bezeichnung
              <input
                type="text"
                value={shareName}
                onChange={(event) => setShareName(event.target.value)}
                className="mt-1 w-full rounded border border-border bg-bg-panel px-2 py-1 text-xs text-text-primary"
              />
            </label>
            <div className="flex justify-end">
              <button
                type="button"
                onClick={() => void exportSelectionAsShare(exportIds, shareName)}
                disabled={shareRunning || exportIds.length === 0 || shareName.trim() === ""}
                className="rounded border border-border px-3 py-1.5 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
              >
                Als .apxs speichern…
              </button>
            </div>
            {shareRunning && <p className="text-xs text-text-muted">Läuft…</p>}
            {shareExportStatus && !shareRunning && <p className="text-xs text-text-secondary">{shareExportStatus}</p>}
          </div>
        ) : (
          <div className="flex flex-col gap-3">
            <div className="flex justify-end">
              <button
                type="button"
                onClick={() => void importShareFile()}
                disabled={shareRunning}
                className="rounded border border-border px-3 py-1.5 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
              >
                .apxs öffnen…
              </button>
            </div>
            {shareRunning && <p className="text-xs text-text-muted">Läuft…</p>}

            {shareImportResult && (
              <div className="flex flex-col gap-3">
                <p className="text-xs text-text-secondary">„{shareImportResult.name}"</p>

                {shareImportResult.unchanged.length > 0 && (
                  <p className="text-[11px] text-text-muted">
                    {shareImportResult.unchanged.length} unverändert (identischer Bearbeitungsstand).
                  </p>
                )}
                {shareImportResult.unmatched.length > 0 && (
                  <p className="text-[11px] text-text-muted">
                    {shareImportResult.unmatched.length} ohne lokale Entsprechung: {" "}
                    {shareImportResult.unmatched.map((u) => u.filename).join(", ")}
                  </p>
                )}

                {shareImportResult.conflicts.length === 0 ? (
                  <p className="text-xs text-text-secondary">Keine offenen Konflikte.</p>
                ) : (
                  <ul className="flex flex-col gap-2">
                    {shareImportResult.conflicts.map((conflict) => (
                      <li key={conflict.photo_id} className="rounded border border-border p-2">
                        <p className="mb-1 text-xs text-text-primary">{conflict.filename}</p>
                        <p className="mb-2 text-[11px] text-text-muted">
                          Vorschlag: {conflict.prefer_incoming ? "übernehmen" : "meins behalten"} (zuletzt geändert
                          gewinnt)
                        </p>
                        <div className="flex gap-1">
                          <button
                            type="button"
                            onClick={() =>
                              void resolveShareConflictAction(conflict.photo_id, conflict.incoming_edl_json, "mine")
                            }
                            className="rounded border border-border px-2 py-1 text-[11px] hover:border-accent"
                          >
                            Meins behalten
                          </button>
                          <button
                            type="button"
                            onClick={() =>
                              void resolveShareConflictAction(
                                conflict.photo_id,
                                conflict.incoming_edl_json,
                                "theirs",
                              )
                            }
                            className="rounded border border-border px-2 py-1 text-[11px] hover:border-accent"
                          >
                            Übernehmen
                          </button>
                          <button
                            type="button"
                            onClick={() =>
                              void resolveShareConflictAction(
                                conflict.photo_id,
                                conflict.incoming_edl_json,
                                "virtual_copy",
                              )
                            }
                            className="rounded border border-border px-2 py-1 text-[11px] hover:border-accent"
                          >
                            Als virtuelle Kopie
                          </button>
                        </div>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            )}
          </div>
        )}

        <div className="mt-3 flex justify-end">
          <button
            type="button"
            onClick={onClose}
            className="rounded border border-border px-3 py-1 text-xs text-text-secondary hover:bg-bg-panel"
          >
            Schließen
          </button>
        </div>
      </div>
    </div>
  );
}
