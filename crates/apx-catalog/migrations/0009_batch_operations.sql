-- Migration 9: Stapelverarbeitungs-Konsole (Phase 11 Schritt 9, siehe
-- PLAN.md/DECISIONS.md ADR-0038 — die in ADR-0036 explizit benannte
-- Lücke aus Phase 9). Ein generisches Journal für gruppierte
-- Katalogmutationen (Bewertung/Schlagwort/Farbmarkierung), das echtes
-- Batch-Undo ermöglicht — der in ADR-0036 benannte eigentliche Blocker.

CREATE TABLE batch_operations (
    id         TEXT PRIMARY KEY,
    -- z. B. "set_rating"/"add_keyword"/"set_color_label" — rein
    -- informativ fürs Frontend (Anzeige in der Verlaufsliste), die
    -- Undo-Logik selbst liest ausschließlich `batch_operation_items`.
    kind       TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    dry_run    INTEGER NOT NULL
);

-- Ein Trockenlauf (`dry_run = 1`) schreibt bewusst *auch* eine Zeile in
-- `batch_operations` (aber keine `batch_operation_items`, siehe
-- `apx_catalog::repository::batch`s Moduldoku) — so bleibt in der
-- Verlaufsliste sichtbar, dass ein Trockenlauf stattfand, ohne dass er
-- rückgängig gemacht werden könnte (keine Items = nichts zu widerrufen).

CREATE TABLE batch_operation_items (
    id            TEXT PRIMARY KEY,
    batch_id      TEXT NOT NULL REFERENCES batch_operations(id) ON DELETE CASCADE,
    photo_id      TEXT NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
    -- "rating"/"color_label"/"keyword" — bestimmt, wie
    -- old_value_json/new_value_json beim Undo interpretiert werden
    -- (siehe apply_batch_rule/undo_batch_operation).
    field         TEXT NOT NULL,
    old_value_json TEXT NOT NULL,
    new_value_json TEXT NOT NULL
);

CREATE INDEX idx_batch_operation_items_batch_id ON batch_operation_items(batch_id);
CREATE INDEX idx_batch_operations_created_at ON batch_operations(created_at);
