import { Check, MapPin, Plus, Trash2, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { originalToView, pointFromEvent, shortLabel, viewToOriginal, type CropWindow } from "../lib/notePins";
import { playCue } from "../lib/sound";
import { useAppStore } from "../store";

/**
 * Notiz-Pins über dem Bild (Phase 32 F6).
 *
 * Wer ein Foto sichtet, merkt sich Dinge, die später zu tun sind —
 * „Staubfleck oben links", „Horizont schief", „diesen Ast wegstempeln".
 * Bisher gab es dafür nur das Bildtitel-/Beschreibungsfeld, also Text
 * ohne Ort. Eine Notiz mit Ort zeigt beim nächsten Öffnen genau dorthin.
 *
 * **Der Setzen-Modus ist ein Schalter, kein Dauerzustand.** Ohne ihn
 * wäre jeder Klick ins Bild eine neue Notiz — beim Zoomen, Ziehen und
 * Pipettieren ständig aus Versehen. Mit Schalter ist das Setzen eine
 * bewusste Handlung, danach schaltet er sich selbst wieder ab.
 *
 * **Pins außerhalb des Ausschnitts werden nicht ans Bild geklebt.**
 * Sie hängen am unbeschnittenen Original (siehe `lib/notePins.ts`);
 * liegt eine Notiz außerhalb des aktuellen Zuschnitts, erscheint sie
 * in der Liste mit Hinweis statt an einer falschen Stelle im Bild.
 */

interface NotesOverlayProps {
  /** Rechteck des angezeigten Bildes im Viewer, in Pixeln. */
  rect: { left: number; top: number; width: number; height: number };
  /** Aktueller Zuschnitt aus dem EDL. */
  crop: CropWindow;
}

export function NotesOverlay({ rect, crop }: NotesOverlayProps) {
  const notes = useAppStore((s) => s.photoNotes);
  const notesMode = useAppStore((s) => s.notesMode);
  const toggleNotesMode = useAppStore((s) => s.toggleNotesMode);
  const addPhotoNote = useAppStore((s) => s.addPhotoNote);
  const editPhotoNote = useAppStore((s) => s.editPhotoNote);
  const removePhotoNote = useAppStore((s) => s.removePhotoNote);

  const [openNoteId, setOpenNoteId] = useState<string | null>(null);
  const [draft, setDraft] = useState<{ x: number; y: number; body: string } | null>(null);
  const layerRef = useRef<HTMLDivElement | null>(null);
  const draftInputRef = useRef<HTMLTextAreaElement | null>(null);

  useEffect(() => {
    if (draft) draftInputRef.current?.focus();
  }, [draft]);

  function placeDraft(event: React.MouseEvent<HTMLDivElement>) {
    if (!notesMode) return;
    const layer = layerRef.current;
    if (!layer) return;
    const view = pointFromEvent(layer.getBoundingClientRect(), event.clientX, event.clientY);
    const original = viewToOriginal(view, crop);
    setDraft({ ...original, body: "" });
    setOpenNoteId(null);
  }

  async function saveDraft() {
    if (!draft || !draft.body.trim()) return;
    await addPhotoNote(draft.x, draft.y, draft.body);
    playCue("select");
    setDraft(null);
    // Der Modus schaltet sich nach dem Setzen ab — eine Notiz ist meist
    // eine einzelne Handlung, keine Serie.
    if (notesMode) toggleNotesMode();
  }

  const placed = notes
    .map((note) => ({ note, view: originalToView({ x: note.x, y: note.y }, crop) }))
    .filter((entry): entry is { note: (typeof notes)[number]; view: { x: number; y: number } } => entry.view !== null);
  const hidden = notes.length - placed.length;

  const draftView = draft ? originalToView({ x: draft.x, y: draft.y }, crop) : null;

  return (
    <div
      data-testid="notes-overlay"
      // Der Rahmen selbst fängt nichts ab: er liegt über dem ganzen Bild
      // und würde sonst Zoom, Pipette und sogar die Werkzeugleiste
      // darunter blockieren. Nur die Klickfläche im Setzen-Modus und die
      // Pins/Popover nehmen Zeigerereignisse an.
      className="pointer-events-none absolute"
      style={{ left: rect.left, top: rect.top, width: rect.width, height: rect.height }}
    >
      {/* Klickfläche nur im Setzen-Modus — sonst blockierte sie Zoom und
          Pipette. */}
      <div
        ref={layerRef}
        onClick={placeDraft}
        aria-hidden={!notesMode}
        className={`absolute inset-0 ${notesMode ? "pointer-events-auto cursor-crosshair" : "pointer-events-none"}`}
      />

      {placed.map(({ note, view }) => (
        <div key={note.id} className="pointer-events-auto absolute" style={{ left: `${view.x * 100}%`, top: `${view.y * 100}%` }}>
          <button
            type="button"
            onClick={() => setOpenNoteId((current) => (current === note.id ? null : note.id))}
            aria-label={`Notiz: ${shortLabel(note.body)}${note.done ? " (erledigt)" : ""}`}
            aria-expanded={openNoteId === note.id}
            data-testid="note-pin"
            data-done={note.done}
            className={`-translate-x-1/2 -translate-y-full rounded-full border p-1 shadow-lg transition-colors duration-[var(--duration-fast)] ${
              note.done
                ? "border-success/60 bg-bg-panel/90 text-success"
                : "border-accent bg-accent text-bg-base"
            }`}
          >
            {note.done ? <Check aria-hidden="true" className="size-3" /> : <MapPin aria-hidden="true" className="size-3" />}
          </button>

          {openNoteId === note.id && (
            <div
              role="dialog"
              aria-label="Notiz bearbeiten"
              data-testid="note-popover"
              className="apx-notice-in absolute left-1/2 z-10 w-60 -translate-x-1/2 translate-y-1 rounded border border-border bg-bg-panel p-2 shadow-xl"
            >
              <textarea
                value={note.body}
                onChange={(event) => void editPhotoNote(note.id, { body: event.target.value })}
                aria-label="Notiztext"
                rows={3}
                className="w-full resize-none rounded border border-border bg-bg-base px-1.5 py-1 text-xs text-text-primary"
              />
              <div className="mt-1 flex items-center gap-2">
                <label className="flex items-center gap-1 text-[11px] text-text-secondary">
                  <input
                    type="checkbox"
                    checked={note.done}
                    onChange={(event) => void editPhotoNote(note.id, { done: event.target.checked })}
                    className="accent-accent"
                  />
                  Erledigt
                </label>
                <span className="flex-1" />
                <button
                  type="button"
                  onClick={() => void removePhotoNote(note.id)}
                  aria-label="Notiz löschen"
                  className="apx-btn-liquid rounded border border-border p-1 text-text-secondary hover:border-danger hover:text-danger"
                >
                  <Trash2 aria-hidden="true" className="size-3" />
                </button>
                <button
                  type="button"
                  onClick={() => setOpenNoteId(null)}
                  aria-label="Notiz schließen"
                  className="apx-btn-liquid rounded border border-border p-1 text-text-secondary"
                >
                  <X aria-hidden="true" className="size-3" />
                </button>
              </div>
            </div>
          )}
        </div>
      ))}

      {draft && draftView && (
        <div className="pointer-events-auto absolute" style={{ left: `${draftView.x * 100}%`, top: `${draftView.y * 100}%` }}>
          <span className="block -translate-x-1/2 -translate-y-full rounded-full border border-accent bg-bg-panel p-1 text-accent shadow-lg">
            <Plus aria-hidden="true" className="size-3" />
          </span>
          <div
            role="dialog"
            aria-label="Neue Notiz"
            data-testid="note-draft"
            className="apx-notice-in absolute left-1/2 w-60 -translate-x-1/2 translate-y-1 rounded border border-accent bg-bg-panel p-2 shadow-xl"
          >
            <textarea
              ref={draftInputRef}
              value={draft.body}
              onChange={(event) => setDraft({ ...draft, body: event.target.value })}
              aria-label="Text der neuen Notiz"
              rows={3}
              placeholder="Was ist hier zu tun?"
              className="w-full resize-none rounded border border-border bg-bg-base px-1.5 py-1 text-xs text-text-primary"
            />
            <div className="mt-1 flex items-center gap-2">
              <button
                type="button"
                onClick={() => setDraft(null)}
                className="apx-btn-liquid rounded border border-border px-2 py-0.5 text-[11px] text-text-secondary"
              >
                Abbrechen
              </button>
              <span className="flex-1" />
              <button
                type="button"
                onClick={() => void saveDraft()}
                disabled={!draft.body.trim()}
                className="apx-btn-liquid rounded border border-accent bg-accent/10 px-2 py-0.5 text-[11px] text-accent disabled:opacity-40"
              >
                Notiz anlegen
              </button>
            </div>
          </div>
        </div>
      )}

      {hidden > 0 && (
        <p
          data-testid="notes-hidden-hint"
          className="pointer-events-none absolute bottom-1 left-1 rounded bg-bg-panel/90 px-1.5 py-0.5 text-[10px] text-text-muted"
        >
          {hidden} {hidden === 1 ? "Notiz liegt" : "Notizen liegen"} außerhalb des Ausschnitts
        </p>
      )}
    </div>
  );
}
