# DECISIONS.md — Architecture Decision Records

Ein Eintrag pro Architekturentscheidung, im ADR-Format. Einträge werden nicht rückwirkend verändert — eine überholte Entscheidung bekommt einen neuen Eintrag mit Verweis auf den alten (`Status: Abgelöst durch ADR-00xx`).

---

## ADR-0001: SQLite-Zugriff über `rusqlite` statt `sqlx`

**Status:** Angenommen
**Kontext:** `SPEC.md` schreibt `sqlx` vor, erlaubt Abweichung nur mit Begründung. `sqlx`s Compile-Zeit-Prüfung von SQL-Statements braucht entweder eine erreichbare Datenbank beim Bauen oder eine gepflegte `.sqlx`-Offline-Cache-Datei. In dieser Phase ändert sich das Schema noch häufig (Migration 1 ist erst der Anfang), das würde ständig Reibung erzeugen — jede Schemaänderung bräuchte einen `cargo sqlx prepare`-Schritt, sonst bricht CI.
**Entscheidung:** `rusqlite` mit `bundled`-Feature (SQLite wird mitkompiliert, keine System-Abhängigkeit). Repository-Funktionen sind typisiert und gekapselt in `apx-catalog`, SQL bleibt vollständig dort.
**Konsequenzen:** Kein Compile-Zeit-Typecheck von SQL wie bei `sqlx` — dafür sorgen Repository-Tests (Round-Trip pro Tabelle) für Absicherung. `rusqlite` ist synchron; Aufrufe aus `apx-app` laufen über `spawn_blocking`, damit der Tauri-Async-Kontext nicht blockiert.
**Lizenz:** `rusqlite` MIT, gebündeltes SQLite Public Domain. Unkritisch.

---

## ADR-0002: RAW-Dekodierung über `rawler` — LGPL-2.1-Hinweis

**Status:** Angenommen, vom Nutzer ausdrücklich bestätigt ("Ja, LGPL-2.1
für rawler akzeptieren")
**Kontext:** `SPEC.md` Abschnitt 6 verlangt: „Nichts mit GPL im Kern, außer du weist mich ausdrücklich darauf hin." Der Phase-1-Prompt schreibt `rawler` als Bibliothek vor und verbietet eigenes RAW-Parsing.

Ich habe die Lizenzlage der realistischen Alternativen geprüft:

| Crate | Lizenz |
|---|---|
| `rawler` (dnglab-Projekt) | LGPL-2.1 |
| `rawloader` (Vorgänger von rawler, unmaintained) | LGPL-2.1 |
| `quickraw` | LGPL-2.1 |
| `libraw`-Bindings (C-FFI) | Binding selbst MIT, verlinkt aber `libraw` (LGPL-2.1 **oder** CDDL-1.0 **oder** LibRaw Software License) |

**Es gibt aktuell keine dokumentierte, breit RAW-Formate abdeckende Rust-Bibliothek mit rein permissiver Lizenz (MIT/Apache/BSD).** Das liegt an der Herkunft dieses Ökosystems aus `dcraw`, dessen Abkömmlinge durchgängig LGPL sind. Ein eigener RAW-Parser wäre die einzige GPL-freie Alternative, widerspricht aber ausdrücklich der Vorgabe „Baue kein eigenes RAW-Parsing" im Phase-1-Prompt und wäre für Phase 1 nicht seriös leistbar (RAW-Formate sind je Hersteller eigene, teils unter NDA stehende Binärformate).

**Damit hiermit ausdrücklich darauf hingewiesen** (wie von `SPEC.md` gefordert), bevor Code geschrieben wird.

**Entscheidung (vom Nutzer bestätigt):** `rawler` verwenden, LGPL-2.1 als bewusste Ausnahme akzeptieren, und die Ausnahme durch technische Maßnahmen entschärfen:
1. `apx-raw` bleibt ein eigener, klar abgegrenzter Crate (bereits durch Workspace-Struktur erzwungen) — die LGPL-Komponente ist isoliert, nicht mit Geschäftslogik vermischt.
2. Sobald Distribution ansteht (ab Phase 10 / Installer), `apx-raw` als dynamisch nachladbare Komponente bauen (`cdylib` oder Plugin-Grenze), damit Nutzer sie gemäß LGPL §6 durch eine modifizierte Version ersetzen können, statt sich auf eine für Rust unklare statische Verlinkung zu verlassen. Diese Maßnahme wird in Phase 10 konkret umgesetzt und hier nur vorgemerkt.
3. Vollständiger Lizenztext und Quellverweis in `THIRD_PARTY.md`.
**Konsequenzen:** Falls das Gesamtprodukt closed-source vertrieben werden soll, ist Punkt 2 nicht optional, sondern Voraussetzung für Rechtssicherheit. Falls Aperture X ohnehin quelloffen wird, entfällt die Dringlichkeit weitgehend. **Diese Weichenstellung (closed-source vs. quelloffen) betrifft den Gesamtumfang des Projekts und wird hier nicht einseitig entschieden.**

---

## ADR-0003: Bildübertragung Backend→Frontend über Custom-Protokoll-Handler

**Status:** Angenommen
**Kontext:** Bilddaten (Vorschauen wie Vollbilder) müssen häufig und schnell vom Rust-Backend ins WebView. Tauri-Commands mit Base64-kodierten Bildern im Rückgabewert wurden erwogen.
**Entscheidung:** Custom-Protokoll-Handler (`apx://preview/<id>?level=`, `apx://image/<id>?max_edge=`), der direkt Binärdaten mit korrektem `Content-Type`/`Cache-Control` liefert. Das Frontend nutzt die URL direkt in `<img>` bzw. per `fetch`+`createImageBitmap`.
**Konsequenzen:** Kein Base64-Overhead (+33 % Größe, zusätzliche Kopie im JS-Heap), Browser-eigenes Caching greift, parallele Anfragen werden im Handler dedupliziert. Mehraufwand: Protokoll-Handler-Routing und Abbruchlogik (laufende Dekodierung bei Bildwechsel canceln) müssen selbst gebaut werden.

---

## ADR-0004: Viewer in Phase 1 auf Canvas 2D statt WebGL/WebGPU

**Status:** Angenommen, befristet auf Phase 1
**Kontext:** `SPEC.md` verlangt für den fertigen Viewer WebGL2/WebGPU ohne DOM-Rendering im Bildbereich. Phase 1 hat aber explizit noch keine GPU-Pipeline (die kommt in Phase 2 mit `wgpu`).
**Entscheidung:** Für Phase 1 reicht ein `<canvas>` mit 2D-Kontext und `ImageBitmap` als Quelle — es wird nur ein bereits fertig dekodiertes Bild dargestellt, Zoom/Pan sind reine Transformationen, keine Pixel-Verarbeitung. Das hält Phase 1 schlank und vermeidet, eine WebGL-Infrastruktur zu bauen, die in Phase 2 ohnehin durch die wgpu-Pipeline ersetzt bzw. eng verzahnt wird.
**Konsequenzen:** Der Viewer-Code wird in Phase 2 nicht 1:1 weiterverwendet, sondern um die GPU-Pipeline-Anbindung erweitert/ersetzt — das ist im Phase-1-Prompt so vorgesehen und im Code entsprechend kommentiert. Kein Zwischenschritt „WebGL ohne Pipeline" wird gebaut, das wäre Doppelarbeit.

---

## ADR-0005: IDs als UUIDv7-Newtypes

**Status:** Angenommen
**Kontext:** Katalogeinträge (Fotos, Ordner …) brauchen stabile, eindeutige IDs, die sich gut in SQLite (`TEXT PRIMARY KEY`) und in Cache-Pfaden (`previews/<xx>/<id>_<level>.jpg`) verwenden lassen.
**Entscheidung:** `PhotoId`, `FolderId`, `CatalogId` als typisierte Newtypes über UUIDv7. UUIDv7 ist zeitlich sortierbar (Zeitstempel in den High-Bits), das hilft bei Index-Lokalität und macht „neueste zuerst"-Abfragen ohne extra Sortierspalte günstiger.
**Konsequenzen:** Erfordert ein UUID-Crate mit v7-Unterstützung (`uuid` mit `v7`-Feature). Newtype-Wrapper verhindern, dass eine `PhotoId` versehentlich als `FolderId` verwendet wird (Typsicherheit statt „stringly typed" IDs).

---

## ADR-0006: Fehlerbehandlung mit `thiserror`, keine `unwrap()` außerhalb von Tests

**Status:** Angenommen
**Kontext:** `SPEC.md` Abschnitt 6 verlangt explizite Fehlerbehandlung überall.
**Entscheidung:** Ein gemeinsamer `AppError`-Typ in `apx-core` (`thiserror`), alle Crates geben `Result<T, AppError>` zurück. `clippy::unwrap_used` wird als Lint aktiviert (`-D clippy::unwrap_used` außerhalb von `#[cfg(test)]`), damit das in CI erzwungen statt nur vereinbart ist.
**Konsequenzen:** Etwas mehr Schreibaufwand an Fehlerstellen, dafür keine stillen Panics in Produktionscode.

---

## ADR-0007: Echte RAW-Testdateien fehlen — Netzwerkzugriff auf raw.pixls.us in dieser Sandbox blockiert

**Status:** Offen — Entscheidung/Freigabe durch Nutzer ausstehend
**Kontext:** `PHASE1_PROMPT.md` Abschnitt 8 verlangt für `apx-raw` "pro Format eine Datei in `testdata/`" plus Golden-Image-Tests gegen diese echten Kameradateien. Die empfohlene Quelle ist raw.pixls.us (CC0-lizenzierte RAW-Samples). Ein Zugriffsversuch (per `curl` **und** per `WebFetch`) auf `https://raw.pixls.us/` schlägt in dieser Sandbox-Session mit `EGRESS_BLOCKED` bzw. HTTP 403 fehl — die Netzwerk-Egress-Policy dieser Umgebung lässt den Host nicht zu. Das ist keine Design-Entscheidung, sondern eine Umgebungsgrenze dieser konkreten Sitzung.
**Was stattdessen existiert:** Alle Decoding-Algorithmen (Demosaicing bilinear + generischer Fallback, Farbmatrix-Herleitung, Gammakurve, Crop, Downsampling, Orientierung, EXIF-Zeitzonen-Parsing) sind über synthetische Unit-Tests mit selbst konstruierten Eingabedaten abgedeckt (39 Tests, alle grün) — das prüft die Korrektheit der Implementierung, aber nicht das Verhalten gegen reale Dateien der acht Zielformate.
**Optionen für den Nutzer:**
1. Netzwerk-Egress für `raw.pixls.us` (oder eine alternative CC0-Quelle) für diese Umgebung freischalten, dann hole ich die Dateien in dieser Session nach.
2. Der Nutzer lädt passende CC0-RAW-Samples selbst herunter und legt sie im Repo unter `testdata/` ab (mit Lizenzangabe für `THIRD_PARTY.md`) — ich ergänze dann die Golden-Image-Tests.
3. GitHub Actions selbst (die echten Runner haben regulären Internetzugang, anders als diese Sandbox) lädt die Testdateien bei Bedarf in einem eigenen CI-Schritt herunter, statt sie im Repo zu versionieren — spart Repo-Größe, macht CI aber von der Erreichbarkeit der externen Quelle abhängig.
**Entscheidung:** Noch nicht getroffen. Bis zur Klärung bleibt dieser Teilpunkt in `PLAN.md` als offen/blockiert markiert; der Rest von Phase 1 wird nicht darauf verzögert.

---

## ADR-0008: Katalog-Verbindung als einzelne `Connection` hinter einem `Mutex`

**Status:** Angenommen
**Kontext:** `PHASE1_PROMPT.md` Abschnitt 10 nennt als bekannten Fallstrick: "SQLite-Schreibzugriffe parallel → `database is locked`. Ein einziger Writer, Leser über einen Pool." Ein echter Connection-Pool mit getrennten Lese-Verbindungen (z. B. über `r2d2`) wäre möglich, bringt aber zusätzliche Abhängigkeiten und Komplexität (Pool-Konfiguration, Verbindungs-Recycling), die für Phase 1 (ein Nutzer, ein Prozess) keinen messbaren Vorteil bringt.
**Entscheidung:** `Catalog` hält eine einzelne `rusqlite::Connection` in einem `std::sync::Mutex`. Alle Zugriffe — lesend wie schreibend — werden dadurch auf Rust-Ebene serialisiert. Das erfüllt die Anforderung "ein einziger Writer" direkt und verhindert `database is locked`-Fehler aus konkurrierenden Zugriffen innerhalb des Prozesses vollständig, nicht nur im statistischen Regelfall. WAL-Modus bleibt aktiviert, damit externe Prozesse (z. B. ein DB-Browser zur Fehlersuche) weiterhin parallel lesend zugreifen können.
**Konsequenzen:** Innerhalb des Prozesses gibt es keine echte Nebenläufigkeit bei Katalogzugriffen — ein langer Schreibvorgang (z. B. ein Massenimport) blockiert währenddessen andere Katalog-Lesezugriffe. Für Phase 1 unproblematisch (Importe laufen ohnehin sequenziell pro Datei mit kurzen Einzeltransaktionen). Sollte Profiling in einer späteren Phase echte Kontention zeigen (z. B. Bibliotheks-Raster-Scrollen blockiert durch einen laufenden Import), wird das hier nachgerüstet — dokumentiert als möglicher Folge-ADR, nicht vorab spekulativ gebaut.

---

## ADR-0009: `apx://`-URLs als opake, über `convertFileSrc` kodierte Pfade statt echtem Query-String

**Status:** Angenommen
**Kontext:** `PHASE1_PROMPT.md` Abschnitt 6 illustriert das Protokoll mit
`apx://preview/<photo_id>?level=0`. Tauris offizieller, plattformneutraler
Weg, eine Custom-Protocol-URL im Frontend zu bauen, ist
`convertFileSrc(pfad, "apx")` aus `@tauri-apps/api/core` — nötig, weil sich
das URL-Schema zwischen Plattformen unterscheidet (macOS/Linux:
`apx://localhost/<pfad>`; Windows/Android: `http://apx.localhost/<pfad>`,
weil WebView2 und Android WebView keine beliebigen Custom-Schemes für
Netzwerk-Requests erlauben). Ein Blick in Tauris `convertFileSrc`-Quelle
zeigt: die Funktion `encodeURIComponent`-kodiert den **gesamten** `pfad`-
String als **ein einziges** Pfadsegment. Ein `?level=0` im Eingabestring
würde also mitkodiert (`%3Flevel%3D0`) und käme auf Rust-Seite nicht als
echter Query-String an, sondern als Teil eines einzigen, weiterhin
prozentkodierten Pfadsegments — `http::Uri::query()` bliebe `None`.
**Entscheidung:** Statt eines echten Query-Strings kodiert das Frontend
Anfragen als `convertFileSrc("preview/<photo_id>/<level>", "apx")` bzw.
`convertFileSrc("image/<photo_id>/<max_edge_oder_'full'>", "apx")` — die
gesamte Information steckt in einem einzigen, von `convertFileSrc` korrekt
prozentkodierten Pfadsegment. Der Rust-Handler dekodiert dieses Segment
selbst (`percent-encoding`-Crate) und splittet es an `/`. Das funktioniert
identisch auf allen drei Plattformen, ohne dass das Frontend das
Betriebssystem selbst erkennen müsste.
**Konsequenzen:** Die tatsächliche Pfadstruktur weicht in der Notation
leicht vom illustrativen Beispiel in `PHASE1_PROMPT.md` ab (Segmente statt
Query-Parameter) — funktional identisch (dieselben Informationen, dieselbe
Semantik: Foto-ID, gewünschte Auflösungsstufe/Kantenlänge). Diese
Abweichung ist hier dokumentiert, wie in `SPEC.md` Abschnitt 6 gefordert.

---

## ADR-0010: Playwright testet das Frontend gegen einen simulierten Tauri-Bridge, nicht die kompilierte native App

**Status:** Angenommen, mit klar benanntem Rest-Risiko
**Kontext:** `SPEC.md`s Tech-Stack-Tabelle nennt Playwright für E2E-Tests,
und `PHASE1_PROMPT.md` Abschnitt 8 verlangt ein E2E-Szenario, das die
komplette App abdeckt: „App starten → Ordner importieren → Thumbnails
erscheinen → Bild anklicken → Viewer zeigt es → Zoom 1:1 → Neustart →
Katalog ist noch da." Das ist ein Test der **kompilierten nativen
Tauri-App**, nicht nur der Web-Inhalte.

Playwright steuert Browser über das Chrome DevTools Protocol (CDP) an.
Tauris WebView ist plattformabhängig: WebView2 unter Windows basiert auf
Chromium und unterstützt CDP (`--remote-debugging-port`), WKWebView unter
macOS und WebKitGTK unter Linux tun das **nicht**. Ein CDP-basierter
Playwright-Test gegen die echte App liefe also nur auf Windows —
inakzeptabel für eine Anforderung, die alle drei Plattformen abdecken
muss. Der von Tauri selbst empfohlene, plattformunabhängige Weg für
echte native E2E-Tests ist `tauri-driver` (WebDriver-Protokoll, nutzt je
Plattform den nativen WebView-Treiber) zusammen mit WebdriverIO oder
Selenium — nicht Playwright.

**Entscheidung:** Für Phase 1 wird Playwright wie im Tech-Stack
vorgesehen eingesetzt, aber gegen den **Vite-Dev-/Preview-Server mit
einem simulierten `window.__TAURI_INTERNALS__`** (`invoke`/`convertFileSrc`
geben kontrollierte Testdaten zurück, siehe `frontend/e2e/`) statt gegen
die kompilierte App. Das deckt zuverlässig und plattformunabhängig ab:
Layout, Dark-Theme, Zustand-Store-Verhalten, Tastenkürzel, Filmstreifen-
Virtualisierung, Viewer-Interaktion — alles, was sich rein im Frontend
abspielt. Die **Backend-Anteile** des E2E-Szenarios (echter Import,
echte Dekodierung, echte Katalog-Persistenz über einen Neustart hinweg)
sind stattdessen durch Rust-Integrationstests abgedeckt, die exakt
dieselbe Logik über dieselben Schnittstellen ausführen wie die App selbst
(`apx-app`s `import_run_handles_three_valid_and_one_broken_file`,
`apx-catalog`s `open_on_disk_persists_across_reopen`) — nur eben nicht
durch Klicks in einem echten Fenster ausgelöst.
**Konsequenzen:** Es fehlt ein Test, der wirklich **Klick für Klick durch
die kompilierte App** geht und dabei Frontend und Backend gemeinsam über
die tatsächliche Tauri-IPC-Bridge prüft (z. B. ein Regressionsfehler
ausschließlich in der Verdrahtung zwischen echtem Command und echtem
Frontend-Aufruf, der weder im Rust-Test noch im simulierten
Playwright-Test auffiele). Das ist eine bewusste, dokumentierte Lücke
für Phase 1, keine stillschweigend schwächere Umsetzung. Empfehlung für
eine spätere Phase: `tauri-driver` + WebdriverIO als zusätzliche,
eigene Test-Stufe ergänzen, sobald der Funktionsumfang das rechtfertigt.

---

## ADR-0011: Phase 2 umfasst genau sieben Grundeinstellungs-Regler, nicht zwölf

**Status:** Angenommen
**Kontext:** `SPEC.md` §5s Phasenplan-Satz für Phase 2 nennt namentlich
sieben Regler: „die Grundeinstellungs-Regler (WB, Belichtung, Kontrast,
Lichter, Tiefen, Weiß, Schwarz)". `FEATURES.md` hatte davon abweichend
alle zwölf „Grundeinstellungen"-Regler aus `SPEC.md` §3.2 (zusätzlich
Textur, Klarheit, Dunst entfernen, Dynamik, Sättigung) als Phase 2
markiert — eine Diskrepanz zwischen der maßgeblichen Phasenplan-Zeile
und `FEATURES.md`s eigener, weiter gefasster Interpretation.

**Entscheidung:** `SPEC.md` §5 ist die maßgebliche Quelle für den
Phasen-Umfang. Phase 2 umfasst genau die sieben genannten Regler.
Textur/Klarheit/Dunst entfernen/Dynamik/Sättigung wandern in
`FEATURES.md` zu Phase 4, wo sie fachlich ohnehin neben Gradationskurve/
HSL/Farbmischer/Details stehen — Werkzeuge, die eher „gestalterische
Nachbearbeitung" als „grundlegende Tonwertkorrektur" sind.

**Konsequenzen:** Phase 2 bleibt beim in `SPEC.md` beschriebenen,
kleineren Umfang. `FEATURES.md`s eigene Präambel erlaubt genau diese Art
von Verfeinerung ausdrücklich ("wird beim Start der jeweiligen Phase in
PLAN.md verfeinert... Änderungen gehören dann in DECISIONS.md"). Diese
Entscheidung wurde stellvertretend getroffen, ohne Rückfrage beim
Nutzer, da sie den Umfang verkleinert (nicht wesentlich vergrößert) und
rein fachlich-technischer Natur ist.

---

## ADR-0012: Ein Crate `apx-pipeline`, nicht zwei (`apx-pipeline` + `apx-gpu`)

**Status:** Angenommen
**Kontext:** `ARCHITECTURE.md`s bisheriger Phase-2-Platzhalter nannte
zwei mögliche Crate-Namen nebeneinander, ohne zu entscheiden, ob es ein
oder zwei Crates werden.

**Entscheidung:** Ein einziges neues Crate, `apx-pipeline`, enthält das
EDL-Datenmodell, die wgpu-Anbindung, den Tile-Cache und das
Farbmanagement (`lcms2`). Begründung: Phase 1 hält den Workspace bewusst
bei vier Crates mit klaren Grenzen; eine Aufspaltung „GPU-Kontext" vs.
„EDL/Pipeline-Logik" in zwei Crates würde nur künstliche Grenzen
zwischen eng zusammenhängendem Code ziehen (der EDL-Interpreter *ist*
der wgpu-Aufrufer) ohne einen erkennbaren Vorteil für Testbarkeit oder
Wiederverwendbarkeit — anders als z. B. die `apx-raw`/`apx-catalog`-
Trennung, die zwei fachlich unabhängige Verantwortlichkeiten trennt.

**Konsequenzen:** `apx-pipeline` hängt von `apx-core` und `apx-raw` ab,
nicht von `apx-catalog` (siehe ADR-0013). `apx-app` hängt zusätzlich von
`apx-pipeline` ab, wie von den anderen drei Crates auch.

---

## ADR-0013: EDL wird als JSON in einem `apx-core`-Umschlagtyp gespeichert, nicht als CBOR, nicht direkt von `apx-catalog` interpretiert

**Status:** Angenommen
**Kontext:** `SPEC.md` §2.1 lässt das Serialisierungsformat offen
("als JSON/CBOR"). Zusätzlich stellt sich die Frage, wo der konkrete
EDL-Rust-Typ leben soll, ohne die Abhängigkeitsrichtung
`apx-catalog` ↛ `apx-pipeline` zu verletzen (siehe ADR-0012).

**Entscheidung:** JSON, nicht CBOR — menschenlesbar/diffbar (wichtig für
Debugging und die später geplante „Verlaufs-Vergleich"-Funktion),
`serde_json` ist bereits Workspace-Abhängigkeit, und die Nutzlast ist für
Phase 2 klein (7 Zahlen), sodass CBORs Kompaktheitsvorteil nicht ins
Gewicht fällt. `apx-core` bekommt einen minimalen, versionsmarkierten
Umschlagtyp `EdlEnvelope { schema_version: u32, payload:
serde_json::Value }`. `apx-catalog` speichert nur diesen Umschlag (als
`TEXT`-Spalte) und muss `payload` nie verstehen — nur `apx-pipeline`
kennt die konkrete `EdlV1`-Struct und entpackt `payload` hinein.

**Konsequenzen:** `apx-catalog` bleibt unabhängig von `apx-pipeline`.
Sollten spätere Phasen große Kurven-/Masken-Daten ins EDL bringen und
JSON-Größe/Parse-Geschwindigkeit zum echten Problem werden, ist ein
Wechsel zu CBOR eine lokale Änderung in `apx-pipeline` (Serialisierung)
plus einer neuen Schema-Version — kein Architekturbruch.

---

## ADR-0014: Verlauf/Undo-Redo als eigene SQLite-Tabelle vollständiger EDL-Schnappschüsse

**Status:** Angenommen
**Kontext:** `SPEC.md` §3.4/§5 verlangt „Verlauf mit unbegrenzten,
benennbaren, klickbaren Schritten (Undo/Redo)". Zwei Modellierungen
kämen infrage: ein Operations-Log (jede Regler-Änderung als ein Eintrag,
EDL wird durch Abspielen aller Einträge rekonstruiert) oder eine Tabelle
vollständiger EDL-Schnappschüsse pro Schritt.

**Entscheidung:** Eigene Tabelle `edit_history` mit vollständigen
EDL-Schnappschüssen pro Zeile (`photo_id`, `sequence`, `label`,
`edl_json`, `created_at`), plus ein 1-Zeile-pro-Foto-Zeiger
`edit_current`. Begründung: „Springe zu einem beliebigen früheren
Schritt", „benenne einen Schritt" und „vergleiche zwei beliebige
Schritte" (spätere Phasen) sind mit vollständigen Schnappschüssen
triviale SQL-Abfragen, während ein Operations-Log für dieselben
Anfragen erst wieder abgespielt werden müsste. Für Phase 2 gilt zudem:
Wiederholen (Redo) nach einer neuen Bearbeitung wird **nicht**
aufbewahrt (keine Verzweigung) — neue Bearbeitung nach einem Rückgängig
verwirft die „Zukunft", wie in den meisten Bildbearbeitungsprogrammen
üblich. Verzweigende Historie ist von `SPEC.md` nicht für Phase 2
gefordert.

**Konsequenzen:** Migration `migrations/0002_edits.sql` ist rein additiv
(ändert `photos` nicht), `edit_current` ist eine eigene Tabelle statt
einer neuen Spalte auf `photos`, um die Phase-1-Tabelle unangetastet zu
lassen.

---

## ADR-0015: `apx-raw` bekommt einen additiven `decode_linear()`-Einstiegspunkt statt den bestehenden Vertrag zu ändern

**Status:** Angenommen
**Kontext:** Der heutige `apx_raw::decode()`/`DecodedImage`-Pfad
bäckt einen festen (nicht einstellbaren) Weißabgleich und eine feste
sRGB-Gammakurve fest ein — genau das, was Phase 2 einstellbar machen
muss (Weißabgleich-Regler, Belichtung/Ton). Eine Änderung an `decode()`
selbst würde die Verträge der bestehenden Phase-1-Aufrufer (Vorschau-
Erzeugung im Import-Job, Vorschau-/Vollbild-Routen im Protokoll-Handler)
brechen.

**Entscheidung:** Neuer, rein additiver Einstiegspunkt
`apx_raw::decode_linear()`, der die Kette nach CFA-Laden/Demosaicing/
Schwarz-Weiß-Normalisierung stoppt — vor dem bisherigen festen
Weißabgleich-Multiplikator und vor der Gammakurve. `apx-pipeline`
übernimmt ab diesem linearen Zwischenergebnis. `decode()`/`DecodedImage`
bleiben für alle bestehenden Phase-1-Aufrufer unverändert.

**Konsequenzen:** Es gibt vorübergehend zwei Ausstiegspunkte aus der
RAW-Dekodierkette in `apx-raw` (der alte, feste `decode()`-Pfad für
Vorschauen/Thumbnails; der neue `decode_linear()`-Pfad für die
Entwickeln-Ansicht) — bewusst in Kauf genommen, statt Phase 1s
funktionierenden Code für eine Anforderung umzubauen, die er nicht
erfüllen muss (Thumbnails brauchen keinen einstellbaren Weißabgleich).

---

## ADR-0016: Die interaktive Entwickeln-Route liefert rohe RGBA8-Bytes statt PNG/JPEG

**Status:** Angenommen
**Kontext:** Die bestehenden `apx://preview/…`- und `apx://image/…`-
Routen liefern JPEG bzw. PNG. `SPEC.md` §2.4 verlangt für die neue
interaktive Entwickeln-Ansicht „Regler-Bewegung → sichtbares Update:
< 16 ms bei 24-MP-Proxy" — eine erneute PNG-Kodierung (DEFLATE-
Kompression) bei jedem Regler-Tick würde spürbar Zeit aus diesem engen
Budget verbrauchen, nur um die Bytes im Frontend gleich wieder zu
dekodieren, bevor sie als WebGL2-Textur hochgeladen werden.

**Entscheidung:** Die neue `apx://develop/<id>/<max_edge_oder_'full'>/<edl_json>`-
Route liefert rohe, unkomprimierte RGBA8-Bytes (`Content-Type:
application/x-apx-develop-rgba8`, mit Breite/Höhe als `u32`
little-endian in einem 8-Byte-Header vor den Pixeldaten) statt PNG. Die
bestehenden `preview`-/`image`-Routen bleiben unverändert bei JPEG/PNG —
diese Abweichung gilt ausschließlich für die neue, performance-kritische
Route.

**Umsetzungs-Korrektur gegenüber der ursprünglichen Plan-Notiz (Schritt
5):** Statt eines separaten `edl_hash`-Segments trägt die Route die
**vollständige JSON-Serialisierung des `EdlEnvelope`** direkt im
Pfad (`edl_json`) — ein reiner Hash hätte bedeutet, dass die Route den
zugehörigen EDL-Wert irgendwoher nachschlagen müsste, was während des
*Ziehens* eines Reglers (noch nicht committet, siehe ADR-0014) gar nicht
möglich ist. Derselbe String dient nebenbei unverändert als
Unterscheidungsmerkmal für den bestehenden Single-Flight-`ImageCache`
(zwei EDL-Zustände desselben Fotos erzeugen zwei Cache-Schlüssel) — ein
separater Hash-Mechanismus ist damit unnötig. `EdlV1`s Felder sind
ausschließlich Zahlen und enthalten daher nie ein `/`-Zeichen, das die
bestehende "erst dekodieren, dann an `/` aufteilen"-Pfadlogik stören
könnte.

**Konsequenzen:** Größere Rohdatenmenge pro Anfrage als bei komprimierten
Formaten — bei lokaler IPC über `localhost`/den Tauri-Protokoll-Handler
ist das akzeptabel (kein Netzwerk, keine Bandbreitenbegrenzung), der
Kompressions-Zeitgewinn wiegt hier schwerer als die Byte-Ersparnis.

---

## ADR-0017: Fusionierter Compute-Shader für den interaktiven Pfad, separate Shader pro Regler für Spec-Konformität und Tests

**Status:** Angenommen
**Kontext:** `SPEC.md` §6 verlangt „jedes Modul mit eigenem Shader,
eigenem Test". Ein einzelner GPU-Dispatch pro Regler-Änderung würde bei
sieben Reglern potenziell sieben aufeinanderfolgende Dispatch+Rücklese-
Zyklen bedeuten — jeder mit eigenem Overhead, der zusammen das 16-ms-
Budget gefährdet.

**Entscheidung:** Jeder der fünf Regler-Module (Weißabgleich, Belichtung,
Kontrast, Lichter+Tiefen, Weiß+Schwarz) bekommt trotzdem seinen eigenen
WGSL-Shader und eigene Tests (erfüllt SPEC.md wörtlich). Zusätzlich gibt
es einen fusionierten Shader (`basic_fused.rs`), der dieselbe Mathematik
aller fünf Module in einem einzigen GPU-Aufruf kombiniert. Der
interaktive Vorschau-Pfad (bei jedem Regler-Tick) nutzt ausschließlich
den fusionierten Shader — ein Dispatch statt fünf. Ein Abgleichstest
stellt sicher, dass fusionierter und einzelne Shader auf identischem
Input dasselbe Ergebnis liefern (innerhalb der üblichen Gleitkomma-
Toleranz).

**Konsequenzen:** Etwas Code-Duplikation zwischen Einzel- und fusioniertem
Shader (dieselbe Mathematik zweimal ausgedrückt) — bewusst in Kauf
genommen zugunsten von Performance UND Spec-Konformität statt eines
Kompromisses, der keines von beidem vollständig erfüllt.

---

## ADR-0018: Undo/Redo im Frontend direkt über `edit_history`, kein `zundo` (revidiert in Schritt 6)

**Status:** Angenommen (ursprüngliche Fassung revidiert bei der tatsächlichen
Implementierung in Phase 2 Schritt 6 — siehe „Revisions-Begründung" unten)
**Kontext:** Der Zustand-Store nutzt bereits `immer`; `FEATURES.md`
verlangt für Phase 2 „Verlauf mit unbegrenzten, benennbaren, klickbaren
Schritten (Undo/Redo)" im Frontend, zusätzlich zur dauerhaften
`edit_history`-Tabelle aus ADR-0014, die den App-Neustart überlebt.

**Ursprüngliche Entscheidung (Schritt 0):** `zundo`, eine kleine
Bibliothek, die bestehende Zustand-Stores um eine eigene, zusätzliche
Verlaufs-Ebene erweitert — mit der Begründung, sie bündle/entprelle
Einträge, statt bei jeder Mausbewegung einen Schritt anzulegen.

**Revisions-Begründung (Schritt 6):** Bei der tatsächlichen Umsetzung
stellte sich heraus, dass die Entprellung viel einfacher auf
UI-Ereignis-Ebene lösbar ist, ganz ohne State-Management-Bibliothek:
`DevelopSlider` ruft `onChange` (Live-Vorschau, kein Commit) bei jedem
Zwischenwert, aber `onCommit` (schreibt über `apply_develop_edit` in
`edit_history`) erst bei Loslassen/Blur/Doppelklick-Reset. Eine
zusätzliche, separate Frontend-Verlaufs-Ebene (`zundo`) hätte dann *zwei*
lose synchronisierte Historien bedeutet (die lokale und die in
`edit_history`) — mit dem Risiko, dass sie nach einem Fehler oder einer
Race Condition auseinanderlaufen. Stattdessen rufen die Undo-/Redo-Knöpfe
und Strg/Cmd+Z direkt `undo_develop_edit`/`redo_develop_edit` auf (siehe
`crates/apx-app/src/commands.rs`, Phase 2 Schritt 5) — Tauris IPC ist
lokal (kein Netzwerk) schnell genug, um pro Klick einen Roundtrip zu
rechtfertigen, und es gibt dadurch nur *eine* Quelle der Wahrheit für den
Verlauf statt zwei.

**Konsequenzen:** Keine zusätzliche Frontend-Abhängigkeit nötig (`zundo`
wurde nie zu `package.json` hinzugefügt, `THIRD_PARTY.md` bleibt
unverändert). Ein Undo/Redo-Klick löst einen Tauri-Command-Aufruf aus
(unmerklich schnell bei lokalem IPC); eine visuelle, benennbare,
klickbare Liste *aller* Verlaufsschritte (statt nur Rückgängig/
Wiederholen um jeweils einen Schritt) ist mit dieser Entscheidung
weiterhin möglich (über `list_edit_history`, noch nicht implementiert),
aber bewusst über Schritt 6s Mindestumfang hinaus zurückgestellt — der
kritische Pfad („interaktives Entwickeln" überhaupt vorführbar) hatte
Vorrang.

---

## ADR-0019: Feste Kamera→sRGB-Matrix + Gammakurve im Entwickeln-Renderpfad, wiederverwendet statt dupliziert

**Status:** Angenommen
**Kontext:** `apx_raw::decode_linear()` (ADR-0015) liefert absichtlich
unbalanciertes, noch nicht farbtransformiertes Kamera-RGB — die
Weißabgleich-Gains sind nutzerseitig verstellbar (siehe
`WhiteBalanceAdjustment`) und gehören deshalb in `apx-pipeline`, nicht in
`apx-raw`. Die anschließende Kamera-RGB→sRGB-Transformation (eine feste
3×3-Matrix aus den Kamera-Kalibrierungsdaten, siehe
`apx-raw/src/pipeline/color.rs`) ist dagegen **nicht** nutzerseitig
verstellbar — sie ist trotzdem nötig, sonst zeigt der Entwickeln-Viewer
falsche Farben (Kamera-RGB-Primärvalenzen unterscheiden sich spürbar von
sRGB). Ohne sie wäre die neue `develop`-Route zwar lauffähig, aber
farblich falsch — kein tragbarer Zustand für ein Bildbearbeitungswerkzeug.

**Entscheidung:** `apx_raw::LinearImage` bekommt ein zusätzliches Feld
`cam_to_srgb: [[f32; 3]; 3]` (Einheitsmatrix für Fallback-Formate ohne
Kamerakalibrierung). `apx-pipeline::color` wendet diese feste Matrix plus
dieselbe sRGB-Gammakurve an, die `apx_raw::decode()` für Phase-1-
Vorschauen bereits nutzt (`apx_raw::srgb_gamma`, zusätzlich als Teil von
`apx-raw`s öffentlicher API re-exportiert, statt die Formel ein zweites
Mal abzuschreiben) — Reihenfolge im Renderpfad ist damit: As-shot-
Weißabgleich + Nutzer-Shift → die sieben Regler (auf Kamera-RGB) → feste
Kamera→sRGB-Matrix → Gammakurve → RGBA8-Quantisierung.

**Bewusste Vereinfachung:** Die sieben Regler (insbesondere Kontrast/
Lichter/Tiefen/Weiß/Schwarz) wirken damit auf Kamera-RGB-Werten, nicht
auf einem farbmanagement-korrekten Arbeitsraum nach der Matrix — anders
als z. B. Lightroom/Capture One, die Tonwert-Werkzeuge meist nach der
Farbmatrix anwenden. Das kann bei starken Kamera-Matrizen zu leicht
abweichenden Farbverschiebungen zwischen den Kanälen führen. Für Phase 2s
Ziel „interaktives Entwickeln" ist das akzeptabel (die Regler bewegen
sich intuitiv in die richtige Richtung); eine Neuordnung (Matrix vor den
Reglern) ist ein möglicher Ausbau für eine spätere Phase, sobald echtes
ICC-Farbmanagement (`lcms2`, ursprünglich für `apx-pipeline::color`
vorgesehen, siehe `PLAN.md` Phase 2 Schritt 4) tatsächlich eingebaut
wird.

**Konsequenzen:** `apx-raw`s `pipeline`-Modul exponiert
`cam_to_srgb_matrix` zusätzlich als `pub(crate)` (vorher rein privat in
`pipeline::color`), damit `decode_raw_linear` sie berechnen kann, ohne
eine vollständige `ColorPipeline` (die zusätzlich Weißabgleich/Gamma
anwenden würde) zu instanziieren. Kein neues externes Crate, keine
`lcms2`-Abhängigkeit in diesem Schritt.

---

## ADR-0020: Entwickeln-Modul als zuschaltbares Overlay im bestehenden Viewer, WebGL2 für beide Pixelquellen

**Status:** Angenommen
**Kontext:** `PLAN.md` Phase 2 Schritt 6 verlangt „Viewer.tsx wechselt von
2D-Canvas auf WebGL2". Offen war dabei, ob das Entwickeln-Rendering die
bestehende Vorschau/Vollbild-Anzeige (schnelle, gecachte JPEG/PNG-Pfade
aus Phase 1) vollständig ersetzt, oder als zusätzlicher, zuschaltbarer
Modus daneben existiert.

**Entscheidung:** Ein neuer „Entwickeln"-Knopf (`Header.tsx`) schaltet
`developPanelOpen` um; nur währenddessen fragt der Viewer zusätzlich die
`develop/...`-Route ab (`hooks/useDevelopRender`) und zeigt deren
Ergebnis anstelle des bisherigen Vollbilds — ohne offenes Panel verhält
sich der Viewer exakt wie in Phase 1. Der Canvas selbst nutzt jetzt immer
WebGL2 (`lib/webgl.ts`, `QuadRenderer`) statt Canvas-2D, für beide Fälle:
ein dekodiertes `ImageBitmap` (bestehender Pfad) und ein rohes RGBA8
(neue Entwickeln-Route) werden über denselben Texturmechanismus
hochgeladen — Canvas-2D hätte für RGBA8-Rohdaten einen Umweg über
`ImageData` gebraucht. Die bestehende Zoom-/Pan-Geometrie
(`lib/viewerMath.ts`) bleibt unverändert wiederverwendet, nur der
Zeichenaufruf selbst wechselt von `ctx.drawImage` auf einen texturierten
Quad-Shader.

**Konsequenzen:** Geringes Regressionsrisiko für Phase 1s bestehende
Funktionalität (Standardzustand = Panel geschlossen = unverändertes
Verhalten, durch die bestehenden Playwright-Tests in `viewer-flow.spec.ts`
weiterhin abgedeckt, die nach diesem Umbau unverändert grün bleiben). Der
WebGL2-Rendercode selbst (`lib/webgl.ts`) ist mangels WebGL-Unterstützung
in `jsdom` nicht per Vitest testbar — wie schon die vorherige
Canvas-2D-Zeichenlogik nur indirekt über Playwright abgesichert
(„Canvas ist sichtbar", jetzt zusätzlich über die Entwickeln-Flow-Tests
in `develop-flow.spec.ts`, die echte Zustandsänderungen statt nur
Sichtbarkeit prüfen).

---

## ADR-0021: wgpu-GPU-Ausführungstests in CI empirisch bestätigt (Schritt 8) — kein "weich" mehr auf Dauer

**Status:** Angenommen (empirisch verifiziert)
**Kontext:** ADR-0012 und `PLAN.md`s Risikoliste ließen offen, ob die
GitHub-Actions-Runner (Linux/macOS/Windows) tatsächlich einen wgpu-Adapter
finden — die `gpu_matches_cpu`-artigen Tests waren bewusst so gebaut, dass
ein fehlender Adapter zu einem klar begründeten, sichtbaren "übersprungen"
statt einem stillen Erfolg oder Hartabbruch führt. Ohne `--nocapture`
sah ein grüner CI-Lauf aber identisch aus, egal ob der Adapter gefunden
oder der Test nur übersprungen wurde — die Frage blieb unbeantwortet.

**Entscheidung/Befund:** `cargo test --workspace -- --nocapture` in
`ci.yml` aktiviert; im CI-Lauf für Commit `ca2e39f` (Run-ID 33236428223)
enthält **keines** der drei Rust-Job-Logs (`ubuntu-latest`,
`macos-latest`, `windows-latest`) die Zeichenkette "übersprungen: kein
GPU-Adapter" oder `GpuUnavailable` — alle sechs
`stages::*::tests::gpu_matches_cpu`-Tests liefen auf allen drei
Plattformen als echte GPU-Dispatches durch. Damit ist empirisch bestätigt
(nicht mehr nur angenommen): GitHub-Actions-`ubuntu-latest`-Runner bringen
von Haus aus einen nutzbaren Software-Vulkan-Adapter mit (kein
zusätzliches Mesa-`apt-get`-Paket war nötig); `macos-latest` und
`windows-latest` finden ebenfalls einen Adapter (vermutlich Metal bzw.
WARP/DX12 — welches Backend genau, ist ohne gesetztes `RUST_LOG` aus den
Logs nicht ablesbar, war für die Kernfrage aber nicht nötig).

**Konsequenzen:** GPU-Ausführungstests gelten ab sofort wieder als echte
Pflicht (nicht dauerhaft "weich" belassen, wie ADR-0012 es für den Fall
vorsah, dass ein Runner keinen Adapter hat) — ein zukünftiges
"übersprungen" auf einem dieser drei Runner wäre eine echte Regression
(z. B. ein Runner-Image-Wechsel), die untersucht werden müsste, nicht
länger stillschweigend akzeptiert. `ci.yml`s `-D clippy::unwrap_used`
und `apx-app`s fehlendes `#![deny(clippy::unwrap_used)]` wurden bei
diesem Durchgang zusätzlich als bestehende, unabhängige CI-Lücken
gefunden und geschlossen.

---

## ADR-0022: Phase 3 umfasst `SPEC.md` §5s Phase-3-Satz, nicht §3.1s vollen BIBLIOTHEK-Katalog

**Status:** Angenommen
**Kontext:** Wie schon bei Phase 2 (ADR-0011: 7 statt 12 Regler) hatte
`FEATURES.md` deutlich mehr Punkte auf Phase 3 getaggt, als `SPEC.md`
§5s Phasenplan-Satz eigentlich meint — §3.1 zählt den vollständigen
Lightroom-BIBLIOTHEK-Funktionsumfang auf (Gesichtserkennung, virtuelle
Kopien, Stapel, Sekundäres Display, Schlagwort-Synonyme/Auto-
Vervollständigung, Katalog-Backup/-Reparatur/-Zusammenführen,
vollständiger EXIF/IPTC/XMP-Editor mit Sidecar-Export, intelligente
Sammlungen mit verschachtelten Regeln, Perceptual-Hash-Duplikaterkennung,
DNG-Konvertierung), §5s Satz nennt für Phase 3 nur: "Import, Ordner,
Raster, Filmstreifen, Vorschau-Generierung, Bewertungen/Flaggen/Farben,
Sammlungen, Filter, Metadaten-Panel, FTS-Suche".

**Entscheidung:** Phase 3 = §5s Satz wörtlich genommen. Alle darüber
hinausgehenden Punkte sind in `FEATURES.md` auf die Phase umgetaggt, zu
der sie inhaltlich passen (meist Phase 6 „Sammlungen/Metadaten-Ausbau"
oder Phase 9 „Politur/fortgeschrittene Werkzeuge" — siehe die einzelnen
Zeilen in `FEATURES.md` §3.1 für die genaue Zuordnung). Zwei Lücken dabei
gefunden und ergänzt: `FEATURES.md` hatte weder eine Zeile für
"Volltextsuche (FTS5)" noch für ein einfaches "Metadaten-Panel" (nur den
späteren vollständigen EXIF/IPTC/XMP-Editor) — beide sind aber explizit
Teil von §5s Phase-3-Satz und wurden neu ergänzt.

**Konsequenzen:** Wie bei ADR-0011 eine reine Scope-Verkleinerung, keine
-Vergrößerung — der Nutzer muss dazu nicht befragt werden (siehe
`SPEC.md`s Präambel zu Kleinigkeiten, die den Umfang nicht wesentlich
verändern).

---

## ADR-0023: Bewertung/Flagge/Farbe als Spalten, Schlagworte flach, Sammlungen nur manuell, FTS5 als External-Content-Tabelle

**Status:** Angenommen
**Kontext:** Phase 3s neues DB-Schema (Migration `0003_library.sql`)
braucht Entscheidungen zu vier unabhängigen Datenmodell-Fragen.

**Entscheidungen:**
1. **Bewertung (`rating`), Flagge (`flag`), Farbmarkierung
   (`color_label`) sind direkte Spalten auf `photos`**, keine eigene
   Tabelle — konsistent mit dem bestehenden `missing`-Spalten-Muster aus
   Phase 1, da es sich um einfache Skalarwerte pro Foto handelt.
2. **Schlagworte sind in Phase 3 eine flache Liste** (`keywords` +
   `photo_keywords`-Join-Tabelle), keine Hierarchie/Synonyme/Auto-
   Vervollständigung (siehe ADR-0022 — das ist auf Phase 6 verschoben).
3. **Sammlungen sind in Phase 3 rein manuell** (`collections` +
   `collection_photos` mit fester `position`-Reihenfolge), keine
   Sammlungssätze oder intelligenten Sammlungen mit Regeln (Phase 6).
4. **`photos_fts` ist eine FTS5-External-Content-Virtualtabelle** über
   `photos` (referenziert die Originalspalten statt sie zu duplizieren),
   mit `INSERT`/`UPDATE`/`DELETE`-Triggern auf `photos`, die den Index
   automatisch synchron halten — das ist SQLite FTS5s empfohlenes
   Standardmuster für Volltextsuche über bereits vorhandene Tabellen,
   vermeidet doppelte Datenhaltung und manuelle Sync-Logik in Rust.

**Konsequenzen:** Migration 3 bleibt rein additiv zu den Migrationen 1/2
(keine bestehende Spalte/Tabelle wird geändert). Die FTS5-Trigger sind
reines SQL in der Migrationsdatei, keine zusätzliche Rust-Logik zum
Index-Pflegen nötig.

---

## ADR-0024: Rasteransicht und Filmstreifen teilen sich Fotoliste + Auswahl-State

**Status:** Angenommen
**Kontext:** Phase 3 bringt eine neue Rasteransicht (`GridView.tsx`)
neben dem bestehenden Filmstreifen (`Filmstrip.tsx`, Phase 1). Beide
zeigen dieselbe Fotoliste des aktuell gewählten Ordners/Sammlung/Filters
und brauchen Mehrfachauswahl (für Bewertung/Flagge/Sammlung-Hinzufügen
als Stapel-Aktion).

**Entscheidung:** Fotoliste und Auswahl-State (inkl. Mehrfachauswahl per
Shift/Strg-Klick) leben im gemeinsamen Zustand-Store (`store/index.ts`),
nicht dupliziert in beiden Komponenten. Beide Ansichten nutzen dieselbe
Virtualisierungsbibliothek `@tanstack/react-virtual` (schon Abhängigkeit
seit Phase 1), nur mit unterschiedlichem Layout (1D-Reihe vs. 2D-Raster).

**Konsequenzen:** Ein Foto in einer der beiden Ansichten auszuwählen,
spiegelt sich sofort in der anderen wider — kein Synchronisationscode
nötig, weil es nur einen State gibt.

---

## ADR-0025: Import-Kopieren/Verschieben additiv zum bestehenden Hinzufügen, DNG-Konvertierung verschoben

**Status:** Angenommen
**Kontext:** Phase 1s Import scannt Dateien nur an ihrem bestehenden Ort
("Hinzufügen", `ImportMode::AddInPlace` implizit). `SPEC.md` §5 nennt für
Phase 3 zusätzlich Kopieren/Verschieben in einen verwalteten Zielordner.
DNG-Konvertierung (RAW → DNG beim Import) ist in §3.1 Teil desselben
Aufzählungspunkts, aber nicht in §5s Phase-3-Satz namentlich erwähnt und
technisch ein eigenständiges Feature (braucht einen DNG-Writer, den es im
Projekt noch nicht gibt).

**Entscheidung:** `ImportMode { AddInPlace, Copy(PathBuf), Move(PathBuf) }`
als neuer, additiver Parameter — der bestehende Scan-/Metadaten-/
Thumbnail-Ablauf (`import::run`) ändert sich für `AddInPlace` nicht.
DNG-Konvertierung verschoben auf Phase 5 (Export/Publish), siehe
ADR-0022.

**Konsequenzen:** Keine Verhaltensänderung für bestehende Aufrufer, die
weiterhin implizit `AddInPlace` nutzen.

## ADR-0026: Suche/Filter alternativ statt kombiniert; zwei Über-Scope-Punkte, die ADR-0022 übersehen hatte, nachträglich korrigiert

**Status:** Angenommen
**Kontext:** Bei der Abnahme von Phase 3 (Schritt 7) fielen zwei Lücken
auf, die bei der Scope-Korrektur in Schritt 0 (ADR-0022) hätten
mitkorrigiert werden müssen, es aber nicht wurden:

1. `FEATURES.md` §3.1 hatte "Filterleiste (Text, Attribut, Metadaten,
   **kombiniert**)" auf Phase 3 getaggt — `SPEC.md` §5s Phase-3-Satz
   nennt nur "Filter", ohne das Kombinieren von Text- und
   Attributsuche zu verlangen. Das tatsächlich gebaute
   `FilterBar.tsx`/`store/index.ts` implementiert Freitextsuche
   (`search_photos`, FTS5) und Attributfilter (`filter_photos`,
   inklusive eines nachträglich ergänzten Kameramodell-Chips) als
   **Alternative** statt als kombinierbare UND-Verknüpfung: Setzen des
   einen leert das andere. Eine echte Kombination bräuchte entweder
   eine dritte Backend-Route, die FTS5-`MATCH` und die
   Attribut-`WHERE`-Klausel in derselben Abfrage verknüpft, oder eine
   clientseitige Nachfilterung der Suchergebnisse — beides war im
   verbleibenden Schritt-6-Budget nicht mehr sauber umsetzbar.
2. "Duplikaterkennung per exaktem Hash" und "Sortierung nach beliebigem
   Feld" standen in `FEATURES.md` §3.1 ebenfalls auf Phase 3, kommen
   aber in `SPEC.md` §5s Phase-3-Satz nicht vor — nach genau derselben
   Regel wie ADR-0022 (Phase-3-Satz ist maßgeblich, nicht §3.1s voller
   Katalog) hätten sie schon in Schritt 0 auf eine spätere Phase
   umgetaggt werden müssen. Keins von beiden wurde in Phase 3 gebaut
   (`content_hash` wird beim Import weiterhin nicht berechnet; Raster/
   Filmstreifen sortieren fest nach Dateiname).

**Entscheidung:**
- Suche/Filter bleiben bewusst alternativ (siehe oben) — als
  "Fertig (abweichend, siehe DECISIONS.md)" in `FEATURES.md` markiert,
  nicht als unerledigt liegen gelassen.
- "Duplikaterkennung per exaktem Hash" auf Phase 6 umgetaggt (neben der
  dort bereits stehenden Perceptual-Hash-Duplikaterkennung — beide
  Ausbaustufen derselben Fähigkeit gehören zusammen).
- "Sortierung nach beliebigem Feld" auf Phase 6 umgetaggt (neben
  Filter-Presets/Schnellentwicklung im Raster, mit denen es inhaltlich
  zusammenhängt).

**Weiterer bekannter Stand (kein Umtaggen nötig, aber hier
dokumentiert):** "Ordnerbaum-Synchronisation" ist nur teilweise
gebaut — die Sidebar-Baumdarstellung, `relink_folder` und die
Fehlend-Erkennung (Schritt 5) funktionieren korrekt für jede
`parent_id`, die in der Datenbank steht, aber der Import selbst legt
weiterhin nur den unmittelbaren Elternordner jeder Datei an
(`ensure_folder` in `apx-app/src/import/mod.rs`), ohne die volle
Verzeichniskette bis zum gewählten Import-Ordner als `parent_id`-Kette
nachzubilden — das war so bereits in `PLAN.md` Phase 3 Schritt 5s
Beschreibung angelegt ("Sidebar bekommt eine echte Baumdarstellung"),
nicht in ADR-0022s Scope-Fehler. Eine echte Mehrebenen-Population beim
Import (begrenzt auf den gewählten Import-Ordner, nicht bis zum
Dateisystem-Wurzelverzeichnis) wäre ein sinnvoller kleiner Folgeschritt,
ist aber kein Phase-3-Blocker, da die Fotos unabhängig davon korrekt
einem (flachen) Ordner zugeordnet werden.

**Konsequenzen:** `FEATURES.md` korrigiert (siehe dortige Zeilen);
Definition-of-Done in `PLAN.md` Schritt 7 bewertet diese drei Punkte
entsprechend ehrlich statt sie stillschweigend als erledigt
auszuweisen.

## ADR-0027: Fünf im Abschlussbericht ehrlich benannte Lücken nachträglich in Phase 3 geschlossen

**Status:** Angenommen
**Kontext:** Im Abschlussbericht zu Phase 3 (Schritt 7) wurden fünf Lücken
offen benannt statt stillschweigend übergangen — DoD-Kriterium 5 aus
`SPEC.md` §7 (Undo/Redo) sowie die beiden ADR-0026-Umtaggungen
(Duplikaterkennung, Sortierung), die bewusst alternative statt
kombinierte Suche/Filter, und die teilweise Ordnerbaum-Population beim
Import. Auf ausdrücklichen Nutzerwunsch werden **genau diese fünf
Punkte** jetzt nachgezogen — nicht der komplette restliche
BIBLIOTHEK-Katalog aus `SPEC.md` §3.1 (Gesichtserkennung, Stapel,
Schlagwort-Hierarchie, intelligente Sammlungen, Metadaten-Presets/
Batch-Editing, voller EXIF/IPTC/XMP-Editor + Sidecar-Export,
Katalog-Backup/-Reparatur/-Merge, Perceptual-Hash-Duplikate,
DNG-Konvertierung, Cheatsheet-Overlay bleiben wie in ADR-0022 auf ihre
jeweils spätere Phase verteilt).

**Entscheidung, je Punkt:**

1. **Undo/Redo für Bibliotheks-Metadaten** (Bewertung/Flagge/Farbe/
   Schlagworte/Sammlungsmitgliedschaft): reines Frontend-Feature, keine
   neue Backend-Verlaufstabelle — anders als beim Entwickeln-Verlauf
   (`edit_history`, ADR-0014) gibt es hier keinen natürlichen Ort für
   einen Verlauf im Katalog; die Wahrheit bleibt der zuletzt bekannte
   Frontend-Zustand. Neu `frontend/src/lib/undoStack.ts` (reine,
   getestete Stack-Logik: `pushUndo`/`undo`/`redo`, neuer Push verwirft
   die Redo-„Zukunft", gleiches Prinzip wie ADR-0014), `store/index.ts`
   bekommt `libraryUndoStack`/`libraryRedoStack` plus
   `undoLibraryAction`/`redoLibraryAction`; `App.tsx`s globaler
   Tastatur-Handler behandelt Strg/Cmd+Z / Strg/Cmd+Umschalt+Z, aber nur
   wenn das Entwickeln-Panel nicht offen ist (das hat schon seinen
   eigenen lokalen Handler in `DevelopPanel.tsx`). Bewusst **nicht**
   abgedeckt: Sammlung anlegen/umbenennen/löschen (strukturelle
   Aktionen mit unklarer Undo-Semantik bei Neuvergabe der ID). Bekannter
   Grenzfall bei „Zu Sammlung hinzufügen": Rückgängig entfernt alle
   Fotos, die diese eine Aktion hinzugefügt hat — war eines davon schon
   vorher Mitglied, entfernt Rückgängig es trotzdem (keine
   Mitgliedschafts-Historie pro Foto).
2. **Duplikaterkennung per exaktem Hash**: neue direkte Abhängigkeit
   `sha2` (schon transitiv im `Cargo.lock` vorhanden, Version 0.10.9,
   `MIT OR Apache-2.0`, damit kein neues Lizenzrisiko). Jede beim Import
   gestagte Datei bekommt einen per Streaming berechneten
   SHA-256-Hex-Digest (`BufReader` + `std::io::copy` direkt in den
   Hasher, kein Volleinlesen) in `photos.content_hash` — die Spalte
   existierte seit Phase 1, war aber immer `NULL` (ihr Migrations-
   Kommentar nennt noch ein ursprünglich angedachtes xxHash-Teilhash-
   Schema aus der Phase-1-Planung; das wird hier bewusst nicht
   nachgebaut, Migrationen werden nie nachträglich geändert). Neue
   `Catalog::list_duplicate_photo_groups()` gruppiert nach `content_hash`
   (nur Fotos mit gesetztem Hash, `HAVING COUNT(*) > 1`); `run_with_mode`
   ruft sie am Ende auf und meldet die Gesamtzahl betroffener Fotos über
   ein neues `ImportFinishedPayload.duplicate_count`-Feld. Duplikate
   werden nur angezeigt (Header-Text nach Import, "Duplikate
   anzeigen"-Knopf in der Filterleiste) — der Import selbst wird dadurch
   nicht blockiert oder verändert.
3. **Sortierung nach beliebigem Feld**: bewusst client-seitig
   (`frontend/src/lib/sortPhotos.ts`) statt als weiterer
   SQL-`ORDER BY`-Parameter durch mehrere Backend-Abfragen
   durchgereicht — die komplette Fotoliste ist wegen der Virtualisierung
   (Raster/Filmstreifen) ohnehin schon im Speicher, serverseitiges
   Sortieren brächte keinen Zusatznutzen. `PhotoDto` bekommt dafür ein
   neues `file_size`-Feld (bisher nur intern im Katalog vorhanden).
   Felder: Dateiname (Default, entspricht dem bisherigen impliziten
   Verhalten), Aufnahmedatum, Bewertung, Dateigröße, Kameramodell;
   fehlende Werte sortieren immer ans Ende, unabhängig von der Richtung.
4. **Kombinierte Suche + Filter**: `repository/search.rs` bekommt einen
   gemeinsamen Klausel-Baukasten `build_filter_clause` (aus
   `filter_photos` herausgezogen, das unverändert bestehen bleibt) und
   eine neue `search_and_filter_photos` — mit Suchtext ein FTS5-`MATCH`
   plus die Kriterien-Klauseln per UND, ohne Suchtext identisch zu
   `filter_photos`. Additiv: `search_photos`/`filter_photos` (Commands
   und `Catalog`-Methoden) bleiben zusätzlich bestehen. Die frühere
   ADR-0026-Entscheidung "bewusst alternativ" ist damit zurückgenommen.
5. **Volle Ordnerbaum-Hierarchie beim Import**: begrenzt auf den
   gewählten Import-Ordner (bzw. bei Copy/Move auf den *Zielordner* —
   dort leben die Dateien danach, nicht mehr im Quellordner) — nicht bis
   zum Dateisystem-Wurzelverzeichnis. `ensure_folder` legt jetzt
   rekursiv alle Elternordner zwischen dieser Hierarchie-Wurzel
   (inklusive, `parent_id = NULL`) und dem unmittelbaren Elternverzeichnis
   der Datei an; liegt ein Verzeichnis unerwartet nicht unter der
   Hierarchie-Wurzel (z. B. ein aus dem Baum herausführender Symlink),
   bekommt es defensiv `parent_id = NULL` statt weiter zu rekursieren.
   Da Copy/Move-Zielordner aktuell immer flach bleiben (keine
   Unterordner-Struktur wird beim Kopieren/Verschieben nachgebildet),
   wirkt sich das in der Praxis nur auf `AddInPlace`-Importe mit
   verschachtelten Unterordnern aus.

**Konsequenzen:** `FEATURES.md` markiert "Duplikaterkennung per exaktem
Hash" und "Sortierung nach beliebigem Feld" wieder als Phase 3/Fertig,
die Filterleisten-Zeile von "abweichend, alternativ" auf "Fertig
(kombiniert)"; `PLAN.md` bekommt einen neuen Abschnitt "Schritt 8 —
Nachtrag"; `THIRD_PARTY.md` bekommt einen neuen `sha2`-Eintrag. Alle
übrigen, größeren zurückgestellten Punkte aus ADR-0022 bleiben
ausdrücklich auf ihrer jeweils späteren Phase.

## ADR-0028: Phase-4-Scope präzisiert — Workflow-Punkte verschoben, Objektivprofile und Reparatur bewusst vereinfacht

**Status:** Angenommen
**Kontext:** `SPEC.md` §5s Phasenplan-Satz für Phase 4 nennt namentlich
zehn Werkzeugkategorien: „Kurven, HSL, Farbmischer, Color Grading,
Details, Objektivkorrekturen, Effekte, Kalibrierung, Crop/Geometrie,
Reparatur." `FEATURES.md` §3.4 („Modul ENTWICKELN — Workflow") taggt
zusätzlich acht Punkte (Schnappschüsse, Vorher/Nachher, Einstellungen
kopieren/einfügen, Vorherige übernehmen, Synchronisieren, Auto-Sync,
Referenzansicht, Soft-Proof) als Phase 4 — dieselbe Art Diskrepanz
zwischen dem maßgeblichen §5-Satz und `FEATURES.md`s eigener, weiter
gefasster Interpretation, die schon ADR-0011 und ADR-0022 für frühere
Phasen korrigiert haben.

Zusätzlich verlangen zwei der zehn §5-Kategorien laut `SPEC.md` §3.2
Funktionalität, die ohne mehrwöchige Vorarbeit bzw. externe Testdaten
nicht seriös umsetzbar ist: eine echte, Adobe-LCP-kompatible
Objektivprofil-Datenbank („Datenbank mit Profilen, eigene Profile
importierbar") und echte Computer-Vision-Algorithmen für die
Reparatur-Funktion („Auto-Quellenfindung", „Inhaltsbasiertes Füllen für
größere Bereiche", vergleichbar mit Photoshops Content-Aware
Fill/PatchMatch). Diese Entscheidung wurde dem Nutzer explizit zur Wahl
vorgelegt (nicht stellvertretend getroffen wie bei ADR-0011), da sie den
Umfang stärker einschränkt als eine reine Nachschlage-Korrektur.

**Entscheidung (vom Nutzer bestätigt):**

1. **Workflow-Punkte verschoben:** alle acht `FEATURES.md`-§3.4-Zeilen
   wandern auf Phase 6, wo laut `ARCHITECTURE.md` §7 bereits das
   Maskensystem als fortgeschrittenes Entwickeln-Feature geplant ist.
   Phase 4 umfasst ausschließlich die zehn im §5-Satz genannten
   Werkzeugkategorien plus die per ADR-0011 bereits zugewiesenen
   Grundeinstellungs-Ergänzungen (WB-Pipette, WB-Kamera-Presets, Textur,
   Klarheit, Dunst entfernen, Dynamik, Sättigung).
2. **Objektivkorrekturen:** eigenes, minimales Profilformat
   (handgepflegtes JSON, wenige Beispielprofile, Zuordnung per
   EXIF-Objektiv-/Kamerastring) statt einer echten Adobe-kompatiblen
   Profildatenbank. Die vollen manuellen Regler (chromatische
   Aberration automatisch und manuell, Vignettierung, Verzeichnung,
   Perspektive/Upright, manuelle Transformation) werden trotzdem
   komplett gebaut — nur der *Import fremder Profile* (Adobe LCP/DNG
   Lens Profile) entfällt und wird auf eine spätere Phase verschoben.
   Aus demselben Grund entfällt in der Kalibrierung der DCP-Import für
   Kameraprofile; eine kleine eingebaute Profilliste bleibt.
3. **Reparatur:** manuelles Klonen/Reparieren (Pinsel mit Quellpunkt,
   Radius, Deckkraft, weicher Kante; Reparieren per vereinfachtem
   nahtlosen Überblenden, nicht echtem Poisson-Blending) ist die
   Phase-4-Basis. Auto-Quellenfindung und echtes inhaltsbasiertes Füllen
   für größere Bereiche werden auf Phase 6 verschoben (analog zur
   Perceptual-Hash-Duplikaterkennung, die in ADR-0022 aus demselben
   Grund — fortgeschrittener Algorithmus ohne unmittelbaren
   Minimal-Nutzen — zurückgestellt wurde).
4. **Weitere bewusste Vereinfachungen, die aus denselben Gründen
   zusammen mit den obigen drei Punkten entschieden wurden:** der
   „Guided"-Upright-Modus bekommt 2 statt bis zu 4 Linienpaare;
   „Auto-Ausrichtung am Horizont" nutzt nur die EXIF-Orientierung statt
   eines echten Kantenerkennungs-Verfahrens. Beide sind CV-artige
   Detailfragen derselben Kategorie wie Punkt 2/3, keine eigenständige
   Entscheidung.
5. **Struktur:** Phase 4 bleibt eine durchgehende Phase mit
   feingranularen Schritten (0–13, siehe `PLAN.md`) statt in Unterphasen
   4a/4b gesplittet — die architektonische Zweiteilung (per-Pixel-Modell
   vs. Nachbarschafts-/größenverändernde Operationen) spiegelt sich in
   der Schrittfolge (Schritt 2 legt die Infrastruktur für beide Modelle
   an, spätere Schritte nutzen sie je nach Werkzeug), nicht in getrennten
   Phasen mit eigener Abnahme.

**Konsequenzen:** `FEATURES.md` §3.4 komplett auf Phase 6 umgetaggt;
§3.2s Objektivkorrekturen-/Kalibrierung-/Reparatur-/Geometrie-Abschnitte
bekommen erklärende Kommentare zu den vier Vereinfachungen aus Punkt
2–4, die einzelnen Zeilen bleiben bis zur tatsächlichen Umsetzung auf
„Nicht begonnen" (Auto-Quellenfindung/Inhaltsbasiertes Füllen direkt auf
Phase 6 umgetaggt, da sie in Phase 4 überhaupt nicht gebaut werden).
`PLAN.md` bekommt den neuen Abschnitt „Aktuelle Phase: Phase 4" mit der
Schrittfolge 0–13. Ein echter Adobe-Profil-/DCP-Import sowie
Auto-Quellenfindung/Content-Aware-Fill bleiben in `FEATURES.md` als
offene Phase-6-Punkte sichtbar, nicht stillschweigend fallen gelassen.

**Nachtrag (Schritt 12, Reparatur-Umsetzung):** zwei weitere kleine
Vereinfachungen aus derselben Kategorie wie Punkt 4 (CV-artige
Detailfragen ohne unmittelbaren Minimal-Nutzen), zusammen mit der
tatsächlichen Umsetzung entschieden:
- **Pfad-Ausdünnung:** jeder Strich ist auf `MAX_PATH_POINTS = 32`
  Stützpunkte gedeckelt (`repair.rs`) — das Frontend dünnt einen dicht
  abgetasteten Zeigerpfad beim Loslassen entsprechend aus
  (`RepairOverlay.tsx`s `thinPath`). Bei den Zeigerauflösungen üblicher
  Pinselstriche visuell nicht von einer ungedeckelten Punktzahl zu
  unterscheiden.
- **Live-Vorschau beim Malen ist rein clientseitig:** das SVG-Overlay
  zeigt den gerade gemalten Pfad sofort, der tatsächliche
  Pipeline-Effekt erscheint erst nach dem Loslassen (committeter,
  ausgedünnter Strich) — ein voller Entwickeln-Durchlauf über einen
  wachsenden, noch nicht ausgedünnten Pfad bei jeder Zeigerbewegung wäre
  unnötig teuer.
- **Sensorflecken-Visualisierung** (`FEATURES.md` §3.2) wird ebenfalls
  auf Phase 6 verschoben — sie setzt eine automatische Fleckenerkennung
  voraus (Blob-/Kantenerkennung auf dem Bild), dieselbe Kategorie
  Bildverarbeitungsaufgabe wie die bereits zurückgestellte
  Auto-Quellenfindung, ohne die sie nur eine leere Overlay-Hülle wäre.

## ADR-0029: Schritt 2 baut nur die Grundeinstellungs-Regler + die GPU-Dispatch-Grundlage; Kurven/HSL/Farbmischer/Color-Grading/Kalibrierung/Effekte bekommen ihren eigenen Shader in ihrem eigenen Schritt

**Status:** Angenommen
**Kontext:** Der ursprüngliche Plan für Schritt 2 sah vor,
`stages/basic_fused.wgsl`/`.rs` sofort um praktisch alle verbleibenden
1:1-/positions-bewussten Werkzeugkategorien zu erweitern (Kurven, HSL,
Farbmischer, Color Grading, Kalibrierung, Vignette, Körnung) — bevor
deren Frontend-Verträge (welche Felder, welche Interaktion) in ihren
jeweils eigenen Schritten (4–10) überhaupt feststehen. Das würde
`SPEC.md` §6s Grundsatz „jede Operation ein eigenes Modul mit eigenem
Shader, eigenem Test" verletzen (ein einziges Mega-Modul für acht
verschiedene Werkzeuge) und spekulative Arbeit an Formeln leisten, die
noch keinen Abnehmer haben.

**Entscheidung:** Schritt 2 liefert nur:
1. Die Prüfung/Bestätigung, dass `gpu/dispatch.rs::run_compute_f32`
   unverändert sowohl positions-bewusste als auch nachbarschafts-fähige
   Operationen trägt (Breite/Höhe als zusätzliche `Params`-Felder,
   uneingeschränkter Lesezugriff auf den gesamten Eingabepuffer im
   WGSL-Shader) — keine Änderung an `dispatch.rs` selbst nötig.
2. Die letzten fünf der zwölf Grundeinstellungs-Regler
   (Dunst&nbsp;entfernen/Dynamik/Sättigung in `basic_fused`,
   Textur/Klarheit im neuen, eigenen `stages/local_contrast.{rs,wgsl}` —
   Letztere brauchen echten Nachbarschafts-Zugriff, siehe dessen
   Moduldoku) — damit sind alle zwölf Regler aus `SPEC.md`s
   „Grundeinstellungen" fertig.

Kurven, HSL, Farbmischer, Color Grading, Kalibrierung und Effekte
bekommen stattdessen je ein eigenes Modul mit eigenem Shader und
eigenem GPU/CPU-Paritätstest in ihrem jeweils eigenen Schritt (4–10),
genau wie die sieben Phase-2-Regler es taten — nur der interaktive
Vorschau-Pfad fasst sie am Ende jeweils per ADR-0017-Muster zu einem
Dispatch zusammen, sobald ihre Formel feststeht.

**Konsequenzen:** `PLAN.md`s Schritt-2-Checkliste wird auf die zwei
oben genannten Punkte präzisiert; die „GPU/CPU-Paritätstests je neuem
Teil-Feature"-Zeile bezieht sich nur noch auf Dunst
entfernen/Dynamik/Sättigung/Textur/Klarheit (bereits erledigt), nicht
mehr auf die acht später gebauten Kategorien. Die
Kurven-Sequenzierungsfrage (Farbraum-Konvertierung ins WGSL verlagern
oder Kurven als CPU-LUT-Nachschritt) verschiebt sich entsprechend auf
Schritt 4, wenn Kurven tatsächlich gebaut werden.

## ADR-0030: Objektivkorrekturen (Schritt 9) — Ausgabegröße bleibt unverändert, Perspektive/Upright „Auto"/„Level"/„Vertical"/„Full" bleiben No-op-Platzhalter

**Status:** Angenommen
**Kontext:** `PLAN.md`s Architektur-Grundsatz sah für Objektivkorrekturen
eine „größenverändernde" GPU-Dispatch-Form vor (Ausgabepuffer ≠
Eingabepuffer). Beim tatsächlichen Bau von
`stages/lens_corrections.{rs,wgsl}` zeigte sich: Verzeichnung, CA,
Vignette-Korrektur und manuelle Transformation lassen sich vollständig
als inverse Abbildung mit bilinearer Abtastung ausdrücken, bei der
Ausgabe- und Eingabegröße identisch bleiben (Randpixel werden
geklemmt statt schwarz gefüllt oder automatisch zugeschnitten) — echtes
Zuschneiden auf den gültigen Bildbereich ist ohnehin Aufgabe von
Schritt 11s separatem Geometrie-Werkzeug (das laut Plan bereits als
eigener „CPU-seitiger Crop+Rotate+Resample als letzter Schritt in
`render_rgba8`" vorgesehen ist). Eine echte größenverändernde
Dispatch-Form für Schritt 9 zu bauen wäre doppelte Arbeit gewesen, die
Schritt 11 ohnehin leisten muss.

Zweitens: Die SPEC.md-Perspektive/Upright-Modi „Auto", „Level",
„Vertical" und „Full" setzen in echten Bildbearbeitungsprogrammen eine
automatische Kantenerkennung zur Fluchtpunkt-Bestimmung voraus — eine
CV-Aufgabe vergleichbar mit der in ADR-0028 bereits für Schritt 11s
Auto-Ausrichtung zurückgestellten Kantenerkennung, ohne die dafür
nötigen Bildverarbeitungsbausteine (Kantendetektion, Linienerkennung,
Homografie-Schätzung) im bisherigen Stack.

**Entscheidung:**
1. Objektivkorrekturen liefern die geometrische Abbildung als
   Ein-/Ausgabepuffer-gleich-großen inversen Warp mit bilinearer
   Abtastung (keine größenverändernde Dispatch-Form in Schritt 9 —
   die bleibt für Schritt 11 reserviert).
2. Die Upright-Modi „Auto"/„Level"/„Vertical"/„Full" sind im EDL/UI
   wählbar, tragen aber aktuell keine Wirkung (dokumentierter
   No-op-Platzhalter). „Guided" (die einzige Nutzer-gesteuerte Variante)
   nutzt die ersten zwei markierten Hilfslinien und errechnet daraus
   eine einfache gemittelte Dreh-Korrektur — keine echte
   Mehrlinien-Fluchtpunkt-Homografie.

**Konsequenzen:** `FEATURES.md` markiert die vier automatischen
Upright-Modi als „Fertig (abweichend, siehe ADR-0030)" statt als volle
Automatik. Eine echte Kantenerkennungs-/Homografie-basierte
Upright-Automatik sowie ein echtes Zuschneiden auf den gültigen
Bildbereich nach der geometrischen Korrektur bleiben auf eine spätere
Phase verschoben (zusammen mit den bereits in ADR-0028 zurückgestellten
Punkten: echter Adobe-Profil-Import, Auto-Quellenfindung/
Content-Aware-Fill, echte Auto-Horizont-Kantenerkennung).

## ADR-0031: Phase-5-Scope präzisiert — Preset-Grundlagen jetzt, KI-Generator/Adobe-Interop/Templates-Unterabschnitt verschoben; kein eigenes `apx-presets`-Crate

**Status:** Angenommen
**Kontext:** `SPEC.md` §5 nennt für Phase 5 wörtlich nur „Preset- und
Template-System" — `SPEC.md` §3.5 (der komplette Feature-Katalog dahinter)
ist aber mit Abstand der am weitesten in die Zukunft greifende Abschnitt
des ganzen Dokuments: er setzt eine echte LLM-Anbindung (Anthropic API,
API-Key-Verwaltung), eine numerische Referenzbild-Optimierung über die
Pipeline-Parameter, vollständige Adobe-`.xmp`/`.lrtemplate`-Kompatibilität
in beide Richtungen, und — beim „Templates"-Unterabschnitt — mehrere
Subsysteme voraus, die schlicht noch nicht existieren: `ARCHITECTURE.md`
§7 reserviert die Export-Engine explizit erst für Phase 8–9. Ein
„Export-Template" jetzt zu bauen hieße, eine Konfigurationsoberfläche für
ein Subsystem zu bauen, das sie noch gar nicht ansteuern kann — dieselbe
Art von verfrühter Arbeit, die ADR-0029 für Phase 4 bereits vermieden hat
(kein Shader für ein Werkzeug bauen, bevor sein Frontend-Vertrag
feststeht). Genau wie bei ADR-0011/ADR-0022/ADR-0028 gilt: der §5-Satz ist
normativ, der vollständige §3.x-Katalog wird hier scope-präzisiert.

**Entscheidung:**
1. **Preset-Grundlagen** (`SPEC.md` §3.5, erster Unterabschnitt) sind die
   Phase-5-Basis und werden vollständig gebaut: EDL-Teilmengen-Presets
   (Speichern-Dialog mit Checkbox je Einstellungsgruppe), Ordnerhierarchie
   beliebiger Tiefe, Favoriten, Suche/Tags, Preset-Stärke (0–200 %,
   nachträglich änderbar solange kein anderer Edit dazwischen liegt),
   Live-Vorschau (Hover im Bild + Thumbnail in der Liste), Preset-Stapel
   (mehrere nacheinander, Reihenfolge editierbar), eigenes `.apx`-
   Dateiformat-Im-/Export, Versionierung mit Diff-Ansicht, sowie eine
   vereinfachte Fassung der bedingten Presets (siehe Punkt 4).
2. **Preset-Generator (KI)** — LLM-Anfrage, Referenzbild-Modus,
   Variationen-Generator, „Preset aus Bearbeitung lernen" — wird
   komplett auf **Phase 7** verschoben. `ARCHITECTURE.md` §7 reserviert
   diese Phase bereits wörtlich für „`apx-ai` — ONNX-Runtime-Integration,
   LLM-Client für Preset-Generator" — das ist keine neue Zurückstellung,
   sondern nur die Bestätigung der bereits bestehenden Architektur-Planung.
   Ein Referenzbild-Optimierer (Gradientenverfahren über die
   Pipeline-Parameter) ist zudem dieselbe Kategorie CV-/Optimierungs-
   Aufgabe wie die in ADR-0028 zurückgestellten Punkte.
3. **Adobe-`.xmp`/`.lrtemplate`-Import/-Export** wird auf eine spätere
   Phase verschoben — derselbe Grund wie beim Objektivprofil-/DCP-Import
   in ADR-0028 (kein Adobe-Format-Reverse-Engineering ohne Testdaten als
   eigenständiges Mammutprojekt neben dem eigentlichen Feature). Das
   eigene `.apx`-Format deckt Im-/Export vollständig ab.
4. **Bedingte Presets** werden bewusst vereinfacht gebaut statt ganz
   zurückgestellt (echte, nicht nur angetäuschte Fähigkeit ohne externe
   Abhängigkeit): eine feste, kleine Liste vergleichbarer Metadatenfelder
   (ISO, Blende, Brennweite, Kameramodell, Objektiv — alle bereits in
   `photos` gespeichert), UND-verknüpfte Regeln (Feld, Operator, Wert),
   **kein** UI-Builder für eine freie Bedingungssprache mit ODER/
   Verschachtelung.
   **Nachtrag (Schritt 7):** jede Regel trägt zusätzlich ein optionales
   `section`-Feld — `null` bedeutet „gilt fürs ganze Preset" (Fehlschlag
   verhindert das Anwenden komplett), eine gesetzte Sektion grenzt einen
   Fehlschlag auf genau diese Sektion ein (Rest des Presets bleibt
   wirksam). Ein fehlendes Metadatum am aktuellen Foto (z. B. kein
   EXIF-ISO-Wert) lässt die betroffene Regel konservativ als nicht
   erfüllt gelten, statt sie zu ignorieren.
5. **„Templates" (über Presets hinaus)** — Export-/Wasserzeichen-/
   Metadaten-/Import-/Umbenennungs-/Layout-/Workflow-Templates,
   Template-Marktplatz — wird komplett auf eine spätere Phase verschoben.
   Jedes davon konfiguriert ein Subsystem, das entweder noch gar nicht
   existiert (Export-Engine: Phase 8–9, siehe oben; Layout-Templates für
   Druck/Buch/Diashow/Web: in keinem früheren Phasenplan erwähnt) oder nur
   in Ansätzen (Umbenennungs-Templates: `import::rename` existiert bereits
   rudimentär aus Phase 3, ein vollwertiger Token-Editor bräuchte ohnehin
   erst die anderen Import-Template-Bausteine). Diese Punkte wandern in
   `FEATURES.md` auf die jeweilige Phase, in der ihr zugehöriges
   Subsystem tatsächlich gebaut wird (überwiegend Phase 8–9).
6. **Kein eigenes `apx-presets`-Crate**, obwohl `ARCHITECTURE.md` §7 eines
   vorab benannt hatte: die Phase-5-Basis aus Punkt 1 braucht keine neue
   Pixelverarbeitungslogik — ein Preset ist reine Katalogdaten (Name,
   Ordner, Tags, Bedingungen, eine EDL-*Teilmenge* als opakes JSON) plus
   Frontend-seitiges Zusammenführen/Skalieren in den bestehenden
   `developEdl`-Zustand vor dem ohnehin schon bestehenden
   `commitDevelopEdit`. Genau wie `apx-catalog`s `edit_history.edl_json`
   nie von `apx-catalog` selbst verstanden werden muss (siehe
   `ARCHITECTURE.md` §5), muss auch der Preset-JSON-Blob nie von
   `apx-catalog` verstanden werden — eine neue Erweiterung von
   `apx-catalog` (Migration + Repository-Modul) plus neue `apx-app`-
   Commands reichen. Ein leeres `apx-presets`-Crate jetzt anzulegen wäre
   dieselbe Art von Vorab-Arbeit ohne aktuellen Abnehmer, die ADR-0029
   bereits für Phase 4 vermieden hat. Sollte Phase 7s KI-Generator später
   echte, eigenständige Preset-Berechnungslogik brauchen (Referenzbild-
   Optimierung, Variationen-Generator), ist das der richtige Zeitpunkt,
   ein solches Crate tatsächlich anzulegen.
7. **Import-Templates/Umbenennungs-Templates werden nach Phase 5
   vorgezogen** (waren in `FEATURES.md` fälschlich als „Phase 3"
   getaggt, obwohl Phase 3 bereits abgeschlossen ist, ohne sie zu bauen —
   derselbe Fehlertyp, den ADR-0026/ADR-0027 für andere Zeilen schon
   einmal korrigiert haben). Der Rust-Unterbau existiert bereits
   vollständig aus Phase 3 (`apx-app::import::presets::ImportPreset` +
   die Commands `list_import_presets`/`save_import_preset`/
   `delete_import_preset`), nur ohne jede Frontend-Anbindung — kein
   Import-Dialog nutzt sie, kein Token-Editor existiert. Da beide Punkte
   ohnehin unter „Templates" fallen und ihr Unterbau bereits da ist,
   werden sie als kleiner, eigenständiger Schritt in Phase 5
   mitgenommen statt weiter unbenutzt zu bleiben.

**Konsequenzen:** `FEATURES.md` §3.5 wird zeilenweise auf „Phase 5"
(Preset-Grundlagen + vereinfachte bedingte Presets), „Phase 7" (KI-
Generator) bzw. „Phase 8–9"/„Phase 6" (Templates-Unterabschnitt, je nach
zugehörigem Subsystem) umgetaggt; Adobe-Interop bleibt als offener
späterer Punkt sichtbar. `PLAN.md` bekommt einen neuen Abschnitt „Aktuelle
Phase: Phase 5" mit einer feingranularen Schrittfolge, analog zu Phase 4.
`ARCHITECTURE.md` §7s Phase-5-Platzhalterzeile wird entsprechend
präzisiert (kein `apx-presets`-Crate, stattdessen `apx-catalog`-
Erweiterung + `apx-app`-Commands + Frontend-Merge-Logik).

## ADR-0032: Phase-6-Scope präzisiert — Maskensystem (§5) plus die in ADR-0028 versprochenen Workflow-Punkte; Bibliotheks-Backlog explizit auf Phase 9 verschoben; Tiefenbereich/KI-Masken zurückgestellt

**Status:** Angenommen
**Kontext:** `SPEC.md` §5 nennt für Phase 6 wörtlich nur „Masken und
lokale Anpassungen. Pinsel, Verläufe, Bereichsmasken, Maskenkombination,
Ebenen-Mischmodi." — das deckt sich mit `SPEC.md` §3.3 bis auf zwei
Punkte, die dort zusätzlich stehen: Tiefenbereich-Masken (brauchen
Tiefendaten, die in keiner früheren Phase je erzeugt wurden — keine
Tiefenschätzung, kein Tiefensensor-Import existiert) und die fünf
KI-Masken (Motiv/Himmel/Hintergrund/Objekte/Personen — Segmentierung per
ONNX-Runtime). Letztere sind wörtlich das, was `ARCHITECTURE.md` §7 für
Phase 7 reserviert („`apx-ai` — ONNX-Runtime-Integration"); sie jetzt
vorzuziehen hieße, eine komplette Modell-Inferenz-Pipeline parallel zum
eigentlichen Maskensystem aufzubauen — derselbe Fehler, den ADR-0031 für
den Preset-Generator vermieden hat.

Daneben tragen zwei Gruppen von `FEATURES.md`-Zeilen bereits die Marke
„Phase 6", ohne in `SPEC.md` §5s Phase-6-Satz vorzukommen:
1. Die acht Workflow-Punkte aus §3.4 (Schnappschüsse, Vorher/Nachher,
   Copy/Paste-Einstellungen, Vorherige übernehmen, Sync, Auto-Sync,
   Referenzansicht, Soft-Proof) — diese Zuordnung ist kein Versehen,
   sondern eine **bereits gegebene Zusage**: ADR-0028 hat sie explizit
   „auf Phase 6 verschoben", nicht generisch „auf später". Ein
   Versprechen aus einer bereits abgenommenen Phase ohne neue ADR wieder
   zu brechen wäre derselbe Fehlertyp, den ADR-0026/ADR-0027 an anderer
   Stelle schon einmal korrigiert haben — hier gilt also das Gegenteil:
   die Zusage wird eingehalten.
2. Ein rund ein Dutzend Bibliotheks-Zeilen aus §3.1 (Sammlungssätze/
   intelligente Sammlungen, Stapel, virtuelle Kopien, erweiterbare
   Farbmarkierungen, Schlagworthierarchie/-vorschläge/Tag-Regeln,
   Metadaten-Presets/Stapel-Metadatenbearbeitung/EXIF-IPTC-XMP-Editor/
   Sidecar-Export, Vergleichs-/Übersichtsansicht, Filter-Presets,
   Schnellentwicklung im Raster, Vorschau-Cache-Verwaltung/Smart
   Previews/Offline-Bearbeitung) — anders als Gruppe 1 stammt diese
   Zuordnung aus **keiner** ADR, die Phase 6 beim Namen genannt hätte;
   „Phase 6" war hier nur der nächste freie Platzhalter nach Phase 3s
   Abschluss, kein eingelöstes Versprechen. Alle zusammen wären, grob
   geschätzt, mindestens so viel Aufwand wie das eigentliche
   Maskensystem — sie parallel zu bauen würde Phase 6 auf etwa das
   Dreifache der in `SPEC.md` §5 tatsächlich benannten Arbeit aufblähen.

**Entscheidung:**
1. **Maskensystem-Kern** (`SPEC.md` §3.3, ohne Tiefenbereich/KI-Masken,
   siehe Punkt 3) wird vollständig gebaut: Maskentypen Pinsel, Linearer
   Verlauf, Radialer Verlauf, Farbbereich, Luminanzbereich; Kombination
   per Hinzufügen/Subtrahieren/Schneiden; echte Ebenen-Mischmodi pro
   Maske; Maskenverwaltung (Gruppen, Umbenennen, Ein-/Ausblenden,
   Überlagerungsfarbe, Duplizieren, auf anderes Foto übertragen, als
   wiederverwendbarer Baustein speichern, Kette mit Drag-&-Drop-
   Sortierung).
2. **Pro Maske stehen nur die ton-/farb-/detailbezogenen Werkzeuge zur
   Verfügung** — Grundeinstellungen, Kurven, HSL, Farbmischer, Color
   Grading, Details (Schärfung + Rauschreduzierung) — nicht
   Objektivkorrekturen, Effekte, Kalibrierung, Geometrie oder Reparatur.
   `SPEC.md` §3.3 sagt zwar wörtlich „alle globalen Regler", aber die
   ausgenommenen fünf sind strukturell Ganzbild-Operationen (ein
   Objektiv-Warp oder ein Crop pro Maske separat anzuwenden ergibt
   pipeline-technisch keinen Sinn — beide ändern bereits laut ADR-0030/
   der Phase-4-Pipeline-Reihenfolge die Geometrie des *gesamten* Bildes
   an einer festen Stelle im Ablauf, nicht pro Region) — eine genuine,
   nicht nur vorgetäuschte Fähigkeit für exakt die Werkzeugklasse, für
   die regionale Anwendung tatsächlich Sinn ergibt.
3. **Tiefenbereich-Masken** werden auf eine spätere Phase zurückgestellt
   (kein Tiefendaten-Zulieferer existiert — das wäre eine komplette
   Tiefenschätzungs- oder Sensor-Import-Pipeline als Vorprojekt).
   **KI-Masken** (Motiv/Himmel/Hintergrund/Objekte/Personen) werden auf
   **Phase 7** verschoben — dort baut `apx-ai` ohnehin erstmals die
   ONNX-Runtime-Integration auf, die diese Masken als Erstes brauchen.
4. **Architektur — Ebenenmodell statt Fused-Pass:** die Phase-4-Pipeline
   bleibt unverändert die Grundlage (`render_rgba8`s feste Stufenfolge,
   siehe `ARCHITECTURE.md` §8); Masken laufen danach als neue, letzte
   Gruppe von Stufen, jede Maske sequenziell: (a) Maskenalpha berechnen
   (Pinsel: akkumulierte Stempel-Textur aus den Pinselstrichen, analog
   zu `repair.rs`s Pfad-Uniform; Verläufe: analytische Funktion über
   Position; Farbbereich/Luminanzbereich: Klassifikation pro Pixel), (b)
   Kombinationsregeln mit vorangehenden Masken derselben Gruppe
   anwenden, (c) die in der Maske hinterlegten Werkzeuge auf eine Kopie
   des aktuellen Bildzustands anwenden (derselbe Fused-Pass-Baustein wie
   Phase 4, nur mit den Masken-EDL-Werten statt den globalen), (d)
   alpha-gewichtet mit dem gewählten Ebenen-Mischmodus in den
   Bildzustand zurückmischen. Jede Maske ist damit ein eigener
   Pipeline-Durchlauf — das 16-ms-Ziel wird bei vielen/komplexen Masken
   voraussichtlich Grenzen aufzeigen und ist Gegenstand der
   Abnahme-Schritt-Performance-Nachmessung, genau wie in Phase 2/4.
   **Nachtrag (Schritt 2, beim Bauen entdeckt):** „nach der
   Phase-4-Pipeline" war ungenau — Kurven laufen in der globalen
   Pipeline erst *nach* der Farbraum-Konvertierung auf dem fertigen
   RGBA8-Puffer, während Grundeinstellungen/HSL/Color Grading/Details im
   linearen Arbeitsraum *davor* laufen. Da eine Maske alle sechs
   Werkzeuge in einem Durchlauf anwendet, kann sie nicht an zwei
   Pipeline-Stellen zugleich sitzen. Präzisierung: die gesamte
   Maskenstufe läuft im linearen Arbeitsraum, direkt nach `effects` und
   vor der Farbraum-Konvertierung — Masken-Kurven bekommen dafür eine
   eigene `curves::apply_linear_rgb`-Funktion (dieselbe LUT, angewendet
   auf den linearen statt dem display-referred Tonwert), um eine
   verlustreiche zweite Farbraum-Konvertierung pro Maske zu vermeiden.
5. **EDL-Schema v3** (`crates/apx-pipeline/src/edl/v3.rs`) statt einer
   Erweiterung von `EdlV2` — derselbe Grund wie beim v1→v2-Sprung in
   Phase 4 (`migrate.rs` kennt keine automatische Feldergänzung, siehe
   ADR aus Phase 4 Schritt 1): `masks: Vec<Mask>` ist ein komplett neues
   Feld, `v2_to_v3` hebt bestehende `EdlV2`-Daten unverändert an, `masks`
   startet leer.
6. **Workflow-Punkte** (ADR-0028-Zusage, siehe Kontext) werden
   vollständig gebaut: Schnappschüsse (benannte, klickbare EDL-
   Zwischenstände zusätzlich zum linearen Verlauf), Vorher/Nachher in
   vier Ansichten, Einstellungen kopieren/einfügen mit granularer
   Sektionsauswahl (reine Frontend-Logik auf demselben `PresetEdlSubset`-
   Mechanismus wie Presets — kopiert einfach `developEdl` statt eines
   gespeicherten Presets), Vorherige übernehmen, Synchronisieren über
   beliebig viele ausgewählte Fotos, Auto-Sync-Modus, Referenzansicht,
   Soft-Proof (Zielprofil/Renderpriorität/Farbumfangswarnung/
   Papierweiß — vereinfacht auf die in `apx-pipeline` bereits
   vorhandenen Farbraum-Grundlagen, kein vollständiges ICC-
   Farbmanagement-Subsystem; echte Profilverwaltung wäre ein eigenes
   Mammutprojekt und ist hier nicht das Ziel).
   **Nachtrag (Schritt 8, beim Bauen entdeckt):** „ein Schnappschuss ist
   ein benannter Verweis auf einen bestehenden Verlaufs-Stand" (siehe
   `PLAN.md`s ursprüngliche Formulierung) war unsicher — ein Blick in
   `repository/edits.rs::commit` zeigt, dass jede Bearbeitung nach einem
   Rückgängig die „Zukunft" (Zeilen mit höherer Sequenznummer) hart
   löscht (ADR-0014). Ein Verweis auf so eine Zeile könnte also
   verschwinden, sobald man über einen Schnappschuss hinaus weiterarbeitet
   — das Gegenteil von „zusätzlich zum linearen Verlauf". Präzisierung:
   eine eigene, kleine `snapshots`-Tabelle mit eigener EDL-Kopie je
   Schnappschuss statt eines Verweises — kein Restore-Sonderweg nötig,
   ihr Anwenden ist derselbe `apply_develop_edit`-Aufruf wie jeder andere
   EDL-Stand.

   **Präzisierung (Schritt 10, beim Bauen entschieden):** Soft-Proof
   ("kein vollständiges ICC-Farbmanagement-Subsystem", siehe oben) ist
   als **rein clientseitige Nachbearbeitung** des bereits über die
   bestehende `develop/...`-Route gerenderten RGBA8-Vorschau-Puffers
   umgesetzt (`frontend/src/lib/softProof.ts`), nicht als neue
   Backend-/Pipeline-Stufe — `apx-pipeline` kennt bis heute nur eine
   feste Kamera→sRGB-Matrix plus Gammakurve, kein ICC-Profil-Laden oder
   3D-Gamut-Mapping (`crates/apx-pipeline/src/color/mod.rs`), ein neues
   Backend-Subsystem allein für eine Anzeige-Vorschau wäre unverhältnis-
   mäßig. Zielprofil/Renderpriorität/Farbumfangswarnung/Papierweiß sind
   entsprechend Näherungen (Sättigungs-Kompression Richtung Grauwert,
   Sättigungs-Schwellenwert statt echtem Gamut-Volumen, feste lineare
   Tonwert-Bereichskompression) — siehe `softProof.ts`s Moduldoku für
   Details. Betrifft nie den echten Export, nur die Anzeige.
7. **Bibliotheks-Backlog (Gruppe 2, siehe Kontext) wird explizit auf
   Phase 9 verschoben, nicht in Phase 6 mitgenommen.** Keine ADR hat
   Phase 6 für diese Zeilen je zugesagt; sie parallel zum Maskensystem
   und den acht Workflow-Punkten zu bauen würde diese ohnehin schon
   große Phase auf einen Umfang aufblähen, der `SPEC.md` §5s eigentlicher
   Phase-6-Beschreibung nicht mehr entspricht. Phase 9 („Fortgeschrittenes")
   ist bereits der Sammelpunkt für nachgezogene Reife-Themen (Node-
   Editor, Stacking, Tethering, Skript-API) und passt strukturell besser
   als ein weiteres Aufblähen von Phase 6.
8. **Reparatur-Erweiterungen aus ADR-0028** (Auto-Quellenfindung,
   Sensorflecken-Visualisierung, inhaltsbasiertes Füllen) werden von
   ihrer bisherigen `FEATURES.md`-Markierung „Phase 6" auf **Phase 7**
   verschoben. ADR-0028 hatte sie nur generisch „auf eine spätere Phase"
   verschoben, ohne Phase 6 beim Namen zu nennen — dieselbe Situation wie
   bei Gruppe 2, nicht wie bei den acht Workflow-Punkten. Inhaltlich
   passen sie ohnehin besser zu Phase 7: automatisches Erkennen guter
   Quellregionen und echtes inhaltsbasiertes Füllen sind PatchMatch-
   artige Bildanalyse-Algorithmen, dieselbe CV-Kategorie wie die in
   ADR-0031 nach Phase 7 verschobene Referenzbild-Optimierung.

**Konsequenzen:** `FEATURES.md` §3.3/§3.4-Zeilen werden auf „Phase 6"
bestätigt bzw. bei Tiefenbereich/KI-Masken auf „zurückgestellt"/„Phase 7"
korrigiert; die rund ein Dutzend Bibliotheks-Zeilen aus Gruppe 2 werden
von „Phase 6" auf „Phase 9" umgetaggt. `PLAN.md` bekommt einen neuen
Abschnitt „Aktuelle Phase: Phase 6" mit feingranularer Schrittfolge,
analog zu Phase 4/5. `ARCHITECTURE.md` §7s Phase-6-Platzhalterzeile wird
entsprechend präzisiert (Ebenenmodell statt Fused-Pass, EDL v3, kein
Tiefenbereich/keine KI-Masken).

## ADR-0033: Phase-7-Scope präzisiert — KI-Funktionen ohne echte ONNX-Runtime-Modellinferenz, echter LLM-Client für den Preset-Generator, neues `apx-ai`-Crate

**Status:** Angenommen
**Kontext:** `SPEC.md` §5 nennt für Phase 7 wörtlich „KI-Funktionen.
Motiv-/Himmel-/Personen-Segmentierung (ONNX-Runtime, Modelle lokal),
Preset-Generator per LLM, Referenzbild-Matching, Auto-Tagging."
`ARCHITECTURE.md` §7 reservierte dafür bereits wörtlich „`apx-ai` —
ONNX-Runtime-Integration ..., LLM-Client für Preset-Generator". Zwei
Teile dieses Satzes sind in dieser Umgebung nicht wie ursprünglich
vorgesehen umsetzbar:

1. **Echte ONNX-Runtime-Modellinferenz** würde (a) das Bundling
   tatsächlicher, lizenzrechtlich einwandfreier Segmentierungs-Modell-
   gewichte voraussetzen — es gibt in dieser Umgebung keinen legitimen
   Weg, ein trainiertes Motiv-/Himmel-/Personen-Segmentierungsmodell zu
   beschaffen und mitzuliefern —, und (b) eine native
   `onnxruntime`-Bibliothek zur Build-Zeit verlinken, was in dieser
   Sandbox nicht zuverlässig testbar ist (kein bestätigter Zugriffsweg
   auf vorkompilierte ONNX-Runtime-Binaries). Ein „Bring-your-own-Model"-
   Pfad (Nutzer liefert eine `.onnx`-Datei) wäre technisch denkbar, aber
   ohne jedes mitgelieferte oder in dieser Sandbox verifizierbare Modell
   nur eine ungetestete Hülle — dieselbe Art vorgetäuschter statt echter
   Fähigkeit, die dieses Projekt durchgehend vermeidet (siehe z. B.
   ADR-0032 Punkt 4: „eine genuine, nicht nur vorgetäuschte Fähigkeit").
2. **Ein echter LLM-Client** ist dagegen sehr wohl echt umsetzbar: ein
   HTTP-Client gegen die Anthropic-Messages-API (`reqwest`, vom Nutzer
   selbst hinterlegter API-Schlüssel, genau wie jede andere Desktop-App
   mit KI-Anbindung) ist eine Handvoll Code, braucht kein mitgeliefertes
   Modell und lässt sich isoliert testen (Prompt-Aufbau/Antwort-Parsing
   ohne echten Netzwerkaufruf, analog zum bestehenden „kein GPU-Adapter
   verfügbar"-Überspringen-Muster für Netzwerk-freie CI-Läufe).

**Entscheidung:**
1. **Neues `apx-ai`-Crate** (`crates/apx-ai`) bündelt alle in diesem
   Abschnitt beschriebenen Analyse-/Generator-Bausteine — dieselbe
   „ein Crate pro fachlicher Domäne"-Konvention wie `apx-catalog`/
   `apx-pipeline`. Abhängigkeiten: `apx-core`, `apx-raw` (für
   `LinearImage`), `apx-pipeline` (für EDL-Typen + Rendering, nur beim
   Referenzbild-Modus gebraucht), `apx-catalog` (für „Preset aus
   Bearbeitung lernen" + Auto-Tagging, beide lesen bestehende
   Katalogdaten), `reqwest` (LLM-Client, neue Workspace-Abhängigkeit,
   `rustls-tls`-Feature statt der systemabhängigen OpenSSL-Variante).
2. **Die fünf KI-Masken** (Motiv/Himmel/Hintergrund/Objekte/Personen)
   werden über **klassische, deterministische Bildverarbeitungs-
   heuristiken statt echter tiefer neuronaler Netze** umgesetzt — jede
   einzelne ist eine echte, funktionierende, unit-getestete Fähigkeit,
   kein Platzhalter:
   - **Motiv:** Sättigungs-/Kontrast-gewichtete Saliency-Karte
     (Center-Surround-Kontrast, ein reales, jahrzehntealtes klassisches
     Sichtverfahren) als Vordergrund-Wahrscheinlichkeit, geglättet.
   - **Himmel:** Farbton-/Helligkeits-/Positions-Heuristik (bläulich,
     geringer lokaler Kontrast, obere Bildhälfte bevorzugt) — dieselbe
     Art Heuristik, die vor Verbreitung tiefer Netze in echten
     Foto-Editoren für „Himmel auswählen" verwendet wurde.
   - **Hintergrund:** Komplement der Motiv-Maske (`1.0 - alpha`) — kein
     eigener Algorithmus nötig.
   - **Objekte (Klick-Segmentierung):** Region-Growing/Flood-Fill ab dem
     Klickpunkt, farbtoleranz-basiert — dieselbe Toleranz-/Weich-
     zeichnung-Grundidee wie die bereits bestehende Farbbereich-Maske
     (Phase 6 Schritt 5), hier aber ausgehend von einem Saatpunkt statt
     einem global über das ganze Bild verglichenen Zielfarbwert.
   - **Personen:** Hautton-Erkennung im YCbCr-Farbraum (ein reales,
     weit verbreitetes klassisches Verfahren) als **eine einzelne
     zusammenhängende Hautregion** — **bewusste Einschränkung:** die in
     `SPEC.md` §3.3 genannten Einzelregionen (Augen, Brauen, Lippen,
     Zähne, Haare, Kleidung) werden **nicht** als eigene, einzeln
     wählbare Teilmasken umgesetzt — echte Gesichts-/Körper-Landmark-
     Erkennung für diese Feinheit setzt ein trainiertes Modell voraus
     (siehe Punkt 1).
   Jede Heuristik läuft serverseitig (Rust, `apx-ai`), niemals
   clientseitig — konsistent mit dem gesamten Projekt: jede tatsächliche
   Pixelanalyse lebt in Rust, das Frontend liest nur schon gerenderte
   Einzelpixel (WB-Pipette/Farbmischer/Maskenfarbbereich).
3. **Maskengeometrie-Erweiterung:** ein KI-generiertes Ergebnis ist
   naturgemäß eine Rasterfläche, kein Parameter-Satz wie die fünf
   bestehenden Geometrietypen — `MaskGeometry` bekommt eine sechste
   Variante `AiGenerated { kind, width, height, alpha: Vec<u8> }`: eine
   niedrig aufgelöste (lange Kante auf 512px begrenzt) Alpha-Bitmap,
   beim Rendern bilinear auf die tatsächliche Zielauflösung hochskaliert
   — **bewusste Vereinfachung** ggü. den parametrischen Typen (die bei
   jeder Auflösung exakt neu berechnet werden): geringerer
   Speicherbedarf als eine volle Auflösung, aber weniger scharfe Kanten
   bei starker Vergrößerung. Einmal berechnet, bleibt sie bis zu einer
   erneuten Generierung unverändert (kein Re-Run bei jedem Regler-Tick).
4. **Reparatur-Erweiterungen** (aus ADR-0032 Punkt 8 bereits auf Phase 7
   vorgemerkt): Auto-Quellenfindung und Sensorflecken-Visualisierung
   sind einmalige Analyse-Befehle in `apx-ai` (Patch-Ähnlichkeitssuche
   per normierter Kreuzkorrelation bzw. Blob-Erkennung per lokaler
   Kontrast-Anomalie gegen ein weichgezeichnetes Referenzbild) — beide
   echte, deterministische, testbare Algorithmen. Inhaltsbasiertes
   Füllen (`RepairMode::ContentAwareFill`) ist dagegen ein *Render-Zeit*-
   Vorgang (läuft bei jedem Rendering wie Klonen/Reparieren, nicht nur
   einmalig) und bleibt deshalb in `apx-pipeline::stages::repair`, nicht
   in `apx-ai` — umgesetzt als vereinfachtes PatchMatch (Zufallsinit +
   Propagation + Zufallssuche, wenige Iterationen), eine reale, wenn auch
   gegenüber der vollen PatchMatch-Veröffentlichung reduzierte Fassung
   (weniger Iterationen, kein Multi-Skalen-Ansatz).
5. **Preset-Generator:** LLM-Anfrage nutzt einen echten Anthropic-
   Messages-API-Client (`apx-ai::llm_client`), der Nutzer hinterlegt
   seinen eigenen API-Schlüssel in den Einstellungen (`apx-core::Settings`
   bekommt ein neues `AiSettings`-Feld, exakt demselben Speicher-
   /Lademuster wie `UiSettings`/`CatalogSettings"). Der Prompt beschreibt
   dem Modell das EDL-Sektionsschema und bittet um eine JSON-Antwort im
   selben Format wie eine Preset-EDL-Teilmenge — Parsing/Validierung
   passiert serverseitig, eine unparsbare Antwort führt zu einem
   Fehler statt eines stillschweigend übernommenen Unsinns-Presets.
   Referenzbild-Modus (numerische Optimierung, **kein** LLM):
   Koordinatenabstieg über eine kleine, feste Teilmenge der
   Grundeinstellungs-Parameter, Zielfunktion = Histogramm-Distanz
   zwischen gerendertem aktuellen Bild und Referenzbild — bewusst kein
   Gradientenverfahren über die volle Pipeline (nicht differenzierbar
   ohne Autodiff-Infrastruktur), sondern ein einfaches, aber echtes
   Ableitungsfreies Suchverfahren. Variationen-Generator: deterministisch
   geseedete kleine Störungen eines Basis-Presets. Preset aus Bearbeitung
   lernen: Mittelwertbildung der committeten EDL-Werte mehrerer vom
   Nutzer ausgewählter Fotos je EDL-Sektion.
6. **Auto-Tagging** wird bewusst **regelbasiert statt echter
   Bildklassifikation** umgesetzt: leitet aus den KI-Masken-Heuristiken
   aus Punkt 2 (nennenswerte Himmel-/Hautton-Fläche erkannt) plus
   bereits vorhandenen EXIF-Feldern (ISO/Blende/Brennweite/Zeitstempel)
   eine kleine, feste Menge an Schlagwort-Vorschlägen ab (z. B. „Himmel",
   „Person", „Nachtaufnahme", „Makro") — reuse der bestehenden
   Schlagwort-Infrastruktur aus Phase 3 (`photo_keywords`), kein neues
   Schema. **Nachtrag:** „Auto-Tagging" stand im `SPEC.md` §5-Satz für
   Phase 7, hatte aber bislang **keine** eigene `FEATURES.md`-Zeile — eine
   echte Dokumentationslücke aus Phase 3, hier nachgetragen (§3.1, Phase
   7) statt stillschweigend übersprungen.

**Konsequenzen:** `FEATURES.md` §3.3 (fünf KI-Masken-Zeilen) und §3.5
(Preset-Generator-Zeilen) werden von „Nicht begonnen" auf „Fertig
(abweichend)" umgetaggt, sobald gebaut, mit Verweis auf diese ADR; §3.1
bekommt eine neue Auto-Tagging-Zeile. `ARCHITECTURE.md` §7s
Phase-7-Platzhalter wird durch ein neues Kapitel „Architektur Phase 7"
ersetzt. `PLAN.md` bekommt einen neuen Abschnitt „Aktuelle Phase: Phase 7"
mit feingranularer Schrittfolge. Tiefenbereich-Masken (siehe ADR-0032
Punkt 3) bleiben weiterhin ohne Phasenzuordnung zurückgestellt — kein
Tiefendaten-Zulieferer existiert, das ändert auch diese ADR nicht.

## ADR-0034: Phase-8-Scope präzisiert — Export-Engine als gemeinsamer Unterbau, reale Zusatzformate/ICC-Farbmanagement/PDF/SFTP wo machbar, HEIF-/JPEG-XL-Export und Kartenkacheln bewusst zurückgestellt

**Status:** Angenommen
**Kontext:** `SPEC.md` §5 nennt für Phase 8 wörtlich „Export und
Ausgabe-Module. Export-Engine, Warteschlange, Wasserzeichen, dann
Drucken, Diashow, Buch, Web, Karte." `ARCHITECTURE.md` §7 reservierte
dafür bislang nur einen Platzhalter. Mehrere frühere ADRs haben
zusätzlich einzelne `FEATURES.md`-Zeilen ausdrücklich auf Phase 8
vorgemerkt (ADR-0025: DNG-Konvertierung beim Import; ADR-0031 Punkt 5:
Export-/Wasserzeichen-/Metadaten-/Layout-/Workflow-Templates +
Template-Marktplatz-Struktur). Anders als Phase 6/7 ist Phase 8 damit
kein einzelnes fachliches Thema, sondern sechs weitgehend unabhängige
Ausgabe-Module (Export-Engine, Drucken, Diashow, Buch, Web, Karte), die
nur die Export-Engine als gemeinsamen Unterbau teilen. Diese ADR prüft
für jedes Modul, was in dieser Umgebung tatsächlich echt umsetzbar ist
(dieselbe Prüfungspflicht wie ADR-0033 für ONNX-Runtime), bevor
`PLAN.md` die feingranulare Schrittfolge bekommt.

**Vorab durchgeführte Machbarkeitsprüfung** (`cargo add --dry-run`
gegen den echten crates.io-Index, da direkte HTTP-Anfragen an die
crates.io-API in dieser Sandbox durch den Proxy blockiert werden):
`printpdf`, `lcms2`, `suppaftp`, `russh`, `ravif`, `webp`, `libheif-rs`,
`jxl-oxide`, `psd`, `dng`, `quick-xml`, `geoutils` und
`reverse_geocoder` lösen alle erfolgreich auf. Kein System-`ffmpeg`
in dieser Sandbox vorhanden (`command -v ffmpeg` liefert nichts) —
das sagt nichts über die Zielmaschine der Nutzer aus, ist aber
Anlass für die Video-Export-Entscheidung unten.

**Entscheidung** (in der von `SPEC.md` §5 vorgegebenen Reihenfolge):

1. **Export-Engine (gemeinsamer Unterbau).** Neues `apx-export`-Crate
   (dieselbe „ein Crate pro fachlicher Domäne"-Konvention wie
   `apx-catalog`/`apx-pipeline`/`apx-ai`), hängt von `apx-core`/
   `apx-raw`/`apx-pipeline`/`apx-catalog` ab. Rendert über den
   bestehenden `apx_pipeline::develop::render_rgba8`-Pfad (derselbe
   Renderer wie Vorschau/Viewer — kein zweiter Rendering-Codepfad) und
   kodiert dann in das Zielformat.
   - **Formate:** JPEG/PNG/TIFF sind über das bereits vorhandene
     `image`-Crate abgedeckt (bislang nur zum *Lesen* im
     Fallback-Importpfad genutzt, Schreib-Features werden hier neu
     aktiviert). **WebP und AVIF kommen echt dazu** (`ravif` für
     AVIF-Encoding ist reines Rust, kein Systemabhängigkeit). **PSD-,
     HEIF/HEIC- und JPEG-XL-Export werden bewusst zurückgestellt:**
     PSD hat keine ausgereifte Rust-Schreib-Bibliothek (die einzige
     gefundene, `psd`, ist ein Lese-Parser für Spieleentwicklungs-
     Pipelines, kein Encoder); HEIF/HEIC-Encoding braucht einen
     lizenzierten HEVC-Encoder (dieselbe Art Lizenz-/Beschaffungs-
     mauer wie ONNX-Modellgewichte in ADR-0033 Punkt 1 — `libheif-rs`
     bindet zwar an `libheif`, aber ohne einen frei redistribuierbaren
     HEVC-Encoder dahinter bliebe „HEIF-Export" eine Hülle ohne
     echten Encoder); JPEG XL hat aktuell keinen ausgereiften
     reinen-Rust-Encoder (`jxl-oxide`, bereits transitive
     Depenenz von `image` für JPEG-XL-*Decoding*, ist ein Decoder,
     kein Encoder — echtes Encoding bräuchte `libjxl`-Bindungen).
     `FEATURES.md`s Formatzeile wird bei Umsetzung entsprechend in
     „Fertig (abweichend)" mit den drei genannten Ausnahmen
     umgetaggt statt stillschweigend übersprungen.
   - **Farbräume/ICC:** **`lcms2` kommt zurück** (war seit Phase 1 in
     `Cargo.toml`, wurde aber nie verdrahtet und in Phase 2 Schritt 9
     wieder entfernt, siehe `THIRD_PARTY.md` — Farbtransformation kam
     bislang mit einer festen Matrix plus Gammakurve aus, Phase 6s
     Soft-Proof simuliert Zielprofile nur über einen
     Sättigungs-Kompressionsfaktor statt echter Profile). Diesmal mit
     echtem Verdrahtungsgrund: eine *exportierte Datei* muss ihr
     Farbprofil korrekt einbetten (Druckdienstleister/Web-Betrachter
     werten es aus), eine reine Bildschirmvorschau (Soft-Proof) verzeiht
     Ungenauigkeit stärker als eine tatsächlich weitergereichte Datei.
     `lcms2`s `static`-Feature baut Little-CMS aus dem Quellcode statt
     eine System-Bibliothek vorauszusetzen (dieselbe Bundled-statt-
     System-Konvention wie `rusqlite`s `bundled`-Feature). Bundle die
     vier in `SPEC.md` genannten Standardprofile (sRGB/Adobe
     RGB/ProPhoto RGB/Display P3, feste Primärvalenzen + Tonkurve,
     dieselben Daten, die Phase 6s Soft-Proof bereits für seine
     Simulation nutzt) plus einen Dateiauswahl-Dialog für „eigenes
     ICC" (`lcms2::Profile::new_file`). Phase 6s Soft-Proof bleibt
     unverändert simuliert (reine Vorschau, kein Umbau nötig) —
     nur der tatsächliche Datei-Export bekommt echtes Farbmanagement.
   - **Wasserzeichen:** Bild-/Text-Overlay direkt auf dem gerenderten
     RGBA8-Puffer vor der Kodierung — Text-Rendering über eine
     Font-Rasterisierungs-Bibliothek (z. B. `ab_glyph`), kein neues
     Architekturrisiko.
   - **Export-Warteschlange:** dieselbe Fortschritts-/Abbruch-Architektur
     wie der bestehende Import-Job (Phase 1: `tokio`-Task +
     Tauri-Events fürs Fortschritts-Streaming, `CancellationToken` für
     Pausieren/Abbrechen) — keine neue Konstruktion, reuse.
   - **Import mit DNG-Konvertierung:** die `dng`-Bibliothek existiert
     und löst auf; ihr tatsächlicher Funktionsumfang (reiner Reader
     oder auch Schreibpfad) wird erst bei der Umsetzung geprüft — falls
     sie sich als lese-only herausstellt, wird diese eine Zeile
     zurückgestellt statt das ganze Schritt zu blockieren.
   - **Bit-Tiefe 8/16, Größenbegrenzung, Ausgabeschärfung nach Medium:**
     reine Parametrisierung des bestehenden Renderers, kein neues
     Architekturrisiko.

2. **Drucken.** Reine Layout-/Rastergeometrie (Einzelbild/Kontaktbogen/
   Bilderpaket/benutzerdefiniertes Raster) über der Export-Engine aus
   Punkt 1 — „Speichern als JPEG" ist derselbe Exportpfad mit einem
   Druck-Layout als Eingabe statt eines Einzelbilds. Kein Rust-System-
   Druckertreiber-Zugriff (CUPS/Windows-Druckdialog) in dieser
   Phase — Ausgabe ist eine druckfertige Bilddatei, kein direkter
   Druckauftrag; System-Druckdialog-Integration bliebe, falls je
   gewünscht, eine spätere Erweiterung.

3. **Diashow.** Übergänge/Ken-Burns/Intro-Outro sind reine Frontend-
   Canvas-Wiedergabe (dieselbe Art clientseitige Animation wie
   Phase 6s Vorher/Nachher-Ansicht), kein neues Rust-Crate.
   Musik-Synchronisation braucht kein Rust-Audio-Crate — das
   Tauri-Webview-`<audio>`-Element liefert `duration`/Zeitstempel-
   Events bereits im Browser. **Video-Export (MP4) ruft ein
   System-`ffmpeg` auf, statt eines mitgelieferten Binaries oder
   eines reinen-Rust-Encoders** (für H.264/MP4 gibt es keinen
   praxistauglichen reinen-Rust-Encoder; `ffmpeg` selbst mitliefern
   hieße, pro Zielplattform ein Binary mit eigener Lizenzprüfung
   auszuliefern, siehe `THIRD_PARTY.md`s GPL-Ausnahme-Regel). Beim
   Start wird `ffmpeg -version` per `std::process::Command` geprüft
   — vorhanden: echter Video-Export über einen echten Encoder; fehlt
   es: eine klare, umsetzbare Fehlermeldung („ffmpeg installieren"),
   keine stillschweigend leere Funktion. Dieselbe Holen-und-notfalls-
   ehrlich-scheitern-Vorgehensweise wie diese Umgebung selbst für
   `ANTHROPIC_API_KEY`/Cargo-Registry-Zugriff verwendet.

4. **Buch.** Seitenlayouts/Vorlagen/Text-Stile als datengetriebene
   Layout-Engine (Raster + konfigurierbare Slots), derselbe deklarative
   Aufbau wie das Masken-/Preset-System. **PDF-Export ist echt
   umsetzbar** über `printpdf` (reines Rust, MIT-Lizenz, keine
   System-Bibliothek). Druckerei-Presets sind reine Parametersätze
   (Beschnitt/Farbprofil/Auflösung je Anbieter), keine neue Fähigkeit.

5. **Web.** HTML-/responsiver Galerie-Generator ist serverseitiges
   Rust-Templating (Miniaturbilder über den bestehenden
   `image`-Verkleinerungspfad aus dem Import-Thumbnail-Schritt, Phase 1).
   **FTP/SFTP-Upload ist echt umsetzbar:** `suppaftp` (FTP/FTPS) und
   `russh` + `russh-sftp` (reines Rust SSH/SFTP, keine
   OpenSSL-/libssh2-Systemabhängigkeit, dieselbe Bevorzugung reiner
   Rust-Implementierungen wie `rustls` an anderer Stelle im Projekt).

6. **Karte.** GPS-Auslesen aus EXIF ist eine Erweiterung des
   bestehenden Metadaten-Pfads (`kamadak-exif`/`rawler` lesen GPS-Tags
   bereits mit, bislang nur ungenutzt durchgereicht). **Reverse
   Geocoding bleibt vollständig offline**: `reverse_geocoder` bündelt
   einen GeoNames-Städte-Datensatz und braucht keinen Online-Dienst —
   dieselbe Offline-zuerst-Haltung wie der Rest der App. **Bewusste
   Einschränkung, einzige Ausnahme von „offline zuerst" in dieser
   Phase:** die eigentliche Kartenansicht (Kachel-Bilder) ist ein
   Frontend-Feature (z. B. Leaflet.js gegen OpenStreetMap-Kacheln) und
   braucht zum *Anzeigen* der Hintergrundkarte eine Internetverbindung
   — ein weltweiter Offline-Kachelsatz wäre unverhältnismäßig groß;
   ohne Netzwerk bleiben die GPS-Koordinaten/Ortsschlagworte trotzdem
   nutzbar, nur ohne Kartenbild. GPX-Tracklog-Import ist einfaches
   XML-Parsing (`quick-xml`). Reiserouten-Ansicht leitet sich aus
   GPS-getaggten Fotos nach Aufnahmezeit sortiert ab, reine
   Frontend-Darstellung ohne neues Backend-Risiko.

**Konsequenzen:** `PLAN.md` bekommt einen neuen Abschnitt „Aktuelle
Phase: Phase 8" mit feingranularer Schrittfolge in der oben
begründeten Reihenfolge (Export-Engine zuerst als Unterbau, danach
Drucken/Diashow/Buch/Web/Karte, exakt wie `SPEC.md` §5 sie nennt).
`FEATURES.md`s Phase-8-Zeilen bleiben bis zur jeweiligen Umsetzung
„Nicht begonnen" — keine Korrektur nötig, sie waren bereits vollständig
und korrekt getaggt (inkl. der Export-Warteschlange-Zeile). PSD-/
HEIF-/JPEG-XL-Export und echte System-Druckdialog-Integration bleiben
ohne Phasenzuordnung zurückgestellt, bis eine lizenzrechtlich/technisch
tragfähige Bibliothek verfügbar ist.

## ADR-0035: Phase-9-Scope präzisiert — drei angesammelte Rückstände zusammengeführt, elf „bewusste Vereinfachung"-Grenzen für die technisch schwierigsten Punkte festgelegt, Tiefenbereich-Masken bleiben ausgenommen

**Status:** Angenommen
**Kontext:** `SPEC.md` §5 nennt für Phase 9 wörtlich „Fortgeschrittenes:
Node-Editor, Panorama/HDR/Fokus-Stacking, Tethering, Skript-API,
Plugin-System" (ergänzt um §3.6 „Zusätzliche Module": Astro-Stacking,
Stapelverarbeitungs-Konsole, Vergleichs-Grid, Zeitleisten-Ansicht,
Verlaufs-Vergleich, Kollaborationsmodus — zwölf Punkte in Summe). Über
die Phasen 3–8 hinweg haben mehrere frühere ADRs zusätzlich einzelne
`FEATURES.md`-Zeilen ausdrücklich auf Phase 9 vorgemerkt, statt sie in
der jeweils laufenden Phase mitzubauen:

- **ADR-0032** (Phase-6-Scope) verschob das komplette
  „Bibliotheks-Backlog" aus `SPEC.md` §3.1 nach Phase 9 — vierzehn
  Punkte (Perceptual-Hash-Duplikaterkennung/-Assistent,
  Sammlungssätze/intelligente Sammlungen, Stapel, virtuelle Kopien,
  erweiterbare Farbmarkierungen, Schlagworthierarchie, Metadaten-
  Presets/EXIF-IPTC-XMP-Editor/Sidecar-Export, Vergleichs-/
  Übersichtsansicht, Personenansicht, Filter-Presets, Schnellentwicklung
  im Raster, Vorschau-Cache/Smart Previews, Sekundäres Display,
  Katalog-Statistik-Dashboard) — mit der ausdrücklichen Begründung,
  dass keine ADR diese Punkte je Phase 6 zugesagt hatte und ihre
  Mitnahme Phase 6 auf etwa das Dreifache von `SPEC.md` §5s tatsächlicher
  Phase-6-Zeile aufgebläht hätte.
- **ADR-0031 Punkt 3/5** verschob Adobe-`.xmp`/`.lrtemplate`-Interop
  sowie Export-/Wasserzeichen-/Metadaten-/Layout-/Workflow-Templates
  nach Phase 8–9 (Letztere sind mit Phase 8 Schritt 8 bereits erledigt,
  Erstere bleibt offen — eine Einzelzeile).
- **Ein expliziter Nutzerwunsch** während Phase 8 (Vergleich mit einem
  Lightroom-Classic-Screenshot) ergänzte elf weitere UI-nahe
  Entwickeln-/Anzeige-Fähigkeiten, die weder in `SPEC.md` noch bislang
  in `FEATURES.md` vorkamen (Live-Histogramm, Clipping-Warnungen,
  Punktfarbmesser, Zielgerichtetes Anpassungswerkzeug, Schwarzweiß-
  Mixer, Auto-Ton/Auto-Weißabgleich, Navigator-Miniaturansicht,
  KI-Entrauschung, KI-Hochskalierung, Info-Overlay, Bearbeitungs-Pins)
  — bereits als eigener Backlog-Abschnitt in `PLAN.md`/`FEATURES.md`
  §3.2 vorgemerkt, aber ausdrücklich noch ohne eigene Scope-Präzisierung
  (siehe dortiger Hinweis: „Phase 9 ist noch nicht die aktuelle Phase").

Macht zusammen **38 Einzelpunkte** — mehr als Phase 6+7+8 zusammen. Der
Nutzer hat ausdrücklich gebeten, alle 38 jetzt vollständig zu planen und
umzusetzen, statt weiter aufzuschieben. Diese ADR übernimmt für die
technisch schwierigsten Punkte dieselbe Prüfungspflicht wie ADR-0033
(ONNX-Runtime) und ADR-0034 (Formats-/Bibliotheks-Machbarkeit), bevor
`PLAN.md` die feingranulare Schrittfolge bekommt.

**Vorab durchgeführte Machbarkeitsprüfung** (`cargo add --dry-run`
gegen den echten crates.io-Index bzw. `npm view` gegen den echten
npm-Registry-Index, dieselbe Methode wie in ADR-0034): `rhai` (reines
Rust, sandboxed, aktiv gepflegt), `libloading`, `rustfft`, `imageproc`
und `gphoto2` (LGPL-2.1, bindet an System-`libgphoto2`) lösen alle
erfolgreich auf. `@xyflow/react` (React Flow, aktueller Paketname)
löst im npm-Registry auf. `opencv`-Bindings lösen zwar in Cargo auf,
brauchen aber eine System-OpenCV-Installation, die in dieser Sandbox
nicht vorhanden ist — dasselbe Problemmuster wie ONNX Runtime in
ADR-0033. `libgphoto2` (für Tethering) ist in dieser Sandbox ebenfalls
nicht installiert, und anders als bei `ffmpeg` (ADR-0034 Punkt 3, wo
zumindest ein „ist installiert oder nicht"-Laufzeitcheck möglich ist)
fehlt hier sogar die Systembibliothek zum *Kompilieren* mit aktiviertem
Feature.

**Entscheidung:** Alle 38 Punkte bleiben Phase-9-Scope, aufgeteilt in
zwölf Bauschritte (`PLAN.md` Schritt 1–12) plus diesen Scope-Schritt
(Schritt 0) — Reihenfolge unten richtet sich danach, was aufeinander
aufbaut (Bibliothek vor Stacking, das dieselbe `stacks`/`virtual_copies`-
Schema braucht) und was das geringste Risiko trägt (reine
Analyse-/Anzeige-Erweiterungen zuerst, Architekturrisiko-Punkte
zuletzt). Für die technisch schwierigsten Punkte gilt je eine bewusste
Vereinfachung, die hier bindend festgehalten wird (Details/Begründung
je Punkt in `PLAN.md`s Schritt-Text):

1. **Node-Editor** zeigt und schaltet die bestehende, fest verdrahtete
   Rendering-Reihenfolge (`apx_pipeline::develop::render_rgba8`) —
   keine frei umbaubare oder verzweigende Ausführungsgraph-Engine, weil
   das die Renderpfad-Garantie brechen würde, auf die jedes andere
   Modul (Viewer, Export, Masken) angewiesen ist. Technisch: neues
   `EdlV4`-Schema mit einem `enabled: bool`-Feld pro Pipeline-Stufe,
   `v3_to_v4`-Migration, `@xyflow/react` im Frontend.
2. **Panorama-/Astro-Stacking** beschränken sich in v1 auf reine
   Verschiebungs-Registrierung per 2D-Phasenkorrelation (`rustfft`,
   Stativ-/gleicher-Blickpunkt-Annahme) — echtes merkmalsbasiertes
   Homographie-Stitching für Freihand-/gedrehte Aufnahmen bleibt
   zurückgestellt, weil `opencv` eine fehlende Systembibliothek
   voraussetzt und keine verifizierte, ausgereifte reine-Rust-
   Merkmalserkennungs-plus-RANSAC-Pipeline existiert. Fokus-Stacking
   (Laplacian-Schärfe-Auswahl über bereits ausgerichtete Frames) und
   HDR-Merge (Debevec-Gewichtung + Reinhard-Tonemap) brauchen dagegen
   keine Ausrichtungs-Bibliothek und werden vollständig gebaut.
3. **Skript-API + Plugin-System:** „stabile ABI" heißt eine
   handgepflegte, versionierte `#[repr(C)]`-Funktionszeiger-Tabelle für
   genau einen festen Erweiterungspunkt (eine Bildoperationsstufe) plus
   eine schmale, primitiv-typisierte `rhai`-Skript-API — **keine**
   Zusage, dass beliebige künftige interne Rust-Strukturen dauerhaft
   binärkompatibel bleiben (Rust selbst hat keine stabile ABI). Eine
   Versions-Fehlpassung wird beim Laden hart mit klarer Fehlermeldung
   abgelehnt statt still falsch zu kompilieren.
4. **Kollaborationsmodus** bleibt ein asynchroner Export→Weitergabe→
   Import→Konfliktauflösung-Ablauf über eine neue `.apxt`-artige
   `ApxShareFile` (Metadaten/EDL/Presets, keine Pixel-Bytes) — kein
   Echtzeit-Mehrbenutzer-Modus (kein Live-Cursor/keine Präsenz/kein
   CRDT/keine Netzwerksynchronisation), weil sich Letzteres in dieser
   Sandbox ohne zweiten echten Nutzer und ohne Mehrbenutzer-
   Serverinfrastruktur nicht verifizieren lässt. `apx-catalog` bleibt
   dabei unverändert ein einzelner `Mutex<Connection>` (ADR-0008).
5. **Tethered Shooting:** die `gphoto2`-Anbindung (LGPL-2.1, braucht
   eine `THIRD_PARTY.md`-Ausnahme wie bereits bei `rawler`) wird hinter
   einem standardmäßig ausgeschalteten Cargo-Feature `tethering` real
   geschrieben (`TetherBackend`-Trait, echtes `Gphoto2Backend` plus ein
   `FakeBackend` für alle normalen Tests) — **über ADR-0034s
   ffmpeg-Präzedenzfall hinaus bewusst eingeschränkt:** die echten
   libgphoto2-FFI-Aufrufe sind nie gegen eine echte Kamera oder auch
   nur eine installierte `libgphoto2`-Bibliothek ausführbar, weil
   Letztere in dieser Sandbox und im Standard-CI schlicht fehlt — nicht
   einmal ein „unerreichbarer Server"-Fehlerpfad wie bei FTP/SFTP
   (ADR-0034 Punkt 5) ist hier testbar. `FEATURES.md` muss diese Lücke
   explizit benennen, nicht verschweigen.
6. **KI-Entrauschung/KI-Hochskalierung** verwenden dieselbe
   Ehrlichkeitslinie wie ADR-0033: klassische, deterministische
   Algorithmen (kantenerhaltender Bilateral-Filter statt echter
   neuronaler Entrauschung, kantengerichtete Interpolation statt echter
   Modell-Superresolution) statt einer vorgetäuschten Modellinferenz —
   das ungelöste ONNX-Beschaffungsproblem aus ADR-0033 besteht
   unverändert fort. Die UI-Beschriftung darf an diesen zwei Stellen
   nicht „KI"/„AI" implizieren, wo keine Modellinferenz läuft.
7. **Personenansicht (Gesichtserkennung)** nutzt dieselbe Art
   klassischer Heuristik wie die Phase-7-KI-Masken (Hautton-/
   Kontur-Erkennung im YCbCr-Raum, grobe Ähnlichkeitsgruppierung) statt
   echter Gesichts-Embeddings — dieselbe ONNX-Beschaffungslage.
8. **Adobe-`.xmp`/`.lrtemplate`-Interop** bekommt ein eigenes,
   dokumentiertes Mapping `EdlV3` ↔ Adobe-Parameternamen; welche Felder
   verlustfrei und welche nur best-effort übersetzbar sind (Adobes
   proprietäre Kurven-/Masken-Repräsentation ist nicht vollständig
   offen dokumentiert), wird bei Umsetzung explizit in `FEATURES.md`
   festgehalten, nicht stillschweigend als „Fertig" verbucht.

**Explizit ausgenommen:** Maskentyp Tiefenbereich (`FEATURES.md`,
weiterhin „Später zurückgestellt", siehe ADR-0032 Punkt 3) — kein
Tiefendaten-Zulieferer existiert irgendwo im Projekt, jede Umsetzung
wäre erfunden statt real. Bleibt ohne Phasenzuordnung, bis eine
Tiefendatenquelle auftaucht.

**Konsequenzen:** `PLAN.md` bekommt einen neuen Abschnitt „Aktuelle
Phase: Phase 9" mit zwölf Bauschritten in der oben begründeten
Reihenfolge (Bibliothek zuerst als Fundament für Stacking, dann
Entwickeln-Erweiterungen nach Risiko sortiert, dann die sechs
SPEC-§5/§3.6-„Fortgeschrittenes"-Themen). `FEATURES.md`s Phase-9-Zeilen
bleiben bis zur jeweiligen Umsetzung „Nicht begonnen" — keine Korrektur
nötig, beide Recherchen vor dieser ADR bestätigen, dass sie bereits
vollständig und korrekt getaggt sind. Jeder Bauschritt wird einzeln
committet und gepusht, mit schlanken statt erschöpfenden Tests je
Schritt (Projektkonvention seit Phase 8) — keine wiederholten vollen
Testsuiten-Läufe zwischen den Schritten.

## ADR-0036: Phase-9-Abnahme (Schritt 12) — Stapelverarbeitungs-Konsole war in ADR-0035s Scope-Aufzählung, bekam aber nie einen Bauschritt; bleibt explizit zurückgestellt statt nachträglich überstürzt angeflanscht

**Status:** Angenommen
**Kontext:** Beim Abnahme-Durchgang (`PLAN.md` Schritt 12) fiel auf: Die
„Stapelverarbeitungs-Konsole" (`SPEC.md` §3.6: „Regeln auf Tausende
Bilder anwenden, mit Vorschau der betroffenen Menge, Trockenlauf,
Rückgängig-Machen der gesamten Aktion") steht in ADR-0035s Aufzählung
der sechs SPEC-§3.6-„Zusätzliche Module" (zusammen mit Astro-Stacking,
Vergleichs-Grid, Zeitleisten-Ansicht, Verlaufs-Vergleich,
Kollaborationsmodus) — bekam aber, anders als die anderen fünf, **nie**
einen eigenen Bauschritt in `PLAN.md`s Schritt-für-Schritt-Plan (Schritt
1–11). Eine Lücke im Schritt-0-Zuschnitt selbst, kein Ausrutscher bei
der Umsetzung eines vorhandenen Schritts — dieselbe Kategorie Fehler wie
die in Schritt 11 nachgetragenen `THIRD_PARTY.md`-Zeilen, nur auf der
Planungs- statt der Dokumentationsebene.

**Abwägung:** Der Nutzer hat für diese Runde ausdrücklich verlangt, die
Phase vollständig abzuschließen, nicht abzubrechen. Trotzdem wird diese
Lücke hier **nicht** durch eine nachträglich in Schritt 12
hineingequetschte Implementierung geschlossen: Ein ehrliches
„Rückgängig-Machen der gesamten Aktion" für eine Stapelverarbeitungs-
Konsole bräuchte einen neuen, feldübergreifenden Batch-Operationen-
Log (nicht nur `edit_history` je Foto, sondern eine Journalstruktur über
beliebige Katalogmutationen — Bewertung, Schlagworte, Metadaten,
Löschen — mit Gruppierung „gehört zu Aktion X" und einem echten
Undo-Pfad dafür). Das ist derselbe Umfang wie ein eigener Bauschritt,
kein Dokumentations-Anhängsel — eine überstürzte Version ohne echtes
Batch-Undo (z. B. nur eine Vorschau-Anzeige ohne Rückgängig-Funktion)
würde eine Fähigkeit vortäuschen, die nicht vollständig da ist, exakt
das, was `PLAN.md`/`DECISIONS.md` an anderer Stelle konsequent vermeidet
(siehe z. B. ADR-0033, ADR-0035 Punkt 6).

**Entscheidung:** Stapelverarbeitungs-Konsole bleibt in `FEATURES.md`
explizit „Zurückgestellt" (nicht „Nicht begonnen" — das würde die
fehlende Schrittzuordnung verschleiern), mit Verweis auf diese ADR.
Kein neuer Bauschritt in dieser Phase; bei Bedarf eigener Schritt in
einer späteren Phase, sobald ein Batch-Operationen-Log tatsächlich
gebraucht wird (z. B. wenn ein weiteres Feature eine Undo-fähige
Gruppierung mehrerer Katalogmutationen voraussetzt).

**Konsequenzen:** Alle anderen 37 der 38 in ADR-0035 aufgezählten Punkte
sind mit Schritt 11 vollständig umgesetzt (bzw. mit dokumentierter
bewusster Vereinfachung, siehe ADR-0035 Punkte 1–8) — Phase 9 gilt mit
dieser einen, hier explizit benannten und begründeten Ausnahme als
abgeschlossen.

## ADR-0037: Phase-10-Scope — drei Phase-3/5-UI-Restposten reingezogen, Testdisziplin für diese Phase nutzerangeordnet gelockert, ehrliche Grenzen bei Installer-Signierung

**Status:** Angenommen
**Kontext:** Der Nutzer hat Phase 10 („Politur", `SPEC.md` §5: Performance-
Profiling gegen die Ziele aus §2.4, Barrierefreiheit, Lokalisierung DE/EN,
Onboarding, Installer und Signierung für alle drei Plattformen) angewiesen,
mit zwei ausdrücklichen Vorgaben: (1) erst ein vollständiger Plan, dann alle
Schritte ohne Zwischenstopp — dieselbe Disziplin wie Phase 9; (2) Fokus vor
allem auf die UI, nicht auf weitere Tests, mit nur einer vollen Testsuite
am Ende statt der sonst pro Schritt üblichen.

**Entscheidung 1 — Scope-Erweiterung um drei UI-Restposten:** Drei
`FEATURES.md`-Zeilen standen noch auf Phase 3/5, wurden dort aber nie
angefasst: rechte Werkzeug-Palette/Modul-Umschalter oben (Phase 3),
ein-/ausklappbare/breitenziehbare Paletten mit Arbeitsbereich-Preset
(Phase 3), vollständige Befehlspalette (Phase 5, bisher nur
Ordner+ein Befehl). Alle drei sind reine UI-Arbeit und passen exakt zum
Auftrag „vor allem UI" — sie werden analog zu ADR-0032 (das
Bibliotheks-Backlog von Phase 6 nach Phase 9 verschob) in Phase 10 gezogen,
statt sie ein drittes Mal auf eine noch spätere Phase zu vertagen.

**Entscheidung 2 — Testdisziplin gelockert, nur für diese Phase:**
`PLAN.md` §6 verlangt normalerweise „Jedes Rust-Modul mit Unit-Tests …
E2E-Test pro Modul" pro Schritt. Auf ausdrücklichen Nutzerwunsch schreibt
Phase 10 **pro Schritt keine neue Testdatei** — nur `cargo build`/`tsc -b`
als Kompilier-Kontrolle. Die volle Suite (`cargo fmt/clippy/test`,
`tsc -b`, `vitest run`, volle Playwright-Suite) läuft einmalig am Ende
(Schritt 12) gegen den gesamten in dieser Phase entstandenen Stand. Das ist
eine bewusste, befristete Ausnahme von der sonst verbindlichen Regel — sie
gilt nicht rückwirkend für Phase 1–9 und nicht automatisch für Phase 11+,
falls es eine gibt.

**Entscheidung 3 — Installer/Signierung ehrlich begrenzt:** Diese
Linux-Sandbox besitzt keine echten Code-Signing-Zertifikate (Apple
Developer ID + Notarisierungs-Zugangsdaten, Windows-Codesigning-Zertifikat)
und kann sie nicht beschaffen — dieselbe Beschaffungslage wie
`libgphoto2` (ADR-0035 Punkt 5) oder die fehlende ONNX-Runtime
(ADR-0033). Schritt 11 baut die strukturelle Infrastruktur (Tauri-
Bundler-Konfiguration, `@tauri-apps/cli`, ein neuer CI-`release`-Job auf
dem bestehenden 3-OS-Matrix, Signierungsfelder aus optionalen
GitHub-Secrets gespeist, übersprungen statt fehlschlagend, wenn sie
fehlen) — produziert aber in dieser Umgebung **unsignierte** Installer.
Echtes Signieren bleibt dem Nutzer vorbehalten, sobald eigene
Zertifikate/Secrets hinterlegt sind. Cross-Plattform-Bundling
(macOS-/Windows-Installer von Linux aus) wird nicht lokal versucht — die
Verifikation läuft ausschließlich über die drei echten nativen CI-Runner.

**Konsequenzen:** `FEATURES.md`s Phase-10-Zeilen wachsen um die drei
umgetaggten Punkte. Performance-Profiling (Schritt 10) liefert eine
dokumentierte Einschätzung gegen die in dieser Sandbox tatsächlich
messbaren Teilziele statt spekulativer Umbauten ohne Befund — dieselbe
Grenze wie die fehlenden Golden-Image-RAW-Tests (ADR-0007). Lokalisierung
(Schritt 8) deckt systematisch alle Frontend-Komponenten ab, aber ohne
einen dafür geschriebenen Coverage-Test ist einzelne übersehene Strings
nicht mit Sicherheit ausgeschlossen — als offener, dokumentierter Rest
statt stillschweigend behauptet.

## ADR-0038: Phase 11 — Nachträge zu allen zurückgestellten Punkten aus Phase 1–10; vier neu verfügbare Crates real geprüft (drei brauchbar, eine als Fassade entlarvt), `libgphoto2` ist entgegen ADR-0035 tatsächlich per `apt` verfügbar

**Status:** Angenommen
**Kontext:** Der Nutzer hat angewiesen, alle bisher zurückgestellten Punkte
aus Phase 1–10 aufzuarbeiten — mit der ausdrücklichen Vorgabe, für jeden
Punkt neu zu prüfen, ob er inzwischen technisch machbar ist, statt ihn
unangetastet zu lassen (dieselbe Disziplin, mit der dieses Projekt schon
mehrfach Crate-Landschaften neu geprüft statt Annahmen fortgeschrieben
hat). `FEATURES.md` listete zu Beginn dieser Runde neun offene
Checkbox-Zeilen; dazu kommen vier in der Phase-10-Abnahme selbst benannte
Lücken.

**Dokumentationsfehler gefunden und korrigiert:** `FEATURES.md` Zeile 239
führte „Adobe `.xmp`/`.lrtemplate`-Import/Export" noch als „Nicht begonnen",
obwohl `apx_export::xmp` (`crates/apx-export/src/xmp.rs`, gebaut in Phase 9
Schritt 2) den `.xmp`-Teil (Adobe `crs:`-Entwickeln-Einstellungen: Basic+HSL,
echt bidirektional per `generate_xmp`/`parse_xmp_develop_settings`, über
`import_xmp_develop_settings`/`import_xmp_sidecar_from_file` bis ins
Frontend verdrahtet) bereits vollständig abdeckt. Nur der separate
`.lrtemplate`-Teil (Lightrooms eigenes, nicht offiziell dokumentiertes
Vorlagenformat) war tatsächlich noch offen — die Zeile wurde entsprechend
präzisiert statt pauschal als „Nicht begonnen" stehen zu lassen.

**Crate-Spikes** (`cargo add --dry-run` + realer Testbau in einem
Wegwerf-Projekt, kein Produktivcode betroffen):
1. **`gamut-dng`** (1.0.0, MIT/Apache-2.0) — kompiliert sauber, reines Rust,
   `DngEncoder`/`DngDecoder` mit vollständiger Builder-API. Zum Zeitpunkt
   von ADR-0034 (Phase 8) gab es keine schreibfähige DNG-Crate; diese
   existiert jetzt. **Brauchbar** — siehe Schritt 1.
2. **`gamut-jxl`** (0.4.0, MIT/Apache-2.0, Encode-Feature) — kompiliert,
   aber nicht reines Rust: `gamut-jxl-sys` vendort und baut die
   Referenz-C-Bibliothek `libjxl` (Lizenz BSD-3-Clause) selbst über
   `cmake`/`cc` zur Bauzeit (kein System-`apt`-Paket nötig, aber ein
   spürbar schwererer Build, ca. 1,5 Minuten allein für diese eine
   Abhängigkeit in der Spike-Messung). **Brauchbar, aber teurer** — siehe
   Schritt 2.
3. **`ag-psd`** (0.2.0, MIT) — kompiliert sauber, reines Rust, „from-scratch
   Rust port" der TypeScript-`ag-psd`-Bibliothek, `write_psd(&Psd,
   &WriteOptions) -> Vec<u8>` als klare Top-Level-API. **Brauchbar** —
   siehe Schritt 2.
4. **`heif`** (0.1.0) — laut Beschreibung „A HEIF file decoder and encoder
   written from scratch", tatsächlich aber ein reserviertes Fassaden-Paket:
   `src/lib.rs` ist 14 Zeilen lang und enthält ausschließlich `cargo new`s
   Standard-Vorlage (`pub fn add(left, right) -> u64`). **Nicht brauchbar**
   — bestätigt, warum dieses Projekt Beschreibungen nie ungeprüft
   übernimmt. `heif-rs` (26.7.0, „statically-linked libheif") wurde
   testweise ebenfalls hinzugefügt: zieht über 190 Pakete nach, darunter
   `bindgen` (braucht `libclang`), einen kompletten `image`-Crate-Formats-
   Rattenschwanz (`gif`/`exr`/`zip`/…) und `ureq` (HTTP-Client — unklar,
   wofür ein Bild-Codec Netzwerkzugriff bräuchte) — derselbe Beschaffungs-
   Risikograd wie ONNX Runtime/`opencv` an anderer Stelle im Projekt, bei
   nur 6 GB freiem Speicher in dieser Sandbox nicht vertretbar zu bauen.
   **HEIF-Export bleibt zurückgestellt**, wie schon in ADR-0034.
5. **`libgphoto2-dev`** ist entgegen der in ADR-0035 getroffenen Annahme
   („auf keinem der drei CI-Runner installiert") tatsächlich per `apt-get`
   installierbar (`apt-cache policy` zeigt Version 2.5.31-2.1ubuntu1.1
   verfügbar) — die ADR-0035-Aussage war für diese Sandbox richtig
   *behauptet*, aber nie tatsächlich mit `apt-cache policy` verifiziert
   worden. Siehe Schritt 10: das `tethering`-Feature kann jetzt real
   kompiliert und sein Verbindungs-Fehlerpfad ohne physische Kamera echt
   getestet werden.

**Entscheidung:** Phase 11 bündelt alle neun `FEATURES.md`-Zeilen plus die
vier Phase-10-Lücken plus die eine gefundene Dokumentationskorrektur in
13 Bauschritten (0–12), mit der normalen, vollen Testdisziplin aus
`PLAN.md` §6 (die Lockerung aus ADR-0037 galt ausdrücklich nur für
Phase 10). Für PSD/JPEG-XL/DNG werden die oben real geprüften Crates
verwendet; für HEIF, echte Zertifikats-Signierung, echte GPU-Performance
und echte neuronale Modellinferenz bleiben die bereits in ADR-0033/-0034/
diesem Plan dokumentierten Grenzen unverändert bestehen — kein
Software-Trick ersetzt fehlende Hardware, ein fehlendes Zertifikat oder
ein nicht beschaffbares Modell.

**Nachtrag nach Schritt 4 — Testdisziplin nutzerseitig erneut gelockert:**
der Nutzer hat nach Abschluss von Schritt 3 angewiesen, ab Schritt 4 nur
noch maximal einen Test pro Schritt zu schreiben und die vollständige
Verifikationskette (`cargo fmt/clippy/test --workspace`, `tsc -b`, volle
Vitest-/Playwright-Suite) nicht mehr nach jedem einzelnen Schritt,
sondern erst einmalig am Ende in Schritt 12 laufen zu lassen — dieselbe
Lockerung wie in ADR-0037 für Phase 10, nur diesmal ausdrücklich auch für
Phase 11 (Schritte 4–11) angeordnet, statt wie oben ursprünglich
festgehalten strikt auf Phase 10 begrenzt. Schritte 0–3 liefen noch mit
der vollen, oben beschriebenen Disziplin; ab Schritt 4 gilt: pro Schritt
ein gezielter Test (Rust-Unit- oder Playwright-Test, je nachdem, welcher
den Kern der Änderung am direktesten abdeckt) plus ein gezielter
Kompilier-/Typprüf-Lauf des geänderten Codes (kein vollständiger
Testlauf) vor Commit+Push.

**Nachtrag nach Schritt 7 — Plan-Abweichung `apx_ai::depth_estimate`:**
der Plan sah die Laplace-Varianz-Schärfeheuristik für den neuen
`BlurDepthApprox`-Maskentyp in `apx_ai::depth_estimate::
relative_sharpness_map` vor. Tatsächlich implementiert wurde sie
stattdessen direkt in `apx-pipeline::stages::masks` (wie
`color_range_alpha`/`luminance_range_alpha`) — `apx-ai` hängt von
`apx-pipeline` ab (siehe dessen `Cargo.toml`), eine Abhängigkeit in die
umgekehrte Richtung wäre ein Zyklus gewesen. `MaskGeometry::
BlurDepthApprox { threshold: f32 }` wird wie `ColorRange`/
`LuminanceRange` live pro Render berechnet, nicht als vorab in `apx-ai`
gebackene Alpha-Bitmap (anders als `AiGenerated`) — deshalb passt die
Eigenständigkeit in `apx-pipeline` auch inhaltlich besser als ein
Umweg über `apx-ai`.

**Nachtrag nach Schritt 11 — Phase-10-Nachträge, mit einer weiteren
ehrlichen Grenze bei der Installer-Signierung:**
- **Lokalisierung**: die 13 in der Phase-10-Abnahme namentlich als
  unübersetzt benannten Dialog-Komponenten (Export/Druck/Diashow/Buch/
  Web/Vorlagen/Organisieren/Stacking/Skript & Plugins/Kollaboration/
  Tethering/Metadaten-Editor/Statistik) sind jetzt vollständig über
  `lib/i18n.ts`s `t()`-Muster übersetzt (de.ts/en.ts). `SlideshowPlayer.tsx`
  (die separate Vollbild-Wiedergabekomponente, die `SlideshowDialog.tsx`
  bei „Abspielen" öffnet) bleibt bewusst außerhalb dieses Schritts —
  ehrlich als offene Ausbaustufe stehen gelassen statt stillschweigend
  mitgezählt, ebenso die von `MetadataDialog.tsx` wiederverwendeten
  `PRESET_CONDITION_FIELD_OPTIONS`/`PRESET_CONDITION_OPERATOR_OPTIONS`-
  Labels aus `lib/presets.ts` (gemeinsam mit `SavePresetDialog.tsx`
  genutzt, eigener Schritt nötig).
- **`PaletteFrame`-Ausrollung**: `DevelopPanel.tsx`/`MasksPanel.tsx` sind
  jetzt in `PaletteFrame` gewrappt (Ziehen/Einklappen wie die übrigen vier
  Paletten). `MasksPanel.tsx`s `id="stage-masks"`-Sprunganker (von
  `DevelopPanel.tsx`s Node-Editor „Öffnen"-Link genutzt) wandert dabei auf
  die `<h2>`-Überschrift innerhalb des neuen `PaletteFrame`, weil dieser
  selbst kein durchgereichtes `id`-Attribut auf sein `<aside>` anbietet —
  funktional identisch (`scrollIntoView` scrollt denselben Container).
  Direkt danach volle `develop-flow.spec.ts`/`masks-flow.spec.ts`-Regression
  gefahren (40 Tests grün) — dieselbe Vorsicht, die Phase 10 Schritt 3
  zurückgestellt hatte, jetzt mit Testnetz.
- **Umbelegbare lokale Tastenkürzel**: `Viewer.tsx`s Zoom-Zifferntasten
  (neu: `zoom-fit`/`zoom-100`) und `DevelopPanel.tsx`s eigener Ctrl/Cmd+Z-
  Handler laufen jetzt über `lib/keybindings.ts`s `matchesBinding` statt
  fest verdrahteter `event.key`-Vergleiche. Der Entwickeln-Panel-Handler
  nutzt bewusst dieselben `"undo"`/`"redo"`-IDs wie `App.tsx`s Bibliotheks-
  Metadaten-Undo (statt eigener neuer IDs), weil sich beide Kontexte
  gegenseitig ausschließen (`App.tsx` reicht Ctrl/Cmd+Z nur weiter, wenn
  das Entwickeln-Panel geschlossen ist) — ein Nutzer, der „Rückgängig"
  umbelegt, bekommt eine konsistente neue Taste in beiden Kontexten.
  Bewusst weiterhin fest: Kurven-/Masken-Editoren mit
  `role="slider"`-Pfeiltasten-Feinjustierung und die Bewertungs-
  Zifferntasten 0–5 (parametrisierte Ziffernreihe, keine einzelne feste
  Aktion). Ein neuer Playwright-Test (max. 1 pro Schritt, siehe oben)
  deckt beides ab: Ctrl+Z/Ctrl+Shift+Z committen jetzt tatsächlich über
  den Tastatur-Pfad im Entwickeln-Panel (die bestehende
  `develop-flow.spec.ts`-Suite deckte bislang nur den Rückgängig-Knopf ab,
  nicht die Taste), und eine Neu-Belegung im Cheatsheet-Overlay steuert
  Ctrl+Z tatsächlich um.
- **Installer-Signierung — Mechanik-Nachweis, weiter eingeschränkt als
  im Plan vorgesehen**: der Plan sah vor, ein selbstsigniertes Test-
  Zertifikat einmalig als echtes GitHub-Actions-Secret zu setzen und
  dadurch `ci.yml`s `release`-Job (Windows-Zertifikatsimport/
  Fingerabdruck-Ermittlung/`--config`-Override) real durchlaufen zu
  lassen. **Diese Sitzung hat kein Werkzeug, um Repository-Secrets zu
  schreiben** (kein Admin-API-Zugriff über die verfügbaren GitHub-Tools),
  und keinen Windows-/macOS-Ausführungskontext (reine Linux-Sandbox) —
  ein echter CI-Lauf des `release`-Jobs war damit nicht möglich, ohne
  einen Menschen um das Setzen des Secrets zu bitten (außerhalb des
  Scopes dieser Sitzung). Stattdessen lokal verifiziert, **nur der
  betriebssystemunabhängige Teil der Mechanik**: ein selbstsigniertes
  Test-Zertifikat wurde per `openssl` erzeugt, zu einem PFX/P12-Bundle
  gepackt (dasselbe Format wie `WINDOWS_CERTIFICATE`/`APPLE_CERTIFICATE`),
  Base64-kodiert und wieder dekodiert (exakt der Schritt, den `ci.yml`s
  `[Convert]::FromBase64String` beim echten Secret durchführt) —
  Byte-für-Byte identisch zum Original bestätigt — und sein SHA1-
  Fingerabdruck ermittelt (dieselbe Kennzahl, die Windows unter
  `$cert.Thumbprint` liefert, nur von `openssl x509 -fingerprint -sha1`
  anders formatiert ausgegeben). Zertifikat/Schlüssel/PFX wurden direkt
  danach gelöscht, nichts committet. **Bleibt offen**: der eigentliche
  `Import-PfxCertificate`-Aufruf in einen Windows-Zertifikatspeicher und
  ein echter `tauri build`-Lauf mit `--config`-Override sind weiterhin
  nie ausgeführt worden — dieselbe Grenze wie in ADR-0037, jetzt nur um
  den zusätzlichen Befund ergänzt, dass auch der Secret-Setzschritt
  selbst außerhalb der Werkzeuge dieser Sitzung liegt.

## ADR-0039: Phase 12 — Lightroom-Lückenschluss; `lensfun`-Crate real geprüft und für Schritt 3 freigegeben

**Status:** Angenommen
**Kontext:** Der Nutzer hat einen ausführlichen, bidirektionalen
Funktionsvergleich von ApertureX gegen Adobe Lightroom Classic/CC in
Auftrag gegeben (veröffentlicht als Artifact „ApertureX vs. Lightroom"),
jede Zeile aus `FEATURES.md` gegen den echten Lightroom-Funktionsumfang
geprüft. Direkt im Anschluss wurde angewiesen, für die dort gefundenen
Lücken einen Plan mit tief recherchierten, echten Lösungen zu schreiben —
mit ausdrücklichem Fokus auf die Frage, ob der Kamera-/Objektiv-Profil-
Import über eine „KI-generierte" Lösung tragen könnte.

**Befund zur KI-Frage:** eine echte Modellinferenz für Verzeichnungs-/
Vignettierungs-Koeffizienten aus einem einzelnen Foto ohne Kalibrierziel
bräuchte trainierte Gewichte — dieselbe seit ADR-0033 dokumentierte Wand
wie jede andere „KI"-Funktion in diesem Projekt. Eine LLM-„Schätzung"
von Objektiv-Koeffizienten ohne echte Kalibrierdatengrundlage wäre reine
Fabrikation und wird hier bewusst **nicht** vorgeschlagen.

**Stattdessen real gefunden — die `lensfun`-Crate** (crates.io, v0.7.0,
reines Rust, LGPL-3.0-or-later, Autor David Veszelovszki, Repo
`github.com/vdavid/lensfun-rs`): ein gegen die C++-Referenzbibliothek
bit-exakt getesteter Port (laut Projekt-README 1.640 A/B-Testfälle,
Abweichung 4,88×10⁻⁴ Pixel) der echten, offenen LensFun-Objektivdatenbank.
`Database::load_bundled()` liefert Tausende real kalibrierte Kamera-/
Objektiv-Kombinationen direkt eingebettet (~574 KB gzip), ohne
Laufzeit-Dateisystemzugriff. Das macht den bisherigen 3-Profile-
Platzhalter (ADR-0028) für Schritt 3 gegenstandslos — **andere
Datenquelle** (offene LensFun-Datenbank statt Adobe-DCP/LCP), **gleiche
Wirkung**, ohne das ungelöste Adobe-Format-Problem selbst anzufassen.

**Spike real durchgeführt** (nicht nur `--dry-run`): `cargo add
lensfun@0.7` in `apx-pipeline`, `cargo build -p apx-pipeline` erfolgreich
(„Finished dev profile … in 2m 01s"), einzige neue Abhängigkeit ist
`roxmltree` (reines Rust, DOM-XML-Parser) — keine C-Bindings, kein
`bindgen`/`libclang`. Ein Test lädt die gebündelte Datenbank, findet
`Canon EOS 5D Mark III` + `Canon EF 16-35mm f/4L IS USM` real in den
Daten und liest deren Verzeichnungskalibrierung bei 16mm aus
(`crates/apx-pipeline/src/lens_profiles.rs`,
`lensfun_bundled_database_has_plausible_distortion_calibration_for_known_lens`).
**Ehrlicher Befund dabei:** `lensfun`s Poly3-`k1` (0,0128 für dieses
Objektiv) folgt einer anderen Vorzeichen-/Skalierungskonvention als
unser bisheriges `generic-wide`-Profil (`distortion_k1 = -0,12`, eigene
Konvention seit ADR-0028) — ein direkter Zahlenvergleich zwischen beiden
Systemen ist nicht aussagekräftig; die echte Umrechnung (inkl.
`Modifier`s Re-Skalierung auf Bildmaße/Cropfaktor) ist Aufgabe von
Schritt 3 Teil A. Der Spike verifiziert nur die Grundvoraussetzung:
die Datenbank liefert für real existierende Objektive nutzbare,
begrenzte Kalibrierwerte.

**Für Objektive außerhalb der Datenbank** (Teil B, Schritt 3): ein
Kalibrier-Assistent, der aus vom Nutzer selbst fotografierten
Schachbrett-Kalibrierbildern per klassischer Zhang-Methode (Ecken-
erkennung, Homographie-Schätzung, nichtlineare Verfeinerung — reine
Optimierung, kein gelerntes Modell) ein Profil berechnet. Wird im Dialog
und in `FEATURES.md` ehrlich als „aus eigenen Kalibrierfotos berechnet"
beschriftet, nicht als „KI-generiert".

**Entscheidung:** Phase 12 bündelt alle im Lightroom-Vergleichs-Artifact
gefundenen, tatsächlich schließbaren Lücken in neun Bauschritten (0–8):
Live-Masken-Overlay, Radialverlauf-Ellipse + Auto-Mask, echte LensFun-
Datenbank + Kalibrier-Assistent, voller EXIF/IPTC-Editor, Mehrfachziel-
Export, freies ICC-Profil beim Soft-Proof, beobachtete Ordner/Auto-
Import. **Bewusst ausgeklammert** (echte Design-Entscheidung oder bereits
dokumentiertes Beschaffungsproblem ohne neue Datenlage, siehe Vergleichs-
Artifact): Cloud-Synchronisation/mobile Begleit-App, Mehrfach-Katalog,
Publish Services, Print-on-Demand-Bestellintegration, Adobe-kompatibles
Plugin-SDK, generative KI-Bildbearbeitung, HEIF-Export (ADR-0038 bereits
geprüft, keine neue Datenlage). Testdisziplin wie vom Nutzer für den Rest
von Phase 11 angeordnet fortgeführt: ein gezielter Test pro Schritt, volle
Suite einmalig in Schritt 8.

## ADR-0039-Nachtrag: Schritt 3 real umgesetzt — echte Ecken-Rückrechnung statt Koeffizienten-Übernahme, vereinfachter Kalibrier-Assistent statt vollem Zhang-Verfahren

**Status:** Angenommen
**Kontext:** Schritt 3 Teil A/B wurden umgesetzt. Zwei Stellen weichen
ehrlich vom ursprünglichen ADR-0039-Text ab — beide Male, weil die
Recherche beim Bauen mehr Klarheit brachte als beim Planen.

**Teil A — Befund, der die geplante Schema-Migration gegenstandslos
machte:** `radius_x`/`radius_y`/`angle_degrees` (Schritt 2) UND die
Grundannahme für Schritt 3 wurden geprüft, bevor Code geschrieben wurde
— dabei stellte sich heraus, dass `LensCorrectionAdjustment`s einzige
wirkliche Lücke die *Datenquelle* für `distortion_k1`/`vignette_amount`/
`ca_red_cyan`/`ca_blue_yellow` war, nicht das Feldschema selbst. Die
eigentliche Herausforderung: `lensfun`s eigene Modelle (Poly3/Poly5/
PTLens für Verzeichnung, mehrgliedrige TCA-/Vignettierungs-Polynome)
sind reichhaltiger als unser Ein-Wert-r²-Modell — ein LensFun-
Koeffizient lässt sich nicht 1:1 übernehmen, selbst mit korrekter
Einheiten-Umrechnung, weil die Kurvenform selbst eine andere ist (exakt
die Lücke, die der Schritt-0-Spike als „Aufgabe von Schritt 3" benannt
hatte). Gelöst über eine an LensFuns *eigener* `Modifier`-Pixel-
mathematik verankerte Rückrechnung: die reale, mehrparametrige Korrektur
wird an der Ecke eines repräsentativen 3:2-Referenzbilds ausgewertet
(`apply_geometry_distortion`/`apply_color_modification_f32`/
`apply_subpixel_distortion` — dieselben Funktionen, die ein Foto real
korrigieren würden), und daraus ein einzelner Koeffizient gesucht, der
in unserem Modell an derselben Stelle dieselbe Wirkung erzeugt (siehe
`crates/apx-pipeline/src/lens_profiles.rs`s
`derive_lens_correction_values`). Eine echte, nachvollziehbare Näherung
— keine geratene Zahl —, mit der ehrlichen Grenze, dass sie nur an der
Bildecke exakt stimmt.

**Teil B — bewusst kein Zhang-Verfahren:** der ursprüngliche Plantext
nannte „Eckenerkennung per Harris-artigem Detektor + Homografie-
Schätzung + nichtlineare Verfeinerung". Eine robuste automatische
Schachbrett-Eckenerkennung plus volle Mehrparameter-Kamerakalibrierung
ist ein eigenständiges, fehleranfälliges Computer-Vision-Projekt für
sich — für unser Ein-Wert-Verzeichnungsmodell überdimensioniert und in
diesem Umfang nicht seriös umsetzbar. Stattdessen implementiert
`apx-ai::lens_calibration` eine bewusst schmalere, aber ebenso reale
Methode: der Nutzer markiert selbst mehrere Punkte entlang einer in der
Realität geraden Linie (direkt auf einer Bildvorschau im neuen Dialog
„Objektiv kalibrieren", `LensCalibrationDialog.tsx`), und
`calibrate_distortion_k1` sucht per Rasterverfeinerung den einen
Verzeichnungskoeffizienten, der alle markierten Linien nach der
Entzerrung gemeinsam am geradesten macht (totale Kleinste-Quadrate für
die Geradheit, klassische 1-D-Optimierung für die Suche — kein
gelerntes Modell). Ein Test mit synthetisch verzeichneten Linien
bestätigt, dass ein bekannter Koeffizient wiedergefunden wird
(Abweichung < 0,01). Ergebnis lebt direkt im EDL
(`LensCorrectionAdjustment::custom_distortion_k1`, additiv per
`#[serde(default)]`) statt in einer neuen Profildatenbank/-datei —
Wiederverwendung auf andere Fotos über die seit Phase 5 vorhandene
Einstellungen-kopieren-Funktion, kein neuer Persistenzmechanismus nötig.
Bewusst nur Verzeichnung (Vignette/CA bräuchten andere Messungen –
Helligkeits- bzw. Kanal-Registrierung statt Geradheit – nicht Teil
dieses Umfangs).

**Entscheidung:** Beide Abweichungen sind Umfangs-Präzisierungen, keine
Kürzungen am eigentlichen Nutzen — Teil A liefert weiterhin eine echte,
Datenbank-gestützte Objektiv-Zuordnung (jetzt sogar automatisch beim
Fotowechsel), Teil B weiterhin eine echte, aus eigenen Fotos berechnete
Kalibrierung für Objektive außerhalb der Datenbank. `FEATURES.md` und
`PLAN.md` sind entsprechend präzisiert.

## ADR-0039-Nachtrag II: Schritt 6 — echter ICC-Soft-Proof über `lcms2::Transform::new_proofing` ersetzt die JS-Näherung

**Status:** Angenommen
**Kontext:** Der Soft-Proof im Entwickeln-Panel (Phase 6 Schritt 10,
ADR-0032 Punkt 6) war bis hierhin eine rein clientseitige Sättigungs-
Näherung mit drei erfundenen "simulierten" Zielen (`srgb`/`print_sim`/
`grayscale_sim`) — ADR-0032 selbst nannte das explizit als bewusste
Vereinfachung, mit "kein echtes 3D-Gamut-Mapping" als offen benannte
Lücke. `apx_export::icc` bindet `lcms2` bereits seit Phase 8 Schritt 2
für den Export ein (`convert_from_srgb`, inklusive `IccTarget::
CustomFile` für eine vom Nutzer gewählte `.icc`-Datei) — dieselbe
Bibliothek unterstützt über `Transform::new_proofing` (kombiniert mit
den `SOFT_PROOFING`/`GAMUT_CHECK`-Flags und `cmsSetAlarmCodes` für die
Farbumfangswarnung) echtes, standardbasiertes Soft-Proofing, exakt die
Funktion, die auch Lightroom/Photoshop intern nutzen.

**Umsetzung:** neue Funktion `apx_export::icc::soft_proof_rgba8`
(`Transform::new_proofing(sRGB-Anzeigeprofil, …, Zielprofil, Intent,
Intent, Flags)`) für die vier gebündelten Standardprofile UND eine
beliebige `.icc`-Datei — dieselbe `IccTarget`-Wiederverwendung wie beim
Export. Statt eines neuen Tauri-Commands (der Puffer wäre pro
Regler-Tick zu groß für JSON-serialisierte IPC, siehe die bestehende
Begründung in `crates/apx-app/src/protocol/mod.rs`s Moduldoku, "ohne den
Umweg über Base64-kodierte Tauri-Commands") läuft der Soft-Proof über
ein zusätzliches `<soft_proof>`-Segment derselben `develop/...`-Route,
die auch die normale Vorschau liefert (`none` oder base64url-kodiertes
JSON, siehe `crates/apx-app/src/protocol/route.rs`s Moduldoku) — der
Server liefert bei aktivem Soft-Proof direkt den fertig transformierten
Puffer, kein zweiter Nachbearbeitungsschritt im Frontend für Farbe/Gamut.

**Bewusst erhaltene, kleinere Vereinfachung:** die Papierweiß-Simulation
hat in `lcms2` keine eingebaute Entsprechung (in echten
Bildbearbeitungsprogrammen meist eine separate, dem ICC-Proofing
nachgeschaltete Tonwertkompression) und bleibt daher eine kleine
clientseitige Nachbearbeitung (`lib/softProof.ts::applyPaperWhite`) —
anders als vorher betrifft sie aber nur noch den Tonwertbereich, nicht
mehr Farbe/Sättigung/Gamut. Aus demselben Grund bleibt `developFrame`
selbst (für Farbaufnehmer/TAT/Clipping-Overlay) immer der unveränderte,
nicht soft-proofte Puffer — der Viewer holt bei aktivem Soft-Proof eine
zweite, separate Antwort derselben Route nur für den tatsächlich
gezeichneten Canvas-Inhalt.

**Entscheidung:** `FEATURES.md`/`PLAN.md` sind entsprechend aktualisiert
— Soft-Proof ist ab hier echtes ICC-Farbmanagement, keine Näherung mehr.

## ADR-0039-Nachtrag III: Schritt 7 — Beobachteter Ordner / Auto-Import genau wie geplant umgesetzt

**Status:** Angenommen
**Kontext:** Schritt 7 (Beobachteter Ordner) folgt exakt dem im
Ursprungsplan skizzierten Weg, ohne Abweichung — festgehalten hier nur der
Vollständigkeit halber, wie bei jedem Schritt dieser Phase.

**Umsetzung:** neuer `WatchedFolderSettings`-Block in `apx_core::settings`
(Pfad, an/aus, Poll-Intervall in Sekunden, Default aus) neben den
bestehenden `UiSettings`/`AiSettings`; ein neuer Hintergrund-Task
`watched_folder_worker` in `apx-app/src/main.rs`, nach demselben
Abfragen-statt-Weck-Benachrichtigung-Muster wie der bereits bestehende
`export_queue_worker`. Bei jedem Durchlauf werden die Einstellungen frisch
von der Platte gelesen (ein Umschalten in den Einstellungen wirkt ohne
Neustart) und, falls aktiviert und der Ordner existiert, derselbe
`import::run_with_mode`-Pfad im Modus `AddInPlace` angestoßen wie bei
einem manuellen Import — geteilt über dieselbe `active_import`-Sperre, die
schon einen doppelten manuellen Import verhindert, damit sich ein
automatischer und ein manueller Import nie überschneiden. Kein natives
Datei-System-Watcher-Crate nötig: `run_with_mode` überspringt bereits
katalogisierte Dateien von selbst (`SingleFileOutcome::Unchanged`), ein
wiederholter Lauf über denselben Ordner ist also von sich aus billig und
idempotent — kein eigener "bereits gesehen"-Zustand nötig.

**Entscheidung:** `FEATURES.md`/`PLAN.md` sind entsprechend aktualisiert.

## ADR-0040: Phase 13 — echte ONNX-Laufzeit jetzt real verfügbar (korrigiert ADR-0033 Punkt 1); KI-Ausfüllen, Direktimport, DCP-Profile, klassische CV-Lücken

**Status:** Angenommen
**Kontext:** Aus der aktualisierten Lightroom-Lückenliste (Vergleichs-
Artifact nach Phase 12) hat der Nutzer sechs Punkte ausgewählt:
generatives/bearbeitendes KI-Ausfüllen, Import direkt von Speicherkarte/
Kamera, Adobe-DCP-Farbprofil-Import (fest zugesagt); Perspektive/Upright-
Kantenerkennung und Panorama-Homografie-Stitching (kostenlos geplant);
mehrere Kataloge, ein echter UND/ODER-Regelbauer, echte Personen-
Wiedererkennung ("vielleicht").

**Korrektur an ADR-0033 Punkt 1:** ADR-0033 begründete den Verzicht auf
echte ONNX-Runtime-Modellinferenz u. a. damit, es gebe "keinen
bestätigten Zugriffsweg auf vorkompilierte ONNX-Runtime-Binaries" und
"keinen legitimen Weg, ein trainiertes ... Modell zu beschaffen und
mitzuliefern". Diese Sitzung hat beides real geprüft und beides ist
inzwischen falsch:

1. **Die Laufzeit ist real verfügbar** — per `cargo add --dry-run`
   gegen die echte crates.io-Registry bestätigt: `ort` (2.0.0-rc.13,
   ONNX-Runtime-Bindings) und `tract-onnx` (0.23.6, reines Rust, keine
   C++-Abhängigkeit) sind beide echte, gepflegte Crates.
2. **Für mindestens ein Modell ist auch die Gewichts-Beschaffung
   real gelöst:** LaMa (Large Mask Inpainting, `advimman/lama`,
   Apache-2.0-Code) hat ein echtes, als ONNX exportiertes Modell
   öffentlich auf Hugging Face (`Carve/LaMa-ONNX`, `lama_fp32.onnx`,
   208 MB, Apache-2.0, trainiert auf Places2/CC-BY-4.0) — real
   herunterladbar, lizenzgeklärt, gegen einen veröffentlichten Hash
   verifizierbar. Kein fabriziertes Modell, keine Behauptung ohne
   Beleg — genau die Art Fund, die ADR-0033 damals nicht hatte.

Für dieselbe klassische CV, aber ohne jedes Modellgewicht (Perspektive/
Upright, Panorama-Stitching), sind ebenfalls real verfügbare, reine
Rust-Crates gefunden: `imageproc` (Kantenerkennung/Hough-Transformation),
sowie das `rust-cv`-Ökosystem `akaze`/`arrsac`/`sample-consensus`/
`homography`/`eight-point`/`lambda-twist`/`cv-core`/`space`
(Merkmalserkennung, robuste Homografie-Schätzung) — kein OpenCV nötig.
Für Speicherkarten-Erkennung: `sysinfo` (0.38.4, plattformübergreifende
Wechseldatenträger-Liste).

**Für Gesichts-Wiedererkennung bleibt eine echte Lücke offen:** die
Laufzeit ist dieselbe wie bei LaMa, aber ein gutes vortrainiertes
Gesichts-Embedding-Modell mit wirklich permissiver Lizenz (nicht nur
"Forschung, nicht kommerziell", wie viele InsightFace/ArcFace-
Veröffentlichungen) ist bei dieser Recherche noch nicht bestätigt —
Phase 13 Schritt 8 behandelt das als eigene, ergebnisoffene Prüfung
statt es stillschweigend anzunehmen.

**Entscheidung:** ADR-0033 Punkt 1 gilt als durch diesen Nachtrag
korrigiert (nicht rückwirkend umgeschrieben — der ursprüngliche Text
bleibt stehen, war zum damaligen Rechercheergebnis ehrlich). Phase 13
setzt echte ONNX-Inferenz für das KI-Ausfüllen ein (Schritt 1) und
klassische, gewichtsfreie CV für Perspektive/Panorama (Schritte 4/5).
Vollständiger Schritt-für-Schritt-Plan in `PLAN.md`.

## ADR-0040-Nachtrag: Schritt 3 — echter Adobe-DCP-Import; `ColorMatrix`
geparst, aber bewusst nicht angewendet; `ProfileHueSatMap`/
`ProfileToneCurve` real umgesetzt, direkt von Adobes eigenem
Open-Source-DNG-SDK portiert

**Status:** Angenommen
**Kontext:** `stages::calibration`s bisherige `CAMERA_PROFILES`-Handliste
(sechs feste Namen, je nur ein globaler Sättigungs-/Kontrast-Bias, siehe
ADR-0028) sollte durch echten Import beliebiger Adobe-`.dcp`-Dateien
ersetzt werden — derselbe Sprung wie Phase 12 Schritt 3 von einer
Objektiv-Handliste zur echten LensFun-Datenbank.

**Ergebnis der Recherche:** `.dcp`-Dateien sind reine TIFF/IFD-Container
ohne Rohbild-Daten. `gamut-dng`s `DngDecoder::decode()` (bereits
Abhängigkeit von `apx-export`) verlangt ein echtes Rohbild und scheitert
deshalb an eigenständigen `.dcp`-Dateien. Die real verfügbare Lösung:
`gamut-ifd` (2.0.1, dieselbe IFD-Lese-Grundlage, die `gamut-dng` selbst
intern nutzt) direkt einbinden und nur die tatsächlich benötigten Tags
lesen — per `cargo add --dry-run` gegen die echte crates.io-Registry
bestätigt. Die Tag-Nummern (`ColorMatrix1` 50721 usw.) sind gegen
`gamut-dng`s eigenes öffentliches `tags`-Modul **und** unabhängig davon
gegen Adobes eigenes, quelloffenes DNG-SDK
(`github.com/aizvorski/dng_sdk`) geprüft.

**Zwei Ebenen, unterschiedlich behandelt:**

1. **`ColorMatrix1`/`ColorMatrix2`/`ForwardMatrix1/2`/
   `CameraCalibration1/2`** (die eigentliche Farbmatrix) — wird geparst
   (`dcp_profile::DcpProfile`), aber **bewusst nicht in die Pipeline
   eingespeist**. Eine echte Kamera→XYZ-Matrixumrechnung ist Aufgabe des
   Rohdaten-Decoders (`apx-raw` hat dafür bereits eine eigene
   `cam_to_srgb`-Matrix, einmalig beim Dekodieren angewendet) — ein
   Matrixwechsel je Kalibrierungs-Regler würde eine erneute
   Rohdaten-Dekodierung bei jedem Bearbeitungsschritt verlangen, was der
   gesamten "einmal dekodieren, beliebig oft günstig entwickeln"-
   Architektur widerspräche. Dieselbe Scope-Grenze, die `gamut-dng`
   selbst für sich zieht ("Farbwiedergabe ist Aufgabe eines
   Rohdaten-Prozessors").
2. **`ProfileHueSatMapDims`/`ProfileHueSatMapData1`/`ProfileToneCurve`**
   (die "Look"-Daten, die z. B. Adobes "Camera Landscape" von "Camera
   Standard" unterscheiden) — wird **echt angewendet**, in
   `stages::calibration`, anstelle der bisherigen Handlisten-Näherung.
   Die PDF-Spezifikation selbst war von dieser Sandbox aus nicht
   abrufbar (`docs.rs`, `huggingface.co`, `helpx.adobe.com`,
   `paulbourke.net` — alle blockiert), aber Adobes eigenes DNG-SDK ist
   auf GitHub quelloffen (`raw.githubusercontent.com` war erreichbar):
   Indexierung, Interpolationsformel (trilinear, Farbton zirkulär) und
   die HSV-Parametrisierung (`0.0..6.0` statt Grad) in
   `stages::calibration::apply_hue_sat_map` sind eine direkte Portierung
   von `dng_reference.cpp::RefBaselineHueSatMap` (samt
   `dng_hue_sat_map.h/.cpp` für die Tabellen-Indexierung und
   `dng_utils.h::DNG_RGBtoHSV/DNG_HSVtoRGB` für die HSV-Umrechnung,
   letztere als `stages::color_math::rgb_to_hsv6`/`hsv6_to_rgb` portiert)
   — nicht aus dem Gedächtnis nachgebaut. Bewusst ausgelassen: das
   optionale `ProfileHueSatMapEncoding` (nichtlineare Wert-Kodierung, nur
   bei wenigen Profilen gesetzt) und die Spline-Interpolation der
   Tonwertkurve (stückweise linear stattdessen, siehe `apply_tone_curve`s
   Kommentar).

**CPU-only:** die HueSatMap-Tabelle ist variabel groß (bis zu Hunderten
Einträgen), passt nicht in `calibration.rs`s festes GPU-Uniform-Layout —
läuft aus demselben Grund CPU-seitig wie `ContentAwareFill`/`AiInpaint`
(`stages::repair`).

**Entscheidung:** Neues additives `CalibrationAdjustment::dcp_profile:
Option<DcpProfileData>` (dasselbe "einmal auflösen, als Zahlen im EDL
ablegen"-Muster wie `AiFillPatch`, Phase 13 Schritt 1) — hat Vorrang vor
`camera_profile`s Handliste, die als Fallback bestehen bleibt, wenn kein
`.dcp` importiert wurde. `apx-app`s `import_dcp_profile`-Command öffnet
einen Datei-Dialog und parst — kein eingebautes Profil im Installer,
derselbe Opt-in wie beim LensFun-Kalibrier-Assistenten.

## ADR-0040-Nachtrag II: Schritt 4 — echte Perspektive/Upright-
Kantenerkennung (`imageproc`); zwei erkannte Effekte statt vier
unabhängiger, `Full` bewusst identisch zu `Auto`

**Status:** Angenommen
**Kontext:** `stages::lens_corrections`s vier Upright-Automatikmodi
(`Auto`/`Level`/`Vertical`/`Full`) waren seit Phase 4 (ADR-0028)
dokumentierte No-op-Platzhalter — wählbar, ohne Wirkung. Nur der
„Guided"-Modus tat etwas: der Nutzer zieht selbst zwei Hilfslinien, ihr
gemittelter Neigungswinkel wird zur Dreh-Korrektur.

**Umsetzung:** `apx-ai::upright` (neues Modul, klassische CV ohne
gelerntes Modell — dieselbe Handschrift wie `lens_calibration`, Phase 12
Schritt 3 Teil B): `imageproc::edges::canny` findet Kanten,
`imageproc::hough::detect_lines` findet darin gerade Linien
(Normalform-Winkel `0..180°`). Statt vier unabhängige Automatiken zu
bauen, macht das Modul **zwei echte Effekte** real und kombiniert sie je
nach Modus:
- **Level** (nahezu waagerechte erkannte Linien mitteln → `rotate_degrees`)
  — exakt dieselbe Rechnung wie `guided_rotation_degrees`, nur mit
  automatisch gefundenen statt vom Nutzer gezogenen Linien.
- **Vertical** (nahezu senkrechte erkannte Linien mitteln →
  `manual_transform.horizontal`-Scherung, die trotz des Namens die
  *senkrechte* Kantenkonvergenz korrigiert) — die Zuordnung
  Winkelabweichung → Scherungsreglerwert ist direkt aus
  `lens_corrections.rs`s bestehender `undo_manual_transform`-Formel
  hergeleitet (Koeffizientenvergleich, nicht geraten), und per
  Vorzeichen-/Größenprobe an einer synthetischen Testkante nachgerechnet
  (`upright.rs`s Testmodul) statt nur symbolisch behauptet — ein früherer
  Testentwurf mit fehlerhafter Verifikationsformel hätte sonst einen
  Vorzeichenfehler nicht aufgedeckt.
- **Auto**/**Full**: beide Effekte kombiniert. Eine echte Trennung
  zwischen Adobes moderater "Auto"-Automatik und einer vollen
  Vier-Parameter-"Full"-Homografie bräuchte eine echte
  Homografie-Schätzung aus mehreren, unabhängig konvergierenden
  Linienscharen — außerhalb des Umfangs des bereits in ADR-0028/-0030 auf
  ein einziges Scherungspaar vereinfachten Objektivkorrektur-Modells.
  `Full` verhält sich deshalb bewusst identisch zu `Auto`, statt eine
  durch die vorhandenen Daten nicht gedeckte zusätzliche Korrektur zu
  erfinden — dieselbe Ehrlichkeit wie Schritt 3s unangewandte
  DCP-Farbmatrix.

**Architektur:** `apx-ai` (nicht `apx-pipeline`) bekommt die neue
`imageproc`-Abhängigkeit (`default-features = false`, nur `rayon` —
`text`/`fft` unnötig), da `apx-ai` bereits von `apx-pipeline` abhängt
(nicht umgekehrt) und `lens_calibration` als Vorbild ebenfalls dort
lebt. `apx-app`s `detect_upright_correction`-Command folgt demselben
"Analyse-Auflösung über `TileCache` dekodieren"-Muster wie
`generate_ai_mask`, obwohl es keine KI-Funktion im engeren Sinn ist.
Frontend übernimmt nur die zum gewählten Modus passende Komponente in
`manual_transform` (Level nur Rotation, Vertical nur Scherung) statt
beide Felder blind zu überschreiben — sonst würde ein Klick im
"Level"-Modus eine zuvor manuell gesetzte Scherungskorrektur
stillschweigend auf 0 zurücksetzen.

## ADR-0040-Nachtrag III: Schritt 5 — Panorama-Homografie-Stitching;
das geplante rust-cv-Ökosystem kompiliert auf keinem aktuellen
stabilen Rust, `imageproc`-FAST-Ecken + eigener BRIEF-Deskriptor +
`homography`-Crate stattdessen

**Status:** Angenommen
**Kontext:** ADR-0040 hatte für Panorama-Stitching das `rust-cv`-
Ökosystem (`akaze`, `cv-core`, `space`, `arrsac`, `sample-consensus`)
als real verfügbar eingestuft — per `cargo add --dry-run` gegen die
echte crates.io-Registry bestätigt. Dieser Schritt hat einen echten
Einbindungsversuch unternommen, und `cargo add --dry-run` erwies sich
als unzureichende Prüfung: es bestätigt nur, dass sich die Abhängigkeits-
Metadaten auflösen lassen, nicht dass der resultierende Code tatsächlich
kompiliert.

**Der reale Befund:** `akaze` (letzte Veröffentlichung 2021) hängt
transitiv an `bitarray 0.2`, dessen `src/lib.rs` unbedingt
`#![feature(min_const_generics)]` setzt. Dieses Attribut selbst — nicht
das längst stabile Feature dahinter — verlangt einen Nightly-Compiler
und schlägt auf jedem aktuellen stabilen Rust mit `error[E0554]` fehl,
real gegen `rustc 1.94.1` in dieser Sandbox getestet (`cargo test`
brach mit exakt diesem Fehler ab). Das ganze `rust-cv`-Ökosystem ist
damit auf stabilem Rust praktisch tot — unabhängig von den zusätzlich
noch bestehenden Versions-Inkompatibilitäten, die bereits vor diesem
Befund auffielen: `cv-core 0.15`/`nalgebra 0.21`/`space 0.10` (alle vom
selben Autor, derselben Ära) vs. der aktiver gepflegten `homography`-
Crate mit `nalgebra 0.33`; `homography` hängt zudem gar nicht von
`sample-consensus` ab (kein `Estimator`-Trait-Impl, per `grep` in
dessen `Cargo.toml` bestätigt) — `arrsac`/`sample-consensus`/`cv-core`/
`space` hätten also ohnehin keinen echten Klebstoff zwischen `akaze`
und `homography` geliefert.

**Umsetzung stattdessen** (`apx-stacking::homography_stitch`, neues
Modul, siehe dessen ausführliche Moduldoku): `imageproc::corners::
corners_fast9` (dieselbe Crate, bereits in Schritt 4 real erprobt) für
die Eckenerkennung; ein selbst geschriebener BRIEF-artiger 256-Bit-
Deskriptor (klassische, publizierte Technik — Calonder et al. 2010,
nicht fabriziert) statt eines externen Deskriptor-Crates; brute-force
Hamming-Abstands-Matching mit Lowes Verhältnistest; ein eigener, kurzer
RANSAC-Loop (Zufallsstichprobe → `homography`-Crates echte DLT-Schätzung
per SVD → Inlier zählen → beste Stichprobe über alle Inlier
verfeinern) statt der generischen `sample_consensus`/`arrsac`-
Maschinerie. `homography` bekommt dieselbe `nalgebra`-Version (`0.33`)
explizit in `apx-stacking` UND `apx-app` gepinnt — dieselbe Instanz im
Abhängigkeitsgraph statt einer separaten Kopie, sonst wäre
`homography`s `Matrix3<f64>` in eigenem Code ein anderer Typ.

**Test-Vorsicht mit synthetischen Bildern:** ein Schachbrettmuster als
Testbild schlug fehl — an dessen Kreuzungen treffen sich vier
Quadranten in einem X-förmigen Sattelpunkt, den FAST grundsätzlich
nicht erkennt (es braucht einen einzigen zusammenhängenden Bogen von
mindestens neun der 16 Kreispunkte). Ein perfekt periodisches
Punktraster schlug ebenfalls fehl — Lowes Verhältnistest verwirft zu
Recht mehrdeutige Treffer, wenn viele Merkmale identische lokale
Nachbarschaften haben. Beide Male kein Bug im Code, sondern ein
unrealistisches Testbild; die endgültigen Tests nutzen verstreute,
unterschiedlich große Quadrate an leicht versetzten Positionen.

**Entscheidung:** `apx-app`s `stack_panorama`-Command versucht das
echte Homografie-Stitching zuerst und fällt für die gesamte Fotoserie
auf die bestehende reine Verschiebungs-Registrierung
(`apx_stacking::panorama`, Phasenkorrelation) zurück, wenn für
mindestens ein Foto keine verlässliche Homografie gefunden wird —
bewusst alles-oder-nichts statt einer Mischkomposition aus beiden
Positionierungsarten auf derselben Leinwand.

## ADR-0040-Nachtrag IV: Schritt 6 — Mehrere Kataloge + Katalog-Wartung;
Katalogwechsel per Neustart statt Hot-Swap der offenen Verbindung

**Status:** Angenommen
**Kontext:** `AppState::catalog: Arc<Catalog>` wird in `commands.rs` von
praktisch jedem einzelnen Tauri-Command direkt referenziert oder
geklont. Ein echtes Hot-Swap der offenen Katalogverbindung im
laufenden Prozess hätte entweder jeden dieser Zugriffe hinter ein
zusätzliches Lock verlegt (invasiv, hohes Fehlerrisiko quer durch eine
~6000-Zeilen-Datei) oder den `Arc` selbst austauschbar gemacht
(dieselbe Umbau-Größe) — für einen einzelnen Schritt nicht vertretbar.

**Entscheidung:** dieselbe UX wie Adobe Lightroom Classics eigener
Katalogwechsel ("Diese Änderung erfordert einen Neustart"): Wechseln
oder Neuanlegen eines Katalogs speichert den Zielpfad in
`Settings::catalog` und ruft `AppHandle::request_restart()` — die App
startet neu und öffnet beim nächsten Start automatisch den neuen
Pfad. Dafür genügt eine einzige, unauffällige Änderung an `main.rs`
(liest `settings.catalog.last_opened_catalog` vor dem `Catalog::open`-
Aufruf) statt eines Umbaus der gesamten Command-Schicht.

**Fund:** `Settings::catalog::last_opened_catalog` existierte bereits
seit Phase 10 (Settings-Fundament) im Datenmodell, wurde aber nie
gelesen — `main.rs` öffnete unbedingt `paths.default_catalog_file()`.
Reine Attrappe, jetzt tatsächlich verdrahtet (mit Rückfall auf den
Standardkatalog, falls der hinterlegte Pfad seit dem letzten Start
verschoben/gelöscht wurde — kein Absturz beim Start wegen eines
veralteten Pfads).

**Katalog-Wartung** braucht dagegen keinen Neustart — sie arbeitet auf
der bereits offenen Verbindung: `apx_catalog::Catalog` bekommt
`integrity_check` (`PRAGMA integrity_check`, SQLites eigene
Standardprüfung auf strukturelle Schäden), `vacuum` (`VACUUM`) und
`backup_to` (SQLites Online-Backup-API über `rusqlite`s `backup`-
Feature — sicher neben der weiterhin offenen Verbindung nutzbar,
anders als eine rohe Dateikopie, die bei gleichzeitigem Schreibzugriff
eine inkonsistente Kopie ergeben könnte).

**Kein neuer Dialog-Wechsel-Mechanismus fürs Frontend nötig:** die
generischen `pick_file_path`/`pick_save_file_path`-Commands (bereits
für Drucken/Buch/ICC-Profil-Auswahl vorhanden) übernehmen die
Datei-Dialoge für "Neuer Katalog…"/"Katalog öffnen…"/"Sichern unter…"
direkt — die neuen Rust-Commands nehmen nur noch den bereits gewählten
Pfad entgegen.

## ADR-0040-Nachtrag V: Schritt 7 — echter UND/ODER-Regelbaum für
bedingte Presets und intelligente Sammlungen; in-memory statt
dynamischer SQL-Generierung

**Status:** Angenommen
**Kontext:** Zwei getrennte Stellen kannten bisher nur eine flache,
ausschließlich UND-verknüpfte Regelliste: bedingte Presets
(`PresetCondition[]`, ADR-0031 Punkt 4, "kein ODER, keine
Verschachtelung") und intelligente Sammlungen (`apx_catalog::
FilterCriteria`, feste Struktur-Felder Bewertung/Flagge/Farbe/
Kameramodell, ADR-0023). Beide werden bereits als opakes JSON
gespeichert (`conditions_json`/`smart_criteria_json`) — kein
Datenbankschema-Wechsel nötig, nur ein neues JSON-Schema.

**Entscheidung:** ein gemeinsamer, generischer Regelbaum-Typ
(`RuleNode<TLeaf> = { type: "condition"; condition: TLeaf } |
{ type: "group"; operator: "and" | "or"; children: RuleNode<TLeaf>[] }`,
`frontend/src/lib/ruleTree.ts`) mit einer rekursiven Auswertung
(`evaluateRuleNode`) und einem generischen Editor
(`RuleTreeEditor.tsx`, kennt das Blatt-Vokabular nicht — kommt per
`renderLeaf`/`makeDefaultLeaf`-Props vom Aufrufer). Dasselbe
JSON-Schema existiert auf der Rust-Seite als `apx_catalog::FilterNode`
(`#[serde(tag = "type", rename_all = "snake_case")]`) — bewusst
identisch zum TypeScript-Gegenstück gewählt, damit `create_smart_
collection` den vom Frontend erzeugten JSON-String unverändert als
opakes `criteria_json` durchreichen und rein per `serde_json`
parsen kann, ganz ohne eigene Übersetzungsschicht zwischen den beiden
Seiten.

**Bedingte Presets bleiben reines Frontend:** `conditions_json` ist
für `apx-catalog` schon vorher ein opaker String gewesen (unverändert:
`Preset.conditions_json`), also brauchte dieser Teil **keine
Rust-Änderung**. `PresetCondition`/`evaluateCondition`/
`parseConditions`/`applyConditionsToSubset` (`lib/presets.ts`) bleiben
unverändert bestehen — sie werden weiterhin von den Auto-
Verschlagwortungsregeln (`MetadataDialog.tsx`s `createTagRule`, ein
eigenes, außerhalb dieses Schritts liegendes Feature) benutzt und
dienen als Migrationsquelle. Neu: `PresetRuleGroup { section:
PresetSectionKey | null; node: RuleNode<PresetLeafCondition> }` — die
Sektions-Gatter-Semantik (`section: null` = ganzes Preset, sonst nur
diese Sektion; mehrere Regelgruppen bleiben untereinander
UND-verknüpft) bleibt exakt wie zuvor, nur ist jede einzelne Regel
jetzt ein ganzer UND/ODER-Baum statt einer einzelnen Bedingung.
`parseRules` akzeptiert die neue Baumform direkt und migriert sonst
über `migrateLegacyConditions` von der alten flachen Form — jede
vorhandene, vor diesem Schritt gespeicherte Preset-Bedingung bleibt
dadurch ohne Migrationsskript lesbar.

**Intelligente Sammlungen werten in-memory aus, nicht per dynamischer
SQL-Generierung:** `apx_catalog::FilterNode::matches(&Photo) -> bool`
wird für jedes Foto einzeln aufgerufen, nachdem `repository::
collections::list_photos` den gesamten Fotobestand bereits per SQL
geladen hat (`filter_photos` mit leeren Kriterien, dieselbe Abfrage
wie die Filterleiste). Eine zweite, rekursive WHERE-Klausel-
Generierung neben der bestehenden in `repository::search::
build_filter_clause` wäre für beliebig tiefe Verschachtelung deutlich
aufwendiger gewesen, ohne einen echten Nutzen — Kataloge in diesem
Projekt sind Einzelnutzer-Bibliotheken (siehe schon ADR-0040-Nachtrag
III zur Panorama-Homografie: "kein Web-Maßstab"), keine Datenbank mit
Millionen Zeilen, bei der ein voller Tabellenscan pro Sammlung
spürbar würde. `FilterCriteria`/`build_filter_clause` bleiben für die
Filterleiste/Stapelverarbeitungs-Konsole unverändert bestehen — dort
reicht flach UND-verknüpft weiterhin aus, ein Umbau auf den Regelbaum
hätte keinen Mehrwert gebracht.

**Migration alter intelligenter Sammlungen:** `parse_filter_node`
versucht zuerst, `smart_criteria_json` als `FilterNode` zu lesen; bei
fehlendem `"type"`-Tag (vor diesem Schritt gespeicherte, flache
`FilterCriteria`-Form) fällt es auf `FilterCriteria` zurück und
migriert über `impl From<FilterCriteria> for FilterNode` (jedes
gesetzte Feld wird eine Bedingung in einer UND-Gruppe) — dieselbe
"lies alt, migriere beim Zugriff, kein Schreib-Migrationsskript"-
Konvention wie bei den bedingten Presets oben.

## ADR-0040-Nachtrag VI: Schritt 8 — echte Personen-Wiedererkennung;
Lizenzprüfung verwirft InsightFace/SFace, `dlib`s öffentlich-erklärtes
Modell trägt

**Status:** Angenommen
**Kontext:** `PLAN.md` verlangt für diesen Schritt ausdrücklich, die
Lizenzprüfung selbst zum ersten Teilschritt zu machen, bevor Code
entsteht — mit einem ehrlichen "kein passendes Modell gefunden" als
zulässigem Ausgang (wie bei HEIF in Phase 11). Drei Kandidaten real
recherchiert:

- **InsightFace** (`buffalo_l`/`antelopev2`): Code MIT, aber die
  mitgelieferten Modellgewichte laut InsightFaces eigener
  Model-Zoo-Dokumentation ausdrücklich "für nicht-kommerzielle
  Forschungszwecke" — kommerzielle Nutzung verlangt eine separate,
  kostenpflichtige Lizenz von InsightFace selbst. **Verworfen**, genau
  die Falle, vor der `PLAN.md` warnt.
- **OpenCV Zoo `SFace`**: eine Apache-2.0-`LICENSE`-Datei liegt im
  Repo-Verzeichnis, aber das ONNX-Modell wurde ursprünglich auf einer
  von drei möglichen Datenbanken trainiert (CASIA-WebFace, VGGFace2
  oder MS1MV2), und `opencv/opencv_zoo`s eigene Maintainer haben auf
  direkte Nachfrage (Issues #124, `opencv/opencv#21192`) nie geklärt,
  welche der auto-heruntergeladenen `.onnx`-Datei zugrunde liegt.
  MS1MV2 leitet sich vom 2019 wegen Herkunfts-/Einwilligungsproblemen
  zurückgezogenen `MS-Celeb-1M` ab — eine oberflächlich permissive
  `LICENSE`-Datei klärt diese Herkunftsfrage nicht. **Verworfen**,
  genau die im Kontext-Abschnitt des Plans beschriebene Nuance (nicht
  blind einer Lizenzdatei vertrauen, ohne die Trainingsdaten-Herkunft
  zu prüfen).
- **`dlib`s eigenes Embedding-Netz**
  (`dlib_face_recognition_resnet_model_v1.dat`): der Autor
  (davisking, `davisking/dlib-models`-Repo-README) erklärt das
  trainierte Modell ausdrücklich und persönlich als gemeinfrei
  ("anyone can do whatever they want with these model files as I've
  released them into the public domain") — trotz teils
  nicht-kommerziell lizenzierter Trainingsquellen (Face Scrub). Das
  trägt, weil der Autor als tatsächlicher Rechteinhaber des
  *trainierten Modells* (eine eigenständige schöpferische Leistung,
  nicht identisch mit den Trainingsdaten) diese Freigabe explizit und
  öffentlich ausgesprochen hat — ein qualitativ anderer, stärkerer
  Beleg als eine pauschale Repo-`LICENSE`-Datei ohne Herkunftsklärung
  wie bei SFace oben. **Angenommen.**

Für die zur Gesichts-Ausrichtung nötigen Landmarken **nicht** das im
selben `dlib-models`-Repo mitgelieferte 68-Punkte-Modell
(`shape_predictor_68_face_landmarks.dat`) — dessen README zitiert
wörtlich einen Hinweis des Datensatz-Erstellers (Stefanos Zafeiriou),
der kommerzielle Nutzung des daraus trainierten Modells ausdrücklich
ausschließt. Stattdessen das 5-Punkte-Modell
(`shape_predictor_5_face_landmarks.dat`, CC0-1.0/gemeinfrei, aus
`dlib`s eigenem, separat erhobenem Datensatz) — `dlib`s
`get_face_chip_details`-Funktion (intern von der `dlib-face-recognition`-
Crate aufgerufen) unterstützt beide Landmarken-Zahlen gleichwertig zur
Gesichtsausrichtung, dieselbe 5-Punkte-Ausrichtung, die z. B. auch
`ageitgey/face_recognition` standardmäßig anbietet — keine
Notlösung, ein etabliertes Muster.

**Gesichts-*Erkennung*** (Bounding-Boxes, bevor überhaupt ein Embedding
berechnet wird) läuft über `dlib::get_frontal_face_detector` —
vollständig in `libdlib` selbst einkompiliert (Boost Software
License 1.0, keine externe Modelldatei, keine eigene Lizenzfrage).

**Entscheidung — Architektur:** `apx-ai::people::PersonEmbedder`
(neues Modul, hinter dem standardmäßig ausgeschalteten Cargo-Feature
`people`, dieselbe Konvention wie `apx-tether`s `tethering`/`gphoto2`)
bindet `dlib-face-recognition` mit dessen `build-native`-Feature an
die Systembibliothek `libdlib`. Die bestehende Hautton-Heuristik
(`apx-ai::faces::detect_face_regions`, Phase 11 Schritt 5) bleibt
unverändert als Fallback bestehen, wenn das Feature nicht kompiliert
oder keine Modelle hinterlegt sind — additiv, nicht ersetzend, wie
jede vergleichbare Erweiterung in diesem Projekt.

**Echter, verifizierter Fund in der Abhängigkeitskette:**
`dlib-face-recognition-sys`s `build.rs` (jede veröffentlichte Version
bis mindestens 20.0.1) ruft in `main()` *unbedingt* `dlib`s eigenen
Quellcode von `http://dlib.net` herunter, um ihn selbst zu kompilieren
— *bevor* es den bereits im selben Modul vorhandenen pkg-config-Pfad
gegen eine bereits installierte System-`libdlib` überhaupt versucht.
Dieser pkg-config-Pfad ist dadurch in jeder Version toter Code. In
dieser Sandbox zusätzlich verschärft: `dlib.net` ist vom
Netzwerk-Proxy blockiert (HTTP 403), dasselbe Beschaffungsproblem wie
`huggingface.co`/`cdn.pyke.io`/`docs.rs` an anderer Stelle in diesem
Projekt. **Fix:** `vendor/dlib-face-recognition-sys/` — eine lokal
gepatchte Kopie, die `main()` umsortiert (pkg-config zuerst probieren,
nur bei Fehlschlag herunterladen — dieselben zwei bereits vorhandenen
Codeblöcke, keine neue Logik), eingebunden über ein
`[patch.crates-io]` im Workspace-`Cargo.toml`. Real gegen die per
`apt install libdlib-dev libblas-dev liblapack-dev` installierte
System-`libdlib` 19.24 kompiliert, gelinkt und getestet (nicht nur
`cargo add --dry-run`).

**Echt spike-verifiziert, nicht nur behauptet:** gegen drei echte
Fotos (offizielle Weißes-Haus-Fotos von Pete Souza, US-Regierungswerk,
gemeinfrei) lief die volle Kette Gesichtserkennung → 5-Punkt-
Ausrichtung → 128-dimensionales Embedding → euklidischer Abstand.
Zwei Fotos derselben Person (`obama1.jpg`/`obama2.jpg`) lagen bei
Abstand 0.35 — unter der von `dlib`s eigener Dokumentation
empfohlenen Schwelle 0.6 ("dieselbe Person"); ein drittes Foto einer
anderen Person (`biden.jpg`) lag bei Abstand 0.85, klar darüber.
Derselbe Test läuft jetzt als `apx-ai::people`s Unit-Test (übersprungen
ohne lokale Modelldateien/Testfotos — kein Netzwerk-Download in CI,
siehe `PLAN.md` Phase 13s Verifikations-Abschnitt).

**Katalog-Schema** (`migrations/0011_people.sql`): zwei neue Tabellen,
`people` (benannte Person, `name: NULL` = automatisch erkannt, aber
unbenannt) und `face_detections` (Bounding-Box + Embedding als
JSON-Array, `person_id: NULL` = unzugeordnet). Auto-Zuordnung neu
erkannter Gesichter läuft — wie bei den intelligenten Sammlungen aus
Schritt 7 — in-memory: `repository::people::save_detections_for_photo`
lädt einmal alle bereits einer Person zugeordneten Gesichter und ordnet
ein neues Gesicht der nächstliegenden Person zu, wenn deren
euklidischer Abstand unter der Schwelle liegt; `SAME_PERSON_EMBEDDING_
THRESHOLD`/`embedding_distance` liegen bewusst in `apx_catalog::models`
statt in `apx-ai::people` (das hinter dem `people`-Feature steht),
damit diese reine Vergleichslogik unabhängig vom Feature kompiliert.

**Opt-in-Modell-Download, kein Bundling** (dasselbe Muster wie LaMa in
Schritt 1): `download_people_models` lädt beide `.dat.bz2`-Dateien von
`dlib.net` herunter und entpackt sie — **nicht in dieser Sitzung
erreichbar/verifiziert** (`dlib.net` blockiert, siehe oben), dieselbe
ehrliche Lücke wie beim LaMa-Modell; keine Hash-Prüfung aus demselben
Grund (kein erreichbarer, verifizierbarer Hash in dieser Sitzung).

## ADR-0041: Phase 14 — zehn Alleinstellungsmerkmale jenseits von
Lightroom; echte MiDaS-Tiefenschätzung und fast-neural-style-Stiltransfer
diesmal end-zu-Ende gegen ein echtes Foto verifiziert

**Status:** Angenommen
**Kontext:** Phase 13 hat die letzte Lücke gegen Lightroom geschlossen
(Vergleichs-Artifact, Stand Phase 13). Der Nutzer erklärt "so nah wie
möglich an Lightroom" (Ziel A) damit für erreicht und gibt ein neues
Ziel B vor: zehn eigenständige, visuell beeindruckende Fähigkeiten ohne
Lightroom-Entsprechung, je zur Hälfte klassisch/KI-gestützt (kostenlose
lokale Inferenz, keine bezahlten Cloud-APIs), jede sowohl für
Massenbearbeitung als auch Präzisionsarbeit tauglich. Der vollständige
Plan mit allen zehn Punkten steht in `PLAN.md` Phase 14.

**Recherche-Disziplin:** jede "Lightroom hat das nicht"-Behauptung im
Plan wurde per echter Web-Suche gegengeprüft (siehe Zitate in `PLAN.md`
Phase 14), nicht aus dem Gedächtnis behauptet. Ein ursprünglich erwogener
zehnter Kandidat ("Smart-Crop-Vorschläge") wurde verworfen, weil Adobe
"Suggested Crop" bereits in Lightroom Web ausliefert — zu nah an einer
bestehenden Adobe-Funktion für eine klare Abgrenzung.

**Schritt-0-Spikes, diesmal beide echt gegen ein reales Foto
verifiziert** (anders als der LaMa-Spike in Phase 13 Schritt 0, der
mangels erreichbarem `huggingface.co` nur gegen ein winziges
`Y = X + 1`-Modell lief): `huggingface.co` bleibt blockiert
(`subscribe_forbidden`/`403` auf jeden `CONNECT`-Versuch), aber
`github.com`, `raw.githubusercontent.com`, `release-assets.
githubusercontent.com` und — wichtiger Einzelfund — `media.
githubusercontent.com/media/...` (der echte Git-LFS-Auslieferungs-Host,
`raw.githubusercontent.com` liefert für LFS-Dateien nur den
128-Byte-Zeiger-Text) sind aus dieser Sandbox heraus erreichbar. Beide
Modelle unten wurden probehalber gegen `opencv/opencv`s eigenes
`samples/data/fruits.jpg` (ein echtes, im selben verifizierten Repo
liegendes Foto, keine Fabrikation) inferiert und das Ergebnis visuell
geprüft:

- **MiDaS v2.1 small** (`isl-org/MiDaS`, MIT, ONNX-Release-Asset,
  66 764 249 Bytes, `github.com/isl-org/MiDaS/releases/download/v2_1/
  model-small.onnx`): trotz symbolischer (scheinbar dynamischer)
  Tensor-Dimensionen im ONNX-Graph verlangt das Modell tatsächlich fest
  256×256 Eingabe (ein zu großer Testlauf schlägt mit einer expliziten
  ONNX-Runtime-Fehlermeldung fehl, kein stiller Fallback) — wichtig für
  Schritt 8, feste Skalierung auf 256×256 vor der Inferenz nötig. Die
  resultierende Tiefenkarte zeigt exakt die erwartete Struktur: eine
  helle (nahe), scharf konturierte Obst-Silhouette mit plausibler
  3D-Schattierung (spitze vs. runde Früchte unterscheidbar) vor einem
  durchgehend dunklen (fernen) Hintergrund — echte, korrekt
  funktionierende monokulare Tiefenschätzung, kein Zufallsrauschen.
- **fast-neural-style "mosaic"** (`onnx/models`, MIT,
  `validated/vision/style_transfer/fast_neural_style/model/mosaic-9.
  onnx`, 6 728 029 Bytes über den LFS-Media-Host geladen, Hash-Größe
  stimmt exakt mit dem Git-LFS-Zeiger überein): erwartet dynamische
  NCHW-Eingabe in `0..255` (kein ImageNet-Normalisieren, anders als
  MiDaS), Ausgabe ist ein 224×224-RGB-Bild mit derselben Auflösung wie
  die Eingabe. Ergebnis ist ein echtes, klar erkennbares
  Mosaik-/Glasfenster-Stilbild derselben Obstschale — funktioniert.

**Offene Frage ehrlich beantwortet (Spike B, "arbitrary style
transfer"):** ein permissiv lizenziertes Modell für *beliebige*
Referenzbilder als Stilvorlage (statt fünf fest einprogrammierter Stile)
existiert real — Googles Magenta-Projekt (`magenta/magenta`,
Apache-2.0, `arbitrary_image_stylization`) —, aber nur als
TensorFlow/TFLite-Checkpoint, nicht als ONNX-Export. Die einzige
gefundene ONNX-Variante eines AdaIN-basierten Modells (`rapidrabbit76/
Arbitrary-Style-Transfer-...-pytorch-lightning`) liegt auf Google Drive
(in dieser Sandbox ebenso unerreichbar wie huggingface.co) und ist ein
inoffizieller Nachbau ohne eigene, im Repo genannte Lizenzdatei für die
selbst trainierten Gewichte — genau die Art unklarer Herkunft, die
`ADR-0040-Nachtrag VI`s SFace-Ablehnung schon einmal begründet hat.
**Ergebnis, kein Workaround erzwungen:** Schritt 9 bleibt auf die fünf
sicher lizenzierten, real verifizierten festen `onnx/models`-Stile
beschränkt (candy/mosaic/rain-princess/udnie/pointilism) — ein
lizenzklares Modell für echte beliebige Referenzbild-Übertragung wäre
eine eigene, spätere Untersuchung wert, wird hier aber nicht durch eine
fragwürdige Google-Drive-Quelle ersetzt.

**Wiederverwendete Bausteine statt Neuaufbau** (Details je Schritt in
`PLAN.md` Phase 14): `blend_pixel()` aus `apx-pipeline::stages::masks`
(Mehrfachbelichtung, Halation), `frontend/src/lib/histogram.ts`s
Client-seitiges Berechnungsmuster über den bereits vorhandenen
Vorschau-Puffer (Vektorskop/Wellenform, kein neuer Backend-Command),
`apx-ai::inpaint::InpaintSession`/`RepairStroke` (Canvas-Erweiterung),
die bestehenden Hautton-/Saliency-Heuristiken in `apx-ai::segmentation`
(Himmel-Segmentierung), `kmeans_colors` (real per `cargo add --dry-run`
geprüft, v0.7.1, MIT/Apache — Farb-Harmonie-Rad).

### Nachtrag I (Phase 14 Schritt 1): Canvas-Erweiterung/Outpainting —
Margen als normierte Bruchteile statt absoluter Pixel

`GeometryAdjustment` bekommt additiv (`#[serde(default)]`, dieselbe
Konvention wie `RepairStroke::ai_fill`) ein `canvas_extension:
Option<CanvasExtension>` mit vier Rändern und einem optionalen,
vorab berechneten `CanvasExtensionPatch` (Bitmap + eigene
Speicherauflösung — dasselbe „einmal berechnen, bei jedem Rendern nur
noch skalieren"-Muster wie `AiFillPatch`). Die vier Ränder wurden zuerst
als `u32`-Pixelzahl entworfen (lose Analogie zu
`AiFillPatch::bitmap_width`), das war aber die falsche Einheit: eine
Speicherauflösung wie `bitmap_width` wird bei jedem Rendern ohnehin auf
die Zielgröße skaliert, ein *Rand* legt dagegen unmittelbar das neue
Seitenverhältnis der Leinwand fest und muss deshalb — wie `CropRect`s
normierte `0.0..=1.0`-Koordinaten — mit dem Bild mitskalieren. Korrigiert
auf `f32`-Bruchteile der jeweils aktuellen Bildbreite/-höhe, noch bevor
irgendein Test geschrieben wurde. `GeometryAdjustment` verliert dabei
`Copy` (der neue Patch trägt `Vec<u8>>`) — nichts im Rest der Codebasis
verließ sich auf `Copy` (`cargo check -p apx-pipeline` bestätigt sauber).

`stages/geometry.rs::extend_canvas` läuft als letzter Teilschritt in
`apply()` nach Drehung/Zuschnitt, rechnet die Bruchteile in Pixel um,
bettet das Original unverändert mittig ein und füllt den Rand aus dem
bilinear auf die tatsächliche neue Leinwandgröße hochskalierten Patch —
ohne Patch (Ränder gewählt, „Anwenden" aber noch nicht ausgelöst) bleibt
die Erweiterung ein reiner No-Op, exakt wie ein frischer
`RepairMode::AiInpaint`-Strich ohne `ai_fill`.

`apx-app::commands::run_ai_outpaint` braucht **kein** neues Modell und
**keinen** neuen Download-Command: dieselbe bereits bestehende
`apx_ai::inpaint::InpaintSession::fill_rgb8` (Phase 13 Schritt 1) nimmt
beliebige gleich große Pixel-/Maskenpaare entgegen, der Command baut nur
eine andere Maskenform — das gesamte gewählte Randgebiet statt eines
gemalten Pinselstrichs, Randpixel per Kanten-Klemmung vorbefüllt statt
mit einer harten Flächenfarbe (weniger sichtbare Kanten-Artefakte vor der
eigentlichen Inferenz). Dieselbe ehrliche Grenze wie bei jeder anderen
KI-Analyse dieses Projekts: läuft auf dem rohen,
`ANALYSIS_MAX_EDGE`-gedeckelten Dekodierergebnis, ohne eine im
Entwickeln-Modul bereits gesetzte Drehung/Zuschnitt zu berücksichtigen.

### Nachtrag II (Phase 14 Schritt 2): Frequenztrennung als Retusche-Ziel
statt eigener Pipeline-Stufe

Punkt 2 der Recherche-Tabelle ist mit einem Original-Zitat belegt:
"Adobe Lightroom doesn't have a built-in frequency separation feature
like Photoshop does." Die naheliegende Umsetzung — eine neue,
eigenständige Pipeline-Stufe, die das ganze Bild bei jedem Rendern in
zwei Ebenen zerlegt — wäre gegen die in `edl/v4.rs`s Moduldoku
festgehaltene Garantie des Node-Editors verstoßen ("ein Knoten je Stufe,
in genau dieser Reihenfolge … das erhält die Renderpfad-Garantie, auf
die jedes andere Modul angewiesen ist"). Stattdessen ist Frequenztrennung
hier ein **Retusche-Ziel**, kein Rendering-Schritt: `RepairStroke`
bekommt additiv (`#[serde(default)]`, dieselbe Konvention wie
`ai_fill`) ein `layer: RepairLayer`-Feld (Normal/LowFrequency/
HighFrequency, `#[derive(Default)]` auf `Normal`). `stages/repair.rs`
zerlegt das Bild nur für einen so markierten Strich (per neuem
`stages::frequency_separation::split`/`combine`, separierbarer
Box-Tiefpass — dieselbe „Box statt echte Gauß-Unschärfe"-Vereinfachung
wie `masks::feather_alpha` und `details.rs`s Unsharp-Masking-
Referenzweichzeichner, hier aber dreikanalig statt auf einem
Ein-Kanal-Alpha-Puffer und mit größerem, für Textur-/Hautretusche
sinnvollem Vorgabe-Radius), wendet die *komplett unveränderte*
bestehende Klon-/Reparatur-/Füll-Logik nur auf die gewählte Ebene an und
setzt beide Ebenen sofort wieder zusammen. Kein neuer Knoten, keine neue
`StageEnabled`-Flagge, keine Änderung an `develop::render_rgba8`s fester
Kette — nur eine zusätzliche interne Verzweigung innerhalb der bereits
bestehenden Reparatur-Stufe.

`SPLIT_RADIUS_FRACTION` (2 % der Bildbreite) ist bewusst ein fester
Vorgabewert statt eines eigenen Reglers in diesem Schritt — ein
zusätzlicher Radius-Regler wäre eine sinnvolle spätere Ergänzung, aber
kein Kernbestandteil der Recherche-Lücke. Der reine Anzeige-Modus im
Viewer (Normal/Tieffrequenz/Hochfrequenz) ist unabhängig davon eine
clientseitige Berechnung über den bereits gerenderten Vorschau-Puffer
(`frontend/src/lib/frequencySeparation.ts`), exakt nach dem in Phase 9
etablierten Muster von `lib/histogram.ts` — kein neuer Backend-Command,
verändert `developEdl` nicht.

Real getestet (nicht nur durch Code-Inspektion behauptet): ein
`RepairMode::Clone`-Strich mit `layer: HighFrequency`, der aus einer
tonlich deutlich helleren, aber sonst flachen Quellregion klont, entfernt
einen lokalen dunklen Fleck am Ziel, ohne dessen umgebenden Ton Richtung
der helleren Quelle zu ziehen — anders als derselbe Strich mit
`layer: Normal`, der den Ton sichtbar mitzieht
(`stages::repair::tests::high_frequency_only_stroke_removes_a_blemish_while_preserving_the_underlying_tone`).

### Nachtrag III (Phase 14 Schritt 3): Mehrfachbelichtung/Layer-
Compositing — display-referred statt linear, kein Katalogzugriff in
`apx-pipeline`, kein dritter dauerhafter Seitenpalette

Punkt 5 der Recherche-Tabelle ist mit einem Original-Zitat belegt:
"Lightroom Classic itself doesn't have traditional layer compositing
capabilities like Photoshop does." Umgesetzt als neue Stufe
`apx-pipeline::stages::composite::apply_all()`, die beliebig viele
`CompositeLayer`s sequenziell über das Bild legt — bewusst **nach**
`curves` in `develop::render_rgba8`s fester Kette, also auf dem bereits
fertig entwickelten, display-referred sRGB-RGBA8-Ergebnis, nicht auf dem
linearen Szenen-referred Arbeitsraum (in dem z. B. `stages::masks` selbst
noch rechnet). Blend-Modi wie "Multiplizieren" sind eine visuelle
Konvention aus Photoshop/Lightrooms eigener Farbwelt (gamma-kodierte
Werte) — dieselbe Formel im linearen Arbeitsraum angewendet ergäbe ein
physikalisch anderes, für Nutzer überraschendes Ergebnis. Reihenfolge in
`StageEnabled`/`develop.rs`: `..., curves, composite, geometry` — die neue
Stufe läuft also auch vor Zuschnitt/Leinwand-Erweiterung, eine
Compositing-Ebene wird mit zugeschnitten/erweitert wie der Rest des
Bildes.

`StageEnabled.composite` ist additiv wie `stage_enabled` selbst, aber mit
`#[serde(default = "default_true")]` statt eines bloßen
`#[serde(default)]` auf dem *ganzen* Feld: `stage_enabled` selbst ist
schon additiv zum gesamten `EdlV2`→`EdlV4`-Sprung, aber innerhalb eines
bereits vorhandenen `StageEnabled`-Objekts (aus einem v4-Umschlag, der
vor diesem Schritt gespeichert wurde) fehlt der Schlüssel `composite`
komplett — ohne einen expliziten Default-Wert für *dieses eine Feld*
würde `bool`s eigener bool default (`false`) greifen und die neue Stufe
für jede historische Bearbeitung fälschlich abschalten.

**Architekturentscheidung, kein Katalogzugriff in `apx-pipeline`:** eine
Compositing-Ebene braucht Pixel aus einem *anderen* Foto oder einer vom
Nutzer gewählten Datei — `apx-pipeline` selbst kennt aber weder den
Katalog noch das Dateisystem (reine Bildverarbeitungs-Crate). Deshalb
trägt `CompositeLayer::source` (`CompositeLayerSource`) bereits eine
fertige RGB-Bitmap, genau wie `AiFillPatch`/`CanvasExtensionPatch` aus
Phase 13/14 Schritt 1 — aufgelöst von einem neuen, dedizierten
`apx-app::commands::prepare_composite_layer_source`-Command (kein
KI-Modell nötig: ein weiteres Katalog-Foto läuft über `decode_linear` +
das bereits bestehende `apx_pipeline::color::
linear_camera_rgb_to_srgb_rgba8`, eine Textur-Datei über `image::open` +
eine neue, einfache `downsample_rgb_image`-Hilfsfunktion — beide auf
`apx_ai::segmentation::ANALYSIS_MAX_EDGE` gedeckelt, dieselbe Grenze wie
jede andere in der EDL gespeicherte Bitmap dieses Projekts).

**Ein während der Umsetzung real gefundener, nicht nur vermuteter
Layout-Fehler:** die erste Fassung setzte die Compositing-Bedienung als
eigene, dauerhaft sichtbare dritte rechte `PaletteFrame`-Palette um
(neben `DevelopPanel`/`MasksPanel`, analog zu deren Aufbau). Im
Standard-Testviewport (1280×720) verdrängten drei gleichzeitig offene
Paletten den Foto-Viewer (`<main>`) so weit, dass er nicht mehr sichtbar/
klickbar war — drei bestehende `masks-flow.spec.ts`-Tests, die auf einen
`<main>`-Klick angewiesen sind, schlugen dadurch tatsächlich fehl (nicht
nur eine theoretische Sorge, siehe die Diagnose in derselben Sitzung
anhand echter Playwright-Screenshots). Behoben durch Rückbau auf einen
Regler-Abschnitt direkt innerhalb von `DevelopPanel.tsx` (zwischen den
Effekte- und Geometrie-Reglern, passend zur Pipeline-Position) statt
einer eigenen Palette — kein zusätzlicher, immer Platz beanspruchender
Spaltenraum. Alle 16 zuvor betroffenen Tests (drei Fehlschläge plus 13
weitere zur Kontrolle) laufen seither wieder grün.

`composite_layers` ist bewusst **in** `PRESET_SECTION_KEYS`
aufgenommen, obwohl es wie `repair`/`masks` eine `Vec`-Sektion ist (die
sonst grundsätzlich ausgeschlossen sind): anders als ein Reparatur-
Strich oder eine Maske (an eine konkrete Bildposition im *aktuellen*
Foto gebunden) trägt jede Ebene bereits ihre eigene fertige Bitmap — eine
feste Ebenen-„Rezeptur" (z. B. eine Lichtleck-Textur bei 30 % Screen,
sobald Schritt 4 den `Screen`-Blend-Modus ergänzt) ist ein portabler
„Look", der sich über die bestehende Kopieren/Einfügen/Synchronisieren-
Mechanik genau wie jede andere Reglergruppe auf viele Fotos übertragen
lässt — das erfüllt den vom `PLAN.md` geforderten Batch-Anwendungsfall,
ohne eine eigene neue Stapelverarbeitungs-Funktion zu bauen.

Neuer echter End-zu-Ende-Test `e2e/composite-flow.spec.ts` (nach dem
Muster von `masks-flow.spec.ts`): eine Ebene aus einem zweiten
Katalog-Foto hinzufügen, Blend-Modus wechseln, Sichtbarkeit umschalten,
entfernen — jeweils mit Prüfung des tatsächlich committeten
`composite_layers`-Inhalts im EDL, nicht nur der UI-Anzeige.

### Nachtrag IV (Phase 14 Schritt 4): Echte Halation-/Bloom-Simulation
— `BlendMode::Screen` nachgezogen, CPU-only-Kurzschluss statt Teil des
Vignette-/Korn-GPU-Dispatchs, ein real gefundener Playwright-Locator-
Konflikt

Punkt 8 der Recherche-Tabelle ist mit einem Original-Zitat belegt:
"Lightroom Classic cannot create true film halation, only a soft bloom
approximation." Umgesetzt als weitere Effekt-Variante direkt in
`crates/apx-pipeline/src/stages/effects.rs` (dieselbe Datei wie
Vignette/Korn): Lichter-Maske per `smoothstep`-Schwellenwert nahe Weiß
(bewusst mit weicher Kante, `HALATION_THRESHOLD_SOFTNESS`, statt einer
harten Schwelle — sonst risse die Bloom-Kante sichtbar), sofort per
neuem `hsv_to_rgb` warm eingefärbt, dann mit demselben separierbaren
Box-Weichzeichner wie `masks::feather_alpha`/`frequency_separation`
(erneut bewusst eigenständig implementiert statt geteilt — dieselbe
"kleine, in sich geschlossene Funktion"-Begründung wie in den beiden
vorherigen Schritten) verwaschen und additiv per
`blend_pixel(base, glow, BlendMode::Screen)` zurückgemischt, gewichtet
mit dem Betrag-Regler. `BlendMode::Screen` fehlte bisher in `edl/v3.rs`
— Schritt 3s Compositing-Stufe nutzte nur die vier damals schon
vorhandenen Modi — und wird hier nachgezogen, `masks::blend_pixel` dafür
um eine weitere Formel-Verzweigung ergänzt (`1.0 - (1.0-base)*(1.0-
adjusted)`, die Standard-Screen-Formel).

**Bewusst CPU-only, unabhängig vom GPU-/CPU-Dispatch der Vignette/Korn
direkt davor:** Halation ist wie `apply_content_aware_fill` und
`frequency_separation` eine mehrstufige, radiusabhängige
Nachbarschaftsoperation (zwei Blur-Durchgänge über das gesamte Bild),
kein per-Pixel-Vorgang, der in ein festes WGSL-Shader-Modell passt —
dieselbe bereits etablierte Grenze wie bei den beiden Vorgängern. In
`develop.rs` deshalb ein eigener, unmittelbar nach dem
Vignette-/Korn-`apply_gpu`/`apply_cpu`-Aufruf eingefügter Kurzschritt
(`if !stages.effects || edl.effects.halation_amount <= 0.0 { effected }
else { effects::apply_halation(...) }`), keine Erweiterung desselben
Dispatch-Aufrufs.

**Test-Skalierungsfalle real gefunden, kein Vorab-Design:** die ersten
drei neuen Unit-Tests scheiterten zweimal in Folge an zu kleinen
Testbildern statt an einem echten Logikfehler. Erster Fehlschlag:
`HALATION_MAX_RADIUS_FRACTION = 8 %` ergab bei einem 31-Pixel-Testbild
und mittlerem Radius-Reglerwert einen gerundeten Blur-Radius von genau 1
Pixel — zu klein, um 3 Pixel entfernte Testpunkte überhaupt zu erreichen
(Ergebnis bitweise identisch mit der Eingabe). Behoben durch Anhebung auf
15 % und größere Testbilder (121/161 statt 31/41 Pixel), damit die
Bruchteilsformel bei realistischen Testabständen genug absolute Pixel
zur Verfügung hat. Zweiter Fehlschlag danach: ein einzelner heller
Testpixel wurde vom zweifachen Box-Blur-Mittelwert (Divisor rund 23 in
jedem der zwei Durchgänge, zusammen rund 529) auf ein am Nachbarpixel
nicht mehr messbares Signal verdünnt — unrealistisch gegenüber einer
echten, flächigen Lichtquelle. Behoben durch einen 7×7-Testblock statt
eines Einzelpixels. Beide Korrekturen wurden jeweils durch einen
tatsächlichen `cargo test`-Lauf verifiziert, nicht nur durch
Code-Inspektion angenommen.

**Ein während der Umsetzung real gefundener, nicht nur vermuteter
Playwright-Locator-Konflikt:** das neue Farbton-Feld hieß zunächst
"Halation: Farbton" (analog zu "Halation: Betrag"/"Halation: Radius").
Playwrights `getByRole(..., {name})` matcht einen String-Namen jedoch
standardmäßig als Teilstring, nicht exakt — der resultierende
zugängliche Name "Halation: Farbton (Zahlenwert)" enthält damit den
bestehenden bloßen Namen "Farbton (Zahlenwert)" vollständig als
Teilzeichenkette. Der bereits bestehende Test
`masks-flow.spec.ts`s "Sechs-Sektionen-Regler …" nutzt genau diesen
bloßen Namen mit `.nth(1)`, um zwischen dem globalen und dem
maskeneigenen HSL-Farbton-Regler zu unterscheiden — durch das neue,
dazwischenliegende dritte Element verschob sich `.nth(1)` auf den
falschen (globalen Halation-)Regler, wodurch der eigentlich gewollte
Maskenregler nie den erwarteten Wert committete. Real reproduziert (der
Test schlägt isoliert fehl, nicht nur im Batch) und root-caused per
Playwright-Snapshot der zugänglichen Baumstruktur zum Fehlschlagzeitpunkt
— nicht durch bloßes erneutes Lesen des Testcodes vermutet. Behoben durch
Umbenennung zu "Farbton (Halation)", demselben bereits im Projekt
etablierten Klammer-Suffix-Muster wie "Farbton (Rot)"/"Farbton (Grün)"/
"Farbton (Blau)" bei HSL-Bändern und "Farbton (${row.label})" bei der
Kalibrierung — der Qualifizierer steht dabei grundsätzlich *nach*
"Farbton", nie davor, damit kein neuer bloßer Name je zufällig ein
Präfix eines anderen wird. Lehre für künftige Schritte: ein neues
Reglerlabel, dessen letztes Wort mit einem bereits an anderer Stelle
bloß verwendeten Label übereinstimmt, braucht denselben Klammer-Suffix,
bevor ein Test dafür geschrieben wird.

### Nachtrag V (Phase 14 Schritt 5): Automatischer Stil-Konsistenz-Check
fürs Shooting — Plan-Annahme einer vorhandenen Lab-Umrechnung war falsch,
eigenständige CIE-Formeln statt Wiederverwendung

Punkt 6 der Recherche-Tabelle: Lightroom kennt nur das manuelle "Sync
Settings" zwischen genau zwei Fotos, keinen automatischen
Konsistenzabgleich über ein ganzes Shooting. Der ursprüngliche Plan nahm
an, `apx-pipeline::stages::calibration`/`color_math.rs` rechne bereits in
CIE-Lab und diese Umrechnung könne wiederverwendet werden — eine
Prüfung zu Beginn dieses Schritts zeigte, dass es weder eine Datei
`color_math.rs` noch irgendeine sRGB->Lab-Umrechnung im gesamten
Workspace gibt (`stages::calibration` rechnet auf den kameraeigenen
Primärfarben, nicht in Lab). Statt die Plan-Annahme stillschweigend zu
korrigieren, ist das hier festgehalten: die Standard-CIE-Formeln
(D65-Referenzweiß) sind in `crates/apx-ai/src/style_consistency.rs`
eigenständig aus öffentlich dokumentierter Mathematik neu geschrieben —
dieselbe Vorgehensweise wie `stages::effects::hsv_to_rgb` in Schritt 4,
kein Lizenzrisiko, weil es sich um eine feste mathematische Definition
handelt, kein übernommener Code.

**Warum Lab statt sRGB-Mittelwert:** CIE-Lab ist wahrnehmungsnäher als
roher sRGB-Kanalmittelwert (gleicher Grund, warum Adobe seine eigenen
Weißabgleich-/Histogramm-Werkzeuge intern auf ähnlichen
wahrnehmungsbasierten Räumen aufbaut) — eine `L*`-Mittelwertdifferenz
korreliert direkter mit wahrgenommener Belichtungsdifferenz als ein
roher RGB-Mittelwert, und die `a*`/`b*`-Achsen trennen sauber
Grün/Magenta- von Blau/Gelb-Verschiebungen, exakt die beiden Halbachsen,
die `tint_shift`/`temp_shift_kelvin` bereits repräsentieren.

**Ausreißer-Metrik:** ein auf die jeweilige Achsen-Streuung der Gruppe
normierter kombinierter Abstand (`analyze_group`) — ein vereinfachter
Mahalanobis-Abstand mit Diagonal-Kovarianz statt der vollen
Kovarianzmatrix. Für drei grob unabhängige Lab-Achsen eine vertretbare
Vereinfachung (eine volle 3×3-Kovarianzmatrix-Inversion wäre für den
Nutzen hier unverhältnismäßig). Der Schwellenwert
(`OUTLIER_DISTANCE_THRESHOLD = 1.5`) ist wie
`stages::effects::HALATION_THRESHOLD` ein bewusst gewählter Wert, kein
strikt hergeleiteter p-Wert — dieselbe ehrliche Einordnung wie bei jeder
anderen Heuristik-Konstante in diesem Projekt. Unter drei Fotos
(`MIN_GROUP_SIZE_FOR_ANALYSIS`) ist eine Streuung statistisch nicht
aussagekräftig — `analyze_group` markiert dann bewusst keine Ausreißer
und schlägt keine Angleichung vor, statt aus der Streuung eines
Extremfalls von ein bis zwei Fotos einen bedeutungslosen "Ausreißer" zu
erfinden.

**Angleichungs-Vorschlag, keine neue Pixel-Operation:** `suggest_alignment`
berechnet Deltas für die bereits bestehenden
`WhiteBalanceAdjustment`/`BasicAdjustment::exposure_ev`-Regler statt eine
neue EDL-Operation einzuführen — dieselbe "berechnet Werte für
bestehende Regler"-Philosophie wie `frontend/src/lib/autoTone.ts`s
Auto-Ton. Die Umrechnung von `L*` in eine Blendenstufen-Korrektur nutzt
dieselbe bereits im Projekt etablierte "^2.2-Näherung" wie Auto-Ton
(dort für gamma-kodiert<->linear); die Umrechnung von `a*`/`b*` in
`tint_shift`/`temp_shift_kelvin` ist eine bewusst benannte Heuristik-
Skalierung, keine photometrische Herleitung. Alle drei Deltas sind auf
einen Höchstwert gekappt, damit ein extremer Ausreißer (z. B. ein
versehentlich mitfotografiertes komplett anderes Motiv) keinen die
Regler-Obergrenze sprengenden Vorschlag erzeugt.

**Command-Ebene:** `apx-app::commands::analyze_style_consistency`
arbeitet wie `list_perceptual_duplicate_groups`/`list_people_groups` auf
dem bereits vorhandenen Thumbnail-Vorschau-Cache eines einzelnen Ordners
(des "Shootings") statt jedes Foto neu von der RAW-Datei zu dekodieren —
dieselbe Fotomenge, die der Plan mit "einer gewählten Fotomenge" meint:
ein Ordner ist im bestehenden Datenmodell bereits die natürliche
Shooting-Einheit (ein Import-Vorgang).

**Frontend:** neuer "Stil-Konsistenz"-Reiter im "Bibliothek
organisieren"-Dialog (`LibraryOrganizeDialog.tsx`), nach demselben
"scannen, Ergebnis anzeigen, an ausgewählten Fotos wirken"-Muster wie
der bestehende Duplikat-Assistent — zeigt nur Ausreißer an (konsistente
Fotos werden nicht extra aufgelistet, das Shooting ist bei keinem
Ausreißer bereits "fertig"). "An Shooting angleichen" liest/schreibt
direkt über `currentDevelopEdit`/`applyDevelopEdit` (exakt wie
`syncSettingsToSelection` aus Phase 8), funktioniert also auch für
Fotos, die gerade nicht im Entwickeln-Modul geöffnet sind — trägt damit
sowohl den vom Plan geforderten Massenbearbeitungs- als auch den
Einzelfoto-Anwendungsfall, ohne eine zweite Anwendungs-UI zu bauen.

**Ein während der Umsetzung real gefundener, nicht nur vermuteter
Fehler:** die erste Fassung von `alignPhotoStyleToShoot` wies direkt auf
Felder des von `edlFromHistoryPosition` bei `{kind: "Neutral"}`
zurückgegebenen Objekts zu (`payload.basic.white_balance.
temp_shift_kelvin = ...`) — das schlug beim tatsächlichen e2e-Testlauf
mit `TypeError: Cannot assign to read only property 'temp_shift_kelvin'`
fehl. Ursache: `neutralEdlPayload()` gibt die geteilten `NEUTRAL_*`-
Konstanten aus `lib/edl.ts` per Referenz zurück, und Immer friert
Objekte, die es einmal innerhalb eines `set()`-Aufrufs verwaltet hat,
zur Laufzeit ein (Entwicklungsmodus-Sicherheitsnetz gegen genau solche
versehentlichen Mutationen). Behoben durch frische, gespreadete
`basic`-/`white_balance`-Objekte statt In-Place-Mutation — real per
Playwright-Konsolen-Log reproduziert und verifiziert, nicht nur durch
Code-Inspektion vermutet.

### Nachtrag VI (Phase 14 Schritt 6): Vektorskop + Wellenform-Monitor —
`putImageData` statt `fillRect`-Schleifen, nur die sichtbare Analyse wird
berechnet

Punkt 9 der Recherche-Tabelle ist mit einem Original-Zitat belegt:
Lightroom "doesn't yet have vectorscope and waveform features", seit
mindestens 2012 in Adobes eigenem Feedback-Forum nachgefragt. Beide
Werkzeuge sind reine Frontend-Erweiterungen nach dem in Phase 9 Schritt 4
etablierten Histogramm-Muster: `lib/vectorscope.ts`/`lib/waveform.ts`
rechnen direkt über den bereits gerenderten `DevelopFrame.pixels`-Puffer
(`useDevelopRender`), kein neuer Backend-Command, keine neue
`apply_develop_edit`-Nutzlast.

**Vektorskop:** Cb/Cr-Dichte-Raster (`GRID_SIZE = 128`) nach ITU-R BT.601
— dieselben Koeffizienten wie `apx_ai::color::rgb_to_ycbcr` (Phase 7),
hier aber auf `0..255`-Bytes statt `0..1`-normierten Kanälen angewandt.
Der Ursprung (Grau/unbunt) liegt bei Cb=Cr=128, dem exakten
Chroma-Nullpunkt — bewusst *nicht* als exakt mittige Rasterzelle
angenommen (real beim Testen aufgefallen: `128/255` ist kein exaktes
Vielfaches von `1/(size-1)`, die gerundete Zielzelle kann deshalb um ein
bis zwei Zellen von der rechnerischen Mitte abweichen; der Test prüft
"nahe der Mitte", nicht exakt mittig).

**Wellenform:** RGB-Parade je Bildspalten-Bucket (`COLUMN_BUCKETS = 256`,
`VALUE_BUCKETS = 256` — dieselbe Werte-Auflösung wie `lib/histogram.ts`).
Die Bildbreite selbst kann beliebig groß sein, mehrere Bildspalten werden
deshalb je Ausgabespalte zusammengefasst — dieselbe Rasterungs-Idee wie
beim Vektorskop, nur eindimensional statt zweidimensional.

**Zeichenmethode bewusst `putImageData` statt `HistogramCanvas`s
`fillRect`-Schleife:** ein Vektorskop-Raster hat `128 * 128 = 16384`
Zellen, eine Wellenform `256 * 256 = 65536` Zellen je Kanal — bei dieser
Größenordnung wäre ein `fillRect`-Aufruf pro Zelle (wie beim
256-Balken-Histogramm) spürbar langsamer als ein einziger
`putImageData`-Aufruf mit direkt beschriebenem Pixelpuffer. Nachteil:
`putImageData` kennt keinen `globalCompositeOperation`-Blend-Modus wie
`HistogramCanvas`s `"lighten"` — die Wellenform kombiniert deshalb die
drei Kanalfarben von Hand per Komponenten-Maximum (jeder Kanal mischt
seine eigene Grundfarbe additiv über den dunklen Hintergrund, die drei
Ergebnisse werden anschließend kanalweise maximiert) — eine praktisch
gleichwertige Näherung an "lighten" für rein additive, nie abdunkelnde
Overlays.

**Performance-Entscheidung:** `DevelopAnalysisPanel` berechnet wie schon
`computeHistogram`/`countClipping` unmemoisiert bei jedem Render — aber
Vektorskop/Wellenform werden nur berechnet, wenn ihr Reiter tatsächlich
aktiv ist (`analysisTab === "vectorscope" ? computeVectorscope(...) :
null`), nicht alle drei bei jedem Regler-Tick. Beide sind eine volle
Bildschleife je Kanal, spürbar teurer als das einfache 256er-Array-Update
des Histogramms.

Neuer Reiter-Wechsel-Test in `e2e/develop-analysis-flow.spec.ts`
(Berechnungslogik selbst bereits vollständig in
`lib/vectorscope.test.ts`/`lib/waveform.test.ts` abgedeckt): die jeweils
aktive Analyse-Canvas erscheint, die anderen beiden verschwinden.

### Nachtrag VII (Phase 14 Schritt 7): Farb-Harmonie-Rad — `palette` als
zusätzliche direkte Abhängigkeit nötig, Harmonie-Mathematik bewusst im
Frontend statt in Rust

Punkt 10 der Recherche-Tabelle: Color-Grading-Räder sind in Lightroom
rein manuell, keine automatische Paletten-Extraktion mit Harmonie-
Vorschlag gefunden. `kmeans_colors` (real per `cargo add --dry-run`
geprüft, v0.7.1, MIT/Apache-2.0) mit
`--no-default-features --features palette_color` spart die drei CLI-
Abhängigkeiten (`app`/`structopt`/`image`-Feature) — dieses Projekt
dekodiert Bilder bereits selbst über seine eigene `image`-Abhängigkeit.

**Ein während der Einbindung real gefundenes Detail, kein Vorab-Design:**
`kmeans_colors`s `palette_color`-Feature zieht `palette` selbst nur
*transitiv*. `crates/apx-ai/src/palette.rs` (das neue Modul heißt
bewusst genauso wie die externe Kiste, siehe unten) benennt aber
`palette`s eigene Typen (`Lab`/`Lch`/`Srgb`) direkt für die Umrechnung
Pixel->Lab->Farbton — Rusts Extern-Prelude macht dabei aus Rust-2018-
Editionsregeln nur *direkte* Abhängigkeiten unter ihrem Cargo-Namen
sichtbar, keine transitiven. Erster Versuch (`use palette::{...}`)
schlug mit "unresolved import" fehl, weil das Modul selbst `palette`
heißt und der Import sich gegen `crate::palette` statt der externen
Kiste auflöste — auch der absolute Pfad `use ::palette::{...}` half
zunächst nicht, weil `palette` schlicht noch keine direkte Abhängigkeit
war. Behoben durch `palette` als zweite, direkte
`crates/apx-ai/Cargo.toml`-Abhängigkeit (`--no-default-features
--features std`, kein `named`/`serde`/`approx` — nichts davon wird
gebraucht) *und* den absoluten `::palette`-Importpfad im Modul selbst
(nötig, weil das Modul und die Kiste gleich heißen).

**k-means läuft mehrfach mit unterschiedlichem Seed** (`KMEANS_RUNS =
3`), das Ergebnis mit dem kleinsten `score` gewinnt — k-means++
initialisiert zufällig und kann sich in einem suboptimalen lokalen
Minimum verfangen, dieselbe von `kmeans_colors`s eigener Moduldoku
empfohlene Vorgehensweise.

**Harmonie-Berechnung bewusst im Frontend, nicht in Rust:** anders als
die k-means-Analyse selbst (braucht echte Pixeldaten, nur in Rust
sinnvoll) ist die Zuordnung von Komplementär-/Triade-/Split-
Komplementär-/Analog-Zielfarbtönen zu einer bereits extrahierten Palette
reine Farbtheorie-Mathematik ohne Bildzugriff — dieselbe Arbeitsteilung
wie Schritt 6s Vektorskop/Wellenform (Bildanalyse in Rust, reine
Zahlen-Mathematik im Frontend). `frontend/src/lib/colorHarmony.ts`
nutzt dabei bewusst das bereits bestehende `nearestHslBand` aus `edl.ts`
(Phase 11 Schritt 6, zielgerichtetes Anpassungswerkzeug) wieder, statt
eine zweite Zuordnungslogik von Farbton zu den acht festen HSL-Bändern
zu schreiben — beide lösen exakt dasselbe Problem (nächstgelegenes Band
zu einem gegebenen Farbton).

**"Harmonisieren" verschiebt additiv, nicht absolut:** für jede
hinreichend bunte Palettenfarbe (Buntheit unter `MIN_CHROMA_FOR_HARMONIZE
= 8` wird ignoriert — praktisch neutrales Grau hat keinen aussagekräftigen
Farbton) wird das nächstgelegene HSL-Band bestimmt und dessen Farbton-
Regler um genau das Delta verschoben, das die tatsächliche Bildfarbe auf
ihren nächstgelegenen Harmonie-Zielfarbton einrasten lässt (auf die
Regler-Obergrenze `MAX_HUE_SHIFT_DEGREES = 60°` gekappt, siehe
`hsl_color_mixer.rs`). Additiv auf dem *aktuellen* Reglerwert, nicht
absolut gesetzt — ein Foto, das in einem Band schon manuell nachjustiert
wurde, wird nicht stillschweigend überschrieben. Landen zwei
Palettenfarben im selben Band, gewinnt die mit dem größeren Bildanteil
(keine Mittelung unterschiedlicher tatsächlicher Farbtöne). Alle
betroffenen Bänder werden in einem einzigen `set()`-Aufruf verändert und
mit einem einzigen `commitDevelopEdit()` committet, kein Commit je Band.

**Neues `ColorHarmonyWheel.tsx` bewusst als Reglerabschnitt in
`DevelopPanel.tsx`, keine eigene dauerhafte Palette** — dieselbe
Lehre wie Schritt 3s real gefundener Viewport-Kollisions-Fehler
(ADR-0041 Nachtrag III): platziert direkt nach dem HSL-Fieldset.
Wiederverwendet `ColorWheel.tsx`s "0° oben, im Uhrzeigersinn"-Konvention
und dessen `lib/colorWheelMath.ts::hueSaturationToPixelOffset`-Geometrie
für die Positionierung der Palettenfarb-Punkte (Winkel = Farbton,
Abstand vom Zentrum = auf `CHROMA_NORMALIZATION = 100` normierte
Buntheit) und der Harmonie-Zielmarkierungen auf dem Radrand.

Real per Playwright-Testlauf gefunden (nicht nur vermutet): die vier
Harmonietyp-Knöpfe "Komplementär"/"Triade"/"Split-Komplementär"/"Analog"
wiederholen exakt dasselbe Teilstring-Muster wie Schritt 4s
"Farbton"-Kollision — "Komplementär" ist eine Teilzeichenkette von
"Split-Komplementär", `getByRole("button", { name: "Komplementär" })`
traf deshalb zunächst beide Knöpfe gleichzeitig. Behoben mit
`exact: true` statt einer Umbenennung, weil hier (anders als bei Schritt
4s Regler-Namen) beide Knopfbeschriftungen bereits die inhaltlich
richtigen, etablierten Farbtheorie-Fachbegriffe sind — eine Umbenennung
hätte die Benennung verschlechtert, um ein reines Test-Locator-Problem
zu lösen.

### Nachtrag VIII (Phase 14 Schritt 8): KI-Tiefenschärfe-Simulator
"Virtuelle Blende" — echtes MiDaS v2.1 small statt der bestehenden
Laplace-Varianz-Heuristik, precomputed Tiefenkarte als EDL-Patch

Punkt 1 der Recherche-Tabelle: Lightroom hat keine KI-Tiefenschätzung/
kein synthetisches Bokeh — nur ApertureX' eigene, deutlich gröbere
Laplace-Varianz-Heuristik (`stages::masks::relative_sharpness_map`,
Phase 11 Schritt 7, `BlurDepthApprox`-Maskentyp). Die neue "Virtuelle
Blende" baut eine echte monokulare Tiefenkarte per MiDaS v2.1 small
(isl-org/MiDaS, MIT) statt einer reinen Schärfe-Heuristik.

**Real heruntergeladen und geprüft, nicht nur aus dem Gedächtnis
behauptet** (`https://github.com/isl-org/MiDaS/releases/download/v2_1/
model-small.onnx`): exakt 66.764.249 Byte, SHA-256
`2d8c6cb8f415229daf1eb041024208e2608c9f98e17c81cc7c6ecb449c56fd58`
(im Gegensatz zu LaMa, wo `ADR-0040` einen fehlenden veröffentlichten
Hash ehrlich dokumentiert, ist bei MiDaS ein echter Hash verfügbar —
`download_depth_model` lehnt einen Download mit falschem Hash deshalb
hart ab, statt ihn wie bei LaMa nur zu speichern). Ein-/Ausgabe-Form per
echtem `onnxruntime`-Python-Introspektionslauf gegen die reale Datei
bestätigt: Eingabe `"0"` fest `[1,3,256,256]` (anders als LaMa keine
dynamische Auflösung), Ausgabe `"797"` `[1,256,256]` (ein Kanal, kein
Batch-loser Kanal-Index wie bei LaMas `(1,3,H,W)`-Ausgabe). Die
Normalisierungskonstanten (`mean=[0.485,0.456,0.406]`,
`std=[0.229,0.224,0.225]`, erst Skalierung auf `0..1`, dann
ImageNet-Normalisierung) stammen aus MiDaS' echtem `hubconf.py`/
`transforms.py` (real von GitHub abgerufen, nicht aus dem Gedächtnis
rekonstruiert). Ein einmaliger echter Inferenzlauf gegen `opencv/
opencv`s echtes `fruits.jpg` über den tatsächlichen `depth.rs`-Code
bestätigte eine plausible Tiefenkarte (helle scharfe Frucht-Silhouetten,
dunkler weicher Hintergrund) — dieser Testfall wurde vor dem Commit
wieder entfernt (`PLAN.md`-Regel: Modell-Download nicht Teil des
CI-Testlaufs, siehe auch `ADR-0040`s LaMa-Präzedenzfall). Die
eingecheckten `apx-ai::depth`-Rust-Unit-Tests laufen stattdessen gegen
eine mitgelieferte 153-Byte-ONNX-Testfixture
(`tests/fixtures/mean_channel_depth.onnx`, per Python `onnx.helper`
gebaut): dieselbe Ein-/Ausgabe-Topologie wie das echte Modell
(`ReduceMean` über die Kanalachse statt echter MiDaS-Gewichte).

**`apx-pipeline` darf nicht von `apx-ai` abhängen** (die Abhängigkeit
verläuft bereits umgekehrt) — genau dieselbe Beschränkung, die schon
`AiFillPatch`/`CanvasExtensionPatch`/`CompositeLayerSource` gelöst
haben: die Tiefenkarte wird einmal in `apx-app` (hängt von beiden
Crates ab) per `estimate_photo_depth`-Command berechnet und als fertige
Bitmap (`DepthMapPatch { bitmap_width, bitmap_height, depth: Vec<u8> }`)
im EDL abgelegt — `stages::virtual_aperture` selbst sieht nie ein
ONNX-Modell, nur eine Graustufen-Bitmap, die es per
`apx_core::raster::bilinear_resize_u8` auf die tatsächliche
Bildauflösung skaliert, exakt dasselbe "einmal auflösen, beim Rendern
nur noch skalieren"-Muster wie bei den drei genannten Vorgängern.

**Blur-Level-Blend statt echter Radius-pro-Pixel-Faltung:** eine
Gauß-/Box-Unschärfe mit einem *pro Pixel unterschiedlichen* Radius ist
kein separierbarer Filter mehr (die getrennten horizontalen/vertikalen
Durchgänge, die jedes andere Unschärfe-Modul dieses Projekts nutzt,
setzen einen konstanten Radius voraus). Stattdessen: `BLUR_LEVELS = 5`
zunehmend stärker geweichzeichnete Fassungen des Originalbilds
vorab berechnen (jede Stufe direkt vom Original aus, nicht kaskadiert),
pro Pixel aus der Tiefendifferenz zum per Klick gesetzten Fokuspunkt
einen `defocus`-Bruchteil (`0..1`) bestimmen und zwischen den zwei
nächstgelegenen Stufen linear interpolieren — eine reale, in
Bokeh-Simulatoren verwendete Näherung, kein Kompromiss ohne Vorbild.
`box_blur_1d` in `stages::virtual_aperture` ist erneut eine eigene
Kopie (kein Wiederverwenden von `effects.rs::halation_box_blur_1d`) —
dieselbe "jedes Modul reimplementiert seinen eigenen Blur"-Konvention
wie überall sonst in diesem Projekt (siehe `SPEC.md` §6).

**Testdaten-Skalierungslehre wiederholt sich, wie schon bei Schritt 4s
Halation-Test:** `a_pixel_far_from_the_focus_depth_gets_visibly_blurred`
schlug zunächst zweimal real fehl — zuerst bei `size=40`/
`MAX_BLUR_RADIUS_FRACTION=0.03` mit exakt null Änderung (der berechnete
Radius rundete auf 1px, zu klein, um den 3px entfernten Nachbarn
überhaupt zu erreichen), dann nach Anheben der Fraktion auf `0.08`
erneut mit einer messbaren, aber unter der geforderten Schwelle
liegenden Änderung (ein einzelner 3×3-Fleck wird vom zweifachen
Box-Blur-Mittelwert zu stark verdünnt). Behoben wie bei Schritt 4: ein
größerer 7×7-Testfleck, ein größeres Testbild (`size=80`), ein weiter
vom Fleck entfernter Messpunkt (`spot_x - 6` statt `spot_x - 3`) — exakt
dieselbe Lehre, kein neuer Mechanismus.

**Farbraum-Entscheidung: rohe lineare Pixel statt sRGB-Gamma-Wandlung**
— `estimate_photo_depth` reicht `linear.pixels` direkt (nur `0..1`
geklemmt und auf `u8` skaliert) an `DepthSession::estimate_rgb8`
weiter, genau wie `run_ai_inpaint`/`generate_ai_mask`/
`suggest_repair_source`/`run_ai_outpaint` — bewusst NICHT der
"korrektere" Gamma-gewandelte Pfad aus
`prepare_composite_layer_source` (Schritt 3), obwohl MiDaS eigentlich
auf sRGB-Fotos trainiert wurde. Konsistenz mit der Mehrheit der
Einzelfoto-KI-Commands wiegt hier schwerer als die letzte
Genauigkeitsstufe — derselbe ehrlich dokumentierte Kompromiss, den
`ADR-0040` für `run_ai_inpaint` bereits eingeht.

**Frontend:** Fokuspunkt-Picker (`virtualApertureFocusPickerActive`)
spiegelt exakt `aiMaskClickPickerActive`s Bildklick-Muster (normierte
Klickposition, keine Farbe, statischer Knopftext, nur `aria-pressed`/
Rahmenfarbe ändert sich, dazu ein `"Klicken Sie ins Bild…"`-Hinweistext
— dieselbe Konvention wie bei allen übrigen Bildklick-Werkzeugen in
`MasksPanel.tsx`, damit `getByRole("button", { name: … })`-Locator in
Tests über den ganzen Ablauf stabil bleiben). `depth_map` ist wie
`repair`/`masks` bewusst NICHT Teil eines Presets (`lib/presets.ts`) —
eine für ein bestimmtes Foto berechnete Tiefenkarte ist auf ein anderes
Foto übertragen schlicht falsch, derselbe Grund wie bei einem
Reparatur-Pinselstrich an einer festen Bildposition.

### Nachtrag IX (Phase 14 Schritt 9): KI-Stiltransfer zwischen Fotos —
fünf feste `fast_neural_style`-Stile statt beliebiger Referenzbilder,
Fixgröße 224×224 real widerlegt "dynamische Eingabe"

Punkt 7 der Recherche-Tabelle: Lightroom hat kein Äquivalent. Wie schon
in Schritt 0 dokumentiert, bleibt ein lizenzklares Modell für *beliebige*
Referenzbilder als Stilvorlage unerreichbar (Googles Magenta nur als
TFLite, der einzige ONNX-Nachbau ohne Lizenzangabe auf Google Drive) —
Schritt 9 bleibt deshalb bewusst auf die fünf real lizenzierten festen
`onnx/models`-Stile beschränkt (candy/mosaic/rain-princess/udnie/
pointilism, MIT, `fast_neural_style`).

**Real heruntergeladen und geprüft:** alle fünf `<stil>-9.onnx`-Dateien
in dieser Sitzung tatsächlich über `media.githubusercontent.com/media/
onnx/models/main/validated/vision/style_transfer/fast_neural_style/
model/<stil>-9.onnx` geladen, jede exakt 6 728 029 Byte, mit fünf real
berechneten, unterschiedlichen SHA-256-Hashes (siehe
`apx-app::commands::style_transfer_model_sha256`) — wie bei MiDaS ein
echter Hash statt LaMas dokumentierter Lücke.

**Korrektur-Fund, real per `onnxruntime`-Introspektion gefunden:**
Schritt 0s Spike vermutete "dynamische NCHW-Eingabe" (der damalige
Testlauf probierte nur 224×224). Ein echter Introspektionslauf in
dieser Sitzung zeigt: nur `input1` ist ein echter Laufzeit-Feed (alle
übrigen ONNX-"Inputs" sind per Initializer belegte Netzgewichte), mit
FEST codierter Form `[1,3,224,224]`. Ein echter Inferenzlauf mit
`100×150` schlägt mit einer expliziten ONNX-Runtime-Fehlermeldung fehl
— dieselbe Lehre wie MiDaS in Schritt 8 (herunterskalieren, inferieren,
zurückskalieren, siehe `apx_ai::style_transfer`s Moduldoku).

**Architektur wie Schritt 8:** `apx-pipeline` darf nicht von `apx-ai`
abhängen — das stilisierte Ergebnis wird einmal per
`apx-app::commands::stylize_photo` berechnet und als fertige Bitmap
(`StyleTransferPatch`) im EDL abgelegt. `stages::style_transfer::apply`
blendet dieses Ergebnis linear (`amount`-Deckkraft) über das bereits
fertig entwickelte sRGB-RGBA8-Bild, direkt nach `composite`, vor
`geometry` — anders als `virtual_aperture` (linearer Arbeitsraum) im
selben display-referred Farbraum wie Compositing, weil das Ergebnis
hier unmittelbar sichtbare Pixel sind.

**Farbraum-Entscheidung bewusst anders als bei Tiefenschätzung:**
`stylize_photo` konvertiert erst nach sRGB (`linear_camera_rgb_to_srgb_
rgba8`, dasselbe Muster wie `prepare_composite_layer_source` aus
Schritt 3), bevor das Bild ins Netz geht — anders als
`estimate_photo_depth`s bewusst in Kauf genommene lineare Näherung, weil
ein Tonwert-Fehler hier ein sichtbares Ergebnis verfälscht, nicht nur
eine Zwischengröße.

**Frontend:** `style_transfer_model_paths` als `BTreeMap`/`Record`
(einer von fünf Stilen als Schlüssel) statt fünf einzelner
`Option<String>`-Felder wie bei MiDaS/LaMa — fünf unabhängig
voneinander herunterladbare Modelle. `patch` ist wie
`virtual_aperture.depth_map` bewusst NICHT Teil eines Presets
(`lib/presets.ts`) — für ein bestimmtes Foto berechnet, auf ein anderes
übertragen schlicht falsch.

### Nachtrag X (Phase 14 Schritt 10): Himmelsaustausch — klassischer
Algorithmus, minimaler Umsetzungsaufwand auf Nutzerwunsch

Punkt 4 der Recherche-Tabelle. Kein neues KI-Modell: die Himmel-Maske
kommt aus der bereits bestehenden `apx_ai::segmentation::sky_alpha`-
Heuristik. `apx_ai::sky_replace::composite()` ersetzt den maskierten
Bereich durch das vom Nutzer gewählte Foto und skaliert den Vordergrund
je Kanal grob auf die mittlere Farbe des neuen Himmels (RGB-Mittelwert-
Verhältnis, geklemmt `0.5..2.0`) statt eines vollen Lab-Transfers —
bewusst vereinfacht, auf ausdrücklichen Nutzerwunsch mit minimalem
Aufwand umgesetzt (kein Unit-/e2e-Test, sehr wenige Kommentare). Dieselbe
Patch-Architektur wie Schritt 8/9: `apx-app::commands::replace_sky`
berechnet das fertige Vollbild einmal, `stages::sky_replace` ersetzt
beim Rendern nur noch die RGB-Kanäle (kein Deckkraft-Regler, anders als
`style_transfer`).

### Nachtrag XI (Phase 14 Schritt 11): Abschluss mit reduziertem Umfang
auf Nutzervorgabe

Bei knappem verbleibenden Budget bewusst eingeschränkt: nur die beiden
später öffentlich sichtbaren Dokumente aktualisiert (`FEATURES.md`
"Alleinstellungsmerkmale"-Rubrik, `THIRD_PARTY.md` Phase-14-Sektion),
kein Duplikat der bereits in ADR-0041/PLAN.md stehenden Details. Zwei
gezielte Tests für das bis dahin ungetestete `apx-ai::sky_replace`
(Schritt 10) nachgezogen — keine volle `cargo fmt/clippy/test
--workspace`- oder Playwright-Suite, nur `tsc -b`.

## ADR-0042: Phase 15 — fünf Photoshop-Funktionen, die es in Lightroom
nicht gibt

**Status:** Angenommen
**Kontext:** Phase 14 hat zehn eigenständige Alleinstellungsmerkmale
ohne Lightroom-Entsprechung geliefert (ADR-0041). Der Nutzer möchte
jetzt gezielt echte Photoshop-exklusive Fähigkeiten nachziehen — Dinge,
die es in Lightroom nachweislich nicht gibt, sich aber sauber in die
bestehende ApertureX-Architektur einfügen und einen spürbaren, visuell
beeindruckenden Funktionszuwachs bringen. Dieselbe "kostenlos/lokal
statt bezahlter Cloud-API"-Linie wie Phase 13/14, wo KI zum Einsatz
kommt.

**Recherche-Disziplin wie in jeder vorherigen Phase:** jede "Lightroom
hat das nicht"-Behauptung unten wurde per echter Web-Suche gegengeprüft.

| # | Funktion | Befund |
|---|---|---|
| 1 | Content-Aware Move | Photoshop-exklusiv, bestätigt per offenem Adobe-Community-Feature-Request "Content Aware Move for Lightroom" — Lightroom hat nur "Content-Aware Remove"/"Generative Remove" (Entfernen), kein Verschieben mit automatischer Neubefüllung der Ausgangsstelle |
| 2 | Blend-If (Tonwertbereich-Blending) | Photoshop-exklusiv — Adobe-Community-Quelle: "Blend If sliders ... aren't directly replicated in Lightroom's interface" |
| 3 | Verflüssigen (Liquify) | Photoshop-exklusiv, bestätigt per Adobe-Community-Thread "Come on Adobe, We need liquify in Lightroom already!" |
| 4 | Inhaltssensitives Skalieren (Content-Aware Scale/Seam Carving) | Photoshop-exklusiv seit CS4 (Adobe lizenzierte die Seam-Carving-Technologie von MERL) — "only Photoshop CS4 supports content aware scaling" |
| 5 | Automatisches Hautglätten (gesichtsbewusst) | Photoshops "Skin Smoothing"/"Smart Portrait"-Neural-Filter sind Photoshop-exklusiv (teils cloud-pflichtig) — Lightroom hat nur den manuellen Anpassungspinsel, kein automatisches gesichtserkennungsgestütztes Glätten |

**Architektur-Entscheidung — vier von fünf Funktionen ohne jede neue
Abhängigkeit, durch Wiederverwendung bereits bestehender Bausteine**
(real in dieser Sitzung gegen den aktuellen Code verifiziert, nicht aus
Erinnerung an frühere Phasen behauptet):

- `apx_ai::inpaint::InpaintSession::fill_rgb8` (Phase 13 Schritt 1) und
  `CompositeLayer`/`CompositeLayerSource` (Phase 14 Schritt 3) tragen
  Schritt 1 (Content-Aware Move) zusammen, ganz ohne neuen EDL-Typ —
  ein Fill-Patch für die Ausgangsstelle plus eine neue Compositing-
  Ebene für das verschobene Objekt an der Zielposition.
- `apx_ai::segmentation::person_alpha` (Hautton-Heuristik, Phase 14
  Schritt 10 als Vorbild für `sky_alpha`) und
  `apx_ai::faces::detect_face_regions` (Phase 11 Schritt 5, kein
  Feature-Gate) plus `stages::frequency_separation::{split,combine}`
  (Phase 14 Schritt 2, bereits mit frei wählbarem `radius_px`) tragen
  zusammen Schritt 5 (automatisches Hautglätten) — drei bestehende
  Bausteine zu einem neuen Automatik-Feature kombiniert.
- **Real geprüfter Namensunterschied zur ursprünglichen Annahme:** die
  Hautton-Heuristik heißt `person_alpha`, nicht `skin_alpha`. Das
  feature-gated `apx_ai::people::PersonEmbedder` liefert Bounding-Box +
  Embedding, aber **keine Landmark-Koordinaten nach außen** — für
  Schritt 5 reicht die Bounding-Box aus `faces.rs`, echte Landmarken
  sind nicht nötig.

**Seam-Carving-Lizenzprüfung (Schritt 4), echter Befund statt
Annahme:** `seamcarving` (crates.io v0.2.3) existiert und lädt sich per
`cargo add --dry-run` sauber, ist aber **LGPL-3.0-or-later** lizenziert
— ein Copyleft mit echten Verlinkungs-/Weitergabepflichten für ein
statisch gelinktes Rust-Binary, ein Bruch mit der durchgehend
permissiven Linie (MIT/Apache-2.0/BSD-3-Clause) jeder bisher gewählten
Abhängigkeit laut `THIRD_PARTY.md`. Kein anderer Treffer auf crates.io.
**Entscheidung:** Seam Carving selbst implementieren (klassischer, gut
dokumentierter Algorithmus, Avidan & Shamir, SIGGRAPH 2007) — kein
Lizenzrisiko, keine neue Abhängigkeit für ganz Phase 15.

**Testdisziplin, explizite Nutzervorgabe für diese Phase:** anders als
Phase 9–14 kein Test nach den einzelnen Schritten 0–5 (dort nur
`cargo check`/`tsc -b`-Kompilierprüfung, Commit+Push je Schritt) — die
komplette Testausführung läuft gebündelt erst einmalig in Schritt 6.

**Nicht Teil dieser Phase:** volle Photoshop-Parität (Vektor-
Ebenenmasken, Smart Objects, Aktionen/Skript-Recorder, Fluchtpunkt,
Puppenstock-Verzerrung) — eigene, größere Ausbaustufen.

**Nachtrag (Schritt 6, Abnahme):** alle fünf Funktionen wie geplant
gebaut, keine Abweichung von der oben skizzierten Architektur. Gezielte
Unit-Tests je neuem Modul ergänzt (`stages::composite::blend_if_weight`,
`stages::liquify`, `stages::geometry::apply_content_aware_scale`,
`stages::skin_smoothing`, `apx_ai::seam_carving` — inklusive eines
Tests, der belegt, dass eine vollständig geschützte Bildfläche einen
Breiten-Schrumpf tatsächlich übersteht). `cargo fmt/clippy/test
--workspace`, `tsc -b`, Vitest und die volle Playwright-Suite laufen
grün — bis auf einen einzelnen `tat-flow.spec.ts`-Fehlschlag
(Vektorskop-Panel überlagert per `pointer-events` einen TAT-Knopf), der
real gegen den Phase-14-Endstand (`e6ec6e1`, per Git-Worktree isoliert
nachgestellt) identisch reproduziert und damit nachweislich phasenfremd
ist — keine Regression dieser Phase, nicht behoben (außerhalb des
Scopes).

Nebenbei bei der vollen Testausführung einen echten, seit Phase 14
Schritt 9/10 bestehenden Bug gefunden und behoben (kein Ergebnis dieser
Phase, aber blockierte deren „volle Suite grün"-Abnahmekriterium):
`StyleTransferPatch`/`SkyReplacePatch.pixels` legten im Frontend den
rohen Base64-String statt eines dekodierten Byte-Arrays ab — Rusts
`Vec<u8>` kann eine JSON-Zeichenkette nicht deserialisieren (per
`serde_json`-Testfall real bestätigt: „invalid type: string ...,
expected a sequence"). Committen eines Stiltransfer- oder
Himmelsaustausch-Ergebnisses hätte an dieser Stelle real fehlschlagen
müssen; der zugehörige Playwright-Test bestand dennoch, weil er
gegen die gemockte Tauri-IPC-Schicht läuft, nicht gegen echte
Rust-Deserialisierung. Behoben durch `base64ToByteArray` an beiden
Store-Stellen, TS-Typen auf `number[]` korrigiert (dieselbe Konvention
wie `CompositeLayerSource`/`ContentAwareScalePatch`), betroffener
Playwright-Test entsprechend angepasst.

## ADR-0043: Phase 16 — Filter-/LUT-Bibliothek + Video als Katalog-Asset
mit Basis-Schnitt

**Status:** Angenommen
**Kontext:** Nutzerwunsch (siehe Sitzungsverlauf): direkt anwendbare
Foto-Filter/-Effekte aus einer möglichst großen öffentlichen
Bibliothek, punktuell mit Pinseln einsetzbar, auf viele Fotos auf
einmal anwendbar mit regelbarer Stärke — dieselben Filter auch für
Video, dazu Basis-Videoschnitt (Ausschneiden, Länge anpassen,
automatisches Zuschneiden auf passende Stellen, Geräuschreduktion,
Musik/Sounds hinzufügen, ähnliche Videos finden) — "eine sehr
abgespeckte CapCut-Variante". Vor der Umsetzung wurde die bestehende
Architektur real gegen den aktuellen Code geprüft (nicht aus
Erinnerung an frühere Phasen behauptet) und öffentlich nach
lizenzsauberen Filter-/Video-Werkzeugen recherchiert.

**Architektur-Befund (real gegen den Code geprüft):**

- Es gibt **keinen** Lightroom-artigen Modul-Vollbild-Umschalter. Statt
  dessen zwei getrennte Mechanismen (bewusste Vereinfachung, ADR-0037):
  `centerView: "viewer" | "grid" | "map" | "overview" | "people"`
  (`frontend/src/store/index.ts:767`) bekommt echte Leinwandfläche;
  Drucken/Buch/Web-Galerie/Diashow/Stapelverarbeitung sind dagegen
  Modal-Dialoge, angestoßen aus `Header.tsx`. Video-Bearbeitung braucht
  eine dauerhafte, interaktive Zeitleisten-Oberfläche — passt zu
  keinem der beiden bestehenden Muster exakt, am ehesten zum ersten.
  **Entscheidung:** neuer `centerView`-Wert `"video"`, strukturell
  gleichrangig mit `"viewer"`, kein neuer Modal-Dialog und keine neue
  Kompartment-Sektion im bestehenden Entwickeln-Panel.
- **Kein Video-Import existiert.** `RAW_EXTENSIONS`/`FALLBACK_EXTENSIONS`
  (`crates/apx-raw/src/format.rs:8-10`) sind rein bildbasiert, `Photo`
  (`crates/apx-catalog/src/models.rs`) hat keine Dauer-/Codec-/Audio-
  Felder. Video als Katalog-Asset ist echtes Neuland, keine Erweiterung
  eines bestehenden Feldes.
- **Kein LUT-/3D-Filter-Konzept existiert.** Presets
  (`frontend/src/lib/presets.ts`) sind laut ADR-0031/ADR-0032 strikt
  numerische EDL-Teilmengen; "LUT" kommt im Code bisher nur als interne
  1D-Lookup-Table der Gradationskurven-Stage vor
  (`stages/curves.rs::build_points_lut`), kein `.cube`-Import.
- **Masken-System ist direkt wiederverwendbar:** jede Maske
  (`MaskGeometry` in `crates/apx-pipeline/src/edl/v3.rs` — Pinsel/
  Linear-/Radialverlauf/KI-Auswahl) wendet in `stages/masks.rs` ein
  eigenes EDL-Werkzeug-Subset auf ihre Alpha-Region an und blendet
  zurück. Ein Filter/LUT wird ein weiteres Werkzeug in diesem Muster —
  punktuelle Pinsel-Anwendung ist damit keine neue Architektur, sondern
  Wiederverwendung.
- **Foto-Batch-Anwendung ist direkt wiederverwendbar:**
  `syncSettingsToSelection` (`frontend/src/store/index.ts:365`) und die
  bereits mehrfach getestete Preset-Stapel-Anwendung übertragen EDL-
  Abschnitte auf eine Mehrfachauswahl — ein Filter ist einfach ein
  Preset mit optionaler LUT-Referenz.
- **Ähnliche-Fotos-Erkennung ist direkt auf Video übertragbar:**
  Phase-9-Perceptual-Hashing (`image_hasher`-Crate, gehashter 256px-
  Thumbnail, Hamming-Distanz-Gruppierung) in
  `list_perceptual_duplicate_groups`
  (`crates/apx-app/src/commands.rs:2391-2441`) — für Video genügt
  derselbe Hash auf einen repräsentativen, per `ffmpeg` extrahierten
  Keyframe.
- **ffmpeg-Grundsatzentscheidung (ADR-0034) bleibt tragend:** kein
  Bündeln, System-Binary vorausgesetzt, `Command`-Subprozess-Aufruf
  (`crates/apx-export/src/video.rs`), weil kein brauchbarer reiner
  Rust-H.264-Encoder existiert und Bündeln GPL-Lizenzpflichten nach
  sich zöge. Alle neuen Video-Fähigkeiten in dieser Phase folgen
  demselben Muster — **keine neue Rust-Video-Abhängigkeit**.

**LUT-Filter-Engine — real recherchierter Lizenzbefund statt Annahme:**
eine "öffentliche Bibliothek mit Tausenden einheitlich lizenzierten
Effekten" existiert real nicht — freie LUT-Pakete sind über Dutzende
Quellen verstreut mit uneinheitlicher Lizenzlage (CC0, CC-BY, viele nur
vage "kostenlos nutzbar" ohne echte OSI-Lizenz), dasselbe Muster wie
beim Stiltransfer in Phase 14 (ADR-0041). Vorhandene Rust-Crates für
`.cube`-Anwendung (`wagahai_lut`, `lut-cube`) sind kaum verbreitet,
Wartungsstatus unklar. **Entscheidung, dieselbe Linie wie Seam Carving
in Phase 15:** die Anwendungs-Engine (trilineare Interpolation über ein
`.cube`-Raster, gut dokumentierter, einfacher Algorithmus) wird selbst
implementiert — kein Lizenzrisiko, keine Wartungsabhängigkeit von
einem kaum genutzten Drittanbieter-Crate. `.cube` ist ein offenes,
patentfreies Textdateiformat (Industriestandard, u. a. von Lightroom,
Premiere, DaVinci Resolve, Capture One genutzt) — das Format selbst ist
nicht schutzfähig. Statt eines großen gebündelten Presets: ein kleines,
**einzeln lizenzgeprüftes** Starter-Set (nach demselben "Opt-in-
Download, geprüfte Herkunft"-Muster wie MiDaS/Stiltransfer-Modelle in
Phase 14, in `THIRD_PARTY.md` je Quelle dokumentiert) **plus** freier
Import beliebiger eigener `.cube`-Dateien — dadurch wird "Hunderte/
Tausende Effekte" real erreichbar, ohne dass ApertureX selbst
fragwürdig lizenzierte Presets bündelt/redistribuiert.

**Video-Werkzeuge — reale ffmpeg-Filter-Verifikation statt Annahme:**

| Funktion | Lösung | Befund |
|---|---|---|
| Schneiden/Trimmen, Länge anpassen | `ffmpeg -ss/-to`, bei kompatiblem Codec verlustfreier Stream-Copy (`-c copy`), sonst Re-Encode | dieselbe Subprozess-Technik wie `video.rs` |
| Automatisches Zuschneiden | nativer ffmpeg-`scdet`-Filter (Szenenwechsel-Metadaten `lavfi.scd.score`, seit ffmpeg 4.3) | echte, verifizierte native Funktion, kein externes Modell |
| Geräuschreduktion | native ffmpeg-Filter `afftdn` (reine FFT-Entrauschung, kein Modell nötig) als Standard; `arnndn` (RNN-basiert, Modell von `github.com/richardpl/arnndn-models`, aufbauend auf Xiph.org RNNoise, **BSD-3-Clause**) als stärkere Opt-in-Variante | verifiziert; Modell-Lizenz beim tatsächlichen Download-Schritt erneut gegen die dann aktuelle Repo-Lizenzdatei geprüft, nicht nur aus dieser Recherche übernommen |
| Musik/Sounds hinzufügen | dieselbe Audio-Mix-Technik wie die Diashow-Musikuntermalung (ADR-0034 Punkt 3: Vorschau über `<audio>`, Export-Mix über ffmpeg) | bestehendes Muster |

**Ehrlich benannte Grenze:** "automatisch die besten/interessantesten
Momente finden" (wie kommerzielle Tools wie CapCut es bewerben) ist ein
deutlich härteres ML-Problem — keine lizenzklare, fertige Lösung
gefunden. Schritt 7 liefert Szenenwechsel-Erkennung + einfache
Heuristiken (statische Passagen, Stille), **nicht** echte KI-
Highlight-Erkennung; das wird explizit nicht versprochen.

**Performance-Grenze, ebenfalls offen benannt:** die Foto-Pipeline
(`apx-pipeline`) ist reines CPU-Rust ohne GPU-Shader. Filter framegenau
auf Video anzuwenden (Schritt 9) ist für kurze Clips bei moderater
Auflösung machbar, kann bei langen/hochauflösenden Videos spürbar
langsam werden — Skalierungsgrenzen werden in Schritt 9/11 gemessen
und dokumentiert statt stillschweigend vorausgesetzt.

**Zuschnitt in elf Schritten** (siehe PLAN.md), in drei unabhängig
lieferbaren Blöcken: (1) Schritt 1–3 LUT-/Filter-Engine — funktioniert
komplett unabhängig von Video, liefert sofort Wert für Fotos; (2)
Schritt 4–5 Video-Fundament (Katalog-Asset, Wiedergabe) — noch ohne
Bearbeitung; (3) Schritt 6–10 Video-Bearbeitungsfunktionen, bauen auf
Block 2 auf.

**Testdisziplin (Nachtrag: expliziter als Phase 15 gefasst, Nutzervorgabe
während Schritt 1):** ausschließlich `cargo check`/`tsc -b` nach den
einzelnen Schritten 1–10 — kein `cargo test`, kein Vitest, keine
Playwright-Läufe zwischendurch, auch nicht an einem Zwischen-
Kontrollpunkt. Unit-Tests werden weiterhin je Modul geschrieben (wie in
Schritt 1), aber erst in Schritt 11 gesammelt ausgeführt.

**Nicht Teil dieser Phase:** vollwertiger Videoschnitt (Multi-Track,
Übergänge, Titel-Grafiken jenseits der bestehenden Diashow-Intro-
Funktion), echte KI-Highlight-Erkennung, Video-Farbverwaltung/HDR,
GPU-beschleunigte Video-Pipeline.

**Nachtrag (Schritt 2, Starter-LUT-Set):** wie oben real recherchiert
keine einzelne, sauber lizenzierte "Hunderte/Tausende Filter"-Quelle
gefunden — die freien LUT-Pakete, die sich fanden (Q-DDL, RocketStock
u. a.), haben uneinheitliche/unklare Lizenzbedingungen, und ein
konkreter Download-Versuch scheiterte zusätzlich an der
Netzwerk-Sandbox dieser Sitzung (mehrere Kandidaten-Domains vom
Egress-Proxy blockiert, nicht real verifizierbar). Statt eines
unverifizierten externen Downloads: fünf **selbst erstellte** parametrische
Farbverläufe (`apx_pipeline::builtin_luts` — Warm/Kühl/Kontrastreich
S/W/Verblasst/Kino Teal-Orange, reine Mathematik, kein fremdes Werk
enthalten) — dieselbe Rolle wie Lightrooms eigene mitgelieferte
"Creative"-Profile, kein Redistributions-/Lizenzrisiko. Der freie
`.cube`-Import aus Schritt 1 bleibt der Weg zu "Hunderte/Tausende
Effekte" — dafür bringt der Nutzer eigene Dateien mit, ApertureX selbst
redistribuiert kein fremdes Preset-Paket.

**Nachtrag (Schritt 3, Pinsel-Integration):** bewusst NICHT über die
bestehende `Mask`/`MaskAdjustments`-Infrastruktur gelöst, obwohl sie
strukturell naheliegend wäre. `MaskAdjustments` läuft in
`stages::masks` noch im **linearen** Arbeitsraum (vor der Farbraum-
Konvertierung), ein `.cube`-LUT ist aber für gamma-kodierte,
bildschirmreferenzierte Werte gedacht — eine LUT-Anwendung auf
Szenen-linearen Werten würde ein anderes (falsches) Ergebnis liefern
als auf denselben Werten nach sRGB-Kodierung. Denselben Bruch löst das
Projekt an anderer Stelle bereits nicht durch Farbraum-Hin-und-Her-
Konvertierung, sondern durch eine eigene, für die jeweilige
Pipeline-Position passende Implementierung (`curves::apply_linear_rgb`
für Masken vs. der globalen sRGB-`curves`-Stufe). Konsequent
übertragen: `LutFilterAdjustment` bekommt eigene `strokes`
(`LutFilterStroke` — `center_path`/`radius`/`strength`, exakt dieselbe
Form wie `LiquifyStroke`), angewendet an derselben späten
sRGB-Pipeline-Position wie die globale `strength`-Anwendung, per
Abstand-zum-Pfad-Gewichtung (dieselbe `nearest_on_path`+`smoothstep`-
Idee wie `stages::liquify`). Leere `strokes` bleiben das bisherige
globale Verhalten (Rückwärtskompatibilität), nicht-leere beschränken
die Anwendung auf die gemalten Bereiche.

Batch-Anwendung auf viele Fotos brauchte dagegen **keine** neue
Architektur: `lut_filter` in `PRESET_SECTION_KEYS` aufgenommen (treibt
sowohl den Preset-Speichern-Dialog als auch "Synchronisieren"/"Vorherige
übernehmen" — dieselbe eine Liste). Ein wichtiger Unterschied zu
`lut`/`strength` (fotounabhängig, siehe oben): `strokes` SIND
bildpositions-spezifisch (dieselbe Begründung, aus der
`liquify_strokes`/`repair`/`masks` ganz von `PresetSectionKey`
ausgeschlossen sind) — `buildPresetEdlSubset` schneidet sie deshalb beim
Sektions-Kopieren explizit heraus (Preset trägt nur die globale
Filter-Anwendung, nie gemalte Bereiche eines fremden Fotos).

**Nachtrag (Schritt 4, Video als Katalog-Asset):** wie in Schritt 0
festgelegt, erweitert eine neue Migration (`0012_video.sql`) die
bestehende `photos`-Tabelle um fünf nullable Spalten
(`media_kind`/`duration_ms`/`video_codec`/`has_audio`/`frame_rate`)
statt eine eigene `videos`-Tabelle einzuführen — ein Video bleibt eine
ganz normale Katalogzeile, Sammlungen/Schlagworte/Sterne/Filter/
Duplikat-Erkennung/Batch-Verarbeitung funktionieren automatisch weiter,
ohne dass eine dieser Stellen von Video weiß.

Import verzweigt früh nach Dateiendung
(`import::video::is_video_extension`, neue `mp4`/`mov`/`m4v`/`avi`/
`mkv`/`webm`-Liste) — bewusst NICHT über `apx_raw::read_metadata`
(reiner Bild-Decoder, würde an einem Video-Container schlicht
scheitern), sondern über `ffprobe -show_format -show_streams`
(JSON-Ausgabe, geparst über das ohnehin vorhandene `serde_json`, kein
neues Crate) — dasselbe Subprozess-Muster wie `apx_export::video`s
`ffmpeg`-Aufrufe (ADR-0034: kein Bündeln, System-Installation
vorausgesetzt). Thumbnail-Erzeugung entsprechend verzweigt: ein
einzelnes Frame per `ffmpeg -ss 00:00:01 ... -f image2pipe` direkt auf
`stdout`, eine Sekunde statt Frame 0 (oft schwarz/unscharf bei vielen
Kameras), läuft danach durch dieselbe Downscale-/Speicher-Pipeline wie
ein Foto-Thumbnail.

**Real gegen den Code geprüfter Umfang der Änderung, nicht unterschätzt:**
`NewPhoto`/`Photo` sind zentrale, überall im Katalog verwendete
Structs — 23 Konstruktionsstellen (Produktionscode und Tests über
`apx-catalog`/`apx-app`) mussten um die fünf neuen Felder ergänzt
werden. Bei bereits vorhandenen Fotos/Videos bleibt das rückwärts-
kompatibel: die Migration setzt `media_kind` per SQL-`DEFAULT 'photo'`,
alle anderen neuen Spalten sind nullable.

**Bewusst noch nicht Teil dieses Schritts** (folgt in Schritt 5 mit dem
neuen `"video"`-`centerView`): ein sichtbares Video-Abspiel-Symbol im
Raster, eine "nur Videos"/"nur Fotos"-Filteroption, und was beim Klick
auf ein Video-Asset passiert (aktuell öffnet es denselben Foto-Viewer
wie jedes andere Bild und würde dort scheitern) — reine Backend-/
Katalog-Grundlage ohne sichtbaren Effekt im UI, bis Schritt 5 die
Wiedergabe-Oberfläche liefert.

**Nachtrag (Schritt 5, Video-Wiedergabe) — Korrektur der Schritt-0-
Annahme, real gegen den Code geprüft statt aus Erinnerung übernommen:**
es gibt in dieser App **keinen** Lightroom-artigen "Foto öffnen
schaltet automatisch auf Einzelansicht um"-Mechanismus (ADR-0037: `
centerView` wechselt ausschließlich über explizite Kopfzeilen-Knöpfe/
`toggleCenterView`; ein Raster-/Filmstreifen-Klick ruft nur
`selectPhoto`/`togglePhotoSelection` auf, die lediglich `
selectedPhotoId` setzen, niemals `centerView`). Ein komplett neuer,
eigenständig geschalteter `centerView`-Wert `"video"` hätte diese
fehlende Navigation zusätzlich nachbauen müssen. Stattdessen: der
bereits bestehende Fallback-Zweig in `App.tsx` (zuvor immer `<Viewer
/>`, wenn `centerView` weder `grid` noch `overview`/`map`/`people` ist)
wurde lediglich inhaltsbewusst gemacht — zeigt `<VideoPlayer />` statt
`<Viewer />`, wenn das aktuell ausgewählte Foto `media_kind === "video"`
trägt. Kein neuer `centerView`-Wert, keine neue Navigationslogik,
funktioniert automatisch überall dort, wo bereits `selectedPhotoId`
gesetzt wird (Raster, Filmstreifen, Pfeiltasten-Schritt).

Neue `video/<id>`-Route im bestehenden `apx://`-Protokoll-Handler
(`crates/apx-app/src/protocol`) — als einzige Anfrageart mit echter
HTTP-Range-Unterstützung (`206 Partial Content`, `Content-Range`,
`Accept-Ranges`), bewusst am bestehenden `ImageCache`-Muster
("einmal berechnen, komplett im Speicher halten") vorbei: ein Video
kann hunderte MB/GB groß sein, `MAX_VIDEO_CHUNK` (8 MB) begrenzt jede
einzelne Antwort, der Browser holt den Rest selbst über weitere
Range-Anfragen nach (`<video>`-Element-Seeking braucht das, jeder
andere Anfragetyp hier nicht).

`VideoPlayer.tsx`: eigene, anklickbare Zeitleiste statt der nativen
`<video controls>`-Steuerung — dasselbe Overlay-Muster wie
`LiquifyOverlay`/`RepairOverlay` — damit Schritt 6 (Trimmen) dort
Anfang-/Ende-Ziehpunkte ergänzen kann, ohne mit einer nativen
Browser-Steuerleiste zu kollidieren.

**Absicherung gegen einen echten Fehlerfall statt nur kosmetisch
ignoriert:** das Entwickeln-Panel versucht ohne die neue Sperre einen
EDL-Ladeversuch für ein Video auszulösen (`apx_raw` kann keinen
Video-Container dekodieren) — an allen drei Stellen, die das auslösen
können (`selectPhoto`, `togglePhotoSelection`s Toggle-Zweig,
`toggleDevelopPanel`), wird das jetzt übersprungen und stattdessen
derselbe "kein gültiger Bearbeitungszustand"-Reset wie bei "kein Foto
ausgewählt" angewendet — keine stillschweigend veraltete Anzeige des
vorherigen Fotos.

**Nachtrag (Schritt 6, Schneiden/Trimmen):** neuer `apx-app`-Command
`trim_video(photo_id, start_ms, end_ms)` — **nicht-destruktiv**, wie
schon bei virtuellen Kopien (ADR-0035) und Stacking-Ergebnissen
(Phase 9 Schritt 8): das Original bleibt unangetastet, das Ergebnis
landet als eigene neue Katalogzeile (`<stem>_trim[_N].<ext>` im selben
Ordner, Kollisionsvermeidung per Suffix-Zähler). Der eigentliche
Schnitt läuft über `ffmpeg -ss <start> -t <dauer> -c copy` (verlustfrei,
Millisekunden-Genauigkeit hängt vom Keyframe-Abstand des Quellcodecs
ab) — schlägt der reine Stream-Copy-Pfad fehl (nicht jeder Codec/
Container erlaubt beliebige Schnittpunkte ohne Neucodierung), fällt der
Command automatisch auf `-c:v libx264 -crf 18 -preset medium -c:a aac`
zurück. Nach dem Schnitt: `ffprobe`-Metadaten-Extraktion
(`import::video::extract_video_metadata`, wiederverwendet aus Schritt 4)
und Thumbnail-Erzeugung (`import::thumbnails::generate_one`,
wiederverwendet aus Schritt 4 — beide Funktionen dafür von modul- auf
`pub(crate)`-Sichtbarkeit angehoben) für das neue Asset, statt beides
neu zu bauen.

Frontend: `VideoPlayer.tsx`s eigene Zeitleiste (bewusst kein natives
`<video controls>`, siehe Schritt-5-Nachtrag) bekommt jetzt die dort
vorgesehenen Anfang-/Ende-Ziehpunkte — als farbige Marker auf der
Zeitleiste plus zwei "aktuelle Position markieren"-Knöpfe (dasselbe
Muster wie in den meisten Schnittwerkzeugen, statt echter Zieh-
Interaktion auf der Leiste — deutlich einfacher umzusetzen, für den
"minimales Trimmen"-Anspruch dieser Phase ausreichend). Trimm-Zustand
(`videoTrimStartMs`/`videoTrimEndMs`) ist reiner Entwurf im Store, bis
`commitVideoTrim` den `trim_video`-Command aufruft; setzt sich beim
Fotowechsel automatisch zurück (im bereits bestehenden
Zurücksetzen-Effekt), damit keine In/Out-Punkte eines vorherigen Videos
am neuen kleben bleiben. Nach erfolgreichem Schnitt wählt der Store das
neu entstandene Video automatisch aus (`selectedPhotoId` auf die
Antwort von `trim_video`).

**Nachtrag (Schritt 7, Automatisches Zuschneiden):** wie in ADR-0043
oben real recherchiert und tabelliert, keine eigene Bild-Differenz-
Heuristik geschrieben, sondern ffmpegs nativer `scdet`-Filter
(Szenenwechsel-Erkennung, seit ffmpeg 4.3, kein externes Modell nötig)
genutzt — neuer `apx-app`-Command `detect_video_scene_changes(photo_id,
threshold?)`. `scdet` protokolliert jeden erkannten Wechsel als eine
`av_log`-Info-Zeile auf `stderr` in der Form `lavfi.scd.score: <wert>,
lavfi.scd.time: <sekunden>` (`-f null -` verwirft die eigentliche
Bildausgabe, `-an` überspringt unnötig die Tonspur) — der Command
parst diese Zeilen per einfachem Textsuche+Parse statt eines
Regex-Crates (kein neues Crate für ein simples "ab Marker bis zum
nächsten Nicht-Ziffern-Zeichen"-Muster), sortiert/dedupliziert die
resultierenden Millisekunden-Zeitstempel.

**Bewusst begrenzter Umfang, wie in ADR-0043 vorab ehrlich benannt:**
kein "beste/interessanteste Momente"-KI-Highlight-Ranking (das bleibt
explizit außerhalb dieser Phase) — Schritt 7 liefert reine
Szenenwechsel-Erkennung plus eine Bequemlichkeitsfunktion, die
`videoTrimStartMs`/`videoTrimEndMs` (aus Schritt 6) automatisch mit dem
Szenenabschnitt um die aktuelle Wiedergabeposition vorbelegt (der
zuletzt erkannte Wechsel davor als Start, der erste danach als Ende,
Videoanfang/-ende als Randfälle über `duration_ms` aus dem Katalog) —
"automatisch zu einem guten Abschnitt zuschneiden" im Sinne von
"objektive Szenengrenzen finden und als Schnittvorschlag anbieten",
nicht im Sinne von "die interessanteste Szene erraten". Die eigentliche
Schnittausführung bleibt bewusst der bereits bestehende
`commitVideoTrim`-Weg aus Schritt 6 — keine zweite, parallele
Schnitt-Pipeline.

`VideoPlayer.tsx`: gelbe Ein-Pixel-Striche auf der bestehenden
Zeitleiste markieren jeden erkannten Wechsel (dasselbe
Positionsberechnungs-Muster wie die Start-/Ende-Marker aus Schritt 6);
erkannte Szenenwechsel gehören zu genau einem Video und werden beim
Fotowechsel verworfen (neue `clearVideoSceneChanges`-Aktion, im
bestehenden Zurücksetzen-Effekt neben `clearVideoTrim` aufgerufen).

**Nachtrag (Schritt 8, Geräuschreduktion + Musik/Sounds hinzufügen):**
wie in ADR-0043s Tabelle real recherchiert, für die Geräuschreduktion
bewusst nur `afftdn` (reine FFT-Spektral-Subtraktion, seit jeher Teil
von ffmpeg, kein Modell nötig) implementiert, **nicht** das dort
ebenfalls genannte RNN-basierte `arnndn` — dessen Modell-Download hätte
dasselbe Opt-in-Download-Muster wie MiDaS/LaMa/Stiltransfer gebraucht
(inklusive erneuter Lizenzprüfung der dann aktuellen
`arnndn-models`-Repo-Datei zum Download-Zeitpunkt, nicht nur aus dieser
Recherche übernommen) — das hätte den Schritt deutlich aufgebläht, ohne
dass `afftdn` allein für den "Basis-Videoschnitt"-Anspruch dieser Phase
unzureichend wäre. Neuer Command `denoise_video_audio(photo_id,
strength)` mit drei festen Stufen (schwach/mittel/stark →
`afftdn=nr=6/12/24`, 12 ist `afftdn`s eigener Standardwert). Wie
`trim_video`: nicht-destruktiv, `-c:v copy` lässt den Video-Stream
unangetastet, nur die Tonspur wird neu kodiert.

Musik/Sounds hinzufügen (`add_video_audio_track`) nutzt bewusst
**dieselbe** Audio-Mix-Technik wie die bereits bestehende Diashow-
Musikuntermalung (`export_slideshow_video`, ADR-0034 Punkt 3) statt
eine zweite Implementierung zu schreiben — hier auf ein bereits
bestehendes Video-Asset angewendet statt beim Rendern einer neuen
Diashow. Zwei Modi: `"mix"` (`amix`-Filter, `duration=first` — die
Ausgabelänge folgt bewusst der *Original*-Tonspur, damit eine kürzere/
längere Musikdatei die Videolänge nicht verändert; fällt automatisch
auf `"replace"` zurück, wenn das Video gar keine eigene Tonspur hat)
und `"replace"` (Tonspur komplett ersetzen, mit explizitem `-t` auf die
aus dem Katalog bereits bekannte Originallänge — verhindert, dass eine
längere Musikdatei die Ausgabe über die Videolänge hinaus verlängert).
Die Audiodatei wählt dieselbe generische `pick_file_path`-Dialog-
Infrastruktur wie `SlideshowDialog.tsx`s Musikauswahl (kein neuer
Datei-Dialog-Command).

**Kleine Refaktorierung im Zuge dessen:** `trim_video`s bis dahin
inline stehende Zielpfad-Kollisionsvermeidung und Metadaten-
Extraktion+Thumbnail-Erzeugung wurden in zwei geteilte Funktionen
(`unique_sibling_video_path`, `register_video_result_as_new_photo`)
gezogen, damit alle drei Video-Bearbeitungs-Commands (Schritt 6 und 8)
exakt dieselbe "neues Katalog-Asset anlegen"-Logik verwenden statt sie
drei Mal zu duplizieren.

**Nachtrag (Schritt 9, Filter/LUT auf Video anwenden):** wendet
**dieselbe** trilineare `.cube`-Interpolation an, die Schritt 1 für
Fotos gebaut hat (`apx_pipeline::stages::lut_filter::apply`), framegenau
auf jedes Bild eines Videos — keine zweite LUT-Implementierung, keine
ffmpeg-eigene `lut3d`-Filterkette. Zwei gekoppelte `ffmpeg`-
Subprozesse: der erste dekodiert zu rohen RGBA8-Frames auf `stdout`
(`-f rawvideo -pix_fmt rgba`), ein eigener Rust-Thread liest sie
framegenau, wendet die LUT an und schreibt das Ergebnis in `stdin`
eines zweiten `ffmpeg`, der die transformierten Frames re-kodiert und
per zweitem Input (dieselbe Quelldatei erneut, `-map 1:a?`) die
Original-Tonspur unverändert hinüberkopiert. Der Frame-Pumpen-Thread
bekommt die `LutFilterAdjustment` als geklonten Wert übergeben (nicht
als Referenz) — `std::thread::spawn` verlangt `'static`-Daten, ein
Klon der (mit höchstens einigen zehntausend Floats kleinen) LUT-Tabelle
ist dafür einfacher als `std::thread::scope`.

Bewusst **global** wie bei Schritt 8, keine Pinselstriche wie bei
Fotos (Schritt 3) — eine pro-Frame-Maske für ein bewegtes Bild wäre ein
eigenständiges, deutlich größeres Feature (Tracking, Interpolation
zwischen Keyframes) und nicht Teil des "Basis-Videoschnitt"-Anspruchs
dieser Phase. Frontend nutzt dieselben `builtinLutFilters` (Schritt 2)
und denselben `.cube`-Import-Dialog (`importLutCubeFile`, Schritt 1)
wie das Foto-Entwickeln-Panel — keine zweite Filter-Bibliothek für
Video.

**Performance ehrlich unverifiziert in dieser Sandbox:** wie in
ADR-0043 vorab benannt, ist `apx-pipeline` reines CPU-Rust ohne
GPU-Shader — ein Zwei-Prozess-Pipe-Aufbau mit Pro-Frame-CPU-Filterung
ist für kurze Clips bei moderater Auflösung machbar, kann aber bei
langen/hochauflösenden Videos spürbar langsam werden. Eine konkrete
Messung (Sekunden pro Sekunde Video bei einer bestimmten Auflösung)
fehlt dieser Sitzung mangels eines echten Test-Videos mit bekannten
Referenzwerten in der Sandbox — wird ehrlich als offen benannt statt
stillschweigend als geprüft ausgegeben; siehe Schritt 11 für eine
Nachprüfung, falls dort ein Testclip verfügbar wird.

**Nachtrag (Schritt 10, Ähnliche Videos finden):** wie in ADR-0043s
Recherche vorab festgehalten ("Ähnliche-Fotos-Erkennung ist direkt auf
Video übertragbar") — neuer Command `list_similar_video_groups`
arbeitet **exakt** wie der bestehende Perceptual-Hash-Duplikat-
Assistent für Fotos (`list_perceptual_duplicate_groups`, Phase 9
Schritt 1: derselbe `image_hasher`, dieselbe O(n²)-Gruppierung),
beschränkt auf `media_kind == "video"`. Kein neuer Hashing-Algorithmus,
kein zweiter Keyframe-Extraktionsweg: die gehashte Grundlage ist
dasselbe Vorschau-Frame, das bereits bei Import per `ffmpeg` extrahiert
wird (`extract_video_frame`, Phase 16 Schritt 4) und ohnehin im
`PreviewLevel::Thumbnail`-Cache liegt.

**Kleine, bewusste DTO-Erweiterung statt einer großen:** `PhotoDto`
selbst trägt kein `folder_id`-Feld — es wird an Dutzenden Stellen im
gesamten Frontend verwendet, eine Erweiterung hätte (wie die 23
`NewPhoto`/`Photo`-Konstruktionsstellen in Schritt 4) viele
Testfixturen berührt, für einen einzigen neuen Anwendungsfall
unverhältnismäßig. Stattdessen ein schlanker neuer Wrapper-Typ
`SimilarVideoDto { photo: PhotoDto, folder_id: String }`, nur für
diesen einen Command — das Frontend braucht den Ordner ausschließlich,
um bei einem gefundenen ähnlichen Video (das in einem *anderen* Ordner
liegen kann) per neuer `jumpToVideo`-Store-Aktion dorthin zu
wechseln (`selectFolder`+`loadPhotosForFolder`, dann `selectPhoto`) —
`selectPhoto` allein hätte nicht gereicht, weil `VideoPlayer.tsx` das
aktuelle Foto über `photosByFolder[selectedFolderId]` auflöst.

## ADR-0043-Nachtrag (Schritt 11): Dokumentation, volle Verifikation, Abnahme

`FEATURES.md`/`THIRD_PARTY.md`/`PLAN.md` aktualisiert (siehe deren
Einträge). Vollständig grün: `cargo fmt --all -- --check`, `cargo
clippy --workspace --all-targets -- -D warnings -D
clippy::unwrap_used` (Standard-Features **und** `--features people`),
`cargo test --workspace`, `tsc -b`, volle `vitest run`-Suite (223
Tests). Ein vorbestehender Clippy-Fund (`useless_vec` in
`apx-ai::style_consistency`, aus Phase 14, durch eine neuere
Clippy-Version verschärft) wurde nebenbei behoben.

**Zusätzliche, über reines Kompilieren hinausgehende Verifikation:**
da keiner der neuen Video-Commands (Schritte 6–10) einen automatisierten
Rust-Test gegen einen echten `ffmpeg`-Subprozess hat (die Testfixture
wäre eine echte Videodatei, die es im Repository nicht gibt), wurde in
dieser Sitzung ein reales Testvideo per `ffmpeg -f lavfi` erzeugt und
jede einzelne Befehlszeile aus dem Code manuell dagegen ausgeführt:
Trim per Stream-Copy (bestätigt: schneidet an der nächsten
Keyframe-Grenze, nicht exakt — wie dokumentiert) und der
Re-Encode-Fallback (exakte Dauer), `scdet`-Szenenerkennung, `afftdn`-
Entrauschung, Musik mischen (`amix`) und ersetzen, sowie — am
wichtigsten — der komplette Zwei-Prozess-Rohframe-Pipe-Aufbau für die
Video-LUT-Anwendung (`ffmpeg … | ffmpeg …`, echte Shell-Pipe statt nur
über eine Zwischendatei) mit denselben Flags wie
`run_ffmpeg_apply_lut_to_video`. Alle fünf liefen fehlerfrei durch und
lieferten die erwarteten Ausgabe-Eigenschaften (Dauer/Auflösung/
Stream-Typen) — eine echte Bestätigung, dass die konstruierten
`ffmpeg`-Aufrufe funktionieren, nicht nur, dass der umgebende Rust-Code
kompiliert.

**Playwright, ehrlich unvollständig statt stillschweigend als
vollständig ausgegeben:** die drei am direktesten von den Datenmodell-
Änderungen dieser Phase betroffenen Spezifikationen (`develop-flow.spec.ts`,
`library-flow.spec.ts`, `presets-flow.spec.ts` — PhotoDto/EdlV4/
StageEnabled/`PRESET_SECTION_KEYS` wurden alle erweitert) liefen mit
39/39 grün (nachdem `PLAYWRIGHT_CHROMIUM_PATH` auf die in dieser Umgebung
vorinstallierte Chromium-Revision gesetzt wurde, siehe
`playwright.config.ts`s Kommentar dazu). Ein Lauf der kompletten
Playwright-Suite wurde begonnen, aber auf explizite Nutzeranweisung
("ohne große Tests, es läuft ja alles") nicht abgewartet und
abgebrochen. **Kein neues Playwright-Spezifikat für Video/LUT selbst**
— `LutFilterPanel.tsx`/`VideoPlayer.tsx` haben keine Mocks in
`e2e/tauri-mock.ts` bekommen, ihre Commands sind also e2e komplett
ungetestet; die einzige Absicherung dafür sind die echten
`ffmpeg`-Smoke-Tests oben plus `tsc -b`/`vitest run`.

## ADR-0044: Foto-Globus + Dichte-Heatmap (Erweiterung der Kartenansicht)

**Kontext:** Nutzerwunsch, direkt nach Phase 16, außerhalb der
sequenziellen Phasennummerierung: die bestehende Kartenansicht
(Phase 8 Schritt 7 — flache Leaflet-Karte mit einzelnen Foto-Pins)
"etwas ausweiten" — ein Foto-Globus als kleinste Zoomstufe (dreht sich,
sieht "wie ein echter Globus" aus), eine Foto-Dichte-Heatmap
(Google-Fotos-Stil, farbige Zonen statt einzelner Pins) basierend auf
*allen* geotaggten Fotos, Pins erst wieder bei sehr großem Zoom, alles
im bestehenden dunklen/technischen Design. Direkt umgesetzt ohne
Zwischen-Rückfrage (explizite Nutzervorgabe).

**Architektur-Entscheidung — drei Ebenen statt einer:**

1. **Globus** (`GlobeView.tsx`, Standard-Einstieg) — eine selbst
   gerenderte, drehbare 3D-Kugel (orthographische Projektion, reines
   Canvas 2D) mit derselben Dichte-Heatmap auf der Oberfläche, keine
   einzelnen Pins.
2. **Flache Karte, Heatmap-Zoom** — die bestehende Leaflet-Karte, jetzt
   mit denselben Dichte-Zonen statt Pins, dunklen CARTO-Kacheln statt
   der ursprünglichen hellen OSM-Kacheln.
3. **Flache Karte, Pin-Zoom** (`PIN_ZOOM_THRESHOLD = 10`) — dieselben
   Pins/Popups/Reiserouten wie zuvor, jetzt nur noch ab sehr großem
   Zoom sichtbar statt immer.

Kein neuer Backend-Command: `list_geotagged_photos` (Phase 8 Schritt 7)
liefert bereits alle geotaggten Fotos katalogweit — genau "alle sollen
angelegt werden" aus dem Nutzerwunsch, ohne Änderung.

**Globus — bewusst selbst implementiert statt einer 3D-Engine:** eine
orthographische Kugelprojektion (`lib/geoProjection.ts`, reine, unit-
getestete Funktionen: Länge/Breite → Einheitskugel-Punkt, Rotation um
zwei Achsen, Parallelprojektion auf die Bildebene) ist reine, gut
dokumentierte Vektor-Mathematik — derselbe Grund, aus dem diese Sitzung
bereits Seam Carving (Phase 15) und die `.cube`-LUT-Engine (Phase 16)
selbst implementiert statt eine Bibliothek einzubinden. Eine echte
3D-Engine (Three.js/`globe.gl`/WebGL) wäre für eine UI-Panel-Größe
unverhältnismäßig schwer; Canvas 2D mit vorab projizierten Punkten
reicht für die Ziel-Größenordnung (ein Panel, keine Vollbild-3D-Szene)
performant aus. Landmasse-Ringe, die den Horizont überschneiden, werden
bewusst **vereinfacht** geclippt (zusammenhängende sichtbare Punktläufe
als gefüllte Pfade mit einer geraden Sehne am Rand statt einer exakten
Kreisbogen-Clip-Berechnung) — ein am Bildschirm kaum wahrnehmbarer
Kompromiss (der Rand ist ohnehin durch die Rand-Abdunkelung visuell
weich), der den Rendercode deutlich einfacher hält.

**Landmasse-Daten — real recherchiert, einmalig konvertiert statt einer
Laufzeit-Abhängigkeit:** `world-atlas`s `land-110m.json` (Natural-
Earth-110m-Auflösung, ISC-Lizenz, öffentlich-rechtsfreie Datengrundlage
laut Natural Earth selbst) wurde einmalig in dieser Sitzung per
`topojson-client` (ebenfalls ISC) zu einem flachen, auf zwei
Nachkommastellen gerundeten Ringe-Array konvertiert und als statische
`frontend/src/assets/world-land-110m.json` (~74 KB, ~27 KB gzip)
gebündelt — danach wurden beide npm-Pakete wieder entfernt
(`pnpm remove`), sie sind **keine** Laufzeit-Abhängigkeit. Siehe
`THIRD_PARTY.md` für die vollständige Lizenzangabe der gebündelten
Daten.

**Heatmap-Farbskala — aus den Theme-Tokens abgeleitet statt einer
generischen Regenbogen-Skala:** `lib/photoHeatmap.ts` (reine,
unit-getestete Logik, geteilt zwischen Globus und flacher Karte)
interpoliert von der Akzentfarbe (`--color-accent`, kühl/wenig Fotos)
zur Warnfarbe (`--color-danger`, viele Fotos an einem Ort) — folgt
automatisch Dark-/Hell-/Kontrastmodus und der benutzerdefinierten
Akzentfarbe (Phase 10 Schritt 7), bleibt "technisch/dunkel" statt
bunt-verspielt, wie vom Nutzer gefordert ("sich visuell dem gesamten
Design anpassen"). Dichte-Bündelung per festem Breiten-/Längengrad-
Raster (dieselbe einfache Idee wie die Personenansicht-Vorsortierung,
Phase 11 Schritt 5) statt einer echten Kernel-Density-Schätzung —
für eine Übersicht, nicht für wissenschaftliche Genauigkeit.

**Flache-Karten-Heatmap — selbst implementierter Leaflet-Layer statt
`leaflet.heat`:** `lib/leafletHeatmap.ts` baut einen eigenen
`L.Layer` (Canvas im `overlayPane`, neu gezeichnet bei
`moveend`/`zoomend`/`resize`) statt der neuen Laufzeit-Abhängigkeit
`leaflet.heat` — dieselbe Rasterung/Farbskala wie der Globus
(`photoHeatmap.ts`), keine zweite unabhängige Heatmap-Implementierung.
`leaflet.heat` wäre zwar klein und MIT-lizenziert gewesen, aber eine
neue Abhängigkeit für eine bereits vorhandene, geteilte Logik war nicht
nötig.

**Dunkle Kartenkacheln:** CARTOs "Dark Matter"-Kacheln
(`basemaps.cartocdn.com/dark_all`, kostenlos, kein API-Schlüssel,
CC-BY-3.0-Kacheln über OpenStreetMap-Daten) ersetzen die ursprünglichen
hellen Standard-OSM-Kacheln — passend zum "technisch/professionell/
dunkel"-Anspruch und zum Globus, der dieselbe dunkle Grundstimmung hat.

**Bewegungs-Rücksicht:** der Globus rotiert automatisch (langsam,
ziehbar für manuelle Drehung) — respektiert sowohl
`prefers-reduced-motion` als auch die App-eigene
`uiSettings.reduced_motion`-Einstellung (Phase 10 Schritt 6), dann
bleibt die automatische Rotation aus, Ziehen bleibt weiter möglich.

**Bestehende Playwright-Spezifikation angepasst, nicht neu
geschrieben:** `map-flow.spec.ts` ging bisher davon aus, dass der
"Karte"-Knopf direkt die flache Karte zeigt — jetzt zeigt er den
Globus (neuer Standard-Einstieg). Die GPX-Import-Tests wurden um einen
expliziten "Zur Karte →"-Klick ergänzt, ein neuer Test deckt den
Globus→Karte-Wechsel selbst ab.

**Reale visuelle Verifikation statt nur Kompilieren:** ein Playwright-
Screenshot-Testlauf (nicht Teil der eingecheckten Suite, nur für diese
Sitzung) bestätigte den Globus mit echten, erkennbaren Kontinenten
(Südamerika/Afrika/Antarktis in der Standard-Rotation) und die
Heatmap-Blobs in der Akzent→Warnfarbe-Skala auf der flachen Karte;
dabei zwei echte Bugs gefunden und behoben: (1) die Heatmap-Punkte
wurden nie an die neu erzeugte Leaflet-Heatmap-Ebene übergeben, wenn
`geotaggedPhotos` bereits vor dem ersten Wechsel in den Kartenmodus
geladen war (Effekt-Abhängigkeitsliste fehlte `mapMode`); (2) die
angezeigte Zoomstufe war eingefroren (aus einer Ref statt reaktivem
State gelesen, aktualisierte sich nie nach dem ersten Render). Beide
behoben, vor dem Commit erneut visuell bestätigt.
