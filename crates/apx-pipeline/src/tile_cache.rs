//! Zwischenspeicher für das teure Dekodierergebnis pro Foto+Auflösung
//! (`apx_raw::decode_linear`), damit ein Regler-Tick nicht bei jedem
//! Aufruf erneut demosaicen muss — siehe `SPEC.md` §5 ("Tile-Cache") und
//! `ARCHITECTURE.md` §5.
//!
//! Absichtlich **kein** EDL im Cache-Schlüssel: `LinearImage` hängt nur
//! von der Bilddatei und der angeforderten maximalen Kantenlänge ab, nicht
//! von den Reglerwerten — genau deshalb lohnt sich das Caching: derselbe
//! Eintrag wird für jeden Regler-Tick desselben Fotos wiederverwendet, nur
//! `crate::develop::render_rgba8` läuft je Tick neu (siehe `PLAN.md` Phase
//! 2 Schritt 5 — die Implementierung war bewusst von Schritt 4 hierher
//! verschoben, weil erst hier der tatsächliche Aufrufer und die
//! Lebensdauer-Anforderungen feststehen).
//!
//! Bewusst ein simpler, klein limitierter Cache statt eines externen
//! Crates: genau ein „aktuelles Foto in Bearbeitung" ist der Normalfall,
//! ein paar zusätzliche Plätze fangen schnelles Wechseln zwischen wenigen
//! Fotos ab (z. B. Filmstreifen-Vorschau parallel zum Entwickeln-Fenster).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use apx_core::{AppError, PhotoId, Result};
use apx_raw::LinearImage;

/// Wie viele dekodierte Bilder gleichzeitig vorgehalten werden, bevor der
/// älteste (am längsten nicht genutzte) Eintrag verdrängt wird.
const CAPACITY: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CacheKey {
    photo_id: PhotoId,
    max_edge: Option<u32>,
}

/// Ein einzelner Cache-Eintrag samt Schlüssel.
type Entry = (CacheKey, Arc<LinearImage>);

/// Hält bis zu [`CAPACITY`] dekodierte [`LinearImage`]s. Anders als
/// `apx-app`s `ImageCache` (schwache Referenzen, reine
/// Anfrage-Deduplizierung, siehe `crates/apx-app/src/protocol/cache.rs`)
/// hält dieser Cache seine Einträge **stark** — das eigentliche Ziel ist
/// Wiederverwendung über mehrere, zeitlich nacheinander folgende
/// Regler-Ticks hinweg, nicht nur das Zusammenfassen gleichzeitiger
/// Anfragen.
#[derive(Default)]
pub struct TileCache {
    // Reihenfolge = Zugriffsreihenfolge, ältestes (am längsten nicht
    // genutztes) Element vorne — bei einem Treffer wandert der Eintrag
    // ans Ende (einfaches LRU).
    entries: Mutex<VecDeque<Entry>>,
}

impl TileCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Liefert das gecachte `LinearImage` für `(photo_id, max_edge)`, oder
    /// ruft `decode` auf und speichert das Ergebnis, falls noch nichts
    /// gecacht ist. Verdrängt bei Bedarf den am längsten nicht mehr
    /// genutzten Eintrag (nicht zwingend denselben Schlüssel).
    pub fn get_or_decode(
        &self,
        photo_id: PhotoId,
        max_edge: Option<u32>,
        decode: impl FnOnce() -> Result<LinearImage>,
    ) -> Result<Arc<LinearImage>> {
        let key = CacheKey { photo_id, max_edge };

        {
            let mut entries = self.lock()?;
            if let Some(pos) = entries.iter().position(|(k, _)| *k == key) {
                // `remove` an einer per `position` gefundenen Stelle kann
                // nicht fehlschlagen.
                #[allow(clippy::expect_used)]
                let (_, image) = entries
                    .remove(pos)
                    .expect("Position stammt aus demselben Iterator, muss also gültig sein");
                entries.push_back((key, image.clone()));
                return Ok(image);
            }
        }

        let decoded = Arc::new(decode()?);

        let mut entries = self.lock()?;
        if entries.len() >= CAPACITY {
            entries.pop_front();
        }
        entries.push_back((key, decoded.clone()));
        Ok(decoded)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, VecDeque<Entry>>> {
        self.entries.lock().map_err(|_| {
            AppError::pipeline("Tile-Cache ist blockiert (vergiftete Sperre)".to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn sample_image() -> LinearImage {
        LinearImage {
            width: 2,
            height: 2,
            pixels: vec![0.5; 2 * 2 * 3],
            as_shot_wb_coeffs: [1.0, 1.0, 1.0, 1.0],
            cam_to_srgb: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    #[test]
    fn second_call_with_same_key_does_not_decode_again() {
        let cache = TileCache::new();
        let photo_id = PhotoId::new();
        let calls = AtomicUsize::new(0);

        for _ in 0..3 {
            cache
                .get_or_decode(photo_id, Some(2048), || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(sample_image())
                })
                .expect("sollte gelingen");
        }

        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "wiederholte Anfragen mit demselben Schlüssel dürfen nur einmal dekodieren"
        );
    }

    #[test]
    fn different_max_edge_is_a_different_cache_entry() {
        let cache = TileCache::new();
        let photo_id = PhotoId::new();
        let calls = AtomicUsize::new(0);

        cache
            .get_or_decode(photo_id, Some(1024), || {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(sample_image())
            })
            .expect("sollte gelingen");
        cache
            .get_or_decode(photo_id, Some(2048), || {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(sample_image())
            })
            .expect("sollte gelingen");

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn different_photo_id_is_a_different_cache_entry() {
        let cache = TileCache::new();
        let calls = AtomicUsize::new(0);

        cache
            .get_or_decode(PhotoId::new(), Some(2048), || {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(sample_image())
            })
            .expect("sollte gelingen");
        cache
            .get_or_decode(PhotoId::new(), Some(2048), || {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(sample_image())
            })
            .expect("sollte gelingen");

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn least_recently_used_entry_is_evicted_when_capacity_is_exceeded() {
        let cache = TileCache::new();
        let photo_ids: Vec<PhotoId> = (0..CAPACITY + 1).map(|_| PhotoId::new()).collect();
        let calls = AtomicUsize::new(0);

        for &id in &photo_ids {
            cache
                .get_or_decode(id, Some(2048), || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(sample_image())
                })
                .expect("sollte gelingen");
        }
        assert_eq!(calls.load(Ordering::SeqCst), CAPACITY + 1);

        // Der zuerst eingefügte Eintrag (photo_ids[0]) sollte verdrängt
        // worden sein — ein erneuter Zugriff darauf dekodiert erneut.
        cache
            .get_or_decode(photo_ids[0], Some(2048), || {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(sample_image())
            })
            .expect("sollte gelingen");
        assert_eq!(calls.load(Ordering::SeqCst), CAPACITY + 2);

        // Der zuletzt eingefügte Eintrag (photo_ids[CAPACITY]) sollte
        // dagegen noch da sein.
        cache
            .get_or_decode(photo_ids[CAPACITY], Some(2048), || {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(sample_image())
            })
            .expect("sollte gelingen");
        assert_eq!(calls.load(Ordering::SeqCst), CAPACITY + 2);
    }
}
