# DECISIONS.md — Architecture Decision Records

Ein Eintrag pro Architekturentscheidung, im ADR-Format. Einträge werden nicht rückwirkend verändert — eine überholte Entscheidung bekommt einen neuen Eintrag mit Verweis auf den alten (`Status: Abgelöst durch ADR-00xx`).

---

**Frühere Einträge:** ADR-0001 bis ADR-0060 stehen vollständig in
`docs/DECISIONS-ARCHIV.md`. Der Index unten nennt jeden Eintrag und wo
er liegt, damit jede ADR-Nummer auffindbar bleibt.

## Index

### Archiv (`docs/DECISIONS-ARCHIV.md`)

- **ADR-0001: SQLite-Zugriff über `rusqlite` statt `sqlx`** — Archiv
- **ADR-0002: RAW-Dekodierung über `rawler` — LGPL-2.1-Hinweis** — Archiv
- **ADR-0003: Bildübertragung Backend→Frontend über Custom-Protokoll-Handler** — Archiv
- **ADR-0004: Viewer in Phase 1 auf Canvas 2D statt WebGL/WebGPU** — Archiv
- **ADR-0005: IDs als UUIDv7-Newtypes** — Archiv
- **ADR-0006: Fehlerbehandlung mit `thiserror`, keine `unwrap()` außerhalb von Tests** — Archiv
- **ADR-0007: Echte RAW-Testdateien fehlen — Netzwerkzugriff auf raw.pixls.us in dieser Sandbox blockiert** — Archiv
- **ADR-0008: Katalog-Verbindung als einzelne `Connection` hinter einem `Mutex`** — Archiv
- **ADR-0009: `apx://`-URLs als opake, über `convertFileSrc` kodierte Pfade statt echtem Query-String** — Archiv
- **ADR-0010: Playwright testet das Frontend gegen einen simulierten Tauri-Bridge, nicht die kompilierte native App** — Archiv
- **ADR-0011: Phase 2 umfasst genau sieben Grundeinstellungs-Regler, nicht zwölf** — Archiv
- **ADR-0012: Ein Crate `apx-pipeline`, nicht zwei (`apx-pipeline` + `apx-gpu`)** — Archiv
- **ADR-0013: EDL wird als JSON in einem `apx-core`-Umschlagtyp gespeichert, nicht als CBOR, nicht direkt von `apx-catalog` interpretiert** — Archiv
- **ADR-0014: Verlauf/Undo-Redo als eigene SQLite-Tabelle vollständiger EDL-Schnappschüsse** — Archiv
- **ADR-0015: `apx-raw` bekommt einen additiven `decode_linear()`-Einstiegspunkt statt den bestehenden Vertrag zu ändern** — Archiv
- **ADR-0016: Die interaktive Entwickeln-Route liefert rohe RGBA8-Bytes statt PNG/JPEG** — Archiv
- **ADR-0017: Fusionierter Compute-Shader für den interaktiven Pfad, separate Shader pro Regler für Spec-Konformität und Tests** — Archiv
- **ADR-0018: Undo/Redo im Frontend direkt über `edit_history`, kein `zundo` (revidiert in Schritt 6)** — Archiv
- **ADR-0019: Feste Kamera→sRGB-Matrix + Gammakurve im Entwickeln-Renderpfad, wiederverwendet statt dupliziert** — Archiv
- **ADR-0020: Entwickeln-Modul als zuschaltbares Overlay im bestehenden Viewer, WebGL2 für beide Pixelquellen** — Archiv
- **ADR-0021: wgpu-GPU-Ausführungstests in CI empirisch bestätigt (Schritt 8) — kein "weich" mehr auf Dauer** — Archiv
- **ADR-0022: Phase 3 umfasst `SPEC.md` §5s Phase-3-Satz, nicht §3.1s vollen BIBLIOTHEK-Katalog** — Archiv
- **ADR-0023: Bewertung/Flagge/Farbe als Spalten, Schlagworte flach, Sammlungen nur manuell, FTS5 als External-Content-Tabelle** — Archiv
- **ADR-0024: Rasteransicht und Filmstreifen teilen sich Fotoliste + Auswahl-State** — Archiv
- **ADR-0025: Import-Kopieren/Verschieben additiv zum bestehenden Hinzufügen, DNG-Konvertierung verschoben** — Archiv
- **ADR-0026: Suche/Filter alternativ statt kombiniert; zwei Über-Scope-Punkte, die ADR-0022 übersehen hatte, nachträglich korrigiert** — Archiv
- **ADR-0027: Fünf im Abschlussbericht ehrlich benannte Lücken nachträglich in Phase 3 geschlossen** — Archiv
- **ADR-0028: Phase-4-Scope präzisiert — Workflow-Punkte verschoben, Objektivprofile und Reparatur bewusst vereinfacht** — Archiv
- **ADR-0029: Schritt 2 baut nur die Grundeinstellungs-Regler + die GPU-Dispatch-Grundlage; Kurven/HSL/Farbmischer/Color-Grading/Kalibrierung/Effekte bekommen ihren eigenen Shader in ihrem eigenen Schritt** — Archiv
- **ADR-0030: Objektivkorrekturen (Schritt 9) — Ausgabegröße bleibt unverändert, Perspektive/Upright „Auto"/„Level"/„Vertical"/„Full" bleiben No-op-Platzhalter** — Archiv
- **ADR-0031: Phase-5-Scope präzisiert — Preset-Grundlagen jetzt, KI-Generator/Adobe-Interop/Templates-Unterabschnitt verschoben; kein eigenes `apx-presets`-Crate** — Archiv
- **ADR-0032: Phase-6-Scope präzisiert — Maskensystem (§5) plus die in ADR-0028 versprochenen Workflow-Punkte; Bibliotheks-Backlog explizit auf Phase 9 verschoben; Tiefenbereich/KI-Masken zurückgestellt** — Archiv
- **ADR-0033: Phase-7-Scope präzisiert — KI-Funktionen ohne echte ONNX-Runtime-Modellinferenz, echter LLM-Client für den Preset-Generator, neues `apx-ai`-Crate** — Archiv
- **ADR-0034: Phase-8-Scope präzisiert — Export-Engine als gemeinsamer Unterbau, reale Zusatzformate/ICC-Farbmanagement/PDF/SFTP wo machbar, HEIF-/JPEG-XL-Export und Kartenkacheln bewusst zurückgestellt** — Archiv
- **ADR-0035: Phase-9-Scope präzisiert — drei angesammelte Rückstände zusammengeführt, elf „bewusste Vereinfachung"-Grenzen für die technisch schwierigsten Punkte festgelegt, Tiefenbereich-Masken bleiben ausgenommen** — Archiv
- **ADR-0036: Phase-9-Abnahme (Schritt 12) — Stapelverarbeitungs-Konsole war in ADR-0035s Scope-Aufzählung, bekam aber nie einen Bauschritt; bleibt explizit zurückgestellt statt nachträglich überstürzt angeflanscht** — Archiv
- **ADR-0037: Phase-10-Scope — drei Phase-3/5-UI-Restposten reingezogen, Testdisziplin für diese Phase nutzerangeordnet gelockert, ehrliche Grenzen bei Installer-Signierung** — Archiv
- **ADR-0038: Phase 11 — Nachträge zu allen zurückgestellten Punkten aus Phase 1–10; vier neu verfügbare Crates real geprüft (drei brauchbar, eine als Fassade entlarvt), `libgphoto2` ist entgegen ADR-0035 tatsächlich per `apt` verfügbar** — Archiv
- **ADR-0039: Phase 12 — Lightroom-Lückenschluss; `lensfun`-Crate real geprüft und für Schritt 3 freigegeben** — Archiv
- **ADR-0039-Nachtrag: Schritt 3 real umgesetzt — echte Ecken-Rückrechnung statt Koeffizienten-Übernahme, vereinfachter Kalibrier-Assistent statt vollem Zhang-Verfahren** — Archiv
- **ADR-0039-Nachtrag II: Schritt 6 — echter ICC-Soft-Proof über `lcms2::Transform::new_proofing` ersetzt die JS-Näherung** — Archiv
- **ADR-0039-Nachtrag III: Schritt 7 — Beobachteter Ordner / Auto-Import genau wie geplant umgesetzt** — Archiv
- **ADR-0040: Phase 13 — echte ONNX-Laufzeit jetzt real verfügbar (korrigiert ADR-0033 Punkt 1); KI-Ausfüllen, Direktimport, DCP-Profile, klassische CV-Lücken** — Archiv
- **ADR-0040-Nachtrag: Schritt 3 — echter Adobe-DCP-Import; `ColorMatrix`** — Archiv
- **ADR-0040-Nachtrag II: Schritt 4 — echte Perspektive/Upright-** — Archiv
- **ADR-0040-Nachtrag III: Schritt 5 — Panorama-Homografie-Stitching;** — Archiv
- **ADR-0040-Nachtrag IV: Schritt 6 — Mehrere Kataloge + Katalog-Wartung;** — Archiv
- **ADR-0040-Nachtrag V: Schritt 7 — echter UND/ODER-Regelbaum für** — Archiv
- **ADR-0040-Nachtrag VI: Schritt 8 — echte Personen-Wiedererkennung;** — Archiv
- **ADR-0041: Phase 14 — zehn Alleinstellungsmerkmale jenseits von** — Archiv
- **ADR-0042: Phase 15 — fünf Photoshop-Funktionen, die es in Lightroom** — Archiv
- **ADR-0043: Phase 16 — Filter-/LUT-Bibliothek + Video als Katalog-Asset** — Archiv
- **ADR-0043-Nachtrag (Schritt 11): Dokumentation, volle Verifikation, Abnahme** — Archiv
- **ADR-0044: Foto-Globus + Dichte-Heatmap (Erweiterung der Kartenansicht)** — Archiv
- **ADR-0045: Phase 17 — Video-Editor-Erweiterung (Timeline, Text/Untertitel, Tempo, Greenscreen, Bild-in-Bild, Stabilisierung)** — Archiv
- **ADR-0046: Phase 18 — UI/UX-Overhaul (Design-System, Navigation, Panels, Bewegung)** — Archiv
- **ADR-0046-Nachtrag: Drei reale Nutzungsfehler nach dem Phase-18-Merge** — Archiv
- **ADR-0046-Nachtrag III: Entwickeln-Panel einklappbare Abschnitte + Filter-Test-Lücke** — Archiv
- **ADR-0047: Phase 19 — Animationen + UI-Sounds (Start/Beenden, Klick,** — Archiv
- **ADR-0047-Nachtrag: KI-Verarbeitung + Drag&Drop — die beiden zuvor** — Archiv
- **ADR-0048: Phase 20 — Qualitätsoffensive nach Phase 19: reale** — Archiv
- **ADR-0048-Nachtrag: Echte GSAP-Bewegung + app-weites Knopf-Feedback** — Archiv
- **ADR-0049: Phase 21 — Liquid-Glass-Überarbeitung, Spotlight-** — Archiv
- **ADR-0050: Phase 22 — ESC-Schnellmenü, schnelleres Masken-Ziehen,** — Archiv
- **ADR-0051: Phase 23 — echte Bewegung, Runde 2: drei externe** — Archiv
- **ADR-0051-Nachtrag: "noch nicht gut genug" — mehr Hover, Animation** — Archiv
- **ADR-0052: Phase 24 — Mehr Transparenz, echte Karten-Bugfixes, zehn** — Archiv
- **ADR-0053: Phase 25 Schritt 1 — Bugfix: Filter/`.cube`-LUTs verändern** — Archiv
- **ADR-0054: Phase 25 — 5-Schritt-Premium-UI-Plan (Typografie,** — Archiv
- **ADR-0055: Phase 25 Nachtrag — echtes "Liquid Glass" (Bugfix:** — Archiv
- **ADR-0055-Nachtrag: Werte deutlich verstärkt + strukturelle Grenze** — Archiv
- **ADR-0056: Phase 26 — drei Ansichts-Funktionen, die erst durch die schwebende Kopfzeile möglich wurden** — Archiv
- **ADR-0057: Phase 27 — zehn Kreativ-Werkzeuge mit großem Bildeffekt + Retro-Fuji-Thailand-Filter** — Archiv
- **ADR-0058: Phase 28 — „Licht & Optik": zwölf Werkzeuge, echtes Bokeh, drei neue .cube-Filter** — Archiv
- **ADR-0059: Phase 29 — Kreativ- und Licht-&-Optik-Werkzeuge preset-fähig machen** — Archiv
- **ADR-0060: Phase 30 — „Direkt am Bild": zehn Werkzeuge mit eigenen Bedienelementen** — Archiv

### In dieser Datei

- **ADR-0061: Installer und Signierung fertiggestellt — und die zwei Punkte, die eine Entscheidung brauchen** — unten
- **ADR-0062: Phase 17 Schritt 9 — Video-Stabilisierung** — unten
- **ADR-0063: Aperture X steht unter Apache-2.0 — und eine Abhängigkeit steht es nicht** — unten
- **ADR-0064: Phase 31 — Layout, Regler, und wie oft „fehlt" in Wahrheit „findet niemand"** — unten
- **ADR-0065: Phase 32 — zehn Funktionen, die etwas verändern** — unten
- **ADR-0066: Phase 33 — zehn Funktionen, die Lücken schließen** — unten
- **ADR-0067: `beforeDevCommand`/`beforeBuildCommand` verzeichnisunabhängig** — unten
- **ADR-0068: Aufnahmedatum für Nicht-RAW-Dateien, und eine Fehlergrenze** — unten
- **ADR-0069: EDL-Altstände auffüllen statt blind casten** — unten
- **ADR-0070: Phase 34 — zehn Funktionen, und was das Bauen über den Bestand verriet** — unten
- **ADR-0071: Geburtstagsausgabe — eine personalisierte Fassung, und zwei Altfehler, die sie aufgedeckt hat** — unten
- **ADR-0072: Die Kaskaden-Falle an der Ursache behoben — und 47 Menüeinträge einzeln angeklickt** — unten
- **ADR-0073: Warum die App zäh war — gemessen, nicht geraten** — unten
- **ADR-0074: Die GPU-Beschleunigung hat nie funktioniert** — unten
- **ADR-0075: Jeder Stufenaufruf baute die halbe GPU neu auf** — unten
- **ADR-0076: Die Effekt-Automatik lief nie — und die Vorgabe war die teuerste Stufe** — unten
- **ADR-0077: Beim Reglerziehen rechneten dutzende überholte Läufe zu Ende** — unten
- **ADR-0078: Die Luftballons waren so gut wie nie zu sehen** — unten
- **ADR-0079: `pnpm install` scheiterte auf jedem frischen Rechner** — unten
- **ADR-0080: Die Schranke aus ADR-0077 hat es verdoppelt** — unten
- **ADR-0081 — Die Luftballons sind ersatzlos entfallen** — unten
- **ADR-0082: Token-Audit — verbindliche Arbeitsregeln, Projekt-Einstellungen, geteilte Entscheidungsdatei** — unten
- **ADR-0083: Der Token-Audit laeuft automatisch beim Kompaktieren** — unten
- **ADR-0084: Einstellungen gehoeren dorthin, wo sie ohne Kopieren wirken** — unten

---

## ADR-0061: Installer und Signierung fertiggestellt — und die zwei Punkte, die eine Entscheidung brauchen

`PLAN.md` Phase 10 Schritt 11 („Installer + Signierung, alle drei
Plattformen, strukturell + konditional") war als einziger Punkt der
Phase 10 nie abgehakt. Bei der Durchsicht zeigte sich: der
CI-Release-Job aus ADR-0037 existierte bereits samt konditionaler
Signierungsschritte — was fehlte, waren die Teile davor und danach.

### Was tatsächlich gefehlt hat

1. **Paketmetadaten.** `tauri.conf.json` enthielt nur `active`,
   `targets` und Icons. Hersteller, Copyright, Kategorie, Kurz- und
   Langbeschreibung, Startseite: alles leer. Ein so gebautes Paket
   zeigt in der Windows-Dateiinfo und in Linux-Paketverwaltungen leere
   Felder — das sieht nach unfertiger Software aus, unabhängig davon,
   wie fertig sie ist.
2. **`ffmpeg` war nirgends als Abhängigkeit deklariert.** Die
   Video-Funktionen rufen es über `std::process::Command::new("ffmpeg")`
   als externes Programm auf (fünf Stellen in `commands.rs`). Auf einem
   frischen System hätte die Anwendung sauber gestartet und beim ersten
   Videoexport versagt. Jetzt als `recommends` im deb-Paket, bewusst
   nicht als `depends`: die Foto-Seite arbeitet vollständig ohne, und
   ein reiner Foto-Nutzer soll nicht zur ffmpeg-Installation gezwungen
   werden. Es mitzuliefern (`bundle.externalBin`) wäre robuster, wirft
   aber je nach ffmpeg-Build eine eigene GPL-Frage auf — deshalb nicht
   gemacht, sondern in `RELEASE.md` benannt.
3. **Eine stille Versions-Divergenz.** `tauri.conf.json` stand fest auf
   `"version": "0.1.0"`, die Crate auf `version.workspace`. Beim
   nächsten Versionssprung hätte der Installer weiter 0.1.0 gemeldet.
   Das Feld ist jetzt entfernt — Tauri nimmt dann die Version aus
   `Cargo.toml` — und ein Test verhindert, dass es zurückkommt.
4. **`targets: "all"` statt einer geprüften Liste.** „all" schließt
   `rpm` und `msi` ein, deren Werkzeugbedarf auf den CI-Runnern hier
   nicht nachweisbar war. Jetzt eine ausdrückliche Liste
   (`deb`, `appimage`, `app`, `dmg`, `nsis`), die je Plattform
   mindestens ein Ziel übrig lässt; ein Test hält das fest.
5. **Der Release-Job konnte lautlos nichts liefern.**
   `if-no-files-found: warn` ließ ihn grün durchlaufen, auch wenn
   `tauri build` kein einziges Paket erzeugt hatte. Jetzt bricht ein
   eigener Prüfschritt mit `::error::` ab, bevor irgendetwas
   hochgeladen wird.
6. **Es gab keine Veröffentlichung.** Die Pakete landeten nur in den
   Workflow-Artefakten — 14 Tage Aufbewahrung, für Nutzer ohne
   Actions-Zugriff überhaupt nicht erreichbar. Damit war die
   „Distribution" aus Phase 10 faktisch nicht vorhanden. Jetzt hängt
   ein Tag-Push die Pakete samt SHA-256-Summen an eine
   **Entwurfs**-Release.

### Prüfsummen sind hier keine Zierde

Bei unsignierten Paketen — und das ist ohne hinterlegte Zertifikate der
Normalfall — sind die `SHA256SUMS-*.txt` die **einzige** verfügbare
Integritätsaussage. Deshalb entstehen sie im selben Job, aus denselben
Dateien, die hochgeladen werden.

### Zwei Shell-Fallen, vorher gefunden statt beim Tag-Push

`mapfile` gibt es erst ab Bash 4; die macOS-Runner haben die
vorinstallierte Bash 3.2, der Prüfschritt wäre dort mit „command not
found" abgebrochen. Und `sha256sum` existiert auf Linux und in der
Git-Bash der Windows-Runner, auf macOS aber nicht (dort `shasum`).
Beides ist ersetzt; die Skripte sind mit `bash -n` syntaktisch geprüft.

### Was NICHT nachgewiesen ist

Unverändert gegenüber ADR-0037 Entscheidung 3: die Signierungsschritte
sind **nie mit einem echten Zertifikat ausgeführt** worden — weder ein
Apple-Developer-Konto noch ein Windows-Codesigning-Zertifikat war
beschaffbar. Sie lesen die dokumentierten Umgebungsvariablen und
überspringen sich selbst, wenn die Secrets fehlen. Ob sie mit einem
echten Zertifikat durchlaufen, bleibt offen. `RELEASE.md` sagt das in
der ersten Tabelle, nicht im Kleingedruckten, und empfiehlt einen
Testtag vor dem ersten echten Release.

Neu dazugekommen ist eine Einschränkung, die ADR-0037 noch nicht kannte:
seit Juni 2023 verlangen alle Zertifizierungsstellen Hardware-
Schlüsselspeicher für Windows-Codesigning. Der vorhandene
PFX-Import-Schritt funktioniert deshalb nur mit älteren oder intern
ausgestellten Zertifikaten; für ein HSM-gebundenes Zertifikat wäre
`bundle.windows.signCommand` der Weg — nicht eingerichtet, weil sich
ohne konkretes Zertifikat nicht sagen lässt, wie der Aufruf aussieht.

### Zwei Punkte, die bewusst offen bleiben

Beides sind keine Programmierfragen:

1. **Es gibt keine Lizenzdatei.** `bundle.license`/`licenseFile` sind
   deshalb nicht gesetzt — eine eingetragene Lizenz ohne Entscheidung
   dahinter wäre eine Behauptung. `THIRD_PARTY.md` wird trotzdem
   mitgeliefert (`bundle.resources`): die Drittanbieter-Hinweise
   müssen beim Nutzer ankommen, unabhängig davon, wie das Gesamtwerk
   lizenziert wird.
**Nachtrag beim Schreiben dieses ADRs gefunden:** `README.md` bezeichnet
Aperture X bereits als „fully open sourced". Das ist eine Aussage des
Projektinhabers und verschiebt die Lage deutlich — LGPL §6 verlangt die
Austauschbarkeit der Komponente nur bei **geschlossener** Weitergabe.
Punkt 2 unten wäre damit weitgehend erledigt. Was fehlt, ist der
formale Teil: **es gibt keine `LICENSE`-Datei.** Ohne sie gilt
urheberrechtlich „alle Rechte vorbehalten", egal was im README steht —
niemand darf den Code rechtssicher weitergeben oder ändern. Eine
Lizenz auszusuchen ist eine Eigentümerentscheidung und wird hier nicht
getroffen; sie zu benennen ist der eine offene Schritt, der aus
„open source gemeint" auch „open source wirksam" macht.

2. **ADR-0002 Punkt 2 ist weiterhin offen.** Dort steht, dass `apx-raw`
   „ab Phase 10 / Installer" als dynamisch nachladbare Komponente
   gebaut werden soll, damit die LGPL-2.1-Komponente `rawler` gemäß
   LGPL §6 austauschbar bleibt. Das ist nur für eine **geschlossene**
   Weitergabe zwingend und hängt vollständig an Punkt 1 — ADR-0002 hält
   ausdrücklich fest, dass diese Weichenstellung nicht einseitig
   getroffen wird. Ein spürbarer architektonischer Umbau auf Verdacht
   wäre hier die falsche Reihenfolge.

Verifiziert: neun neue Integrationstests in
`crates/apx-app/tests/bundle_config.rs`, die die Konfiguration gegen die
echten Dateien prüfen (jedes referenzierte Icon existiert, je Plattform
ein passendes Format, die Ressourcendatei existiert und nennt die
LGPL-Komponente, keine gepinnte Version, alle Anzeige-Metadaten gesetzt,
nur geprüfte Ziele, gültiger Bezeichner, und die CI baut die Installer
tatsächlich noch). `cargo fmt`/`clippy --workspace --all-targets` ohne
Warnung, `cargo test --workspace` grün. Die YAML ist geparst und die
beiden neuen Shell-Schritte mit `bash -n` geprüft.

---

## ADR-0062: Phase 17 Schritt 9 — Video-Stabilisierung

**Status:** angenommen (Phase 17 Schritt 9)

### Kontext

Der letzte offene Funktionspunkt von Phase 17. Der Plan von damals sah
ausdrücklich „Wiederverwendung `apx-stacking`-Homografie" vor: die
merkmalsbasierte Bild-zu-Bild-Messung, die seit Phase 13 Schritt 5 das
Panorama-Stitching trägt (`apx_stacking::homography_stitch`, AKAZE +
eigener RANSAC-Loop), misst genau das, was eine Stabilisierung braucht —
wie sich die Kamera zwischen zwei Aufnahmen bewegt hat. Der Punkt war
auf ausdrücklichen Nutzerwunsch pausiert und ist jetzt freigegeben
worden.

### Entscheidungen

**1. Zwei Durchgänge, nicht einer.** Eine Kamerabahn lässt sich nicht
glätten, solange man ihre Zukunft nicht kennt. Ein einzelner Durchlauf
könnte Bewegung nur *dämpfen* — und würde damit auch jeden gewollten
Schwenk verschleppen. Also: erst das ganze Video messen, dann die Bahn
glätten, dann in einem zweiten Durchlauf korrigieren. Der Preis ist,
dass das Video zweimal dekodiert wird; abgefedert wird er dadurch, dass
der Messdurchgang auf 480 px längste Kante herunterskaliert läuft (siehe
Punkt 3).

**2. Vier Freiheitsgrade statt acht.** Die Messung liefert eine volle
8-Freiheitsgrad-Homografie. Die direkt auf eine wackelige
Freihandaufnahme anzuwenden erzeugt den berüchtigten „Wackelpudding":
perspektivische Anteile, die aus Rauschen in den Merkmalspaaren stammen,
lassen Bildkanten schwabbeln. `Similarity::from_homography` projiziert
deshalb auf Verschiebung, Drehung und Maßstab — dieselbe Beschränkung,
die übliche Stabilisierer vornehmen. Maßstab und Winkel kommen aus dem
linken oberen 2×2-Block, gemittelt über beide Diagonalen statt aus einem
herausgegriffenen Eintrag, weil eine verrauschte Messung dort nur
näherungsweise die Form `s·[[cos, −sin], [sin, cos]]` hat.

**3. Gemessen wird verkleinert, korrigiert in voller Auflösung.** Das
Wackeln einer Freihandaufnahme steckt in groben Bildstrukturen, nicht im
Pixelrauschen; in voller Auflösung zu messen kostet ein Vielfaches, ohne
die Bahn genauer zu machen. Der Rückrechnungsfaktor kommt aus der
*tatsächlich* entstandenen Messbreite, nicht aus dem ungerundeten
Wunschfaktor — `ffmpeg`s `scale` rundet auf gerade Kanten, und ein
Bruchteil Abweichung je Bild summiert sich über die Bahn auf. Genau das
prüft `the_scale_back_factor_comes_from_the_rounded_width`.

**4. Korrekturen werden auf den Zuschnitt-Rand begrenzt, nicht
gehofft.** Jede Korrektur schiebt das Bild und legt am Rand leere Fläche
frei; dagegen hilft nur Hineinzoomen. Statt darauf zu vertrauen, dass
der gewählte Zoom reicht, *klemmt* `stabilize_path` jede Korrektur auf
das, was der Rand hergibt. Lieber eine leicht verbleibende Restbewegung
als ein schwarzer Rand, der im fertigen Video nicht mehr zu reparieren
ist. Die Konsequenz ist bewusst und im UI benannt: bei `crop_zoom = 1.0`
gibt es keinen Rand, also auch keine Korrektur — der Video-Modus sagt
das als Hinweis, statt den Nutzer einen wirkungslosen Lauf starten zu
lassen.

**5. Eine nicht messbare Stelle heißt „keine Bewegung", nicht
„geschätzte Bewegung".** Findet die Merkmalssuche zwischen zwei Bildern
nichts Verlässliches (zu wenig Struktur, zu starke Bewegungsunschärfe),
liefert sie `None`. Daraus einen Sprung zu raten wäre im fertigen Video
als Ruck sichtbar und damit schlimmer als das ausgelassene Bild.

**6. Die Mathematik liegt in `apx-stacking`, nicht in `apx-app`.** Die
gesamte Stabilisierung ist in `crates/apx-stacking/src/stabilize.rs`
gekapselt: Zahlen rein, Zahlen raus, ohne `ffmpeg` und ohne Videodatei —
sonst wäre sie nur über einen echten Videolauf prüfbar gewesen.
`apx-app`s `stabilize_video` macht ausschließlich das Drumherum
(dekodieren, messen, neu kodieren), nach genau demselben
zwei-`ffmpeg`-Prozesse-Muster wie `apply_lut_filter_to_video` und
`remove_video_background`, inklusive `-map 1:a?` für die unverändert
durchgereichte Tonspur.

### Verifikation

Sechzehn Unit-Tests in `apx_stacking::stabilize` — darunter, dass ein
gleichmäßiger Schwenk unangetastet bleibt, dass Zittern *auf* einem
Schwenk verschwindet während der Schwenk überlebt, dass ohne
Zuschnitt-Rand nichts korrigiert wird, und dass ein wackeliges Bild nach
der Stabilisierung dort landet, wo die ruhigen Bilder landen (verglichen
gegen ein ruhiges Bild durch dieselbe Kette, nicht gegen einen fest
eingetippten Zielwert). Vier weitere in `apx-app` für die
Messauflösungs-Rechnung. Dazu der erste e2e-Test überhaupt, der das
Video-Modul betritt (`video-stabilize-flow.spec.ts`).

### Bekannte Grenze

Der e2e-Test prüft die Kette *vor* der Stabilisierung — Reglerwerte,
Command-Argumente, Umschalten auf das Ergebnis, der Hinweis bei
wirkungslosem Zuschnitt. Die Stabilisierung selbst braucht `ffmpeg` und
läuft komplett in Rust; sie ist dort geprüft, nicht im Browser. Das ist
dieselbe Grenze, die ADR-0010 für alle Playwright-Tests dieses Projekts
festhält.

---

---

## ADR-0063: Aperture X steht unter Apache-2.0 — und eine Abhängigkeit steht es nicht

**Status:** angenommen (Nutzerentscheidung, auf Vorlage von drei
maschinell geprüften Befunden)

### Kontext

`README.md` nannte das Projekt seit Phase 1 „fully open sourced", es gab
aber keine `LICENSE`-Datei. Urheberrechtlich heißt das: alle Rechte
vorbehalten. Der Satz im README erlaubte niemandem, den Code
weiterzugeben oder zu ändern. RELEASE.md führte das seit ADR-0061 als
den einen offenen Punkt, der aus „open source gemeint" auch „open
source wirksam" macht.

### Was die Prüfung ergab — und warum sie nötig war

Statt eine Lizenz zu wählen und die Abhängigkeiten für unkritisch zu
halten, sind beide Bäume einmal wirklich gelesen worden (`Cargo.lock`
gegen die Manifeste der entpackten Registry, `pnpm`-Speicher gegen die
`package.json`). Drei Befunde, von denen keiner aus der handgepflegten
`THIRD_PARTY.md` hervorging:

**1. `Cargo.toml` behauptete seit jeher `license = "MIT"`.** Eine
Angabe ohne Datei dahinter — sie erschien in jedem
`cargo metadata`-Aufruf und in jedem daraus erzeugten
Abhängigkeitsbericht, ohne dass sich jemand darauf hätte berufen
können. Sie ist jetzt korrigiert, nicht gelöscht.

**2. `lensfun` steht unter LGPL-3.0-or-later, nicht LGPL-2.1.** Das
schließt **GPL-2.0-only als Projektlizenz aus** — die beiden sind
unvereinbar. Die Wahl war also von Anfang an enger, als sie aussah.
`lensfun` ist zudem keine optionale Abhängigkeit wie `gphoto2`, sondern
fest in `apx-pipeline`.

**3. GSAP ist nicht quelloffen.** Die Animationsbibliothek (seit Phase
19, acht Dateien) steht unter GreenSocks eigener Standard-„no
charge"-Lizenz: „Copyright (c) 2008-2026, GreenSock. All rights
reserved." Kostenlos nutzbar und weitergebbar, auch kommerziell — aber
keine OSI-anerkannte Open-Source-Lizenz, und nicht unter Apache-2.0
unterlizenzierbar. GSAP wird ins Anwendungsbündel kompiliert, ist also
Teil jeder Weitergabe. `THIRD_PARTY.md`s bisheriger Eintrag beschrieb
die Lizenz als „100 % kostenlos … kommerzielle Nutzung ausdrücklich
erlaubt" — alles zutreffend, aber die entscheidende Eigenschaft fehlte.

### Entscheidungen

**1. Apache-2.0 für Aperture X' eigenen Code.** Gegenüber MIT die
ausdrückliche Patentlizenz (§3) — eine Bildverarbeitungsanwendung ist
die Art Software, bei der Patentfragen auftauchen können, und MIT
schweigt dazu. Verträglich mit allen vier LGPL-Abhängigkeiten. Der
Lizenztext ist nicht abgetippt, sondern aus drei unabhängigen lokalen
Kopien übernommen, die bis auf den SHA-256 übereinstimmen.

**2. GSAP bleibt und wird ausdrücklich ausgewiesen.** Die Alternative
wäre gewesen, es gegen eine quelloffene Bibliothek zu tauschen — ein
eigener Arbeitsblock über acht Dateien plus erneute visuelle Prüfung
der Phase-19/20-Animationen. Dagegen sprach nichts Rechtliches: GSAPs
Lizenz erlaubt genau diese Weitergabe. Dafür sprach, dass ein
stillschweigendes Mitlaufen unter der eigenen Lizenz die schlechtere
Variante gewesen wäre. Also: `NOTICE`, `THIRD_PARTY.md` und `README.md`
benennen es, und der README-Satz „fully open sourced" ist entsprechend
präzisiert — ein Fork muss wissen, dass er für diese eine Komponente an
GreenSocks Bedingungen gebunden ist.

**3. ADR-0002 Punkt 2 erledigt sich.** Weil Aperture X quelloffen mit
vollständigem Quelltext weitergegeben wird, ist die Austauschbarkeit,
die LGPL-2.1 §6 bzw. LGPL-3.0 §4 verlangt, durch diese Weitergabe schon
gegeben. `apx-raw` muss **nicht** als dynamisch nachladbare Bibliothek
gebaut werden. Für eine geschlossene Weitergabe wäre der Punkt wieder
offen.

**4. Rechteinhaber: „Aperture X contributors".** Kein Klarname im
Repository, und neue Beitragende sind ohne weitere Änderung abgedeckt.

### Was diese Entscheidung dauerhaft hält

Eine handgepflegte `THIRD_PARTY.md` hat genau den Fehler erzeugt, der
oben unter Punkt 3 steht. Deshalb zusätzlich:

- `tools/license-audit.py` liest die echten Manifeste und schreibt
  `licenses/rust.tsv` (952 Einträge) und `licenses/npm.tsv` (170).
  Plattformgebundene Crates, die auf Linux nie geholt werden
  (`winreg`, `system-configuration`, `wasi`), sind dafür eigens über
  `cargo fetch --target …` nachgeladen worden — sonst hätte die
  Momentaufnahme vier ungeprüfte Löcher gehabt und der Test hätte in
  falscher Sicherheit gewiegt.
- Zwei Tests in `crates/apx-app/tests/bundle_config.rs`: einer verlangt
  vollständige Abdeckung von `Cargo.lock` durch die Momentaufnahme, der
  andere prüft jede erfasste Lizenz gegen eine Liste unbedenklicher plus
  die fünf benannten Ausnahmen.

**Zwei eigene Fehler, die diese Tests beim ersten Lauf gefunden haben**
— beide festgehalten, weil sie die Art Fehler sind, die sonst als
grüner Test durchgehen:

- Der erste Entwurf des Wächters verglich Crate-**Namen** gegen eine
  Sperrliste (`"libgphoto2" | "gtk" | …`). Das sah aus wie eine Prüfung,
  war aber keine: ein Crate-Name sagt nichts über seine Lizenz, und die
  Liste hätte auf crates.io praktisch nie angeschlagen. Ersetzt durch
  den Weg über die Momentaufnahme.
- Die SPDX-Auswertung behandelte `AND` wie `OR`. Das ist genau in die
  falsche Richtung falsch: `Unbedenklich AND GPL` hätte bestanden.
  Aufgefallen an `brotli` (`BSD-3-Clause AND MIT`), das der Test
  zurecht bemängelte, und danach richtig implementiert — `OR` braucht
  einen unbedenklichen Zweig, `AND` alle.

### Ergebnis der Prüfung

Außer den vier LGPL-Crates (`rawler`, `lensfun`, `gphoto2`,
`libgphoto2_sys`) und `gsap` trägt keine der 1.122 Abhängigkeiten eine
Auflage, die über Namensnennung hinausgeht. Kein GPL ohne „L"
irgendwo im Baum — weder als Pflicht noch als Wahlmöglichkeit.

---

---

## ADR-0064: Phase 31 — Layout, Regler, und wie oft „fehlt" in Wahrheit „findet niemand"

**Status:** angenommen (Phase 31)

### Kontext

Nutzerurteil: „besseres Design, mehr Funktionen". Statt zu raten, wurde
die App zuerst wirklich angesehen — Playwright-Screenshot des
Entwickeln-Modus bei 1680×1000.

### Was der Blick zeigte

1. **Fünf Spalten, und das Foto ist die schmalste.** Ordner, Presets,
   Bild, Entwickeln, Masken teilen sich die Breite; auf das Foto
   entfielen rund 370 px.
2. **Das Analyse-Panel lag ÜBER dem Foto.** Verschieben und Einklappen
   gab es seit Phase 18 — aber der Zustand beim Öffnen war „Panel
   verdeckt das Bild".
3. **Der KI-Preset-Generator füllte die Preset-Palette** mit acht
   Bedienelementen, direkt unter dem Satz „Keine Presets in diesem
   Ordner". Das Seltene nahm den Platz, das Wichtige stand als
   Leermeldung darüber.
4. **Regler brauchten zwei Zeilen à 42 px.** Bei rund vierzig Reglern
   ist das der Grund, warum man schon für die Grundeinstellungen
   scrollen muss.

### Entscheidungen

**1. Die Analyse dockt an, statt zu schweben** — als `PaletteFrame`, mit
Breite-Ziehen, Einklappen und Persistenz wie jede andere Palette. Der
Viewer misst sich an seinem eigenen `<main>`, deshalb steht die Analyse
als Geschwister daneben: die Restbreite ist dann automatisch die, mit
der das Bild rechnet, ohne eine Zeile Einpass-Mathematik anzufassen.

**2. Ein Regler, eine Zeile.** Beschriftung | Regler | Zahl
nebeneinander, rund 22 px. Am Screenshot nachgemessen: die dreizehn
Regler von Weißabgleich und Grundeinstellungen brauchen 312 px statt
546 px — der Kurven-Editor ist dadurch ohne Scrollen sichtbar. Die
Beschriftung wird abgeschnitten statt umzubrechen; ein Umbruch machte
die Zeile wieder hoch und die Änderung damit zunichte.

**3. Sichthilfen gehören nicht ins EDL.** Fokus-Peaking und Farbpalette
rechnen im Frontend auf dem fertig entwickelten Vorschaubild, nach dem
Muster des Clipping-Overlays. Im EDL würden sie exportiert und in
Presets weitergereicht — beides will niemand.

**4. Erweiterungen am Bild sind rückwärtskompatibel zu BELEGEN, nicht zu
behaupten.** Das mitteltonbetonte Korn kam als neues Feld mit
Neutralwert 0 dazu, und ein Test vergleicht das Ergebnis bit-genau gegen
die von Hand nachgebaute alte Formel — dieselbe Disziplin wie bei der
Bokeh-Erweiterung in Phase 28.

### Der Befund, der die Phase am meisten geprägt hat

**Zweimal von zehn Schritten war die geplante Funktion bereits da.**

- *Auto-Horizont*: `apx_ai::upright` macht seit Phase 13 Schritt 4 genau
  das (Canny + Hough), samt Tauri-Command und Tests. Es lag als Eintrag
  „Level" in einer Klappliste „Perspektive/Upright" im
  Objektivkorrekturen-Feldsatz unter der Registerkarte „Details", plus
  ein zweiter Klick auf „Automatisch erkennen".
- *Vergleichsmodus*: existiert seit Phase 9 mit neun Fotos und
  gemeinsamem Zoom. Was fehlte, war der Sichtungs-Ablauf — die Ansicht
  konnte gar nichts mit der Tastatur.

In beiden Fällen wurde **nicht neu gebaut**, sondern das Vorhandene
erreichbar gemacht: ein Kommando-Register-Eintrag unter dem
gebräuchlichen Namen, bzw. Tastaturbedienung und „aus dem Vergleich
nehmen". Das ist die eigentliche Lehre dieser Phase: bei einer App
dieser Größe ist „fehlt" oft „findet niemand", und der Unterschied
zeigt sich nur, wenn man vor dem Bauen nachsieht.

### Drei eigene Fehler, die nur durch reales Hinsehen auffielen

**1. Wechselnder Wurzelknoten.** Der erste Entwurf der Andockung gab
`viewerMain` direkt zurück, solange keine Analyse gebraucht wurde — und
wechselte damit den Wurzelknoten zwischen `<main>` und `<div>`, sobald
das Entwickeln-Bild eintraf. React reconciliert nach Position UND
Elementtyp: der ganze Viewer samt Canvas wurde neu eingehängt, der
ResizeObserver begann wieder bei 0×0, „Einpassen" fiel auf 100 % statt
10 % — **das Bild war weg**. Kompiliert hat das einwandfrei.

**2. Die neue Spalte drückte den Viewer auf null Breite.** In einem
1280-px-Fenster sind alle Paletten `shrink-0`, der Viewer ist das
einzige Element, das nachgibt. Deshalb jetzt eine Mindestbreite und eine
Untergrenze fürs Andocken (ab 1500 px Fensterbreite).

**3. Ein Listener, der sich selbst abhängte.** Die Tastaturbedienung der
Vergleichsansicht hatte `[photos, cursor, …]` in der
Abhängigkeitsliste; `photos` ist bei jedem Rendern ein neues Array. Bei
den **Pfeiltasten** ändert der App-weite Handler die Auswahl, React
rendert synchron neu — und der Listener wurde mitten in der laufenden
Ereignis-Auslieferung entfernt und neu gehängt. Das DOM ruft einen so
entfernten Listener nicht mehr auf. Ergebnis: Pfeiltasten taten nichts,
`Entf` (löst kein Neurendern aus) funktionierte einwandfrei. Diese
Teil-Funktion ist das Tückische — ein Test mit `Entf` allein wäre grün
gewesen. Gefunden durch gezieltes Messen im Browser (welche Taste
erreicht den Handler überhaupt?), nicht durch Raten.

### Bekannte Grenze

Auch nach Schritt 1 bleiben es fünf Spalten. Die strukturell richtige
Antwort wäre, im Entwickeln-Modus Ordner- und Preset-Palette zu EINER
linken Spalte zusammenzulegen, wie Lightroom es tut. Das ist ein eigener
Umbau und wurde hier bewusst nicht nebenbei mitgemacht; die Paletten
sind einzeln einklappbar, und der Fokus-Modus (Phase 26) blendet sie
gesammelt aus.

---

## ADR-0065: Phase 32 — zehn Funktionen, die etwas verändern

**Kontext.** Nutzerwunsch nach Phase 31: zehn weitere Funktionen, jede
mit echtem Umfang und wirklich neuen Bedienelementen — nicht zehn
Regler mehr in einem bestehenden Panel. Die erste Liste enthielt drei
Punkte, die es längst gab (Auto-Horizont, Vergleichsansicht,
Kompositionsraster); aufgefallen ist das erst, als die Suche nicht nach
den deutschen Beschriftungen, sondern nach den englischen Bezeichnern im
Code lief. Diese drei wurden ausgetauscht bzw. auf das ausgebaut, was
tatsächlich fehlte.

**Die zehn Funktionen und ihre wesentliche Entscheidung.**

1. **Kompositionsraster** (F1): die Raster gab es, aber Spirale und
   Diagonalen lagen fest in einer Ecke. Neu sind echte SVG-Bögen für die
   goldene Spirale mit alternierenden φ-Schnitten und eine Dreh-/
   Spiegel-Steuerung. Der Test, der die Breite monoton fallen sah, hatte
   die falsche Invariante — bei alternierenden Schnittrichtungen fällt
   die *Fläche*.
2. **Histogramm zum Ziehen** (F2): fünf Zonen, **nicht** gleich breit.
   Ein gleichmäßiges Fünftel gäbe „Weiß" so viel Fläche wie der
   Belichtung, obwohl der Weißpunkt nur die obersten Werte betrifft.
3. **Kalenderansicht** (F3): gruppiert nach dem **lokalen** Kalendertag
   (UTC schöbe eine mitteleuropäische Abendaufnahme auf den Folgetag);
   Fotos ohne Aufnahmedatum werden benannt statt geraten; leere Monate
   bleiben sichtbar, weil die Pause die Information ist.
4. **Stapel-Umbenennung** (F4): Vorschau und Anwenden benutzen
   **denselben** Rust-Planer. Zweiphasiges Umbenennen über
   Zwischennamen, sonst scheitert jeder Ringtausch. Musterfehler
   blockieren den ganzen Stapel, eine virtuelle Kopie wird nur
   übersprungen — Fehler *im Muster* gegen Eigenschaft *eines Fotos*.
5. **Ausrüstung & Belichtung** (F5): Klassengrenzen nach Objektiv- und
   Blendenstufen statt gleichmäßig; fehlende EXIF-Werte bekommen einen
   eigenen Balken „keine Angabe"; Brennweiten werden **nicht** auf
   Kleinbild umgerechnet (der Crop-Faktor wäre geraten).
6. **Notizen am Foto** (F6): normierte Koordinaten aufs **unbeschnittene
   Original** — anders als `face_detections`, das Pixel einer
   Vorschaustufe speichert. Liegt eine Notiz außerhalb des Zuschnitts,
   wird sie nicht an den Rand geklebt, sondern gezählt und benannt.
7. **Serien-Erkennung** (F7): unterscheidet Reihenaufnahme und
   Belichtungsreihe über EV bei ISO 100. Ohne die ISO-Normierung sähe
   eine ISO-Belichtungsreihe wie eine Reihenaufnahme aus. Fokusreihen
   und Panoramen werden ausdrücklich **nicht** erkannt — sie sind in den
   EXIF-Daten von einer Reihenaufnahme nicht zu unterscheiden.
8. **Rahmen und Passepartout** (F8): der Rahmen vergrößert die
   Bildfläche **nicht**, er wird hineingezeichnet. Eine wachsende
   Leinwand würde jede Größenangabe im Export, jedes Zuschnitt-Rechteck
   und jedes Overlay verschieben. Die Stufe läuft als letzte, nach
   `geometry` — und deshalb CPU-only: dort liegen die Daten als RGBA8
   auf der CPU, ein Compute-Dispatch fürs Füllen von vier Rechtecken
   würde mehr Zeit mit Datentransfer verbringen als mit Rechnen.
9. **Sammlungs-Board** (F9): Ziehen verschiebt, es kopiert nicht —
   „Auswahl" und „Aussortiert" schließen sich aus. Beim Verschieben erst
   hinzufügen, dann entfernen: bricht es dazwischen ab, ist das Foto in
   beiden Sammlungen statt in keiner.
10. **Export-Vorschau** (F10): eine Schätzung mit ausgewiesener Spanne
    statt einer Zahl, die Genauigkeit vortäuscht. Für unkomprimiertes
    TIFF ist die Rechnung exakt und wird auch so benannt. Eine Messung
    wäre der ganze Export, nur eben zweimal.

**Rückwärtskompatibilität.** Zwei neue EDL-Felder (`grain_midtone_bias`
war Phase 31, `frame` ist F8) und eine neue Migration (0013,
`photo_notes`). Wie seit Phase 28 gilt: bewiesen, nicht behauptet — ein
Test entfernt das Feld aus einem serialisierten `EdlV4` und prüft den
Neutralwert, ein zweiter prüft, dass ein neutraler Rahmen das Bild
bit-für-bit unverändert lässt. Bei `frame` kam ein dritter Punkt dazu:
`mat_color` bekommt ausdrücklich Weiß als Default, nicht das
`[0.0, 0.0, 0.0]`, das ein `f32`-Array von sich aus liefert.

**Was bewusst nicht gebaut wurde.** Ein Datumsfilter im Backend (F3
füllt stattdessen die vorhandene Mehrfachauswahl), ein Objektivfilter
(F5 macht Objektive deshalb bewusst nicht klickbar), eine Fokusreihen-
Erkennung (F7, siehe oben) und ein WGSL-Zwilling für den Rahmen (F8).
Jede dieser Lücken steht an ihrer Stelle im Code, statt durch ein
Bedienelement überdeckt zu werden, das nichts tut.

---

## ADR-0066: Phase 33 — zehn Funktionen, die Lücken schließen

**Kontext.** Nach Phase 32 stand die Frage, was im Katalog tatsächlich
fehlt und nicht nur anders heißt. Die Recherche lief diesmal über
englische Bezeichner statt deutscher Beschriftungen — in Phase 32 hatten
sich drei vermeintliche Lücken als bereits gebaute Funktionen
herausgestellt, weil nach der Oberflächensprache gesucht worden war.
Übrig blieben zehn Befunde, jeder im Code nachgewiesen.

**Die zehn Funktionen und ihre jeweils entscheidende Wahl:**

**F1 — Papierkorb.** `delete_photo` gab es nicht; nur
`delete_photo_note`. Weggeworfene Fotos bekommen jetzt ein
`deleted_at` (Migration 14) statt gelöscht zu werden. Eine eigene
`trash`-Tabelle mit einer Kopie der Zeile wäre die Alternative gewesen;
dagegen sprach, dass an einem Foto ein Dutzend Tabellen per
`ON DELETE CASCADE` hängen — ein echtes DELETE reißt sie alle mit, und
ein „Wiederherstellen" könnte davon nur die Foto-Zeile zurückbringen,
nicht die daran hängende Arbeit. Der Preis ist, dass jede auflistende
Abfrage `deleted_at IS NULL` mitführen muss; dafür gibt es genau eine
Konstante (`repository::photos::NOT_TRASHED`), und ein Test prüft, dass
ein weggeworfenes Foto wirklich aus Ordnerliste, Volltextsuche,
Attributfilter und Statistik verschwindet.

**F2 — Ordner abgleichen.** `reconcile.rs` prüfte nur „ist die Datei
noch da?", der Import nur die Gegenrichtung. Was der Katalog *mehr* hat
als der Ordner, sah keiner von beiden. Die Planung ist jetzt eine reine
Funktion über zwei Listen — dieselbe Trennung wie bei der
Stapel-Umbenennung aus Phase 32 F4, damit Vorschau und Anwenden
garantiert dasselbe rechnen. Eine Änderung heißt Größe **oder**
Zeitstempel: nur die Größe übersieht ein verlustfrei gedrehtes JPEG, nur
der Zeitstempel schlägt bei einem Backup-Rückspiel falsch an. Sicher
wäre nur ein Hash — und den für jede Datei jedes Mal zu lesen macht aus
dem Abgleich einen Neu-Import.

**F3 — Schärfe bewerten.** Kein Schärfemaß für ein ganzes Bild
existierte. Gemessen wird je 64px-Kachel und als 90. Perzentil
ausgewertet, nicht als Bildmittel: ein Porträt mit offener Blende ist zu
80 % unscharf und soll es sein. Die Laplace-Energie wird durch die
Varianz derselben Kachel geteilt, sonst gewönne jedes Mal die hellere
Aufnahme, auch die unschärfere.

**F4 — Metadaten-Vorgaben.** Keine neue Tabelle: `templates` ist seit
Migration 6 der Ablageort für „ein JSON unter einem Namen". Die
Entscheidung, an der alles hängt, ist `Option<String>` je Feld — `None`
heißt „nicht Teil der Vorgabe", `Some("")` heißt „leeren". Ohne diese
Unterscheidung hätte die Vorgabe, die nur Urheber und Copyright
nachträgt, beim ersten Anwenden alle einzeln geschriebenen
Bildunterschriften mitgenommen.

**F5 — Wasserzeichen-Vorlagen.** Wasserzeichen gab es seit Phase 8, aber
mit Schriftgröße und Rand in absoluten Pixeln — auf einem 6000-px-Export
ein Fliegenschiss, auf einem 800-px-Export ein Balken. Genau deshalb
ließen sie sich nicht als Vorlage speichern. Jetzt Prozent **der
kürzeren Kante** (nur so ist Hoch- und Querformat gleich), dazu
Kachelung mit Drehung. Reihenfolge ist Absicht: erst skalieren, dann
drehen — andersherum mittelte die Skalierung die Zwischenwerte der
Drehung noch einmal und die Kanten würden treppig.

**F6 — RGB-Parade.** Die Wellenform aus Phase 14 legt die Kanäle
übereinander und kombiniert sie per Maximum; wo zwei dicht
beieinanderliegen, verdeckt einer den anderen — genau dort, wo ein
Farbstich entsteht. Neu ist nicht nur die Darstellung nebeneinander,
sondern die Auswertung: Schwarz- und Weißpunkt je Kanal über das
0,5-%-Perzentil (ein einziger ausgefressener Pixel verschöbe ein
Minimum auf 0) und der benannte Farbstich je Tonwertbereich. Fehlt einem
Kanal in einem Bereich jedes Pixel, wird dort kein Stich gemeldet — aus
zwei von drei Kanälen lässt sich keiner ablesen; dafür gibt es die
Gesamtzeile.

**F7 — Erweiterte Katalogfilter.** Schließt die beiden in ADR-0065
ausdrücklich offengelassenen Lücken (Datum aus F3, Objektiv aus F5) und
ergänzt ISO-Bereich, Seitenverhältnis und Medienart. Das
Seitenverhältnis heißt `Aspect` und nicht `orientation`, weil die
`photos`-Tabelle diese Spalte bereits für das EXIF-Drehungs-Flag
verwendet; bei den Werten 5 bis 8 steht das Bild in der Datei quer und
auf dem Schirm hochkant, und gefiltert wird nach dem, was man sieht.
`FilterField` und der Regelbaum haben dieselben Felder bekommen — ohne
das hätte die Umwandlung `FilterCriteria` → `FilterNode` die neuen
Filter beim Speichern einer intelligenten Sammlung verloren. Dafür
brauchte es den neuen Operator `AtMost`.

**F8 — Manuelle Reihenfolge.** `collection_photos.position` gab es seit
Migration 3 und bestimmte die Anzeigereihenfolge — nur ändern ließ sie
sich nicht. Beim Speichern werden alle Positionen neu als 0..n-1
geschrieben statt Bruchzahlen zwischen die Nachbarn zu setzen: das
Lücken-Verfahren verschiebt die Neuvergabe nur, bis keine ganze Zahl
mehr passt. Nur in einer Sammlung, nicht im Ordner — der hat die
Reihenfolge des Dateisystems.

**F9 — Ähnliche Fotos zu einem Referenzfoto.** Die Duplikatsuche
gruppiert alle gegen alle, ohne Referenz und damit ohne Rangfolge. Hier
braucht es zwei Maße: ein Perceptual Hash ist fast farbenblind — ein
Foto und seine Schwarzweiß-Fassung sind für ihn nahezu identisch, und
wer „ähnliche" sagt, meint oft genau das Gegenteil. Struktur und Farbe
werden beide gerechnet, ein Regler mischt.

**F10 — Stapel im Raster.** Stapel gab es seit Phase 9, im Raster sah
man davon nichts. Ein eingeklappter Stapel steht an der Stelle seines
ersten sichtbaren Mitglieds (sonst würde Einklappen das Raster
umsortieren), gezählt wird nur, was sichtbar ist (die volle Zahl wäre
ein Versprechen auf weggefilterte Fotos), und ist das Deckblatt
weggefiltert, übernimmt das erste sichtbare Mitglied.

**Bewusst offen geblieben.** Ein automatisches Leeren des Papierkorbs
nach N Tagen (die Abfrage dafür steht, der Auslöser fehlt); eine
Umbenennungs-Erkennung beim Ordner-Abgleich (bräuchte einen
Inhaltsvergleich, und ein falsch geratenes „dieselbe Datei" würde
Bearbeitungen dem falschen Foto zuschlagen); die Schärfe-Bewertung auf
dem Original statt auf der Vorschau; eine Vorschau der echten Schrift im
Wasserzeichen-Dialog. Jede dieser Lücken steht an ihrer Stelle im Code.

---

## ADR-0067: `beforeDevCommand`/`beforeBuildCommand` verzeichnisunabhängig

**Status:** Akzeptiert

**Kontext.** `pnpm tauri dev` bzw. `npx --prefix ../../frontend tauri dev`
brach reproduzierbar ab:

```
Running BeforeDevCommand (`pnpm --dir ../../frontend dev`)
[ERROR] ENOENT: no such file or directory, lstat 'C:\Users\nikla\frontend'
Error The "beforeDevCommand" terminated with a non-zero status code.
```

Ursache ist nicht der Aufrufer, sondern eine Fehlannahme in der Konfiguration.
Die Tauri-CLI führt die Hooks **nicht** im Verzeichnis der `tauri.conf.json`
aus, sondern in ihrem sogenannten *app dir*. Das ermittelt sie, indem sie vom
aktuellen Arbeitsverzeichnis aus nach oben nach einer `package.json` sucht und
— wenn sie keine findet — auf das **Elternverzeichnis** des Tauri-Verzeichnisses
zurückfällt. In diesem Projekt liegt `tauri.conf.json` bewusst in
`crates/apx-app/` (kein `src-tauri/`-Unterordner), und es gibt keine
`package.json` im Repository-Wurzelverzeichnis. Daraus folgt:

| Aufruf aus        | app dir der CLI | `../../frontend` zeigt auf |
| ----------------- | --------------- | -------------------------- |
| `crates/apx-app/` | `crates/`       | *außerhalb* des Repos      |
| `frontend/`       | `frontend/`     | *außerhalb* des Repos      |
| Repo-Wurzel       | `crates/`       | *außerhalb* des Repos      |

Der bisherige Pfad `../../frontend` war damit in **keinem** Startverzeichnis
richtig — der Hook konnte nie funktionieren. Aufgefallen ist es bisher nicht,
weil der einzige Job, der ihn auslöst (`release` in `.github/workflows/ci.yml`),
nur bei Tag-Push oder `workflow_dispatch` läuft, und weil die übrigen CI-Jobs
das Frontend vorher separat mit `pnpm build` in `frontend/` erzeugen.

**Entscheidung.** Beide Hooks probieren die möglichen Lagen der Reihe nach
durch, statt eine davon anzunehmen:

```
pnpm --dir ../frontend dev || pnpm --dir frontend dev || pnpm dev
```

Das deckt alle drei app dirs ab, die die CLI in diesem Repository wählen kann,
und trifft in den beiden realistischen Fällen schon beim ersten Versuch:
aus `crates/` ist `../frontend` das Frontend, aus `frontend/` ist
`frontend/../frontend` wieder dasselbe Verzeichnis. Nur beim (unüblichen)
Start aus der Repo-Wurzel greift der zweite Zweig, nach einer Fehlzeile von
`pnpm`. `pnpm` beendet sich bei falschem `--dir` mit Status 1 (verifiziert),
die Kette schaltet also zuverlässig weiter.

**Verworfene Alternativen.**

- **`beforeDevCommand` in Objektform mit `cwd`.** Die CLI benutzt ein relatives
  `cwd` direkt als Arbeitsverzeichnis des Kindprozesses, also relativ zum
  Aufrufer — damit wandert das Problem nur, statt zu verschwinden.
- **`node -e` mit Aufwärtssuche nach `frontend/package.json`.** Sauber im
  Verhalten und real getestet, aber die Hooks laufen durch `sh -c` bzw.
  `cmd /S /C`: unquotiert scheitert jede Klammer an `sh`
  (`Syntax error: "(" unexpected`), quotiert kollidieren die inneren
  Anführungszeichen mit der Argument-Maskierung unter `cmd`. Eine
  Ein-Zeilen-Lösung ohne Anführungszeichen, Leerzeichen und Klammern ist
  möglich, aber unlesbar — für zwei Zeilen Konfiguration ein schlechter Tausch.
- **`package.json` in der Repo-Wurzel**, damit das app dir deterministisch wird.
  Löst den Fall „Start aus `frontend/`" nicht und zieht Werkzeugketten-Folgen
  (Lockfile-Ort, Workspace-Erkennung) nach sich, die in keinem Verhältnis stehen.

**Nicht geändert.** `frontendDist: "../../frontend/dist"` bleibt, wie es ist:
diesen Pfad löst `tauri-build` relativ zur `tauri.conf.json` auf, dort ist
`../../frontend` korrekt. Der Unterschied zwischen beiden Auflösungsbasen ist
genau die Falle, in die die alte Fassung gelaufen ist.

---

## ADR-0068: Aufnahmedatum für Nicht-RAW-Dateien, und eine Fehlergrenze

**Status:** Akzeptiert

Zwei Meldungen aus der Benutzung, beide mit derselben Eigenschaft: der
Code sah unverdächtig aus, die Tests waren grün, und trotzdem war die
Funktion für den Nutzer nicht vorhanden.

### Befund 1 — der Kalender blieb für jedes Foto leer

`crates/apx-raw/src/fallback.rs` (der Pfad für JPEG/PNG/TIFF, also für
fast jeden realen Import) setzte `captured_at` hart auf `None`, mit dem
Kommentar, Datum/Zeit-Parsing sei „für den Fallback-Pfad in Phase 1 nicht
erforderlich" — JPEG/PNG/TIFF seien „nur ein Auffangnetz". Diese Annahme
war schon bei ihrer Niederschrift falsch und ist es über 30 Phasen
geblieben: jedes so importierte Foto steht ohne Aufnahmedatum im Katalog.

Betroffen war damit alles, was am Datum hängt — die Kalenderansicht
(Phase 32 F3) am sichtbarsten, aber auch Sortierung nach Aufnahmezeit,
Serien-/Belichtungsreihen-Erkennung (Phase 32 F7) und die
Datumsplatzhalter beim Import-Umbenennen. Die Kalender-Tests waren
grün, weil sie mit gesetztem `captured_at` arbeiten; getestet war die
Ansicht, nie der Weg von der Datei dorthin.

**Entscheidung.** Der Fallback-Pfad liest `DateTimeOriginal` (ersatzweise
`DateTimeDigitized`) und `OffsetTimeOriginal`/`OffsetTime` und benutzt
dafür **dieselben** beiden Parser wie der RAW-Pfad
(`metadata.rs::parse_exif_datetime`/`parse_exif_offset`, jetzt
`pub(crate)`). Zwei Parser für dasselbe Format wären genau die
Doppelpflege, aus der der Fehler entstanden ist. Ohne Offset-Tag gilt
weiterhin die Kamera-Uhrzeit als UTC — dieselbe Auslegung wie bisher im
RAW-Pfad, nicht eine zweite daneben.

Getestet wird über einen echten APP1/EXIF-Block mit **Exif-Sub-IFD**
(Tag `0x8769`), nicht über den String-Parser allein: die Datumstags
liegen laut Standard nicht in der IFD0, und `kamadak-exif` findet sie
ohne diese Sub-IFD nicht. Der Testhelfer aus Phase 18 wurde dafür
verallgemeinert statt ein zweiter danebengestellt.

**Der Fix allein hätte dem Nutzer nichts genützt.** Er wirkt nur auf
neue Importe; ein über Monate aufgebauter Katalog bliebe dauerhaft ohne
Datum. Deshalb zusätzlich:

- `apx_catalog::TechnicalMetadata` + `photos::set_technical_metadata` —
  schreibt **nur** die aus der Datei gelesenen Spalten. `upsert`s
  `update_row` wäre das falsche Werkzeug: es überschreibt auch
  Dateigröße, Änderungszeit und Inhalts-Hash, die beim Metadaten-Abgleich
  gar nicht neu berechnet werden, und würde den Katalog damit gegenüber
  der Platte verfälschen. Bewertung, Flagge, Farbmarkierung, die selbst
  gepflegten IPTC-Felder und ein von Hand gesetzter Ort bleiben
  unberührt.
- Befehl `rescan_photo_metadata(folder_id?)`. Virtuelle Kopien (keine
  eigene Datei) und Videos (kein EXIF) werden übersprungen, ein nicht
  lesbares Original wird gezählt statt geworfen — ein einzelnes
  ausgehängtes Laufwerk darf den Durchlauf nicht abbrechen.
- Der Knopf sitzt **in der Kalenderansicht**, direkt an der Zeile
  „N ohne Aufnahmedatum", nicht in einem Wartungsdialog. Die Stelle, an
  der ein Mangel sichtbar wird, ist die Stelle, an der seine Behebung
  angeboten gehört; dieselbe Linie wie ADR-0046 Entwurfsentscheidung 4
  (eine Funktion, die man erst suchen muss, ist für die meisten nicht
  vorhanden).

### Befund 2 — ein weißer Bildschirm ohne Ausweg

Gemeldet als „weißer Bildschirm und keine Möglichkeit es wegzumachen".
Die Ursache dafür, dass daraus ein *unentrinnbarer* Zustand wird, ist
unabhängig davon, was genau abgestürzt ist: im gesamten Frontend gab es
keine einzige Fehlergrenze (`componentDidCatch`/
`getDerivedStateFromError` kamen nirgends vor). React hängt bei einem
Render-Fehler den kompletten Baum ab — übrig bleibt ein leeres Fenster,
ohne Kopfleiste, ohne Escape, ohne Hinweis, was passiert ist.

**Entscheidung.** `components/ErrorBoundary.tsx` mit zwei Ebenen:

- `scope="app"` in `main.tsx` als letztes Netz — ganzseitige Meldung mit
  „Neu laden", „Weiter ohne Neuladen" und „Details kopieren".
- `scope="region"` um die Mittelansicht, das Entwickeln- und das
  Masken-Panel. Stürzt eines davon ab, wird nur dieser Bereich ersetzt;
  Kopfleiste, Seitenleiste und die übrigen Paletten bleiben bedienbar.

Die Fehlermeldung steht bewusst im Klartext in der Oberfläche und ist
kopierbar. Ein Absturz, dessen einzige Spur ein weißes Fenster ist,
lässt sich aus einem Fehlerbericht heraus nicht nachvollziehen — genau
daran ist die Untersuchung von Befund 2 aufgelaufen: die gemeldete
Registerkarte („Am Bild") ließ sich im Browserlauf gegen den e2e-Mock
nicht zum Absturz bringen, der Auslöser liegt also in den echten Daten.
Mit der Fehlergrenze benennt der nächste Absturz sich selbst, statt die
App zu verschlucken.

**Nicht gemacht.** Keine Testing-Library als neue Abhängigkeit für den
Komponententest der Fehlergrenze — `react-dom/client` liegt ohnehin vor
und reicht für genau diesen Zweck (siehe
`components/ErrorBoundary.test.tsx`).

---

## ADR-0069: EDL-Altstände auffüllen statt blind casten

**Status:** Akzeptiert

Nachtrag zu ADR-0068, Befund 2. Dessen Fehlergrenze hat beim ersten
Auftreten genau das geliefert, wofür sie gebaut wurde — den Absturz im
Klartext statt eines weißen Fensters:

```
TypeError: Cannot read properties of undefined (reading 'point_lights')
    at ImageToolsPanel (src/components/ImageToolsPanel.tsx:159:37)
    at DevelopPanel
    at ErrorBoundary
```

**Befund.** `apx-core`s `EdlEnvelope.payload` ist ein opakes
`serde_json::Value`. Der Katalog speichert das EDL-JSON wortwörtlich und
gibt es wortwörtlich zurück; es läuft nie durch `EdlV4`. Ein EDL, das
eine ältere Version der App geschrieben hat, enthält deshalb genau die
Felder von damals — alles, was seither **additiv** dazugekommen ist
(`creative` Phase 15, `light_optics` Phase 28, `interactive` Phase 30,
`frame` Phase 32 …), fehlt darin schlicht.

Die Rust-Seite fängt das seit jeher ab: jedes dieser Felder trägt
`#[serde(default)]`, weshalb alte EDLs korrekt gerendert werden und am
Bild nie etwas auffiel. Das Frontend dagegen castete blind:

```ts
// Keine tiefe Struktur-Validierung (Feld für Feld) … reicht hier ein
// grober Plausibilitätscheck, da diese Funktion nur auf Antworten
// angewendet wird, die dasselbe Backend gerade erst geschrieben hat.
return payload as EdlPayload;
```

Die Begründung im Kommentar war der Fehler. Geschrieben hat das EDL
womöglich eine **ältere Version** der App, und das Backend reicht solche
Altstände unverändert durch. Der Cast war damit nicht bloß ungeprüft,
sondern falsch — und zwar für **jedes** seit Phase 15 hinzugekommene
Feld, nicht nur für das gemeldete `interactive`.

**Entscheidung.** `normalizeEdlPayload` legt das geparste EDL über
`neutralEdlPayload()` und füllt dabei jedes fehlende Feld mit seinem
Neutralwert — rekursiv, damit auch ein Unterfeld aufgefüllt wird, das
innerhalb einer bereits bekannten Gruppe fehlt (der häufigere Fall).
Das ist die Frontend-Entsprechung von `#[serde(default)]` und schließt
die Lücke ein für alle Mal, statt sie Feld für Feld nachzupflegen.

**Arrays ersetzen, Objekte verschmelzen.** Gespeicherte Listen (Masken,
Reparaturstriche, Kurvenstützstellen, gesetzte Lichter) ersetzen den
Neutralwert vollständig; elementweises Verschmelzen würde eine bewusst
geleerte Liste aus den Neutralwerten wieder auffüllen.

**Verworfen: `interactive` einzeln absichern.** Hätte genau diesen
Absturz behoben und alle übrigen Felder mit derselben Lücke
stehengelassen — dieselbe Doppelpflege, aus der ADR-0068s Befund 1
entstanden ist.

**Verifikation mit Gegenprobe.** Der e2e-Test belegt den Fund
deckungsgleich: der Mock lässt sich jetzt über `editHistoryByPhoto` mit
einem echten Altstand vorbelegen (im Browser sonst nicht herstellbar,
weil das Frontend eigene EDLs immer vollständig schreibt). Mit einem
sonst vollständigen EDL ohne `interactive` verschwindet das
Entwickeln-Panel ohne den Fix genau wie beim Nutzer; mit dem Fix
rendert es, und die Fehlergrenze greift gar nicht erst. Die Gegenprobe
wurde real gefahren, nicht behauptet.

---

---

## ADR-0070: Phase 34 — zehn Funktionen, und was das Bauen über den Bestand verriet

**Kontext.** Nach Phase 33 war die Frage wieder, was fehlt. Die
Auswahl dieser zehn folgte einer Linie: nicht „was hat eine andere App
noch", sondern „wo hört die App auf halbem Weg auf". Sieben der zehn
knüpfen an etwas an, das schon da war und ohne Fortsetzung nutzlos
blieb — Duplikatgruppen ohne Aufräumen, ein Kalender ohne Daten, ein
Cache ohne Füllung.

Zwei Befunde aus dem Bauen stehen am Anfang, weil sie über die Funktion
hinausgehen, zu der sie gehören:

**Der Cache-Befund (F10).** Der Vorschau-Cache hat drei Stufen. Der
Import schrieb immer nur die erste (256px Miniaturansicht).
`protocol::compute_preview` liest die zweite (2048px), findet nichts
und dekodiert die RAW-Datei — jedes Mal, für jedes Foto, beim ersten
Ansehen. Die Stufe existierte im Schema, in der Enum, im Leseweg, und
wurde nie geschrieben. Das ist keine fehlende Funktion, das ist eine
gebaute Funktion ohne Zulieferung.

**Der zweite-Antwort-Befund (F9).** Für „welches Duplikat behalte ich"
gab es bereits eine Antwort: `frontend/src/lib/duplicates.ts`, Auflösung
vor Dateigröße vor Bewertung, als Sternchen im Organisieren-Dialog. Die
neue Regel im Backend entscheidet anders (siehe unten). Zwei Antworten
auf dieselbe Frage sind schlimmer als eine unvollständige, deshalb ist
die alte Datei gelöscht und nicht zusätzlich stehengelassen.

**Die zehn Funktionen und ihre jeweils entscheidende Wahl:**

**F1 — Suche über alles.** Die Volltextsuche (FTS5) indizierte nur
Spalten der `photos`-Tabelle; Schlagworte und Notizen liegen in eigenen
Tabellen und waren damit unauffindbar. FTS5-Tabellen mit externem
Inhalt lassen sich nicht um Spalten erweitern — die Tabelle musste neu
gebaut und ihre Trigger neu angelegt werden (Migration 15). Die
Erweiterung selbst ist kein zweiter Index, sondern ein zweiter
`UNION ALL`-Zweig mit `LIKE` über die verbundenen Tabellen. `UNION`
allein hätte nicht dedupliziert, weil die Zweige eine unterschiedliche
`ordering`-Spalte tragen; der zweite Zweig schließt deshalb ausdrücklich
aus, was der erste schon gefunden hat.

**F2 — Belichtung angleichen.** Ein Stopp ist eine Verdoppelung des
*linearen* Lichts, also muss sRGB linearisiert werden, bevor `log2`
darauf angewandt wird. Geklippte Pixel (unter 4, über 251) tragen keine
Belichtungsinformation mehr und werden verworfen statt mitgemittelt.
Gemessen wird in Rust, geschrieben aus dem Frontend über
`apply_develop_edit` — dieselbe Trennung wie bei der Schärfe-Bewertung
aus Phase 33 F3, damit EDL-Validierung, Verlauf und Beschriftung durch
eine Stelle laufen.

**F3 — Differenzbild.** Zwei Fotos übereinander, Pixel gegen Pixel. Die
Berechnung läuft auf einem festen 512er-Raster statt auf der vollen
Auflösung: zwei Aufnahmen desselben Motivs haben selten exakt dieselben
Maße, und ein Vergleich, der daran scheitert, hilft niemandem.

**F4 — GPX-Geotagging.** Nach einem selbstgeschriebenen XML-Scanner
stellte sich heraus, dass `apx_export::map::parse_gpx` (quick_xml)
längst existierte. Der eigene Scanner ist gelöscht; geblieben sind die
beiden Teile, die es wirklich noch nicht gab: die Zuordnung Zeit →
Position mit Interpolation, und die Regel, dass über eine Lücke im Track
hinweg **nicht** interpoliert wird — zwischen zwei Punkten im Abstand
von drei Stunden liegt keine Gerade, sondern eine Pause.

**F5 — Katalog-Gesundheit.** Sechs Lücken, die einzeln über Filter
auffindbar waren — wenn man auf die Idee kam, danach zu suchen. Die
Reihenfolge der sechs ist eine Aussage über Dringlichkeit (fehlende
Dateien zuerst, Bewertungslücken zuletzt) und gehört deshalb in die
Katalogschicht, nicht in die Darstellung. Jede Zeile führt zur Arbeit,
nicht nur zur Zahl: „Anzeigen" legt die Fotos in dieselbe
Mehrfachauswahl, mit der Stapel-Bewertung und Export ohnehin arbeiten.

**F6 — Verlaufs-Vergleich über das ganze EDL.** Verglichen wurde bis
dahin nur der preset-fähige Sektionsumfang. Für Presets ist diese
Auswahl richtig (nicht alles lässt sich sinnvoll übertragen), für die
Frage „was hat dieser Schritt geändert" ist sie falsch: ein Schritt, der
eine Maske verschoben hat, wurde als „keine Unterschiede" gemeldet. Der
erste Entwurf verglich Arrays über `JSON.stringify` und brachte den
Renderer zum Absturz — 3D-LUT-Tabellen und eingebettete Maskendaten
ergeben mehrere Megabyte pro Vergleich. Ersetzt durch ein `deepEqual`
mit frühem Ausstieg.

**F7 — Standardentwicklung je Kamera.** Beim Import bekommt ein Foto
automatisch das EDL, das für sein Kameramodell hinterlegt ist. Die
Vorgaben werden einmal pro Importlauf geladen, nicht pro Datei; dafür
wurde aus den acht Einzelparametern von `import_single_file` ein
`ImportRun`-Struct — nicht um clippy zu beruhigen, sondern weil acht
Parameter, von denen vier für den ganzen Lauf konstant sind, ohnehin
falsch geschnitten waren.

**F8 — Nach Aufnahmedatum einsortieren.** Der Import legt Fotos dort
ab, wo sie herkommen, und das war auf der Speicherkarte ein einziges
Verzeichnis. Plan und Anwenden getrennt wie beim Ordner-Abgleich (F2 der
Vorphase) — hier noch wichtiger, weil echte Dateien verschoben werden.
Der Kern ist die Kollisionsprüfung: zwei Fotos vom selben Tag mit
demselben Dateinamen landen im selben Zielordner, und ohne Prüfung
überschriebe das zweite das erste unwiederbringlich. Der erste Anspruch
auf einen Zielpfad gewinnt, jeder weitere wird gemeldet und bleibt
liegen; ein bereits richtig liegendes Foto belegt seinen Platz dabei
ebenfalls. Der Rückfall auf die Dateizeit ist abgeschaltet vorgegeben:
sie sagt, wann zuletzt geschrieben wurde, und ein Kopiervorgang setzt
sie auf heute.

**F9 — Duplikat-Assistent.** Geordnete Kriterien statt einer
Punktesumme, und jedes kann sagen, warum es entschieden hat. Die
Reihenfolge ist die eigentliche Aussage: zuerst, was ein Mensch
ausdrücklich entschieden hat (Pick-Flagge, Bewertung) — eine Messung
darf eine Entscheidung nicht überstimmen —, dann was Arbeit wäre zu
verlieren (vorhandene Bearbeitungen), dann das Original vor dem Ableger
(RAW vor JPEG, weil aus einem RAW ein JPEG wird und nicht umgekehrt),
erst danach Auflösung und Dateigröße. Ein Gleichstand entscheidet
nicht, aber er verkleinert das Feld: wer bei der Bewertung hinten liegt,
ist raus, auch wenn er die größere Datei hat. Ein Reject-markiertes Foto
bleibt nie, solange es eine Alternative gibt. Der Vorschlag ist
umstellbar; die eine harte Grenze ist, dass aus keiner Gruppe jede
Version verschwinden darf, und die wird beim Anwenden neu gerechnet
statt der Auswahl geglaubt.

**F10 — Vorschauen vorbereiten.** Siehe den Cache-Befund oben. Die
Lösung erfindet keinen zweiten Cache, sie füllt den vorhandenen: gleiches
Pfadschema wie `import::thumbnails`, nur mit der Stufennummer 1 statt 0,
und `WARM_EDGE` **ist** `protocol::STANDARD_EDGE` statt einer
gespiegelten Zahl — eine Kopie wäre irgendwann abgedriftet, und dann
bereitete das Modul etwas vor, das niemand abruft. Eigenes
Abbruch-Token statt `active_import` mitzubenutzen, weil das Vorwärmen
eines alten Ordners keinen neuen Import blockieren soll.

**Verworfen: die Vorbereitung automatisch an den Import hängen.** Das
hätte jeden Import um ein Vielfaches verlängert, und zwar auch für die
Fotos, die niemand je im Einzelbild ansieht. Der Nutzer entscheidet,
wann sich die Wartezeit lohnt; deshalb steht vor dem Start die Zahl, wie
viel Arbeit anfällt.

**Verifikation.** Jede der zehn Funktionen hat Rust-Unit-Tests für ihre
Rechenregel und einen Playwright-Test für den Weg durch die Oberfläche;
die e2e-Mocks rechnen dieselbe Regel wirklich nach, statt ein Ergebnis
zu behaupten. Zwei Fehler in dieser Phase wurden von den eigenen Tests
gefunden, nicht vom Nutzer: das fehlende Deduplizieren in F1 und die
gemeinsam genutzten Referenzen in `neutralEdlPayload`, die zwei
„unabhängige" Neutral-EDLs aneinander koppelten.

---

---

## ADR-0071: Geburtstagsausgabe — eine personalisierte Fassung, und zwei Altfehler, die sie aufgedeckt hat

**Kontext.** Diese Fassung ist ein Geschenk: eine Begrüßung beim
Öffnen, ein festliches Thema, drei Looks, eine eingestellte Kamera und
eine ausführliche Einführung. Die Anforderung dazu war ausdrücklich,
dass alles davon wirklich funktioniert und nichts das Arbeiten
behindert — und genau dieser zweite Teil hat während der Umsetzung mehr
gekostet als der erste.

### Die Entscheidungen

**Die Deko ist vom Thema getrennt.** `theme: Birthday` färbt,
`birthday_decorations` bewegt. Zwei Schalter, weil es zwei Fragen sind:
Farben sind Geschmack, fliegendes Konfetti ist Konzentration. Wer am
Abend des Geburtstags noch zwei Stunden Fotos sortiert, soll die
festliche Palette behalten und die Bewegung abschalten können, ohne
das Thema zu verlieren. `reduced_motion` schlägt beides unabhängig —
Zugänglichkeit geht vor Schmuck.

**Die Fläche direkt um das Foto bleibt farbneutral.** Das ist die
wichtigste Entscheidung im ganzen Theme-Block. `--color-bg-base` ist
der Hintergrund des Viewers; ein warmer Stich unmittelbar neben dem
Bild verschiebt die Farbwahrnehmung des Betrachters (simultaner
Farbkontrast). Man korrigiert dann gegen die Oberfläche an und wundert
sich später über kühle Fotos. Die Festlichkeit sitzt deshalb in
Paletten, Rahmen, Akzent und Deko — nicht auf der Urteilsfläche. Aus
demselben Grund schwebt kein Luftballon über dem Bild: die
Ballonschicht liegt mit `z-index: 0` hinter dem Inhalt.

**Kontrast nachgerechnet, nicht geschätzt.** Der naheliegende
gedämpfte Altrosa-Ton für `--color-text-muted` lag bei 4,37:1 gegen
`--color-bg-panel` und damit unter AA für normalen Text. Der gewählte
Wert liegt bei 5,2:1, der Akzent bei 8,8:1.

**Eine eigene Konfetti-Physik statt einer Bibliothek.** Für Konfetti
gibt es fertige npm-Pakete; sie bringen eigenes Canvas-Management,
eigene rAF-Schleife und eine weitere Lizenzzeile mit — für etwa
sechzig Zeilen Euler-Integration. Dieselbe Linie wie `lib/i18n.ts`s
eigene Begründung: keine Abhängigkeit für etwas, das kleiner ist als
ihre Anbindung. Die Physik steht rein in `lib/confetti.ts` (kein DOM),
die Zeichenschicht in `ConfettiLayer.tsx`, der Kanal dazwischen in
`lib/celebrate.ts` — Letzteres, weil die Aufrufstellen zum Teil im
Store sitzen und der Store keine React-Komponente importieren darf.

**Drei Werte machen den Unterschied zwischen Konfetti und
Kieselsteinen:** Luftwiderstand (sonst wächst die Geschwindigkeit
unbegrenzt), Eigendrehung mit `cos`-Stauchung (macht aus einem Punkt
ein flatterndes Plättchen) und Lebensdauer (räumt jeden Ausbruch von
selbst ab, sodass die rAF-Schleife endet). Dazu ein app-weites Budget
von 220 Teilchen, gegen das jeder Ausbruch gekappt wird — damit
schnelle Klicks sich nicht aufaddieren.

**Gefeiert wird sparsam.** Bei Import- und Export-Ende, und nur ohne
Fehler. Nicht bei jedem angewendeten Preset: Konfetti, das bei jedem
Klick fliegt, ist nach zehn Minuten kein Fest mehr, sondern ein
Flackern, das man abschalten will.

**Die Begrüßung fängt keine Klicks ab.** Sie erscheint bei *jedem*
Start und liegt über allem; würde sie Zeigerereignisse annehmen, wäre
die App in den ersten fünf Sekunden unbenutzbar. Dieselbe Bauweise wie
`StartupSplash.tsx`: `pointer-events-none` auf der Fläche, nur der
Schließen-Knopf nimmt Klicks an.

**Die drei Looks sind gegen die echten Vorzeichen-Konventionen der
Pipeline geschrieben**, alle vier vorher im Code nachgesehen statt
geraten: `+temp_shift_kelvin` = wärmer, `+blacks` = Schwarzpunkt
angehoben, `post_vignette_amount` **negativ** = dunkle Ränder (die
Stufe addiert ihr Delta), Grading-Rad in Grad mit Sättigung 0..100.
Jeder Look hat genau einen Kerngedanken: „Goldene Stunde" warme
Lichter bei *kühlen* Schatten (ein reiner Kelvin-Schub wärmt die
Schatten mit und wirkt dann wie ein Farbstich); „Familienfoto"
lokalen Kontrast nach *unten* (genau der holt Poren und Fältchen
hervor — ein schöner Look für Menschen geht in die Gegenrichtung von
allem, was Landschaftspresets tun); „Erinnerung" den angehobenen
Schwarzpunkt, der einen alten Abzug ausmacht.

**Nachgewiesen wird am Pixel, nicht am JSON.** Jeder Preset-Test
schickt ein synthetisches Bild durch
`apx_pipeline::develop::render_rgba8` und misst die Wirkung: mindestens
zwei 8-Bit-Stufen Veränderung, messbar größerer Rot-Blau-Abstand,
angehobener Schwarzpunkt, dunklere Ränder. Ein Preset, das gültiges
JSON ist und nichts tut, hätte den alten Prüfweg bestanden.

**Die Geschenke sind löschbar und veränderbar.** Der Seed legt nur an,
was fehlt, und fasst Vorhandenes nicht an. Etwas, das sich nicht
wegräumen lässt, ist kein Geschenk, sondern eine Zumutung.

**Die Kamera-Vorgabe ist absichtlich zurückhaltender als die Looks.**
Sie ist ein Startpunkt für *jedes* importierte Foto; so stark
einzugreifen wie ein Look würde alles vorgefärbt importieren. Ein Test
prüft genau dieses Verhältnis.

**Die Kamera-Abbildung ist gezeichnet, nicht fotografiert.** Die
Netzwerk-Policy dieser Umgebung blockiert jede externe Bildquelle
(ADR-0047), und ein mitgeliefertes Produktfoto wäre ein fremdes
Lichtbildwerk im Paket. Gezeichnet ist die Bauform, die jede DSLR
teilt — kein Herstellerlogo; das Modell steht als Text daneben, wo es
hingehört.

### Die zwei Altfehler, die dabei herauskamen

Beide waren vorher da, beide hat erst das Messen gefunden, und beide
sind genau die Art Fehler, die eine Oberfläche „billig" wirken lässt.

**1. Der Scheinwerfer der Einführungstour hat nie funktioniert.**
`.apx-glass-strong` setzt unlayered `position: relative`, und
Tailwinds `fixed`-Utility ist in Tailwind v4 gegenüber unlayered CSS
nachrangig. Die vier Abdunkel-Rechtecke und die Karte lagen deshalb
seit Phase 21 **im Textfluss** statt über dem Bildschirm: der
Scheinwerfer zeigte keinen Ausschnitt, sondern stapelte vier graue
Blöcke untereinander und schob die Karte um deren Gesamthöhe nach
unten (gemessen: `position` war `"relative"`, die Karte lag bei y=878
in einem 720 Pixel hohen Fenster). Aufgefallen ist es nie, weil kein
Test die Geometrie gemessen hat, sondern nur Textsichtbarkeit — und
die stimmte. Aufgedeckt haben es erst die längeren Texte der neuen
Tour, die die Karte hoch genug machten, dass ihr „Weiter"-Knopf aus
dem Fenster rutschte.

Genau diese Kaskaden-Falle ist im Code bereits dreimal dokumentiert
(ADR-0055 `z-index` beim Überlauf-Menü, ADR-0055-Nachtrag III `fixed`
bei der Kopfzeile, ADR-0056 `absolute` bei der Zoom-Steuerung). Dies
ist der vierte Fall. Behoben wie der zweite: über Spezifität statt
Reihenfolge, mit einer eigenen Marker-Klasse `.apx-spotlight` — ein
`div.apx-glass-strong` hätte jede andere Glasfläche getroffen, die zu
Recht relativ ist.

**2. Die Kopfzeile lief bei 1280 Pixeln über.** Die Ansicht-Leiste war
686 Pixel breit und `shrink-0`; `header.scrollWidth` war 1342 bei
`clientWidth` 1280. „Einstellungen" war rechts angeschnitten, und nach
einem Import schob die (nicht schrumpfbare, unbegrenzte)
Ergebniszeile das Überlauf-Menü vollständig aus dem Fenster — beide
waren danach unerreichbar. Bei 1280 Pixeln, einer der häufigsten
Laptop-Breiten.

Behoben an beiden Stellen: die Ergebniszeile gibt ihren Platz jetzt
als Erste auf (`min-w-0 shrink`), und die Beschriftungen der sieben
Ansicht-Knöpfe erscheinen erst ab 1400 Pixeln. Damit sie dabei nicht
*namenlos* werden — die Symbole sind `aria-hidden`, ohne den Text
hätten die Knöpfe überhaupt keinen zugänglichen Namen mehr —, trägt
jeder Knopf nun ein `aria-label`. Sonst wären Screenreader und die
bestehenden `getByRole`-Tests gleichzeitig blind geworden.

### Ein Test, der nichts bewies

Die erste Fassung des „die Begrüßung blockiert nichts"-Tests klickte
einfach und prüfte das Ergebnis — und war wertlos: Playwrights Klick
wartet auf Klickbarkeit, also wartete er stillschweigend die 5,2
Sekunden ab, bis sich die Begrüßung selbst schließt, und klickte dann.
Mit abgeschaltetem `pointer-events-none` lief er genauso grün durch;
die Gegenprobe hat das gezeigt. Jetzt mit kurzer Frist plus der
Prüfung, dass die Begrüßung beim Klick noch dalag — und zusätzlich ein
von Zeit unabhängiger Test auf die berechneten `pointer-events`-Werte.
Derselbe Fehler steckte im Vignetten-Test der Presets: er verglich
Bildmitte gegen Rand, wo im Testbild aber ein dunkler Hautblock gegen
einen hellen Graukeil stand — gemessen wurden Motivhelligkeiten, nicht
die Vignette.

### Was diese Fassung ausdrücklich nicht leistet

Die Anforderung nannte „schnell und zuverlässig wie Lightroom". Dass
die Deko nichts kostet, ist gemessen (Start bis bedienbar: 459 ms mit,
480 ms ohne — innerhalb des Rauschens; ein voller Ausbruch über 600
Bilder bleibt unter 150 ms Rechenzeit). Dass die **Entwickeln- und
Import-Wege** insgesamt Lightroom-Niveau erreichen, ist damit nicht
behauptet: daran ändert diese Fassung nichts, und eine solche Aussage
bräuchte Messungen gegen echte RAW-Bestände, die dieses Projekt
bewusst nicht hat (ADR-0007). Verbessert wurde, was messbar im Weg
stand — die beiden Altfehler oben.

---

## ADR-0072: Die Kaskaden-Falle an der Ursache behoben — und 47 Menüeinträge einzeln angeklickt

**Anlass.** Nutzer-Rückfrage nach der Geburtstagsausgabe: „aber was ist
mit den anderen bugs die vorher ggf. vorhanden waren bzw. einfach
elemente die nicht funktioniert haben". Also keine neue Funktion,
sondern eine Bestandsaufnahme — und zwar eine, die misst statt liest.

**Der Fund, der alles erklärt.** Bei geöffnetem Überlaufmenü errechnete
sich die Aufklappliste als `position: relative` statt `absolute` und lag
damit bei y = −210, also 210 Pixel **über** dem Fensterrand. Die ersten
sechs Einträge — der gesamte KI-Abschnitt — waren per Maus unerreichbar.
Das deckt sich exakt mit der früheren Nutzermeldung, „KI-Ausfüllen" sei
unauffindbar.

Die Ursache ist dieselbe wie in ADR-0055 (`z-index`), ADR-0055 Nachtrag
III (`fixed` bei der Kopfzeile), ADR-0056 (`absolute` bei der
Zoom-Steuerung) und ADR-0071 (`fixed` beim Einführungs-Scheinwerfer):
ungeschichtetes CSS schlägt Tailwind v4s `@layer utilities`
grundsätzlich, unabhängig von der Spezifität. `.apx-glass` setzte
`position: relative` ungeschichtet — und damit verlor **jede**
Positionierungs-Utility auf einer Glasfläche, still und ohne Fehler.

**Fünf Vorfälle, fünf Einzelflicken.** Jedes Mal wurde der konkrete Fall
per Spezifitäts-Gegenregel repariert (`header.apx-glass`,
`.apx-spotlight`, ein äußerer Positionierungs-Rahmen). Jede dieser
Reparaturen war für sich richtig und keine verhinderte die nächste.

**Die Entscheidung.** Genau diese eine Eigenschaft steht jetzt in
`@layer components`. Tailwind v4 deklariert die Ebenenfolge
`theme, base, components, utilities`; `components` verliert damit gegen
jede Utility, und `relative` wirkt nur noch dort, wo die Aufrufstelle
nichts anderes sagt. Alle übrigen Glas-Eigenschaften bleiben
ungeschichtet — sie kollidieren mit keiner Utility, und ein
Ebenenwechsel für sie wäre Risiko ohne Gegenwert. Die alten
Gegenregeln bleiben stehen: sie schaden nicht und ein Rückbau wäre eine
weitere Änderung ohne Nutzen.

**Was das Audit sonst fand.** Alle 47 Einträge wurden einzeln aus einem
frischen Start heraus angeklickt und die Wirkung gemessen (Dialoge,
Vollbild-Überlagerungen, Reiterwechsel, Hauptbereich, Meldungen, und —
für die beiden Einträge, die bewusst nichts am DOM ändern — der
IPC-Mitschnitt).

- `KI-Ausfüllen` öffnete einen Dialog, der nur **innerhalb** des
  Entwickeln-Panels gerendert wird (`if (!open) return null`). Ein von
  außen gesetztes Store-Flag tat deshalb nichts. Das Panel wird jetzt
  mitgeöffnet.
- `Hintergrund entfernen` ist eine reine Video-Funktion und tat bei
  Fotos still nichts. Jetzt gesperrt.
- `Untertitel` und `Personen-Erkennung` trugen das Abzeichen „Nicht
  verfügbar" und blieben trotzdem klickbar. Jetzt gesperrt.
- `Hautglätten`, `Himmel ersetzen`, `Stilübertragung` und `Tiefenkarte`
  öffneten das Entwickeln-Panel im Reiter „Licht" statt bei ihrem
  Werkzeug. Neue Brücke `requestDevelopTab` (dasselbe Muster wie
  `pendingCommand`).
- `Versionen vergleichen` öffnete bei einem Foto ohne virtuelle Kopien
  eine Vergleichsansicht mit genau einem Foto. Der Eintrag sperrt sich
  jetzt, sobald der Kopien-Stand bekannt ist; beim allerersten Klick ist
  er das noch nicht, und dann sagt die Ansicht es selbst.
- Der zugängliche Name der KI-Einträge lief zusammen
  („HautglättenBereit"), weil Beschriftung und Statusabzeichen in zwei
  Geschwister-`<span>`s stehen. Jetzt ein `aria-label` mit Trennung.

`Import mit Vorlage…` und `Zweites Display…` sind **nachweislich in
Ordnung**: der IPC-Mitschnitt zeigt den Ordnerdialog bzw.
`plugin:webview|create_webview_window`. Nur der Test-Mock kann weder ein
Betriebssystem-Fenster noch ein zweites Tauri-Fenster zeigen.

**Was daraus bleibt.** `frontend/e2e/ui-invariants.spec.ts` hält beide
Invarianten dauerhaft fest: keine Glasfläche, deren errechnete Position
ihrer Utility widerspricht, und jeder Menüeintrag wirkt messbar oder ist
gesperrt. Der Kaskaden-Test wurde gegenkontrolliert — mit
zurückgedrehter Ursache schlägt er fehl und nennt die Aufklappliste.

**Selbstkritik.** Mein erster Audit-Durchlauf meldete acht angeblich
tote Einträge. Vier davon waren Messfehler meines eigenen Skripts
(geschluckte Klickfehler plus Namensvergleiche, die wegen des
zusammengelaufenen Namens nie trafen). Das steht hier, weil die
Verlockung groß war, die acht als Produktfehler zu melden und dann
„vier behoben" zu schreiben.

---

## ADR-0073: Warum die App zäh war — gemessen, nicht geraten

**Anlass.** Nutzer-Rückmeldung mitten in der Arbeit: „APERTURE X LÄUFT
HOLYYYYY SLOW — da geht fast gar nix", zusammen mit einem Screenshot
eines leeren Globus und „keine gefundenen gps daten".

**Die Versuchung war, zu raten.** Die Umgebung, in der das Problem
auftritt (WebKitGTK unter Linux, über Tauri), lässt sich hier nicht
nachbilden; Chromium im Headless-Betrieb erreicht unabhängig von den
Effekten 60 Bilder je Sekunde. Eine Messung, die nicht misst, worum es
geht, ist aber schlimmer als keine — sie gibt falsche Sicherheit.
Deshalb wurde hier nur das gemessen, was sich hier messen lässt, und für
alles Übrige die Messung dorthin verlegt, wo das Problem auftritt: auf
das Gerät des Nutzers.

**Was sich hier messen ließ, und was dabei herauskam.** Im
Entwickeln-Modus bei 1600×900 tragen **acht Glasflächen** gleichzeitig
einen `backdrop-filter` (Weichzeichnen plus Sättigung) **und** einen
SVG-Verzerrungsfilter aus `feTurbulence` + `feGaussianBlur` +
`feDisplacementMap`, zusammen über **1,08 Millionen Bildpunkte** — rund
drei Viertel des Fensters.

Die Moduldoku von `ui/GlassDistortionFilter.tsx` behauptete, ein
geteiltes `<filter id>` genüge für beliebig viele Flächen und es gebe
„keine Vervielfachung der teuren SVG-Filterberechnung pro Aufrufstelle".
Das ist falsch: geteilt wird die *Definition*, die *Auswertung*
geschieht pro Element über dessen eigene Fläche.

**Der Auslöser kam aus der Geburtstagsausgabe.** Solange hinter einer
Glasfläche nichts passiert, ist das Ergebnis zwischenspeicherbar und die
Kosten fallen einmal an. Die Luftballons (ADR-0071) bewegen sich
**dauerhaft** und liegen **hinter allem**. Ihre eigene Moduldoku
versichert, der Compositor bewege sie ohne Layout- oder
Zeichendurchlauf — das stimmt für die Ballons selbst und übersieht die
Folge: ein bewegtes Element hinter einer `backdrop-filter`-Fläche macht
deren Zwischenspeicher bei **jedem Bild** ungültig. Acht
Verzerrungsfilter werden damit sechzig Mal je Sekunde neu gerechnet, für
immer. Auf einer nicht hardwarebeschleunigten Webansicht reicht das.

**Entscheidung 1: Effektstufen, und `auto` misst wirklich.** Neue
Einstellung `effect_level` mit `auto` (Vorgabe), `full`, `reduced`,
`plain`. `reduced` nimmt die SVG-Verzerrung und die Dauerbewegung und
lässt den Weichzeichner — das ist der größte Gewinn bei kleinstem
optischen Verlust. `plain` nimmt auch den Weichzeichner und ersetzt die
Glas-Tönung durch deckende Flächen; schlicht heißt nicht unlesbar.

`auto` misst die tatsächliche Bildzeit auf dem Gerät. Die Messung hängt
sich dafür **selbst eine bewegte Last hinter die Glasflächen** — genau
das, was die Ballons dauerhaft tun. Ohne diese Last misst man den
Ruhezustand, und dann sieht jedes Gerät schnell aus, auch das, auf dem
es ruckelt.

Heruntergestuft wird, nie hinauf. Eine Automatik, die bei jeder ruhigen
Sekunde zurückschaltet und beim nächsten Ballon wieder herunter,
flackert — und ein flackerndes Fenster ist schlimmer als ein dauerhaft
schlichtes. Wer wieder mehr will, wählt die Stufe von Hand.

**Entscheidung 2: die App sagt selbst, ob sie optimiert läuft.** Der
häufigste und am schlechtesten sichtbare Grund für „alles ist zäh" ist
ein Debug-Build: wer mit `cargo tauri dev` startet, bekommt einen, und
die Bildberechnung dieser App besteht aus Rechnerei über Megapixel-
Arrays — genau die Art Code, die ohne Optimierung ein Vielfaches
verliert. Der neue Dialog „Leistung prüfen…" meldet das Build-Profil
(`cfg!(debug_assertions)`) als ersten und einzigen als Warnung
gestalteten Befund, mit dem konkreten Gegenmittel. Daneben misst er eine
vollständige Entwicklung über zwölf Megapixel und die Bildzeit der
Oberfläche, und stellt bei zäher Oberfläche die Effektstufe direkt um —
wer gerade erfahren hat, dass sein Fenster 15 Bilder je Sekunde schafft,
soll nicht erst den Einstellungsdialog suchen müssen.

**Entscheidung 3: Abhängigkeiten auch im Entwicklungsbuild voll
optimieren.** `[profile.dev] opt-level = 1` galt nur für den eigenen
Code. Die eigentliche Rechenarbeit steckt in fremden Kisten (`rawler`,
`image`, `zune-jpeg`, `rustfft`, `ndarray`) und lief damit ebenfalls auf
Stufe 1. Neu: `[profile.dev.package."*"] opt-level = 3`. Abhängigkeiten
werden einmal übersetzt und danach zwischengespeichert; die
Übersetzungszeit des eigenen Codes ändert sich nicht, nur der erste
Build dauert länger und das Zielverzeichnis wächst.

**Die Karte.** Der gemeldete leere Globus ist kein Fehler der Karte,
sondern ein fehlender Leerzustand. Die wenigsten Kameras haben einen
GPS-Empfänger — die Canon EOS, die diese Fassung als Standardmodell
führt, hat keinen. Die Anzeige „0 Fotos mit GPS" bestätigte den
Eindruck eines Defekts, ohne den Grund zu nennen oder den Weg zur
Abhilfe zu zeigen, obwohl beides vorhanden ist (GPX-Track, Standort von
Hand setzen).

Dabei fiel eine echte Testlücke auf: die Zusage, Aperture X lese
vorhandene Koordinaten auch aus Kamera-JPEGs, war für den RAW-Weg
geprüft, für den JPEG/TIFF-Weg nicht — `fallback.rs` hatte zu GPS kein
einziges Testfoto hinter sich. Drei neue Tests mit von Hand gebauten
APP1/EXIF-Blöcken decken das jetzt ab, einschließlich der Vorzeichen für
Süd und West (ohne sie landet ein Foto aus Buenos Aires in Russland) und
des Regelfalls ohne GPS-Block, der `None` ergeben muss und nicht (0, 0)
— sonst säße jedes Foto ohne Empfänger im Golf von Guinea.

**Was hier ausdrücklich NICHT behauptet wird.** Dass diese Änderungen
die vom Nutzer erlebte Zähigkeit beheben, ist nicht bewiesen — das kann
nur eine Messung auf seinem Gerät, und genau dafür ist der
Diagnose-Dialog da. Belegt ist, was belegt werden konnte: die Last der
Glasflächen in Zahlen, dass die Stufen sie nachweislich entfernen
(`e2e/effect-levels.spec.ts` misst die errechneten Eigenschaften, mit
der vollen Stufe als Gegenkontrolle), und dass der Diagnose-Dialog einen
Debug-Build als solchen benennt.

---

## ADR-0074: Die GPU-Beschleunigung hat nie funktioniert

**Anlass.** Nutzer-Log aus einem echten Lauf unter Windows, eingereicht
mit der Bitte, es auszuwerten. Darin, bei **jedem einzelnen** Rendern:

```
WARN apx_pipeline::develop: GPU-Rendering fehlgeschlagen, nutze CPU-Fallback
  In a dispatch command: Each current dispatch group size dimension
  ([107337, 1, 1]) must be less or equal to 65535
```

Nicht einmal hat die GPU durchgerechnet.

**Die Ursache.** `gpu/dispatch.rs` legte alle Workgroups in die
X-Dimension:

```rust
let workgroup_count = element_count.div_ceil(workgroup_size).max(1);
pass.dispatch_workgroups(workgroup_count, 1, 1);
```

WebGPU begrenzt jede Dimension auf
`max_compute_workgroups_per_dimension`, in aller Regel 65535. Bei 64
Elementen je Gruppe und drei Kanälen je Pixel liegt die Obergrenze
damit bei

> 65535 × 64 ÷ 3 = 1.398.080 Pixel, also knapp **1,4 Megapixel**.

Die Zahlen aus dem Log gehen exakt auf: 1911×860 → 77.038 Gruppen;
2256×1015 → 107.337; 3456×1555 → 251.910; 1361×3024 → 192.922. Jede
über dem Limit, jede Validierung abgelehnt, jedes Mal CPU-Fallback.

Betroffen waren **alle 14 Shader-Stufen**, weil alle durch denselben
Helfer laufen. Und betroffen war nicht nur das Exportieren grosser
Bilder: schon die verkleinerten Live-Vorschauen beginnen bei rund 1,6
Megapixeln. Die GPU-Beschleunigung dieser App hat damit für kein
einziges realistisches Foto je gearbeitet.

**Warum es niemandem auffiel.** Drei Dinge haben zusammengewirkt, und
jedes einzelne hätte den Fund verhindert:

1. **Die Fehlermeldung zeigte in die falsche Richtung.** Jeder
   Validierungsfehler im `push_error_scope`-Bereich wurde als
   `PipelineError::ShaderCompile` gemeldet — „Shader 'basic_fused'
   konnte nicht kompiliert werden". Der Shader war tadellos übersetzt;
   erst der Aufruf scheiterte. Wer das las, suchte im WGSL-Quelltext,
   wo nichts zu finden war.
2. **Der Fallback war still.** Er griff, das Foto erschien, nur eben
   langsamer. Für den Nutzer gab es nichts zu sehen — nur eine App,
   die „irgendwie zäh" war.
3. **Kein Test ging je über 1,4 Megapixel.** `add_offset_roundtrip…`
   arbeitete mit 256 Elementen, die Stufen-Tests mit winzigen
   Bildern. Alle grün, alle unterhalb der Grenze.

**Die Entscheidung.** Der Dispatch füllt X bis zum **vom Gerät
gemeldeten** Limit und legt den Rest in Y. Jeder Shader nimmt
`@builtin(num_workgroups)` entgegen und bildet den Index als
`gid.x + gid.y * groups.x * 64u`. Die Aufteilung ist lückenlos: Zeile
`y` deckt `[y·gx·64, (y+1)·gx·64)` ab, also genau anschliessend an die
vorige. Die letzte Zeile ist meist nur teilweise belegt; das fängt die
`arrayLength`-Schranke ab, die ohnehin in jedem Shader steht.

Das Gerätelimit wird abgefragt statt 65535 anzunehmen — auf einem
Adapter, der mehr kann, bleibt der Dispatch dann eindimensional.

**Was dafür sorgt, dass es nicht wiederkommt.** Vier Ebenen, weil eine
nicht gereicht hat:

- `workgroup_grid_stays_within_the_per_dimension_limit` rechnet mit
  **genau den Gruppenzahlen aus dem Nutzer-Log** und prüft vorab, dass
  die eigene Rechnung dieselbe ist wie die des Geräts — sonst prüfte
  der Test einen anderen Fall als den gemeldeten.
- `a_buffer_past_the_old_limit_round_trips_through_the_gpu` schickt
  einen Puffer über der Grenze wirklich durch die GPU und prüft das
  **letzte** Element, das in der zweiten Dispatch-Zeile liegt. Die
  Grösse leitet sich aus dem echten Gerätelimit ab, damit der Test auf
  jedem Adapter den zweidimensionalen Pfad trifft. Gegenkontrolliert:
  mit zurückgedrehtem Shader-Index schlägt er fehl mit „Element
  4194302 von 4194304 wurde nicht berechnet".
- `shader_contract_tests` liest alle `.wgsl`-Dateien als Text und
  verlangt `@builtin(num_workgroups)`, die Zweidimensionalität und
  dass die Zeilenbreite zur `@workgroup_size`-Deklaration passt. Ein
  neuer Shader kann den Fehler nicht still wieder einführen. Unelegant,
  aber die einzige Stelle, an der sich der Vertrag ohne vierzehn
  GPU-Läufe prüfen lässt.
- Die Leistungs-Diagnose (ADR-0073) misst jetzt **beide** Pfade. Sie
  führt dafür zuerst eine Stufe direkt auf der GPU aus, denn
  `render_rgba8` fängt einen gescheiterten GPU-Aufruf selbst ab — eine
  Diagnose, die das nicht umgeht, meldete CPU-Zeiten als GPU-Zeiten
  und hätte denselben Befund ein zweites Mal verfehlt. Der Probelauf
  nimmt das volle Messbild und keine kleine Probe, denn genau eine
  kleine Probe hätte „GPU geht" gemeldet.

**Die zweite Decke, direkt dahinter.** Beim Absichern der eigenen
Korrektur fiel auf, dass unmittelbar hinter dem Gruppenlimit das
nächste wartete: `wgpu::Limits::default()` deckelt
`max_storage_buffer_binding_size` auf **128 MiB**. Bei drei `f32` je
Pixel sind das

> 134.217.728 ÷ 12 = 11.184.810 Pixel, also gut **11 Megapixel**.

Jedes größere Foto wäre am GPU-Pfad vorbeigelaufen — diesmal an der
Puffergröße statt an der Gruppenzahl, und mit demselben stillen
Fallback. Im Nutzer-Log steht dieser Fall nicht, weil dort nur
Vorschauen bis 7,2 Megapixel vorkamen; bei einem Export in voller
Auflösung wäre er sofort aufgetreten.

Den Fund hier liegen zu lassen, hätte geheissen, eine halbe Korrektur
auszuliefern und den nächsten Bericht abzuwarten. Deshalb fordert der
Kontext jetzt `adapter.limits()` an statt der Vorgabewerte. Die
Vorgabewerte sind auf den kleinsten gemeinsamen Nenner inklusive
Mobilgeräten ausgelegt; was ein Adapter wirklich kann, meldet er
selbst, und mehr als das lässt sich per Definition nicht anfordern.
`the_context_requests_what_the_adapter_can_do` hält fest, dass hier
nicht wieder nach unten gedeckelt wird. Die Diagnose prüft die
Puffergröße zusätzlich vorab und nennt sie im Klartext, statt einen
kryptischen Validierungsfehler durchzureichen.

**Und die Fehlermeldung.** Neue Variante `PipelineError::GpuExecution`
für abgelehnte Aufrufe, getrennt von `ShaderCompile` für echte
Übersetzungsfehler. Ein Test hält fest, dass ein Ausführungsfehler
nicht nach einem Übersetzungsfehler klingen darf. Eine Meldung, die in
die falsche Richtung zeigt, ist schlechter als eine vage.

**Was das für das Tempo heisst — und was nicht.** Dass der GPU-Pfad
nun greift, ist belegt. Um wie viel das auf dem Gerät des Nutzers
schneller ist, ist es nicht: das misst nur sein Rechner, und genau
dafür steht der Vergleich jetzt im Diagnose-Dialog. Aus demselben Log
geht ausserdem hervor, dass die **Dekodierung** mit 1,2 bis 1,9
Sekunden den grösseren Posten stellt; das Rendern lag bei 0,2 bis 0,8
Sekunden. Die Dekodierung ist von dieser Änderung unberührt — sie
profitiert von `[profile.dev.package."*"] opt-level = 3` (ADR-0073)
und von einem Release-Build, nicht von der GPU.

---

## ADR-0075: Jeder Stufenaufruf baute die halbe GPU neu auf

**Anlass.** Nach ADR-0074 lief der GPU-Pfad endlich — und war trotzdem
kaum schneller. Die Leistungs-Diagnose auf dem Gerät des Nutzers:

> Entwickeln, neutral: 939 ms für 12 Megapixel · **Entwickeln auf der
> Grafikkarte: 784 ms (1,2× schneller als die CPU)**

Eine GeForce MX350 rechnet zwölf Megapixel pro Pixel-Operation in
einstelligen Millisekunden. 784 ms heisst: die Zeit geht fast
vollständig woanders hin.

**Gemessen, nicht vermutet.** Aufschlüsselung eines einzelnen
Stufenaufrufs bei 11,2 Megapixeln:

| Posten | Zeit |
| --- | --- |
| Shader übersetzen | 0,2–0,6 ms |
| Pipeline bauen | 110 ms erstmalig, danach 0,4 ms |
| **Puffer anlegen + hochladen** | **1,5–2,8 s** |
| Absenden | 106–211 ms |
| Zurücklesen | 87–110 ms |

`run_compute_f32` legte bei **jedem** Aufruf alles neu an: WGSL neu
übersetzen, Compute-Pipeline neu bauen, vier Puffer frisch belegen,
drei davon in voller Bildgröße. Bei vierzehn Stufen und jedem
Regler-Tick.

**Die Entscheidung.** Ein Arbeitsbereich (`gpu/workspace.rs`) im
`GpuContext` hält Pipelines, Bind-Groups und Puffer über den einzelnen
Aufruf hinaus. Puffer wachsen bei Bedarf und schrumpfen nie —
Vorschaugrößen wechseln im Betrieb ständig (Ziehen 1280, Loslassen
volle Anzeigegröße), und bei jedem Wechsel neu zu belegen wäre genau
der Aufwand zurück. Hinter einem Mutex, weil zwei gleichzeitige
Renderläufe sich sonst dieselben Puffer teilen würden; das ist ein
Datenfehler, kein Tempoproblem, und serialisieren kostet nichts, was
auf einem einzelnen Gerät nicht ohnehin verloren wäre.

**Warum genau diese Stufe und nicht gleich mehr.** Drei Varianten
wurden bei 2 Megapixeln gegeneinander gemessen, eine Stufe, bester von
drei Läufen:

| Weg | Zeit |
| --- | --- |
| alles pro Aufruf neu (vorher) | 228 ms |
| **Pipeline + Puffer wiederverwendet** | **33 ms** |
| zusätzlich das Bild GPU-resident | 13 ms |

Die mittlere Zeile bringt den Löwenanteil und berührt weder die
vierzehn Stufen noch den Renderpfad — die untere verlangt, dass
Zwischenergebnisse gar nicht erst zur CPU zurückwandern, und das ist
ein Umbau des Kernpfads eines Bildeditors. Die zweieinhalbfache
Differenz dazwischen rechtfertigt diesen Umbau nicht am Ende einer
Sitzung.

**Am echten Renderpfad nachgemessen**, gleicher Adapter, 2 Megapixel:

| | vorher | nachher |
| --- | --- | --- |
| GPU, neutral | 136,6 ms | **80,9 ms** |
| GPU, mit Reglern | 366,0 ms | **256,2 ms** |
| CPU (Kontrolle) | 72,3 / 241,8 ms | 67,6 / 248,9 ms |

Die CPU-Werte bewegen sich nicht — die Änderung betrifft nur den
GPU-Pfad, und genau das soll die Kontrollzeile zeigen.

**Was den Fehler festhält.** Eine Zeitmessung wäre hier der
naheliegende, aber falsche Test: sie hängt an der Maschine und würde
mal grün, mal rot. `repeated_calls_reuse_pipeline_and_buffers` prüft
stattdessen die Tatsache dahinter — fünf Aufrufe derselben Stufe
erzeugen genau eine Pipeline und genau eine Pufferbelegung.
Gegenkontrolliert: mit abgeschalteter Wiederverwendung schlägt er fehl.

Dazu `a_stage_name_reused_with_another_shader_gets_a_new_pipeline`: der
Zwischenspeicher ist nach dem Stufennamen geschlüsselt, und ein Name,
der später mit anderem Shader aufgerufen wird, darf nicht die alte
Pipeline bekommen. Das wäre der unangenehmste Fehler überhaupt, weil
das Ergebnis plausibel aussähe.

**Nebenbei gelernt:** bei `layout: None` leitet wgpu das
Bind-Group-Layout aus dem Quelltext ab und **streicht eine Bindung,
die der Shader nie liest**. Ein Teststufen-Shader ohne `params`-Zugriff
scheiterte deshalb mit „3 Bindungen gegen ein Layout mit 2". Alle
echten Stufen lesen ihre Parameter; der Test tut es jetzt auch.

**Was offen bleibt.** Die dritte Zeile der Tabelle oben: das Bild
einmal hochladen, alle Stufen darauf laufen lassen, einmal
herunterladen — und es zwischen den Regler-Ticks oben lassen, denn
beim Ziehen ändert sich die Quelle nicht. Das ist der Unterschied
zwischen 33 und 13 Millisekunden je Stufe, und bei mehreren aktiven
Stufen deutlich mehr, weil jede Stufe heute noch das ganze Bild
hin- und herschiebt. Es verlangt einen `GpuChain`-Typ, eine
Buffer-Variante jeder der vierzehn Stufen und einen Paritätstest
GPU-Kette gegen CPU — eigenständige Arbeit, nicht ein Anhängsel.

---

## ADR-0076: Die Effekt-Automatik lief nie — und die Vorgabe war die teuerste Stufe

**Anlass.** Nutzer-Rückmeldung nach ADR-0075, unmissverständlich: „DAS
KANN NICHT DER HAUPTGRUND SEIN, ES IST IMMER NOCH LANGSAM." Zu Recht.
Die Pufferwiederverwendung brachte 136 → 81 ms je Vorschau; das
erklärt weder „4216 ms" noch verspätet angenommene Klicks.

**Die Zahl, die ich zu schnell abgehakt hatte.** Im Diagnose-Screenshot
stand, rot:

> Bildzeit der Oberfläche: **83 ms (12 Bilder/s)**

Zwölf Bilder je Sekunde **ist** „Klicks werden spät registriert". Ein
Klick wartet im schlechtesten Fall 83 Millisekunden allein auf den
Bildtakt. Das ist kein Durchsatz-, sondern ein Reaktionsproblem, und es
hat mit der Bildrechnung nichts zu tun.

**Der Fehler, und er war meiner.** Die Automatik aus ADR-0073 setzte
ihr „schon gemessen"-Flag, bevor sie mass:

```ts
effectProbeDone.current = true;            // sofort
const timer = setTimeout(…, 2500);         // misst erst in 2,5 s
return () => clearTimeout(timer);          // Abhängigkeit [uiSettings]
```

Ändert sich `uiSettings` in diesen 2,5 Sekunden — beim Start der
Regelfall —, räumt die Aufräumfunktion den Timer ab, der Effekt läuft
erneut und steigt in der ersten Zeile sofort wieder aus. **Die Messung
lief nie**, für den Rest der Sitzung. Die App blieb auf der vollen
Stufe stehen.

Behoben an beiden Enden: das Flag wird erst gesetzt, wenn wirklich
gemessen wird, und die Abhängigkeitsliste enthält nur noch das eine
Feld, auf das es ankommt, statt des ganzen Einstellungsobjekts.

**Die zweite Hälfte: die Vorgabe war falsch herum.** Selbst mit
laufender Automatik startete sie bei `full`. Und `full` ist, gemessen
(ADR-0073), die teuerste denkbare Konfiguration dieser App:

- acht Glasflächen mit `backdrop-filter` **und**
  SVG-`feDisplacementMap` über 1,08 Millionen Bildpunkten,
- und dahinter, als Vorgabe eingeschaltet, dauerhaft steigende
  Luftballons, die den Zwischenspeicher dieser acht Filter bei **jedem
  Bild** ungültig machen.

Das war die Auslieferungs-Vorgabe. Niemand hat sie gewählt; sie war
einfach da.

`auto` beginnt deshalb jetzt bei `reduced` und geht von dort nur noch
weiter herunter. Die Verzerrung ist ein dekorativer Feinschliff, der
mehr kostet als alles andere in der Oberfläche zusammen, und der
Unterschied zur reduzierten Stufe ist im Betrieb kaum zu sehen. Wer ihn
trotzdem will, wählt „Voll" — bewusst, und mit der Kostenangabe in der
Beschriftung.

**Was das festhält.** `Stufe „automatisch" lässt nie die teuerste Stufe
stehen` prüft nicht, OB gemessen wurde, sondern was dabei herauskommen
muss: bei `auto` darf `data-effects` nie `full` sein, und die
Verzerrung und die Ballons müssen dann auch wirklich weg sein — sonst
stünde nur ein anderes Wort im Attribut. Gegenkontrolliert: mit dem
alten Startwert schlägt der Test fehl.

Dazu zeigt die Leistungs-Diagnose jetzt die **tatsächlich gezeichnete**
Stufe, aus dem DOM gelesen statt aus der Einstellung abgeleitet. Genau
diese Unterscheidung fehlte: die Einstellung sagte „automatisch", und
niemand konnte sehen, dass trotzdem alles an war.

**Selbstkritik.** Ich hatte in ADR-0075 eine echte, gemessene
Verbesserung gefunden und sie für die Antwort gehalten. Sie war eine
Teilantwort auf die falsche Frage: ich habe den Durchsatz der
Bildrechnung optimiert, während die Beschwerde die Reaktionszeit der
Oberfläche betraf — und die Zahl dafür stand die ganze Zeit rot im
selben Screenshot. Eine gemessene Verbesserung ist kein Beleg dafür,
dass man das richtige Problem gemessen hat.

**Was weiterhin offen ist.** Die 939 ms für zwölf Megapixel auf der CPU
und 784 ms auf der GPU sind damit nicht erklärt — dafür steht die
GPU-Residenz aus (ADR-0075, „Was offen bleibt"). Und der Debug-Build
bleibt der Faktor, gegen den alles andere ankämpft.

---

## ADR-0077: Beim Reglerziehen rechneten dutzende überholte Läufe zu Ende

**Anlass.** Dieselbe Rückmeldung wie bei ADR-0076, und der Teil, den
ADR-0076 nicht erklärt: „letztes Rendering 4216 ms". Die Oberfläche war
nach der Effekt-Korrektur flüssig, das Rendern blieb zäh.

**Der Fund.** Der `apx://`-Handler startet pro Anfrage einen eigenen
Betriebssystem-Thread:

```rust
std::thread::spawn(move || {
    let response = handle(&app_handle, &request, &cache);
    responder.respond(response);
});
```

Unbegrenzt viele, ohne Warteschlange, ohne Abbruch. Das Frontend
entprellt zwar auf einen Frame und bricht die vorige Anfrage per
`AbortController` ab — aber das beendet nur die `fetch` im Webview.
**Der Thread in Rust rechnet weiter bis zum Ende.**

Nach einer Sekunde Ziehen laufen also dutzende vollständige Renderläufe
gleichzeitig. Sie kämpfen um dieselben Kerne und dieselbe einzelne
Grafikkarte; seit ADR-0075 stehen sie dort zusätzlich hintereinander am
selben Puffersatz. Das Ergebnis, auf das der Nutzer wartet, ist das
letzte in der Schlange.

**Der Mechanismus war entworfen und nie gebaut.**
`PipelineError::Cancelled` trägt seit jeher den Kommentar „Eine
Rendering-Operation wurde abgebrochen (z. B. weil eine neuere Anfrage
für dasselbe Foto sie überholt hat)" — und wird in der gesamten
Codebasis **nirgends erzeugt**. Eine Fehlervariante ohne Erzeuger ist
eine Absichtserklärung, keine Funktion.

**Die Entscheidung.** Jede Entwickeln-Anfrage zieht eine Ticketnummer
für ihr Foto. Die eigentliche Rechenarbeit ist serialisiert; wer
drankommt, prüft zuerst, ob inzwischen eine neuere Anfrage für dasselbe
Foto gezogen hat, und steigt dann aus, ohne ein einziges Pixel zu
rechnen. Aus sechzig Anfragen während eines Ziehvorgangs rechnet genau
eine.

**Warum je Foto und nicht global.** Der Filmstreifen fordert Vorschauen
für mehrere Fotos gleichzeitig an. Ein globales Ticket würde die alle
gegenseitig verhungern lassen; je Foto bleibt jede dieser Anfragen die
neueste für ihr eigenes Foto.

**Warum serialisiert.** Es gibt eine Grafikkarte. Parallele Läufe haben
sich dort bisher nur gegenseitig ausgebremst — und seit ADR-0075 teilen
sie sich sogar dieselben Puffer, was ohne Serialisierung ein Datenfehler
wäre, kein Tempoproblem.

**Nur Entwickeln-Anfragen.** Miniaturbilder und Vorschauen sind viele
unabhängige Anfragen, die einander nicht überholen; sie gehen an der
Schranke vorbei.

**Die Antwort für einen überholten Lauf** ist `409 Conflict`, kein
Fehler: fachlich ist nichts schiefgegangen, das Ergebnis ist nur nicht
mehr gefragt. Das Frontend verwirft sie still, statt sie als
Rendering-Fehler zu protokollieren.

**Was das festhält.** `a_burst_lets_only_the_newest_through` bildet den
Ziehvorgang nach: aus sechzig Tickets für dasselbe Foto darf genau
eines durchkommen. `requests_for_different_photos_do_not_supersede_each_other`
hält den Filmstreifen-Fall fest, und `the_turn_is_exclusive` lässt zwei
Threads fünfzig Runden gegeneinander laufen und prüft, dass nie zwei
gleichzeitig in der Rechenphase sind.

**Was damit NICHT behauptet wird.** Ein einzelner Renderlauf wird davon
nicht schneller. Was wegfällt, ist die Schlange davor — und die war der
Unterschied zwischen „das Bild kommt gleich" und „4216 ms". Die Dauer
eines einzelnen Laufs bleibt die Sache von ADR-0075s offenem Punkt (das
Bild zwischen den Stufen auf der GPU lassen) und des Release-Builds.

---

## ADR-0078: Die Luftballons waren so gut wie nie zu sehen

**Anlass.** Nutzer-Rückmeldung: „ehrlich gesagt sehe ich die
Luftballons gar nicht — habe ich auch nie, obwohl ich zweimal gepullt
habe."

Das widersprach direkt meiner Erklärung aus ADR-0073 und ADR-0076, in
der die dauerhaft steigenden Ballons als Auslöser für die zähe
Oberfläche figurierten. Wenn sie nie zu sehen waren, stimmt diese
Erklärung so nicht.

**Zwei Prüfungen, die nicht taugten — beide meine.**

1. `document.elementFromPoint` auf der Mitte jedes Ballons lieferte
   `MAIN`, `FOOTER`, einen `BUTTON`. Das sah nach einem Beweis aus und
   war keiner: die Schicht trägt `pointer-events: none`, der Strahl
   geht also **immer** hindurch und liefert, was darunter liegt —
   unabhängig davon, was man sieht.
2. „Zwei Aufnahmen unterscheiden sich" bestand auch **mit** dem Fehler.
   Ein Test, der mit und ohne Korrektur grün ist, behauptet nichts.

**Die Messung, die etwas sagt.** Allein die Ballonschicht unsichtbar
schalten — derselbe Zustand, dieselbe Begrüßung, dieselbe
Animationsphase, eine Variable — und die geänderten Bildpunkte zählen:

| | sichtbare Ballon-Bildpunkte | Anteil bei 1600×900 |
| --- | --- | --- |
| vorher (`z-index: 0`) | 2.462 | **0,17 %** |
| jetzt (`z-index: 35`) | 8.909 | **0,62 %** |

Sechs Ballons hätten als volle Fläche rund 12.000 Bildpunkte. Vorher
waren also gut vier Fünftel jedes Ballons von deckenden Flächen
verdeckt, und der Rest lag bei 22 % Deckkraft. Das ist genau das, was
der Nutzer als „sehe ich nicht" beschreibt.

**Die Ursache.** Im CSS stand `z-index: 0` mit der Begründung „hinter
dem Inhalt, damit kein Ballon über einem Foto schwebt". Die Absicht war
richtig, die Umsetzung konnte nicht aufgehen: der Inhalt dieser App ist
**deckend** (`bg-bg-base`, `bg-bg-panel`). „Hinter dem Inhalt" heisst
dann nicht „dezent im Hintergrund", sondern „nicht vorhanden". Beides
zugleich gibt es nicht.

**Die Entscheidung.** `z-index: 35` löst drei Dinge auf einmal:

- über dem Inhalt und über der Kopfzeile (`z-30`), also wirklich zu
  sehen;
- unter Menüs (`z-40`) und Dialogen (`z-50`), also nie im Weg;
- und — der Punkt, der für das Tempo zählt — **vor** der Kopfzeile statt
  dahinter. Ein bewegtes Element hinter einer `backdrop-filter`-Fläche
  macht deren Zwischenspeicher bei jedem Bild ungültig; davor kostet es
  nichts.

Sie schweben jetzt tatsächlich über dem Foto. Bei 22 % Deckkraft ist
das der Preis dafür, dass es die Deko überhaupt gibt; wem das beim
Beurteilen im Weg ist, schaltet sie in den Einstellungen ab.

Weil sie der Kopfzeile nicht mehr in den Weg kommen, dürfen sie in der
Stufe „reduziert" bleiben — sie waren der Grund, sie dort zu
verstecken, und der Grund ist weg. Mit der neuen Vorgabe aus ADR-0076
sieht der Nutzer die Deko damit zum ersten Mal.

**Korrektur an ADR-0073 und ADR-0076.** Dort steht, die Ballons machten
„den Zwischenspeicher dieser acht Filter bei jedem Bild ungültig". Das
war überzogen: sie lagen hinter der Kopfzeile, nicht hinter allen acht
Glasflächen, und die meisten von ihnen waren ohnehin verdeckt. Dass sie
Kosten verursachten, bleibt richtig; dass sie *der* Auslöser der 83
Millisekunden waren, ist nicht belegt. Belegt ist der Fehler in der
Automatik (ADR-0076) — die lief nachweislich nie.

**Selbstkritik.** Ich habe in dieser Frage zweimal hintereinander eine
Messung für einen Beweis gehalten, die keiner war: einmal eine, die
durch das geprüfte Element hindurchgriff, einmal eine, die mit und ohne
Fehler grün war. Beide Male hätte eine Gegenkontrolle das sofort
gezeigt — und beide Male habe ich sie erst gemacht, nachdem der Nutzer
widersprochen hat. Die Gegenkontrolle gehört vor die Schlussfolgerung,
nicht hinter den Einspruch.

---

## ADR-0079: `pnpm install` scheiterte auf jedem frischen Rechner

**Anlass.** Nutzer folgte der Bauanleitung aus dem README und kam nicht
über den ersten Befehl hinaus:

```
Error: ERR_PNPM_IGNORED_BUILDS
  × installing dependencies
  ╰─▶ Ignored build scripts: esbuild@0.25.12
```

davor, als Warnung:

```
The "pnpm" field in package.json is no longer read by pnpm.
The following keys were ignored: "pnpm.onlyBuiltDependencies".
```

**Die Ursache.** pnpm 10 führt Installationsskripte standardmäßig nicht
mehr aus und bricht ab, wenn eines übersprungen wurde. `esbuild`
braucht seines — es verknüpft die plattformeigene Binärdatei, und ohne
die scheitert jeder Vite-Build.

Die Freigabe dafür war korrekt hinterlegt, nur an der Stelle, die pnpm
10 nicht mehr liest: unter `"pnpm"` in `frontend/package.json`. pnpm
meldet das als Warnung und bricht trotzdem ab. Die Absicht war da, die
Wirkung nicht.

**Warum es niemandem auffiel.** Auf einem Rechner mit bereits
installierten `node_modules` tritt der Fehler nicht auf — nur bei einer
frischen Installation. In dieser Entwicklungsumgebung lagen sie vor;
der Fehler liess sich hier erst nach `rm -rf node_modules`
reproduzieren. Die CI pinnt `PNPM_VERSION: "10"`, also dieselbe
Hauptversion — sie war ebenso betroffen, sobald ihr Zwischenspeicher
kalt lief.

Das ist dieselbe Fehlerklasse wie der Rest dieser Sitzung: eine
Einstellung, die dasteht, aussieht als wirke sie, und nichts tut.

**Die Entscheidung.** Die Liste steht jetzt in
`frontend/pnpm-workspace.yaml` unter `onlyBuiltDependencies` — der
Stelle, die pnpm 10 liest. Der Eintrag in `package.json` ist entfernt,
nicht zusätzlich belassen: zwei Orte für dieselbe Aussage, von denen
einer stillschweigend ignoriert wird, sind genau das Problem, das hier
behoben wird. Der Kommentar in `ci.yml`, der auf die alte Stelle
verwies, zeigt jetzt auf die neue.

**Verifiziert**, nicht angenommen: `rm -rf node_modules` gefolgt von
`pnpm install --frozen-lockfile` läuft durch, die esbuild-Binärdatei
liegt danach an ihrem Platz, und `vite build` baut.

---

## ADR-0080: Die Schranke aus ADR-0077 hat es verdoppelt

**Anlass.** Nutzer nach dem Einspielen von ADR-0077: „warte mal, DU
HAST ES LANGSAMER GEMACHT! Letztes Rendering: 8879 ms." Vorher waren es
4216 ms.

**Was falsch war.** ADR-0077 liess die Entwickeln-Läufe der Reihe nach
durch, mit der Begründung, es gebe ohnehin nur eine Grafikkarte und
parallele Läufe bremsten sich dort gegenseitig aus. Das klang
vernünftig und war es nicht:

> **Eine Reihenfolge bricht den laufenden Lauf nicht ab.**

Die neueste Anfrage wartete also erst darauf, dass ein längst
veralteter Lauf zu Ende rechnet, und rechnete danach selbst. Aus „so
lange wie ein Rendern" wurde „so lange wie zwei". Parallel war für die
Anfrage, auf die der Nutzer wartet, schneller — auch wenn dabei
Rechenzeit verschwendet wurde.

Die Ticketprüfung war richtig. Das Schloss davor war der Fehler.

**Die Korrektur.** Die Schranke prüft, **ohne zu warten**. Wer beim
Start schon überholt ist, steigt sofort aus; wer es nicht ist, fängt
sofort an. Kein Schloss, keine Reihenfolge.

Das genügt, weil die Zahlen dafür sprechen: beim Ziehen kommen rund
sechzig Anfragen je Sekunde, ein Rendern dauert hunderte
Millisekunden. Bis ein Lauf überhaupt beginnt, haben also längst
mehrere neuere Anfragen gezogen — er steigt aus, bevor er etwas tut.
Übrig bleibt die jeweils neueste, und die läuft, ohne etwas vor sich in
der Schlange zu haben.

**Was das festhält.** `the_check_never_blocks_a_caller` lässt einen
Aufrufer so tun, als rechne er lange, und verlangt, dass ein zweiter
trotzdem in unter 100 Millisekunden durchkommt. Gegenkontrolliert: mit
250 Millisekunden Verzögerung in der Prüfung schlägt er fehl und nennt
die gemessene Wartezeit.

Der Test aus ADR-0077, der die *Ausschliesslichkeit* festhielt
(`the_turn_is_exclusive`), ist ersatzlos entfallen — er hielt genau das
fest, was der Fehler war.

**Selbstkritik.** Das ist in dieser Sitzung die dritte Korrektur an
einer eigenen Schlussfolgerung, und die teuerste: ADR-0075 optimierte
am falschen Problem vorbei, ADR-0078 korrigierte eine überzogene
Ursachenbehauptung, und hier war die Änderung nicht nur unvollständig,
sondern **schädlich**. Gemeinsam ist allen dreien, dass ich eine
plausible Begründung für eine Messung gehalten habe. „Es gibt nur eine
Grafikkarte, also schadet Parallelität" ist eine Begründung. Was die
Wartezeit der neuesten Anfrage tut, wäre eine Messung gewesen — und
die hätte vor dem Ausliefern passieren müssen, nicht beim Nutzer.

**Was weiterhin offen ist.** Ein einzelner Renderlauf bleibt
langsam; daran ändert diese Korrektur nichts, sie stellt nur den
Zustand vor ADR-0077 wieder her und spart die Arbeit für Anfragen, die
schon beim Start veraltet sind. Der eigentliche Hebel bleibt ADR-0075s
offener Punkt: das Bild zwischen den Stufen auf der GPU lassen.

---

## ADR-0081 — Die Luftballons sind ersatzlos entfallen

**Status:** entschieden. **Ersetzt** ADR-0071s Ballon-Schicht und
beendet die Korrekturreihe aus ADR-0073, ADR-0076 und ADR-0078.

**Die Rückmeldung.** „Mach die verdammten Luftballons weg, man sieht
sie eh immer noch nicht." Nach ADR-0078, also nach einer gemessenen
Korrektur, die 2.462 geänderte Bildpunkte auf 8.909 hob und mit einem
gegenkontrollierten Test festgehalten war.

**Was das heisst.** Der Test war nicht falsch — 8.909 Bildpunkte
ändern sich nachweislich, und ohne die Korrektur schlägt er mit 2.461
fehl. Er misst nur die falsche Sache. 8.909 von 1.440.000 sind 0,62 %
des Fensters, verteilt auf sechs Formen bei 22 % Deckkraft über einem
Hintergrund, der farblich in derselben Familie liegt. Das ist ein
messbarer Unterschied und kein **sichtbarer**. „Ein Pixelzähler sagt
ja" und „ein Mensch sieht es" sind zwei verschiedene Aussagen, und ich
habe die erste als Beleg für die zweite genommen.

**Die Entscheidung.** Die Schicht ist weg, nicht noch einmal
reparierbar gemacht:

- `frontend/src/components/BalloonLayer.tsx` gelöscht,
- `.apx-balloon-layer`, `.apx-balloon`, `.apx-balloon::after` und
  `@keyframes apx-balloon-rise` aus `index.css` entfernt,
- die Einbindung in `App.tsx` entfernt,
- der Pixelzähl-Test aus `e2e/birthday-flow.spec.ts` und das Feld
  `sichtbareBallons` samt vier Zusicherungen aus
  `e2e/effect-levels.spec.ts` entfernt,
- die Oberflächentexte in `de.ts`/`en.ts` nennen jetzt nur noch
  Konfetti und die Begrüßung.

**Warum ersatzlos und nicht noch ein Versuch.** Zwei Anläufe
(`z-index: 0` → `z-index: 35`, dazu Freigabe für die Stufe
„reduziert") haben an der Wahrnehmung des Nutzers nichts geändert. Ein
dritter hätte die Ballons deutlich grösser, deckender oder schneller
machen müssen — und damit genau das, was ADR-0071 bewusst nicht wollte:
Deko, die den Blick vom Foto wegzieht. Eine Dekoration, die
unsichtbar sein muss, um nicht zu stören, hat keinen Zustand, in dem
sie ihren Zweck erfüllt.

Konfetti und Begrüßung bleiben unverändert. Die sieht der Nutzer; sie
waren nie Teil der Beschwerde.

**Was dabei auch wegfällt.** Die Begründungskette „bewegte Deko hinter
den Glasflächen macht die Verzerrungsfilter teuer" stützte sich auf die
Ballons als Dauerbewegung. Mit ihnen fällt die einzige **dauerhafte**
Bewegung hinter den Glasflächen weg; Konfetti fliegt nur bei fertigem
Import und Export. An den Effektstufen ändert das nichts: die Messung
in `probeFrameTimes` hängt ihre eigene Last ein (ADR-0073) und hat die
Ballons nie gebraucht. Die Moduldoku in `effects.ts` nennt die Kosten
jetzt allgemein, ohne sie an die Ballons zu binden.

**Selbstkritik.** Vierte Korrektur an einer eigenen Schlussfolgerung in
dieser Sitzung, und die lehrreichste: diesmal war die Messung sauber,
gegenkontrolliert und vollständig dokumentiert — und trotzdem war der
Schluss falsch, weil die Messgrösse nicht die Frage beantwortet hat.
Die Frage war „sieht der Nutzer das?". Der Test hat „ändert sich etwas
im Bild?" beantwortet. Wenn eine Beschwerde über Wahrnehmung mit einer
Zahl beantwortet wird, muss die Zahl zur Wahrnehmung passen, nicht nur
zum Code.


---

## ADR-0082: Token-Audit — verbindliche Arbeitsregeln, Projekt-Einstellungen, geteilte Entscheidungsdatei

**Status:** entschieden.

**Der Anlass.** Ein Audit der Sitzungstranskripte hat gemessen, was eine
Sitzung dieses Projekts kostet. **Eine** Sitzung mit 1.649 Turns:
674.751.012 Tokens Kontext-Neulesung, im Schnitt 409.188 je Turn, Spitze
783.223, drei Kompaktierungen. Dazu 918 Bash-Aufrufe und 850.095 Zeichen
Werkzeugausgaben.

**Der überraschende Teil.** Die Fixkosten sind nicht das Problem:
Systemprompt 10.100, Werkzeuge 17.700, MCP 16.500 + 6.900,
Skill-Beschreibungen 7.400 — zusammen 58.600 Tokens je Turn, also 5,9 %
eines 1-Mio.-Fensters. Die als „42 %" ausgewiesenen deferred
MCP-Werkzeuge (422.200) zählen gar nicht mit; nachgerechnet: 173.200
Gesamt = 58.600 Fix + 114.600 Verlauf. Wer MCP-Server abschaltet, um
422k zu sparen, spart 23k.

Das Problem ist die **Sitzungslänge**. Die Kosten einer Sitzung sind
ungefähr `Turns × Ø-Kontext`, und beide Faktoren wuchsen ungebremst.

**Was dabei auch auffiel.** Es gab **keine** `CLAUDE.md` — 623 Dateien,
13 Crates, null Zeilen verbindlicher Regeln. Jede Sitzung hat die
Konventionen per Gespräch neu etabliert: Sprache, Prüfbefehle, die
Tauri-Kommandozeile, die Plattenquote, die Messdisziplin. Dazu 16mal
„Erneut versuchen" und vier Korrekturen an eigenen Schlussfolgerungen
(ADR-0075, ADR-0078, ADR-0080, ADR-0081) — jede eine vollständige
Nacharbeitsschleife bei 409k Kontext je Turn.

**Die Entscheidung.** Vier Dinge, alle ohne Einfluss auf die
Funktionalität der App:

1. **`CLAUDE.md` im Wurzelverzeichnis**, verbindlich und unter 150
   Zeilen: Sprache, sparsame Arbeitsweise, die Verifikationsregeln, die
   Befehle in Kurzform, die Liste der Dateien, die nie vollständig
   gelesen werden, die Dokumentationspflicht. Alles Ausführliche steht
   in `docs/` und wird nur bei Bedarf gelesen — `ARBEITSWEISE.md`
   (Verifikation, Gegenkontrolle, Sitzungsführung), `BEFEHLE.md` (jeder
   Befehl mit gemessener Ausgabelänge), `UMGEBUNG.md` (Plattenquote,
   pnpm, Tauri-CLI, Playwright, Windows, Tailwind, GPU-Grenzen).
   `docs/CLAUDE.global.md` ist die versionierte Vorlage für die globale
   `~/.claude/CLAUDE.md`.

2. **`.claude/settings.json`** mit drei Einträgen:
   `permissions.deny` hält 16 Leseregeln von `node_modules`, `target`,
   `dist`, Lockfiles, den 3,88 MB LUT-Dateien und den 2,15 MB
   Skill-Daten fern; `bashOutputMaxChars: 15000` begrenzt
   Werkzeugausgaben, **ohne etwas zu verlieren** (was darüber liegt,
   landet in einer Datei und bleibt lesbar); `skillOverrides` setzt 32
   für dieses Projekt irrelevante Skills auf `"name-only"` — das nimmt
   ihnen die Beschreibung aus dem Dauerkontext und **lässt sie
   aufrufbar**. Bewusst NICHT gesetzt: `disableClaudeAiConnectors`, weil
   nicht belegt ist, ob darunter auch der GitHub-Server fällt, über den
   die Pull Requests laufen.

3. **`DECISIONS.md` geteilt.** Die Datei war 519.214 Zeichen groß, also
   rund 151.000 Tokens — zu teuer, um sie beim Arbeiten zu lesen.
   ADR-0001 bis ADR-0060 (76 Einträge) stehen jetzt in
   `docs/DECISIONS-ARCHIV.md`; die Hauptdatei trägt einen vollständigen
   Index aller 97 Einträge mit Fundort und die 21 jüngsten im Volltext.
   Von 519.214 auf 113.375 Zeichen, −78 %.
   Gegenkontrolliert statt angenommen: 97 Überschriften vorher und
   nachher, 81 eindeutige ADR-Nummern identisch, und `comm` über alle
   nichtleeren Zeilen zeigt keine verlorene Originalzeile.

4. **`typecheck`-Skript** in `frontend/package.json`. Der Befehl wurde
   in einer Sitzung 64mal aufgerufen und hatte keinen Namen.

**Was ausdrücklich NICHT gemacht wurde.** `crates/apx-app/src/commands.rs`
(461.618 Zeichen) und `frontend/src/store/index.ts` (338.892) bleiben
ungeteilt. Das wäre der nächste strukturelle Hebel, ist aber ein großes
Diff an zentralem Code — eine Entscheidung für den Nutzer, nicht für
einen Aufräumschritt. Bis dahin greift die Leseregel in `CLAUDE.md`.

**Die Zusage, in Zahlen.** Die Einträge 1–4 senken die Fixkosten um rund
12.000 Tokens je Turn und verhindern Nacharbeit. Der große Hebel bleibt
Verhalten: Sitzungen bei 20 % Auslastung schneiden und
Werkzeugaufrufe bündeln. Modellrechnung: zehn Sitzungen à 165 Turns bei
Ø 120k Kontext kosten rund 200 Mio. statt 675 Mio. Tokens — etwa ein
Drittel. Nachgewiesen ist das erst, wenn dieselbe Referenzaufgabe
vorher und nachher gemessen wurde; die Abnahme steht in
`docs/ARBEITSWEISE.md`.


---

## ADR-0083: Der Token-Audit laeuft automatisch beim Kompaktieren

**Status:** entschieden. Baut auf ADR-0082 auf.

**Der Wunsch.** „Kann ich Claude automatisch im Terminal einen Audit
machen lassen, immer beim Erreichen des Limits, das in eine Datei
festhalten, mir sagen wie es aussieht, und dann nach Bestaetigung
Massnahmen ergreifen?"

**Was das Hook-Schema zulaesst — und was nicht.** Zwei Grenzen, die die
Loesung bestimmt haben:

1. **`PreCompact` nimmt nur `command`-Hooks.** Die Typen `prompt` und
   `agent`, die ein Sprachmodell einbinden wuerden, gibt es laut Schema
   nur bei Werkzeug-Ereignissen (`PreToolUse`, `PostToolUse`,
   `PermissionRequest`). Der Audit muss also ein Skript sein: es liefert
   **Zahlen, kein Urteil**. Das Urteil kommt im naechsten Turn vom
   Modell, das den Bericht ueber `additionalContext` bekommt.
2. **Ein Hook kann keine Massnahme ausloesen.** Er kann melden und
   Kontext einspeisen, nicht handeln. Das ist hier kein Mangel, sondern
   der richtige Ablauf: Hook misst, Modell schlaegt vor, Nutzer
   bestaetigt, dann wird gehandelt.

**Die Umsetzung.** `tools/token-audit.mjs`, als `PreCompact`- und
`SessionEnd`-Hook in `.claude/settings.json` eingetragen. Er liest das
Sitzungstranskript, schreibt einen kurzen Bericht nach
`.claude/token-audit/` (zeitgestempelt plus `latest.md`, nicht
versioniert) und gibt Hook-JSON aus: `systemMessage` ist die eine Zeile
im Terminal, `additionalContext` traegt den Bericht ins Modell.

**Warum Node und nicht Python.** Node ist harte Voraussetzung dieses
Projekts; `python3` gibt es auf einem Windows-Rechner oft nicht, und
Hooks laufen dort ohne Git Bash durch PowerShell. Ein Hook, der still
scheitert, ist schlimmer als keiner. Das Skript hat keine
Abhaengigkeiten.

**Warum nur Statistik.** Der Bericht landet im
Repository-Verzeichnis. Er enthaelt deshalb ausschliesslich Zahlen,
Werkzeugnamen und Dateipfade — keine Nachrichtentexte, keine Befehle,
keine Dateiinhalte.

**Das Transkript finden.** Drei Wege, weil nicht jedes Hook-Ereignis
dieselben Felder mitbringt: der mitgelieferte `transcript_path`, der aus
`session_id` gebildete Pfad, und als letzter Ausweg die neueste
`.jsonl` im Projektverzeichnis. Ohne Transkript meldet der Hook das und
endet mit 0 — er blockiert nie das Kompaktieren.

**Was der Bericht nennt.** Turns, Ø Kontext je Turn, groesster Turn,
cache_read, Kompaktierungen, Werkzeugausgaben in Zeichen, laengster
Bash-Befehl, Aufrufe je Werkzeug, Ergebnisse ueber 10.000 Zeichen,
Zaehler fuer teure Befehle, mehrfach gelesene Dateien — und daraus einen
Befund mit konkreter Massnahme je Auffaelligkeit. Schwelle fuer den
Rat zu schneiden: 200.000 Ø Kontext.

**Geprueft, nicht angenommen.** Rohtest nach Hook-Anleitung: Eingabe-JSON
in das Skript gepipet, EXIT 0, leerer stderr, Bericht geschrieben (1.418
Zeichen), `systemMessage` lesbar. Die Zahlen reproduzieren die
Handmessung aus ADR-0082 (Ø 410.871 gegen 409.188 — die Sitzung ist
seitdem gewachsen). Schema-Pruefung mit `jq -e` auf beide Hook-Pfade
bestanden, die uebrigen Schluessel der Datei unveraendert (16 Leseregeln,
32 Skill-Ueberschreibungen, bashOutputMaxChars).

**Was NICHT geprueft ist.** Dass der Hook tatsaechlich feuert.
`PreCompact` und `SessionEnd` treten ausserhalb eines Turns ein und
lassen sich innerhalb einer Sitzung nicht ausloesen. Dazu kommt: der
Einstellungs-Beobachter ueberwacht nur Verzeichnisse, die beim
Sitzungsstart schon eine Einstellungsdatei hatten. In einer Sitzung, die
vor dieser Datei begonnen hat, greifen die Hooks also erst nach
`/hooks` oder einem Neustart.

**Das Kontext-Limit selbst.** `autoCompactWindow` (100.000 bis
1.000.000) ist der Wert, der entscheidet, wann kompaktiert wird; im
Terminal auch per `/autocompact`. Er gehoert in die persoenlichen
Einstellungen, nicht in die geteilten des Projekts — Vorlage in
`docs/settings.global.json` mit der Empfehlung 200.000. Gemessen waere
das statt Ø 409.188 grob Ø 130.000 bis 150.000, also etwa ein Drittel,
ohne Verhaltensaenderung.

**Was es weiterhin nicht gibt.** Keine Einstellung startet automatisch
eine neue Sitzung. Auto-Kompaktierung ist der eingebaute Mechanismus,
`/clear` bleibt manuell — und das ist der ehrlichere Weg: Kompaktieren
fasst zusammen und verliert dabei, `/clear` verliert nichts, solange der
Stand in `PLAN.md` steht.


---

## ADR-0084: Einstellungen gehoeren dorthin, wo sie ohne Kopieren wirken

**Status:** entschieden. **Korrigiert** ADR-0083s letzten Abschnitt.

**Der Einspruch.** Nutzer, mit Bildschirmfoto von
`docs/settings.global.json`: „settings ist im falschen Ordner".

**Er hat recht, und es war nicht nur der Ordner.** ADR-0083 hat das
Kontext-Limit (`autoCompactWindow`) in eine Vorlagendatei gelegt, die
von Hand nach `~/.claude/settings.json` kopiert werden muss. Begruendung
damals: der Wert sei eine persoenliche Vorliebe und gehoere nicht in die
geteilten Projekt-Einstellungen.

Das Argument ist nicht falsch, aber es hat die wichtigere Eigenschaft
uebersehen: **eine Einstellung, die erst nach einem Handgriff wirkt,
wirkt meistens nicht.** Genau diese Fehlerklasse hat in ADR-0079 schon
einmal zugeschlagen — `onlyBuiltDependencies` stand an einer Stelle, die
aussah als wirke sie, und tat nichts. Eine Vorlage, die kopiert werden
muss, ist dieselbe Falle mit mehr Schritten.

**Die Korrektur.** `autoCompactEnabled`, `autoCompactWindow` (200.000)
und `precomputeCompactionEnabled` stehen jetzt in
`.claude/settings.json`, also dort, wo sie mit dem Klon mitkommen und
ohne Zutun wirken. Das Schema markiert diese Schluessel nicht als
eingeschraenkt — anders als etwa `spellcheck` oder `footerLinksRegexes`,
die ausdruecklich sagen, dass Projekt-Einstellungen ignoriert werden.
`docs/settings.global.json` ist geloescht: zwei Orte fuer dieselbe
Aussage, von denen einer stillschweigend nichts tut, sind genau das
Problem.

**Was global bleiben muss.** `docs/CLAUDE.global.md` — sie gilt fuer
ANDERE Projekte und kann deshalb nicht im Projekt liegen. Damit sie
nicht am Pfad-Gefummel scheitert, gibt es
`tools/setup-claude-global.mjs`: ein Befehl, plattformunabhaengig,
**ueberschreibt nichts**. Gibt es dort schon eine Datei, legt er die neue
Fassung als `CLAUDE.md.neu` daneben und meldet den Unterschied. Eine
Einrichtungshilfe, die vorhandene Regeln wegwirft, waere schlimmer als
gar keine.

**Geprueft, alle vier Zweige.** Datei fehlt → angelegt (3.023 Zeichen in
einem frischen HOME). Datei identisch → „Schon aktuell". Datei
abweichend → Original nachweislich unveraendert (`MEINE EIGENEN REGELN`
stand danach noch da), Vorlage als `.neu` daneben. Falsches
Verzeichnis → Fehlermeldung, Exit 1. `jq` bestaetigt die neuen
Schluessel in `.claude/settings.json` und die unveraenderten alten.

**Die Lehre, schon wieder dieselbe.** Ein Entwurf ist nicht fertig,
wenn er richtig ist, sondern wenn er von selbst wirkt. „Der Nutzer muss
nur noch kurz" ist die Stelle, an der es nicht passiert.
