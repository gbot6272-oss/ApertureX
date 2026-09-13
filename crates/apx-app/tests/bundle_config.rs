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
