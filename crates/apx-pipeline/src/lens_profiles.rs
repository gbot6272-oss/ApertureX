//! Mini-Objektivprofildatenbank (`SPEC.md` §3.2 „Objektivkorrekturen").
//!
//! **Bewusste Vereinfachung** (siehe `DECISIONS.md` ADR-0028): kein echter
//! Adobe-LCP-/DCP-Import — stattdessen eine kleine handgepflegte Liste
//! (`crates/apx-pipeline/lens_profiles/*.json`), zur Kompilierzeit über
//! `include_str!` eingebettet (kein Laufzeit-Verzeichnis-Scan, keine
//! Tauri-Ressourcen-Bündelung nötig — die Liste ändert sich nur, wenn
//! jemand den Code ändert). Jedes Profil trägt feste Korrekturwerte
//! (Verzeichnung/Vignette/CA) statt eines echten pixelgenauen
//! Kalibrierungsdatensatzes, plus eine Liste von Teilstrings, gegen die
//! ein EXIF-Objektiv-/Kamerastring per einfachem Case-insensitive-
//! Substring-Vergleich abgeglichen wird (kein echtes Metadaten-Parsing
//! nach Hersteller-Konvention).

use serde::Deserialize;

const PROFILE_JSONS: &[&str] = &[
    include_str!("../lens_profiles/generic-wide.json"),
    include_str!("../lens_profiles/generic-standard.json"),
    include_str!("../lens_profiles/generic-tele.json"),
];

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LensProfile {
    pub id: String,
    pub display_name: String,
    /// Teilstrings (case-insensitive), gegen die ein EXIF-Objektiv-/
    /// Kamerastring abgeglichen wird — siehe Moduldoku.
    pub matches: Vec<String>,
    pub distortion_k1: f32,
    pub vignette_amount: f32,
    pub ca_red_cyan: f32,
    pub ca_blue_yellow: f32,
}

/// Alle eingebauten Profile, in fester Reihenfolge (Weitwinkel/Standard/
/// Tele) — parst die eingebetteten JSON-Strings einmalig.
pub fn all_profiles() -> Vec<LensProfile> {
    PROFILE_JSONS
        .iter()
        .map(|json| serde_json::from_str(json).expect("eingebettete Profil-JSON muss gültig sein"))
        .collect()
}

/// Sucht ein Profil per `id` (siehe `LensCorrectionAdjustment::profile_id`).
pub fn find_profile(id: &str) -> Option<LensProfile> {
    all_profiles().into_iter().find(|p| p.id == id)
}

/// Ordnet einen EXIF-Objektiv-/Kamerastring per Case-insensitive-
/// Substring-Abgleich einem Profil zu — das erste Profil mit einem
/// passenden Eintrag in `matches` gewinnt (Reihenfolge wie
/// [`all_profiles`]). `None`, wenn kein Profil passt.
pub fn match_profile_for_lens_string(lens_or_camera_model: &str) -> Option<LensProfile> {
    let haystack = lens_or_camera_model.to_lowercase();
    all_profiles().into_iter().find(|profile| {
        profile
            .matches
            .iter()
            .any(|needle| haystack.contains(&needle.to_lowercase()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_embedded_profiles_parse_and_have_unique_ids() {
        let profiles = all_profiles();
        assert_eq!(profiles.len(), 3);
        let mut ids: Vec<&str> = profiles.iter().map(|p| p.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 3, "Profil-IDs müssen eindeutig sein");
    }

    #[test]
    fn find_profile_locates_a_known_id() {
        let profile = find_profile("generic-wide").expect("sollte existieren");
        assert_eq!(
            profile.display_name,
            "Generisches Weitwinkel (typische Tonnenverzeichnung)"
        );
    }

    #[test]
    fn find_profile_returns_none_for_unknown_id() {
        assert!(find_profile("nicht-existent").is_none());
    }

    #[test]
    fn match_profile_for_lens_string_finds_wide_angle_by_focal_length() {
        let profile = match_profile_for_lens_string("EF16-35mm f/4L IS USM")
            .expect("sollte das Weitwinkel-Profil finden");
        assert_eq!(profile.id, "generic-wide");
    }

    #[test]
    fn match_profile_for_lens_string_is_case_insensitive() {
        let profile = match_profile_for_lens_string("70-200 F/2.8 TELE ZOOM")
            .expect("sollte trotz Großschreibung matchen");
        assert_eq!(profile.id, "generic-tele");
    }

    #[test]
    fn match_profile_for_lens_string_returns_none_when_nothing_matches() {
        assert!(match_profile_for_lens_string("Unbekanntes Objektiv 999mm").is_none());
    }

    /// Phase 12 Schritt 0 Spike (siehe `DECISIONS.md` ADR-0039): prüft, dass
    /// die echte, in der `lensfun`-Crate gebündelte LensFun-Datenbank für ein
    /// real existierendes Weitwinkelobjektiv überhaupt eine Verzeichnungs-
    /// kalibrierung liefert und dass deren Koeffizient in einer plausiblen,
    /// begrenzten Größenordnung liegt (kein `NaN`, keine Ausreißer).
    ///
    /// **Ehrlicher Befund dieses Spikes:** `lensfun`s Poly3-Koeffizient
    /// (`k1 = 0.0128` für dieses Objektiv bei 16mm) folgt einer anderen
    /// Vorzeichen-/Skalierungskonvention als unser bisheriges handgepflegtes
    /// `generic-wide`-Profil (`distortion_k1 = -0.12`, eigene Konvention seit
    /// ADR-0028) — ein direkter Zahlenvergleich zwischen beiden ist daher
    /// **nicht** aussagekräftig. Die echte Umrechnung zwischen den beiden
    /// Konventionen (inkl. `Modifier`s Re-Skalierung auf Bildmaße/Cropfaktor)
    /// ist Aufgabe von Schritt 3 Teil A, nicht dieses Spikes. Dieser Test
    /// verifiziert nur die Grundvoraussetzung dafür: dass die Datenbank für
    /// ein real existierendes Objektiv überhaupt eine sinnvolle Kalibrierung
    /// liefert.
    #[test]
    fn lensfun_bundled_database_has_plausible_distortion_calibration_for_known_lens() {
        let db =
            lensfun::Database::load_bundled().expect("gebündelte LensFun-Datenbank muss laden");
        let cameras = db.find_cameras(Some("Canon"), "EOS 5D Mark III");
        let camera = cameras
            .first()
            .expect("Canon EOS 5D Mark III sollte in der Datenbank enthalten sein");
        let lenses = db.find_lenses(Some(camera), "Canon EF 16-35mm f/4L IS USM");
        let lens = lenses
            .first()
            .expect("Canon EF 16-35mm f/4L IS USM sollte in der Datenbank enthalten sein");
        let calib = lens
            .interpolate_distortion(16.0)
            .expect("Verzeichnungskalibrierung bei 16mm sollte existieren");
        let k1 = match calib.model {
            lensfun::DistortionModel::Poly3 { k1 } => k1,
            lensfun::DistortionModel::Ptlens { a, b, c } => a + b + c,
            other => panic!("unerwartetes Verzeichnungsmodell für dieses Objektiv: {other:?}"),
        };
        assert!(
            k1.is_finite(),
            "Verzeichnungskoeffizient darf nicht NaN/unendlich sein"
        );
        assert!(
            k1 != 0.0 && k1.abs() < 1.0,
            "Verzeichnungskoeffizient eines Weitwinkelobjektivs sollte spürbar \
             aber begrenzt sein (war {k1})"
        );
    }
}
