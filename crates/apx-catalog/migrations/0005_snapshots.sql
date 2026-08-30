-- Migration 5: Schnappschüsse für Phase 6 Schritt 8 (siehe DECISIONS.md
-- ADR-0032). Additiv zu den Migrationen 1-4.
--
-- **Nachtrag/Korrektur gegenüber der ursprünglichen Plan-Formulierung**
-- ("kein neues Backend-Konzept nötig — ein Schnappschuss ist ein
-- benannter Verweis auf einen bestehenden Verlaufs-Stand"): eine
-- benannte Referenz auf eine `edit_history`-Zeile wäre nicht sicher,
-- weil `commit_edit` (`repository/edits.rs`) jede "Zukunft" (Zeilen mit
-- höherer Sequenznummer als der aktuellen) beim nächsten Bearbeitungs-
-- schritt hart löscht (ADR-0014) — ein Schnappschuss, der auf eine
-- inzwischen per Rückgängig verlassene und dann überschriebene Zeile
-- zeigt, würde ohne Vorwarnung verschwinden. "Zusätzlich zum linearen
-- Verlauf" (die Formulierung aus PLAN.md) heißt aber gerade: unabhängig
-- vom linearen Verlauf erhalten bleiben. Deshalb eine eigene, kleine
-- Tabelle mit einer eigenen Kopie des EDL-JSON statt eines Verweises.
CREATE TABLE snapshots (
    id            TEXT PRIMARY KEY,
    photo_id      TEXT NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    edl_json      TEXT NOT NULL,
    created_at    INTEGER NOT NULL
);

CREATE INDEX idx_snapshots_photo ON snapshots(photo_id, created_at);
