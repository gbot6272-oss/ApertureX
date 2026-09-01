-- Migration 7: Bibliotheks-Backlog für Phase 9 Schritt 1 (siehe
-- DECISIONS.md ADR-0032/ADR-0035). Additiv zu den Migrationen 1-6, mit
-- einer Ausnahme: `photos` muss neu aufgebaut werden, weil SQLite eine
-- inline `UNIQUE`-Constraint (Migration 1) nicht per `ALTER TABLE`
-- entfernen kann — virtuelle Kopien brauchen eine eigene `photos`-Zeile
-- (damit sie an edit_history/keywords/collections/snapshots/rating wie
-- jedes andere Foto teilnehmen, ohne diese Subsysteme zu verdoppeln),
-- aber dieselbe (folder_id, filename) wie ihr Quellfoto — die alte
-- `UNIQUE (folder_id, filename)` gilt jetzt nur noch für "echte"
-- Foto-Zeilen (`source_photo_id IS NULL`) über eine partielle
-- Unique-Index statt der Tabellen-Constraint.

PRAGMA foreign_keys = OFF;

CREATE TABLE photos_new (
    id            TEXT PRIMARY KEY,
    folder_id     TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    filename      TEXT NOT NULL,
    file_size     INTEGER NOT NULL,
    file_mtime    INTEGER NOT NULL,
    content_hash  TEXT,
    width         INTEGER, height INTEGER,
    orientation   INTEGER NOT NULL DEFAULT 1,
    camera_make   TEXT, camera_model TEXT, lens TEXT,
    iso           INTEGER, shutter REAL, aperture REAL, focal_length REAL,
    captured_at   INTEGER,
    gps_lat       REAL, gps_lon REAL,
    imported_at   INTEGER NOT NULL,
    missing       INTEGER NOT NULL DEFAULT 0,
    rating        INTEGER NOT NULL DEFAULT 0,
    flag          INTEGER NOT NULL DEFAULT 0,
    color_label   TEXT,
    -- NULL = echtes Foto mit eigener Datei. Gesetzt = virtuelle Kopie,
    -- teilt sich Datei/Pfad mit dem referenzierten Quellfoto, hat aber
    -- eigene rating/flag/color_label/edit_history/keywords/collections.
    source_photo_id TEXT REFERENCES photos(id) ON DELETE CASCADE
);

INSERT INTO photos_new (
    id, folder_id, filename, file_size, file_mtime, content_hash,
    width, height, orientation, camera_make, camera_model, lens,
    iso, shutter, aperture, focal_length, captured_at, gps_lat, gps_lon,
    imported_at, missing, rating, flag, color_label, source_photo_id
)
SELECT
    id, folder_id, filename, file_size, file_mtime, content_hash,
    width, height, orientation, camera_make, camera_model, lens,
    iso, shutter, aperture, focal_length, captured_at, gps_lat, gps_lon,
    imported_at, missing, rating, flag, color_label, NULL
FROM photos;

DROP TABLE photos;
ALTER TABLE photos_new RENAME TO photos;

CREATE INDEX idx_photos_folder   ON photos(folder_id);
CREATE INDEX idx_photos_captured ON photos(captured_at);
CREATE INDEX idx_photos_hash     ON photos(content_hash);
CREATE INDEX idx_photos_source   ON photos(source_photo_id);
CREATE UNIQUE INDEX idx_photos_unique_master ON photos(folder_id, filename)
    WHERE source_photo_id IS NULL;

-- FTS5-External-Content-Tabelle/Trigger müssen neu angelegt werden — die
-- Rowids der neuen `photos`-Tabelle stimmen nicht mit denen der alten
-- überein (SQLite vergibt sie beim `INSERT ... SELECT` neu). Die drei
-- Trigger (`ON photos`) hat `DROP TABLE photos` oben bereits automatisch
-- mitgelöscht — SQLite entfernt Trigger einer Tabelle beim Löschen der
-- Tabelle selbst, ein explizites `DROP TRIGGER` hier würde mit
-- "no such trigger" fehlschlagen.
DROP TABLE photos_fts;

CREATE VIRTUAL TABLE photos_fts USING fts5(
    filename, camera_make, camera_model, lens,
    content='photos', content_rowid='rowid'
);

INSERT INTO photos_fts(rowid, filename, camera_make, camera_model, lens)
SELECT rowid, filename, camera_make, camera_model, lens FROM photos;

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

PRAGMA foreign_keys = ON;

-- Stapel (Stacks): manuelle oder automatische (Aufnahmezeit-Fenster)
-- Gruppierung mehrerer Fotos, ein optionales Titelbild.
CREATE TABLE stacks (
    id              TEXT PRIMARY KEY,
    name            TEXT,
    cover_photo_id  TEXT REFERENCES photos(id) ON DELETE SET NULL,
    created_at      INTEGER NOT NULL
);

CREATE TABLE stack_photos (
    stack_id  TEXT NOT NULL REFERENCES stacks(id) ON DELETE CASCADE,
    photo_id  TEXT NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
    position  INTEGER NOT NULL,
    PRIMARY KEY (stack_id, photo_id)
);

CREATE INDEX idx_stack_photos_photo ON stack_photos(photo_id);

-- Sammlungssätze: verschachtelte Ordnerhierarchie für `collections`,
-- exakt dasselbe Muster wie `preset_folders` (Migration 4).
CREATE TABLE collection_folders (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    parent_id  TEXT REFERENCES collection_folders(id) ON DELETE CASCADE,
    position   INTEGER NOT NULL
);

ALTER TABLE collections ADD COLUMN folder_id TEXT REFERENCES collection_folders(id) ON DELETE SET NULL;
-- Intelligente Sammlungen: `smart_criteria_json` ist ein serialisiertes
-- `FilterCriteria` (siehe apx-catalog::models) — **bewusste
-- Vereinfachung**: flache UND-Verknüpfung der bestehenden Kriterien
-- statt verschachtelter UND/ODER-Regeln, wiederverwendet exakt die
-- Filterlogik aus Phase 3 (`repository::search::filter_photos`) statt
-- eine zweite Regel-Engine zu bauen.
ALTER TABLE collections ADD COLUMN is_smart INTEGER NOT NULL DEFAULT 0;
ALTER TABLE collections ADD COLUMN smart_criteria_json TEXT;

CREATE INDEX idx_collections_folder ON collections(folder_id);

-- Erweiterbare Farbmarkierungen: ersetzt die bisher im Rust-Code fest
-- verdrahtete `ALLOWED_COLOR_LABELS`-Konstante durch eine Tabelle.
CREATE TABLE color_label_definitions (
    name          TEXT PRIMARY KEY,
    display_name  TEXT NOT NULL,
    hex           TEXT NOT NULL,
    position      INTEGER NOT NULL
);

INSERT INTO color_label_definitions (name, display_name, hex, position) VALUES
    ('red',    'Rot',    '#e53e3e', 0),
    ('yellow', 'Gelb',   '#d69e2e', 1),
    ('green',  'Grün',   '#38a169', 2),
    ('blue',   'Blau',   '#3182ce', 3),
    ('purple', 'Lila',   '#805ad5', 4);
