-- Migration 11: Echte Personen-Wiedererkennung (Phase 13 Schritt 8, siehe
-- PLAN.md/DECISIONS.md ADR-0040-Nachtrag VI). Ersetzt nicht die grobe
-- Hautton-Heuristik-Gruppierung aus Phase 11 Schritt 5 (die bleibt als
-- Fallback ohne konfigurierte Modelle bestehen) — zwei neue Tabellen statt
-- einer Erweiterung von `photos`, weil ein Foto mehrere Gesichter (also
-- mehrere Personen) enthalten kann.

-- Eine vom Nutzer benannte Person. `name IS NULL` = automatisch erkannt,
-- aber noch nicht benannt (dieselbe "erkannt, aber unbenannt"-UX wie
-- Adobe Lightroom Classics Personenansicht).
CREATE TABLE people (
    id              TEXT PRIMARY KEY,
    name            TEXT,
    -- Zeigt auf das Gesicht, das als Vorschaubild dient (meist das erste
    -- zugeordnete) — `ON DELETE SET NULL` statt CASCADE: eine Person
    -- ohne Titelbild bleibt bestehen, zeigt nur kein Bild mehr.
    cover_face_id   TEXT REFERENCES face_detections(id) ON DELETE SET NULL,
    created_at      INTEGER NOT NULL
);

-- Ein einzelnes erkanntes Gesicht in einem Foto — Bounding-Box in
-- Vorschaubild-Pixelkoordinaten (derselben Vorschaustufe, mit der
-- erkannt wurde, siehe `apx_ai::people`s Moduldoku) plus dessen
-- 128-dimensionales Embedding als JSON-Array (kein `BLOB`: bewusst
-- lesbar/inspizierbar wie `conditions_json` an anderer Stelle in diesem
-- Katalog, die Größe — rund 2 KB je Gesicht als Text — ist hier
-- unerheblich). `person_id IS NULL` = noch keiner Person zugeordnet
-- (weder automatisch geclustert noch manuell benannt).
CREATE TABLE face_detections (
    id              TEXT PRIMARY KEY,
    photo_id        TEXT NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
    person_id       TEXT REFERENCES people(id) ON DELETE SET NULL,
    rect_left       INTEGER NOT NULL,
    rect_top        INTEGER NOT NULL,
    rect_right      INTEGER NOT NULL,
    rect_bottom     INTEGER NOT NULL,
    embedding_json  TEXT NOT NULL,
    created_at      INTEGER NOT NULL
);

CREATE INDEX idx_face_detections_photo_id ON face_detections(photo_id);
CREATE INDEX idx_face_detections_person_id ON face_detections(person_id);
