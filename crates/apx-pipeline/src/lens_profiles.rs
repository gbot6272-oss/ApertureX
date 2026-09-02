//! Objektivprofildatenbank (`SPEC.md` §3.2 „Objektivkorrekturen").
//!
//! **Phase 12 Schritt 3 Teil A (siehe `DECISIONS.md` ADR-0039) hat
//! ADR-0028s ursprüngliche Vereinfachung ersetzt:** statt einer kleinen
//! handgepflegten Liste von drei Beispielprofilen nutzt dieses Modul jetzt
//! die `lensfun`-Crate — ein bit-exakt gegen die C++-Referenzbibliothek
//! getesteter, reiner-Rust-Port der echten, offenen LensFun-Objektiv-
//! datenbank (Tausende real kalibrierte Kamera-/Objektiv-Kombinationen,
//! `Database::load_bundled()` liefert sie direkt eingebettet, kein
//! Laufzeit-Dateisystemzugriff). Das löst weiterhin **nicht** das
//! Adobe-DCP/LCP-Format-Problem (andere, offene Datenquelle statt
//! Adobes proprietärem Format) — siehe ADR-0039 für die volle
//! Begründung, warum das für die praktische Abdeckung trotzdem reicht.
//!
//! **Die drei ursprünglichen Profile (`generic-wide`/`generic-standard`/
//! `generic-tele`) bleiben unter ihrer alten `id` auflösbar** — bereits
//! gespeicherte `profile_id`-Werte in Alt-EDLs dürfen nicht plötzlich ins
//! Leere laufen. [`match_profile_for_lens_string`] nutzt sie aber nur
//! noch als allerletzten Rückfallpfad, wenn die echte Datenbank für den
//! EXIF-Objektivstring gar nichts findet.
//!
//! **Ein-Wert-Modell bleibt bestehen** (`stages::lens_corrections.rs`
//! erwartet weiterhin nur je einen `distortion_k1`/`vignette_amount`/
//! `ca_red_cyan`/`ca_blue_yellow`-Skalar, kein pixelgenaues LUT) — siehe
//! [`derive_lens_correction_values`]s Moduldoku, wie ein einzelner
//! Koeffizienten-Satz ehrlich (nicht geraten) aus LensFuns echter,
//! reichhaltigerer `Modifier`-Pixelmathematik zurückgerechnet wird.

use std::sync::LazyLock;

use serde::Deserialize;

const LEGACY_PROFILE_JSONS: &[&str] = &[
    include_str!("../lens_profiles/generic-wide.json"),
    include_str!("../lens_profiles/generic-standard.json"),
    include_str!("../lens_profiles/generic-tele.json"),
];

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LensProfile {
    pub id: String,
    pub display_name: String,
    /// Nur für die drei Alt-Profile befüllt (Substring-Abgleich, siehe
    /// Moduldoku) — bei LensFun-Treffern leer, die kommen über
    /// [`db().find_lenses`] statt Substring-Vergleich zustande.
    pub matches: Vec<String>,
    pub distortion_k1: f32,
    pub vignette_amount: f32,
    pub ca_red_cyan: f32,
    pub ca_blue_yellow: f32,
}

/// Die drei ursprünglichen handgepflegten Beispielprofile aus Phase 6
/// (ADR-0028) — nur noch für Altbestand-`profile_id`-Auflösung und den
/// allerletzten Rückfallpfad in [`match_profile_for_lens_string`]
/// gebraucht, siehe Moduldoku.
fn legacy_profiles() -> Vec<LensProfile> {
    LEGACY_PROFILE_JSONS
        .iter()
        .map(|json| serde_json::from_str(json).expect("eingebettete Profil-JSON muss gültig sein"))
        .collect()
}

/// Präfix für stabile `profile_id`-Werte echter LensFun-Einträge —
/// `lensfun:{maker}|{model}`, siehe [`lensfun_profile_id`].
const LENSFUN_ID_PREFIX: &str = "lensfun:";

fn lensfun_profile_id(lens: &lensfun::Lens) -> String {
    format!("{LENSFUN_ID_PREFIX}{}|{}", lens.maker, lens.model)
}

fn parse_lensfun_id(id: &str) -> Option<(&str, &str)> {
    id.strip_prefix(LENSFUN_ID_PREFIX)?.split_once('|')
}

/// Die gebündelte LensFun-Datenbank, einmalig geladen (Dekomprimieren +
/// XML-Parsen mehrerer tausend Einträge lohnt sich nicht bei jedem
/// Aufruf) und für die Lebensdauer des Prozesses zwischengespeichert.
fn db() -> &'static lensfun::Database {
    static DB: LazyLock<lensfun::Database> = LazyLock::new(|| {
        lensfun::Database::load_bundled().expect("gebündelte LensFun-Datenbank muss laden")
    });
    &DB
}

// ---- Ein-Wert-Rückrechnung aus LensFuns realer Modifier-Mathematik ------------

/// Repräsentative Bildgröße (3:2-Seitenverhältnis, ein häufiges
/// Vollformat-Sensorformat), an deren Ecke [`derive_lens_correction_values`]
/// auswertet — die tatsächliche Fotoauflösung ist zum Zeitpunkt der
/// Profil-Zuordnung (Zuordnung passiert einmal je Objektiv, nicht je
/// Foto) nicht bekannt, und das Seitenverhältnis beeinflusst die Ecklage
/// stärker als die absolute Pixelzahl.
const REFERENCE_IMAGE_WIDTH: u32 = 3000;
const REFERENCE_IMAGE_HEIGHT: u32 = 2000;
/// Blende, an der die Vignettierungs-Kalibrierung ausgewertet wird, wenn
/// kein spezifischerer Wert vorliegt — ein häufiger mittlerer Blendenwert.
const REFERENCE_APERTURE: f32 = 5.6;
/// Fokusdistanz in Metern für dieselbe Auswertung — ohne EXIF-
/// Fokusdistanz ist „weiter entfernt" der plausibelste Standardfall.
const REFERENCE_DISTANCE: f32 = 10.0;

fn representative_focal(lens: &lensfun::Lens) -> f32 {
    if lens.focal_max > lens.focal_min {
        (lens.focal_min + lens.focal_max) / 2.0
    } else if lens.focal_min > 0.0 {
        lens.focal_min
    } else {
        50.0
    }
}

fn representative_aperture(lens: &lensfun::Lens) -> f32 {
    if lens.aperture_min > 0.0 && lens.aperture_max > 0.0 {
        REFERENCE_APERTURE.clamp(lens.aperture_min, lens.aperture_max)
    } else if lens.aperture_min > 0.0 {
        lens.aperture_min
    } else {
        REFERENCE_APERTURE
    }
}

/// Normierte Bild-Eckkoordinate (`stages::lens_corrections.rs`s eigene
/// `nx`/`ny`-Konvention: `±1` an den Bildkanten, nicht Pixelmitten-
/// versetzt) für eine `width`×`height`-Referenzgröße.
fn reference_corner_normalized() -> (f32, f32) {
    let half_w = REFERENCE_IMAGE_WIDTH as f32 / 2.0;
    let half_h = REFERENCE_IMAGE_HEIGHT as f32 / 2.0;
    let corner_x = (REFERENCE_IMAGE_WIDTH - 1) as f32;
    let corner_y = (REFERENCE_IMAGE_HEIGHT - 1) as f32;
    ((corner_x - half_w) / half_w, (corner_y - half_h) / half_h)
}

/// Rechnet LensFuns reale, pixelgenaue `Modifier`-Korrektur für `lens` an
/// der Ecke eines repräsentativen 3:2-Referenzbilds in unser eigenes
/// Ein-Wert-Shader-Modell zurück (`stages::lens_corrections.rs`s
/// `LensCorrectionParams`).
///
/// **Warum eine Ecke, statt einen LensFun-Koeffizienten direkt zu
/// übernehmen:** LensFuns Modelle (Poly3/Poly5/PTLens für Verzeichnung,
/// mehrgliedrige TCA-/Vignettierungs-Polynome) sind reichhaltiger als
/// unser eigenes r²-Ein-Term-Modell (siehe `lens_corrections.rs`s
/// Moduldoku, bewusste Vereinfachung seit ADR-0028/-0030) — ein
/// Koeffizient ließe sich selbst mit korrekter Einheiten-Umrechnung nicht
/// 1:1 übernehmen, weil die Kurvenform selbst eine andere ist. Statt eine
/// der beiden Kurvenformen zu verwerfen oder einen Wert zu raten, wird
/// LensFuns *echte* `Modifier`-Pixelmathematik — dieselbe, die eine
/// geladene Foto-Korrektur tatsächlich anwenden würde — an der Bildecke
/// ausgewertet, und dann ein einzelner Koeffizient gesucht, der in
/// unserem Modell an derselben Stelle dieselbe Wirkung erzeugt. Das ist
/// eine an LensFuns eigener Berechnung verankerte Näherung, keine
/// geratene Zahl — mit der ehrlichen Grenze, dass sie nur an der
/// Bildecke exakt stimmt, nicht überall im Bild (unser Modell hat dafür
/// schlicht nicht genug Freiheitsgrade).
fn derive_lens_correction_values(lens: &lensfun::Lens) -> (f32, f32, f32, f32) {
    let focal = representative_focal(lens);
    let aperture = representative_aperture(lens);
    let crop = if lens.crop_factor > 0.0 {
        lens.crop_factor
    } else {
        1.0
    };
    let (w, h) = (REFERENCE_IMAGE_WIDTH, REFERENCE_IMAGE_HEIGHT);
    let corner_x = (w - 1) as f32;
    let corner_y = (h - 1) as f32;
    let (nx, ny) = reference_corner_normalized();
    let r2 = nx * nx + ny * ny;

    let mut modifier = lensfun::Modifier::new(lens, focal, crop, w, h, true);
    let has_distortion = modifier.enable_distortion_correction(lens);
    let has_vignetting = modifier.enable_vignetting_correction(lens, aperture, REFERENCE_DISTANCE);
    let has_tca = modifier.enable_tca_correction(lens);

    let distortion_k1 = if has_distortion && r2 > 1e-6 {
        let mut coords = [0.0f32; 2];
        modifier.apply_geometry_distortion(corner_x, corner_y, 1, 1, &mut coords);
        let dnx = (coords[0] - corner_x) / (w as f32 / 2.0);
        let dny = (coords[1] - corner_y) / (h as f32 / 2.0);
        let k1_x = (nx.abs() > 1e-3).then(|| dnx / (nx * r2));
        let k1_y = (ny.abs() > 1e-3).then(|| dny / (ny * r2));
        match (k1_x, k1_y) {
            (Some(a), Some(b)) => (a + b) / 2.0,
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => 0.0,
        }
    } else {
        0.0
    };

    let vignette_amount = if has_vignetting && r2 > 1e-6 {
        let mut pixel = [1.0f32, 1.0, 1.0];
        modifier.apply_color_modification_f32(&mut pixel, corner_x, corner_y, 1, 1, 3);
        // Grünkanal: unbeeinflusst von der separaten TCA-Verschiebung.
        let gain = pixel[1];
        ((gain - 1.0) / (crate::stages::lens_corrections::VIGNETTE_STRENGTH * r2))
            .clamp(-100.0, 100.0)
    } else {
        0.0
    };

    let (ca_red_cyan, ca_blue_yellow) = if has_tca {
        let mut coords = [0.0f32; 6];
        modifier.apply_subpixel_distortion(corner_x, corner_y, 1, 1, &mut coords);
        let half_w = w as f32 / 2.0;
        let half_h = h as f32 / 2.0;
        let radius =
            |x: f32, y: f32| -> f32 { ((x - half_w).powi(2) + (y - half_h).powi(2)).sqrt() };
        let r_g = radius(coords[2], coords[3]).max(1e-3);
        let r_r = radius(coords[0], coords[1]);
        let r_b = radius(coords[4], coords[5]);
        let ca_strength = crate::stages::lens_corrections::CA_STRENGTH;
        (
            ((r_r / r_g - 1.0) * 100.0 / ca_strength).clamp(-100.0, 100.0),
            ((r_b / r_g - 1.0) * 100.0 / ca_strength).clamp(-100.0, 100.0),
        )
    } else {
        (0.0, 0.0)
    };

    (distortion_k1, vignette_amount, ca_red_cyan, ca_blue_yellow)
}

fn lens_profile_from_lensfun(lens: &lensfun::Lens) -> LensProfile {
    let (distortion_k1, vignette_amount, ca_red_cyan, ca_blue_yellow) =
        derive_lens_correction_values(lens);
    LensProfile {
        id: lensfun_profile_id(lens),
        display_name: format!("{} {}", lens.maker, lens.model),
        matches: Vec::new(),
        distortion_k1,
        vignette_amount,
        ca_red_cyan,
        ca_blue_yellow,
    }
}

/// Sucht ein Profil per `id` — entweder ein echter LensFun-Eintrag
/// (`lensfun:{maker}|{model}`, siehe [`lensfun_profile_id`]) oder eines
/// der drei Alt-Profile (`generic-wide`/`generic-standard`/`generic-tele`,
/// für Altbestand-`profile_id`-Werte, siehe Moduldoku).
pub fn find_profile(id: &str) -> Option<LensProfile> {
    if let Some((maker, model)) = parse_lensfun_id(id) {
        let lens = db()
            .lenses
            .iter()
            .find(|l| l.maker == maker && l.model == model)?;
        return Some(lens_profile_from_lensfun(lens));
    }
    legacy_profiles().into_iter().find(|p| p.id == id)
}

/// Ordnet einen EXIF-Objektiv-/Kamerastring einem Profil zu — sucht
/// zuerst in der echten LensFun-Datenbank (`Database::find_lenses`,
/// dieselbe unscharfe Zuordnungslogik wie LensFun selbst: Marke/Modell/
/// Brennweite/Blende werden aus dem String geraten, wenn nicht separat
/// angegeben), fällt nur bei komplett leerem Ergebnis auf die drei
/// Alt-Profile per Substring-Abgleich zurück (siehe Moduldoku).
pub fn match_profile_for_lens_string(lens_or_camera_model: &str) -> Option<LensProfile> {
    if let Some(lens) = db().find_lenses(None, lens_or_camera_model).first() {
        return Some(lens_profile_from_lensfun(lens));
    }

    let haystack = lens_or_camera_model.to_lowercase();
    legacy_profiles().into_iter().find(|profile| {
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
    fn legacy_profiles_parse_and_have_unique_ids() {
        let profiles = legacy_profiles();
        assert_eq!(profiles.len(), 3);
        let mut ids: Vec<&str> = profiles.iter().map(|p| p.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 3, "Profil-IDs müssen eindeutig sein");
    }

    #[test]
    fn find_profile_still_resolves_a_legacy_id() {
        // Altbestand: gespeicherte profile_id-Werte aus Phase 6 (ADR-0028)
        // dürfen nach der Umstellung auf LensFun nicht ins Leere laufen.
        let profile = find_profile("generic-wide").expect("sollte weiterhin existieren");
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
    fn match_profile_for_lens_string_finds_a_real_lensfun_entry_for_a_known_lens() {
        // Phase 12 Schritt 3 (siehe DECISIONS.md ADR-0039): dieselbe
        // Objektivzeichenkette wie der Schritt-0-Spike — die reale
        // Datenbank sollte jetzt Vorrang vor den drei Alt-Profilen haben.
        let profile = match_profile_for_lens_string("EF16-35mm f/4L IS USM")
            .expect("sollte einen echten LensFun-Eintrag finden");
        assert!(
            profile.id.starts_with(LENSFUN_ID_PREFIX),
            "sollte ein echter LensFun-Treffer sein, war: {}",
            profile.id
        );
        assert!(profile.display_name.to_lowercase().contains("canon"));
    }

    #[test]
    fn match_profile_for_lens_string_returns_none_when_nothing_matches() {
        assert!(match_profile_for_lens_string("Unbekanntes Objektiv 999mm").is_none());
    }

    /// Phase 12 Schritt 3 Teil A: `find_profile` muss für eine per
    /// [`match_profile_for_lens_string`] gefundene reale `id` exakt
    /// dasselbe Profil zurückliefern (Rundreise Zuordnung → Speichern der
    /// `id` im EDL → spätere Auflösung beim Rendern).
    #[test]
    fn find_profile_roundtrips_a_real_lensfun_id_from_match_profile_for_lens_string() {
        let matched = match_profile_for_lens_string("EF16-35mm f/4L IS USM")
            .expect("sollte einen echten LensFun-Eintrag finden");
        let resolved = find_profile(&matched.id).expect("sollte über die id auflösbar sein");
        assert_eq!(matched, resolved);
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
    /// ist Aufgabe von Schritt 3 Teil A (siehe
    /// [`derive_lens_correction_values`], inzwischen umgesetzt), nicht
    /// dieses Spikes. Dieser Test verifiziert weiterhin nur die
    /// Grundvoraussetzung dafür: dass die Datenbank für ein real
    /// existierendes Objektiv überhaupt eine sinnvolle Kalibrierung liefert.
    #[test]
    fn lensfun_bundled_database_has_plausible_distortion_calibration_for_known_lens() {
        let cameras = db().find_cameras(Some("Canon"), "EOS 5D Mark III");
        let camera = cameras
            .first()
            .expect("Canon EOS 5D Mark III sollte in der Datenbank enthalten sein");
        let lenses = db().find_lenses(Some(camera), "Canon EF 16-35mm f/4L IS USM");
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
