//! **Überholte Entwickeln-Anfragen gar nicht erst rechnen** (siehe
//! `DECISIONS.md` ADR-0077 und die Korrektur in ADR-0080).
//!
//! **Das Problem.** Der `apx://`-Handler startet pro Anfrage einen
//! eigenen Betriebssystem-Thread — unbegrenzt viele, ohne Abbruch. Beim
//! Ziehen an einem Regler schickt das Frontend je Bild eine Anfrage los
//! und bricht die vorige per `AbortController` ab. Das beendet aber nur
//! die `fetch` im Webview: **der Thread in Rust rechnet weiter bis zum
//! Ende.** Nach einer Sekunde Ziehen laufen dutzende vollständige
//! Renderläufe gleichzeitig.
//!
//! **Was hier NICHT hilft: serialisieren.** Der erste Entwurf
//! (ADR-0077) liess die Läufe der Reihe nach durch, damit sie sich
//! nicht gegenseitig ausbremsen. Das hat die Lage verschlechtert, und
//! zwar messbar: 4216 → 8879 Millisekunden für die zuletzt gemeldete
//! Antwort. Der Grund ist einfach und hätte vorher auffallen müssen —
//! eine Reihenfolge bricht den LAUFENDEN Lauf nicht ab. Die neueste
//! Anfrage wartete also erst darauf, dass ein längst veralteter Lauf
//! fertig rechnet, und rechnete dann selbst. Parallel war für sie
//! schneller, auch wenn dabei Rechenzeit verschwendet wurde.
//!
//! **Was hilft: früh und ohne Warten aussteigen.** Jede Anfrage zieht
//! eine Ticketnummer für ihr Foto und prüft, bevor sie irgendetwas
//! Teures anfängt, ob inzwischen eine neuere gezogen hat. Beim Ziehen
//! mit rund sechzig Anfragen je Sekunde und Renderzeiten im
//! Hundert-Millisekunden-Bereich ist fast jede Anfrage schon beim
//! Start überholt — sie steigt sofort aus, ohne Schloss, ohne
//! Wartezeit. Übrig bleibt die jeweils neueste, und die läuft ohne
//! etwas vor sich in der Schlange.
//!
//! **Warum je Foto und nicht global.** Der Filmstreifen fordert
//! Vorschauen für mehrere Fotos gleichzeitig an. Ein globales Ticket
//! würde die alle gegenseitig verhungern lassen; je Foto bleibt jede
//! dieser Anfragen die neueste für ihr eigenes Foto.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use apx_core::PhotoId;

#[derive(Default)]
pub(super) struct DevelopGate {
    next_ticket: AtomicU64,
    /// Die zuletzt gezogene Ticketnummer je Foto.
    latest: Mutex<HashMap<PhotoId, u64>>,
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

    /// Ob inzwischen eine neuere Anfrage für dasselbe Foto gezogen hat.
    ///
    /// Bewusst ohne jede Wartezeit: wer hier `true` bekommt, steigt
    /// sofort aus, und wer `false` bekommt, fängt sofort an. Ein
    /// Schloss an dieser Stelle hat die Wartezeit der neuesten Anfrage
    /// verdoppelt (ADR-0080).
    pub(super) fn is_superseded(&self, photo: PhotoId, ticket: u64) -> bool {
        match self.latest.lock() {
            // Kein Eintrag heisst: noch nie etwas gezogen. Dann ist
            // dieser Lauf per Definition nicht überholt.
            Ok(latest) => latest.get(&photo).is_some_and(|newest| *newest > ticket),
            // Ein vergifteter Mutex darf das Rendern nicht lahmlegen:
            // im schlimmsten Fall wird eben nichts mehr aussortiert,
            // und das ist immer noch besser als kein Bild.
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

    /// Der Fehler aus ADR-0077, maschinell ausgeschlossen.
    ///
    /// Dort hielt die Schranke ein Schloss, solange gerechnet wurde.
    /// Das klang vernünftig — es gibt schliesslich nur eine
    /// Grafikkarte — und war es nicht: eine Reihenfolge bricht den
    /// laufenden Lauf nicht ab, also wartete die neueste Anfrage erst
    /// auf einen längst veralteten und rechnete dann selbst. Gemessen
    /// 4216 → 8879 Millisekunden.
    ///
    /// Dieser Test hält fest, dass die Prüfung niemandem den Weg
    /// versperrt: zwei Threads, die gleichzeitig fragen, kommen beide
    /// sofort durch.
    #[test]
    fn the_check_never_blocks_a_caller() {
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;
        use std::time::{Duration, Instant};

        let gate = Arc::new(DevelopGate::default());
        let photo_a = PhotoId::new();
        let photo_b = PhotoId::new();

        // Der erste Aufrufer tut so, als rechne er lange — ohne dabei
        // irgendetwas zu halten.
        let haelt = Arc::new(AtomicBool::new(true));
        let gate_a = gate.clone();
        let haelt_a = haelt.clone();
        let langlaeufer = std::thread::spawn(move || {
            let ticket = gate_a.claim(photo_a);
            assert!(!gate_a.is_superseded(photo_a, ticket));
            while haelt_a.load(Ordering::SeqCst) {
                std::thread::yield_now();
            }
        });

        // Der zweite muss sofort drankommen, obwohl der erste „rechnet".
        let start = Instant::now();
        let ticket_b = gate.claim(photo_b);
        let superseded = gate.is_superseded(photo_b, ticket_b);
        let gebraucht = start.elapsed();

        haelt.store(false, Ordering::SeqCst);
        langlaeufer.join().expect("Thread darf nicht panischen");

        assert!(!superseded);
        assert!(
            gebraucht < Duration::from_millis(100),
            "die Prüfung hat {gebraucht:?} gewartet — sie darf gar nicht warten"
        );
    }
}
