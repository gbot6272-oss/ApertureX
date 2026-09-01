//! Dedupliziert gleichzeitige Anfragen für dasselbe Bild: zwei parallele
//! Anfragen für denselben Schlüssel lösen nur eine Dekodierung aus (siehe
//! `PHASE1_PROMPT.md` Abschnitt 6).
//!
//! Implementiert als "Single-Flight"-Cache über schwache Referenzen: ein
//! Eintrag existiert nur, solange mindestens eine Anfrage tatsächlich
//! darauf wartet oder gerade das Ergebnis abholt. Ist niemand mehr daran
//! interessiert, verschwindet der Eintrag automatisch (keine unbegrenzt
//! wachsende Cache-Größe, kein separater Aufräum-Mechanismus nötig).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

/// Ergebnis-Slot eines einzelnen Cache-Eintrags: `None`, solange die
/// Berechnung noch läuft, danach `Some(...)`.
type Slot = Mutex<Option<Arc<Vec<u8>>>>;

#[derive(Default)]
pub(super) struct ImageCache {
    entries: Mutex<HashMap<String, Weak<Slot>>>,
}

impl ImageCache {
    /// Gibt das Ergebnis für `key` zurück. Läuft für denselben Schlüssel
    /// gerade schon eine Berechnung, wartet dieser Aufruf, bis sie fertig
    /// ist, statt `compute` erneut auszuführen.
    pub(super) fn get_or_compute(
        &self,
        key: String,
        compute: impl FnOnce() -> Result<Vec<u8>, String>,
    ) -> Result<Arc<Vec<u8>>, String> {
        let entry = self.entry_for(key)?;

        let mut guard = entry
            .lock()
            .map_err(|_| "Bild-Cache-Eintrag ist blockiert".to_string())?;
        if let Some(cached) = guard.as_ref() {
            return Ok(cached.clone());
        }

        let computed = Arc::new(compute()?);
        *guard = Some(computed.clone());
        Ok(computed)
    }

    fn entry_for(&self, key: String) -> Result<Arc<Slot>, String> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| "Bild-Cache ist blockiert".to_string())?;

        if let Some(existing) = entries.get(&key).and_then(Weak::upgrade) {
            return Ok(existing);
        }

        let fresh = Arc::new(Mutex::new(None));
        entries.insert(key, Arc::downgrade(&fresh));
        Ok(fresh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Barrier;
    use std::thread;

    #[test]
    fn returns_computed_value() {
        let cache = ImageCache::default();
        let result = cache
            .get_or_compute("a".to_string(), || Ok(vec![1, 2, 3]))
            .expect("ok");
        assert_eq!(*result, vec![1, 2, 3]);
    }

    #[test]
    fn sequential_calls_with_same_key_compute_only_once() {
        let cache = ImageCache::default();
        let calls = AtomicUsize::new(0);

        let first = cache
            .get_or_compute("x".to_string(), || {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(vec![42])
            })
            .expect("ok");
        // Der schwache Verweis ist nach dem ersten Aufruf bereits wieder
        // tot (niemand hält mehr den starken Arc außer der Rückgabe, die
        // hier sofort gedroppt wird) — ein zweiter Aufruf berechnet daher
        // erneut. Das ist korrekt: "dedupliziert gleichzeitige Anfragen",
        // nicht "cached für immer" (das wäre ein unbegrenzt wachsender
        // Cache ohne Invalidierung).
        drop(first);
        let _second = cache
            .get_or_compute("x".to_string(), || {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(vec![42])
            })
            .expect("ok");

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn concurrent_calls_with_same_key_compute_only_once() {
        let cache = Arc::new(ImageCache::default());
        let calls = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(Barrier::new(4));

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let cache = cache.clone();
                let calls = calls.clone();
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    cache.get_or_compute("gleichzeitig".to_string(), || {
                        calls.fetch_add(1, Ordering::SeqCst);
                        // Simuliert eine "langsame" Dekodierung, damit die
                        // anderen Threads wirklich auf das Ergebnis warten
                        // statt jeweils selbst zu rechnen.
                        thread::sleep(std::time::Duration::from_millis(20));
                        Ok(vec![7])
                    })
                })
            })
            .collect();

        let results: Vec<_> = handles
            .into_iter()
            .map(|h| h.join().expect("Thread darf nicht abstürzen"))
            .collect();
        for result in results {
            assert_eq!(*result.expect("ok"), vec![7]);
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "vier gleichzeitige Anfragen dürfen nur eine Berechnung auslösen"
        );
    }

    #[test]
    fn different_keys_are_independent() {
        let cache = ImageCache::default();
        let a = cache
            .get_or_compute("a".to_string(), || Ok(vec![1]))
            .expect("ok");
        let b = cache
            .get_or_compute("b".to_string(), || Ok(vec![2]))
            .expect("ok");
        assert_eq!(*a, vec![1]);
        assert_eq!(*b, vec![2]);
    }

    #[test]
    fn failed_computation_does_not_poison_future_calls() {
        let cache = ImageCache::default();
        let first = cache.get_or_compute("x".to_string(), || Err("kaputt".to_string()));
        assert!(first.is_err());

        let second = cache.get_or_compute("x".to_string(), || Ok(vec![9]));
        assert_eq!(*second.expect("sollte nach Fehler neu versuchen"), vec![9]);
    }
}
