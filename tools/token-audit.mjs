#!/usr/bin/env node
/**
 * Kurzer Token-Audit der laufenden Sitzung.
 *
 * Läuft als `PreCompact`- und `SessionEnd`-Hook (siehe
 * `.claude/settings.json`) oder von Hand:
 *
 *     node tools/token-audit.mjs
 *
 * Liest das Sitzungstranskript, schreibt einen kurzen Bericht nach
 * `.claude/token-audit/` und gibt auf stdout das Hook-JSON aus:
 * `systemMessage` für die eine Zeile im Terminal, `additionalContext`
 * mit dem Bericht für das Modell.
 *
 * **Gibt nur Statistik aus, niemals Inhalte.** Keine Nachrichtentexte,
 * keine Befehle, keine Dateiinhalte — nur Zahlen, Werkzeugnamen und
 * Dateipfade. Der Bericht landet im Repository-Verzeichnis und darf
 * deshalb nichts enthalten, was nicht hineingehört.
 *
 * Node ohne Abhängigkeiten, damit er auch unter Windows-PowerShell
 * läuft (`python3` gibt es dort oft nicht).
 */

import { createReadStream, existsSync, mkdirSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { homedir } from "node:os";
import path from "node:path";

/** Schwelle, ab der der Bericht zum Schneiden rät. */
const SCHWELLE_KONTEXT = 200_000;
/** Ab so vielen Zeichen gilt ein Werkzeugergebnis als groß. */
const GROSS = 10_000;

async function stdinJson() {
  if (process.stdin.isTTY) return {};
  let roh = "";
  for await (const stueck of process.stdin) roh += stueck;
  try {
    return JSON.parse(roh || "{}");
  } catch {
    return {};
  }
}

/**
 * Findet das Transkript. Drei Wege, weil nicht jedes Hook-Ereignis
 * dieselben Felder mitbringt: der mitgelieferte Pfad, der Pfad aus
 * `session_id`, und als letzter Ausweg die neueste `.jsonl` im
 * Projektverzeichnis.
 */
function findeTranskript(eingabe) {
  if (eingabe.transcript_path && existsSync(eingabe.transcript_path)) {
    return eingabe.transcript_path;
  }
  const projektDir = path.join(
    homedir(),
    ".claude",
    "projects",
    process.cwd().replace(/[/\\:]/g, "-"),
  );
  if (eingabe.session_id) {
    const geraten = path.join(projektDir, `${eingabe.session_id}.jsonl`);
    if (existsSync(geraten)) return geraten;
  }
  if (!existsSync(projektDir)) return null;
  const kandidaten = readdirSync(projektDir)
    .filter((n) => n.endsWith(".jsonl"))
    .map((n) => path.join(projektDir, n))
    .map((p) => ({ p, zeit: statSync(p).mtimeMs }))
    .sort((a, b) => b.zeit - a.zeit);
  return kandidaten.length ? kandidaten[0].p : null;
}

async function auswerten(datei) {
  const z = {
    turns: 0,
    kompaktierungen: 0,
    cacheRead: 0,
    cacheCreation: 0,
    input: 0,
    output: 0,
    spitze: 0,
    werkzeugAufrufe: new Map(),
    werkzeugZeichen: new Map(),
    grosse: [],
    lesungen: new Map(),
    teuer: new Map(),
    befehlMax: 0,
  };
  const TEUER = ["cargo test", "cargo clippy", "playwright", "vitest", "tsc -b", "tauri build", "grep -r"];
  const namenVonId = new Map();

  const leser = createInterface({ input: createReadStream(datei), crlfDelay: Infinity });
  for await (const zeile of leser) {
    let rec;
    try {
      rec = JSON.parse(zeile);
    } catch {
      continue;
    }
    if (rec.isCompactSummary || rec.type === "compact_boundary") z.kompaktierungen += 1;

    const nachricht = rec.message || {};
    const u = nachricht.usage;
    if (u) {
      z.turns += 1;
      z.cacheRead += u.cache_read_input_tokens || 0;
      z.cacheCreation += u.cache_creation_input_tokens || 0;
      z.input += u.input_tokens || 0;
      z.output += u.output_tokens || 0;
      const eingangsseite =
        (u.input_tokens || 0) + (u.cache_read_input_tokens || 0) + (u.cache_creation_input_tokens || 0);
      if (eingangsseite > z.spitze) z.spitze = eingangsseite;
    }

    if (!Array.isArray(nachricht.content)) continue;
    for (const block of nachricht.content) {
      if (!block || typeof block !== "object") continue;

      if (block.type === "tool_use") {
        const name = block.name || "?";
        namenVonId.set(block.id, name);
        z.werkzeugAufrufe.set(name, (z.werkzeugAufrufe.get(name) || 0) + 1);
        const eingabe = block.input || {};
        if (name === "Read" || name === "Edit" || name === "Write") {
          if (eingabe.file_path) {
            const k = `${name} ${eingabe.file_path}`;
            z.lesungen.set(k, (z.lesungen.get(k) || 0) + 1);
          }
        }
        if (name === "Bash" && typeof eingabe.command === "string") {
          if (eingabe.command.length > z.befehlMax) z.befehlMax = eingabe.command.length;
          for (const t of TEUER) {
            if (eingabe.command.includes(t)) z.teuer.set(t, (z.teuer.get(t) || 0) + 1);
          }
        }
      }

      if (block.type === "tool_result") {
        const c = block.content;
        let groesse = 0;
        if (typeof c === "string") groesse = c.length;
        else if (Array.isArray(c))
          groesse = c.reduce((s, b) => s + (b && typeof b.text === "string" ? b.text.length : 0), 0);
        else if (c != null) groesse = JSON.stringify(c).length;
        const name = namenVonId.get(block.tool_use_id) || "?";
        z.werkzeugZeichen.set(name, (z.werkzeugZeichen.get(name) || 0) + groesse);
        if (groesse >= GROSS) z.grosse.push({ groesse, name });
      }
    }
  }
  return z;
}

function zahl(n) {
  return n.toLocaleString("de-DE");
}

function bericht(z, datei) {
  const schnitt = z.turns ? Math.round((z.cacheRead + z.cacheCreation + z.input) / z.turns) : 0;
  const werkzeugZeichen = [...z.werkzeugZeichen.values()].reduce((a, b) => a + b, 0);
  const sortiert = (m, n = 6) => [...m.entries()].sort((a, b) => b[1] - a[1]).slice(0, n);
  const mehrfach = [...z.lesungen.entries()].filter(([, c]) => c > 1).sort((a, b) => b[1] - a[1]).slice(0, 8);

  const zeilen = [];
  zeilen.push(`# Token-Audit — ${new Date().toISOString()}`);
  zeilen.push("");
  zeilen.push(`Transkript: \`${datei}\``);
  zeilen.push("");
  zeilen.push("| Posten | Wert |");
  zeilen.push("| --- | --- |");
  zeilen.push(`| Turns | ${zahl(z.turns)} |`);
  zeilen.push(`| **Ø Kontext je Turn** | **${zahl(schnitt)}** |`);
  zeilen.push(`| größter Turn | ${zahl(z.spitze)} |`);
  zeilen.push(`| cache_read gesamt | ${zahl(z.cacheRead)} |`);
  zeilen.push(`| cache_creation | ${zahl(z.cacheCreation)} |`);
  zeilen.push(`| output | ${zahl(z.output)} |`);
  zeilen.push(`| Kompaktierungen | ${z.kompaktierungen} |`);
  zeilen.push(`| Werkzeugausgaben | ${zahl(werkzeugZeichen)} Zeichen |`);
  zeilen.push(`| längster Bash-Befehl | ${zahl(z.befehlMax)} Zeichen |`);
  zeilen.push("");

  zeilen.push("## Werkzeugaufrufe");
  zeilen.push("");
  for (const [n, c] of sortiert(z.werkzeugAufrufe)) zeilen.push(`- ${n}: ${zahl(c)}`);
  zeilen.push("");

  zeilen.push(`## Werkzeugergebnisse über ${zahl(GROSS)} Zeichen`);
  zeilen.push("");
  if (z.grosse.length === 0) {
    zeilen.push("- keine");
  } else {
    for (const g of z.grosse.sort((a, b) => b.groesse - a.groesse).slice(0, 5)) {
      zeilen.push(`- ${zahl(g.groesse)} Zeichen (${g.name})`);
    }
    if (z.grosse.length > 5) zeilen.push(`- … und ${z.grosse.length - 5} weitere`);
  }
  zeilen.push("");

  if (z.teuer.size) {
    zeilen.push("## Teure Befehle");
    zeilen.push("");
    for (const [n, c] of sortiert(z.teuer, 8)) zeilen.push(`- \`${n}\`: ${zahl(c)}×`);
    zeilen.push("");
  }

  if (mehrfach.length) {
    zeilen.push("## Mehrfach gelesen oder geschrieben");
    zeilen.push("");
    for (const [k, c] of mehrfach) zeilen.push(`- ${c}× ${k}`);
    zeilen.push("");
  }

  zeilen.push("## Befund");
  zeilen.push("");
  const befunde = [];
  if (schnitt > SCHWELLE_KONTEXT) {
    befunde.push(
      `Ø Kontext ${zahl(schnitt)} liegt über der Schwelle ${zahl(SCHWELLE_KONTEXT)} — ` +
        `Stand in \`PLAN.md\` festhalten und \`/clear\`.`,
    );
  }
  if (z.grosse.length > 3) {
    befunde.push(
      `${z.grosse.length} Werkzeugergebnisse über ${zahl(GROSS)} Zeichen — ` +
        `lange Ausgaben in eine Datei umleiten und nur Fehlerzeilen zeigen.`,
    );
  }
  if (z.befehlMax > 4000) {
    befunde.push(`Längster Bash-Befehl ${zahl(z.befehlMax)} Zeichen — Skripte in eine Datei schreiben.`);
  }
  if ((z.teuer.get("grep -r") || 0) > 10) {
    befunde.push(`\`grep -r\` ${z.teuer.get("grep -r")}× — das Grep-Werkzeug respektiert \`.gitignore\`.`);
  }
  if (mehrfach.length) {
    befunde.push(`${mehrfach.length} Datei(en) mehrfach gelesen — nichts erneut lesen, was gerade geschrieben wurde.`);
  }
  if (befunde.length === 0) befunde.push("Nichts auffällig. Keine Maßnahme nötig.");
  for (const b of befunde) zeilen.push(`- ${b}`);
  zeilen.push("");
  zeilen.push("Regeln: `CLAUDE.md`, Begründungen: `docs/ARBEITSWEISE.md`.");
  zeilen.push("");

  return { text: zeilen.join("\n"), schnitt, befunde };
}

const eingabe = await stdinJson();
const datei = findeTranskript(eingabe);

if (!datei) {
  process.stdout.write(
    JSON.stringify({ systemMessage: "Token-Audit: kein Transkript gefunden, übersprungen.", suppressOutput: true }),
  );
  process.exit(0);
}

const z = await auswerten(datei);
const { text, schnitt, befunde } = bericht(z, datei);

const ziel = path.join(process.cwd(), ".claude", "token-audit");
mkdirSync(ziel, { recursive: true });
const stempel = new Date().toISOString().replace(/[:.]/g, "-");
const pfad = path.join(ziel, `${stempel}.md`);
writeFileSync(pfad, text, "utf8");
writeFileSync(path.join(ziel, "latest.md"), text, "utf8");

const rel = path.relative(process.cwd(), pfad);
process.stdout.write(
  JSON.stringify({
    systemMessage:
      `Token-Audit: ${zahl(z.turns)} Turns, Ø Kontext ${zahl(schnitt)}, ` +
      `${befunde.length} Befund(e) → ${rel}`,
    hookSpecificOutput: {
      hookEventName: eingabe.hook_event_name || "PreCompact",
      additionalContext:
        `Token-Audit dieser Sitzung (automatisch, Skript \`tools/token-audit.mjs\`, Bericht in \`${rel}\`).\n\n` +
        text +
        `\n\nNenne dem Nutzer den Befund in zwei bis drei Zeilen und schlage die passenden Maßnahmen vor. ` +
        `Führe nichts aus, bevor er bestätigt hat.`,
    },
  }),
);
