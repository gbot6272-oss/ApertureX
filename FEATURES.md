# FEATURES.md — Feature-Matrix Aperture X

Vollständige Feature-Liste aus `SPEC.md`, ein Punkt pro Zeile mit Checkbox, Ziel-Phase (aus `SPEC.md` Abschnitt 5) und Status. Phasen-Zuordnung ist die sinnvollste Interpretation der Phasenbeschreibung und wird beim Start der jeweiligen Phase in `PLAN.md` verfeinert, falls nötig — Änderungen daran gehören dann in `DECISIONS.md`.

**Status-Werte:** `Nicht begonnen` · `In Arbeit` · `Fertig` · `Fertig (abweichend, siehe DECISIONS.md)`

---

## 3.1 Modul BIBLIOTHEK

- [x] Import: Ordner scannen, Metadaten lesen, Thumbnails erzeugen (Basisfunktion) — Phase 1 — Status: Fertig
- [ ] Import mit Kopieren/Verschieben/Hinzufügen/DNG-Konvertierung — Phase 3 — Status: Nicht begonnen
- [ ] Import-Presets — Phase 3 — Status: Nicht begonnen
- [ ] Automatisches Umbenennen mit Token-System — Phase 3 — Status: Nicht begonnen
- [ ] Duplikaterkennung per Hash + Perceptual Hash — Phase 3 — Status: Nicht begonnen
- [ ] Ordnerbaum (Basis-Anzeige, Fotoanzahl je Ordner) — Phase 1 — Status: Nicht begonnen
- [ ] Ordnerbaum-Synchronisation — Phase 3 — Status: Nicht begonnen
- [ ] Ordner fehlend/wiederfinden — Phase 3 — Status: Nicht begonnen
- [ ] Sammlungen, Sammlungssätze — Phase 3 — Status: Nicht begonnen
- [ ] Intelligente Sammlungen mit verschachtelten UND/ODER-Regeln — Phase 3 — Status: Nicht begonnen
- [ ] Zielsammlung — Phase 3 — Status: Nicht begonnen
- [ ] Stapel (automatisch nach Zeit, manuell) — Phase 3 — Status: Nicht begonnen
- [ ] Virtuelle Kopien — Phase 3 — Status: Nicht begonnen
- [ ] Bewertung 0–5 — Phase 3 — Status: Nicht begonnen
- [ ] Farbmarkierungen (erweiterbar) — Phase 3 — Status: Nicht begonnen
- [ ] Flaggen — Phase 3 — Status: Nicht begonnen
- [ ] Schlagworthierarchie (Synonyme, Export-Steuerung, Auto-Vervollständigung) — Phase 3 — Status: Nicht begonnen
- [ ] Schlagwortvorschläge — Phase 3 — Status: Nicht begonnen
- [ ] Metadaten-Presets — Phase 3 — Status: Nicht begonnen
- [ ] Stapel-Metadatenbearbeitung — Phase 3 — Status: Nicht begonnen
- [ ] EXIF/IPTC/XMP-Editor (alle Felder) — Phase 3 — Status: Nicht begonnen
- [ ] Rasteransicht — Phase 3 — Status: Nicht begonnen
- [ ] Lupe/Einzelbildansicht (Basis-Viewer) — Phase 1 — Status: Nicht begonnen
- [ ] Vergleichsansicht — Phase 3 — Status: Nicht begonnen
- [ ] Übersichtsansicht — Phase 3 — Status: Nicht begonnen
- [ ] Personenansicht — Phase 3 — Status: Nicht begonnen
- [ ] Filterleiste (Text, Attribut, Metadaten, kombiniert) — Phase 3 — Status: Nicht begonnen
- [ ] Filter-Presets — Phase 3 — Status: Nicht begonnen
- [ ] Sortierung nach beliebigem Feld — Phase 3 — Status: Nicht begonnen
- [ ] Schnellentwicklung im Raster — Phase 3 — Status: Nicht begonnen
- [ ] Vorschau-Cache-Verwaltung (Standard, 1:1, Smart Previews) — Phase 3 — Status: Nicht begonnen
- [ ] Offline-Bearbeitung über Smart Previews — Phase 3 — Status: Nicht begonnen
- [ ] Sekundäres Display mit unabhängiger Ansicht — Phase 3 — Status: Nicht begonnen
- [ ] Frei definierbare Metadaten-Felder — Phase 3 — Status: Nicht begonnen
- [ ] Beliebig viele benannte Farbmarkierungen — Phase 3 — Status: Nicht begonnen
- [ ] Tag-Regeln (bedingte Auto-Tags) — Phase 3 — Status: Nicht begonnen
- [ ] Katalog-Statistiken-Dashboard — Phase 3 — Status: Nicht begonnen
- [ ] Duplikat-Assistent mit Auto-Auswahl bester Version — Phase 3 — Status: Nicht begonnen

## 3.2 Modul ENTWICKELN — Globale Werkzeuge

### Grundeinstellungen
- [ ] Weißabgleich (Temperatur, Tint, Pipette, Kamera-Presets) — Phase 2 — Status: Nicht begonnen
- [ ] Belichtung — Phase 2 — Status: Nicht begonnen
- [ ] Kontrast — Phase 2 — Status: Nicht begonnen
- [ ] Lichter — Phase 2 — Status: Nicht begonnen
- [ ] Tiefen — Phase 2 — Status: Nicht begonnen
- [ ] Weiß — Phase 2 — Status: Nicht begonnen
- [ ] Schwarz — Phase 2 — Status: Nicht begonnen
- [ ] Textur — Phase 2 — Status: Nicht begonnen
- [ ] Klarheit — Phase 2 — Status: Nicht begonnen
- [ ] Dunst entfernen — Phase 2 — Status: Nicht begonnen
- [ ] Dynamik — Phase 2 — Status: Nicht begonnen
- [ ] Sättigung — Phase 2 — Status: Nicht begonnen

### Gradationskurve
- [ ] Punktkurve — Phase 4 — Status: Nicht begonnen
- [ ] Parametrische Kurve — Phase 4 — Status: Nicht begonnen
- [ ] RGB-Verbundkurve + einzelne Kanäle — Phase 4 — Status: Nicht begonnen
- [ ] Luminanz-Kurve — Phase 4 — Status: Nicht begonnen
- [ ] Numerische Punkteingabe — Phase 4 — Status: Nicht begonnen
- [ ] Kurven-Presets — Phase 4 — Status: Nicht begonnen

### HSL / Farbe
- [ ] HSL für 8 Standardbereiche — Phase 4 — Status: Nicht begonnen

### Farbmischer erweitert
- [ ] Frei definierbare Farbbereiche per Klick im Bild — Phase 4 — Status: Nicht begonnen

### Farbklassifizierung / Color Grading
- [ ] Farbräder Schatten/Mitteltöne/Lichter/Global — Phase 4 — Status: Nicht begonnen
- [ ] Luminanz und Mischung pro Rad — Phase 4 — Status: Nicht begonnen
- [ ] Balance, Überblendung — Phase 4 — Status: Nicht begonnen

### Details
- [ ] Schärfung (Betrag, Radius, Detail, Maskierung) — Phase 4 — Status: Nicht begonnen
- [ ] Luminanzrauschen (Betrag, Detail, Kontrast) — Phase 4 — Status: Nicht begonnen
- [ ] Farbrauschen (Betrag, Detail, Glättung) — Phase 4 — Status: Nicht begonnen
- [ ] Deconvolution-Schärfung — Phase 4 — Status: Nicht begonnen

### Objektivkorrekturen
- [ ] Profilbasierte Korrektur (Datenbank + Import) — Phase 4 — Status: Nicht begonnen
- [ ] Chromatische Aberration (auto + manuell) — Phase 4 — Status: Nicht begonnen
- [ ] Vignettierung — Phase 4 — Status: Nicht begonnen
- [ ] Verzeichnung — Phase 4 — Status: Nicht begonnen
- [ ] Perspektive/Upright (Auto, Level, Vertical, Full, Guided) — Phase 4 — Status: Nicht begonnen
- [ ] Manuelle Transformation (V/H, Drehen, Seitenverhältnis, Skalieren, Versatz) — Phase 4 — Status: Nicht begonnen

### Effekte
- [ ] Nachträgliche Vignettierung — Phase 4 — Status: Nicht begonnen
- [ ] Körnung — Phase 4 — Status: Nicht begonnen

### Kalibrierung
- [ ] Prozessversion — Phase 4 — Status: Nicht begonnen
- [ ] Schattentönung — Phase 4 — Status: Nicht begonnen
- [ ] Primärfarben-Regler R/G/B (Farbton, Sättigung) — Phase 4 — Status: Nicht begonnen
- [ ] Kamera-Profile inkl. DCP-Import — Phase 4 — Status: Nicht begonnen

### Geometrie
- [ ] Freistellen (Presets, eigene Verhältnisse, Rasterüberlagerungen) — Phase 4 — Status: Nicht begonnen
- [ ] Winkel-Werkzeug — Phase 4 — Status: Nicht begonnen
- [ ] Auto-Ausrichtung am Horizont — Phase 4 — Status: Nicht begonnen

### Reparatur
- [ ] Bereichsreparatur klonen/reparieren — Phase 4 — Status: Nicht begonnen
- [ ] Auto-Quellenfindung — Phase 4 — Status: Nicht begonnen
- [ ] Sensorflecken-Visualisierung — Phase 4 — Status: Nicht begonnen
- [ ] Inhaltsbasiertes Füllen — Phase 4 — Status: Nicht begonnen

## 3.3 Modul ENTWICKELN — Lokale Anpassungen

- [ ] Maskentyp Pinsel — Phase 6 — Status: Nicht begonnen
- [ ] Maskentyp Linearer Verlauf — Phase 6 — Status: Nicht begonnen
- [ ] Maskentyp Radialer Verlauf — Phase 6 — Status: Nicht begonnen
- [ ] Maskentyp Farbbereich — Phase 6 — Status: Nicht begonnen
- [ ] Maskentyp Luminanzbereich — Phase 6 — Status: Nicht begonnen
- [ ] Maskentyp Tiefenbereich — Phase 6 — Status: Nicht begonnen
- [ ] KI-Motiv-Maske — Phase 7 — Status: Nicht begonnen
- [ ] KI-Himmel-Maske — Phase 7 — Status: Nicht begonnen
- [ ] KI-Hintergrund-Maske — Phase 7 — Status: Nicht begonnen
- [ ] KI-Objekte-Maske (Klick-Segmentierung) — Phase 7 — Status: Nicht begonnen
- [ ] KI-Personen-Maske (Haut, Augen, Brauen, Lippen, Zähne, Haare, Kleidung) — Phase 7 — Status: Nicht begonnen
- [ ] Masken kombinieren (Hinzufügen/Subtrahieren/Schneiden) — Phase 6 — Status: Nicht begonnen
- [ ] Pro Maske: alle globalen Regler + Deckkraft/Weichzeichnung/Umkehren/Verfeinern — Phase 6 — Status: Nicht begonnen
- [ ] Maskengruppen, Umbenennen, Ein-/Ausblenden, Überlagerungsfarbe — Phase 6 — Status: Nicht begonnen
- [ ] Maske duplizieren / auf anderes Foto übertragen — Phase 6 — Status: Nicht begonnen
- [ ] Ebenen-Mischmodi pro Maske — Phase 6 — Status: Nicht begonnen
- [ ] Masken als wiederverwendbare Bausteine speichern — Phase 6 — Status: Nicht begonnen
- [ ] Maskenkette mit Drag-&-Drop-Sortierung — Phase 6 — Status: Nicht begonnen

## 3.4 Modul ENTWICKELN — Workflow

- [ ] Verlauf mit unbegrenzten, benennbaren, klickbaren Schritten (Undo/Redo) — Phase 2 — Status: Nicht begonnen
- [ ] Schnappschüsse — Phase 4 — Status: Nicht begonnen
- [ ] Vorher/Nachher in vier Ansichten — Phase 4 — Status: Nicht begonnen
- [ ] Einstellungen kopieren/einfügen (granular) — Phase 4 — Status: Nicht begonnen
- [ ] Vorherige übernehmen — Phase 4 — Status: Nicht begonnen
- [ ] Synchronisieren über beliebig viele Bilder — Phase 4 — Status: Nicht begonnen
- [ ] Auto-Sync-Modus — Phase 4 — Status: Nicht begonnen
- [ ] Referenzansicht — Phase 4 — Status: Nicht begonnen
- [ ] Soft-Proof (Zielprofil, Renderpriorität, Farbumfangswarnung, Papierweiß) — Phase 4 — Status: Nicht begonnen

## 3.5 PRESET- UND TEMPLATE-SYSTEM

- [ ] Presets: wählbare EDL-Teilmenge, Checkbox-Dialog beim Speichern — Phase 5 — Status: Nicht begonnen
- [ ] Preset-Ordnerhierarchie, Drag & Drop, Favoriten, Suche, Tags — Phase 5 — Status: Nicht begonnen
- [ ] Preset-Stärke 0–200 %, nachträglich änderbar — Phase 5 — Status: Nicht begonnen
- [ ] Live-Vorschau (Hover im Bild + Thumbnail in der Liste) — Phase 5 — Status: Nicht begonnen
- [ ] Preset-Stapel mit editierbarer Reihenfolge — Phase 5 — Status: Nicht begonnen
- [ ] Bedingte Presets (Bedingungssprache im UI-Builder) — Phase 5 — Status: Nicht begonnen
- [ ] Import/Export `.apx` — Phase 5 — Status: Nicht begonnen
- [ ] Import/Export Adobe `.xmp` / `.lrtemplate` (beide Richtungen) — Phase 5 — Status: Nicht begonnen
- [ ] Preset-Versionierung mit Diff-Ansicht — Phase 5 — Status: Nicht begonnen
- [ ] Preset-Generator per LLM (natürlichsprachliche Beschreibung) — Phase 7 — Status: Nicht begonnen
- [ ] Referenzbild-Modus (numerische Optimierung, kein LLM) — Phase 7 — Status: Nicht begonnen
- [ ] Variationen-Generator (Kontaktbogen) — Phase 7 — Status: Nicht begonnen
- [ ] Preset aus Bearbeitung lernen (Mustererkennung über mehrere Bilder) — Phase 7 — Status: Nicht begonnen
- [ ] Export-Templates (Ziel, Format, Qualität, Farbraum, Größe, Schärfung, Metadaten, Wasserzeichen, Mehrfachziel) — Phase 8 — Status: Nicht begonnen
- [ ] Wasserzeichen-Templates — Phase 8 — Status: Nicht begonnen
- [ ] Metadaten-Templates (Copyright/Ersteller/Kontakt/IPTC) — Phase 8 — Status: Nicht begonnen
- [ ] Import-Templates — Phase 3 — Status: Nicht begonnen
- [ ] Umbenennungs-Templates mit Token-Editor — Phase 3 — Status: Nicht begonnen
- [ ] Layout-Templates (Druck/Buch/Diashow/Web) — Phase 8 — Status: Nicht begonnen
- [ ] Workflow-Templates (Import→Filter→Preset→Export als ein Klick) — Phase 8 — Status: Nicht begonnen
- [ ] Template-Marktplatz-Struktur (lokales Repo-Format, Manifest, Installation) — Phase 5 — Status: Nicht begonnen

## 3.6 Weitere Module

### Karte
- [ ] GPS aus EXIF, Kartenansicht — Phase 8 — Status: Nicht begonnen
- [ ] GPX-Tracklog-Import — Phase 8 — Status: Nicht begonnen
- [ ] Fotos per Drag auf Karte setzen — Phase 8 — Status: Nicht begonnen
- [ ] Ortsschlagworte automatisch (Reverse Geocoding) — Phase 8 — Status: Nicht begonnen
- [ ] Reiserouten-Ansicht — Phase 8 — Status: Nicht begonnen

### Buch
- [ ] Seitenlayouts, Vorlagen, Text-Stile — Phase 8 — Status: Nicht begonnen
- [ ] Automatische Befüllung — Phase 8 — Status: Nicht begonnen
- [ ] PDF-Export — Phase 8 — Status: Nicht begonnen
- [ ] Druckerei-Presets — Phase 8 — Status: Nicht begonnen

### Diashow
- [ ] Übergänge, Ken-Burns-Effekt — Phase 8 — Status: Nicht begonnen
- [ ] Musik-Synchronisation — Phase 8 — Status: Nicht begonnen
- [ ] Intro/Outro-Screens — Phase 8 — Status: Nicht begonnen
- [ ] Video-Export (MP4 via ffmpeg) — Phase 8 — Status: Nicht begonnen

### Drucken
- [ ] Einzelbild, Kontaktbogen, Bilderpaket, benutzerdefiniertes Raster — Phase 8 — Status: Nicht begonnen
- [ ] Randeinstellungen, Zellen, Zoom — Phase 8 — Status: Nicht begonnen
- [ ] Druckschärfung, Farbmanagement, Druckauflösung — Phase 8 — Status: Nicht begonnen
- [ ] Speichern als JPEG — Phase 8 — Status: Nicht begonnen

### Web
- [ ] HTML-/responsive Galerie-Generator, Themes — Phase 8 — Status: Nicht begonnen
- [ ] Upload via FTP/SFTP — Phase 8 — Status: Nicht begonnen

### Export
- [ ] Formate JPEG/PNG/TIFF/PSD/DNG/WebP/AVIF/HEIF/JPEG XL — Phase 8 — Status: Nicht begonnen
- [ ] Farbräume sRGB/AdobeRGB/ProPhoto/Display-P3/eigenes ICC — Phase 8 — Status: Nicht begonnen
- [ ] Bit-Tiefe 8/16 — Phase 8 — Status: Nicht begonnen
- [ ] Größenbegrenzung (Kante/Megapixel/Dateigröße) — Phase 8 — Status: Nicht begonnen
- [ ] Ausgabeschärfung nach Medium — Phase 8 — Status: Nicht begonnen
- [ ] Wasserzeichen, Metadaten-Filter — Phase 8 — Status: Nicht begonnen
- [ ] Export-Warteschlange (Fortschritt, Pausieren, Priorisieren) — Phase 8 — Status: Nicht begonnen

### Zusätzliche Module (über Lightroom hinaus)
- [ ] Node-Editor (Pipeline als Knotengraph) — Phase 9 — Status: Nicht begonnen
- [ ] Stapelverarbeitungs-Konsole (Vorschau, Trockenlauf, Rückgängig) — Phase 9 — Status: Nicht begonnen
- [ ] Fokus-Stacking — Phase 9 — Status: Nicht begonnen
- [ ] HDR-Zusammenführung — Phase 9 — Status: Nicht begonnen
- [ ] Panorama-Zusammenführung (sphärisch/zylindrisch/perspektivisch, Auto-Crop/-Fill) — Phase 9 — Status: Nicht begonnen
- [ ] Astro-Stacking mit Sternausrichtung — Phase 9 — Status: Nicht begonnen
- [ ] Tethered Shooting (gphoto2/PTP, Live-View, Auto-Preset) — Phase 9 — Status: Nicht begonnen
- [ ] Vergleichs-Grid (bis 9 Versionen, sync. Zoom) — Phase 9 — Status: Nicht begonnen
- [ ] Skript-API (Lua/Rhai) + Plugin-System mit stabilem ABI — Phase 9 — Status: Nicht begonnen
- [ ] Zeitleisten-Ansicht der Bearbeitungshistorie — Phase 9 — Status: Nicht begonnen
- [ ] Verlaufs-Vergleich (zwei Schritte gegenüberstellen) — Phase 9 — Status: Nicht begonnen
- [ ] Kollaborationsmodus (Katalog-Teilfreigabe, Merge, Konfliktauflösung) — Phase 9 — Status: Nicht begonnen
- [ ] Barrierefreiheit (Tastatur, Screenreader, Kontrastmodus, UI-Skalierung 75–200 %) — Phase 10 — Status: Nicht begonnen

## 4. UI-Anforderungen

- [ ] Grundlayout (Kopfzeile, linke Spalte, Mitte-Viewer, unten Filmstreifen) — Phase 1 — Status: Nicht begonnen
- [ ] Rechte Werkzeug-Palette, Modul-Umschalter oben — Phase 3 — Status: Nicht begonnen
- [ ] Dark-First-Theme — Phase 1 — Status: Nicht begonnen
- [ ] Vollständiges helles Theme + benutzerdefinierte Themes (Design-Tokens) — Phase 10 — Status: Nicht begonnen
- [ ] Paletten ein-/ausklappbar, breitenziehbar, Arbeitsbereich-Preset speicherbar — Phase 3 — Status: Nicht begonnen
- [ ] Regler-Standardverhalten (Doppelklick=Reset, Direkteingabe, Pfeiltasten, Shift, Alt-Maskenvorschau) — Phase 2 — Status: Nicht begonnen
- [ ] Grundlegende Tastenkürzel (Bildwechsel, Zoom, Vollbild) — Phase 1 — Status: Nicht begonnen
- [ ] Vollständig belegbare Tastenkürzel + Cheatsheet-Overlay (`?`) — Phase 10 — Status: Nicht begonnen
- [ ] Befehlspalette `Strg/Cmd+K` — Grundgerüst (Ordner/Befehle) — Phase 1 — Status: Nicht begonnen
- [ ] Befehlspalette — vollständig (jede Funktion/jedes Preset) — Phase 5 — Status: Nicht begonnen
- [x] Nicht-blockierende UI für alle langen Operationen (Hintergrund, Fortschritt, Abbruch) — Phase 1 (Import) / laufend erweitert — Status: Fertig für Import; wird bei jeder neuen langen Operation (Export, Stapelverarbeitung, …) fortgeführt

## Technische Grundlage (Phase 1, keine Endnutzer-Features)

- [x] Rust-Workspace mit Crate-Grenzen (`apx-core`, `apx-raw`, `apx-catalog`, `apx-app`) — Phase 1 — Status: Fertig
- [x] `apx-core` (IDs, AppError, AppPaths, Settings, Logging) — Phase 1 — Status: Fertig
- [x] SQLite-Katalog mit versionierten Migrationen — Phase 1 — Status: Fertig
- [x] RAW-Dekodierung (provisorische Kette, Formate CR2/CR3/NEF/ARW/RAF/ORF/RW2/DNG + JPEG/PNG/TIFF) — Phase 1 — Status: Fertig (abweichend, siehe DECISIONS.md ADR-0007 — Golden-Image-Tests gegen echte Kameradateien fehlen noch, Netzwerkzugriff auf raw.pixls.us blockiert)
- [x] Custom-Protokoll-Handler für Bildübertragung — Phase 1 — Status: Fertig
- [ ] Viewer mit Zoom/Pan (Canvas 2D, provisorisch) — Phase 1 — Status: Nicht begonnen
- [ ] CI (Windows/macOS/Linux, fmt/clippy/test/build) — Phase 1 — Status: Nicht begonnen
