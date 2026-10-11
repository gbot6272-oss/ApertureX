# Befehle — mit gemessenen Ausgabelängen

Alle Befehle laufen im **Wurzelverzeichnis** des Repositorys. Die
Zeichenzahlen sind echte Messungen aus Läufen dieses Projekts; „~Tokens"
ist eine Schätzung (Zeichen ÷ 4).

`$LOG` steht für eine Datei außerhalb des Repositorys (im Scratchpad der
Sitzung).

## Rust

| Befehl | Ausgabe | ~Tokens | Hinweis |
| --- | --- | --- | --- |
| `cargo fmt --all -- --check` | 0 (sauber) | 0 | billig, immer zuerst |
| `cargo clippy --workspace --all-targets -- -D warnings -D clippy::unwrap_used` | 931–1.281 | ~300 | billig bei sauberem Stand |
| `cargo test -p <crate>` | je Crate klein | — | **bevorzugt** |
| `cargo test --workspace` | **118.194** | ~30.000 | **immer umleiten** |

```sh
# Workspace-Tests ohne Kontextkosten:
cargo test --workspace > "$LOG" 2>&1; echo "EXIT=$?"
grep -o 'test result: ok. [0-9]* passed' "$LOG" | awk '{s+=$5} END {print "bestanden="s}'
grep -o '[0-9]* failed' "$LOG" | awk '{s+=$1} END {print "fehlgeschlagen="s}'
grep -n "^failures:" -A5 "$LOG" | head -40    # nur bei Fehlern
```

**Der Exit-Code allein genügt nicht.** Bei clippy werden die Warnungen
gezählt:

```sh
cargo clippy --workspace --all-targets -- -D warnings -D clippy::unwrap_used > "$LOG" 2>&1
echo "warnungen=$(grep -c '^warning' "$LOG") fehler=$(grep -c '^error' "$LOG")"
```

Die 13 Crates: `apx-core`, `apx-raw`, `apx-catalog`, `apx-pipeline`,
`apx-ai`, `apx-export`, `apx-stacking`, `apx-script`, `apx-plugin-abi`,
`apx-plugin-example`, `apx-plugin-host`, `apx-tether`, `apx-app`.

## Frontend

| Befehl | Ausgabe | ~Tokens | Hinweis |
| --- | --- | --- | --- |
| `pnpm --dir frontend run typecheck` | 0 (sauber) | 0 | = `tsc -b` |
| `pnpm --dir frontend test` | mittel | — | `vitest run` |
| `pnpm --dir frontend exec playwright test e2e/<datei>.spec.ts` | klein | — | **bevorzugt** |
| `pnpm --dir frontend run test:e2e` | **45.000–59.398** | ~12.000–15.000 | volle Suite, umleiten |

```sh
pnpm --dir frontend run test:e2e > "$LOG" 2>&1; echo "EXIT=$?"
grep -E "passed|failed" "$LOG" | tail -3
```

Playwright braucht in der Containerumgebung:

```sh
export PLAYWRIGHT_CHROMIUM_PATH=/opt/pw-browsers/chromium-1194/chrome-linux/chrome
```

## Bauen

```sh
pnpm --dir frontend install --frozen-lockfile
npx --prefix frontend tauri build --no-bundle   # nur ausführbare Datei
npx --prefix frontend tauri build               # mit Installer
npx --prefix frontend tauri dev                 # Debug-Build, langsam
```

`cargo tauri build` funktioniert **nicht** ohne separat installiertes
`cargo-tauri`. Die Tauri-Kommandozeile ist eine npm-devDependency des
Frontends (`@tauri-apps/cli`).

**Für das tägliche Benutzen einen Release-Build nehmen.** Die
Bildberechnung ist Rechnerei über Megapixel-Arrays — genau die Art Code,
die ohne Optimierung ein Vielfaches verliert. Die App sagt es selbst:
„Weitere Funktionen" → „Leistung prüfen…" meldet das Build-Profil als
ersten Befund. Das Ergebnis liegt unter `target/release/`
(`--no-bundle`) bzw. `target/release/bundle/`.

## Die volle Abnahme

In dieser Reihenfolge, damit der billigste Befund zuerst kommt:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings -D clippy::unwrap_used
pnpm --dir frontend run typecheck
pnpm --dir frontend test
cargo test --workspace            # umgeleitet
pnpm --dir frontend run test:e2e  # umgeleitet
```

## Suchen

Das **Grep-Werkzeug** statt `grep -r`: es respektiert `.gitignore`.
Rohes `grep -r` trifft `frontend/dist/` (gebaut, nicht getrackt) und die
2,15 MB Skill-Daten unter `.claude/skills/ui-ux-pro-max/data/`. Gemessen:
zwei unbedachte `grep -r`-Aufrufe erzeugten 89.123 und 86.718 Zeichen.

Wenn es doch Bash sein muss:

```sh
grep -rn "muster" --include="*.rs" --include="*.ts" --include="*.tsx" . \
  | grep -v node_modules | grep -v "^./target" | cut -c1-120
```

## Platte

```sh
du -sh target/debug/incremental   # erster Kandidat zum Löschen
cargo clean                       # gab einmal 29,8 GiB frei
```

Siehe `docs/UMGEBUNG.md` — `df` täuscht, die Quote ist fest.
