//! Das EDL-Datenmodell (Edit Decision List) — wie eine Bearbeitung
//! non-destruktiv beschrieben wird (siehe `SPEC.md` §2.1, `ARCHITECTURE.md`
//! §5, `DECISIONS.md` ADR-0013/ADR-0014).

mod migrate;
mod v1;

pub use migrate::{from_envelope, to_envelope};
pub use v1::{BasicAdjustments, EdlV1, WhiteBalanceAdjustment};

/// Die aktuelle EDL-Schema-Version. Erhöht sich, sobald ein `v2`-Modul
/// mit einer neuen `EdlV2`-Struktur dazukommt (siehe `migrate.rs`).
pub const EDL_SCHEMA_VERSION: u32 = 1;
