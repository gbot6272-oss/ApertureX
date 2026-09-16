#!/usr/bin/env python3
"""Erzeugt die Lizenz-Momentaufnahmen unter `licenses/`.

Warum eine Momentaufnahme statt einer Prüfung zur Testlaufzeit: die
Lizenz einer Abhängigkeit steht in ihrem eigenen Manifest, nicht in
`Cargo.lock`/`pnpm-lock.yaml`. Ein Test, der sie zur Laufzeit lesen
wollte, bräuchte die entpackte Registry bzw. `node_modules` — beides ist
in der CI nicht verlässlich da. Also: hier einmal aus den echten
Manifesten erzeugen, Ergebnis einchecken, und der Test vergleicht die
Sperrdateien gegen die Momentaufnahme. Neue oder aktualisierte
Abhängigkeit ohne Zeile => Test rot => jemand muss die Lizenz ansehen.

Aufruf (aus dem Projektwurzelverzeichnis):

    python3 tools/license-audit.py
"""

import collections
import glob
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def rust_rows():
    lock = open(os.path.join(ROOT, "Cargo.lock"), encoding="utf-8").read()
    pkgs = re.findall(r'\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"', lock)
    registries = glob.glob("/root/.cargo/registry/src/*/") + glob.glob(
        os.path.expanduser("~/.cargo/registry/src/*/")
    )
    workspace = {
        os.path.basename(os.path.dirname(p))
        for p in glob.glob(os.path.join(ROOT, "crates", "*", "Cargo.toml"))
    }
    rows, missing = [], []
    for name, version in sorted(set(pkgs)):
        if name in workspace:
            rows.append((name, version, "Apache-2.0", "workspace"))
            continue
        license_text = None
        for registry in registries:
            manifest = os.path.join(registry, f"{name}-{version}", "Cargo.toml")
            if not os.path.exists(manifest):
                continue
            text = open(manifest, encoding="utf-8", errors="replace").read()
            m = re.search(r'^license\s*=\s*"([^"]+)"', text, re.M)
            if m:
                license_text = m.group(1)
            else:
                m2 = re.search(r'^license-file\s*=\s*"([^"]+)"', text, re.M)
                license_text = f"license-file:{m2.group(1)}" if m2 else "UNBEKANNT"
            break
        if license_text is None:
            missing.append(f"{name} {version}")
        else:
            rows.append((name, version, license_text, "crates.io"))
    return rows, missing


def npm_rows():
    base = os.path.join(ROOT, "frontend", "node_modules", ".pnpm")
    if not os.path.isdir(base):
        return [], ["node_modules/.pnpm fehlt — erst `pnpm install` ausführen"]
    found = {}
    for entry in os.listdir(base):
        nm = os.path.join(base, entry, "node_modules")
        if not os.path.isdir(nm):
            continue
        stack = [nm]
        while stack:
            directory = stack.pop()
            for item in os.listdir(directory):
                path = os.path.join(directory, item)
                if not os.path.isdir(path):
                    continue
                if item.startswith("@"):
                    stack.append(path)
                    continue
                manifest = os.path.join(path, "package.json")
                if not os.path.exists(manifest):
                    continue
                try:
                    data = json.load(open(manifest, encoding="utf-8"))
                except Exception:
                    continue
                name, version = data.get("name"), data.get("version")
                if not name or not version:
                    continue
                lic = data.get("license")
                if isinstance(lic, dict):
                    lic = lic.get("type")
                if not lic and isinstance(data.get("licenses"), list):
                    lic = " OR ".join(x.get("type", "") for x in data["licenses"])
                found[(name, version)] = lic or "UNBEKANNT"
    rows = [(n, v, l, "npm") for (n, v), l in sorted(found.items())]
    return rows, []


def write(path, rows, header):
    with open(os.path.join(ROOT, path), "w", encoding="utf-8") as fh:
        fh.write(header)
        for name, version, license_text, origin in rows:
            fh.write(f"{name}\t{version}\t{license_text}\t{origin}\n")
    print(f"{path}: {len(rows)} Zeilen")


def main():
    os.makedirs(os.path.join(ROOT, "licenses"), exist_ok=True)
    head = (
        "# Erzeugt von tools/license-audit.py — NICHT von Hand bearbeiten.\n"
        "# Spalten: Name\\tVersion\\tLizenz\\tHerkunft\n"
    )
    rust, rust_missing = rust_rows()
    npm, npm_missing = npm_rows()
    write("licenses/rust.tsv", rust, head)
    write("licenses/npm.tsv", npm, head)
    problems = rust_missing + npm_missing
    if problems:
        print("\nNicht auflösbar:", file=sys.stderr)
        for p in problems:
            print("   ", p, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
