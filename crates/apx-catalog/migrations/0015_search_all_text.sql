-- Volltextsuche über ALLE Textfelder eines Fotos (Phase 34 F1, siehe
-- `DECISIONS.md` ADR-0070).
--
-- Bisher indizierte `photos_fts` nur Dateiname, Kamerahersteller,
-- Kameramodell und Objektiv. Titel, Beschriftung, Urheber und Copyright
-- — also genau die Felder, die der Nutzer selbst pflegt (siehe
-- `MetadataDialog`/`MetadataPresetDialog`) — waren nicht auffindbar. Wer
-- eine Bildunterschrift vergeben hat, konnte danach nicht suchen.
--
-- `photos_fts` ist eine External-Content-Tabelle über `photos`: sie kann
-- nur Spalten DIESER Tabelle indizieren. Schlagworte (`photo_keywords`)
-- und Notizen (`photo_notes`) liegen in eigenen Tabellen und werden
-- deshalb in `repository::search` per EXISTS-Unterabfrage mitgesucht,
-- nicht hier indiziert — Begründung siehe ADR-0070.
--
-- Spalten lassen sich einer FTS5-Tabelle nicht nachträglich hinzufügen;
-- sie muss neu angelegt und neu befüllt werden. Die drei Trigger hängen
-- an `photos` (nicht an `photos_fts`) und überleben ein `DROP TABLE
-- photos_fts`, müssen also ausdrücklich gelöscht werden — anders als in
-- Migration 0007, wo `DROP TABLE photos` sie automatisch mitnahm.

DROP TRIGGER photos_fts_after_insert;
DROP TRIGGER photos_fts_after_delete;
DROP TRIGGER photos_fts_after_update;
DROP TABLE photos_fts;

CREATE VIRTUAL TABLE photos_fts USING fts5(
    filename, camera_make, camera_model, lens,
    title, caption, creator, copyright,
    content='photos', content_rowid='rowid'
);

INSERT INTO photos_fts(
    rowid, filename, camera_make, camera_model, lens,
    title, caption, creator, copyright
)
SELECT
    rowid, filename, camera_make, camera_model, lens,
    title, caption, creator, copyright
FROM photos;

CREATE TRIGGER photos_fts_after_insert AFTER INSERT ON photos BEGIN
    INSERT INTO photos_fts(
        rowid, filename, camera_make, camera_model, lens,
        title, caption, creator, copyright
    )
    VALUES (
        new.rowid, new.filename, new.camera_make, new.camera_model, new.lens,
        new.title, new.caption, new.creator, new.copyright
    );
END;

CREATE TRIGGER photos_fts_after_delete AFTER DELETE ON photos BEGIN
    INSERT INTO photos_fts(
        photos_fts, rowid, filename, camera_make, camera_model, lens,
        title, caption, creator, copyright
    )
    VALUES (
        'delete', old.rowid, old.filename, old.camera_make, old.camera_model, old.lens,
        old.title, old.caption, old.creator, old.copyright
    );
END;

CREATE TRIGGER photos_fts_after_update AFTER UPDATE ON photos BEGIN
    INSERT INTO photos_fts(
        photos_fts, rowid, filename, camera_make, camera_model, lens,
        title, caption, creator, copyright
    )
    VALUES (
        'delete', old.rowid, old.filename, old.camera_make, old.camera_model, old.lens,
        old.title, old.caption, old.creator, old.copyright
    );
    INSERT INTO photos_fts(
        rowid, filename, camera_make, camera_model, lens,
        title, caption, creator, copyright
    )
    VALUES (
        new.rowid, new.filename, new.camera_make, new.camera_model, new.lens,
        new.title, new.caption, new.creator, new.copyright
    );
END;
