//! Anwendungseinstellungen: Laden/Speichern als TOML, mit sinnvollen
//! Defaults, falls noch keine Einstellungsdatei existiert.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Geburtstagsausgabe (siehe `DECISIONS.md` ADR-0071) — warme,
    /// festliche Palette, Vorgabe dieser Fassung.
    ///
    /// Dass hier der Standard steht und nicht `Dark`, ist der Zweck
    /// dieser Fassung: sie ist ein Geschenk, nicht eine Variante, die
    /// man erst suchen muss. `Dark` und `Light` bleiben vollständig
    /// erhalten und auswählbar — wer das Arbeitsgerät braucht, bekommt
    /// es mit einem Klick zurück.
    #[default]
    Birthday,
    // Dark-First laut SPEC.md Abschnitt 4 — bis zur Geburtstagsausgabe
    // die Vorgabe, weiterhin unverändert verfügbar.
    Dark,
    Light,
}

/// Wie aufwendig die Oberfläche gezeichnet werden darf (ADR-0073).
///
/// **Warum es diese Stufe gibt.** Die Glasflächen dieser App tragen
/// zwei der teuersten Dinge, die eine Webansicht kennt, und zwar
/// gleichzeitig und auf derselben Ebene: einen `backdrop-filter`
/// (Weichzeichnen plus Sättigung von allem, was durchscheint) und
/// einen SVG-Verzerrungsfilter aus `feTurbulence` +
/// `feDisplacementMap`. Im Entwickeln-Modus sind das acht Flächen mit
/// zusammen gut einer Million Bildpunkten, also rund drei Viertel des
/// Fensters — und ein geteiltes `<filter id>` spart dabei nichts: die
/// Definition wird geteilt, die Auswertung geschieht pro Element.
///
/// Solange nichts dahinter passiert, ist das verkraftbar, weil das
/// Ergebnis zwischengespeichert werden kann. Sobald sich hinter einer
/// solchen Fläche etwas bewegt — ein aufsteigender Luftballon genügt
/// — ist der Zwischenspeicher bei jedem Bild ungültig und alles wird
/// neu gerechnet. Auf einer schwachen oder nicht hardwarebeschleunigten
/// Webansicht (WebKitGTK unter Linux ist der Regelfall) reicht das, um
/// die ganze Oberfläche zäh zu machen.
///
/// **Warum `Auto` die Vorgabe ist.** Welche Stufe ein Gerät trägt,
/// lässt sich nicht aus der Ferne entscheiden und auch nicht aus
/// Kennzahlen ableiten — nur messen. `Auto` misst beim Start die
/// tatsächliche Bildzeit und stuft herunter, wenn das Budget nicht
/// gehalten wird. Die drei festen Stufen sind daneben die ehrliche
/// Antwort für alle, die lieber selbst entscheiden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EffectLevel {
    /// Misst die echte Bildzeit und stuft selbsttätig herunter.
    #[default]
    Auto,
    /// Alles an: Verzerrung, Weichzeichner, Deko.
    Full,
    /// Ohne SVG-Verzerrung und ohne dauerhaft laufende Deko; der
    /// Weichzeichner bleibt. Das ist die Stufe, die auf den meisten
    /// Geräten den größten Unterschied macht, ohne dass die Oberfläche
    /// ihren Charakter verliert.
    Reduced,
    /// Ohne jeden Hintergrundeffekt: deckende Flächen statt Glas. Die
    /// schnellste Stufe und die einzige, die auch auf sehr schwacher
    /// Hardware sicher flüssig bleibt.
    Plain,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    pub theme: Theme,
    /// Benutzerdefinierte Akzentfarbe als `#rrggbb`-Hex-String, `None` =
    /// Standard-Akzent des jeweiligen Themes (siehe Phase 10 Schritt 7).
    pub accent_color: Option<String>,
    /// Sprachcode, "de" oder "en" (Phase 10 Schritt 8).
    pub locale: String,
    /// UI-Skalierung in Prozent, 75–200 laut SPEC.md Abschnitt 3.6
    /// (Barrierefreiheit, Phase 10 Schritt 6).
    pub ui_scale_percent: u16,
    /// Kontrastmodus (Phase 10 Schritt 6): schaltet auf ein
    /// High-Contrast-Tokenset im Frontend um.
    pub high_contrast: bool,
    /// Reduzierte Bewegung (Phase 10 Schritt 6): deaktiviert nicht
    /// notwendige CSS-Übergänge/-Animationen im Frontend.
    pub reduced_motion: bool,
    /// Ob das Onboarding (Phase 10 Schritt 9) bereits einmal gezeigt wurde
    /// — steuert nur das automatische Erstanzeigen, nicht die manuelle
    /// erneute Aufrufbarkeit über die Befehlspalette.
    pub onboarding_seen: bool,
    /// UI-Sounds an/aus (Phase 19, siehe `DECISIONS.md` ADR-0047) —
    /// unabhängig von `reduced_motion`: manche Nutzer wollen Bewegung,
    /// aber keinen Ton, oder umgekehrt.
    pub sound_enabled: bool,
    /// Lautstärke der UI-Sounds in Prozent, 0–100.
    pub sound_volume_percent: u16,
    /// Klangwelt (`uisfx`-"Pack", z. B. "glass"/"studio"/"minimal"/
    /// "cinematic") — siehe `frontend/src/lib/sound.ts`. Freitext statt
    /// Enum, damit ein künftiges `uisfx`-Update mit weiteren Packs ohne
    /// Rust-seitige Änderung nutzbar ist; ungültige Werte fängt das
    /// Frontend beim Aufbau des Players ab (Fallback auf "glass").
    pub sound_pack: String,
    /// Geburtstags-Deko (Konfetti, Luftballons, Begrüßung beim Öffnen)
    /// an/aus — siehe `DECISIONS.md` ADR-0071.
    ///
    /// Bewusst ein eigener Schalter und nicht an `theme == Birthday`
    /// gekoppelt: die Farben sind eine Geschmacksfrage, fliegendes
    /// Konfetti ist eine Konzentrationsfrage. Wer am Abend des
    /// Geburtstags noch zwei Stunden Fotos sortiert, soll die festliche
    /// Palette behalten und die Bewegung abschalten können, ohne das
    /// ganze Thema zu verlieren.
    ///
    /// `reduced_motion` schaltet die Bewegung zusätzlich und unabhängig
    /// ab (Barrierefreiheit geht vor Deko) — dieser Schalter ist die
    /// Geschmacks-, jener die Zugänglichkeitsentscheidung.
    pub birthday_decorations: bool,
    /// Aufwand der Oberflächen-Effekte — siehe `EffectLevel`.
    pub effect_level: EffectLevel,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            accent_color: None,
            locale: "de".to_string(),
            ui_scale_percent: 100,
            high_contrast: false,
            reduced_motion: false,
            onboarding_seen: false,
            sound_enabled: true,
            // 85 statt zuvor 70 (Phase 20, siehe `DECISIONS.md` ADR-0048):
            // Nutzer-Rückmeldung nach Phase 19 war, dass Sounds im
            // normalen Gebrauch kaum wahrnehmbar wirkten — 70 % war zu
            // leise, um gegen normale Systemlautstärke/Umgebungsgeräusche
            // wahrgenommen zu werden.
            sound_volume_percent: 85,
            // "cinematic" statt "glass" (ADR-0071): diese Fassung soll
            // beim Öffnen einen Moment haben, und `cinematic` ist laut
            // `uisfx`s eigener Beschreibung genau dafür gedacht
            // ("satte Wirkung, gut für Start-/Export-Momente").
            sound_pack: "cinematic".to_string(),
            birthday_decorations: true,
            effect_level: EffectLevel::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CatalogSettings {
    /// Pfad zum zuletzt geöffneten Katalog. `None` beim allerersten Start.
    /// Seit Phase 13 Schritt 6 (siehe `DECISIONS.md` ADR-0040-Nachtrag IV)
    /// tatsächlich von `apx-app`s `main.rs` gelesen, um beim Start den
    /// richtigen Katalog zu öffnen — vorher ein nie gelesenes Feld.
    pub last_opened_catalog: Option<String>,
    /// Zuletzt verwendete Katalogpfade, neuester zuerst, `MAX_RECENT_
    /// CATALOGS` Einträge — für eine "Zuletzt geöffnet"-Liste im
    /// Katalog-Wechsler. Enthält auch nicht mehr existierende Pfade (das
    /// Frontend blendet sie aus/markiert sie, statt sie hier
    /// stillschweigend zu entfernen — sonst würde ein kurzzeitig
    /// getrenntes Netzlaufwerk den Verlaufseintrag löschen).
    pub recent_catalogs: Vec<String>,
}

/// Obergrenze für [`CatalogSettings::recent_catalogs`].
const MAX_RECENT_CATALOGS: usize = 10;

impl CatalogSettings {
    /// Trägt `path` als zuletzt geöffneten Katalog ein: setzt
    /// `last_opened_catalog`, hebt `path` in `recent_catalogs` an die
    /// erste Stelle (entfernt einen vorhandenen Eintrag zuerst statt ihn
    /// zu duplizieren) und kappt die Liste auf [`MAX_RECENT_CATALOGS`].
    pub fn record_opened(&mut self, path: &str) {
        self.last_opened_catalog = Some(path.to_string());
        self.recent_catalogs.retain(|p| p != path);
        self.recent_catalogs.insert(0, path.to_string());
        self.recent_catalogs.truncate(MAX_RECENT_CATALOGS);
    }
}

/// Einstellungen für die KI-Funktionen (Phase 7, siehe `DECISIONS.md`
/// ADR-0033) — bislang nur der vom Nutzer selbst hinterlegte
/// Anthropic-API-Schlüssel für den LLM-Preset-Generator (kein
/// mitgelieferter Schlüssel, genau wie jede andere Desktop-App mit
/// KI-Anbindung). Liegt im Klartext in derselben TOML-Einstellungsdatei
/// wie alles andere — dieselbe Vertrauensgrenze wie z. B.
/// `last_opened_catalog` (ein lokales, nicht synchronisiertes Profil).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiSettings {
    pub anthropic_api_key: Option<String>,
    /// Lokaler Pfad zum heruntergeladenen LaMa-Inpainting-ONNX-Modell
    /// (Phase 13, siehe `DECISIONS.md` ADR-0040) — `None`, solange der
    /// Nutzer den Download nicht ausdrücklich bestätigt hat (kein
    /// mitgeliefertes ~208-MB-Modell im Installer, derselbe Opt-in wie
    /// der Anthropic-Schlüssel oben).
    pub inpainting_model_path: Option<String>,
    /// Lokale Pfade zu den beiden für echte Personen-Wiedererkennung
    /// nötigen `dlib`-Modelldateien (Phase 13 Schritt 8, siehe
    /// `DECISIONS.md` ADR-0040-Nachtrag VI, `apx-ai::people`s Moduldoku)
    /// — `people_landmark_model_path` muss auf
    /// `shape_predictor_5_face_landmarks.dat` zeigen (**nicht** die
    /// 68-Punkte-Variante, die kommerzielle Nutzung ausschließt),
    /// `people_encoder_model_path` auf
    /// `dlib_face_recognition_resnet_model_v1.dat`. Beide gemeinfrei,
    /// vom Nutzer selbst heruntergeladen, kein Bundling im Installer.
    pub people_landmark_model_path: Option<String>,
    pub people_encoder_model_path: Option<String>,
    /// Lokaler Pfad zum heruntergeladenen MiDaS-v2.1-small-ONNX-Modell
    /// (Phase 14 Schritt 8, siehe `DECISIONS.md` ADR-0041 Nachtrag VIII)
    /// — `None`, solange der Nutzer den Download nicht ausdrücklich
    /// bestätigt hat, derselbe Opt-in wie `inpainting_model_path` oben.
    pub depth_model_path: Option<String>,
    /// Lokale Pfade zu den heruntergeladenen `fast_neural_style`-ONNX-
    /// Modellen (Phase 14 Schritt 9, siehe `DECISIONS.md` ADR-0041
    /// Nachtrag IX), je Stil einzeln per `apx_ai::style_transfer::
    /// StyleKind::id()` als Schlüssel (`"candy"`, `"mosaic"`, …) —
    /// anders als `depth_model_path`/`inpainting_model_path` (je genau
    /// ein Modell) gibt es hier fünf unabhängig voneinander
    /// herunterladbare Stile, ein fehlender Schlüssel heißt „dieser Stil
    /// noch nicht heruntergeladen".
    pub style_transfer_model_paths: BTreeMap<String, String>,
    /// Lokaler Pfad zum heruntergeladenen Whisper-`ggml-base.en.bin`-
    /// Modell (Phase 17 Schritt 5, siehe `DECISIONS.md` ADR-0045,
    /// `apx_ai::subtitles`s Moduldoku) — `None`, solange der Nutzer den
    /// Download nicht ausdrücklich bestätigt hat, derselbe Opt-in wie
    /// `depth_model_path` oben.
    pub whisper_model_path: Option<String>,
    /// Lokaler Pfad zum heruntergeladenen MediaPipe-Selfie-Segmentation-
    /// ONNX-Modell (Phase 17 Schritt 8, siehe `DECISIONS.md` ADR-0045,
    /// `apx_ai::selfie_segmentation`s Moduldoku) — `None`, solange der
    /// Nutzer den Download nicht ausdrücklich bestätigt hat, derselbe
    /// Opt-in wie `depth_model_path` oben.
    pub selfie_segmentation_model_path: Option<String>,
}

/// Einstellungen für die Karte (Foto-Globus, Phase 15/ADR-0044) — bislang
/// nur der optionale CARTO-API-Schlüssel für die dunklen Raster-Basemap-
/// Kacheln (`https://{s}.basemaps.cartocdn.com/dark_all/...`, siehe
/// `MapView.tsx`). Dieselbe Vertrauensgrenze/derselbe Klartext-in-TOML-
/// Ansatz wie `AiSettings::anthropic_api_key` — ein lokaler, nicht
/// synchronisierter Einzelnutzer-Schlüssel, kein mitgelieferter Schlüssel
/// im Installer (CARTO verlangt inzwischen einen eigenen Account/Key, um
/// den "API key required"-Wasserzeichen-Hinweis auf den Kacheln zu
/// vermeiden — ohne Schlüssel funktioniert die Karte weiterhin, nur mit
/// diesem Hinweis-Overlay).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MapSettings {
    pub carto_api_key: Option<String>,
}

/// Einstellungen für den beobachteten Ordner (Phase 12 Schritt 7, siehe
/// `DECISIONS.md` ADR-0039-Nachtrag III) — ein optionaler lokaler Ordner,
/// der im Hintergrund periodisch auf neue Dateien geprüft wird, um sie
/// automatisch über denselben Weg wie ein manueller Import
/// (`import::run_with_mode`, Modus `AddInPlace`) einzuspielen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WatchedFolderSettings {
    /// Absoluter Pfad des beobachteten Ordners. `None`/leer = nichts
    /// konfiguriert (unabhängig von `enabled`).
    pub path: Option<String>,
    pub enabled: bool,
    /// Poll-Intervall in Sekunden — derselbe Abfragen-statt-Weck-
    /// Benachrichtigung-Ansatz wie beim Export-Queue-Worker
    /// (`crates/apx-app/src/main.rs`s Moduldoku), hier aber mit einem für
    /// Dateisystem-Polling sinnvollen Wert statt 150 ms.
    pub poll_seconds: u32,
}

impl Default for WatchedFolderSettings {
    fn default() -> Self {
        Self {
            path: None,
            enabled: false,
            poll_seconds: 30,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub ui: UiSettings,
    pub catalog: CatalogSettings,
    pub ai: AiSettings,
    pub watched_folder: WatchedFolderSettings,
    pub map: MapSettings,
}

impl Settings {
    /// Lädt die Einstellungen aus `path`. Existiert die Datei nicht, werden
    /// stillschweigend die Defaults zurückgegeben (kein Fehler) — das ist
    /// der Normalfall beim allerersten Start.
    pub fn load_or_default(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|source| AppError::Settings {
                message: format!(
                    "Einstellungsdatei '{}' ist ungültig: {source}",
                    path.display()
                ),
            }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(AppError::io(path, err)),
        }
    }

    /// Speichert die Einstellungen als TOML nach `path`. Das
    /// Elternverzeichnis muss bereits existieren (siehe `AppPaths`).
    pub fn save(&self, path: &Path) -> Result<()> {
        let text = toml::to_string_pretty(self).map_err(|source| AppError::Settings {
            message: format!("Einstellungen konnten nicht serialisiert werden: {source}"),
        })?;
        fs::write(path, text).map_err(|source| AppError::io(path, source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_defaults() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let path = tmp.path().join("settings.toml");

        let settings = Settings::load_or_default(&path).expect("Defaults dürfen nicht scheitern");
        assert_eq!(settings, Settings::default());
        // Geburtstagsausgabe (ADR-0071): die Vorgabe ist jetzt `Birthday`
        // statt `Dark`. Der Test prüft weiter den konkreten Wert und
        // nicht nur `Theme::default()` — sonst würde er jede künftige
        // Änderung der Vorgabe stillschweigend mitgehen und hätte
        // aufgehört, etwas zu behaupten.
        assert_eq!(settings.ui.theme, Theme::Birthday);
        assert!(
            settings.ui.birthday_decorations,
            "die Deko ist in dieser Fassung standardmäßig an"
        );
    }

    /// Die beiden Arbeits-Themes müssen nach wie vor wählbar und
    /// speicherbar sein — die Geburtstagsausgabe nimmt nichts weg.
    #[test]
    fn dark_and_light_survive_a_roundtrip() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        for theme in [Theme::Dark, Theme::Light, Theme::Birthday] {
            let path = tmp.path().join(format!("settings-{theme:?}.toml"));
            let mut settings = Settings::default();
            settings.ui.theme = theme;
            settings
                .save(&path)
                .expect("Speichern darf nicht scheitern");
            let loaded = Settings::load_or_default(&path).expect("Laden darf nicht scheitern");
            assert_eq!(loaded.ui.theme, theme);
        }
    }

    #[test]
    fn roundtrip_save_and_load() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let path = tmp.path().join("settings.toml");

        let mut settings = Settings::default();
        settings.ui.theme = Theme::Light;
        settings.ui.locale = "en".to_string();
        settings.catalog.last_opened_catalog = Some("/pfad/zum/katalog.sqlite".to_string());
        settings
            .save(&path)
            .expect("Speichern darf nicht scheitern");

        let loaded = Settings::load_or_default(&path).expect("Laden darf nicht scheitern");
        assert_eq!(loaded, settings);
    }

    #[test]
    fn record_opened_moves_an_existing_entry_to_the_front_instead_of_duplicating_it() {
        let mut settings = CatalogSettings::default();
        settings.record_opened("/a.sqlite");
        settings.record_opened("/b.sqlite");
        settings.record_opened("/a.sqlite");
        assert_eq!(settings.last_opened_catalog, Some("/a.sqlite".to_string()));
        assert_eq!(settings.recent_catalogs, vec!["/a.sqlite", "/b.sqlite"]);
    }

    #[test]
    fn record_opened_caps_the_recent_list_at_ten_entries() {
        let mut settings = CatalogSettings::default();
        for i in 0..15 {
            settings.record_opened(&format!("/katalog-{i}.sqlite"));
        }
        assert_eq!(settings.recent_catalogs.len(), 10);
        // Neuester zuerst.
        assert_eq!(settings.recent_catalogs[0], "/katalog-14.sqlite");
        assert_eq!(settings.recent_catalogs[9], "/katalog-5.sqlite");
    }

    #[test]
    fn watched_folder_settings_roundtrip_and_default_to_disabled() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let path = tmp.path().join("settings.toml");

        // Ohne gespeicherte Einstellungsdatei ist der beobachtete Ordner
        // aus (kein automatischer Dateisystemzugriff ohne, dass der
        // Nutzer ihn jemals konfiguriert hat).
        let defaults = Settings::load_or_default(&path).expect("Defaults dürfen nicht scheitern");
        assert!(!defaults.watched_folder.enabled);
        assert_eq!(defaults.watched_folder.path, None);
        assert_eq!(defaults.watched_folder.poll_seconds, 30);

        let mut settings = Settings::default();
        settings.watched_folder.path = Some("/home/user/Fotos/Eingang".to_string());
        settings.watched_folder.enabled = true;
        settings.watched_folder.poll_seconds = 60;
        settings
            .save(&path)
            .expect("Speichern darf nicht scheitern");

        let loaded = Settings::load_or_default(&path).expect("Laden darf nicht scheitern");
        assert_eq!(loaded, settings);
    }

    #[test]
    fn invalid_toml_is_reported_as_error() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let path = tmp.path().join("settings.toml");
        fs::write(&path, "das ist kein gültiges TOML {{{").unwrap();

        let result = Settings::load_or_default(&path);
        assert!(result.is_err());
    }
}
