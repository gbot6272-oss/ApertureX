-- Migration 14: Papierkorb (Phase 33 F1).
--
-- Fotos werden nicht mehr hart gelöscht, sondern bekommen einen
-- Zeitstempel in `deleted_at`. Eine eigene `trash`-Tabelle mit einer
-- Kopie der Zeile wäre die Alternative gewesen; dagegen sprach, dass an
-- einem Foto ein Dutzend Tabellen per `ON DELETE CASCADE` hängen
-- (Bearbeitungen, Schnappschüsse, Stichwörter, Notizen, Sammlungen,
-- Stapel, Gesichter). Ein echtes DELETE würde die alle mitreißen, und
-- ein „Wiederherstellen" könnte nur die Foto-Zeile zurückbringen, nicht
-- die daran hängende Arbeit. Ein Datum in der Zeile lässt alles stehen.
--
-- Preis dieser Entscheidung: jede Abfrage, die den Katalog auflistet,
-- muss `deleted_at IS NULL` mitführen. Damit das nicht vergessen wird,
-- gibt es dafür genau eine Konstante (`repository::photos::NOT_TRASHED`),
-- und `repository::photos::get` liefert bewusst auch Fotos im
-- Papierkorb — sonst könnte die Papierkorb-Ansicht sie nicht anzeigen.
--
-- `deleted_reason` merkt sich, warum etwas im Papierkorb liegt (von Hand
-- weggeworfen, als Duplikat aussortiert, als unscharf aussortiert). Die
-- Papierkorb-Ansicht gruppiert danach, und beim Leeren kann man eine
-- Gruppe gezielt behalten.
ALTER TABLE photos ADD COLUMN deleted_at INTEGER;
ALTER TABLE photos ADD COLUMN deleted_reason TEXT;

CREATE INDEX idx_photos_deleted_at ON photos(deleted_at);
