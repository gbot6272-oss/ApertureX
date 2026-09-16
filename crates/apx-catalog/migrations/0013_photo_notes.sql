-- Migration 13: Notizen am Foto (Phase 32 F6). Eine eigene Tabelle statt
-- einer Spalte in `photos`, weil ein Foto beliebig viele Notizen tragen
-- kann und jede an einer eigenen Stelle im Bild hängt.
--
-- **Koordinaten sind normiert (0..1) und beziehen sich auf das
-- UNBESCHNITTENE Original.** Das ist bewusst anders als bei
-- `face_detections` (Migration 11), die Pixelkoordinaten einer bestimmten
-- Vorschaustufe speichert: eine Gesichtserkennung läuft einmal auf genau
-- dieser Stufe, eine Notiz dagegen wird in jeder Zoomstufe und neben
-- jedem Bearbeitungsstand angezeigt. Normiert bleibt sie überall gültig;
-- und aufs Original bezogen bleibt sie es auch, wenn der Beschnitt später
-- geändert wird — die Notiz meint eine Stelle im Motiv, nicht im
-- aktuellen Ausschnitt.
--
-- `done` macht aus einer Notiz eine abhakbare Aufgabe ("Staubfleck hier
-- weg") — der häufigste Grund, sich etwas an ein Foto zu schreiben.
CREATE TABLE photo_notes (
    id          TEXT PRIMARY KEY,
    photo_id    TEXT NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
    x           REAL NOT NULL,
    y           REAL NOT NULL,
    body        TEXT NOT NULL,
    done        INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE INDEX idx_photo_notes_photo_id ON photo_notes(photo_id);
