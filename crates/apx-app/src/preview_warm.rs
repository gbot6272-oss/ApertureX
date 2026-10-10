//! Vorschauen für einen Ordner vorbereiten (Phase 34 F10, siehe
//! `DECISIONS.md` ADR-0070).
//!
//! **Der Befund, der dahinter steht.** Der Import legt für jedes Foto
//! eine 256px-Miniaturansicht an (`import::thumbnails`) — die reicht
//! fürs Raster. Die 2048px-Stufe (`PreviewLevel::Standard`), von der
//! das Durchblättern im Einzelbild lebt, wird dagegen *nie* im Voraus
//! erzeugt: `protocol::compute_preview` liest den Cache, findet nichts
//! und dekodiert die RAW-Datei — jedes Mal, für jedes Foto, beim ersten
//! Ansehen. Bei einem 45-Megapixel-RAW sind das ein bis zwei Sekunden
//! pro Bild, und zwar genau dann, wenn man durch einen frisch
//! importierten Ordner blättert und gerade *nicht* warten will.
//!
//! Dieses Modul füllt genau diese Lücke vorab. Es erfindet keinen
//! zweiten Cache: es schreibt in denselben, den `compute_preview` schon
//! liest, mit demselben Pfadschema wie `import::thumbnails` — nur eine
//! Stufe höher. Danach ist der erste Blick auf ein Foto ein Dateiabruf
//! statt einer Dekodierung.
//!
//! Was hier rein gerechnet wird (und deshalb testbar ist), ist der
//! Cache-Pfad und die Frage, für welche Fotos überhaupt Arbeit anfällt.
//! Das Dekodieren selbst steht in [`warm_one`].

use std::path::{Path, PathBuf};

use apx_catalog::{Catalog, PreviewLevel};
use apx_core::PhotoId;

/// Lange Kante der vorbereiteten Vorschau.
///
/// Bewusst **dieselbe Konstante**, die `protocol::compute_preview` für
/// `PreviewLevel::Standard` ausliefert, statt einer eigenen Zahl: eine
/// gespiegelte Konstante wäre irgendwann abgedriftet, und dann bereitete
/// dieses Modul etwas vor, das niemand abruft.
pub(crate) const WARM_EDGE: u32 = crate::protocol::STANDARD_EDGE;

/// Ein Foto, das vorbereitet werden könnte.
pub(crate) struct WarmCandidate {
    pub photo_id: PhotoId,
    pub source_path: PathBuf,
    /// Ob für dieses Foto schon eine gültige Vorschau im Cache liegt.
    pub cached: bool,
}

pub(crate) struct WarmPlan {
    /// Fotos, für die gerechnet werden muss.
    pub pending: Vec<(PhotoId, PathBuf)>,
    /// Fotos, die schon vorbereitet sind.
    pub already: usize,
}

/// Trennt, was zu tun ist, von dem, was schon getan ist.
///
/// `force` erzwingt die Neuberechnung auch für vorhandene Vorschauen —
/// der Weg zurück, wenn der Cache einmal Unsinn enthält.
pub(crate) fn plan_warm(candidates: Vec<WarmCandidate>, force: bool) -> WarmPlan {
    let mut pending = Vec::new();
    let mut already = 0usize;
    for candidate in candidates {
        if candidate.cached && !force {
            already += 1;
        } else {
            pending.push((candidate.photo_id, candidate.source_path));
        }
    }
    WarmPlan { pending, already }
}

/// Der Cache-Pfad einer vorbereiteten Vorschau.
///
/// Gleiches Schema wie `import::thumbnails::thumbnail_cache_path`
/// (zweistelliges Präfix-Unterverzeichnis gegen zu viele Dateien in
/// einem Ordner), nur mit der Stufennummer `1` statt `0` — derselbe
/// Cache, andere Stufe.
pub(crate) fn warm_cache_path(cache_root: &Path, photo_id: PhotoId) -> PathBuf {
    let id = photo_id.to_string();
    let prefix: String = id.chars().take(2).collect();
    cache_root.join(prefix).join(format!("{id}_1.jpg"))
}

/// Bereitet die Vorschau eines Fotos vor: dekodieren, als JPEG in den
/// Cache schreiben, im Katalog vermerken.
///
/// Reihenfolge Datei-vor-Katalogzeile wie überall sonst: ein
/// Katalogeintrag, der auf eine nicht geschriebene Datei zeigt, wäre
/// schlimmer als eine Datei, die niemand kennt — die erste Variante
/// lässt `compute_preview` ins Leere greifen, die zweite kostet nur
/// Plattenplatz und wird beim nächsten Lauf überschrieben.
pub(crate) fn warm_one(
    catalog: &Catalog,
    cache_root: &Path,
    photo_id: PhotoId,
    source_path: &Path,
) -> Result<(), String> {
    let decoded = apx_raw::decode(source_path, Some(WARM_EDGE)).map_err(|err| err.to_string())?;
    let image = decoded
        .into_dynamic_image()
        .ok_or_else(|| "Dekodiertes Bild hat inkonsistente Maße".to_string())?;

    let cache_path = warm_cache_path(cache_root, photo_id);
    if let Some(dir) = cache_path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|err| format!("Cache-Verzeichnis nicht anlegbar: {err}"))?;
    }
    image
        .to_rgb8()
        .save_with_format(&cache_path, image::ImageFormat::Jpeg)
        .map_err(|err| format!("Vorschau nicht speicherbar: {err}"))?;

    catalog
        .upsert_preview(photo_id, PreviewLevel::Standard, &cache_path)
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(cached: bool) -> WarmCandidate {
        WarmCandidate {
            photo_id: PhotoId::new(),
            source_path: PathBuf::from("/fotos/IMG_0001.CR3"),
            cached,
        }
    }

    #[test]
    fn nimmt_nur_die_fotos_ohne_vorschau() {
        let plan = plan_warm(
            vec![candidate(true), candidate(false), candidate(true)],
            false,
        );
        assert_eq!(plan.pending.len(), 1);
        assert_eq!(plan.already, 2);
    }

    #[test]
    fn force_rechnet_auch_vorhandene_neu() {
        let plan = plan_warm(vec![candidate(true), candidate(true)], true);
        assert_eq!(plan.pending.len(), 2);
        assert_eq!(plan.already, 0);
    }

    #[test]
    fn ein_leerer_ordner_ergibt_nichts_zu_tun() {
        let plan = plan_warm(Vec::new(), false);
        assert!(plan.pending.is_empty());
        assert_eq!(plan.already, 0);
    }

    /// Der Cache-Pfad muss sich vom Thumbnail-Pfad desselben Fotos
    /// unterscheiden — sonst überschriebe die vorbereitete Vorschau die
    /// Miniaturansicht, und das Raster zeigte 2048px-Bilder in 256px-
    /// Kacheln (oder umgekehrt, je nachdem wer zuletzt schrieb).
    #[test]
    fn liegt_neben_dem_thumbnail_nicht_darauf() {
        let id = PhotoId::new();
        let root = Path::new("/cache");
        let warm = warm_cache_path(root, id);
        assert!(warm.to_string_lossy().ends_with("_1.jpg"));
        assert_ne!(
            warm,
            root.join(id.to_string().chars().take(2).collect::<String>())
                .join(format!("{id}_0.jpg"))
        );
    }

    #[test]
    fn nutzt_dasselbe_praefix_unterverzeichnis_wie_die_thumbnails() {
        let id = PhotoId::new();
        let path = warm_cache_path(Path::new("/cache"), id);
        let prefix: String = id.to_string().chars().take(2).collect();
        assert_eq!(
            path.parent().expect("hat ein Elternverzeichnis"),
            Path::new("/cache").join(prefix)
        );
    }
}
