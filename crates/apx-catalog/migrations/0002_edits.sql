-- Migration 2: Bearbeitungsverlauf für Phase 2 (siehe DECISIONS.md
-- ADR-0013/ADR-0014). Additiv zu Migration 1 — photos/folders/previews
-- bleiben unverändert, Migrationen werden nie nachträglich geändert.
--
-- edit_history speichert vollständige EDL-Schnappschüsse pro Bearbeitungs-
-- schritt (kein Operations-Log). edl_json ist für diese Tabelle
-- undurchsichtig — sie enthält einen serialisierten apx_core::EdlEnvelope
-- (schema_version + payload); apx-catalog interpretiert den Inhalt nicht.
--
-- edit_current zeigt pro Foto auf die aktuell aktive Zeile in
-- edit_history. Fehlt ein Eintrag für ein Foto, ist der Ausgangszustand
-- ("wie aufgenommen", kein EDL) aktiv — dafür wird bewusst keine eigene
-- Zeile mit "leerem EDL" angelegt.

CREATE TABLE edit_history (
    id            TEXT PRIMARY KEY,
    photo_id      TEXT NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
    sequence      INTEGER NOT NULL,          -- 0 = erster Bearbeitungsschritt
    label         TEXT,                      -- NULL = kein benannter Schritt
    edl_json      TEXT NOT NULL,
    created_at    INTEGER NOT NULL,
    UNIQUE (photo_id, sequence)
);

CREATE INDEX idx_edit_history_photo ON edit_history(photo_id, sequence);

CREATE TABLE edit_current (
    photo_id      TEXT PRIMARY KEY REFERENCES photos(id) ON DELETE CASCADE,
    history_id    TEXT NOT NULL REFERENCES edit_history(id) ON DELETE CASCADE
);
