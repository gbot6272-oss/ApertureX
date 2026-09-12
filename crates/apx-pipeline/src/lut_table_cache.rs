//! Serverseitiger Zwischenspeicher für vollständige LUT-Rastertabellen,
//! adressiert über ihre Inhalts-ID (`stages::lut_filter::compute_lut_id`)
//! — behebt den in `edl::LutFilterData`s Moduldoku beschriebenen Bug
//! ("Filter verändern das Bild nicht wirklich" / spürbare Verlangsamung
//! bei aktivem Filter, siehe `DECISIONS.md`, aktuelles ADR).
//!
//! Die `develop/...`-Live-Vorschau-Route (`apx-app`s `protocol`-Modul)
//! bekommt bei jedem einzelnen Regler-Tick das komplette EDL erneut als
//! JSON im URL-Pfad — für alle anderen Felder unkritisch (ein paar
//! hundert Byte), für `LutFilterData::table` aber katastrophal (ein
//! 17er-Raster allein ist über 300 KB JSON, ein importiertes 33er-Raster
//! über eine Megabyte, *pro Anfrage*, auch wenn der Regler-Tick einen
//! ganz anderen Wert betraf). Dieser Cache trennt die selten wechselnde,
//! große Nutzlast (die Tabelle selbst) von der häufig wechselnden,
//! kleinen (allen anderen EDL-Feldern): das Frontend schickt `table` nur
//! noch beim allerersten Mal für eine gegebene `id` mit (bzw. erneut nach
//! einem App-Neustart, wenn dieser Cache leer ist) — [`LutTableCache::
//! resolve`] füllt eine anschließend leer ankommende `table` aus dem
//! Cache auf und frischt den Cache bei jeder tatsächlich mitgelieferten
//! Tabelle auf (Selbstheilung: kein separater Registrierungs-Aufruf ist
//! zwingend nötig, auch wenn `apx-app`s `register_lut_filter_table`-
//! Befehl den Cache zusätzlich proaktiv vorwärmt, um das allererste
//! Rendern nach Filterwahl nicht auf einen Zufallstreffer ankommen zu
//! lassen).
//!
//! Bewusst ein simpler, unbegrenzter `HashMap`-Cache statt einer
//! Verdrängungsstrategie wie `tile_cache::TileCache`: die Anzahl
//! unterschiedlicher LUTs, die eine Sitzung tatsächlich berührt (die elf
//! eingebauten Looks + ein paar vom Nutzer importierte `.cube`-Dateien),
//! bleibt immer klein — anders als `TileCache`s potenziell sehr vielen
//! unterschiedlichen Fotos gibt es hier keinen Wachstumsdruck, der eine
//! Kapazitätsgrenze rechtfertigen würde.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::edl::LutFilterData;

#[derive(Default)]
pub struct LutTableCache {
    entries: Mutex<HashMap<String, (u32, Vec<f32>)>>,
}

impl LutTableCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registriert/aktualisiert die vollständige Tabelle unter `id`. Ein
    /// leeres `id` wird ignoriert (nichts, worüber sich später
    /// nachschlagen ließe).
    pub fn register(&self, id: &str, size: u32, table: &[f32]) {
        if id.is_empty() {
            return;
        }
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        entries.insert(id.to_string(), (size, table.to_vec()));
    }

    /// Löst `lut` gegen den Cache auf: ist `table` bereits vollständig
    /// vorhanden, wird der Cache damit aufgefrischt (Selbstheilung nach
    /// einem Neustart). Ist `table` leer, aber `id` gesetzt, wird die
    /// zuvor registrierte Tabelle eingesetzt — ohne Treffer bleibt
    /// `table` leer, sodass `stages::lut_filter::apply` seinen
    /// bestehenden "zu wenig Daten → kein Filter" Sicherheitsweg nimmt,
    /// statt mit falschen Daten zu rendern.
    pub fn resolve(&self, lut: &mut LutFilterData) {
        if lut.table.is_empty() {
            if lut.id.is_empty() {
                return;
            }
            let entries = self
                .entries
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            if let Some((size, table)) = entries.get(&lut.id) {
                lut.size = *size;
                lut.table = table.clone();
            }
        } else {
            self.register(&lut.id, lut.size, &lut.table);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str, table: Vec<f32>) -> LutFilterData {
        LutFilterData {
            name: "Test".to_string(),
            size: 2,
            table,
            domain_min: [0.0, 0.0, 0.0],
            domain_max: [1.0, 1.0, 1.0],
            id: id.to_string(),
        }
    }

    #[test]
    fn resolve_fills_empty_table_from_a_prior_full_request() {
        let cache = LutTableCache::new();
        let mut warm = sample("abc", vec![1.0, 2.0, 3.0]);
        cache.resolve(&mut warm); // registriert "abc" -> [1,2,3]

        let mut cold = sample("abc", Vec::new());
        cache.resolve(&mut cold);
        assert_eq!(cold.table, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn resolve_leaves_table_empty_on_cache_miss() {
        let cache = LutTableCache::new();
        let mut cold = sample("unbekannt", Vec::new());
        cache.resolve(&mut cold);
        assert!(cold.table.is_empty());
    }

    #[test]
    fn resolve_ignores_empty_id() {
        let cache = LutTableCache::new();
        let mut cold = sample("", Vec::new());
        cache.resolve(&mut cold);
        assert!(cold.table.is_empty());
    }

    #[test]
    fn register_is_idempotent_and_overwrite_safe() {
        let cache = LutTableCache::new();
        cache.register("x", 2, &[1.0, 2.0]);
        cache.register("x", 2, &[9.0, 9.0]);
        let mut cold = sample("x", Vec::new());
        cache.resolve(&mut cold);
        assert_eq!(cold.table, vec![9.0, 9.0]);
    }
}
