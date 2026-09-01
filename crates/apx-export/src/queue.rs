//! Export-Warteschlange (Phase 8 Schritt 2, `PLAN.md`: „dieselbe
//! Fortschritts-/Abbruch-Architektur wie der bestehende Import-Job" —
//! Phase 1). Reine, synchrone Warteschlangen-Logik ohne Threads/Tauri-
//! Abhängigkeit (wie `apx-catalog`s Modelle) — `apx-app` treibt sie in
//! einer eigenen Hintergrund-Task an und meldet Fortschritt über
//! Tauri-Events, genau wie beim Import-Job (siehe `crates/apx-app/src/
//! import/mod.rs`). Generisch über die Nutzlast `T` (hier: ein
//! aufgelöster [`crate::engine::ExportRequest`] + Zielpfad), damit dieses
//! Modul unabhängig von `apx-app`s DTOs bleibt.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemStatus {
    Pending,
    Running,
    Done,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone)]
struct Item<T> {
    id: u64,
    priority: i32,
    /// Einfügereihenfolge — Tiebreaker bei gleicher Priorität, damit die
    /// Reihenfolge innerhalb derselben Priorität stabil (FIFO) bleibt.
    sequence: u64,
    payload: T,
    status: ItemStatus,
}

/// Fortschritts-Momentaufnahme — `done` zählt `Done`+`Failed`+`Cancelled`
/// (abgeschlossene Einträge, egal mit welchem Ergebnis), `total` alle
/// jemals eingereihten Einträge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueProgress {
    pub done: usize,
    pub total: usize,
    pub failed: usize,
}

/// Eine Export-Warteschlange mit Priorisierung und Pausieren — siehe
/// Moduldoku für die Architekturentscheidung (Logik hier, Threading in
/// `apx-app`).
#[derive(Debug, Default)]
pub struct ExportQueue<T> {
    items: Vec<Item<T>>,
    next_id: u64,
    next_sequence: u64,
    paused: bool,
}

impl<T> ExportQueue<T> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            next_id: 1,
            next_sequence: 0,
            paused: false,
        }
    }

    /// Reiht `payload` mit `priority` ein (höher = zuerst dran) und gibt
    /// die neu vergebene Auftrags-ID zurück.
    pub fn push(&mut self, payload: T, priority: i32) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        self.items.push(Item {
            id,
            priority,
            sequence,
            payload,
            status: ItemStatus::Pending,
        });
        id
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Setzt die Priorität eines noch ausstehenden Auftrags neu — kein
    /// Effekt (aber auch kein Fehler) auf einen bereits abgeschlossenen
    /// oder laufenden Auftrag.
    pub fn set_priority(&mut self, id: u64, priority: i32) {
        if let Some(item) = self
            .items
            .iter_mut()
            .find(|i| i.id == id && i.status == ItemStatus::Pending)
        {
            item.priority = priority;
        }
    }

    /// Bricht einen ausstehenden ODER laufenden Auftrag ab. Gibt `true`
    /// zurück, wenn ein passender Auftrag gefunden wurde.
    pub fn cancel(&mut self, id: u64) -> bool {
        if let Some(item) = self
            .items
            .iter_mut()
            .find(|i| i.id == id && matches!(i.status, ItemStatus::Pending | ItemStatus::Running))
        {
            item.status = ItemStatus::Cancelled;
            true
        } else {
            false
        }
    }

    /// Nimmt den nächsten ausstehenden Auftrag (höchste Priorität, bei
    /// Gleichstand Einfügereihenfolge) zur Bearbeitung, markiert ihn als
    /// `Running` und gibt seine ID + Nutzlast-Referenz zurück. `None`,
    /// wenn pausiert oder nichts mehr ansteht — der Aufrufer muss die
    /// Pause-Prüfung nicht separat wiederholen.
    pub fn take_next(&mut self) -> Option<(u64, &T)> {
        if self.paused {
            return None;
        }
        let next_index = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.status == ItemStatus::Pending)
            .max_by_key(|(_, item)| (item.priority, std::cmp::Reverse(item.sequence)))
            .map(|(index, _)| index)?;
        self.items[next_index].status = ItemStatus::Running;
        let item = &self.items[next_index];
        Some((item.id, &item.payload))
    }

    pub fn mark_done(&mut self, id: u64) {
        self.set_status_if_running(id, ItemStatus::Done);
    }

    pub fn mark_failed(&mut self, id: u64, message: impl Into<String>) {
        self.set_status_if_running(id, ItemStatus::Failed(message.into()));
    }

    fn set_status_if_running(&mut self, id: u64, status: ItemStatus) {
        if let Some(item) = self
            .items
            .iter_mut()
            .find(|i| i.id == id && i.status == ItemStatus::Running)
        {
            item.status = status;
        }
    }

    pub fn status(&self, id: u64) -> Option<&ItemStatus> {
        self.items.iter().find(|i| i.id == id).map(|i| &i.status)
    }

    pub fn progress(&self) -> QueueProgress {
        let total = self.items.len();
        let mut done = 0;
        let mut failed = 0;
        for item in &self.items {
            match item.status {
                ItemStatus::Done | ItemStatus::Cancelled => done += 1,
                ItemStatus::Failed(_) => {
                    done += 1;
                    failed += 1;
                }
                ItemStatus::Pending | ItemStatus::Running => {}
            }
        }
        QueueProgress {
            done,
            total,
            failed,
        }
    }

    /// Entfernt alle abgeschlossenen Aufträge (egal welchen Ergebnisses)
    /// — hält die Warteschlange nach einem langen Batch nicht unbegrenzt
    /// wachsen, ohne laufende/ausstehende Aufträge anzutasten.
    pub fn clear_finished(&mut self) {
        self.items
            .retain(|item| matches!(item.status, ItemStatus::Pending | ItemStatus::Running));
    }

    #[cfg(test)]
    fn statuses_by_id(&self) -> std::collections::HashMap<u64, ItemStatus> {
        self.items
            .iter()
            .map(|i| (i.id, i.status.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn higher_priority_is_taken_first() {
        let mut queue = ExportQueue::new();
        let low = queue.push("niedrig", 0);
        let high = queue.push("hoch", 10);
        let (taken_id, payload) = queue.take_next().unwrap();
        assert_eq!(taken_id, high);
        assert_eq!(*payload, "hoch");
        let _ = low;
    }

    #[test]
    fn equal_priority_is_fifo() {
        let mut queue = ExportQueue::new();
        let first = queue.push("a", 0);
        let _second = queue.push("b", 0);
        let (taken_id, _) = queue.take_next().unwrap();
        assert_eq!(taken_id, first);
    }

    #[test]
    fn paused_queue_yields_nothing() {
        let mut queue = ExportQueue::new();
        queue.push("x", 0);
        queue.pause();
        assert!(queue.take_next().is_none());
        queue.resume();
        assert!(queue.take_next().is_some());
    }

    #[test]
    fn cancel_prevents_a_pending_item_from_being_taken() {
        let mut queue = ExportQueue::new();
        let id = queue.push("x", 0);
        assert!(queue.cancel(id));
        assert!(queue.take_next().is_none());
        assert_eq!(queue.status(id), Some(&ItemStatus::Cancelled));
    }

    #[test]
    fn cancel_of_a_running_item_marks_it_cancelled_not_done() {
        let mut queue = ExportQueue::new();
        let id = queue.push("x", 0);
        queue.take_next();
        assert!(queue.cancel(id));
        queue.mark_done(id); // sollte den Cancelled-Status NICHT überschreiben
        assert_eq!(queue.status(id), Some(&ItemStatus::Cancelled));
    }

    #[test]
    fn progress_counts_done_failed_and_total_correctly() {
        let mut queue = ExportQueue::new();
        let a = queue.push("a", 0);
        let b = queue.push("b", 0);
        queue.push("c", 0);

        queue.take_next();
        queue.mark_done(a);
        queue.take_next();
        queue.mark_failed(b, "kaputt");
        // "c" bleibt Pending.

        let progress = queue.progress();
        assert_eq!(progress.total, 3);
        assert_eq!(progress.done, 2);
        assert_eq!(progress.failed, 1);
    }

    #[test]
    fn set_priority_changes_take_order_for_pending_items() {
        let mut queue = ExportQueue::new();
        let first = queue.push("a", 0);
        let second = queue.push("b", 0);
        queue.set_priority(second, 5);
        let (taken_id, _) = queue.take_next().unwrap();
        assert_eq!(taken_id, second);
        let _ = first;
    }

    #[test]
    fn clear_finished_keeps_pending_and_running_untouched() {
        let mut queue = ExportQueue::new();
        let done_id = queue.push("a", 0);
        let pending_id = queue.push("b", 0);
        queue.take_next();
        queue.mark_done(done_id);

        queue.clear_finished();

        assert_eq!(queue.status(done_id), None);
        assert_eq!(queue.status(pending_id), Some(&ItemStatus::Pending));
        let statuses = queue.statuses_by_id();
        assert_eq!(statuses.len(), 1);
    }
}
