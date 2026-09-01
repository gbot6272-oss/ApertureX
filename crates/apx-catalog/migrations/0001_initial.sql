-- Migration 1: Grundschema für Phase 1 (Ordner, Fotos, Vorschauen).
-- Siehe PHASE1_PROMPT.md Abschnitt 4. Weitere Tabellen aus SPEC.md
-- Abschnitt 2.3 (collections, keywords, edits, presets, …) kommen in
-- späteren Phasen als eigene Migrationen dazu — Migrationen werden nie
-- nachträglich geändert, nur ergänzt.

CREATE TABLE folders (
    id            TEXT PRIMARY KEY,
    path          TEXT NOT NULL UNIQUE,
    parent_id     TEXT REFERENCES folders(id) ON DELETE CASCADE,
    added_at      INTEGER NOT NULL
);

CREATE TABLE photos (
    id            TEXT PRIMARY KEY,
    folder_id     TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    filename      TEXT NOT NULL,
    file_size     INTEGER NOT NULL,
    file_mtime    INTEGER NOT NULL,
    content_hash  TEXT,                    -- xxHash der ersten+letzten 1 MB + Größe
    width         INTEGER, height INTEGER,
    orientation   INTEGER NOT NULL DEFAULT 1,
    camera_make   TEXT, camera_model TEXT, lens TEXT,
    iso           INTEGER, shutter REAL, aperture REAL, focal_length REAL,
    captured_at   INTEGER,                 -- Unix-Sekunden; siehe apx-raw::RawMetadata::captured_at
                                            -- für die Zeitzonen-Annahme, wenn EXIF keinen Offset trägt
    gps_lat       REAL, gps_lon REAL,
    imported_at   INTEGER NOT NULL,
    missing       INTEGER NOT NULL DEFAULT 0,
    UNIQUE (folder_id, filename)
);

CREATE INDEX idx_photos_folder   ON photos(folder_id);
CREATE INDEX idx_photos_captured ON photos(captured_at);
CREATE INDEX idx_photos_hash     ON photos(content_hash);

CREATE TABLE previews (
    photo_id      TEXT NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
    level         INTEGER NOT NULL,        -- 0=Thumb 256, 1=Standard 2048, 2=1:1
    path          TEXT NOT NULL,           -- Datei im Cache-Verzeichnis
    generated_at  INTEGER NOT NULL,
    PRIMARY KEY (photo_id, level)
);
