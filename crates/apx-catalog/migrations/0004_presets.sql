-- Migration 4: Presets für Phase 5 (siehe DECISIONS.md ADR-0031).
-- Additiv zu den Migrationen 1-3.
--
-- Ein Preset ist reine Katalogdaten — die eigentliche EDL-Teilmenge
-- (`preset_versions.edl_subset_json`) und die Bedingungsregeln
-- (`presets.conditions_json`) sind für `apx-catalog` opake JSON-Blobs,
-- genau wie `edit_history.edl_json` (siehe ARCHITECTURE.md §5) — nur
-- `apx-pipeline`/das Frontend kennen ihre Struktur.

-- Ordnerhierarchie beliebiger Tiefe, analog zu `folders` (Phase 1) —
-- eigene Tabelle statt Wiederverwendung von `folders`, da Preset-Ordner
-- nichts mit Dateisystem-Pfaden zu tun haben. `position` hält die vom
-- Nutzer per Drag & Drop festgelegte Reihenfolge innerhalb eines
-- Elternordners (analog zu `collection_photos.position`).
CREATE TABLE preset_folders (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    parent_id     TEXT REFERENCES preset_folders(id) ON DELETE CASCADE,
    position      INTEGER NOT NULL,
    created_at    INTEGER NOT NULL
);

CREATE INDEX idx_preset_folders_parent ON preset_folders(parent_id);

-- `folder_id` ist NULLable mit `ON DELETE SET NULL` (nicht CASCADE): ein
-- gelöschter Ordner nimmt seine Presets nicht mit ins Nichts, sie
-- rutschen stattdessen an die Wurzel — passt zum Lightroom-typischen
-- Verhalten, dass Presets wertvoller sind als ihre Einordnung.
--
-- `tags_json` ist ein einfaches JSON-String-Array — anders als die
-- normalisierten `keywords`/`photo_keywords` aus Phase 3 (dort über
-- mehrere Fotos hinweg wiederverwendet und daher eine eigene Tabelle
-- wert), sind Preset-Tags freie Einzel-Labels ohne Wiederverwendungs-
-- Tracking; eine Normalisierung lohnt sich erst, wenn z. B. eine
-- Tag-Autovervollständigung über alle Presets gebraucht wird.
CREATE TABLE presets (
    id              TEXT PRIMARY KEY,
    folder_id       TEXT REFERENCES preset_folders(id) ON DELETE SET NULL,
    name            TEXT NOT NULL,
    is_favorite     INTEGER NOT NULL DEFAULT 0,
    tags_json       TEXT NOT NULL DEFAULT '[]',
    conditions_json TEXT NOT NULL DEFAULT '[]',
    created_at      INTEGER NOT NULL
);

CREATE INDEX idx_presets_folder ON presets(folder_id);

-- Versionierung: jede erneute Speicherung über ein bestehendes Preset
-- legt eine neue Zeile mit fortlaufender `sequence` an (wie
-- `edit_history`, aber ohne Undo/Redo-Zeiger — hier ist immer die Zeile
-- mit der höchsten `sequence` je `preset_id` die aktuell gültige).
CREATE TABLE preset_versions (
    id              TEXT PRIMARY KEY,
    preset_id       TEXT NOT NULL REFERENCES presets(id) ON DELETE CASCADE,
    sequence        INTEGER NOT NULL,
    edl_subset_json TEXT NOT NULL,
    created_at      INTEGER NOT NULL,
    UNIQUE(preset_id, sequence)
);

CREATE INDEX idx_preset_versions_preset ON preset_versions(preset_id);
