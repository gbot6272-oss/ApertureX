//! Typisierte IDs für Katalog-Entitäten.
//!
//! Alle IDs basieren auf UUIDv7: die High-Bits kodieren einen Zeitstempel,
//! wodurch IDs zeitlich sortierbar sind. Das hilft z. B. bei
//! „neueste zuerst"-Abfragen ohne separate Sortierspalte und verbessert die
//! B-Tree-Lokalität von Indizes gegenüber zufälligen UUIDv4-IDs.
//! Siehe `DECISIONS.md`, ADR-0005.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

/// Erzeugt einen typisierten Newtype-Wrapper über `Uuid`.
///
/// Die drei ID-Typen (`PhotoId`, `FolderId`, `CatalogId`) sind absichtlich
/// verschiedene Rust-Typen und nicht nur `Uuid`-Aliase: der Compiler
/// verhindert damit, dass z. B. eine `PhotoId` versehentlich dort verwendet
/// wird, wo eine `FolderId` erwartet wird ("stringly typed IDs" vermeiden).
macro_rules! define_id_type {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Erzeugt eine neue, zeitlich sortierbare ID (UUIDv7).
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            /// Erstellt eine ID aus einer bereits bekannten `Uuid`, z. B.
            /// beim Lesen aus der Datenbank. Es wird nicht geprüft, ob es
            /// sich tatsächlich um eine v7-UUID handelt — das ist bewusst,
            /// damit auch Alt-Daten mit anderer UUID-Version eingelesen
            /// werden können.
            pub fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// Gibt die zugrunde liegende `Uuid` zurück.
            pub fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl FromStr for $name {
            type Err = AppError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s)
                    .map(Self)
                    .map_err(|source| AppError::InvalidId {
                        value: s.to_string(),
                        source,
                    })
            }
        }
    };
}

define_id_type!(PhotoId);
define_id_type!(FolderId);
define_id_type!(CatalogId);
define_id_type!(EditHistoryId);
define_id_type!(KeywordId);
define_id_type!(CollectionId);
define_id_type!(PresetFolderId);
define_id_type!(PresetId);
define_id_type!(PresetVersionId);
// Phase 6 Schritt 8: ein benannter Schnappschuss (siehe
// `apx_catalog::repository::snapshots`s Moduldoku für die Begründung,
// warum das eine eigene Tabelle statt eines Verweises auf eine
// `edit_history`-Zeile ist). Ein Doc-Kommentar auf einer
// Makro-Invokation wird von rustdoc nicht übernommen (siehe
// `unused_doc_comments`-Warnung), deshalb ein normaler Kommentar.
define_id_type!(SnapshotId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_display_and_parse() {
        let id = PhotoId::new();
        let text = id.to_string();
        let parsed: PhotoId = text.parse().expect("gültige UUID muss parsbar sein");
        assert_eq!(id, parsed);
    }

    #[test]
    fn different_id_types_are_distinct_rust_types() {
        // Dieser Test dokumentiert die Typsicherheit: der folgende Code
        // würde nicht kompilieren, wenn PhotoId und FolderId austauschbar
        // wären — `let _: PhotoId = FolderId::new();` ist absichtlich
        // NICHT Teil dieses Tests, weil er sonst nicht kompiliert.
        let photo = PhotoId::new();
        let folder = FolderId::new();
        assert_ne!(photo.as_uuid(), folder.as_uuid());
    }

    #[test]
    fn ids_are_time_sortable() {
        let first = PhotoId::new();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = PhotoId::new();
        // UUIDv7 kodiert den Zeitstempel in den High-Bits, daher ist die
        // spätere ID auch die "größere" beim direkten Vergleich.
        assert!(second > first);
    }

    #[test]
    fn invalid_string_fails_to_parse() {
        let result = "not-a-uuid".parse::<PhotoId>();
        assert!(result.is_err());
    }

    #[test]
    fn from_uuid_preserves_value() {
        let uuid = Uuid::now_v7();
        let id = CatalogId::from_uuid(uuid);
        assert_eq!(id.as_uuid(), uuid);
    }
}
