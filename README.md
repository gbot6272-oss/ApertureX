## APERTURE X ##

Aperture X is an open source photo and video editor in the tradition of
Adobe Lightroom, Photoshop and DaVinci Resolve. Unlike them, Aperture X
does not rely on a pay-per-use or annual subscription model. It is
licensed under the **Apache License, Version 2.0** (see `LICENSE`) and
is built to be user-focused. The feature list is in `FEATURES.md`.

Development Team

## Lizenz

Aperture X' eigener Code steht unter der **Apache-2.0-Lizenz**
(`LICENSE`). Wer ihn weitergibt, muss `LICENSE` und `NOTICE` mitgeben —
beides liegt dem gebauten Installer bei.

**Zwei Einschränkungen, die „open source" hier nicht vollständig
machen** — sie stehen ausführlich in `NOTICE` und in `DECISIONS.md`
ADR-0063:

1. **GSAP** (Oberflächenanimationen) steht nicht unter einer
   Open-Source-Lizenz, sondern unter GreenSocks eigener Standard-
   „no charge"-Lizenz. Die Nutzung und Weitergabe ist kostenlos
   erlaubt, aber GSAP lässt sich nicht unter Apache-2.0
   unterlizenzieren. Wer Aperture X forkt, ist für diese eine
   Komponente an GreenSocks Bedingungen gebunden.
2. **Drei LGPL-Bibliotheken** — `rawler` (RAW-Dekodierung, LGPL-2.1),
   `lensfun` (Objektivkorrekturen, LGPL-3.0-or-later) und optional
   `gphoto2` (Tethering, LGPL-2.1-only). Für die quelloffene Weitergabe
   ist das unproblematisch: die Austauschbarkeit, die LGPL verlangt, ist
   durch den mitgelieferten Quelltext gegeben. Für eine geschlossene
   Weitergabe wäre sie es nicht.

Die vollständige Lizenzliste aller 952 Rust- und 170 npm-Abhängigkeiten
liegt maschinenlesbar unter `licenses/` und wird von
`tools/license-audit.py` aus den echten Manifesten erzeugt. Ein Test
(`crates/apx-app/tests/bundle_config.rs`) schlägt an, sobald eine
Abhängigkeit dazukommt, die dort nicht erfasst ist.

## Starten und bauen

**Der wichtigste Satz zuerst: für das tägliche Arbeiten einen
optimierten Build nehmen.** `cargo tauri dev` erzeugt einen
Debug-Build, und die Bildberechnung dieser App besteht aus Rechnerei
über Megapixel-Arrays — genau die Art Code, die ohne Optimierung ein
Vielfaches verliert. Wer die App benutzen und nicht an ihr entwickeln
will, baut sie einmal und startet das Ergebnis. Im Zweifel sagt die App
es selbst: Menü „Weitere Funktionen" → „Leistung prüfen…" meldet das
Build-Profil als ersten Befund (siehe `DECISIONS.md` ADR-0073).

### Systemabhängigkeiten

Node 20+ mit `pnpm` und die in `rust-toolchain.toml` gepinnte
Rust-Fassung (rustup holt sie selbst, sobald ein `cargo`-Befehl im
Repo-Verzeichnis läuft).

Unter Linux zusätzlich — dieselbe Liste, die die CI verwendet
(`.github/workflows/ci.yml`), also die einzige, die nachweislich
reicht:

```sh
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
  librsvg2-dev patchelf build-essential curl wget file \
  libxdo-dev libssl-dev libgphoto2-dev libdlib-dev \
  libblas-dev liblapack-dev
```

Unter macOS genügen die Xcode-Kommandozeilenwerkzeuge
(`xcode-select --install`); unter Windows die Visual-Studio-Build-Tools
mit dem C++-Arbeitslast-Paket.

### Benutzen

Alle Befehle vom Wurzelverzeichnis des Repositorys aus.

```sh
pnpm --dir frontend install --frozen-lockfile
npx --prefix frontend tauri build
```

Die Tauri-Kommandozeile ist eine npm-Abhängigkeit des Frontends
(`@tauri-apps/cli`), **kein** `cargo`-Unterbefehl — `cargo tauri build`
funktioniert nur, wer zusätzlich `cargo-tauri` installiert hat. Der
Aufruf oben kommt ohne das aus. Die Kommandozeile findet
`crates/apx-app/tauri.conf.json` dabei selbst (siehe `DECISIONS.md`
ADR-0067); die CI ruft denselben Befehl aus `crates/apx-app` heraus auf
(`npx --prefix ../../frontend tauri build`), beides geht.

Das Ergebnis liegt unter `target/release/bundle/` — `RELEASE.md` sagt,
welches Paket auf welcher Plattform entsteht.

### Entwickeln

```sh
pnpm --dir frontend install --frozen-lockfile
npx --prefix frontend tauri dev
```

**Der erste Build dauert lange.** `Cargo.toml` setzt
`[profile.dev.package."*"] opt-level = 3`: Abhängigkeiten werden auch
im Entwicklungsbuild voll optimiert, weil die eigentliche Rechenarbeit
in ihnen steckt (`rawler`, `image`, `zune-jpeg`, `rustfft`, `ndarray`).
Sie werden einmal übersetzt und danach zwischengespeichert — die
Übersetzungszeit des eigenen Codes ändert sich dadurch nicht, das
Zielverzeichnis wird aber deutlich größer.

### Prüfen

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings -D clippy::unwrap_used
cargo test --workspace
pnpm --dir frontend exec tsc -b
pnpm --dir frontend test
pnpm --dir frontend exec playwright install --with-deps chromium
pnpm --dir frontend run test:e2e
```

## Installer bauen und veröffentlichen

Siehe `RELEASE.md` — welche Pakete auf welcher Plattform entstehen,
welche Secrets für die Code-Signierung nötig sind, und was passiert
(bzw. nicht passiert), solange keine hinterlegt sind.
