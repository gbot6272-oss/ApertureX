-- Migration 8: Metadaten, Schlagworthierarchie, Adobe-Interop (Phase 9
-- Schritt 2, siehe PLAN.md/DECISIONS.md ADR-0035). Additiv zu den
-- Migrationen 1-7 — rein `ALTER TABLE ADD COLUMN`/`CREATE TABLE`, keine
-- Tabellen-Neuaufbauten nötig (im Unterschied zu Migration 7).

-- Schlagworthierarchie: Eltern/Kind über `parent_id` (NULL = Wurzel-
-- Schlagwort), Synonyme als JSON-Array-Text (dasselbe Muster wie
-- `collections.smart_criteria_json`) — bewusst kein eigenes
-- Synonym-Tabellenpaar, ein Schlagwort hat selten mehr als eine
-- Handvoll Synonyme.
ALTER TABLE keywords ADD COLUMN parent_id TEXT REFERENCES keywords(id) ON DELETE SET NULL;
ALTER TABLE keywords ADD COLUMN synonyms TEXT NOT NULL DEFAULT '[]';

CREATE INDEX idx_keywords_parent ON keywords(parent_id);

-- Tag-Regeln: bedingte Auto-Schlagworte. `conditions_json` ist derselbe
-- `PresetCondition[]`-JSON-Vertrag wie bei Import-Presets
-- (`frontend/src/lib/presets.ts`) — beim Import wird jedes neu
-- importierte Foto gegen alle aktiven Regeln geprüft, bei Treffer wird
-- das Zielschlagwort automatisch verknüpft (Frontend-Auswertung, gleiche
-- `evaluateConditions`-Funktion wie Import-Presets, keine zweite
-- Implementierung in Rust).
CREATE TABLE tag_rules (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    keyword_id     TEXT NOT NULL REFERENCES keywords(id) ON DELETE CASCADE,
    conditions_json TEXT NOT NULL,
    enabled        INTEGER NOT NULL DEFAULT 1,
    created_at     INTEGER NOT NULL
);

CREATE INDEX idx_tag_rules_enabled ON tag_rules(enabled);

-- IPTC-artige Metadaten-Überschreibungen je Foto: eigene Felder statt nur
-- im EXIF der Datei, weil RAW-Originale i. d. R. nicht beschreibbar sind
-- (dieselbe Begründung wie `apx_export::metadata`s minimaler
-- JPEG-EXIF-Writer nur beim Export). Der volle EXIF/IPTC/XMP-Editor im
-- Frontend liest/schreibt diese vier Spalten sowie die bestehenden
-- Kameradaten (camera_make/camera_model/lens/iso/...), Sidecar-Export
-- (`apx_export::xmp`) liest sie ebenfalls.
ALTER TABLE photos ADD COLUMN title TEXT;
ALTER TABLE photos ADD COLUMN caption TEXT;
ALTER TABLE photos ADD COLUMN copyright TEXT;
ALTER TABLE photos ADD COLUMN creator TEXT;
