//! **Überholte Entwickeln-Anfragen abbrechen** (siehe `DECISIONS.md`
//! ADR-0077).
//!
//! **Was vorher passierte.** Der `apx://`-Handler startet pro Anfrage
//! einen eigenen Betriebssystem-Thread — unbegrenzt viele, ohne
//! Warteschlange und ohne Abbruch. Beim Ziehen an einem Regler schickt
//! das Frontend je Bild eine Anfrage los und bricht die vorige per
//! `AbortController` ab. Das beendet aber nur die `fetch` im Webview:
//! **der Thread in Rust rechnet weiter bis zum Ende.**
//!
//! Nach einer Sekunde Ziehen laufen also dutzende vollständige
//! Renderläufe gleichzeitig, kämpfen um dieselben Kerne und dieselbe
//! einzelne Grafikkarte, und das Ergebnis, auf das der Nutzer wartet,
//! steht hinter allen anderen in der Schlange. Genau das meldete die
//! Oberfläche mit „letztes Rendering 4216 ms".
//!
//! `PipelineError::Cancelled` trägt seit jeher den Kommentar „weil eine
//! neuere Anfrage für dasselbe Foto sie überholt hat" — der Mechanismus
//! war entworfen und nie gebaut; die Variante wird nirgends erzeugt.
//!
//! **Wie es jetzt läuft.** Jede Entwickeln-Anfrage zieht eine
//! Ticketnummer für ihr Foto. Die eigentliche Rechenarbeit ist
//! serialisiert — es gibt ohnehin nur eine Grafikkarte, und parallele
//! Läufe haben sich dort bisher nur gegenseitig ausgebremst. Wer
//! drankommt, prüft zuerst, ob inzwischen eine neuere Anfrage für
//! dasselbe Foto gezogen hat; wenn ja, steigt er aus, ohne ein einziges
//! Pixel gerechnet zu haben.
//!
//! **Warum je Foto und nicht global.** Der Filmstreifen fordert
//! Vorschauen für mehrere Fotos gleichzeitig an. Ein globales Ticket
//! würde die alle gegenseitig verhungern lassen; je Foto bleibt jede
//! dieser Anfragen die neueste für ihr eigenes Foto und läuft durch.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use apx_core::PhotoId;

#[derive(Default)]
pub(super) struct DevelopGate {
    next_ticket: AtomicU64,
    /// Die zuletzt gezogene Ticketnummer je Foto.
    latest: Mutex<HashMap<PhotoId, u64>>,
    /// Serialisiert die eigentliche Rechenarbeit.
    worker: Mutex<()>,
}

/// Der Platz in der Reihe. Solange dieser Wert lebt, rechnet niemand
/// sonst — er wird deshalb genau so lange gehalten wie die Berechnung
/// dauert und nicht länger.
pub(super) struct Turn<'a> {
    _guard: MutexGuard<'a, ()>,
}

impl DevelopGate {
    /// Zieht eine Ticketnummer für `photo` und meldet sie als die
    /// neueste.
    pub(super) fn claim(&self, photo: PhotoId) -> u64 {
        let ticket = self.next_ticket.fetch_add(1, Ordering::SeqCst) + 1;
        if let Ok(mut latest) = self.latest.lock() {
            latest.insert(photo, ticket);
        }
        ticket
    }

    /// Wartet, bis die Reihe an `ticket` ist.
    ///
    /// `None` heisst: in der Wartezeit hat eine neuere Anfrage für
    /// dasselbe Foto gezogen — dieser Lauf ist überholt und soll gar
    /// nicht erst anfangen.
    pub(super) fn wait_turn(&self, photo: PhotoId, ticket: u64) -> Option<Turn<'_>> {
        // Ein vergifteter Mutex darf hier nicht das ganze Rendern
        // lahmlegen: im schlimmsten Fall wird eben nicht serialisiert,
        // und das ist immer noch besser als gar kein Bild.
        let guard = match self.worker.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if self.is_superseded(photo, ticket) {
            return None;
        }
        Some(Turn { _guard: guard })
    }

    /// Ob inzwischen eine neuere Anfrage für dasselbe Foto gezogen hat.
    pub(super) fn is_superseded(&self, photo: PhotoId, ticket: u64) -> bool {
        match self.latest.lock() {
            // Kein Eintrag heisst: noch nie etwas gezogen. Dann ist
            // dieser Lauf per Definition nicht überholt.
            Ok(latest) => latest.get(&photo).is_some_and(|newest| *newest > ticket),
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_request_is_never_superseded() {
        let gate = DevelopGate::default();
        let photo = PhotoId::new();
        let ticket = gate.claim(photo);
        assert!(!gate.is_superseded(photo, ticket));
        assert!(gate.wait_turn(photo, ticket).is_some());
    }

    #[test]
    fn an_older_request_for_the_same_photo_is_superseded() {
        // Der Fall, um den es geht: beim Reglerziehen zieht jedes Bild
        // ein neues Ticket, und alle älteren sollen aussteigen, ohne zu
        // rechnen.
        let gate = DevelopGate::default();
        let photo = PhotoId::new();
        let first = gate.claim(photo);
        let second = gate.claim(photo);
        let third = gate.claim(photo);

        assert!(gate.is_superseded(photo, first));
        assert!(gate.is_superseded(photo, second));
        assert!(
            !gate.is_superseded(photo, third),
            "die neueste Anfrage muss durchlaufen"
        );
    }

    #[test]
    fn requests_for_different_photos_do_not_supersede_each_other() {
        // Der Filmstreifen fordert Vorschauen für mehrere Fotos
        // gleichzeitig an. Ein globales Ticket würde die alle
        // gegenseitig verhungern lassen.
        let gate = DevelopGate::default();
        let (a, b, c) = (PhotoId::new(), PhotoId::new(), PhotoId::new());
        let ticket_a = gate.claim(a);
        let ticket_b = gate.claim(b);
        let ticket_c = gate.claim(c);

        assert!(!gate.is_superseded(a, ticket_a));
        assert!(!gate.is_superseded(b, ticket_b));
        assert!(!gate.is_superseded(c, ticket_c));
    }

    #[test]
    fn the_turn_is_exclusive() {
        // Zwei gleichzeitige Läufe würden sich auf einer einzigen
        // Grafikkarte nur gegenseitig ausbremsen — und seit ADR-0075
        // teilen sie sich dort sogar dieselben Puffer.
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;

        let gate = Arc::new(DevelopGate::default());
        let photo_a = PhotoId::new();
        let photo_b = PhotoId::new();
        let inside = Arc::new(AtomicBool::new(false));
        let overlapped = Arc::new(AtomicBool::new(false));

        let mut handles = Vec::new();
        for photo in [photo_a, photo_b] {
            let gate = gate.clone();
            let inside = inside.clone();
            let overlapped = overlapped.clone();
            handles.push(std::thread::spawn(move || {
                for _ in 0..50 {
                    let ticket = gate.claim(photo);
                    let Some(turn) = gate.wait_turn(photo, ticket) else {
                        continue;
                    };
                    if inside.swap(true, Ordering::SeqCst) {
                        overlapped.store(true, Ordering::SeqCst);
                    }
                    std::thread::yield_now();
                    inside.store(false, Ordering::SeqCst);
                    drop(turn);
                }
            }));
        }
        for handle in handles {
            handle.join().expect("Thread darf nicht panischen");
        }
        assert!(
            !overlapped.load(Ordering::SeqCst),
            "zwei Läufe waren gleichzeitig in der Rechenphase"
        );
    }

    #[test]
    fn a_burst_lets_only_the_newest_through() {
        // Die eigentliche Zusage, in Zahlen: aus einem Schwall von
        // Anfragen für dasselbe Foto darf genau eine rechnen.
        let gate = DevelopGate::default();
        let photo = PhotoId::new();
        let tickets: Vec<u64> = (0..60).map(|_| gate.claim(photo)).collect();
        let durchgelassen = tickets
            .iter()
            .filter(|ticket| !gate.is_superseded(photo, **ticket))
            .count();
        assert_eq!(
            durchgelassen, 1,
            "von 60 Anfragen während eines Ziehvorgangs darf genau die letzte rechnen"
        );
    }
}
