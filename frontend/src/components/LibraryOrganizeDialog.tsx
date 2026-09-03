import { useEffect, useState } from "react";

import { useT } from "../lib/i18n";
import { suggestBestPhoto } from "../lib/duplicates";
import { SMART_COLLECTION_FIELD_OPTIONS, SMART_COLLECTION_OPERATOR_OPTIONS } from "../lib/tauri";
import type { SmartCollectionLeaf } from "../lib/tauri";
import { conditionNode, groupNode } from "../lib/ruleTree";
import type { RuleNode } from "../lib/ruleTree";
import { RuleTreeEditor } from "./RuleTreeEditor";
import { useAppStore } from "../store";

function makeDefaultSmartLeaf(): SmartCollectionLeaf {
  return { field: "rating", op: "at_least", value: "4" };
}

interface LibraryOrganizeDialogProps {
  open: boolean;
  onClose: () => void;
}

type Tab = "sets" | "stacks" | "copies" | "colors" | "duplicates" | "style";

/**
 * Bibliotheks-Backlog-Dialog (Phase 9 Schritt 1, siehe `PLAN.md`/
 * `DECISIONS.md` ADR-0032/ADR-0035) — ein Dialog für alle fünf neuen
 * Bausteine (Sammlungssätze/intelligente Sammlungen, Stapel, virtuelle
 * Kopien, erweiterbare Farbmarkierungen, Perceptual-Hash-Duplikat-
 * Assistent), analog zum `TemplatesDialog.tsx`-Muster aus Phase 8.
 */
export function LibraryOrganizeDialog({ open, onClose }: LibraryOrganizeDialogProps) {
  const t = useT();
  const [tab, setTab] = useState<Tab>("sets");

  const TAB_LABELS: Record<Tab, string> = {
    sets: t("libraryOrganizeDialog.tabSets"),
    stacks: t("libraryOrganizeDialog.tabStacks"),
    copies: t("libraryOrganizeDialog.tabCopies"),
    colors: t("libraryOrganizeDialog.tabColors"),
    duplicates: t("libraryOrganizeDialog.tabDuplicates"),
    style: t("libraryOrganizeDialog.tabStyle"),
  };

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

  const selectedFolderId = useAppStore((s) => s.selectedFolderId);
  const styleConsistencyResult = useAppStore((s) => s.styleConsistencyResult);
  const styleConsistencyRunning = useAppStore((s) => s.styleConsistencyRunning);
  const runStyleConsistencyCheck = useAppStore((s) => s.runStyleConsistencyCheck);
  const alignPhotoStyleToShoot = useAppStore((s) => s.alignPhotoStyleToShoot);
  const [aligningPhotoId, setAligningPhotoId] = useState<string | null>(null);

  const smartPreviewsGenerating = useAppStore((s) => s.smartPreviewsGenerating);
  const smartPreviewsGeneratedCount = useAppStore((s) => s.smartPreviewsGeneratedCount);
  const generateSmartPreviewsForSelection = useAppStore((s) => s.generateSmartPreviewsForSelection);
  const runPerceptualDuplicateDetection = useAppStore((s) => s.runPerceptualDuplicateDetection);

  const [newFolderName, setNewFolderName] = useState("");
  const [newSmartName, setNewSmartName] = useState("");
  const [smartCriteria, setSmartCriteria] = useState<RuleNode<SmartCollectionLeaf>>(() => groupNode("and", [conditionNode(makeDefaultSmartLeaf())]));
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
    await createSmartCollection(newSmartName.trim(), undefined, smartCriteria);
    setNewSmartName("");
    setSmartCriteria(groupNode("and", [conditionNode(makeDefaultSmartLeaf())]));
  }

  const virtualCopies = selectedPhotoId ? (virtualCopiesByPhotoId[selectedPhotoId] ?? []) : [];

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-16" onClick={onClose}>
      <div
        onClick={(e) => e.stopPropagation()}
        className="max-h-[85vh] w-full max-w-2xl overflow-y-auto rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
      >
        <h2 className="mb-1 text-sm font-semibold text-text-primary">{t("libraryOrganizeDialog.title")}</h2>
        <p className="mb-3 text-xs text-text-muted">{t("libraryOrganizeDialog.subtitle")}</p>

        <div className="mb-3 flex gap-1 border-b border-border pb-2">
          {(Object.keys(TAB_LABELS) as Tab[]).map((key) => (
            <button
              key={key}
              type="button"
              onClick={() => setTab(key)}
              className={`rounded px-2 py-1 text-xs ${tab === key ? "border border-accent bg-accent/10 text-accent" : "border border-border hover:border-accent"}`}
            >
              {TAB_LABELS[key]}
            </button>
          ))}
        </div>

        {tab === "sets" && (
          <div className="flex flex-col gap-3">
            <div>
              <p className="mb-1 text-xs font-semibold text-text-secondary">{t("libraryOrganizeDialog.collectionSets")}</p>
              <ul className="mb-2 flex flex-col gap-1">
                {collectionFolders.map((f) => (
                  <li key={f.id} className="flex items-center justify-between rounded border border-border px-2 py-1 text-xs">
                    <span>{f.name}</span>
                    <button type="button" onClick={() => void deleteCollectionFolder(f.id)} className="rounded border border-border px-1.5 py-0.5 hover:border-danger">
                      {t("libraryOrganizeDialog.delete")}
                    </button>
                  </li>
                ))}
                {collectionFolders.length === 0 && <li className="text-xs text-text-muted">{t("libraryOrganizeDialog.noCollectionSets")}</li>}
              </ul>
              <div className="flex gap-1">
                <input type="text" value={newFolderName} onChange={(e) => setNewFolderName(e.target.value)} placeholder={t("libraryOrganizeDialog.newCollectionSet")} className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs" />
                <button type="button" onClick={() => void handleCreateFolder()} className="shrink-0 rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent">
                  {t("libraryOrganizeDialog.create")}
                </button>
              </div>
            </div>

            <div>
              <p className="mb-1 text-xs font-semibold text-text-secondary">{t("libraryOrganizeDialog.collections")}</p>
              <ul className="mb-2 flex flex-col gap-1">
                {collections.map((c) => (
                  <li key={c.id} className="flex items-center justify-between rounded border border-border px-2 py-1 text-xs">
                    <span>
                      {c.name} {c.is_smart && <span className="text-text-muted">{t("libraryOrganizeDialog.smartSuffix")}</span>}
                    </span>
                    <select
                      value={c.folder_id ?? ""}
                      onChange={(e) => void moveCollectionToFolder(c.id, e.target.value || null)}
                      className="rounded border border-border bg-bg-panel px-1 py-0.5 text-xs"
                    >
                      <option value="">{t("libraryOrganizeDialog.root")}</option>
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
              <p className="mb-2 text-xs font-semibold text-text-secondary">{t("libraryOrganizeDialog.newSmartCollection")}</p>
              <input type="text" value={newSmartName} onChange={(e) => setNewSmartName(e.target.value)} placeholder={t("libraryOrganizeDialog.name")} className="mb-2 w-full rounded border border-border bg-bg-panel px-2 py-1 text-xs" />
              <div className="mb-2">
                <RuleTreeEditor
                  node={smartCriteria}
                  onChange={setSmartCriteria}
                  makeDefaultLeaf={makeDefaultSmartLeaf}
                  renderLeaf={(leaf, onLeafChange) => (
                    <>
                      <select
                        aria-label="Feld"
                        value={leaf.field}
                        onChange={(e) => onLeafChange({ ...leaf, field: e.target.value as SmartCollectionLeaf["field"] })}
                        className="min-w-0 rounded border border-border bg-bg-panel px-1 py-0.5"
                      >
                        {SMART_COLLECTION_FIELD_OPTIONS.map((option) => (
                          <option key={option.value} value={option.value}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                      <select
                        aria-label="Operator"
                        value={leaf.op}
                        onChange={(e) => onLeafChange({ ...leaf, op: e.target.value as SmartCollectionLeaf["op"] })}
                        className="min-w-0 rounded border border-border bg-bg-panel px-1 py-0.5"
                      >
                        {SMART_COLLECTION_OPERATOR_OPTIONS.map((option) => (
                          <option key={option.value} value={option.value}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                      <input
                        type="text"
                        aria-label="Wert"
                        value={leaf.value}
                        onChange={(e) => onLeafChange({ ...leaf, value: e.target.value })}
                        className="w-16 min-w-0 rounded border border-border bg-bg-panel px-1 py-0.5"
                      />
                    </>
                  )}
                />
              </div>
              <button type="button" onClick={() => void handleCreateSmartCollection()} className="w-full rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent">
                {t("libraryOrganizeDialog.createSmartCollection")}
              </button>
            </div>
          </div>
        )}

        {tab === "stacks" && (
          <div className="flex flex-col gap-3">
            <div className="flex gap-2">
              <input type="text" value={newStackName} onChange={(e) => setNewStackName(e.target.value)} placeholder={t("libraryOrganizeDialog.stackNamePlaceholder")} className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs" />
              <button type="button" onClick={() => void createStackFromSelection(newStackName || undefined)} className="shrink-0 rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent">
                {t("libraryOrganizeDialog.stackFromSelection")}
              </button>
            </div>
            <div className="flex gap-2">
              <label className="flex items-center gap-1 text-xs text-text-secondary">
                {t("libraryOrganizeDialog.timeWindow")}
                <input type="number" min={1} value={autoStackWindow} onChange={(e) => setAutoStackWindow(Number(e.target.value))} className="w-20 rounded border border-border bg-bg-panel px-2 py-1" />
              </label>
              <button type="button" onClick={() => void autoStackSelectionByTime(autoStackWindow)} className="rounded border border-border px-2 py-1 text-xs hover:border-accent">
                {t("libraryOrganizeDialog.autoStackSelection")}
              </button>
            </div>
            <ul className="flex flex-col gap-1">
              {stacks.map((s) => (
                <li key={s.id} className="flex items-center justify-between rounded border border-border px-2 py-1 text-xs">
                  <span>
                    {s.name ?? t("libraryOrganizeDialog.stackFallbackName")} — {t("libraryOrganizeDialog.photoCount", { count: s.photo_ids.length })}
                  </span>
                  <button type="button" onClick={() => void deleteStack(s.id)} className="rounded border border-border px-1.5 py-0.5 hover:border-danger">
                    {t("libraryOrganizeDialog.delete")}
                  </button>
                </li>
              ))}
              {stacks.length === 0 && <li className="text-xs text-text-muted">{t("libraryOrganizeDialog.noStacks")}</li>}
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
              {t("libraryOrganizeDialog.createVirtualCopy")}
            </button>
            {!selectedPhotoId && <p className="text-xs text-text-muted">{t("libraryOrganizeDialog.noPhotoSelected")}</p>}
            <ul className="flex flex-col gap-1">
              {virtualCopies.map((copy) => (
                <li key={copy.id} className="flex items-center justify-between rounded border border-border px-2 py-1 text-xs">
                  <span>{copy.filename} — {copy.rating}★</span>
                  <button type="button" onClick={() => selectPhoto(copy.id)} className="rounded border border-border px-1.5 py-0.5 hover:border-accent">
                    {t("libraryOrganizeDialog.open")}
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
                    {t("libraryOrganizeDialog.delete")}
                  </button>
                </li>
              ))}
            </ul>
            <div className="rounded border border-border p-2">
              <p className="mb-2 text-xs font-semibold text-text-secondary">{t("libraryOrganizeDialog.newColorLabel")}</p>
              <div className="mb-2 flex gap-2">
                <input type="text" value={newLabelName} onChange={(e) => setNewLabelName(e.target.value)} placeholder={t("libraryOrganizeDialog.internalNamePlaceholder")} className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs" />
                <input type="text" value={newLabelDisplayName} onChange={(e) => setNewLabelDisplayName(e.target.value)} placeholder={t("libraryOrganizeDialog.displayNamePlaceholder")} className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs" />
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
                {t("libraryOrganizeDialog.create")}
              </button>
            </div>
          </div>
        )}

        {tab === "duplicates" && (
          <div className="flex flex-col gap-3">
            <div className="flex gap-2">
              <label className="flex items-center gap-1 text-xs text-text-secondary">
                {t("libraryOrganizeDialog.similarityThreshold")}
                <input type="number" min={0} max={64} value={maxDistance} onChange={(e) => setMaxDistance(Number(e.target.value))} className="w-16 rounded border border-border bg-bg-panel px-2 py-1" />
              </label>
              <button
                type="button"
                onClick={() => void runPerceptualDuplicateDetection(maxDistance)}
                disabled={perceptualDuplicatesRunning}
                className="rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
              >
                {perceptualDuplicatesRunning ? t("libraryOrganizeDialog.searching") : t("libraryOrganizeDialog.searchDuplicates")}
              </button>
            </div>
            {perceptualDuplicateGroups.length === 0 && !perceptualDuplicatesRunning && <p className="text-xs text-text-muted">{t("libraryOrganizeDialog.noGroupsFound")}</p>}
            <ul className="flex flex-col gap-2">
              {perceptualDuplicateGroups.map((group, index) => {
                const best = suggestBestPhoto(group);
                return (
                  <li key={index} className="rounded border border-border p-2 text-xs">
                    <p className="mb-1 text-text-muted">{t("libraryOrganizeDialog.groupLabel", { index: index + 1, count: group.length })}</p>
                    {group.map((photo) => (
                      <div key={photo.id} className="flex items-center justify-between py-0.5">
                        <span>
                          {photo.filename} {best?.id === photo.id && <span className="text-accent">{t("libraryOrganizeDialog.suggestionSuffix")}</span>}
                        </span>
                        <button type="button" onClick={() => selectPhoto(photo.id)} className="rounded border border-border px-1.5 py-0.5 hover:border-accent">
                          {t("libraryOrganizeDialog.open")}
                        </button>
                      </div>
                    ))}
                  </li>
                );
              })}
            </ul>
          </div>
        )}

        {tab === "style" && (
          <div className="flex flex-col gap-3">
            {!selectedFolderId && <p className="text-xs text-text-muted">{t("libraryOrganizeDialog.styleNoFolderSelected")}</p>}
            {selectedFolderId && (
              <>
                <div className="flex gap-2">
                  <button
                    type="button"
                    onClick={() => void runStyleConsistencyCheck()}
                    disabled={styleConsistencyRunning}
                    className="rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {styleConsistencyRunning ? t("libraryOrganizeDialog.styleCheckRunning") : t("libraryOrganizeDialog.styleCheckRun")}
                  </button>
                </div>
                {styleConsistencyResult !== null && styleConsistencyResult.length > 0 && styleConsistencyResult.length < 3 && (
                  <p className="text-xs text-text-muted">{t("libraryOrganizeDialog.styleTooFewPhotos")}</p>
                )}
                {styleConsistencyResult !== null && styleConsistencyResult.length >= 3 && !styleConsistencyResult.some((analysis) => analysis.is_outlier) && (
                  <p className="text-xs text-text-muted">{t("libraryOrganizeDialog.styleNoOutliers")}</p>
                )}
                <ul className="flex flex-col gap-2">
                  {(styleConsistencyResult ?? [])
                    .filter((analysis) => analysis.is_outlier)
                    .map((analysis) => (
                      <li key={analysis.photo.id} className="rounded border border-border p-2 text-xs">
                        <div className="flex items-center justify-between gap-2">
                          <span>
                            {analysis.photo.filename} <span className="text-danger">{t("libraryOrganizeDialog.styleOutlierBadge")}</span>{" "}
                            <span className="text-text-muted">{t("libraryOrganizeDialog.styleDistance", { distance: analysis.distance_from_group.toFixed(2) })}</span>
                          </span>
                          <div className="flex shrink-0 items-center gap-1">
                            <button type="button" onClick={() => selectPhoto(analysis.photo.id)} className="rounded border border-border px-1.5 py-0.5 hover:border-accent">
                              {t("libraryOrganizeDialog.open")}
                            </button>
                            <button
                              type="button"
                              onClick={async () => {
                                setAligningPhotoId(analysis.photo.id);
                                try {
                                  await alignPhotoStyleToShoot(analysis);
                                } finally {
                                  setAligningPhotoId(null);
                                }
                              }}
                              disabled={aligningPhotoId === analysis.photo.id}
                              className="rounded border border-accent bg-accent/10 px-1.5 py-0.5 text-accent disabled:cursor-not-allowed disabled:opacity-50"
                            >
                              {aligningPhotoId === analysis.photo.id ? t("libraryOrganizeDialog.styleAligning") : t("libraryOrganizeDialog.styleAlign")}
                            </button>
                          </div>
                        </div>
                      </li>
                    ))}
                </ul>
              </>
            )}
          </div>
        )}

        <div className="mt-3 flex items-center justify-between gap-2 border-t border-border pt-3">
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => void generateSmartPreviewsForSelection()}
              disabled={smartPreviewsGenerating}
              title={t("libraryOrganizeDialog.smartPreviewsTitle")}
              className="rounded border border-border px-2 py-1 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-50"
            >
              {smartPreviewsGenerating ? t("libraryOrganizeDialog.generating") : t("libraryOrganizeDialog.generateSmartPreviews")}
            </button>
            {smartPreviewsGeneratedCount !== null && !smartPreviewsGenerating && (
              <span className="text-xs text-text-muted">{t("libraryOrganizeDialog.generatedCount", { count: smartPreviewsGeneratedCount })}</span>
            )}
          </div>
          <button type="button" onClick={onClose} className="rounded border border-border px-3 py-1 text-xs hover:border-accent">
            {t("libraryOrganizeDialog.close")}
          </button>
        </div>
      </div>
    </div>
  );
}
