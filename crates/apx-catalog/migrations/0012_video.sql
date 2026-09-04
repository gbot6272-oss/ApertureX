-- Migration 12: Video als Katalog-Asset (Phase 16 Schritt 4, siehe
-- PLAN.md/DECISIONS.md ADR-0043). Erweitert die bestehende `photos`-
-- Tabelle statt einer eigenen `videos`-Tabelle: ein Video lebt in
-- derselben Bibliothek wie Fotos (Sammlungen, Schlagworte, Sterne,
-- Filter, Duplikat-Erkennung, Batch-Verarbeitung — alles funktioniert
-- automatisch weiter), unterscheidet sich nur durch ein paar zusätzliche,
-- bei Fotos leere Spalten.

ALTER TABLE photos ADD COLUMN media_kind TEXT NOT NULL DEFAULT 'photo';
ALTER TABLE photos ADD COLUMN duration_ms INTEGER;
ALTER TABLE photos ADD COLUMN video_codec TEXT;
-- 0/1, NULL = unbekannt (z. B. noch nicht per ffprobe geprüft).
ALTER TABLE photos ADD COLUMN has_audio INTEGER;
ALTER TABLE photos ADD COLUMN frame_rate REAL;

-- Für Filter/Grid-Trennung "nur Videos"/"nur Fotos" (Phase 16 Schritt 5).
CREATE INDEX idx_photos_media_kind ON photos(media_kind);
