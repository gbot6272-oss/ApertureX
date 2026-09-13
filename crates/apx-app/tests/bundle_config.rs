//! Prüft die Bündelungs-Konfiguration (`tauri.conf.json`) gegen die
//! tatsächlich im Repository liegenden Dateien — Phase 10 Schritt 11,
//! siehe `DECISIONS.md` ADR-0061.
//!
//! **Warum als Test und nicht als Kommentar:** ein Tippfehler in einem
//! Icon-Pfad oder eine verschobene Ressourcendatei fällt sonst erst im
//! Release-Build auf, also genau dann, wenn ein Tag schon gesetzt ist
//! und drei CI-Runner laufen. Diese Prüfungen kosten Millisekunden und
//! laufen bei jedem `cargo test`.
//!
//! Was hier bewusst NICHT geprüft wird: ob ein signierter Installer
//! herauskommt. Das braucht echte Zertifikate und einen echten
//! Plattform-Runner (siehe ADR-0037 Entscheidung 3 und `RELEASE.md`).

use std::path::PathBuf;

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn config() -> serde_json::Value {
    let raw = std::fs::read_to_string(crate_dir().join("tauri.conf.json"))
        .expect("tauri.conf.json ist nicht lesbar");
    serde_json::from_str(&raw).expect("tauri.conf.json ist kein gültiges JSON")
}

#[test]
fn every_referenced_icon_exists() {
    let config = config();
    let icons = config["bundle"]["icon"]
        .as_array()
        .expect("bundle.icon fehlt oder ist kein Array");
    assert!(!icons.is_empty(), "ohne Icon baut kein Installer");
    for icon in icons {
        let relative = icon.as_str().expect("Icon-Eintrag ist kein String");
        let path = crate_dir().join(relative);
        assert!(
            path.is_file(),
            "in tauri.conf.json referenziertes Icon fehlt: {relative}"
        );
    }
}

/// Die drei Plattformen brauchen je ein eigenes Icon-Format. Fehlt eines,
/// baut der Installer trotzdem — nur eben mit dem Tauri-Standardicon,
/// und das fällt erst auf, wenn jemand das Paket öffnet.
#[test]
fn each_platform_has_its_own_icon_format() {
    let config = config();
    let icons: Vec<String> = config["bundle"]["icon"]
        .as_array()
        .expect("bundle.icon fehlt")
        .iter()
        .map(|value| value.as_str().unwrap_or_default().to_string())
        .collect();
    for (suffix, platform) in [(".icns", "macOS"), (".ico", "Windows"), (".png", "Linux")] {
        assert!(
            icons.iter().any(|icon| icon.ends_with(suffix)),
            "kein {suffix}-Icon für {platform} in bundle.icon"
        );
    }
}

#[test]
fn bundled_resources_exist() {
    let config = config();
    let Some(resources) = config["bundle"]["resources"].as_object() else {
        panic!(
            "bundle.resources fehlt — die Drittanbieter-Hinweise müssen mit ausgeliefert werden"
        );
    };
    for source in resources.keys() {
        let path = crate_dir().join(source);
        assert!(
            path.is_file(),
            "als Ressource eingetragene Datei fehlt: {source}"
        );
    }
    assert!(
        resources.keys().any(|key| key.ends_with("THIRD_PARTY.md")),
        "THIRD_PARTY.md muss mit ins Paket: die LGPL-Komponente `rawler` \
         verlangt, dass die Lizenzhinweise beim Nutzer ankommen (ADR-0002)"
    );
}

/// Die Installer-Version darf nicht von der Crate-Version abweichen.
/// Tauri zieht sie aus `Cargo.toml`, **solange** `version` in der
/// Konfiguration fehlt. Steht sie dort, läuft sie beim nächsten
/// Versionssprung still auseinander — genau das war vorher der Fall
/// (`tauri.conf.json` stand fest auf 0.1.0, die Crate auf
/// `version.workspace`).
#[test]
fn version_is_not_pinned_in_the_config() {
    let config = config();
    assert!(
        config.get("version").is_none(),
        "tauri.conf.json pinnt eine Version — dann weicht der Installer \
         beim nächsten Versionssprung von der Crate-Version ab. Feld \
         entfernen, Tauri nimmt dann die Version aus Cargo.toml."
    );
}

/// Ohne diese Angaben zeigt Windows im Eigenschaften-Dialog und Linux in
/// der Paketverwaltung leere Felder an — das sieht nach unfertiger
/// Software aus, unabhängig davon, wie gut sie ist.
#[test]
fn the_bundle_carries_the_metadata_installers_display() {
    let config = config();
    for field in [
        "publisher",
        "copyright",
        "category",
        "shortDescription",
        "longDescription",
        "homepage",
    ] {
        let value = config["bundle"][field]
            .as_str()
            .unwrap_or_else(|| panic!("bundle.{field} fehlt"));
        assert!(!value.trim().is_empty(), "bundle.{field} ist leer");
    }
}

/// `rpm` und `msi` sind bewusst NICHT in der Zielliste (siehe ADR-0061):
/// beide brauchen Werkzeuge, deren Verfügbarkeit auf den CI-Runnern hier
/// nicht nachgewiesen werden konnte. Ein stillschweigend wieder
/// hinzugefügtes Ziel würde den Release-Job erst beim Tag-Push
/// abbrechen.
#[test]
fn only_verified_bundle_targets_are_listed() {
    let config = config();
    let targets = config["bundle"]["targets"]
        .as_array()
        .expect("bundle.targets muss eine ausdrückliche Liste sein, nicht \"all\"");
    let names: Vec<&str> = targets.iter().filter_map(|t| t.as_str()).collect();
    assert!(!names.is_empty(), "bundle.targets ist leer");
    for name in &names {
        assert!(
            ["deb", "appimage", "app", "dmg", "nsis"].contains(name),
            "unerwartetes Bündelungsziel {name:?} — siehe ADR-0061, bevor \
             es aufgenommen wird"
        );
    }
    // Je Plattform muss mindestens ein Ziel übrig bleiben, sonst baut der
    // Release-Job dort nichts und meldet trotzdem Erfolg.
    for (platform, expected) in [
        ("Linux", vec!["deb", "appimage"]),
        ("macOS", vec!["app", "dmg"]),
        ("Windows", vec!["nsis"]),
    ] {
        assert!(
            expected.iter().any(|target| names.contains(target)),
            "kein Bündelungsziel für {platform}"
        );
    }
}

/// Der Bezeichner landet in Systempfaden (Anwendungsdaten-Ordner,
/// macOS-Bundle-ID). Ein Tippfehler hier trennt eine installierte
/// Fassung von ihrem eigenen Katalog.
#[test]
fn the_identifier_is_a_reverse_domain_name() {
    let config = config();
    let identifier = config["identifier"].as_str().expect("identifier fehlt");
    assert!(
        identifier.split('.').count() >= 3,
        "identifier {identifier:?} ist keine umgekehrte Domain"
    );
    assert!(
        identifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-'),
        "identifier {identifier:?} enthält Zeichen, die Tauri ablehnt"
    );
}

/// `THIRD_PARTY.md` muss die LGPL-Komponente wirklich benennen — der
/// Ressourcen-Eintrag oben liefert die Datei nur aus, er prüft ihren
/// Inhalt nicht.
#[test]
fn the_shipped_notice_names_the_lgpl_component() {
    let path = crate_dir().join("../../THIRD_PARTY.md");
    let text = std::fs::read_to_string(&path).expect("THIRD_PARTY.md fehlt");
    assert!(
        text.contains("rawler"),
        "THIRD_PARTY.md nennt `rawler` nicht — die LGPL-Komponente muss \
         dort stehen (ADR-0002)"
    );
    assert!(
        text.to_lowercase().contains("lgpl"),
        "THIRD_PARTY.md nennt die LGPL nicht"
    );
}

/// Der Release-Job in der CI muss die oben konfigurierten Ziele auch
/// wirklich bauen — ein Test, der nur die Konfiguration prüft, würde
/// eine gelöschte CI-Stufe nicht bemerken.
#[test]
fn the_ci_workflow_still_builds_and_publishes_installers() {
    let path = crate_dir().join("../../.github/workflows/ci.yml");
    let workflow = std::fs::read_to_string(&path).expect("ci.yml fehlt");
    assert!(
        workflow.contains("tauri build"),
        "die CI baut keine Installer mehr"
    );
    for os in ["ubuntu-latest", "macos-latest", "windows-latest"] {
        assert!(workflow.contains(os), "der Release-Job deckt {os} nicht ab");
    }
    assert!(
        workflow.contains("APPLE_CERTIFICATE") && workflow.contains("WINDOWS_CERTIFICATE"),
        "die konditionalen Signierungs-Schritte fehlen"
    );
}

// --- Lizenzierung (Phase 10 Nachtrag, siehe DECISIONS.md ADR-0063) ---
//
// Diese vier Tests halten zusammen, was sonst still auseinanderläuft:
// die Lizenz in der Paketkonfiguration, die Lizenzdateien im Repository,
// ihre Auslieferung an den Nutzer, und — der eigentliche Wächter — die
// Behauptung, dass keine unbemerkte Copyleft-Abhängigkeit dazukommt.

/// Apache-2.0 §4(a) und §4(d) verlangen, dass LICENSE **und** NOTICE bei
/// jeder Weitergabe mitgehen. Ein Installer ist eine Weitergabe.
#[test]
fn the_bundle_declares_the_license_and_ships_both_license_files() {
    let config = config();
    assert_eq!(
        config["bundle"]["license"].as_str(),
        Some("Apache-2.0"),
        "bundle.license fehlt oder weicht ab — ohne sie zeigen deb/NSIS \
         keine Lizenz an, und ein leeres Feld sieht aus wie gar keine Lizenz"
    );
    assert_eq!(
        config["bundle"]["licenseFile"].as_str(),
        Some("../../LICENSE"),
        "bundle.licenseFile zeigt nicht auf die LICENSE-Datei"
    );

    let resources = config["bundle"]["resources"]
        .as_object()
        .expect("bundle.resources fehlt");
    for required in ["LICENSE", "NOTICE"] {
        assert!(
            resources.values().any(|v| v.as_str() == Some(required)),
            "{required} wird nicht mit ausgeliefert — Apache-2.0 §4 verlangt \
             beide Dateien bei jeder Weitergabe, also auch im Installer"
        );
    }
}

/// Die Lizenzangabe in `Cargo.toml` und die tatsächliche Lizenzdatei
/// müssen dasselbe sagen. Vorher stand dort `license = "MIT"`, ohne dass
/// es eine LICENSE-Datei gab — eine Angabe, auf die sich niemand
/// berufen konnte.
#[test]
fn the_license_file_matches_what_the_manifests_claim() {
    let license = std::fs::read_to_string(crate_dir().join("../../LICENSE"))
        .expect("LICENSE fehlt — ohne sie gilt urheberrechtlich 'alle Rechte vorbehalten'");
    assert!(
        license.contains("Apache License")
            && license.contains("Version 2.0, January 2004")
            && license.contains("APPENDIX: How to apply the Apache License"),
        "LICENSE ist nicht der vollständige Apache-2.0-Text (Anhang fehlt?)"
    );
    assert!(
        license.contains("Copyright 2026 Aperture X contributors"),
        "LICENSE trägt keine ausgefüllte Copyright-Zeile"
    );

    let workspace = std::fs::read_to_string(crate_dir().join("../../Cargo.toml")).unwrap();
    assert!(
        workspace.contains(r#"license = "Apache-2.0""#),
        "Cargo.toml nennt nicht Apache-2.0"
    );
    let package_json =
        std::fs::read_to_string(crate_dir().join("../../frontend/package.json")).unwrap();
    assert!(
        package_json.contains(r#""license": "Apache-2.0""#),
        "frontend/package.json nennt nicht Apache-2.0"
    );
}

/// NOTICE muss die Komponenten benennen, deren Bedingungen von der
/// permissiven Norm abweichen — genau die sind es, nach denen ein
/// Weitergebender handeln muss.
#[test]
fn the_notice_names_every_component_a_redistributor_must_act_on() {
    let notice = std::fs::read_to_string(crate_dir().join("../../NOTICE"))
        .expect("NOTICE fehlt — Apache-2.0 §4(d)");
    for component in ["rawler", "lensfun", "gphoto2", "gsap", "ffmpeg"] {
        assert!(
            notice.to_lowercase().contains(component),
            "NOTICE nennt `{component}` nicht"
        );
    }
    // Ohne Normalisierung hinge diese Prüfung daran, wo NOTICE gerade
    // umbricht — der Satz steht im Fließtext und wandert beim nächsten
    // Umformatieren über die Zeilengrenze.
    let flat = notice.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains("not an OSI-approved open-source license"),
        "NOTICE verschweigt, dass GSAPs Lizenz keine Open-Source-Lizenz ist \
         — genau das ist der Punkt, den ein Fork wissen muss (ADR-0063)"
    );
}

/// Wertet einen SPDX-Ausdruck gegen eine Liste unbedenklicher Lizenzen aus.
///
/// ODER und UND bedeuten Verschiedenes und dürfen nicht gleich behandelt
/// werden:
///
/// - `A OR B` ist erfüllt, sobald **ein** Zweig unbedenklich ist — man
///   darf sich aussuchen, unter welcher Lizenz man die Komponente nimmt.
///   Daran hängt z. B. `r-efi` (`MIT OR Apache-2.0 OR LGPL-2.1-or-later`):
///   die LGPL ist dort eine Wahlmöglichkeit, keine Auflage.
/// - `A AND B` verlangt, dass **alle** Teile erfüllt sind — etwa `brotli`
///   (`BSD-3-Clause AND MIT`), wo beide Bedingungen gleichzeitig gelten.
///
/// Ein erster Entwurf behandelte beides als ODER. Das wäre genau in die
/// falsche Richtung falsch gewesen: eine `Unbedenklich AND GPL`-Kombination
/// hätte den Test bestanden.
fn spdx_is_acceptable(expression: &str, harmless: &[&str]) -> bool {
    // Das historische `MIT/Apache-2.0` meint dasselbe wie `MIT OR Apache-2.0`.
    let normalized = expression.replace('/', " OR ");
    normalized.split(" OR ").any(|branch| {
        let branch = branch.trim();
        !branch.is_empty()
            && branch.split(" AND ").all(|term| {
                let term = term.trim().trim_matches(|c| c == '(' || c == ')').trim();
                harmless.contains(&term)
            })
    })
}

/// **Der eigentliche Wächter.** Jede Abhängigkeit aus `Cargo.lock` muss
/// in der eingecheckten Lizenz-Momentaufnahme stehen, und ihre Lizenz
/// muss entweder unbedenklich sein oder als benannte Ausnahme geführt
/// werden.
///
/// Ein früherer Entwurf dieses Tests verglich nur Crate-**Namen** gegen
/// eine Sperrliste. Das sah aus wie eine Prüfung, war aber keine: ein
/// Name sagt nichts über eine Lizenz. Die Lizenz steht im Manifest der
/// Abhängigkeit, das in der CI nicht verlässlich entpackt vorliegt —
/// deshalb der Umweg über `tools/license-audit.py`, das die Manifeste
/// einmal wirklich liest, und eine eingecheckte Momentaufnahme, gegen
/// die hier verglichen wird.
///
/// Neue oder aktualisierte Abhängigkeit => keine Zeile => Test rot. Wer
/// ihn grün bekommen will, muss `python3 tools/license-audit.py` laufen
/// lassen, und dabei landet die neue Lizenz sichtbar im Diff.
#[test]
fn every_dependency_license_is_recorded_and_acceptable() {
    /// Mit Apache-2.0 verträglich, keine Auflage über die
    /// Namensnennung hinaus.
    const HARMLESS: &[&str] = &[
        "MIT",
        "Apache-2.0",
        "BSD-2-Clause",
        "BSD-3-Clause",
        "BSD-1-Clause",
        "ISC",
        "Zlib",
        "Unlicense",
        "0BSD",
        "CC0-1.0",
        "MIT-0",
        "Unicode-3.0",
        "OFL-1.1",
        "CC-BY-4.0",
        "BlueOak-1.0.0",
        "CDLA-Permissive-2.0",
        "NCSA",
        "BSL-1.0",
        "MPL-2.0",
        "MPL-2.0+",
        "Apache-2.0 WITH LLVM-exception",
        "LicenseRef-UFL-1.0",
        "CC-BY-3.0",
    ];

    /// Auflagen, die über Namensnennung hinausgehen. Jede steht in
    /// NOTICE und ist dort begründet.
    const DOCUMENTED: &[(&str, &str)] = &[
        ("rawler", "LGPL-2.1"),
        ("lensfun", "LGPL-3.0-or-later"),
        ("gphoto2", "LGPL-2.1-only"),
        ("libgphoto2_sys", "LGPL-2.1-only"),
        (
            "gsap",
            "Standard 'no charge' license: https://gsap.com/standard-license.",
        ),
    ];

    let mut checked = 0usize;
    for (file, lock) in [
        ("licenses/rust.tsv", "Cargo.lock"),
        ("licenses/npm.tsv", "frontend/pnpm-lock.yaml"),
    ] {
        let snapshot = std::fs::read_to_string(crate_dir().join("../..").join(file))
            .unwrap_or_else(|_| {
                panic!("{file} fehlt — `python3 tools/license-audit.py` ausführen")
            });

        for line in snapshot.lines().filter(|l| !l.starts_with('#')) {
            let mut columns = line.split('\t');
            let (Some(name), Some(version), Some(license)) =
                (columns.next(), columns.next(), columns.next())
            else {
                panic!("kaputte Zeile in {file}: {line:?}");
            };
            checked += 1;

            if DOCUMENTED.iter().any(|(n, l)| *n == name && *l == license) {
                continue;
            }
            let acceptable = spdx_is_acceptable(license, HARMLESS);
            assert!(
                acceptable,
                "{name} {version} ({file}) steht unter '{license}' — weder \
                 unbedenklich noch als Ausnahme in NOTICE geführt. Erst \
                 Verträglichkeit mit Apache-2.0 prüfen, dann THIRD_PARTY.md \
                 und NOTICE ergänzen, dann hier eintragen. Sperrdatei: {lock}"
            );
        }
    }
    assert!(
        checked > 900,
        "nur {checked} Zeilen geprüft — die Momentaufnahme ist \
         offensichtlich unvollständig, der Test würde nichts absichern"
    );
}

/// Die Momentaufnahme muss die Sperrdatei **vollständig** abdecken.
/// Ohne diesen Test könnte jemand eine Abhängigkeit hinzufügen und die
/// Momentaufnahme unverändert lassen — der Test oben liefe weiter grün,
/// weil er nur prüft, was dort schon steht.
#[test]
fn the_license_snapshot_covers_every_locked_crate() {
    let lock = std::fs::read_to_string(crate_dir().join("../../Cargo.lock")).unwrap();
    let snapshot = std::fs::read_to_string(crate_dir().join("../../licenses/rust.tsv"))
        .expect("licenses/rust.tsv fehlt");
    let recorded: std::collections::HashSet<(&str, &str)> = snapshot
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| {
            let mut c = l.split('\t');
            Some((c.next()?, c.next()?))
        })
        .collect();

    let mut locked = Vec::new();
    for block in lock.split("[[package]]").skip(1) {
        let name = block
            .lines()
            .find_map(|l| l.strip_prefix("name = "))
            .map(|l| l.trim_matches('"'));
        let version = block
            .lines()
            .find_map(|l| l.strip_prefix("version = "))
            .map(|l| l.trim_matches('"'));
        if let (Some(name), Some(version)) = (name, version) {
            locked.push((name, version));
        }
    }
    assert!(locked.len() > 900, "Cargo.lock ließ sich nicht lesen");

    let missing: Vec<_> = locked
        .iter()
        .filter(|entry| !recorded.contains(*entry))
        .collect();
    assert!(
        missing.is_empty(),
        "{} Abhängigkeit(en) ohne Lizenz-Eintrag: {:?} — \
         `python3 tools/license-audit.py` ausführen und das Ergebnis ansehen",
        missing.len(),
        &missing[..missing.len().min(10)]
    );
}
