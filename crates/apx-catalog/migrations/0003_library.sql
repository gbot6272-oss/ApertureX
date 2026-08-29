-- Migration 3: Bibliothek für Phase 3 (siehe DECISIONS.md ADR-0022 bis
-- ADR-0025). Additiv zu den Migrationen 1/2 — photos/folders/previews/
-- edit_history/edit_current bleiben unverändert, bis auf die drei neuen
-- Spalten unten (ALTER TABLE ADD COLUMN ist additiv: bestehende Zeilen
-- bekommen den jeweiligen Default, keine Daten gehen verloren).

-- Bewertung/Flagge/Farbmarkierung: einfache Skalarwerte pro Foto,
-- konsistent mit dem bestehenden `missing`-Spalten-Muster (ADR-0023).
ALTER TABLE photos ADD COLUMN rating INTEGER NOT NULL DEFAULT 0;
ALTER TABLE photos ADD COLUMN flag INTEGER NOT NULL DEFAULT 0;
ALTER TABLE photos ADD COLUMN color_label TEXT;

-- Schlagworte: flache Liste, keine Hierarchie/Synonyme (Phase 6, siehe
-- ADR-0022).
CREATE TABLE keywords (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL UNIQUE
);

CREATE TABLE photo_keywords (
    photo_id      TEXT NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
    keyword_id    TEXT NOT NULL REFERENCES keywords(id) ON DELETE CASCADE,
    PRIMARY KEY (photo_id, keyword_id)
);

CREATE INDEX idx_photo_keywords_keyword ON photo_keywords(keyword_id);

-- Sammlungen: rein manuell, feste Reihenfolge über `position` (keine
-- Sammlungssätze/intelligenten Sammlungen — Phase 6, siehe ADR-0023).
CREATE TABLE collections (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    created_at    INTEGER NOT NULL
);

CREATE TABLE collection_photos (
    collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    photo_id      TEXT NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
    position      INTEGER NOT NULL,
    PRIMARY KEY (collection_id, photo_id)
);

CREATE INDEX idx_collection_photos_photo ON collection_photos(photo_id);

-- Volltextsuche: FTS5-External-Content-Virtualtabelle über `photos`
-- (referenziert die Originalspalten statt sie zu duplizieren, siehe
-- ADR-0023). `photos` hat keinen expliziten INTEGER-Primärschlüssel
-- (`id` ist TEXT) und damit einen normalen impliziten `rowid`, den
-- `content_rowid` referenziert.
CREATE VIRTUAL TABLE photos_fts USING fts5(
    filename, camera_make, camera_model, lens,
    content='photos', content_rowid='rowid'
);

-- Backfill für bereits vorhandene Fotos (bei einem Katalog, der schon
-- Migration 1/2 hatte, bevor Migration 3 angewendet wird) — bei einem
-- brandneuen Katalog ist `photos` hier noch leer, der SELECT liefert
-- dann einfach keine Zeilen.
INSERT INTO photos_fts(rowid, filename, camera_make, camera_model, lens)
SELECT rowid, filename, camera_make, camera_model, lens FROM photos;

-- Hält den FTS5-Index synchron mit `photos` — Standardmuster für
-- External-Content-Tabellen (siehe SQLite-Dokumentation zu FTS5).
CREATE TRIGGER photos_fts_after_insert AFTER INSERT ON photos BEGIN
    INSERT INTO photos_fts(rowid, filename, camera_make, camera_model, lens)
    VALUES (new.rowid, new.filename, new.camera_make, new.camera_model, new.lens);
END;

CREATE TRIGGER photos_fts_after_delete AFTER DELETE ON photos BEGIN
    INSERT INTO photos_fts(photos_fts, rowid, filename, camera_make, camera_model, lens)
    VALUES ('delete', old.rowid, old.filename, old.camera_make, old.camera_model, old.lens);
END;

CREATE TRIGGER photos_fts_after_update AFTER UPDATE ON photos BEGIN
    INSERT INTO photos_fts(photos_fts, rowid, filename, camera_make, camera_model, lens)
    VALUES ('delete', old.rowid, old.filename, old.camera_make, old.camera_model, old.lens);
    INSERT INTO photos_fts(rowid, filename, camera_make, camera_model, lens)
    VALUES (new.rowid, new.filename, new.camera_make, new.camera_model, new.lens);
END;
