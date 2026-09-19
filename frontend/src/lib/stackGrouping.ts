/**
 * Stapel im Raster ein- und ausklappen (Phase 33 F10).
 *
 * **Warum es das braucht.** Stapel gibt es im Katalog seit Phase 9
 * Schritt 1, und die Serien-Erkennung aus Phase 32 F7 legt sie mit einem
 * Klick an — im Raster sah man davon aber nichts. Zwanzig Aufnahmen
 * einer Reihenaufnahme blieben zwanzig Kacheln, und genau das soll ein
 * Stapel ja verhindern.
 *
 * **Die Entscheidungen, die das Gruppieren richtig oder falsch machen:**
 *
 * 1. **Ein eingeklappter Stapel steht an der Stelle seines ersten
 *    sichtbaren Mitglieds.** Sonst würde das Einklappen das ganze Raster
 *    umsortieren, und man fände die Stelle nicht wieder, an der man
 *    gerade war.
 * 2. **Gezählt wird, was sichtbar ist.** Hat ein Filter die Hälfte des
 *    Stapels entfernt, zeigt das Abzeichen die verbliebene Hälfte. Die
 *    volle Zahl wäre ein Versprechen auf Fotos, die der Filter gerade
 *    ausgeschlossen hat — und ein Aufklappen könnte es nicht einlösen.
 * 3. **Das Deckblatt muss in der Liste liegen.** Ist das im Katalog
 *    hinterlegte Deckblatt gerade weggefiltert, übernimmt das erste
 *    sichtbare Mitglied. Ein Deckblatt zu zeigen, das im aufgeklappten
 *    Zustand gar nicht da ist, wäre schlimmer als ein anderes Bild.
 * 4. **Ein Foto in zwei Stapeln gehört zum ersten.** Der Katalog
 *    verbietet das nicht; es hier stillschweigend doppelt anzuzeigen
 *    wäre die schlechtere Antwort als eine feste, nachvollziehbare Regel.
 */

import type { PhotoDto, StackDto } from "./tauri";

export interface GridEntry {
  photo: PhotoDto;
  /** Gesetzt, wenn diese Kachel einen eingeklappten Stapel vertritt. */
  stackId?: string;
  /** Wie viele sichtbare Fotos der Stapel enthält (inklusive dieses). */
  stackSize?: number;
}

/**
 * Baut die anzuzeigende Kachelliste aus Fotos, Stapeln und dem Satz der
 * gerade aufgeklappten Stapel.
 */
export function groupPhotosByStack(
  photos: PhotoDto[],
  stacks: StackDto[],
  expandedStackIds: readonly string[],
): GridEntry[] {
  if (stacks.length === 0) return photos.map((photo) => ({ photo }));

  const expanded = new Set(expandedStackIds);
  // Foto-ID -> Stapel. Der erste Stapel gewinnt (Entscheidung 4).
  const stackOf = new Map<string, StackDto>();
  for (const stack of stacks) {
    for (const photoId of stack.photo_ids) {
      if (!stackOf.has(photoId)) stackOf.set(photoId, stack);
    }
  }

  // Sichtbare Mitglieder je Stapel, in der Reihenfolge der Liste
  // (Entscheidung 2).
  const visibleMembers = new Map<string, PhotoDto[]>();
  for (const photo of photos) {
    const stack = stackOf.get(photo.id);
    if (!stack) continue;
    const list = visibleMembers.get(stack.id) ?? [];
    list.push(photo);
    visibleMembers.set(stack.id, list);
  }

  const emitted = new Set<string>();
  const entries: GridEntry[] = [];
  for (const photo of photos) {
    const stack = stackOf.get(photo.id);
    if (!stack || expanded.has(stack.id)) {
      entries.push({ photo });
      continue;
    }
    if (emitted.has(stack.id)) continue;
    emitted.add(stack.id);

    const members = visibleMembers.get(stack.id) ?? [photo];
    // Ein Stapel mit nur einem sichtbaren Foto ist kein Stapel — ein
    // Abzeichen mit „1" wäre eine Kachel, die sich aufklappen lässt und
    // danach genauso aussieht.
    if (members.length < 2) {
      entries.push({ photo });
      continue;
    }
    const cover =
      members.find((member) => member.id === stack.cover_photo_id) ?? members[0] ?? photo;
    entries.push({ photo: cover, stackId: stack.id, stackSize: members.length });
  }
  return entries;
}

/** Die IDs aller Stapel, die in `photos` mit mindestens zwei sichtbaren
 * Fotos vertreten sind — Grundlage für „alle auf-/zuklappen". */
export function stacksInView(photos: PhotoDto[], stacks: StackDto[]): string[] {
  const visible = new Set(photos.map((photo) => photo.id));
  return stacks
    .filter((stack) => stack.photo_ids.filter((id) => visible.has(id)).length >= 2)
    .map((stack) => stack.id);
}
