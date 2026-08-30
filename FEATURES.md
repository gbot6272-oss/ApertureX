# FEATURES.md — Feature-Matrix Aperture X

Vollständige Feature-Liste aus `SPEC.md`, ein Punkt pro Zeile mit Checkbox, Ziel-Phase (aus `SPEC.md` Abschnitt 5) und Status. Phasen-Zuordnung ist die sinnvollste Interpretation der Phasenbeschreibung und wird beim Start der jeweiligen Phase in `PLAN.md` verfeinert, falls nötig — Änderungen daran gehören dann in `DECISIONS.md`.

**Status-Werte:** `Nicht begonnen` · `In Arbeit` · `Fertig` · `Fertig (abweichend, siehe DECISIONS.md)`

---

## 3.1 Modul BIBLIOTHEK
<!-- Phasen-Zuordnung korrigiert (siehe DECISIONS.md ADR-0022, analog zu
     ADR-0011 bei Phase 2): SPEC.md §5s Phase-3-Satz nennt nur "Import,
     Ordner, Raster, Filmstreifen, Vorschau-Generierung, Bewertungen/
     Flaggen/Farben, Sammlungen, Filter, Metadaten-Panel, FTS-Suche" —
     §3.1s vollständiger Feature-Katalog (der komplette Lightroom-
     BIBLIOTHEK-Umfang) hatte aber deutlich mehr Punkte auf Phase 3
     getaggt. Die unten nicht mehr auf Phase 3 stehenden Punkte sind auf
     die Phase verschoben, zu der sie inhaltlich am besten passen.
     Zwei Punkte (Duplikaterkennung per Hash, Sortierung) waren bei
     dieser ersten Korrektur übersehen worden und sind erst bei der
     Phase-3-Abnahme (Schritt 7) nachträglich umgetaggt worden, siehe
     DECISIONS.md ADR-0026 — auf ausdrücklichen Nutzerwunsch aber noch
     in Phase 3 nachgezogen worden (zusammen mit drei weiteren im
     Abschlussbericht ehrlich benannten Lücken), siehe ADR-0027. -->

- [x] Import: Ordner scannen, Metadaten lesen, Thumbnails erzeugen (Basisfunktion) — Phase 1 — Status: Fertig
- [x] Import mit Kopieren/Verschieben/Hinzufügen — Phase 3 — Status: Fertig
- [ ] Import mit DNG-Konvertierung — Phase 5 — Status: Nicht begonnen
- [x] Import-Presets — Phase 3 — Status: Fertig
- [x] Automatisches Umbenennen mit Token-System — Phase 3 — Status: Fertig
- [x] Duplikaterkennung per exaktem Hash — Phase 3 — Status: Fertig (siehe ADR-0027: `content_hash`-Spalte existierte bereits seit Phase 1, wird jetzt per Streaming-SHA-256 beim Import befüllt; reine Anzeige, blockiert den Import nicht)
- [ ] Duplikaterkennung per Perceptual Hash, Duplikat-Assistent mit Auto-Auswahl bester Version — Phase 9 — Status: Nicht begonnen
- [x] Ordnerbaum (Basis-Anzeige, Fotoanzahl je Ordner) — Phase 1 — Status: Fertig (flache Liste, kein Baum — echte Hierarchie/Synchronisation ist Phase 3, siehe Zeile darunter)
- [x] Ordnerbaum-Synchronisation (echte Hierarchie über `parent_id`) — Phase 3 — Status: Fertig (siehe DECISIONS.md ADR-0027: Import legt jetzt die volle Verzeichniskette bis zum gewählten Import-Ordner bzw. bei Copy/Move bis zum Zielordner an, statt nur den unmittelbaren Elternordner)
- [x] Ordner fehlend/wiederfinden — Phase 3 — Status: Fertig
- [x] Sammlungen (manuell, feste Reihenfolge) — Phase 3 — Status: Fertig
- [ ] Sammlungssätze, intelligente Sammlungen mit verschachtelten UND/ODER-Regeln, Zielsammlung — Phase 6 — Status: Nicht begonnen
- [ ] Stapel (automatisch nach Zeit, manuell) — Phase 6 — Status: Nicht begonnen
- [ ] Virtuelle Kopien — Phase 6 — Status: Nicht begonnen
- [x] Bewertung 0–5 — Phase 3 — Status: Fertig
- [x] Farbmarkierungen (fester Grundsatz) — Phase 3 — Status: Fertig (feste Palette aus 5 Farben: rot/gelb/grün/blau/violett)
- [ ] Farbmarkierungen erweiterbar auf beliebig viele, benannt — Phase 6 — Status: Nicht begonnen
- [x] Flaggen — Phase 3 — Status: Fertig
- [x] Schlagworte (flache Liste, ohne Hierarchie) — Phase 3 — Status: Fertig
- [ ] Schlagworthierarchie (Synonyme, Export-Steuerung, Auto-Vervollständigung), Schlagwortvorschläge, Tag-Regeln (bedingte Auto-Tags) — Phase 6 — Status: Nicht begonnen
- [x] Metadaten-Panel (Basisfelder lesen, Bewertung/Flagge/Farbe/Schlagworte editieren) — Phase 3 — Status: Fertig
- [x] Undo/Redo für Bibliotheks-Metadaten (Bewertung/Flagge/Farbe/Schlagworte/Sammlungsmitgliedschaft) — Phase 3 — Status: Fertig (siehe DECISIONS.md ADR-0027; deckt bewusst nicht Sammlung anlegen/umbenennen/löschen ab)
- [ ] Metadaten-Presets, Stapel-Metadatenbearbeitung, EXIF/IPTC/XMP-Editor (alle Felder), frei definierbare Metadaten-Felder, Sidecar-Export (.xmp) — Phase 6 — Status: Nicht begonnen
- [x] Volltextsuche (FTS5) über Dateiname, Kamera, Objektiv — Phase 3 — Status: Fertig
- [x] Rasteransicht — Phase 3 — Status: Fertig
- [x] Lupe/Einzelbildansicht (Basis-Viewer) — Phase 1 — Status: Fertig
- [ ] Vergleichsansicht, Übersichtsansicht — Phase 6 — Status: Nicht begonnen
- [ ] Personenansicht (Gesichtserkennung) — Phase 9 — Status: Nicht begonnen
- [x] Filterleiste (Text, Attribut, Metadaten, kombiniert) — Phase 3 — Status: Fertig (siehe DECISIONS.md ADR-0027: Text- und Attributfilter [inkl. Kameramodell] sind jetzt per UND kombinierbar, nicht mehr alternativ wie in ADR-0026)
- [ ] Filter-Presets — Phase 6 — Status: Nicht begonnen
- [x] Sortierung nach beliebigem Feld — Phase 3 — Status: Fertig (siehe ADR-0027: client-seitig, Dateiname/Aufnahmedatum/Bewertung/Dateigröße/Kameramodell, fehlende Werte immer ans Ende)
- [ ] Schnellentwicklung im Raster — Phase 6 — Status: Nicht begonnen
- [ ] Vorschau-Cache-Verwaltung (Standard, 1:1), Smart Previews, Offline-Bearbeitung über Smart Previews — Phase 6 — Status: Nicht begonnen
- [ ] Sekundäres Display mit unabhängiger Ansicht — Phase 9 — Status: Nicht begonnen
- [ ] Katalog-Statistiken-Dashboard — Phase 9 — Status: Nicht begonnen

## 3.2 Modul ENTWICKELN — Globale Werkzeuge

### Grundeinstellungen
<!-- Phasen-Zuordnung korrigiert (siehe DECISIONS.md ADR-0011): SPEC.md §5s
     Phase-2-Satz nennt nur die folgenden sieben Regler namentlich; Textur/
     Klarheit/Dunst entfernen/Dynamik/Sättigung gehören daher zu Phase 4,
     nicht zu Phase 2. Aus demselben Grund gehören Pipette und
     Kamera-Presets (Teil von §3.2s vollem Weißabgleich-Feature, aber
     nicht Teil des Phase-2-Satzes) ebenfalls erst zu einer späteren
     Phase — Weißabgleich selbst (Temperatur+Tint als Regler) ist für
     Phase 2 fertig. -->
- [x] Weißabgleich (Temperatur, Tint) — Phase 2 — Status: Fertig
- [x] Weißabgleich-Pipette (Klick ins Bild) — Phase 4 — Status: Fertig (bewusst vereinfacht: rechnet auf dem gamma-kodierten Anzeigebild statt linearem Kamera-RGB, siehe `frontend/src/lib/whiteBalancePicker.ts`)
- [x] Weißabgleich-Kamera-Presets (Tageslicht/Bewölkt/…) — Phase 4 — Status: Fertig (feste, nicht kamera-kalibrierte Presets, siehe ADR-0028)
- [x] Belichtung — Phase 2 — Status: Fertig
- [x] Kontrast — Phase 2 — Status: Fertig
- [x] Lichter — Phase 2 — Status: Fertig
- [x] Tiefen — Phase 2 — Status: Fertig
- [x] Weiß — Phase 2 — Status: Fertig
- [x] Schwarz — Phase 2 — Status: Fertig
- [x] Textur — Phase 4 — Status: Fertig
- [x] Klarheit — Phase 4 — Status: Fertig
- [x] Dunst entfernen — Phase 4 — Status: Fertig (vereinfachtes Modell, kein Dark-Channel-Prior, siehe `crates/apx-pipeline/src/edl/v2.rs`)
- [x] Dynamik — Phase 4 — Status: Fertig
- [x] Sättigung — Phase 4 — Status: Fertig

### Gradationskurve
- [x] Punktkurve — Phase 4 — Status: Fertig (monotone kubische Fritsch-Carlson-Spline, siehe `crates/apx-pipeline/src/stages/curves.rs`)
- [x] Parametrische Kurve — Phase 4 — Status: Fertig (vereinfachtes Gauß-gewichtetes Vier-Zonen-Modell statt echter verschiebbarer Split-Punkte, siehe `curves.rs`)
- [x] RGB-Verbundkurve + einzelne Kanäle — Phase 4 — Status: Fertig
- [x] Luminanz-Kurve — Phase 4 — Status: Fertig
- [x] Numerische Punkteingabe — Phase 4 — Status: Fertig
- [x] Kurven-Presets — Phase 4 — Status: Fertig (4 feste Presets: Linear/Leichter Kontrast/Starker Kontrast/Negativ)

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
<!-- Scope-Präzisierung siehe DECISIONS.md ADR-0028: eine echte
     Adobe-LCP-kompatible Profildatenbank ist ohne die nötigen Testdaten
     ein eigenständiges Mammutprojekt. Phase 4 liefert ein eigenes,
     minimales Profilformat (handgepflegtes JSON, wenige Beispielprofile,
     Zuordnung per EXIF-Objektiv-/Kamerastring) plus die vollen manuellen
     Regler; echter Adobe-Profil-Import wird auf eine spätere Phase
     verschoben. -->
- [ ] Profilbasierte Korrektur (Datenbank + Import) — Phase 4 — Status: Nicht begonnen
- [ ] Chromatische Aberration (auto + manuell) — Phase 4 — Status: Nicht begonnen
- [ ] Vignettierung — Phase 4 — Status: Nicht begonnen
- [ ] Verzeichnung — Phase 4 — Status: Nicht begonnen
- [ ] Perspektive/Upright (Auto, Level, Vertical, Full, Guided) — Phase 4 — Status: Nicht begonnen (Guided-Modus vereinfacht auf 2 statt bis zu 4 Linienpaare, siehe ADR-0028)
- [ ] Manuelle Transformation (V/H, Drehen, Seitenverhältnis, Skalieren, Versatz) — Phase 4 — Status: Nicht begonnen

### Effekte
- [ ] Nachträgliche Vignettierung — Phase 4 — Status: Nicht begonnen
- [ ] Körnung — Phase 4 — Status: Nicht begonnen

### Kalibrierung
- [ ] Prozessversion — Phase 4 — Status: Nicht begonnen
- [ ] Schattentönung — Phase 4 — Status: Nicht begonnen
- [ ] Primärfarben-Regler R/G/B (Farbton, Sättigung) — Phase 4 — Status: Nicht begonnen
- [ ] Kamera-Profile (kleine eingebaute Liste) inkl. DCP-Import — Phase 4 — Status: Nicht begonnen (DCP-Import selbst auf spätere Phase verschoben, siehe ADR-0028 — dasselbe Adobe-Format-Problem wie bei Objektivprofilen)

### Geometrie
- [ ] Freistellen (Presets, eigene Verhältnisse, Rasterüberlagerungen) — Phase 4 — Status: Nicht begonnen
- [ ] Winkel-Werkzeug — Phase 4 — Status: Nicht begonnen
- [ ] Auto-Ausrichtung am Horizont — Phase 4 — Status: Nicht begonnen (vereinfacht: kein echtes Kantenerkennungs-Verfahren, siehe ADR-0028)

### Reparatur
<!-- Scope-Präzisierung siehe DECISIONS.md ADR-0028: Auto-Quellenfindung
     und inhaltsbasiertes Füllen für größere Bereiche sind fortgeschrittene
     Computer-Vision-Algorithmen (vergleichbar mit PatchMatch/Content-Aware
     Fill) und werden auf eine spätere Phase verschoben. Phase 4 liefert
     manuelles Klonen/Reparieren (Pinsel mit Quellpunkt, Radius, Deckkraft,
     weicher Kante). -->
- [ ] Bereichsreparatur klonen/reparieren — Phase 4 — Status: Nicht begonnen
- [ ] Auto-Quellenfindung — Phase 6 — Status: Nicht begonnen (siehe ADR-0028)
- [ ] Sensorflecken-Visualisierung — Phase 4 — Status: Nicht begonnen
- [ ] Inhaltsbasiertes Füllen — Phase 6 — Status: Nicht begonnen (siehe ADR-0028)

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

- [x] Undo/Redo (unbegrenzt, dauerhaft über `edit_history`, überlebt App-Neustart) — Phase 2 — Status: Fertig — **Teil-Einschränkung:** kein sichtbares, klickbares Verlaufs-*Panel* mit benannten Schritten (nur Rückgängig/Wiederholen um je einen Schritt) — siehe `DECISIONS.md` ADR-0018s Korrektur-Notiz; die Backend-Grundlage (`list_edit_history`) für ein solches Panel existiert noch nicht und ist ein möglicher Ausbau für eine spätere Phase
- [ ] Schnappschüsse — Phase 6 — Status: Nicht begonnen (siehe ADR-0028: `SPEC.md` §5s Phase-4-Satz nennt nur die 10 Werkzeugkategorien, nicht die Workflow-Punkte aus §3.4)
- [ ] Vorher/Nachher in vier Ansichten — Phase 6 — Status: Nicht begonnen (siehe ADR-0028)
- [ ] Einstellungen kopieren/einfügen (granular) — Phase 6 — Status: Nicht begonnen (siehe ADR-0028)
- [ ] Vorherige übernehmen — Phase 6 — Status: Nicht begonnen (siehe ADR-0028)
- [ ] Synchronisieren über beliebig viele Bilder — Phase 6 — Status: Nicht begonnen (siehe ADR-0028)
- [ ] Auto-Sync-Modus — Phase 6 — Status: Nicht begonnen (siehe ADR-0028)
- [ ] Referenzansicht — Phase 6 — Status: Nicht begonnen (siehe ADR-0028)
- [ ] Soft-Proof (Zielprofil, Renderpriorität, Farbumfangswarnung, Papierweiß) — Phase 6 — Status: Nicht begonnen (siehe ADR-0028)

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

- [x] Grundlayout (Kopfzeile, linke Spalte, Mitte-Viewer, unten Filmstreifen) — Phase 1 — Status: Fertig
- [ ] Rechte Werkzeug-Palette, Modul-Umschalter oben — Phase 3 — Status: Nicht begonnen
- [x] Dark-First-Theme — Phase 1 — Status: Fertig (Tailwind-CSS-4-`@theme`-Tokens in `src/index.css`)
- [ ] Vollständiges helles Theme + benutzerdefinierte Themes (Design-Tokens) — Phase 10 — Status: Nicht begonnen
- [ ] Paletten ein-/ausklappbar, breitenziehbar, Arbeitsbereich-Preset speicherbar — Phase 3 — Status: Nicht begonnen
- [x] Regler-Standardverhalten (Doppelklick=Reset, Direkteingabe, Pfeiltasten=Feinschritt, Shift=Grobschritt) — Phase 2 — Status: Fertig — **Teil-Einschränkung:** Alt-Maskenvorschau nicht implementiert (keiner der 7 Phase-2-Regler hat eine Maskenvorschau — die betrifft eher spätere Regler wie Schärfung/lokale Masken, siehe Phase 4/6)
- [x] Grundlegende Tastenkürzel (Bildwechsel, Zoom, Vollbild) — Phase 1 — Status: Fertig
- [ ] Vollständig belegbare Tastenkürzel + Cheatsheet-Overlay (`?`) — Phase 10 — Status: Nicht begonnen
- [x] Befehlspalette `Strg/Cmd+K` — Grundgerüst (Ordner/Befehle) — Phase 1 — Status: Fertig
- [ ] Befehlspalette — vollständig (jede Funktion/jedes Preset) — Phase 5 — Status: Nicht begonnen
- [x] Nicht-blockierende UI für alle langen Operationen (Hintergrund, Fortschritt, Abbruch) — Phase 1 (Import) / laufend erweitert — Status: Fertig für Import; wird bei jeder neuen langen Operation (Export, Stapelverarbeitung, …) fortgeführt

## Technische Grundlage (Phase 1, keine Endnutzer-Features)

- [x] Rust-Workspace mit Crate-Grenzen (`apx-core`, `apx-raw`, `apx-catalog`, `apx-app`) — Phase 1 — Status: Fertig
- [x] `apx-core` (IDs, AppError, AppPaths, Settings, Logging) — Phase 1 — Status: Fertig
- [x] SQLite-Katalog mit versionierten Migrationen — Phase 1 — Status: Fertig
- [x] RAW-Dekodierung (provisorische Kette, Formate CR2/CR3/NEF/ARW/RAF/ORF/RW2/DNG + JPEG/PNG/TIFF) — Phase 1 — Status: Fertig (abweichend, siehe DECISIONS.md ADR-0007 — Golden-Image-Tests gegen echte Kameradateien fehlen noch, Netzwerkzugriff auf raw.pixls.us blockiert)
- [x] Custom-Protokoll-Handler für Bildübertragung — Phase 1 — Status: Fertig
- [x] Viewer mit Zoom/Pan (Canvas 2D, provisorisch) — Phase 1 — Status: Fertig
- [x] Testabdeckung (Rust-Unit-/Integrationstests, Vitest, Playwright-E2E) — Phase 1 — Status: Fertig (abweichend, siehe DECISIONS.md ADR-0010 — Playwright läuft gegen den Produktions-Build im Browser mit simulierter Tauri-Brücke, nicht gegen die kompilierte native App; echtes natives E2E bräuchte `tauri-driver` + WebdriverIO)
- [x] CI (Windows/macOS/Linux, fmt/clippy/test/build) — Phase 1 — Status: Fertig (`.github/workflows/ci.yml`; volles `tauri build` mit Installer/Signierung bleibt Phase 10)
