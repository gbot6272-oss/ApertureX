import { Layers, StickyNote } from "lucide-react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import { previewUrl } from "../lib/media";
import { groupPhotosByStack } from "../lib/stackGrouping";
import { playCue } from "../lib/sound";
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
  // Stapel im Raster (Phase 33 F10): aus der Fotoliste wird eine
  // Kachelliste, in der ein eingeklappter Stapel nur eine Kachel
  // belegt. Die Gruppierungsregeln stehen in `lib/stackGrouping.ts` und
  // sind dort getestet — hier nur die Anwendung.
  const stacks = useAppStore(useShallow((s) => s.stacks));
  const expandedStackIds = useAppStore(useShallow((s) => s.expandedStackIds));
  const toggleStackExpanded = useAppStore((s) => s.toggleStackExpanded);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const multiSelectedIds = useAppStore((s) => s.multiSelectedIds);
  const noteOpenCounts = useAppStore((s) => s.noteOpenCounts);
  const refreshNoteOpenCounts = useAppStore((s) => s.refreshNoteOpenCounts);
  const togglePhotoSelection = useAppStore((s) => s.togglePhotoSelection);
  const selectPhoto = useAppStore((s) => s.selectPhoto);
  const setPhotoRating = useAppStore((s) => s.setPhotoRating);
  const setPhotoFlag = useAppStore((s) => s.setPhotoFlag);
  const setPhotoColorLabel = useAppStore((s) => s.setPhotoColorLabel);

  const [hoveredPhotoId, setHoveredPhotoId] = useState<string | null>(null);
  // Manuelle Reihenfolge (Phase 33 F8): nur innerhalb einer Sammlung und
  // nur, wenn auch wirklich manuell sortiert wird. Sonst wäre das Ziehen
  // ein Versprechen, das die Ansicht im nächsten Moment bricht — die
  // Liste würde sofort wieder nach Dateiname umsortiert.
  const selectedCollectionId = useAppStore((s) => s.selectedCollectionId);
  const librarySortField = useAppStore((s) => s.librarySortField);
  const libraryResults = useAppStore((s) => s.libraryResults);
  const reorderCollectionPhoto = useAppStore((s) => s.reorderCollectionPhoto);
  const canReorder = librarySortField === "manual" && selectedCollectionId !== null && libraryResults === null;
  const [draggedPhotoId, setDraggedPhotoId] = useState<string | null>(null);
  const [dropTargetId, setDropTargetId] = useState<string | null>(null);

  // Klick-vs-Ziehen-Unterscheidung (Phase 20, siehe `DECISIONS.md`
  // ADR-0048): der Browser feuert nach einem `mousedown`/`mouseup`-Paar
  // ein `click`, selbst wenn der Zeiger dazwischen deutlich bewegt wurde
  // (z. B. beim Versuch, die Rasteransicht per Ziehen zu verschieben/zu
  // scrollen, oder beim — vom nativen `<img draggable>`-Verhalten
  // ausgelösten — Versuch, ein Foto zu verschieben). Ohne diese Wächter
  // wählte jede solche Zieh-Geste ungewollt ein Foto aus, genau der von
  // Nutzern gemeldete "automatisches Auswählen beim Foto-Verschieben"-
  // Fehler. `useRef` statt `useState`, weil der Wert nur innerhalb einer
  // einzigen Klick-Geste gebraucht wird und keinen Re-Render auslösen soll.
  const pointerDownPosRef = useRef<{ x: number; y: number } | null>(null);
  const CLICK_MOVE_THRESHOLD_PX = 6;

  const containerRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [containerWidth, setContainerWidth] = useState(0);

  // Offene Notizen einmal beim Öffnen des Rasters holen (Phase 32 F6) —
  // eine Abfrage für alle Kacheln statt einer je Kachel; jede Änderung
  // im Viewer frischt sie ohnehin selbst nach.
  useEffect(() => {
    void refreshNoteOpenCounts();
  }, [refreshNoteOpenCounts]);

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

  const entries = groupPhotosByStack(photos, stacks, expandedStackIds);
  const columns = Math.max(1, Math.floor((containerWidth + cellGap) / (cellSize + cellGap)));
  const rowCount = Math.ceil(entries.length / columns);

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
    <main ref={containerRef} data-testid="photo-grid" className="flex flex-1 overflow-hidden">
      <div ref={scrollRef} className="w-full overflow-y-auto p-2">
        <div style={{ height: rowVirtualizer.getTotalSize(), position: "relative" }}>
          {rowVirtualizer.getVirtualItems().map((row) => {
            const rowEntries = entries.slice(row.index * columns, row.index * columns + columns);
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
                {rowEntries.map((entry) => {
                  const photo = entry.photo;
                  const isSelected = multiSelectedIds.includes(photo.id);
                  const isFocused = photo.id === selectedPhotoId;
                  // Übersichtsansicht: das volle Sieben-Regler-Overlay nur
                  // bei echtem Hover (Phase 18 Schritt 5, siehe
                  // `DECISIONS.md` ADR-0046 — vorher deckte es das Foto
                  // schon bei reiner Fokussierung ohne Hover ab, was in der
                  // Nutzer-Rückmeldung als "verdeckt das Foto in der
                  // Übersicht standardmäßig" bemängelt wurde). Fokus ohne
                  // Hover zeigt stattdessen nur ein kleines Eck-Symbol
                  // (`showQuickDevelopHint` unten), das Foto bleibt sichtbar.
                  const showQuickDevelop = isOverview && photo.id === hoveredPhotoId;
                  const showQuickDevelopHint = isOverview && !showQuickDevelop && isFocused;
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
                      draggable={canReorder}
                      onDragStart={(event) => {
                        if (!canReorder) return;
                        setDraggedPhotoId(photo.id);
                        // Ohne gesetzte Daten startet in manchen Browsern
                        // gar kein Zieh-Vorgang.
                        event.dataTransfer.setData("text/plain", photo.id);
                        event.dataTransfer.effectAllowed = "move";
                      }}
                      onDragOver={(event) => {
                        if (!canReorder || draggedPhotoId === null) return;
                        // Ohne `preventDefault` lehnt der Browser das
                        // Ablegen ab — dasselbe wie im Sammlungs-Board.
                        event.preventDefault();
                        event.dataTransfer.dropEffect = "move";
                        setDropTargetId(photo.id);
                      }}
                      onDragLeave={() => {
                        setDropTargetId((current) => (current === photo.id ? null : current));
                      }}
                      onDrop={(event) => {
                        if (!canReorder || draggedPhotoId === null) return;
                        event.preventDefault();
                        const targetIndex = photos.findIndex((candidate) => candidate.id === photo.id);
                        const moved = draggedPhotoId;
                        setDraggedPhotoId(null);
                        setDropTargetId(null);
                        if (targetIndex < 0 || moved === photo.id) return;
                        playCue("drop");
                        void reorderCollectionPhoto(moved, targetIndex);
                      }}
                      onDragEnd={() => {
                        setDraggedPhotoId(null);
                        setDropTargetId(null);
                      }}
                      onMouseDown={(event) => {
                        pointerDownPosRef.current = { x: event.clientX, y: event.clientY };
                      }}
                      onClick={(event) => {
                        // Nur auswählen, wenn sich der Zeiger seit dem
                        // `mousedown` kaum bewegt hat — siehe Kommentar bei
                        // `pointerDownPosRef` oben. Kein gespeicherter
                        // Startpunkt (z. B. Klick per Touch/Assistiv-
                        // Technologie ohne vorheriges `mousedown`) zählt
                        // weiterhin als normaler Klick.
                        const start = pointerDownPosRef.current;
                        pointerDownPosRef.current = null;
                        if (start) {
                          const dx = event.clientX - start.x;
                          const dy = event.clientY - start.y;
                          if (Math.hypot(dx, dy) > CLICK_MOVE_THRESHOLD_PX) return;
                        }
                        playCue("select");
                        if (isOverview) selectPhoto(photo.id);
                        else togglePhotoSelection(photo.id, resolveSelectionMode(event));
                      }}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.preventDefault();
                          playCue("select");
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
                      className={`apx-grid-cell-in relative shrink-0 cursor-pointer overflow-hidden rounded border-2 text-left transition-transform duration-[var(--duration-fast)] hover:z-10 hover:scale-[1.04] hover:shadow-lg ${
                        isFocused ? "border-accent" : isSelected ? "border-accent/50" : "border-transparent hover:border-border"
                      } ${photo.missing ? "opacity-40" : ""} ${
                        dropTargetId === photo.id ? "ring-2 ring-accent" : ""
                      } ${draggedPhotoId === photo.id ? "opacity-50" : ""}`}
                    >
                      <img
                        src={previewUrl(photo.id, 0)}
                        alt={photo.filename}
                        className="h-full w-full object-cover"
                        loading="lazy"
                        // `<img>` ist im Browser standardmäßig ziehbar —
                        // ohne dies löst ein Zieh-Versuch auf der Kachel
                        // (z. B. beim Verschieben-Versuch oder beim Scrollen
                        // per Ziehen) einen nativen Bild-Drag statt eines
                        // einfachen Klicks aus (siehe `pointerDownPosRef`-
                        // Kommentar oben, `DECISIONS.md` ADR-0048).
                        draggable={false}
                      />
                      {photo.missing && (
                        <span className="absolute right-1 top-1 rounded bg-bg-base/80 px-1 text-[10px] leading-tight text-danger">
                          fehlt
                        </span>
                      )}
                      {entry.stackId !== undefined && (
                        // Das Abzeichen ist selbst der Schalter: ein
                        // eigener Aufklapp-Knopf daneben hätte auf einer
                        // 168px-Kachel keinen Platz, und ein Klick auf
                        // die Kachel selbst muss weiterhin auswählen.
                        <button
                          type="button"
                          data-testid="grid-stack-badge"
                          aria-label={
                            expandedStackIds.includes(entry.stackId)
                              ? `Stapel mit ${entry.stackSize} Fotos einklappen`
                              : `Stapel mit ${entry.stackSize} Fotos aufklappen`
                          }
                          onClick={(event) => {
                            event.stopPropagation();
                            playCue("select");
                            toggleStackExpanded(entry.stackId!);
                          }}
                          className="absolute right-1 top-1 flex items-center gap-0.5 rounded bg-bg-base/85 px-1 text-[10px] leading-tight text-text-primary hover:text-accent"
                        >
                          <Layers aria-hidden="true" className="size-2.5" />
                          {entry.stackSize}
                        </button>
                      )}
                      {(noteOpenCounts[photo.id] ?? 0) > 0 && (
                        // Offene Notizen sichtbar machen, ohne die Kachel
                        // zuzubauen (Phase 32 F6): ein kleines Zeichen mit
                        // Anzahl, oben links, wo sonst nichts liegt.
                        <span
                          data-testid="grid-note-badge"
                          title={`${noteOpenCounts[photo.id]} offene Notiz(en)`}
                          className="absolute left-1 top-1 flex items-center gap-0.5 rounded bg-bg-base/80 px-1 text-[10px] leading-tight text-accent"
                        >
                          <StickyNote aria-hidden="true" className="size-2.5" />
                          {noteOpenCounts[photo.id]}
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
                          {showQuickDevelopHint && (
                            <button
                              type="button"
                              onClick={(event) => {
                                event.stopPropagation();
                                setHoveredPhotoId(photo.id);
                              }}
                              title="Schnellentwicklung anzeigen"
                              aria-label="Schnellentwicklung anzeigen"
                              className="absolute right-1 top-1 flex h-5 w-5 items-center justify-center rounded bg-bg-base/85 text-xs text-text-secondary shadow transition-colors duration-[var(--duration-fast)] hover:text-accent"
                            >
                              {/* Einfaches Unicode-Zeichen statt Emoji (wie
                                  die übrigen Icon-Knöpfe dieser Codebasis,
                                  z. B. ✎/↶/↷/‹/›) — rendert zuverlässig ohne
                                  Emoji-Schriftart. */}
                              ≡
                            </button>
                          )}
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
