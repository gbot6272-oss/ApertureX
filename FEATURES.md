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
- [x] Import mit Kopieren/Verschieben/Hinzufügen — Phase 3 — Status: Fertig (Backend seit Phase 3; die damalige Fertig-Markierung war vorschnell — es gab bis Phase 5 Schritt 9 **kein** Frontend dafür, siehe `ARCHITECTURE.md` §9 und `DECISIONS.md` ADR-0031 Punkt 7. Jetzt über `ImportDialog.tsx` erreichbar)
- [ ] Import mit DNG-Konvertierung — Phase 8 — Status: Nicht begonnen (ADR-0025 taggte diese Zeile ursprünglich „Phase 5 (Export/Publish)", noch bevor `SPEC.md` §5 festlegte, dass Phase 5 tatsächlich das Preset-/Template-System ist, siehe ADR-0031 — Retag auf Phase 8, wo laut `ARCHITECTURE.md` §7 die Export-Engine inkl. DNG-Ausgabeformat tatsächlich gebaut wird)
- [x] Import-Presets — Phase 3 — Status: Fertig (dieselbe Korrektur wie die Zeile darüber: Backend seit Phase 3, Frontend erst Phase 5 Schritt 9)
- [x] Automatisches Umbenennen mit Token-System — Phase 3 — Status: Fertig (dieselbe Korrektur: der Token-Editor mit Live-Vorschau kam erst mit `ImportDialog.tsx` in Phase 5 Schritt 9 hinzu)
- [x] Duplikaterkennung per exaktem Hash — Phase 3 — Status: Fertig (siehe ADR-0027: `content_hash`-Spalte existierte bereits seit Phase 1, wird jetzt per Streaming-SHA-256 beim Import befüllt; reine Anzeige, blockiert den Import nicht)
- [ ] Duplikaterkennung per Perceptual Hash, Duplikat-Assistent mit Auto-Auswahl bester Version — Phase 9 — Status: Nicht begonnen
- [x] Ordnerbaum (Basis-Anzeige, Fotoanzahl je Ordner) — Phase 1 — Status: Fertig (flache Liste, kein Baum — echte Hierarchie/Synchronisation ist Phase 3, siehe Zeile darunter)
- [x] Ordnerbaum-Synchronisation (echte Hierarchie über `parent_id`) — Phase 3 — Status: Fertig (siehe DECISIONS.md ADR-0027: Import legt jetzt die volle Verzeichniskette bis zum gewählten Import-Ordner bzw. bei Copy/Move bis zum Zielordner an, statt nur den unmittelbaren Elternordner)
- [x] Ordner fehlend/wiederfinden — Phase 3 — Status: Fertig
- [x] Sammlungen (manuell, feste Reihenfolge) — Phase 3 — Status: Fertig
- [ ] Sammlungssätze, intelligente Sammlungen mit verschachtelten UND/ODER-Regeln, Zielsammlung — Phase 9 — Status: Nicht begonnen (siehe ADR-0032: von Phase 6 auf Phase 9 verschoben — keine ADR hatte Phase 6 für diese Zeile je zugesagt)
- [ ] Stapel (automatisch nach Zeit, manuell) — Phase 9 — Status: Nicht begonnen (siehe ADR-0032)
- [ ] Virtuelle Kopien — Phase 9 — Status: Nicht begonnen (siehe ADR-0032)
- [x] Bewertung 0–5 — Phase 3 — Status: Fertig
- [x] Farbmarkierungen (fester Grundsatz) — Phase 3 — Status: Fertig (feste Palette aus 5 Farben: rot/gelb/grün/blau/violett)
- [ ] Farbmarkierungen erweiterbar auf beliebig viele, benannt — Phase 9 — Status: Nicht begonnen (siehe ADR-0032)
- [x] Flaggen — Phase 3 — Status: Fertig
- [x] Schlagworte (flache Liste, ohne Hierarchie) — Phase 3 — Status: Fertig
- [ ] Schlagworthierarchie (Synonyme, Export-Steuerung, Auto-Vervollständigung), Schlagwortvorschläge, Tag-Regeln (bedingte Auto-Tags) — Phase 9 — Status: Nicht begonnen (siehe ADR-0032)
- [x] Auto-Tagging — Phase 7 — Status: Fertig (abweichend, siehe ADR-0033 Punkt 6) — regelbasierte Vorschläge aus Segmentierungs-Heuristiken (Himmel-/Personen-Flächenanteil) + EXIF-Faustregeln (ISO/Blende/Brennweite) statt echter Bildklassifikation; `apx-ai::tagging::suggest_tags` schreibt nichts selbst in den Katalog, das Frontend übernimmt Vorschläge über das bestehende `add_photo_keyword`
- [x] Metadaten-Panel (Basisfelder lesen, Bewertung/Flagge/Farbe/Schlagworte editieren) — Phase 3 — Status: Fertig
- [x] Undo/Redo für Bibliotheks-Metadaten (Bewertung/Flagge/Farbe/Schlagworte/Sammlungsmitgliedschaft) — Phase 3 — Status: Fertig (siehe DECISIONS.md ADR-0027; deckt bewusst nicht Sammlung anlegen/umbenennen/löschen ab)
- [ ] Metadaten-Presets, Stapel-Metadatenbearbeitung, EXIF/IPTC/XMP-Editor (alle Felder), frei definierbare Metadaten-Felder, Sidecar-Export (.xmp) — Phase 9 — Status: Nicht begonnen (siehe ADR-0032)
- [x] Volltextsuche (FTS5) über Dateiname, Kamera, Objektiv — Phase 3 — Status: Fertig
- [x] Rasteransicht — Phase 3 — Status: Fertig
- [x] Lupe/Einzelbildansicht (Basis-Viewer) — Phase 1 — Status: Fertig
- [ ] Vergleichsansicht, Übersichtsansicht — Phase 9 — Status: Nicht begonnen (siehe ADR-0032)
- [ ] Personenansicht (Gesichtserkennung) — Phase 9 — Status: Nicht begonnen
- [x] Filterleiste (Text, Attribut, Metadaten, kombiniert) — Phase 3 — Status: Fertig (siehe DECISIONS.md ADR-0027: Text- und Attributfilter [inkl. Kameramodell] sind jetzt per UND kombinierbar, nicht mehr alternativ wie in ADR-0026)
- [ ] Filter-Presets — Phase 9 — Status: Nicht begonnen (siehe ADR-0032)
- [x] Sortierung nach beliebigem Feld — Phase 3 — Status: Fertig (siehe ADR-0027: client-seitig, Dateiname/Aufnahmedatum/Bewertung/Dateigröße/Kameramodell, fehlende Werte immer ans Ende)
- [ ] Schnellentwicklung im Raster — Phase 9 — Status: Nicht begonnen (siehe ADR-0032)
- [ ] Vorschau-Cache-Verwaltung (Standard, 1:1), Smart Previews, Offline-Bearbeitung über Smart Previews — Phase 9 — Status: Nicht begonnen (siehe ADR-0032)
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
- [x] HSL für 8 Standardbereiche — Phase 4 — Status: Fertig (Gauß-gewichtete Farbton-Bandzuordnung statt scharfer Grenzen, siehe `crates/apx-pipeline/src/stages/hsl_color_mixer.rs`)

### Farbmischer erweitert
- [x] Frei definierbare Farbbereiche per Klick im Bild — Phase 4 — Status: Fertig (auf 8 Regionen begrenzt, siehe `hsl_color_mixer.rs`s Moduldoku)

### Farbklassifizierung / Color Grading
- [x] Farbräder Schatten/Mitteltöne/Lichter/Global — Phase 4 — Status: Fertig (Gauß-gewichtete Tonwertzonen statt fester Umschlagpunkte, siehe `crates/apx-pipeline/src/stages/color_grading.rs`)
- [x] Luminanz und Mischung pro Rad — Phase 4 — Status: Fertig
- [x] Balance, Überblendung — Phase 4 — Status: Fertig (Balance verschiebt das Gewicht zwischen Schatten-/Lichter-Zonen statt deren Zentren zu verschieben, siehe Moduldoku in `color_grading.rs`)

### Details
- [x] Schärfung (Betrag, Radius, Detail, Maskierung) — Phase 4 — Status: Fertig (abweichend, siehe ADR-0028: Unsharp-Masking mit ganzzahligem Box-Filter-Radius statt eines echten Gauß-Kerns, `sharpen_masking` über eine `smoothstep`-Schwelle statt echter Kantenerkennung, siehe `stages/details.rs`)
- [x] Luminanzrauschen (Betrag, Detail, Kontrast) — Phase 4 — Status: Fertig (abweichend: einfacher fester 3×3-Box-Weichzeichner statt eines echten bilateralen Filters)
- [x] Farbrauschen (Betrag, Detail, Glättung) — Phase 4 — Status: Fertig (abweichend: teilt sich den Luminanz-Kantenwert mit der Luminanzrauschen-Reduktion statt eines eigenen Chroma-Kantenmaßes)
- [x] Deconvolution-Schärfung — Phase 4 — Status: Fertig (abweichend, siehe ADR-0028: Potenzfunktions-Verstärkung des Hochpass-Anteils als bewusst einfacher Stand-in, kein echtes iteratives Entfaltungsverfahren)

### Objektivkorrekturen
<!-- Scope-Präzisierung siehe DECISIONS.md ADR-0028: eine echte
     Adobe-LCP-kompatible Profildatenbank ist ohne die nötigen Testdaten
     ein eigenständiges Mammutprojekt. Phase 4 liefert ein eigenes,
     minimales Profilformat (handgepflegtes JSON, wenige Beispielprofile,
     Zuordnung per EXIF-Objektiv-/Kamerastring) plus die vollen manuellen
     Regler; echter Adobe-Profil-Import wird auf eine spätere Phase
     verschoben. -->
- [x] Profilbasierte Korrektur (Datenbank + Import) — Phase 4 — Status: Fertig (abweichend, siehe ADR-0028: `crates/apx-pipeline/lens_profiles/*.json`, 3 handgepflegte Beispielprofile statt eines echten Adobe-LCP-Imports, Zuordnungsfunktion per EXIF-Objektiv-/Kamerastring implementiert und getestet, aber im Frontend noch manuell per Dropdown statt automatisch beim Fotowechsel angewendet)
- [x] Chromatische Aberration (auto + manuell) — Phase 4 — Status: Fertig (radiale Kanalverschiebung um den Bildmittelpunkt, `auto_ca` nutzt Profilwerte statt echter Kantenerkennung)
- [x] Vignettierung — Phase 4 — Status: Fertig (radiale Aufhellung zum Bildrand, additiv mit dem Profilwert kombiniert)
- [x] Verzeichnung — Phase 4 — Status: Fertig (abweichend, siehe ADR-0030: Ein-Koeffizienten-Radialmodell statt eines mehrparametrigen Brown-Conrady-Modells)
- [x] Perspektive/Upright (Auto, Level, Vertical, Full, Guided) — Phase 4 — Status: Fertig (abweichend, siehe ADR-0030: „Auto"/„Level"/„Vertical"/„Full" sind wählbare, aber wirkungslose Platzhalter — echte Kantenerkennung ist eine CV-Aufgabe außerhalb des Stacks; „Guided" mittelt die ersten zwei Hilfslinien zu einer einfachen Dreh-Korrektur statt einer echten Mehrlinien-Homografie, Hilfslinien per Zahlenfeld statt Viewer-Klick-Interaktion eingegeben)
- [x] Manuelle Transformation (V/H, Drehen, Seitenverhältnis, Skalieren, Versatz) — Phase 4 — Status: Fertig (V/H als Scherung statt echter Homografie, Ausgabegröße bleibt unverändert — Randpixel geklemmt, echtes Zuschneiden folgt in Schritt 11, siehe ADR-0030)

### Effekte
- [x] Nachträgliche Vignettierung — Phase 4 — Status: Fertig (abweichend: `roundness` blendet nur in Richtung „runder", `feather`/`midpoint` steuern eine einzelne `smoothstep`-Übergangszone statt eines mehrstufigen Verlaufs, siehe `stages/effects.rs`)
- [x] Körnung — Phase 4 — Status: Fertig (abweichend: deterministischer Ganzzahl-Hash aus der Pixelposition statt echten mehrstufigen Frequenz-Rauschens — dadurch automatisch stabil über Re-Renders, kein Flackern)

### Kalibrierung
- [x] Prozessversion — Phase 4 — Status: Fertig (abweichend, siehe ADR-0028: nur `V1` existiert, reiner Vorwärtskompatibilitäts-Platzhalter ohne aktuellen Effekt, siehe `edl/v2.rs`s Moduldoku)
- [x] Schattentönung — Phase 4 — Status: Fertig (additive Grün-/Magenta-Verschiebung, gewichtet mit einer festen Gauß-Schatten-Zone statt eines editierbaren Umschlagpunkts, siehe `stages/calibration.rs`)
- [x] Primärfarben-Regler R/G/B (Farbton, Sättigung) — Phase 4 — Status: Fertig (Gauß-gewichtete Farbton-Bänder um 0°/120°/240° statt echter Matrixrotation, siehe Moduldoku)
- [x] Kamera-Profile (kleine eingebaute Liste) inkl. DCP-Import — Phase 4 — Status: Fertig (abweichend, siehe ADR-0028: `CAMERA_PROFILES` ist eine kleine handgepflegte Liste mit festem Sättigungs-/Kontrast-Bias je Profil, kein echter DCP-/ICC-Profilwechsel; DCP-Import selbst auf spätere Phase verschoben — dasselbe Adobe-Format-Problem wie bei Objektivprofilen)

### Geometrie
- [x] Freistellen (Presets, eigene Verhältnisse, Rasterüberlagerungen) — Phase 4 — Status: Fertig (abweichend, siehe ADR-0030: „Spirale" als verschachtelte Goldener-Schnitt-Rechtecke statt echter logarithmischer Spirale, „Diagonalen" mit zwei statt vier Linien; `stages/geometry.rs` extrahiert das Zuschnitt-Rechteck pixel-genau ohne Resampling)
- [x] Winkel-Werkzeug — Phase 4 — Status: Fertig (bilinear abgetastete Drehung um den Bildmittelpunkt, Randpixel geklemmt statt schwarz gefüllt)
- [x] Auto-Ausrichtung am Horizont — Phase 4 — Status: Fertig (abweichend, siehe ADR-0028/ADR-0030: dokumentierter No-op-Platzhalter in dieser Stufe — die EXIF-Ausrichtung läuft bereits vor der EDL-Pipeline in `apx-raw`, ein weiteres automatisches Ausrichten bräuchte echte Kantenerkennung außerhalb des Stacks)

### Reparatur
<!-- Scope-Präzisierung siehe DECISIONS.md ADR-0028: Auto-Quellenfindung
     und inhaltsbasiertes Füllen für größere Bereiche sind fortgeschrittene
     Computer-Vision-Algorithmen (vergleichbar mit PatchMatch/Content-Aware
     Fill) und werden auf eine spätere Phase verschoben. Phase 4 liefert
     manuelles Klonen/Reparieren (Pinsel mit Quellpunkt, Radius, Deckkraft,
     weicher Kante). -->
- [x] Bereichsreparatur klonen/reparieren — Phase 4 — Status: Fertig (abweichend, siehe ADR-0028: Reparieren per vereinfachtem Tiefpass/Hochpass-Überblenden statt echten Poisson-Blendings, Pfad-Abstand als minimaler Stützpunkt-Abstand statt echter Punkt-zu-Liniensegment-Distanz, Striche sequenziell statt als ein Fused-Pass angewendet, siehe `stages/repair.rs`)
- [x] Auto-Quellenfindung — Phase 7 — Status: Fertig — `apx-ai::repair_analysis::suggest_source_point`, normierte Kreuzkorrelation über einen festen Kandidatenring; im Frontend per Checkbox „Quelle automatisch vorschlagen" aktivierbar, ersetzt dann den manuellen Quellpunkt-Klick
- [x] Sensorflecken-Visualisierung — Phase 7 — Status: Fertig — `apx-ai::repair_analysis::detect_spots`, Blob-Erkennung per lokaler Kontrast-Anomalie gegen ein weichgezeichnetes Referenzbild; reine Analyse (kein automatisches Committen), „Reparieren" übernimmt einen Fund als `ContentAwareFill`-Strich
- [x] Inhaltsbasiertes Füllen — Phase 7 — Status: Fertig (abweichend, siehe ADR-0033 Punkt 4) — vereinfachtes PatchMatch (Nächster-Nachbar-Vorbelegung, Zufallsinit, Propagation, Zufallssuche) statt eines vollständigen Multi-Skalen-Verfahrens; bleibt bewusst in `apx_pipeline::stages::repair` als vierter Reparatur-Modus (render-zeitlich, nicht `apx-ai`), da es bei jedem Rendering läuft statt nur einmal auf Knopfdruck

## 3.3 Modul ENTWICKELN — Lokale Anpassungen
<!-- Scope-Präzisierung siehe DECISIONS.md ADR-0032: SPEC.md §5s
     Phase-6-Satz nennt nur "Pinsel, Verläufe, Bereichsmasken,
     Maskenkombination, Ebenen-Mischmodi" — Tiefenbereich (kein
     Tiefendaten-Zulieferer existiert) und die fünf KI-Masken
     (Phase 7, dieselbe apx-ai/ONNX-Runtime-Integration wie der
     Preset-Generator) fallen aus dem Phase-6-Kern heraus. -->

- [x] Maskentyp Pinsel — Phase 6 — Status: Fertig
- [x] Maskentyp Linearer Verlauf — Phase 6 — Status: Fertig
- [x] Maskentyp Radialer Verlauf — Phase 6 — Status: Fertig — **Teil-Einschränkung:** nur ein einzelner, gemeinsamer Radius (kreisförmig), keine unabhängigen Ellipsen-Achsen/Rotation im Ziehgriff (siehe `PLAN.md` Schritt 3)
- [x] Maskentyp Farbbereich — Phase 6 — Status: Fertig
- [x] Maskentyp Luminanzbereich — Phase 6 — Status: Fertig
- [ ] Maskentyp Tiefenbereich — Später zurückgestellt — Status: Nicht begonnen (siehe ADR-0032: kein Tiefendaten-Zulieferer existiert, keinem Phasenplan-Punkt zugeordnet)
- [x] KI-Motiv-Maske — Phase 7 — Status: Fertig (abweichend, siehe ADR-0033 Punkt 1/2) — Center-Surround-Saliency-Heuristik statt echter ONNX-Modellinferenz (kein legitimer Weg, echte Segmentierungs-Modellgewichte in dieser Umgebung zu beschaffen und mitzuliefern)
- [x] KI-Himmel-Maske — Phase 7 — Status: Fertig (abweichend, siehe ADR-0033 Punkt 1/2) — Farbton-/Helligkeits-/Positions-Heuristik
- [x] KI-Hintergrund-Maske — Phase 7 — Status: Fertig (abweichend, siehe ADR-0033 Punkt 1/2) — Komplement der Motiv-Maske
- [x] KI-Objekte-Maske (Klick-Segmentierung) — Phase 7 — Status: Fertig (abweichend, siehe ADR-0033 Punkt 1/2) — farbtoleranzbasiertes Region-Growing ab einem Klickpunkt, kein gelerntes Instanzsegmentierungs-Modell
- [x] KI-Personen-Maske (Haut, Augen, Brauen, Lippen, Zähne, Haare, Kleidung) — Phase 7 — Status: Fertig, mit Teil-Einschränkung (siehe ADR-0033 Punkt 1/2) — Hautton-Erkennung im YCbCr-Raum als eine einzelne Region; Einzelteile (Augen/Brauen/Lippen/Zähne/Haare/Kleidung getrennt wählbar) bewusst nicht umgesetzt, siehe `PLAN.md`
- [x] Masken kombinieren (Hinzufügen/Subtrahieren/Schneiden) — Phase 6 — Status: Fertig — mehrere `MaskComponent`s je Maske, je mit eigenem `MaskCombine` + Invertieren (siehe `PLAN.md` Schritt 6)
- [x] Pro Maske: alle globalen Regler + Deckkraft/Weichzeichnung/Umkehren/Verfeinern — Phase 6 — Status: Fertig (abweichend, siehe ADR-0032 Punkt 2) — eingegrenzt auf die ton-/farb-/detailbezogenen Werkzeuge (Grundeinstellungen, Kurven, HSL, Farbmischer, Color Grading, Details); Objektivkorrekturen/Effekte/Kalibrierung/Geometrie/Reparatur bleiben bewusst global
- [x] Maskengruppen, Umbenennen, Ein-/Ausblenden, Überlagerungsfarbe — Phase 6 — Status: Fertig (abweichend) — Gruppen (anlegen/umbenennen/Sichtbarkeit/entfernen) vollständig; Überlagerungsfarbe (`overlay_color`, seit Schritt 1 im EDL) bekommt bewusst keine UI, da sie eine Masken-Flächen-Voransicht im Viewer steuern würde, die es (noch) nicht gibt (siehe `PLAN.md` Schritt 7)
- [x] Maske duplizieren / auf anderes Foto übertragen — Phase 6 — Status: Fertig
- [x] Ebenen-Mischmodi pro Maske — Phase 6 — Status: Fertig — alle fünf Modi (Normal/Multiplizieren/Weiches Licht/Farbe/Luminanz), CPU-only (siehe ADR-0032 Punkt 4/Schritt 11 zur GPU-Zurückstellung)
- [x] Masken als wiederverwendbare Bausteine speichern — Phase 6 — Status: Fertig (abweichend, siehe ADR-0032 Punkt 6) — bewusst nur clientseitig/session-lokal statt über die Presets-Katalog-Infrastruktur (Ordner/Versionen/SQLite) aus Phase 5; ein katalogseitiges Pendant wäre dieselbe Größenordnung an Aufwand wie das gesamte Presets-System
- [x] Maskenkette mit Drag-&-Drop-Sortierung — Phase 6 — Status: Fertig — die Reihenfolge ist zugleich die Anwendungsreihenfolge in der Pipeline, nicht nur die Anzeige

## 3.4 Modul ENTWICKELN — Workflow

- [x] Undo/Redo (unbegrenzt, dauerhaft über `edit_history`, überlebt App-Neustart) — Phase 2 — Status: Fertig — **Teil-Einschränkung:** kein sichtbares, klickbares Verlaufs-*Panel* mit benannten Schritten (nur Rückgängig/Wiederholen um je einen Schritt) — siehe `DECISIONS.md` ADR-0018s Korrektur-Notiz; die Backend-Grundlage (`list_edit_history`) für ein solches Panel existiert noch nicht und ist ein möglicher Ausbau für eine spätere Phase
- [x] Schnappschüsse — Phase 6 — Status: Fertig — eigene `snapshots`-Tabelle mit unabhängiger EDL-Kopie je Schnappschuss statt eines Verweises auf einen `edit_history`-Stand (Korrektur ggü. der ursprünglichen Plan-Formulierung, siehe `DECISIONS.md` ADR-0032 Punkt 6 Nachtrag)
- [x] Vorher/Nachher in vier Ansichten — Phase 6 — Status: Fertig — **Teil-Einschränkung:** die Trennlinie der geteilten Modi sitzt fest bei 50 % (kein ziehbarer Regler), siehe `PLAN.md` Schritt 8
- [x] Einstellungen kopieren/einfügen (granular) — Phase 6 — Status: Fertig
- [x] Vorherige übernehmen — Phase 6 — Status: Fertig
- [x] Synchronisieren über beliebig viele Bilder — Phase 6 — Status: Fertig
- [x] Auto-Sync-Modus — Phase 6 — Status: Fertig (abweichend) — überträgt bewusst immer alle EDL-Sektionen statt der für den manuellen Sync-Knopf verfügbaren granularen Auswahl (siehe `PLAN.md` Schritt 9)
- [x] Referenzansicht — Phase 6 — Status: Fertig — Referenzbild links (statisch, letzter committeter Stand eines frei wählbaren anderen Fotos) und Arbeitsbild rechts, unabhängiger Zoom/Pan je Hälfte
- [x] Soft-Proof (Zielprofil, Renderpriorität, Farbumfangswarnung, Papierweiß) — Phase 6 — Status: Fertig (abweichend, siehe ADR-0032 Punkt 6) — rein clientseitige Vorschau-Nachbearbeitung (keine Backend-/ICC-Profilverwaltung); drei simulierte Zielprofile über eine angenäherte Sättigungskompression statt echtem 3D-Gamut-Mapping

## 3.5 PRESET- UND TEMPLATE-SYSTEM
<!-- Scope-Präzisierung siehe DECISIONS.md ADR-0031: Preset-Grundlagen
     (erste 9 Zeilen unten, bedingte Presets vereinfacht auf UND-verknüpfte
     Regeln über eine feste Metadatenfeld-Liste statt einer freien
     Bedingungssprache) sind die Phase-5-Basis. Preset-Generator (KI) auf
     Phase 7 verschoben (bereits in ARCHITECTURE.md §7 so vorgesehen).
     Adobe-Interop auf eine spätere Phase verschoben (dasselbe
     Format-Problem wie beim Objektivprofil-/DCP-Import, ADR-0028).
     Templates-Unterabschnitt: nur Import-/Umbenennungs-Templates ziehen
     nach Phase 5 vor (Rust-Unterbau existiert bereits unbenutzt aus
     Phase 3, war dort fälschlich als erledigt-fällig getaggt), alle
     anderen bleiben auf der Phase ihres zugehörigen Subsystems (Export-
     Engine: Phase 8–9). -->

- [x] Presets: wählbare EDL-Teilmenge, Checkbox-Dialog beim Speichern — Phase 5 — Status: Fertig
- [x] Preset-Ordnerhierarchie, Drag & Drop, Favoriten, Suche, Tags — Phase 5 — Status: Fertig (abweichend, siehe ADR-0031: Verschieben zwischen Ordnern per Dropdown-Auswahl statt Drag & Drop — dieselbe funktionale Fähigkeit, andere Interaktion; kein separates Suchfeld, Presets werden nach Ordner gefiltert angezeigt)
- [x] Preset-Stärke 0–200 %, nachträglich änderbar — Phase 5 — Status: Fertig
- [x] Live-Vorschau (Hover im Bild + Thumbnail in der Liste) — Phase 5 — Status: Fertig
- [x] Preset-Stapel mit editierbarer Reihenfolge — Phase 5 — Status: Fertig
- [x] Bedingte Presets (Bedingungssprache im UI-Builder) — Phase 5 — Status: Fertig (abweichend vereinfacht umgesetzt, siehe ADR-0031: feste Feldliste + UND-verknüpfte Regeln statt freiem UI-Builder mit ODER/Verschachtelung)
- [x] Import/Export `.apx` — Phase 5 — Status: Fertig
- [ ] Import/Export Adobe `.xmp` / `.lrtemplate` (beide Richtungen) — Phase 8–9 — Status: Nicht begonnen (siehe ADR-0031 Punkt 3: „eine spätere Phase", ohne Phase 6 zu benennen — `ARCHITECTURE.md` §7 hatte das schon korrekt auf Phase 8–9 gelegt, diese Zeile war stehen geblieben; korrigiert im Rahmen der Phase-6-Scope-Präzisierung, ADR-0032)
- [x] Preset-Versionierung mit Diff-Ansicht — Phase 5 — Status: Fertig
- [x] Preset-Generator per LLM (natürlichsprachliche Beschreibung) — Phase 7 — Status: Fertig — echter Anthropic-Messages-API-Aufruf (`apx-ai::preset_generator::generate_from_llm`), serverseitig validiert (Antwort muss auf ein neutrales EDL gemergt vollständig deserialisierbar sein), Anthropic-API-Schlüssel vom Nutzer selbst hinterlegt (kein mitgelieferter Schlüssel)
- [x] Referenzbild-Modus (numerische Optimierung, kein LLM) — Phase 7 — Status: Fertig (abweichend, siehe ADR-0033 Punkt 4) — Koordinatenabstieg über die sechs tonwertbezogenen Grundeinstellungs-Parameter, Histogramm-Distanz (Kumulativsummen/Earth-Mover's) als Zielfunktion statt eines vollständigen Gradientenverfahrens über alle Regler
- [x] Variationen-Generator (Kontaktbogen) — Phase 7 — Status: Fertig — deterministisch geseedete kleine Störungen eines Basis-Presets, reproduzierbar über denselben Seed
- [x] Preset aus Bearbeitung lernen (Mustererkennung über mehrere Bilder) — Phase 7 — Status: Fertig (abweichend, siehe ADR-0033 Punkt 4) — arithmetisches Mittel der committeten EDL-Werte je Sektion über die ausgewählten Fotos statt echter Mustererkennung; strukturierte Listen (Kurvenpunkte, Farbmischer-Regionen) werden vom ersten Foto übernommen statt zusammengeführt
- [ ] Export-Templates (Ziel, Format, Qualität, Farbraum, Größe, Schärfung, Metadaten, Wasserzeichen, Mehrfachziel) — Phase 8 — Status: Nicht begonnen
- [ ] Wasserzeichen-Templates — Phase 8 — Status: Nicht begonnen
- [ ] Metadaten-Templates (Copyright/Ersteller/Kontakt/IPTC) — Phase 8 — Status: Nicht begonnen
- [x] Import-Templates — Phase 5 — Status: Fertig (vorgezogen aus Phase 3, siehe ADR-0031 Punkt 7 — Rust-Unterbau existierte bereits seit Phase 3 unbenutzt, Frontend-Anbindung jetzt in Schritt 9 nachgezogen)
- [x] Umbenennungs-Templates mit Token-Editor — Phase 5 — Status: Fertig (vorgezogen aus Phase 3, siehe ADR-0031 Punkt 7)
- [ ] Layout-Templates (Druck/Buch/Diashow/Web) — Phase 8 — Status: Nicht begonnen
- [ ] Workflow-Templates (Import→Filter→Preset→Export als ein Klick) — Phase 8 — Status: Nicht begonnen
- [ ] Template-Marktplatz-Struktur (lokales Repo-Format, Manifest, Installation) — Phase 8 — Status: Nicht begonnen (siehe ADR-0031: setzt die anderen Template-Bausteine voraus)

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
