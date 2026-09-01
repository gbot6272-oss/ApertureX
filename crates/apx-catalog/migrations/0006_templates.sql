-- Migration 6: Vorlagen (Templates) für Phase 8 Schritt 8 (siehe
-- PLAN.md/DECISIONS.md ADR-0034). Additiv zu den Migrationen 1-5.
--
-- Eine generische Tabelle statt fünf fast identischer Tabellen (eine je
-- Ausgabemodul Export/Druck/Buch/Diashow/Web plus Workflow) — jede
-- "Vorlage" ist ohnehin nur ein benannter, gespeicherter Parametersatz
-- (dieselben `*Options`-DTOs, die die jeweiligen Dialoge schon als JSON
-- über den Tauri-IPC schicken), unterschieden über `kind`. Siehe
-- `repository::templates`s Moduldoku.
CREATE TABLE templates (
    id            TEXT PRIMARY KEY,
    kind          TEXT NOT NULL,
    name          TEXT NOT NULL,
    payload_json  TEXT NOT NULL,
    created_at    INTEGER NOT NULL
);

CREATE INDEX idx_templates_kind ON templates(kind, name);
