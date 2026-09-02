import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import { previewUrl } from "../lib/media";
import { resolveSelectionMode, selectActivePhotos, useAppStore } from "../store";
import { QuickDevelopOverlay } from "./QuickDevelopOverlay";
import { ColorLabelPicker, FlagToggle, RatingStars } from "./RatingFlagColor";

// Feste Zellgröße statt individueller Seitenverhältnisse — hält die
// Virtualisierung einfach (siehe `Filmstrip.tsx`s selbe Entscheidung) und
// muss bei Fenstergrößenänderung nur die Spaltenzahl neu berechnen, nicht
// jede Zellgröße einzeln.
const CELL_SIZE = 168;
const CELL_GAP = 8;

// Übersichtsansicht (Phase 11 Schritt 3): deutlich größere Kacheln als das
// normale Raster — ein Sichtungsmodus zum Durchblättern, nicht zum
// Massen-Markieren, siehe Moduldoku unten.
const OVERVIEW_CELL_SIZE = 360;
const OVERVIEW_CELL_GAP = 16;

interface GridViewProps {
  /** `"grid"` (Standard) ist das dichte Mehrfachauswahl-Raster aus Phase 3
   * Schritt 6; `"overview"` ist die Übersichtsansicht aus Phase 11
   * Schritt 3 — größere Kacheln, reduzierte Metadaten-Overlays, kein
   * Mehrfachauswahl-Raster (Klick wählt nur `selectedPhotoId`, wie im
   * Filmstreifen), stattdessen bei Hover/Auswahl ein Schnellentwicklung-
   * Overlay (`QuickDevelopOverlay`) statt der Bewertungs-/Flaggen-/
   * Farb-Widgets. Teilt sich Virtualisierung und Fotoliste 1:1 mit dem
   * normalen Raster statt einer komplett neuen Implementierung. */
  variant?: "grid" | "overview";
}

/**
 * Rasteransicht (`GridView.tsx`, Phase 3 Schritt 6, `DECISIONS.md`
 * ADR-0024): 2D-Kachel-Raster statt des Filmstreifens 1D-Reihe, aber
 * dieselbe Virtualisierungsbibliothek und dieselbe Fotoliste/Auswahl-Logik
 * aus dem Store (`selectActivePhotos`/`togglePhotoSelection`) — Raster und
 * Filmstreifen teilen sich denselben Selektions-Zustand, keine
 * Duplizierung. `@tanstack/react-virtual` virtualisiert nur Zeilen (kein
 * eingebautes 2D-Grid), jede Zeile rendert dafür `columns`-viele Zellen
 * aus der flachen Fotoliste — Standardmuster für Rasterlayouts mit dieser
 * Bibliothek.
 */
export function GridView({ variant = "grid" }: GridViewProps) {
  const isOverview = variant === "overview";
  const cellSize = isOverview ? OVERVIEW_CELL_SIZE : CELL_SIZE;
  const cellGap = isOverview ? OVERVIEW_CELL_GAP : CELL_GAP;

  // `selectActivePhotos` liefert bei leerer Auswahl jedes Mal ein neues
  // `[]`-Literal zurück — `useShallow` vergleicht Elemente statt der
  // Array-Referenz, sonst hält `useSyncExternalStore` das für eine sich
  // ständig ändernde Snapshot und rendert endlos neu.
  const photos = useAppStore(useShallow(selectActivePhotos));
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const multiSelectedIds = useAppStore((s) => s.multiSelectedIds);
  const togglePhotoSelection = useAppStore((s) => s.togglePhotoSelection);
  const selectPhoto = useAppStore((s) => s.selectPhoto);
  const setPhotoRating = useAppStore((s) => s.setPhotoRating);
  const setPhotoFlag = useAppStore((s) => s.setPhotoFlag);
  const setPhotoColorLabel = useAppStore((s) => s.setPhotoColorLabel);

  const [hoveredPhotoId, setHoveredPhotoId] = useState<string | null>(null);

  const containerRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [containerWidth, setContainerWidth] = useState(0);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (entry) setContainerWidth(entry.contentRect.width);
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const columns = Math.max(1, Math.floor((containerWidth + cellGap) / (cellSize + cellGap)));
  const rowCount = Math.ceil(photos.length / columns);

  const rowVirtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => cellSize + cellGap,
    overscan: 4,
  });

  if (photos.length === 0) {
    return (
      <main ref={containerRef} className="flex flex-1 items-center justify-center text-sm text-text-muted">
        Keine Fotos zum Anzeigen.
      </main>
    );
  }

  return (
    <main ref={containerRef} className="flex flex-1 overflow-hidden">
      <div ref={scrollRef} className="w-full overflow-y-auto p-2">
        <div style={{ height: rowVirtualizer.getTotalSize(), position: "relative" }}>
          {rowVirtualizer.getVirtualItems().map((row) => {
            const rowPhotos = photos.slice(row.index * columns, row.index * columns + columns);
            return (
              <div
                key={row.index}
                style={{
                  position: "absolute",
                  top: row.start,
                  left: 0,
                  width: "100%",
                  height: cellSize,
                  display: "flex",
                  gap: cellGap,
                }}
              >
                {rowPhotos.map((photo) => {
                  const isSelected = multiSelectedIds.includes(photo.id);
                  const isFocused = photo.id === selectedPhotoId;
                  // Übersichtsansicht: Overlay bei Hover *oder* Auswahl
                  // zeigen (siehe `GridViewProps`s Moduldoku), nicht bei
                  // beidem parallel für mehrere Kacheln — sonst müsste
                  // `QuickDevelopOverlay` für jede sichtbare Kachel den
                  // Bearbeitungsstand laden statt gezielt nur für eine.
                  const showQuickDevelop = isOverview && (photo.id === hoveredPhotoId || (hoveredPhotoId === null && isFocused));
                  return (
                    <div
                      key={photo.id}
                      // Kein <button> als Zellen-Container, weil die
                      // Bewertungs-/Flaggen-/Farb-Widgets (bzw. in der
                      // Übersichtsansicht die Schnellentwicklung-Regler)
                      // darunter selbst <button>-Elemente sind — HTML
                      // erlaubt kein interaktives Element (button)
                      // verschachtelt in einem anderen; role="button" +
                      // Tastatur-Handler hält die Zelle trotzdem
                      // vollwertig fokussierbar.
                      role="button"
                      tabIndex={0}
                      onClick={(event) => (isOverview ? selectPhoto(photo.id) : togglePhotoSelection(photo.id, resolveSelectionMode(event)))}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.preventDefault();
                          if (isOverview) {
                            selectPhoto(photo.id);
                          } else {
                            togglePhotoSelection(photo.id, resolveSelectionMode(event));
                          }
                        }
                      }}
                      onMouseEnter={() => isOverview && setHoveredPhotoId(photo.id)}
                      onMouseLeave={() => isOverview && setHoveredPhotoId((current) => (current === photo.id ? null : current))}
                      title={photo.missing ? `${photo.filename} (Datei fehlt)` : photo.filename}
                      style={{ width: cellSize, height: cellSize }}
                      className={`relative shrink-0 cursor-pointer overflow-hidden rounded border-2 text-left ${
                        isFocused ? "border-accent" : isSelected ? "border-accent/50" : "border-transparent hover:border-border"
                      } ${photo.missing ? "opacity-40" : ""}`}
                    >
                      <img
                        src={previewUrl(photo.id, 0)}
                        alt={photo.filename}
                        className="h-full w-full object-cover"
                        loading="lazy"
                      />
                      {photo.missing && (
                        <span className="absolute right-1 top-1 rounded bg-bg-base/80 px-1 text-[10px] leading-tight text-danger">
                          fehlt
                        </span>
                      )}
                      {isOverview ? (
                        <>
                          {/* Reduziertes Metadaten-Overlay (Sichtungsmodus,
                              siehe `GridViewProps`s Moduldoku): nur der
                              Dateiname, keine Bewertungs-/Flaggen-/Farb-
                              Widgets — die verdrängt bei Hover/Auswahl
                              ohnehin `QuickDevelopOverlay`. */}
                          {!showQuickDevelop && (
                            <div className="absolute inset-x-0 bottom-0 truncate bg-bg-base/80 px-1.5 py-1 text-xs text-text-secondary">
                              {photo.filename}
                            </div>
                          )}
                          {showQuickDevelop && <QuickDevelopOverlay photoId={photo.id} />}
                        </>
                      ) : (
                        <div className="absolute inset-x-0 bottom-0 flex items-center justify-between gap-1 bg-bg-base/85 px-1.5 py-1">
                          <RatingStars compact rating={photo.rating} onChange={(rating) => void setPhotoRating(photo.id, rating)} />
                          <FlagToggle compact flag={photo.flag} onChange={(flag) => void setPhotoFlag(photo.id, flag)} />
                          <ColorLabelPicker
                            compact
                            colorLabel={photo.color_label}
                            onChange={(color) => void setPhotoColorLabel(photo.id, color)}
                          />
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            );
          })}
        </div>
      </div>
    </main>
  );
}
