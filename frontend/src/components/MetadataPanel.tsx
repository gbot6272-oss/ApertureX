import { useState } from "react";
import { useShallow } from "zustand/react/shallow";

import { formatShutter } from "../lib/format";
import { selectActivePhotos, useAppStore } from "../store";
import { PaletteFrame } from "./PaletteFrame";
import { ColorLabelPicker, FlagToggle, RatingStars } from "./RatingFlagColor";

/**
 * Metadaten-Panel (Phase 3, Schritt 6): strukturell wie `DevelopPanel.tsx`
 * — ein zusätzliches, ein-/ausblendbares rechtes Seitenpanel, umgeschaltet
 * über einen Header-Knopf (siehe `store/index.ts`s `metadataPanelOpen`).
 * Zeigt die bestehenden `PhotoDto`-Felder read-only plus Bewertung/
 * Flagge/Farbe/Schlagworte editierbar (siehe `PLAN.md` Phase 3, Schritt 6).
 */
export function MetadataPanel() {
  const open = useAppStore((s) => s.metadataPanelOpen);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  // `useShallow` statt einer bloßen Referenzprüfung — siehe `GridView.tsx`
  // für die Begründung (sonst Endlos-Rerender bei leerer Auswahl).
  const photos = useAppStore(useShallow(selectActivePhotos));
  const photo = photos.find((p) => p.id === selectedPhotoId) ?? null;
  const keywords = useAppStore((s) => (selectedPhotoId ? s.photoKeywords[selectedPhotoId] : undefined)) ?? [];
  const setPhotoRating = useAppStore((s) => s.setPhotoRating);
  const setPhotoFlag = useAppStore((s) => s.setPhotoFlag);
  const setPhotoColorLabel = useAppStore((s) => s.setPhotoColorLabel);
  const addKeywordToPhoto = useAppStore((s) => s.addKeywordToPhoto);
  const removeKeywordFromPhoto = useAppStore((s) => s.removeKeywordFromPhoto);
  const tagSuggestions = useAppStore((s) => s.tagSuggestions);
  const tagSuggestionsLoading = useAppStore((s) => s.tagSuggestionsLoading);
  const fetchTagSuggestions = useAppStore((s) => s.fetchTagSuggestions);
  const acceptTagSuggestion = useAppStore((s) => s.acceptTagSuggestion);
  const clearTagSuggestions = useAppStore((s) => s.clearTagSuggestions);

  const [newKeyword, setNewKeyword] = useState("");

  if (!open) return null;

  async function handleAddKeyword() {
    if (!photo || !newKeyword.trim()) return;
    await addKeywordToPhoto(photo.id, newKeyword);
    setNewKeyword("");
  }

  return (
    <PaletteFrame id="metadata" side="right" defaultWidth={288} label="Metadaten" className="gap-4 border-l border-border bg-bg-raised p-3">
      <h2 className="text-sm font-semibold text-text-primary">Metadaten</h2>

      {!photo && <p className="text-xs text-text-muted">Kein Foto ausgewählt.</p>}

      {photo && (
        <>
          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between">
              <span className="text-xs text-text-secondary">Bewertung</span>
              <RatingStars rating={photo.rating} onChange={(rating) => void setPhotoRating(photo.id, rating)} />
            </div>
            <div className="flex items-center justify-between">
              <span className="text-xs text-text-secondary">Flagge</span>
              <FlagToggle flag={photo.flag} onChange={(flag) => void setPhotoFlag(photo.id, flag)} />
            </div>
            <div className="flex items-center justify-between">
              <span className="text-xs text-text-secondary">Farbe</span>
              <ColorLabelPicker colorLabel={photo.color_label} onChange={(color) => void setPhotoColorLabel(photo.id, color)} />
            </div>
          </div>

          <div className="flex flex-col gap-1">
            <span className="text-xs font-medium text-text-secondary">Schlagworte</span>
            <div className="flex flex-wrap gap-1">
              {keywords.length === 0 && <span className="text-xs text-text-muted">Keine.</span>}
              {keywords.map((keyword) => (
                <span key={keyword.id} className="flex items-center gap-1 rounded bg-bg-panel px-1.5 py-0.5 text-xs">
                  {keyword.name}
                  <button
                    type="button"
                    onClick={() => void removeKeywordFromPhoto(photo.id, keyword.id)}
                    aria-label={`Schlagwort "${keyword.name}" entfernen`}
                    className="text-text-muted hover:text-danger"
                  >
                    ×
                  </button>
                </span>
              ))}
            </div>
            <div className="flex gap-1">
              <input
                type="text"
                value={newKeyword}
                onChange={(event) => setNewKeyword(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void handleAddKeyword();
                }}
                placeholder="Neues Schlagwort…"
                className="w-full rounded border border-border bg-bg-panel px-2 py-1 text-xs"
              />
              <button
                type="button"
                onClick={() => void handleAddKeyword()}
                className="shrink-0 rounded border border-border px-2 py-1 text-xs hover:border-accent"
              >
                +
              </button>
            </div>

            {/* Auto-Tagging (Phase 7 Schritt 5, siehe DECISIONS.md ADR-0033)
                — regelbasierte Vorschläge aus Segmentierungs-Heuristiken +
                EXIF, keine Klassifikation. Schreibt nichts in den Katalog,
                bis der Nutzer einen Vorschlag ausdrücklich übernimmt. */}
            <div className="flex items-center gap-1">
              <button
                type="button"
                disabled={tagSuggestionsLoading}
                onClick={() => void fetchTagSuggestions(photo.id)}
                className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
              >
                {tagSuggestionsLoading ? "Analysiere…" : "Tag-Vorschläge"}
              </button>
              {tagSuggestions.length > 0 && (
                <button type="button" onClick={clearTagSuggestions} className="text-xs text-text-muted hover:text-danger">
                  Verwerfen
                </button>
              )}
            </div>
            {tagSuggestions.length > 0 && (
              <div className="flex flex-wrap gap-1">
                {tagSuggestions.map((tag) => (
                  <button
                    key={tag}
                    type="button"
                    onClick={() => void acceptTagSuggestion(photo.id, tag)}
                    title="Als Schlagwort übernehmen"
                    className="rounded border border-dashed border-accent/50 px-1.5 py-0.5 text-xs text-accent hover:bg-accent/10"
                  >
                    + {tag}
                  </button>
                ))}
              </div>
            )}
          </div>

          <dl className="grid grid-cols-[auto_1fr] gap-x-2 gap-y-1 text-xs">
            <dt className="text-text-muted">Datei</dt>
            <dd className="truncate text-text-secondary" title={photo.filename}>
              {photo.filename}
            </dd>
            {photo.width && photo.height && (
              <>
                <dt className="text-text-muted">Auflösung</dt>
                <dd className="text-text-secondary">
                  {photo.width} × {photo.height}
                </dd>
              </>
            )}
            {photo.camera_make && (
              <>
                <dt className="text-text-muted">Kamera</dt>
                <dd className="text-text-secondary">
                  {photo.camera_make} {photo.camera_model}
                </dd>
              </>
            )}
            {photo.lens && (
              <>
                <dt className="text-text-muted">Objektiv</dt>
                <dd className="text-text-secondary">{photo.lens}</dd>
              </>
            )}
            {photo.iso && (
              <>
                <dt className="text-text-muted">ISO</dt>
                <dd className="text-text-secondary">{photo.iso}</dd>
              </>
            )}
            {photo.aperture && (
              <>
                <dt className="text-text-muted">Blende</dt>
                <dd className="text-text-secondary">f/{photo.aperture}</dd>
              </>
            )}
            {photo.shutter && (
              <>
                <dt className="text-text-muted">Belichtung</dt>
                <dd className="text-text-secondary">{formatShutter(photo.shutter)}</dd>
              </>
            )}
            {photo.focal_length && (
              <>
                <dt className="text-text-muted">Brennweite</dt>
                <dd className="text-text-secondary">{photo.focal_length} mm</dd>
              </>
            )}
            {photo.captured_at && (
              <>
                <dt className="text-text-muted">Aufgenommen</dt>
                <dd className="text-text-secondary">{new Date(photo.captured_at).toLocaleString()}</dd>
              </>
            )}
          </dl>
        </>
      )}
    </PaletteFrame>
  );
}
