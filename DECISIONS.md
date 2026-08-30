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
