# Vendored `dlib-face-recognition-sys`

Grund: `dlib-face-recognition-sys` v19.24.0-rc.1 (letzte von
`dlib-face-recognition` 0.3.2 verwendete Version, siehe `Cargo.toml`) hat
einen echten, verifizierten Ordnungsfehler in `build.rs::main()` — die
Funktion ruft **unbedingt** `download_and_unzip` (lädt `dlib`s Quellcode
von `http://dlib.net` herunter, um es aus dem Quellcode zu kompilieren)
auf, **bevor** sie `build()` aufruft, obwohl `build()` selbst bereits
einen pkg-config-Pfad enthält, der eine bereits installierte
System-`libdlib` (z. B. Debian/Ubuntu `libdlib-dev`) direkt verlinken
würde. Dieser pkg-config-Pfad ist dadurch in jeder veröffentlichten
Version toter Code — verifiziert auch gegen die neuere, aber von keiner
`dlib-face-recognition`-Version referenzierte Version 20.0.1 auf
crates.io, derselbe Fehler.

Der Fund ist real: in dieser Entwicklungs-Sandbox ist `dlib.net` vom
Netzwerk-Proxy blockiert (HTTP 403), das unveränderte `build.rs` scheitert
deshalb selbst dann, wenn `libdlib-dev`/`libblas-dev`/`liblapack-dev`
bereits installiert sind — dieselbe Art Beschaffungsproblem wie
`huggingface.co`/`docs.rs` an anderer Stelle in diesem Projekt
(`DECISIONS.md` ADR-0040).

**Fix hier:** `main()` probiert zuerst pkg-config (`dlib-1`), genau der
Code, der in `build()` bereits vorhanden war, und lädt nur bei
Fehlschlag von `dlib.net` herunter. Keine Logik geändert, nur zwei
bereits vorhandene Codeblöcke umsortiert — real gegen die in CI
installierte `libdlib-dev` (siehe `.github/workflows/ci.yml`) kompiliert,
gelinkt und mit echten Testfotos ausgeführt (siehe `apx-ai::people`s
Moduldoku).

Quelle: <https://crates.io/crates/dlib-face-recognition-sys/19.24.0-rc.1>
(`LICENSE` in diesem Verzeichnis unverändert von dort übernommen,
BSD-3-Clause).
