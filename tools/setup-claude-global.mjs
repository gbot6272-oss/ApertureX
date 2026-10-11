#!/usr/bin/env node
/**
 * Installiert die projektübergreifenden Arbeitsregeln.
 *
 *     node tools/setup-claude-global.mjs
 *
 * Kopiert `docs/CLAUDE.global.md` nach `~/.claude/CLAUDE.md`. Mehr
 * nicht: alles, was ohne Kopieren wirken kann, steht in
 * `.claude/settings.json` und kommt mit dem Klon mit — das
 * Kontext-Limit (`autoCompactWindow`) zum Beispiel.
 *
 * **Überschreibt nichts.** Gibt es dort schon eine Datei, wird die neue
 * Fassung daneben als `CLAUDE.md.neu` abgelegt und der Unterschied
 * gemeldet. Eine Einrichtungshilfe, die vorhandene Regeln wegwirft,
 * wäre schlimmer als gar keine.
 *
 * Node ohne Abhängigkeiten, damit es unter Windows-PowerShell
 * genauso läuft.
 */

import { copyFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";

const quelle = path.join(process.cwd(), "docs", "CLAUDE.global.md");
if (!existsSync(quelle)) {
  console.error(`Nicht gefunden: ${quelle}`);
  console.error("Der Befehl läuft im Wurzelverzeichnis des Repositorys.");
  process.exit(1);
}

const zielDir = path.join(homedir(), ".claude");
const ziel = path.join(zielDir, "CLAUDE.md");
mkdirSync(zielDir, { recursive: true });

if (!existsSync(ziel)) {
  copyFileSync(quelle, ziel);
  console.log(`Angelegt: ${ziel}`);
  console.log("Wirksam ab der nächsten Sitzung.");
  process.exit(0);
}

const alt = readFileSync(ziel, "utf8");
const neu = readFileSync(quelle, "utf8");

if (alt === neu) {
  console.log(`Schon aktuell: ${ziel}`);
  process.exit(0);
}

const daneben = `${ziel}.neu`;
copyFileSync(quelle, daneben);
console.log(`Es gibt dort bereits eine Datei — sie wurde NICHT überschrieben.`);
console.log(`  vorhanden: ${ziel} (${alt.length} Zeichen)`);
console.log(`  neu:       ${daneben} (${neu.length} Zeichen)`);
console.log("Beide vergleichen und von Hand zusammenführen.");
