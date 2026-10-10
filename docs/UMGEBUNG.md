# Umgebung — Fallen, die in diesem Projekt schon zugeschlagen haben

Jeder Eintrag hier hat mindestens einmal Zeit gekostet. Die Reihenfolge
ist die Häufigkeit.

## Plattenquote — `df` täuscht

Die Containerumgebung hat eine **feste Quote** (gemessen ~39 GB). `df`
zeigt dabei „Avail 0" bei niedrigem „Used": die Quote ist aufgebraucht,
die Maschine ist nicht defekt. Löschen gelingt weiterhin, während
Schreiben scheitert, und freigegebener Platz ist sofort nutzbar.

**Symptome, die nicht nach Plattenplatz aussehen:**

- Linker bricht mit `signal 7 [Bus error]` ab
- Playwright meldet „Target crashed" (einmal 6 Fehlschläge, alle rein
  umgebungsbedingt)
- `cargo build` stirbt ohne verständliche Meldung

**Reihenfolge beim Aufräumen:**

```sh
rm -rf target/debug/incremental     # erster Kandidat
rm -f  target/debug/apx-app
rm -rf frontend/test-results frontend/playwright-report
cargo clean                         # letzte Stufe, gab 29,8 GiB frei
```

Das Zielverzeichnis ist groß, weil `Cargo.toml`
`[profile.dev.package."*"] opt-level = 3` setzt: Abhängigkeiten werden
auch im Entwicklungsbuild voll optimiert, weil die Rechenarbeit in ihnen
steckt (`rawler`, `image`, `zune-jpeg`, `rustfft`, `ndarray`).

## pnpm 10+ liest `onlyBuiltDependencies` nicht mehr in `package.json`

Die Freigabe für Installationsskripte steht in
**`frontend/pnpm-workspace.yaml`** unter `onlyBuiltDependencies`. Stand
sie in `package.json` unter `"pnpm"`, meldet pnpm nur noch eine Warnung
und bricht dann mit `ERR_PNPM_IGNORED_BUILDS` ab — `esbuild` braucht
sein Skript, es verknüpft die plattformeigene Binärdatei.

**Die Warnung ist auch ein Diagnosewerkzeug:** erscheint
„The `pnpm` field in package.json is no longer read", liest pnpm eine
alte `package.json` — das Arbeitsverzeichnis ist nicht auf dem aktuellen
Stand.

Der Fehler tritt **nur bei frischer Installation** auf. Reproduzieren:
`rm -rf frontend/node_modules && pnpm --dir frontend install
--frozen-lockfile`. Siehe ADR-0079.

## Tauri-Kommandozeile ist npm, nicht cargo

`cargo tauri build` scheitert ohne separat installiertes `cargo-tauri`.
Richtig:

```sh
npx --prefix frontend tauri build --no-bundle
```

Die Kommandozeile findet `crates/apx-app/tauri.conf.json` selbst
(ADR-0067). Die CI ruft denselben Befehl aus `crates/apx-app` heraus auf
(`npx --prefix ../../frontend tauri build`); beides geht.

## Playwright

```sh
export PLAYWRIGHT_CHROMIUM_PATH=/opt/pw-browsers/chromium-1194/chrome-linux/chrome
```

Chromium ist vorinstalliert (`PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers`);
`playwright install` niemals ausführen.

`e2e/ui-invariants.spec.ts` hat in der vollen Suite den Browser zum
Absturz gebracht (47 Neustarts auf einer Seite). Behoben, indem die Seite
alle 10 Einträge über `context.newPage()` erneuert wird. Wenn ein
ähnlicher Absturz wiederkommt: zuerst die Plattenquote prüfen, dann die
Zahl der Operationen je Seite.

## Windows

- Befehle im **Wurzelverzeichnis**, nicht in `crates\apx-app`.
- `pnpm --dir frontend install` — nicht `--dir ..\..\frontend` aus einem
  Unterverzeichnis (scheitert beim Kanonisieren des Pfads).
- `git diff` zeigt bei `M <datei>` manchmal nichts: dann ist es nur
  LF→CRLF, keine inhaltliche Änderung.
- Eine **unversionierte** Datei kann `git pull` blockieren
  („untracked working tree files would be overwritten"). Beispiel:
  `frontend/pnpm-workspace.yaml`, die pnpm sich selbst angelegt hatte.
  Löschen oder umbenennen, dann ziehen.
- Die `.exe` liegt nach `--no-bundle` unter `target\release\`; Tauri
  benennt sie nach `productName` („Aperture X"), das Cargo-Paket heißt
  `apx-app`.

## Tailwind v4 — Kaskadenfalle

Unlayered CSS schlägt `@layer utilities`. Das hat in diesem Projekt
**fünfmal** denselben Fehler erzeugt, bis er an der Ursache behoben war:
nur `position: relative` wandert in `@layer components`. Wer eine
Utility-Klasse „wirkt nicht" erlebt, prüft zuerst, ob eine unlayered
Regel sie überschreibt — nicht die Spezifität.

## GPU-Grenzen (WebGPU)

Zwei harte Grenzen, beide schon einmal übersehen:

- `max_compute_workgroups_per_dimension` = 65.535 **je Dimension**. Bei
  64 Elementen je Workgroup und 3 Kanälen je Bildpunkt liegt die
  eindimensionale Decke bei 65.535 × 64 ÷ 3 = **1.398.080 px ≈ 1,4 MP**.
  Der Dispatch muss zweidimensional aufgeteilt werden (ADR, siehe
  `crates/apx-pipeline/src/gpu/dispatch.rs`).
- `wgpu::Limits::default()` deckelt
  `max_storage_buffer_binding_size` auf **128 MiB** ≈ 11,2 MP bei
  3 × f32. Behoben durch `required_limits: adapter.limits()`.

Der Viewer rendert nach dem Loslassen bis `MAX_FULL_EDGE = 4096` px
≈ 11,2 MP — also genau auf dieser Kante. Beim Ziehen sind es 1280 px
≈ 1,1 MP (`LIVE_DRAG_MAX_EDGE`).
