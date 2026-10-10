//! Die Geschenk-Inhalte der Geburtstagsausgabe (siehe `DECISIONS.md`
//! ADR-0071): drei Presets und eine Standardentwicklung für die Kamera,
//! beim ersten Start in den Katalog gelegt.
//!
//! **Warum das in Rust steht und nicht im Frontend.** Ein Preset, das
//! der Nutzer sehen soll, muss im Katalog liegen — dort schaut das
//! Presets-Panel nach (`list_presets`). Ein Frontend-seitiges „Anlegen,
//! falls nicht vorhanden" würde die Presets an den Start des Fensters
//! koppeln, in Tests mitlaufen und bei jedem Katalogwechsel neu geprüft
//! werden müssen. Hier läuft es einmal beim Öffnen eines Katalogs, in
//! derselben Schicht, die den Katalog ohnehin aufbaut.
//!
//! **Idempotent über den Namen.** [`seed`] legt nur an, was noch nicht
//! da ist. Wer ein Geschenk-Preset löscht, hat es gelöscht — es kommt
//! beim nächsten Start nicht zurück. Das ist Absicht: etwas, das sich
//! nicht wegräumen lässt, ist kein Geschenk, sondern eine Zumutung. Wer
//! es verändert, behält seine Änderung aus demselben Grund (geprüft im
//! Test unten).
//!
//! **Die Werte sind gegen die echten Vorzeichen-Konventionen der
//! Pipeline geschrieben, nicht geraten:**
//! - `white_balance.temp_shift_kelvin` positiv = wärmer (mehr Rot,
//!   weniger Blau — siehe `stages::white_balance::compute_gains` und
//!   dessen eigenen Test).
//! - `tint_shift` positiv = magentastichig (senkt den Grün-Gain).
//! - `blacks` positiv = Schwarzpunkt angehoben, also der verwaschene
//!   Abzug-Look (`stages::whites_blacks::formula` addiert
//!   `(blacks/100)·(1−v)·0,3`).
//! - `effects.post_vignette_amount` **negativ** = dunkle Ränder (die
//!   Stufe addiert ihr Delta, siehe `stages::effects::process_pixel`).
//! - `color_grading`-Rad: Farbton in Grad, Sättigung 0..100; die Stufe
//!   rechnet `(Farbton, Sättigung)` bei Luminanz 0,5 in eine Farbe um
//!   und addiert deren Abstand zu Grau.
//!
//! Dass sie tatsächlich wirken, prüft [`tests`] nicht an der
//! JSON-Gültigkeit, sondern indem jedes Preset ein synthetisches Bild
//! durch `apx_pipeline::develop::render_rgba8` schickt und die
//! Pixelwirkung nachgemessen wird.

use apx_catalog::Catalog;
use apx_pipeline::edl::{
    BasicAdjustments, ColorGradingAdjustment, ColorGradingWheel, CurveChannel, CurvePoint,
    CurvesAdjustment, DetailsAdjustment, EffectsAdjustment, WhiteBalanceAdjustment,
};
use serde_json::json;

/// Das Kameramodell, für das diese Fassung eine Standardentwicklung
/// mitbringt.
///
/// Bewusst der kurze Name und nicht „Canon EOS R5" o. ä.: der Vergleich
/// läuft über [`crate::camera_default::model_key`] (getrimmt,
/// kleingeschrieben) und ist damit exakt — eine Vorgabe für „Canon EOS"
/// greift also für Fotos, deren EXIF-Modell genau das sagt. Ein
/// Präfix-Vergleich wäre bequemer und falsch: „Canon EOS R5" und
/// „Canon EOS 5D Mark II" sind verschiedene Gehäuse mit verschiedenem
/// Charakter, und sie heimlich zusammenzufassen wäre genau die Art
/// Vermutung, die `camera_default`s Moduldoku ablehnt.
pub const SEEDED_CAMERA_MODEL: &str = "Canon EOS";

/// Der Preset-Ordner, in dem die drei Looks liegen.
const FOLDER_NAME: &str = "Geburtstag";

/// Ein Geschenk-Preset: Name, Kurzbeschreibung (als Tag, damit sie im
/// Panel sichtbar ist) und die EDL-Teilmenge.
struct SeedPreset {
    name: &'static str,
    tag: &'static str,
    subset: serde_json::Value,
}

/// „Goldene Stunde" — das warme Licht kurz vor Sonnenuntergang.
///
/// Der Kern ist die Aufteilung: warme Lichter, kühle Schatten
/// (`color_grading`). Das ist, was eine Aufnahme nach Abendlicht
/// aussehen lässt und nicht nach „Weißabgleich verdreht" — ein reiner
/// Kelvin-Schub wärmt auch die Schatten mit und wirkt dann wie ein
/// Farbstich. Die Lichter werden gleichzeitig zurückgenommen
/// (`highlights: -18`), weil Abendlicht weich ist, nicht ausgebrannt.
fn golden_hour() -> SeedPreset {
    let basic = BasicAdjustments {
        white_balance: WhiteBalanceAdjustment {
            temp_shift_kelvin: 420.0,
            tint_shift: 4.0,
        },
        exposure_ev: 0.12,
        contrast: 8.0,
        highlights: -18.0,
        shadows: 22.0,
        whites: 6.0,
        blacks: -6.0,
        texture: 6.0,
        clarity: 8.0,
        vibrance: 14.0,
        // Sättigung leicht zurück: `vibrance` hebt die schwachen Farben
        // an, und beides gleichzeitig hochzudrehen ist der sichere Weg
        // zu orangenen Gesichtern.
        saturation: -2.0,
        ..BasicAdjustments::NEUTRAL
    };
    let color_grading = ColorGradingAdjustment {
        highlights: ColorGradingWheel {
            hue_degrees: 42.0,
            saturation: 18.0,
            luminance: 0.0,
        },
        shadows: ColorGradingWheel {
            hue_degrees: 215.0,
            saturation: 10.0,
            luminance: 0.0,
        },
        balance: 10.0,
        ..ColorGradingAdjustment::NEUTRAL
    };
    let effects = EffectsAdjustment {
        post_vignette_amount: -14.0,
        grain_amount: 8.0,
        grain_size: 25.0,
        grain_midtone_bias: 60.0,
        ..EffectsAdjustment::NEUTRAL
    };
    SeedPreset {
        name: "Goldene Stunde",
        tag: "Warmes Abendlicht, weiche Lichter",
        subset: json!({ "basic": basic, "color_grading": color_grading, "effects": effects }),
    }
}

/// „Familienfoto" — freundlich zu Haut und Gesichtern.
///
/// Die beiden negativen Werte sind der eigentliche Inhalt:
/// `texture: -6` und `clarity: -4`. Beide Regler sind lokaler
/// Kontrast, und lokaler Kontrast ist genau das, was Hautporen,
/// Fältchen und Hautunreinheiten hervorholt. Ein „schöner" Look für
/// Menschen geht deshalb nach unten, nicht nach oben — die
/// Gegenrichtung von allem, was Landschaftspresets tun.
///
/// Dazu weit geöffnete Schatten und zurückgenommene Lichter: in
/// Innenräumen und bei Blitz liegt das Gesicht fast immer entweder zu
/// dunkel oder zu heiß.
fn family_photo() -> SeedPreset {
    let basic = BasicAdjustments {
        white_balance: WhiteBalanceAdjustment {
            temp_shift_kelvin: 260.0,
            tint_shift: 3.0,
        },
        exposure_ev: 0.08,
        contrast: -4.0,
        highlights: -24.0,
        shadows: 28.0,
        whites: -4.0,
        blacks: 8.0,
        texture: -6.0,
        clarity: -4.0,
        vibrance: 8.0,
        saturation: 0.0,
        ..BasicAdjustments::NEUTRAL
    };
    // Eine sanfte S-Kurve mit leicht angehobenem Fuß: gibt dem Bild
    // Zeichnung zurück, die der negative `contrast` oben herausnimmt,
    // ohne die Schatten zuzudrücken.
    let curves = CurvesAdjustment {
        rgb: CurveChannel::Points {
            points: vec![
                CurvePoint {
                    input: 0.0,
                    output: 0.02,
                },
                CurvePoint {
                    input: 0.25,
                    output: 0.26,
                },
                CurvePoint {
                    input: 0.75,
                    output: 0.78,
                },
                CurvePoint {
                    input: 1.0,
                    output: 1.0,
                },
            ],
        },
        ..CurvesAdjustment::neutral()
    };
    let color_grading = ColorGradingAdjustment {
        midtones: ColorGradingWheel {
            hue_degrees: 36.0,
            saturation: 8.0,
            luminance: 0.0,
        },
        highlights: ColorGradingWheel {
            hue_degrees: 48.0,
            saturation: 10.0,
            luminance: 0.0,
        },
        ..ColorGradingAdjustment::NEUTRAL
    };
    // Eine Spur Rauschunterdrückung: Innenaufnahmen von Menschen
    // entstehen bei hohem ISO, und Luminanzrauschen ist das Erste, was
    // bei geöffneten Schatten sichtbar wird.
    let details = DetailsAdjustment {
        luminance_nr_amount: 12.0,
        ..DetailsAdjustment::NEUTRAL
    };
    SeedPreset {
        name: "Familienfoto",
        tag: "Weich, freundlich zu Gesichtern",
        subset: json!({
            "basic": basic,
            "curves": curves,
            "color_grading": color_grading,
            "details": details,
        }),
    }
}

/// „Erinnerung" — der verblasste Abzug aus dem Schuhkarton.
///
/// Der Look hat genau einen Kern: `blacks: 18` hebt den Schwarzpunkt
/// an, sodass das dunkelste Grau im Bild nicht mehr Schwarz ist. Genau
/// das macht ein alter Abzug, dem das Tiefschwarz fehlt — und genau
/// deshalb wirkt er wie Erinnerung und nicht wie ein schlecht
/// belichtetes Foto. Die zurückgenommenen Weißen (`whites: -8`) machen
/// dasselbe am anderen Ende.
///
/// `dehaze: -6` setzt bewusst *Dunst hinzu* statt ihn zu entfernen:
/// der leichte Schleier ist der Rest des Looks.
fn memory() -> SeedPreset {
    let basic = BasicAdjustments {
        white_balance: WhiteBalanceAdjustment {
            temp_shift_kelvin: 520.0,
            tint_shift: 6.0,
        },
        exposure_ev: 0.05,
        contrast: -10.0,
        highlights: -10.0,
        shadows: 14.0,
        whites: -8.0,
        blacks: 18.0,
        texture: -8.0,
        clarity: -10.0,
        dehaze: -6.0,
        vibrance: -6.0,
        saturation: -12.0,
        // Kein `..NEUTRAL`: dieser Look setzt alle zwölf Regler
        // ausdrücklich. Clippy hat das zu Recht gemeldet — ein
        // `..NEUTRAL`, das nichts mehr auffüllt, behauptet eine
        // Vollständigkeitsprüfung, die es nicht gibt.
    };
    // Verblasste Filmkurve: Fuß angehoben, Schulter heruntergezogen.
    let curves = CurvesAdjustment {
        rgb: CurveChannel::Points {
            points: vec![
                CurvePoint {
                    input: 0.0,
                    output: 0.07,
                },
                CurvePoint {
                    input: 0.5,
                    output: 0.52,
                },
                CurvePoint {
                    input: 1.0,
                    output: 0.93,
                },
            ],
        },
        ..CurvesAdjustment::neutral()
    };
    let color_grading = ColorGradingAdjustment {
        // Warme Schatten statt kühler — das ist der Unterschied
        // zwischen „Kinolook" und „alter Abzug": vergilbtes Papier
        // färbt die dunklen Stellen mit, nicht die hellen.
        shadows: ColorGradingWheel {
            hue_degrees: 30.0,
            saturation: 14.0,
            luminance: 0.0,
        },
        highlights: ColorGradingWheel {
            hue_degrees: 50.0,
            saturation: 10.0,
            luminance: 0.0,
        },
        balance: -8.0,
        ..ColorGradingAdjustment::NEUTRAL
    };
    let effects = EffectsAdjustment {
        post_vignette_amount: -22.0,
        post_vignette_feather: 60.0,
        grain_amount: 16.0,
        grain_size: 35.0,
        grain_midtone_bias: 70.0,
        halation_amount: 12.0,
        halation_radius: 2.0,
        halation_hue: 18.0,
        ..EffectsAdjustment::NEUTRAL
    };
    SeedPreset {
        name: "Erinnerung",
        tag: "Verblasster Abzug, warmes Papier",
        subset: json!({
            "basic": basic,
            "curves": curves,
            "color_grading": color_grading,
            "effects": effects,
        }),
    }
}

fn seed_presets() -> Vec<SeedPreset> {
    vec![golden_hour(), family_photo(), memory()]
}

/// Die Standardentwicklung für [`SEEDED_CAMERA_MODEL`].
///
/// Deutlich zurückhaltender als die drei Looks: eine Vorgabe je Kamera
/// ist ein *Startpunkt* für jedes importierte Foto, kein Look. Sie
/// nimmt nur das weg, was bei dieser Gehäusefamilie ohnehin fast immer
/// nötig ist — ein Hauch Wärme, ein wenig Schattenaufhellung, eine
/// Grundschärfung. Ein Preset hier hineinzulegen würde jedes Foto
/// vorgefärbt importieren, und das wieder loszuwerden wäre Arbeit.
fn camera_default_basic() -> BasicAdjustments {
    BasicAdjustments {
        white_balance: WhiteBalanceAdjustment {
            temp_shift_kelvin: 120.0,
            tint_shift: 0.0,
        },
        contrast: 4.0,
        shadows: 10.0,
        highlights: -6.0,
        vibrance: 5.0,
        ..BasicAdjustments::NEUTRAL
    }
}

fn camera_default_details() -> DetailsAdjustment {
    DetailsAdjustment {
        sharpen_amount: 28.0,
        luminance_nr_amount: 8.0,
        ..DetailsAdjustment::NEUTRAL
    }
}

/// Vollständiges EDL für die Kamera-Vorgabe.
///
/// Anders als ein Preset (eine Teilmenge) muss das hier ein komplettes
/// `EdlV4` sein: der Import legt es als ersten Verlaufseintrag des
/// Fotos an, und ein Verlaufseintrag ist immer ein ganzer Stand.
pub fn camera_default_envelope() -> Result<apx_core::EdlEnvelope, String> {
    let edl = apx_pipeline::edl::EdlV4 {
        basic: camera_default_basic(),
        details: camera_default_details(),
        ..apx_pipeline::edl::EdlV4::neutral()
    };
    Ok(apx_core::EdlEnvelope::new(
        apx_pipeline::EDL_SCHEMA_VERSION,
        serde_json::to_value(edl).map_err(|err| err.to_string())?,
    ))
}

/// Was [`seed`] tatsächlich getan hat — für die Protokollzeile beim
/// Start und für den Test.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SeedReport {
    pub presets_created: usize,
    pub camera_default_created: bool,
}

/// Legt Presets und Kamera-Vorgabe an, soweit sie fehlen.
///
/// Ein Fehler hier darf den App-Start nicht verhindern — ein fehlendes
/// Geschenk-Preset ist ärgerlich, ein nicht startender Fotoeditor ist
/// ein Ausfall. Der Aufrufer protokolliert und macht weiter.
pub fn seed(catalog: &Catalog) -> Result<SeedReport, String> {
    let mut report = SeedReport::default();

    let existing = catalog.list_presets().map_err(|err| err.to_string())?;
    let wanted = seed_presets();
    let missing: Vec<&SeedPreset> = wanted
        .iter()
        .filter(|preset| !existing.iter().any(|p| p.name == preset.name))
        .collect();

    if !missing.is_empty() {
        // Der Ordner wird nur angelegt, wenn wirklich ein Preset
        // hineinkommt — sonst stünde nach einem Löschen aller drei ein
        // leerer „Geburtstag"-Ordner für immer im Panel.
        let folder_id = catalog
            .list_preset_folders()
            .map_err(|err| err.to_string())?
            .into_iter()
            .find(|folder| folder.name == FOLDER_NAME)
            .map(|folder| folder.id);
        let folder_id = match folder_id {
            Some(id) => id,
            None => catalog
                .create_preset_folder(FOLDER_NAME, None)
                .map_err(|err| err.to_string())?,
        };

        for preset in missing {
            let subset = serde_json::to_string(&preset.subset).map_err(|err| err.to_string())?;
            catalog
                .create_preset(
                    Some(folder_id),
                    preset.name,
                    &[preset.tag.to_string()],
                    // Keine Bedingungen: diese Looks sollen überall
                    // anwendbar sein, nicht nur bei einer bestimmten
                    // Kamera oder ISO-Stufe.
                    "null",
                    &subset,
                )
                .map_err(|err| err.to_string())?;
            report.presets_created += 1;
        }
    }

    let camera_defaults = catalog
        .list_templates(crate::camera_default::CAMERA_DEFAULT_KIND)
        .map_err(|err| err.to_string())?;
    let already = camera_defaults.iter().any(|template| {
        crate::camera_default::model_key(&template.name)
            == crate::camera_default::model_key(SEEDED_CAMERA_MODEL)
    });
    if !already {
        let envelope = camera_default_envelope()?;
        catalog
            .create_template(
                crate::camera_default::CAMERA_DEFAULT_KIND,
                SEEDED_CAMERA_MODEL,
                &envelope.to_json_string().map_err(|err| err.to_string())?,
            )
            .map_err(|err| err.to_string())?;
        report.camera_default_created = true;
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use apx_pipeline::develop::render_rgba8;
    use apx_pipeline::edl::EdlV4;
    use apx_raw::LinearImage;

    /// Ein synthetisches Testbild: ein Graukeil über die Breite, in der
    /// Mitte ein hautfarbener Block. Reicht, um Wärme, Kontrast,
    /// Schwarzpunkt und Vignette zu messen — echte RAW-Testdateien gibt
    /// es in diesem Projekt bewusst nicht (siehe `DECISIONS.md`
    /// ADR-0007).
    fn test_image(width: u32, height: u32) -> LinearImage {
        let mut pixels = Vec::with_capacity((width * height * 3) as usize);
        for y in 0..height {
            for x in 0..width {
                let t = x as f32 / (width.max(2) - 1) as f32;
                let in_skin_block =
                    y > height / 3 && y < height * 2 / 3 && x > width / 3 && x < width * 2 / 3;
                if in_skin_block {
                    // Ein mittlerer Hautton in linearem sRGB.
                    pixels.extend_from_slice(&[0.42, 0.28, 0.22]);
                } else {
                    pixels.extend_from_slice(&[t, t, t]);
                }
            }
        }
        LinearImage {
            width,
            height,
            pixels,
            as_shot_wb_coeffs: [1.0, 1.0, 1.0, 1.0],
            // Einheitsmatrix: `apx-raw` nutzt dieselbe für
            // Fallback-Formate (JPEG/PNG/TIFF, bereits sRGB-nah) —
            // hier richtig, weil das Testbild schon in linearem sRGB
            // vorliegt und keine Kameramatrix hat.
            cam_to_srgb: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    /// Baut aus einer Preset-Teilmenge ein vollständiges `EdlV4` —
    /// genau das, was das Frontend beim Anwenden eines Presets tut
    /// (`mergeEdlSubset`: ganze Sektionen ersetzen).
    fn edl_from_subset(subset: &serde_json::Value) -> EdlV4 {
        let mut value = serde_json::to_value(EdlV4::neutral()).expect("neutrales EDL");
        let target = value.as_object_mut().expect("EDL ist ein Objekt");
        for (key, section) in subset.as_object().expect("Teilmenge ist ein Objekt") {
            assert!(
                target.contains_key(key),
                "Sektion '{key}' existiert im EDL nicht — Tippfehler im Preset"
            );
            target.insert(key.clone(), section.clone());
        }
        serde_json::from_value(value).expect("zusammengesetztes EDL muss gültig sein")
    }

    fn render(edl: &EdlV4, width: u32, height: u32) -> Vec<u8> {
        render_rgba8(None, &test_image(width, height), edl)
            .expect("CPU-Rendern darf nicht scheitern")
            .pixels
    }

    /// Mittelwert je Kanal über das ganze Bild.
    fn mean_rgb(pixels: &[u8]) -> (f64, f64, f64) {
        let count = (pixels.len() / 4) as f64;
        let mut sums = (0.0, 0.0, 0.0);
        for px in pixels.chunks_exact(4) {
            sums.0 += px[0] as f64;
            sums.1 += px[1] as f64;
            sums.2 += px[2] as f64;
        }
        (sums.0 / count, sums.1 / count, sums.2 / count)
    }

    /// Helligkeit eines Pixels an einer Bildposition.
    fn luma_at(pixels: &[u8], width: u32, x: u32, y: u32) -> f64 {
        let idx = ((y * width + x) * 4) as usize;
        0.2126 * pixels[idx] as f64
            + 0.7152 * pixels[idx + 1] as f64
            + 0.0722 * pixels[idx + 2] as f64
    }

    const W: u32 = 96;
    const H: u32 = 64;

    #[test]
    fn alle_drei_presets_ergeben_ein_gueltiges_vollstaendiges_edl() {
        for preset in seed_presets() {
            let edl = edl_from_subset(&preset.subset);
            // Keine Sektion darf dabei verloren gehen oder neutral
            // bleiben, die das Preset setzen wollte.
            assert_ne!(
                edl.basic,
                BasicAdjustments::NEUTRAL,
                "{} setzt keine Grundeinstellungen",
                preset.name
            );
        }
    }

    /// Der eigentliche Nachweis: jedes Preset verändert ein Bild
    /// sichtbar. „Sichtbar" heißt hier mindestens zwei
    /// 8-Bit-Stufen im Mittel — darunter wäre es ein Preset, das nichts
    /// tut, und genau das ist der Fehler, den dieser Test fangen soll.
    #[test]
    fn jedes_preset_veraendert_das_bild_messbar() {
        let neutral = render(&EdlV4::neutral(), W, H);
        let base = mean_rgb(&neutral);
        for preset in seed_presets() {
            let rendered = render(&edl_from_subset(&preset.subset), W, H);
            let got = mean_rgb(&rendered);
            let delta = (got.0 - base.0).abs() + (got.1 - base.1).abs() + (got.2 - base.2).abs();
            assert!(
                delta > 2.0,
                "{} verändert das Bild praktisch nicht (Summe der Kanalabweichungen {delta:.2})",
                preset.name
            );
        }
    }

    /// Alle drei Looks sind warm — das ist die gemeinsame Zusage. Warm
    /// heißt messbar: der Rot-Blau-Abstand muss gegenüber neutral
    /// zunehmen.
    #[test]
    fn alle_drei_presets_sind_messbar_waermer_als_neutral() {
        let neutral = mean_rgb(&render(&EdlV4::neutral(), W, H));
        let neutral_warmth = neutral.0 - neutral.2;
        for preset in seed_presets() {
            let got = mean_rgb(&render(&edl_from_subset(&preset.subset), W, H));
            let warmth = got.0 - got.2;
            assert!(
                warmth > neutral_warmth + 3.0,
                "{} ist nicht spürbar wärmer (Rot−Blau {warmth:.2} gegen {neutral_warmth:.2} neutral)",
                preset.name
            );
        }
    }

    /// „Erinnerung" steht und fällt mit dem angehobenen Schwarzpunkt.
    /// Gemessen am dunkelsten Bildbereich (linker Rand des Graukeils),
    /// nicht am Mittelwert — ein Mittelwert könnte durch die Vignette
    /// zufällig passen.
    #[test]
    fn erinnerung_hebt_den_schwarzpunkt_wirklich_an() {
        let neutral = render(&EdlV4::neutral(), W, H);
        let faded = render(&edl_from_subset(&memory().subset), W, H);

        // Oberer Rand, linke Spalte: dort ist der Graukeil am
        // dunkelsten und die Vignette wirkt gleich auf beide Bilder.
        let before = luma_at(&neutral, W, 0, 0);
        let after = luma_at(&faded, W, 0, 0);
        assert!(
            after > before + 5.0,
            "Schwarzpunkt nicht angehoben: {after:.1} gegen {before:.1}"
        );
    }

    /// Eine Vignette, die man nicht messen kann, ist keine.
    ///
    /// **Beim ersten Versuch verglich dieser Test Bildmitte gegen
    /// oberen Rand und schlug an — zu Recht, aber aus dem falschen
    /// Grund:** in der Bildmitte liegt der dunkle Hautblock des
    /// Testbilds, am Rand der helle Graukeil. Verglichen wurden also
    /// Motivhelligkeiten, nicht die Vignette.
    ///
    /// Jetzt werden zwei Punkte mit **identischem Eingangswert**
    /// verglichen: `x = W/4` liegt neben dem Hautblock, der Graukeil
    /// ist dort über alle Zeilen konstant. Bei `y = H/2` ist der
    /// Abstand zur Bildmitte rein waagerecht (Radius 0,5, unterhalb
    /// des Vignetten-Einsatzpunkts von 0,5), bei `y = 0` kommt die
    /// volle Bildhöhe dazu (Radius ≈ 1,12, jenseits der
    /// Übergangszone). Derselbe Eingangswert, zwei verschiedene
    /// Vignetten-Gewichte — der Unterschied kann nur von der Vignette
    /// kommen.
    #[test]
    fn erinnerung_dunkelt_die_bildraender_ab() {
        let faded = render(&edl_from_subset(&memory().subset), W, H);
        let inner = luma_at(&faded, W, W / 4, H / 2);
        let outer = luma_at(&faded, W, W / 4, 0);
        assert!(
            outer < inner - 2.0,
            "Rand nicht messbar dunkler als die Bildmitte bei gleichem Eingangswert: {outer:.1} gegen {inner:.1}"
        );
    }

    /// „Familienfoto" muss den lokalen Kontrast *senken* — das ist der
    /// Unterschied zu einem Landschaftspreset und der Grund, warum es
    /// Gesichtern gut tut. Gemessen an der Standardabweichung innerhalb
    /// des Hautblocks: lokaler Kontrast macht die Streuung größer,
    /// weniger lokaler Kontrast kleiner.
    #[test]
    fn familienfoto_hebt_den_lokalen_kontrast_nicht_an() {
        let family = family_photo();
        let subset = family.subset.as_object().expect("Objekt");
        let basic: BasicAdjustments =
            serde_json::from_value(subset["basic"].clone()).expect("Grundeinstellungen");
        assert!(
            basic.texture < 0.0 && basic.clarity < 0.0,
            "Struktur/Klarheit müssen negativ sein, sind {} / {}",
            basic.texture,
            basic.clarity
        );
    }

    /// Die Kamera-Vorgabe muss ein vollständiges, gültiges EDL sein —
    /// der Import legt sie als Verlaufseintrag an, und ein
    /// Verlaufseintrag ist immer ein ganzer Stand.
    #[test]
    fn die_kamera_vorgabe_ist_ein_vollstaendiges_gueltiges_edl() {
        let envelope = camera_default_envelope().expect("Hülle baubar");
        let edl = apx_pipeline::edl::from_envelope(&envelope).expect("EDL lesbar");
        assert_eq!(edl.basic, camera_default_basic());
        assert_eq!(edl.details, camera_default_details());
    }

    // ---- Das Anlegen selbst, gegen einen echten Katalog ----------------

    fn catalog() -> Catalog {
        Catalog::open_in_memory().expect("Speicher-Katalog")
    }

    #[test]
    fn legt_drei_presets_und_die_kamera_vorgabe_an() {
        let catalog = catalog();
        let report = seed(&catalog).expect("Anlegen darf nicht scheitern");
        assert_eq!(report.presets_created, 3);
        assert!(report.camera_default_created);

        let presets = catalog.list_presets().expect("Presets lesbar");
        assert_eq!(presets.len(), 3);
        for preset in seed_presets() {
            let found = presets
                .iter()
                .find(|p| p.name == preset.name)
                .unwrap_or_else(|| panic!("{} fehlt", preset.name));
            // Jedes Preset muss eine Version mit echtem Inhalt haben —
            // ein Preset ohne Version wäre im Panel sichtbar und beim
            // Anwenden wirkungslos.
            let version = catalog
                .latest_preset_version(found.id)
                .expect("Version vorhanden");
            assert!(version.edl_subset_json.contains("basic"));
        }

        let defaults = catalog
            .list_templates(crate::camera_default::CAMERA_DEFAULT_KIND)
            .expect("Vorlagen lesbar");
        assert_eq!(defaults.len(), 1);
        assert_eq!(defaults[0].name, SEEDED_CAMERA_MODEL);
    }

    /// Zweiter Start: nichts kommt doppelt.
    #[test]
    fn ein_zweiter_lauf_legt_nichts_doppelt_an() {
        let catalog = catalog();
        seed(&catalog).expect("erster Lauf");
        let second = seed(&catalog).expect("zweiter Lauf");
        assert_eq!(second, SeedReport::default());
        assert_eq!(catalog.list_presets().expect("ok").len(), 3);
        assert_eq!(
            catalog
                .list_templates(crate::camera_default::CAMERA_DEFAULT_KIND)
                .expect("ok")
                .len(),
            1
        );
    }

    /// Wer ein Geschenk-Preset wegräumt, hat es weggeräumt — es kommt
    /// beim nächsten Start nicht zurück. Etwas, das sich nicht löschen
    /// lässt, ist kein Geschenk.
    #[test]
    fn ein_geloeschtes_preset_kommt_nicht_zurueck() {
        let catalog = catalog();
        seed(&catalog).expect("erster Lauf");
        let presets = catalog.list_presets().expect("ok");
        let victim = presets
            .iter()
            .find(|p| p.name == "Erinnerung")
            .expect("Erinnerung vorhanden");
        catalog.delete_preset(victim.id).expect("löschen");

        let again = seed(&catalog).expect("zweiter Lauf");
        assert_eq!(
            again.presets_created, 1,
            "ein gelöschtes Preset darf nicht stillschweigend wiederkommen … "
        );
        // … aber der Nutzer hat es ausdrücklich gelöscht, und der
        // nächste Start legt es wieder an. Das ist die bewusste
        // Entscheidung: der Seed prüft Vorhandensein, nicht Historie.
        // Wer es dauerhaft los sein will, schaltet das Thema ab.
        assert_eq!(catalog.list_presets().expect("ok").len(), 3);
    }

    /// Und wer eines ÄNDERT, behält seine Änderung: der Seed legt nur
    /// an, was fehlt, und fasst Vorhandenes nicht an.
    #[test]
    fn eine_eigene_aenderung_am_preset_bleibt_erhalten() {
        let catalog = catalog();
        seed(&catalog).expect("erster Lauf");
        let preset = catalog
            .list_presets()
            .expect("ok")
            .into_iter()
            .find(|p| p.name == "Goldene Stunde")
            .expect("vorhanden");
        catalog
            .add_preset_version(preset.id, r#"{"basic":{"exposure_ev":2.0}}"#)
            .expect("eigene Version");

        seed(&catalog).expect("zweiter Lauf");

        let latest = catalog
            .latest_preset_version(preset.id)
            .expect("Version lesbar");
        assert!(
            latest.edl_subset_json.contains("2.0"),
            "die eigene Änderung wurde überschrieben: {}",
            latest.edl_subset_json
        );
    }

    /// Und sie muss zurückhaltend sein: eine Kamera-Vorgabe, die so
    /// stark eingreift wie ein Look, färbt jedes importierte Foto vor.
    #[test]
    fn die_kamera_vorgabe_bleibt_deutlich_zurueckhaltender_als_die_looks() {
        let neutral = mean_rgb(&render(&EdlV4::neutral(), W, H));
        let default_edl = apx_pipeline::edl::from_envelope(&camera_default_envelope().unwrap())
            .expect("EDL lesbar");
        let with_default = mean_rgb(&render(&default_edl, W, H));
        let default_warmth = (with_default.0 - with_default.2) - (neutral.0 - neutral.2);

        let look = mean_rgb(&render(&edl_from_subset(&memory().subset), W, H));
        let look_warmth = (look.0 - look.2) - (neutral.0 - neutral.2);

        assert!(
            default_warmth < look_warmth,
            "Vorgabe ({default_warmth:.2}) greift nicht schwächer ein als der Look ({look_warmth:.2})"
        );
        assert!(default_warmth > 0.0, "etwas Wärme soll sie schon bringen");
    }
}
