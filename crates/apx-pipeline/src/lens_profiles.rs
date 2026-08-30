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
}
