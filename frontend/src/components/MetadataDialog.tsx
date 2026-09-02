import { useEffect, useState } from "react";

import { useT } from "../lib/i18n";
import type { PresetCondition, PresetConditionField, PresetConditionOperator } from "../lib/presets";
import { PRESET_CONDITION_FIELD_OPTIONS, PRESET_CONDITION_OPERATOR_OPTIONS } from "../lib/presets";
import { useAppStore } from "../store";

interface MetadataDialogProps {
  open: boolean;
  onClose: () => void;
}

type Tab = "keywords" | "rules" | "fields";

/**
 * Metadaten-Dialog (Phase 9 Schritt 2, siehe `PLAN.md`/`DECISIONS.md`
 * ADR-0035) — Schlagworthierarchie/Synonyme, bedingte Auto-Tag-Regeln,
 * IPTC-artige Feldbearbeitung sowie Adobe-XMP-Sidecar-Export/-Import,
 * analog zum `LibraryOrganizeDialog.tsx`-Muster aus Schritt 1.
 */
export function MetadataDialog({ open, onClose }: MetadataDialogProps) {
  const t = useT();
  const [tab, setTab] = useState<Tab>("keywords");

  const TAB_LABELS: Record<Tab, string> = {
    keywords: t("metadataDialog.tabKeywords"),
    rules: t("metadataDialog.tabRules"),
    fields: t("metadataDialog.tabFields"),
  };

  const keywords = useAppStore((s) => s.keywords);
  const refreshKeywords = useAppStore((s) => s.refreshKeywords);
  const setKeywordParent = useAppStore((s) => s.setKeywordParent);
  const setKeywordSynonyms = useAppStore((s) => s.setKeywordSynonyms);
  const deleteKeywordEntry = useAppStore((s) => s.deleteKeywordEntry);

  const tagRules = useAppStore((s) => s.tagRules);
  const refreshTagRules = useAppStore((s) => s.refreshTagRules);
  const createTagRule = useAppStore((s) => s.createTagRule);
  const setTagRuleEnabled = useAppStore((s) => s.setTagRuleEnabled);
  const deleteTagRule = useAppStore((s) => s.deleteTagRule);

  const selectedFolderId = useAppStore((s) => s.selectedFolderId);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const photosInFolder = useAppStore((s) => (selectedFolderId ? s.photosByFolder[selectedFolderId] : undefined));
  const updatePhotoMetadata = useAppStore((s) => s.updatePhotoMetadata);
  const xmpStatus = useAppStore((s) => s.xmpStatus);
  const exportXmpSidecarForSelected = useAppStore((s) => s.exportXmpSidecarForSelected);
  const importXmpSidecarForSelected = useAppStore((s) => s.importXmpSidecarForSelected);
  const updatePhotoCustomMetadata = useAppStore((s) => s.updatePhotoCustomMetadata);
  const wellKnownIptcFields = useAppStore((s) => s.wellKnownIptcFields);
  const refreshWellKnownIptcFields = useAppStore((s) => s.refreshWellKnownIptcFields);

  const [synonymDrafts, setSynonymDrafts] = useState<Record<string, string>>({});
  const [ruleName, setRuleName] = useState("");
  const [ruleKeywordId, setRuleKeywordId] = useState("");
  const [ruleField, setRuleField] = useState<PresetConditionField>("camera_model");
  const [ruleOp, setRuleOp] = useState<PresetConditionOperator>("contains");
  const [ruleValue, setRuleValue] = useState("");
  const [titleDraft, setTitleDraft] = useState("");
  const [captionDraft, setCaptionDraft] = useState("");
  const [copyrightDraft, setCopyrightDraft] = useState("");
  const [creatorDraft, setCreatorDraft] = useState("");
  const [withDevelopSettings, setWithDevelopSettings] = useState(true);
  // Phase 12 Schritt 4 (siehe DECISIONS.md ADR-0039): voller EXIF/IPTC-
  // Editor — ein Draft für die gesamte custom_metadata-Map (wohlbekannte
  // Felder + frei benannte Zusatzfelder zusammen), plus ein separates
  // Eingabepaar für ein neues, noch unbenanntes Zusatzfeld.
  const [customMetadataDraft, setCustomMetadataDraft] = useState<Record<string, string>>({});
  const [newFieldKey, setNewFieldKey] = useState("");
  const [newFieldValue, setNewFieldValue] = useState("");

  useEffect(() => {
    if (!open) return;
    void refreshKeywords();
    void refreshTagRules();
    void refreshWellKnownIptcFields();
  }, [open, refreshKeywords, refreshTagRules, refreshWellKnownIptcFields]);

  const selectedPhoto = selectedPhotoId ? photosInFolder?.find((p) => p.id === selectedPhotoId) : undefined;

  useEffect(() => {
    if (!open || !selectedPhoto) return;
    setTitleDraft(selectedPhoto.title ?? "");
    setCaptionDraft(selectedPhoto.caption ?? "");
    setCopyrightDraft(selectedPhoto.copyright ?? "");
    setCreatorDraft(selectedPhoto.creator ?? "");
    setCustomMetadataDraft(selectedPhoto.custom_metadata ?? {});
    setNewFieldKey("");
    setNewFieldValue("");
  }, [open, selectedPhoto]);

  if (!open) return null;

  async function handleCreateRule() {
    if (!ruleName.trim() || !ruleKeywordId) return;
    const condition: PresetCondition = { field: ruleField, op: ruleOp, value: ruleValue, section: null };
    await createTagRule(ruleName.trim(), ruleKeywordId, [condition]);
    setRuleName("");
    setRuleValue("");
  }

  async function handleSaveFields() {
    if (!selectedPhotoId) return;
    await updatePhotoMetadata(selectedPhotoId, {
      title: titleDraft || null,
      caption: captionDraft || null,
      copyright: copyrightDraft || null,
      creator: creatorDraft || null,
    });
  }

  async function handleSaveCustomMetadata() {
    if (!selectedPhotoId) return;
    await updatePhotoCustomMetadata(selectedPhotoId, customMetadataDraft);
  }

  function handleAddCustomField() {
    const key = newFieldKey.trim();
    if (!key) return;
    setCustomMetadataDraft((prev) => ({ ...prev, [key]: newFieldValue }));
    setNewFieldKey("");
    setNewFieldValue("");
  }

  function handleRemoveCustomField(key: string) {
    setCustomMetadataDraft((prev) => {
      const next = { ...prev };
      delete next[key];
      return next;
    });
  }

  const wellKnownKeys = new Set(wellKnownIptcFields.map(([key]) => key));
  const extraFieldEntries = Object.entries(customMetadataDraft).filter(([key]) => !wellKnownKeys.has(key));

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-16" onClick={onClose}>
      <div
        onClick={(e) => e.stopPropagation()}
        className="max-h-[85vh] w-full max-w-2xl overflow-y-auto rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
      >
        <h2 className="mb-1 text-sm font-semibold text-text-primary">{t("metadataDialog.title")}</h2>
        <p className="mb-3 text-xs text-text-muted">{t("metadataDialog.subtitle")}</p>

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

        {tab === "keywords" && (
          <div className="flex flex-col gap-2">
            {keywords.length === 0 && <p className="text-xs text-text-muted">{t("metadataDialog.noKeywords")}</p>}
            {keywords.map((k) => (
              <div key={k.id} className="rounded border border-border p-2 text-xs">
                <div className="mb-1 flex items-center justify-between">
                  <span className="font-medium">{k.name}</span>
                  <button type="button" onClick={() => void deleteKeywordEntry(k.id)} className="rounded border border-border px-1.5 py-0.5 hover:border-danger">
                    {t("metadataDialog.delete")}
                  </button>
                </div>
                <label className="mb-1 flex items-center gap-2 text-text-secondary">
                  {t("metadataDialog.parentKeyword")}
                  <select
                    value={k.parent_id ?? ""}
                    onChange={(e) => void setKeywordParent(k.id, e.target.value || null)}
                    className="rounded border border-border bg-bg-panel px-1 py-0.5"
                  >
                    <option value="">{t("metadataDialog.rootKeyword")}</option>
                    {keywords
                      .filter((other) => other.id !== k.id)
                      .map((other) => (
                        <option key={other.id} value={other.id}>
                          {other.name}
                        </option>
                      ))}
                  </select>
                </label>
                <label className="flex items-center gap-2 text-text-secondary">
                  {t("metadataDialog.synonyms")}
                  <input
                    type="text"
                    value={synonymDrafts[k.id] ?? k.synonyms.join(", ")}
                    onChange={(e) => setSynonymDrafts((d) => ({ ...d, [k.id]: e.target.value }))}
                    onBlur={(e) => {
                      const list = e.target.value.split(",").map((s) => s.trim()).filter(Boolean);
                      void setKeywordSynonyms(k.id, list);
                    }}
                    className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-1 py-0.5"
                  />
                </label>
              </div>
            ))}
          </div>
        )}

        {tab === "rules" && (
          <div className="flex flex-col gap-3">
            <ul className="flex flex-col gap-1">
              {tagRules.map((r) => (
                <li key={r.id} className="flex items-center justify-between rounded border border-border px-2 py-1 text-xs">
                  <label className="flex items-center gap-2">
                    <input type="checkbox" checked={r.enabled} onChange={(e) => void setTagRuleEnabled(r.id, e.target.checked)} />
                    {r.name}
                  </label>
                  <button type="button" onClick={() => void deleteTagRule(r.id)} className="rounded border border-border px-1.5 py-0.5 hover:border-danger">
                    {t("metadataDialog.delete")}
                  </button>
                </li>
              ))}
              {tagRules.length === 0 && <li className="text-xs text-text-muted">{t("metadataDialog.noRules")}</li>}
            </ul>

            <div className="rounded border border-border p-2">
              <p className="mb-2 text-xs font-semibold text-text-secondary">{t("metadataDialog.newRule")}</p>
              <input
                type="text"
                value={ruleName}
                onChange={(e) => setRuleName(e.target.value)}
                placeholder={t("metadataDialog.ruleNamePlaceholder")}
                className="mb-2 w-full rounded border border-border bg-bg-panel px-2 py-1 text-xs"
              />
              <select value={ruleKeywordId} onChange={(e) => setRuleKeywordId(e.target.value)} className="mb-2 w-full rounded border border-border bg-bg-panel px-2 py-1 text-xs">
                <option value="">{t("metadataDialog.chooseTargetKeyword")}</option>
                {keywords.map((k) => (
                  <option key={k.id} value={k.id}>
                    {k.name}
                  </option>
                ))}
              </select>
              <div className="mb-2 flex gap-1">
                <select value={ruleField} onChange={(e) => setRuleField(e.target.value as PresetConditionField)} className="rounded border border-border bg-bg-panel px-1 py-1 text-xs">
                  {PRESET_CONDITION_FIELD_OPTIONS.map((o) => (
                    <option key={o.value} value={o.value}>
                      {o.label}
                    </option>
                  ))}
                </select>
                <select value={ruleOp} onChange={(e) => setRuleOp(e.target.value as PresetConditionOperator)} className="rounded border border-border bg-bg-panel px-1 py-1 text-xs">
                  {PRESET_CONDITION_OPERATOR_OPTIONS.map((o) => (
                    <option key={o.value} value={o.value}>
                      {o.label}
                    </option>
                  ))}
                </select>
                <input type="text" value={ruleValue} onChange={(e) => setRuleValue(e.target.value)} placeholder={t("metadataDialog.valuePlaceholder")} className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs" />
              </div>
              <button type="button" onClick={() => void handleCreateRule()} className="w-full rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent">
                {t("metadataDialog.createRule")}
              </button>
            </div>
          </div>
        )}

        {tab === "fields" && (
          <div className="flex flex-col gap-3">
            {!selectedPhoto && <p className="text-xs text-text-muted">{t("metadataDialog.noPhotoSelected")}</p>}
            {selectedPhoto && (
              <>
                <label className="flex flex-col gap-1 text-xs text-text-secondary">
                  {t("metadataDialog.fieldTitle")}
                  <input type="text" value={titleDraft} onChange={(e) => setTitleDraft(e.target.value)} className="rounded border border-border bg-bg-panel px-2 py-1" />
                </label>
                <label className="flex flex-col gap-1 text-xs text-text-secondary">
                  {t("metadataDialog.fieldCaption")}
                  <input type="text" value={captionDraft} onChange={(e) => setCaptionDraft(e.target.value)} className="rounded border border-border bg-bg-panel px-2 py-1" />
                </label>
                <label className="flex flex-col gap-1 text-xs text-text-secondary">
                  {t("metadataDialog.fieldCopyright")}
                  <input type="text" value={copyrightDraft} onChange={(e) => setCopyrightDraft(e.target.value)} className="rounded border border-border bg-bg-panel px-2 py-1" />
                </label>
                <label className="flex flex-col gap-1 text-xs text-text-secondary">
                  {t("metadataDialog.fieldCreator")}
                  <input type="text" value={creatorDraft} onChange={(e) => setCreatorDraft(e.target.value)} className="rounded border border-border bg-bg-panel px-2 py-1" />
                </label>
                <button type="button" onClick={() => void handleSaveFields()} className="w-full rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent">
                  {t("metadataDialog.saveMetadata")}
                </button>

                {/* Voller EXIF/IPTC-Editor (Phase 12 Schritt 4, siehe
                    DECISIONS.md ADR-0039) — wohlbekannte IPTC-Kernfelder
                    plus frei benannte Zusatzfelder, beide in derselben
                    custom_metadata-Map gespeichert (siehe apx_catalog::iptc). */}
                <div className="rounded border border-border p-2">
                  <p className="mb-2 text-xs font-semibold text-text-secondary">{t("metadataDialog.iptcFields")}</p>
                  <div className="flex flex-col gap-2">
                    {wellKnownIptcFields.map(([key, label]) => (
                      <label key={key} className="flex flex-col gap-1 text-xs text-text-secondary">
                        {label}
                        <input
                          type="text"
                          value={customMetadataDraft[key] ?? ""}
                          onChange={(e) => setCustomMetadataDraft((prev) => ({ ...prev, [key]: e.target.value }))}
                          className="rounded border border-border bg-bg-panel px-2 py-1"
                        />
                      </label>
                    ))}
                  </div>

                  <p className="mb-1 mt-3 text-xs font-semibold text-text-secondary">{t("metadataDialog.customFields")}</p>
                  {extraFieldEntries.length === 0 && <p className="mb-2 text-xs text-text-muted">{t("metadataDialog.noCustomFields")}</p>}
                  <ul className="mb-2 flex flex-col gap-1">
                    {extraFieldEntries.map(([key, value]) => (
                      <li key={key} className="flex items-center gap-2 text-xs">
                        <span className="w-1/3 truncate text-text-secondary" title={key}>
                          {key}
                        </span>
                        <input
                          type="text"
                          value={value}
                          onChange={(e) => setCustomMetadataDraft((prev) => ({ ...prev, [key]: e.target.value }))}
                          className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1"
                        />
                        <button type="button" onClick={() => handleRemoveCustomField(key)} className="rounded border border-border px-1.5 py-0.5 hover:border-danger">
                          {t("metadataDialog.delete")}
                        </button>
                      </li>
                    ))}
                  </ul>
                  <div className="mb-2 flex gap-1">
                    <input
                      type="text"
                      value={newFieldKey}
                      onChange={(e) => setNewFieldKey(e.target.value)}
                      placeholder={t("metadataDialog.customFieldKeyPlaceholder")}
                      className="w-1/3 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
                    />
                    <input
                      type="text"
                      value={newFieldValue}
                      onChange={(e) => setNewFieldValue(e.target.value)}
                      placeholder={t("metadataDialog.customFieldValuePlaceholder")}
                      className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
                    />
                    <button type="button" onClick={handleAddCustomField} className="rounded border border-border px-2 py-1 text-xs hover:border-accent">
                      {t("metadataDialog.addCustomField")}
                    </button>
                  </div>

                  <button type="button" onClick={() => void handleSaveCustomMetadata()} className="w-full rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent">
                    {t("metadataDialog.saveIptcFields")}
                  </button>
                </div>

                <div className="rounded border border-border p-2">
                  <p className="mb-2 text-xs font-semibold text-text-secondary">{t("metadataDialog.xmpSidecar")}</p>
                  <label className="mb-2 flex items-center gap-2 text-xs text-text-secondary">
                    <input type="checkbox" checked={withDevelopSettings} onChange={(e) => setWithDevelopSettings(e.target.checked)} />
                    {t("metadataDialog.includeDevelopSettings")}
                  </label>
                  <div className="flex gap-2">
                    <button type="button" onClick={() => void exportXmpSidecarForSelected(withDevelopSettings)} className="flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs hover:border-accent">
                      {t("metadataDialog.exportXmp")}
                    </button>
                    <button type="button" onClick={() => void importXmpSidecarForSelected()} className="flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs hover:border-accent">
                      {t("metadataDialog.importXmp")}
                    </button>
                  </div>
                  {xmpStatus && <p className="mt-2 text-xs text-text-muted">{xmpStatus}</p>}
                </div>
              </>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
