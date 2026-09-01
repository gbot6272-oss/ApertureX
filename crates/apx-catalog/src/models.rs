//! Datenmodelle für die Katalog-Tabellen aus Migration 1, plus kleine
//! Hilfsfunktionen für die Zeit-Konvertierung (Spalten sind laut Schema
//! `INTEGER`/Unix-Sekunden, siehe `migrations/0001_initial.sql`).

use std::path::PathBuf;

use apx_core::{
    AppError, CollectionId, EditHistoryId, EdlEnvelope, FolderId, KeywordId, PhotoId,
    PresetFolderId, PresetId, PresetVersionId, Result, SnapshotId, TemplateId,
};
use time::OffsetDateTime;

pub(crate) fn to_unix(dt: OffsetDateTime) -> i64 {
    dt.unix_timestamp()
}

pub(crate) fn from_unix(seconds: i64) -> Result<OffsetDateTime> {
    OffsetDateTime::from_unix_timestamp(seconds).map_err(|source| AppError::Database {
        message: format!("Ungültiger Zeitstempel {seconds}: {source}"),
    })
}

pub(crate) fn to_unix_opt(dt: Option<OffsetDateTime>) -> Option<i64> {
    dt.map(to_unix)
}

pub(crate) fn from_unix_opt(seconds: Option<i64>) -> Result<Option<OffsetDateTime>> {
    seconds.map(from_unix).transpose()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Folder {
    pub id: FolderId,
    pub path: PathBuf,
    pub parent_id: Option<FolderId>,
    pub added_at: OffsetDateTime,
}

/// Felder, die beim Anlegen eines Fotos bekannt sind. `id` und
/// `imported_at` werden von [`crate::Catalog::insert_photo`] selbst
/// erzeugt, deshalb sind sie hier nicht enthalten.
#[derive(Debug, Clone, PartialEq)]
pub struct NewPhoto {
    pub folder_id: FolderId,
    pub filename: String,
    pub file_size: u64,
    pub file_mtime: OffsetDateTime,
    pub content_hash: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// EXIF-Orientierungscode (1–8), siehe `apx-raw`. Default laut Schema
    /// ist 1 (Normal).
    pub orientation: u16,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<u32>,
    pub shutter: Option<f32>,
    pub aperture: Option<f32>,
    pub focal_length: Option<f32>,
    pub captured_at: Option<OffsetDateTime>,
    pub gps_lat: Option<f64>,
    pub gps_lon: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Photo {
    pub id: PhotoId,
    pub folder_id: FolderId,
    pub filename: String,
    pub file_size: u64,
    pub file_mtime: OffsetDateTime,
    pub content_hash: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub orientation: u16,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<u32>,
    pub shutter: Option<f32>,
    pub aperture: Option<f32>,
    pub focal_length: Option<f32>,
    pub captured_at: Option<OffsetDateTime>,
    pub gps_lat: Option<f64>,
    pub gps_lon: Option<f64>,
    pub imported_at: OffsetDateTime,
    pub missing: bool,
    /// Bewertung in Sternen, 0 (unbewertet) bis 5 — siehe
    /// `migrations/0003_library.sql`, `DECISIONS.md` ADR-0023.
    pub rating: u8,
    /// Pick/Reject-Flagge: 1 = Pick, -1 = Reject, 0 = keine.
    pub flag: i8,
    /// Farbmarkierung (`"red"`/`"yellow"`/`"green"`/`"blue"`/`"purple"`),
    /// `None` = keine.
    pub color_label: Option<String>,
}

/// Auflösungsstufe eines Vorschaubilds, siehe `PHASE1_PROMPT.md` Abschnitt 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewLevel {
    /// 256 px lange Kante.
    Thumbnail,
    /// 2048 px lange Kante.
    Standard,
    /// 1:1 (volle Auflösung).
    Full,
}

impl PreviewLevel {
    pub(crate) fn as_i64(self) -> i64 {
        match self {
            PreviewLevel::Thumbnail => 0,
            PreviewLevel::Standard => 1,
            PreviewLevel::Full => 2,
        }
    }

    pub(crate) fn from_i64(value: i64) -> Result<Self> {
        match value {
            0 => Ok(PreviewLevel::Thumbnail),
            1 => Ok(PreviewLevel::Standard),
            2 => Ok(PreviewLevel::Full),
            other => Err(AppError::Database {
                message: format!("Unbekannte Preview-Stufe in der Datenbank: {other}"),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    pub photo_id: PhotoId,
    pub level: PreviewLevel,
    pub path: PathBuf,
    pub generated_at: OffsetDateTime,
}

/// Ein Bearbeitungsschritt im Verlauf eines Fotos (siehe
/// `migrations/0002_edits.sql`, `DECISIONS.md` ADR-0014). `edl` ist für
/// `apx-catalog` undurchsichtig — nur `apx-pipeline` weiß, wie
/// `edl.payload` zu interpretieren ist.
#[derive(Debug, Clone, PartialEq)]
pub struct EditHistoryEntry {
    pub id: EditHistoryId,
    pub photo_id: PhotoId,
    pub sequence: i64,
    pub label: Option<String>,
    pub edl: EdlEnvelope,
    pub created_at: OffsetDateTime,
}

/// Wo im Bearbeitungsverlauf ein Foto gerade steht — siehe
/// [`crate::Catalog::current_edit`]/[`crate::Catalog::undo_edit`]/
/// [`crate::Catalog::redo_edit`].
#[derive(Debug, Clone, PartialEq)]
pub enum HistoryPosition {
    /// Kein Bearbeitungsschritt aktiv — Ausgangszustand "wie aufgenommen".
    Neutral,
    /// Ein konkreter, gespeicherter Bearbeitungsschritt ist aktiv.
    At(EditHistoryEntry),
}

/// Ein benannter Schnappschuss (Phase 6 Schritt 8, siehe
/// `migrations/0005_snapshots.sql`s Moduldoku für die Abgrenzung
/// gegenüber `EditHistoryEntry`) — trägt seine eigene Kopie des EDL,
/// unabhängig vom linearen Verlauf.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub id: SnapshotId,
    pub photo_id: PhotoId,
    pub name: String,
    pub edl: EdlEnvelope,
    pub created_at: OffsetDateTime,
}

/// Eine gespeicherte Vorlage (Phase 8 Schritt 8, siehe
/// `migrations/0006_templates.sql`s Moduldoku) — ein benannter
/// Parametersatz für eines der Export-Module oder einen Workflow.
/// `kind` unterscheidet die Art ("export"/"print"/"book"/"slideshow"/
/// "web"/"workflow"), `payload_json` ist das jeweilige `*Options`-DTO als
/// JSON — dieselbe Form, die der zugehörige Dialog ohnehin schon über den
/// Tauri-IPC schickt.
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    pub id: TemplateId,
    pub kind: String,
    pub name: String,
    pub payload_json: String,
    pub created_at: OffsetDateTime,
}

/// Ein Schlagwort — flache Liste ohne Hierarchie/Synonyme, siehe
/// `DECISIONS.md` ADR-0022.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyword {
    pub id: KeywordId,
    pub name: String,
}

/// Eine manuell zusammengestellte Sammlung, siehe `DECISIONS.md` ADR-0023
/// (keine intelligenten/verschachtelten Sammlungen in Phase 3).
#[derive(Debug, Clone, PartialEq)]
pub struct Collection {
    pub id: CollectionId,
    pub name: String,
    pub created_at: OffsetDateTime,
}

// ---- Presets (ab Phase 5, siehe DECISIONS.md ADR-0031) --------------------

/// Ein Ordner in der Preset-Baumhierarchie (siehe `migrations/0004_presets.sql`).
#[derive(Debug, Clone, PartialEq)]
pub struct PresetFolder {
    pub id: PresetFolderId,
    pub name: String,
    pub parent_id: Option<PresetFolderId>,
    pub position: i64,
    pub created_at: OffsetDateTime,
}

/// Ein Preset — die eigentliche EDL-Teilmenge lebt in seiner jeweils
/// aktuellen [`PresetVersion`], nicht hier (siehe Moduldoku der Migration).
#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub id: PresetId,
    pub folder_id: Option<PresetFolderId>,
    pub name: String,
    pub is_favorite: bool,
    pub tags: Vec<String>,
    /// Bedingungsregeln (Feld/Operator/Wert, UND-verknüpft, siehe
    /// `DECISIONS.md` ADR-0031 Punkt 4) als opakes JSON — `apx-catalog`
    /// muss ihre Struktur nie verstehen, nur speichern/zurückgeben.
    pub conditions_json: String,
    pub created_at: OffsetDateTime,
}

/// Eine gespeicherte Version eines Presets — `edl_subset_json` ist ein
/// opakes JSON-Objekt mit genau den EDL-Sektionen, die beim Speichern
/// ausgewählt wurden (kein vollständiges `EdlPayload`, siehe
/// `ARCHITECTURE.md` §5s Analogie zu `edit_history.edl_json`).
#[derive(Debug, Clone, PartialEq)]
pub struct PresetVersion {
    pub id: PresetVersionId,
    pub preset_id: PresetId,
    pub sequence: i64,
    pub edl_subset_json: String,
    pub created_at: OffsetDateTime,
}

/// Kombinierbare Filterkriterien für [`crate::Catalog::filter_photos`] —
/// jedes `None`-Feld wird ignoriert, alle gesetzten Felder werden per UND
/// verknüpft (siehe `PLAN.md` Phase 3, Schritt 2).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FilterCriteria {
    /// Nur Fotos mit Bewertung >= diesem Wert.
    pub rating_at_least: Option<u8>,
    pub flag: Option<i8>,
    pub color_label: Option<String>,
    pub camera_model: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_roundtrip_preserves_seconds() {
        let now = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        let seconds = to_unix(now);
        let back = from_unix(seconds).expect("sollte parsen");
        assert_eq!(back, now);
    }

    #[test]
    fn preview_level_roundtrip() {
        for level in [
            PreviewLevel::Thumbnail,
            PreviewLevel::Standard,
            PreviewLevel::Full,
        ] {
            let value = level.as_i64();
            assert_eq!(PreviewLevel::from_i64(value).expect("gültig"), level);
        }
    }

    #[test]
    fn unknown_preview_level_is_rejected() {
        assert!(PreviewLevel::from_i64(42).is_err());
    }
}
