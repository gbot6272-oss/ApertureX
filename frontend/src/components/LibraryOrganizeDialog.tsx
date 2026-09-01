import { useEffect, useState } from "react";

import { suggestBestPhoto } from "../lib/duplicates";
import type { FilterCriteriaDto } from "../lib/tauri";
import { useAppStore } from "../store";

interface LibraryOrganizeDialogProps {
  open: boolean;
  onClose: () => void;
}

type Tab = "sets" | "stacks" | "copies" | "colors" | "duplicates";

const TAB_LABELS: Record<Tab, string> = {
  sets: "Sammlungssätze",
  stacks: "Stapel",
  copies: "Virtuelle Kopien",
  colors: "Farbmarkierungen",
  duplicates: "Duplikate",
};

/**
 * Bibliotheks-Backlog-Dialog (Phase 9 Schritt 1, siehe `PLAN.md`/
 * `DECISIONS.md` ADR-0032/ADR-0035) — ein Dialog für alle fünf neuen
 * Bausteine (Sammlungssätze/intelligente Sammlungen, Stapel, virtuelle
 * Kopien, erweiterbare Farbmarkierungen, Perceptual-Hash-Duplikat-
 * Assistent), analog zum `TemplatesDialog.tsx`-Muster aus Phase 8.
 */
export function LibraryOrganizeDialog({ open, onClose }: LibraryOrganizeDialogProps) {
  const [tab, setTab] = useState<Tab>("sets");

  const collectionFolders = useAppStore((s) => s.collectionFolders);
  const refreshCollectionFolders = useAppStore((s) => s.refreshCollectionFolders);
  const createCollectionFolder = useAppStore((s) => s.createCollectionFolder);
  const deleteCollectionFolder = useAppStore((s) => s.deleteCollectionFolder);
  const collections = useAppStore((s) => s.collections);
  const refreshCollections = useAppStore((s) => s.refreshCollections);
  const createSmartCollection = useAppStore((s) => s.createSmartCollection);
  const moveCollectionToFolder = useAppStore((s) => s.moveCollectionToFolder);

  const stacks = useAppStore((s) => s.stacks);
  const refreshStacks = useAppStore((s) => s.refreshStacks);
  const createStackFromSelection = useAppStore((s) => s.createStackFromSelection);
  const deleteStack = useAppStore((s) => s.deleteStack);
  const autoStackSelectionByTime = useAppStore((s) => s.autoStackSelectionByTime);

  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const virtualCopiesByPhotoId = useAppStore((s) => s.virtualCopiesByPhotoId);
  const createVirtualCopyForSelected = useAppStore((s) => s.createVirtualCopyForSelected);
  const refreshVirtualCopies = useAppStore((s) => s.refreshVirtualCopies);
  const selectPhoto = useAppStore((s) => s.selectPhoto);

  const colorLabelDefinitions = useAppStore((s) => s.colorLabelDefinitions);
  const refreshColorLabelDefinitions = useAppStore((s) => s.refreshColorLabelDefinitions);
  const createColorLabelDefinition = useAppStore((s) => s.createColorLabelDefinition);
  const deleteColorLabelDefinition = useAppStore((s) => s.deleteColorLabelDefinition);

  const perceptualDuplicateGroups = useAppStore((s) => s.perceptualDuplicateGroups);
  const perceptualDuplicatesRunning = useAppStore((s) => s.perceptualDuplicatesRunning);
  const runPerceptualDuplicateDetection = useAppStore((s) => s.runPerceptualDuplicateDetection);

  const [newFolderName, setNewFolderName] = useState("");
  const [newSmartName, setNewSmartName] = useState("");
  const [smartRatingAtLeast, setSmartRatingAtLeast] = useState<number | "">("");
  const [smartColorLabel, setSmartColorLabel] = useState("");
  const [newStackName, setNewStackName] = useState("");
  const [autoStackWindow, setAutoStackWindow] = useState(30);
  const [newLabelName, setNewLabelName] = useState("");
  const [newLabelDisplayName, setNewLabelDisplayName] = useState("");
  const [newLabelHex, setNewLabelHex] = useState("#888888");
  const [maxDistance, setMaxDistance] = useState(10);

  useEffect(() => {
    if (!open) return;
    void refreshCollectionFolders();
    void refreshCollections();
    void refreshStacks();
    void refreshColorLabelDefinitions();
    if (selectedPhotoId) void refreshVirtualCopies(selectedPhotoId);
  }, [open, refreshCollectionFolders, refreshCollections, refreshStacks, refreshColorLabelDefinitions, refreshVirtualCopies, selectedPhotoId]);

  if (!open) return null;

  async function handleCreateFolder() {
    if (!newFolderName.trim()) return;
    await createCollectionFolder(newFolderName.trim());
    setNewFolderName("");
  }

  async function handleCreateSmartCollection() {
    if (!newSmartName.trim()) return;
    const criteria: FilterCriteriaDto = {
      rating_at_least: smartRatingAtLeast === "" ? undefined : smartRatingAtLeast,
      color_label: smartColorLabel || undefined,
    };
    await createSmartCollection(newSmartName.trim(), undefined, criteria);
    setNewSmartName("");
  }

  const virtualCopies = selectedPhotoId ? (virtualCopiesByPhotoId[selectedPhotoId] ?? []) : [];

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-16" onClick={onClose}>
      <div
        onClick={(e) => e.stopPropagation()}
        className="max-h-[85vh] w-full max-w-2xl overflow-y-auto rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
      >
        <h2 className="mb-1 text-sm font-semibold text-text-primary">Bibliothek organisieren</h2>
        <p className="mb-3 text-xs text-text-muted">Sammlungssätze, Stapel, virtuelle Kopien, Farbmarkierungen, Duplikat-Assistent</p>

        <div className="mb-3 flex gap-1 border-b border-border pb-2">
          {(Object.keys(TAB_LABELS) as Tab[]).map((t) => (
            <button
              key={t}
              type="button"
              onClick={() => setTab(t)}
              className={`rounded px-2 py-1 text-xs ${tab === t ? "border border-accent bg-accent/10 text-accent" : "border border-border hover:border-accent"}`}
            >
              {TAB_LABELS[t]}
            </button>
          ))}
        </div>

        {tab === "sets" && (
          <div className="flex flex-col gap-3">
            <div>
              <p className="mb-1 text-xs font-semibold text-text-secondary">Sammlungssätze</p>
              <ul className="mb-2 flex flex-col gap-1">
                {collectionFolders.map((f) => (
                  <li key={f.id} className="flex items-center justify-between rounded border border-border px-2 py-1 text-xs">
                    <span>{f.name}</span>
                    <button type="button" onClick={() => void deleteCollectionFolder(f.id)} className="rounded border border-border px-1.5 py-0.5 hover:border-danger">
                      Löschen
                    </button>
                  </li>
                ))}
                {collectionFolders.length === 0 && <li className="text-xs text-text-muted">Keine Sammlungssätze</li>}
              </ul>
              <div className="flex gap-1">
                <input type="text" value={newFolderName} onChange={(e) => setNewFolderName(e.target.value)} placeholder="Neuer Sammlungssatz" className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs" />
                <button type="button" onClick={() => void handleCreateFolder()} className="shrink-0 rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent">
                  Anlegen
                </button>
              </div>
            </div>

            <div>
              <p className="mb-1 text-xs font-semibold text-text-secondary">Sammlungen</p>
              <ul className="mb-2 flex flex-col gap-1">
                {collections.map((c) => (
                  <li key={c.id} className="flex items-center justify-between rounded border border-border px-2 py-1 text-xs">
                    <span>
                      {c.name} {c.is_smart && <span className="text-text-muted">(intelligent)</span>}
                    </span>
                    <select
                      value={c.folder_id ?? ""}
                      onChange={(e) => void moveCollectionToFolder(c.id, e.target.value || null)}
                      className="rounded border border-border bg-bg-panel px-1 py-0.5 text-xs"
                    >
                      <option value="">Wurzel</option>
                      {collectionFolders.map((f) => (
                        <option key={f.id} value={f.id}>
                          {f.name}
                        </option>
                      ))}
                    </select>
                  </li>
                ))}
              </ul>
            </div>

            <div className="rounded border border-border p-2">
              <p className="mb-2 text-xs font-semibold text-text-secondary">Neue intelligente Sammlung</p>
              <input type="text" value={newSmartName} onChange={(e) => setNewSmartName(e.target.value)} placeholder="Name" className="mb-2 w-full rounded border border-border bg-bg-panel px-2 py-1 text-xs" />
              <div className="mb-2 flex gap-2">
                <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
                  Bewertung mind.
                  <input type="number" min={0} max={5} value={smartRatingAtLeast} onChange={(e) => setSmartRatingAtLeast(e.target.value === "" ? "" : Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
                </label>
                <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
                  Farbmarkierung
                  <input type="text" value={smartColorLabel} onChange={(e) => setSmartColorLabel(e.target.value)} placeholder="z. B. red" className="rounded border border-border bg-bg-panel px-2 py-1" />
                </label>
              </div>
              <button type="button" onClick={() => void handleCreateSmartCollection()} className="w-full rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent">
                Intelligente Sammlung anlegen
              </button>
            </div>
          </div>
        )}

        {tab === "stacks" && (
          <div className="flex flex-col gap-3">
            <div className="flex gap-2">
              <input type="text" value={newStackName} onChange={(e) => setNewStackName(e.target.value)} placeholder="Stapel-Name (optional)" className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs" />
              <button type="button" onClick={() => void createStackFromSelection(newStackName || undefined)} className="shrink-0 rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent">
                Aus Auswahl stapeln
              </button>
            </div>
            <div className="flex gap-2">
              <label className="flex items-center gap-1 text-xs text-text-secondary">
                Zeitfenster (s)
                <input type="number" min={1} value={autoStackWindow} onChange={(e) => setAutoStackWindow(Number(e.target.value))} className="w-20 rounded border border-border bg-bg-panel px-2 py-1" />
              </label>
              <button type="button" onClick={() => void autoStackSelectionByTime(autoStackWindow)} className="rounded border border-border px-2 py-1 text-xs hover:border-accent">
                Auswahl automatisch stapeln
              </button>
            </div>
            <ul className="flex flex-col gap-1">
              {stacks.map((s) => (
                <li key={s.id} className="flex items-center justify-between rounded border border-border px-2 py-1 text-xs">
                  <span>
                    {s.name ?? "Stapel"} — {s.photo_ids.length} Fotos
                  </span>
                  <button type="button" onClick={() => void deleteStack(s.id)} className="rounded border border-border px-1.5 py-0.5 hover:border-danger">
                    Löschen
                  </button>
                </li>
              ))}
              {stacks.length === 0 && <li className="text-xs text-text-muted">Keine Stapel</li>}
            </ul>
          </div>
        )}

        {tab === "copies" && (
          <div className="flex flex-col gap-3">
            <button
              type="button"
              onClick={() => void createVirtualCopyForSelected()}
              disabled={!selectedPhotoId}
              className="w-full rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
            >
              Virtuelle Kopie vom ausgewählten Foto erstellen
            </button>
            {!selectedPhotoId && <p className="text-xs text-text-muted">Kein Foto ausgewählt</p>}
            <ul className="flex flex-col gap-1">
              {virtualCopies.map((copy) => (
                <li key={copy.id} className="flex items-center justify-between rounded border border-border px-2 py-1 text-xs">
                  <span>{copy.filename} — {copy.rating}★</span>
                  <button type="button" onClick={() => selectPhoto(copy.id)} className="rounded border border-border px-1.5 py-0.5 hover:border-accent">
                    Öffnen
                  </button>
                </li>
              ))}
            </ul>
          </div>
        )}

        {tab === "colors" && (
          <div className="flex flex-col gap-3">
            <ul className="flex flex-col gap-1">
              {colorLabelDefinitions.map((def) => (
                <li key={def.name} className="flex items-center justify-between rounded border border-border px-2 py-1 text-xs">
                  <span className="flex items-center gap-2">
                    <span className="inline-block h-3 w-3 rounded-full" style={{ backgroundColor: def.hex }} />
                    {def.display_name} ({def.name})
                  </span>
                  <button type="button" onClick={() => void deleteColorLabelDefinition(def.name)} className="rounded border border-border px-1.5 py-0.5 hover:border-danger">
                    Löschen
                  </button>
                </li>
              ))}
            </ul>
            <div className="rounded border border-border p-2">
              <p className="mb-2 text-xs font-semibold text-text-secondary">Neue Farbmarkierung</p>
              <div className="mb-2 flex gap-2">
                <input type="text" value={newLabelName} onChange={(e) => setNewLabelName(e.target.value)} placeholder="interner Name (z. B. orange)" className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs" />
                <input type="text" value={newLabelDisplayName} onChange={(e) => setNewLabelDisplayName(e.target.value)} placeholder="Anzeigename" className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs" />
                <input type="color" value={newLabelHex} onChange={(e) => setNewLabelHex(e.target.value)} className="h-7 w-10 shrink-0 rounded border border-border bg-bg-panel" />
              </div>
              <button
                type="button"
                onClick={() => {
                  void createColorLabelDefinition(newLabelName, newLabelDisplayName, newLabelHex);
                  setNewLabelName("");
                  setNewLabelDisplayName("");
                }}
                disabled={!newLabelName.trim() || !newLabelDisplayName.trim()}
                className="w-full rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
              >
                Anlegen
              </button>
            </div>
          </div>
        )}

        {tab === "duplicates" && (
          <div className="flex flex-col gap-3">
            <div className="flex gap-2">
              <label className="flex items-center gap-1 text-xs text-text-secondary">
                Ähnlichkeitsschwelle
                <input type="number" min={0} max={64} value={maxDistance} onChange={(e) => setMaxDistance(Number(e.target.value))} className="w-16 rounded border border-border bg-bg-panel px-2 py-1" />
              </label>
              <button
                type="button"
                onClick={() => void runPerceptualDuplicateDetection(maxDistance)}
                disabled={perceptualDuplicatesRunning}
                className="rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
              >
                {perceptualDuplicatesRunning ? "Suche läuft…" : "Duplikate suchen"}
              </button>
            </div>
            {perceptualDuplicateGroups.length === 0 && !perceptualDuplicatesRunning && <p className="text-xs text-text-muted">Keine Gruppen gefunden (oder noch nicht gesucht)</p>}
            <ul className="flex flex-col gap-2">
              {perceptualDuplicateGroups.map((group, index) => {
                const best = suggestBestPhoto(group);
                return (
                  <li key={index} className="rounded border border-border p-2 text-xs">
                    <p className="mb-1 text-text-muted">Gruppe {index + 1} ({group.length} Fotos)</p>
                    {group.map((photo) => (
                      <div key={photo.id} className="flex items-center justify-between py-0.5">
                        <span>
                          {photo.filename} {best?.id === photo.id && <span className="text-accent">— Vorschlag</span>}
                        </span>
                        <button type="button" onClick={() => selectPhoto(photo.id)} className="rounded border border-border px-1.5 py-0.5 hover:border-accent">
                          Öffnen
                        </button>
                      </div>
                    ))}
                  </li>
                );
              })}
            </ul>
          </div>
        )}

        <div className="mt-3 flex justify-end">
          <button type="button" onClick={onClose} className="rounded border border-border px-3 py-1 text-xs hover:border-accent">
            Schließen
          </button>
        </div>
      </div>
    </div>
  );
}
