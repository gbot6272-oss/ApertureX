//! Datenmodelle für die Katalog-Tabellen aus Migration 1, plus kleine
//! Hilfsfunktionen für die Zeit-Konvertierung (Spalten sind laut Schema
//! `INTEGER`/Unix-Sekunden, siehe `migrations/0001_initial.sql`).

use std::path::PathBuf;

use apx_core::{
    AppError, CollectionFolderId, CollectionId, EditHistoryId, EdlEnvelope, FaceDetectionId,
    FolderId, KeywordId, PersonId, PhotoId, PresetFolderId, PresetId, PresetVersionId, Result,
    SnapshotId, StackId, TagRuleId, TemplateId,
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
    /// Farbmarkierung, `None` = keine — Name einer Zeile in
    /// `color_label_definitions` (Phase 9 Schritt 1, erweiterbar statt
    /// der früher fest verdrahteten Palette).
    pub color_label: Option<String>,
    /// `None` = echtes Foto mit eigener Datei. `Some(quelle)` = virtuelle
    /// Kopie (Phase 9 Schritt 1, `migrations/0007_library_backlog.sql`s
    /// Moduldoku) — teilt sich Datei/Pfad mit dem Quellfoto, hat aber
    /// eigene rating/flag/color_label/edit_history/keywords/collections.
    pub source_photo_id: Option<PhotoId>,
    /// IPTC-artige Metadaten-Überschreibungen (Phase 9 Schritt 2,
    /// `migrations/0008_metadata_keywords.sql`) — eigene Spalten statt nur
    /// im Datei-EXIF, weil RAW-Originale i. d. R. nicht beschreibbar sind.
    pub title: Option<String>,
    pub caption: Option<String>,
    pub copyright: Option<String>,
    pub creator: Option<String>,
    /// Voller EXIF/IPTC-Editor (Phase 12 Schritt 4, siehe `DECISIONS.md`
    /// ADR-0039, `migrations/0010_custom_metadata.sql`) — frei benannte
    /// Zusatzfelder über die vier festen Spalten oben hinaus (die üblichen
    /// IPTC-Kernfelder, siehe `crate::iptc::WELL_KNOWN_FIELDS`, plus
    /// beliebige eigene Schlüssel). Leere Werte werden beim Speichern
    /// entfernt statt als leerer String gehalten (siehe
    /// `Catalog::set_photo_custom_metadata`).
    pub custom_metadata: std::collections::BTreeMap<String, String>,
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

/// Ein Schlagwort — seit Phase 9 Schritt 2 mit optionaler Eltern-Kind-
/// Hierarchie und Synonymen (`migrations/0008_metadata_keywords.sql`,
/// `DECISIONS.md` ADR-0022/ADR-0035).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyword {
    pub id: KeywordId,
    pub name: String,
    /// `None` = Wurzel-Schlagwort.
    pub parent_id: Option<KeywordId>,
    /// Alternative Bezeichnungen, über die dasselbe Schlagwort ebenfalls
    /// gefunden werden soll (z. B. Suche/Auto-Vervollständigung im
    /// Frontend) — reine Anzeige-/Such-Hilfe, keine eigene Verknüpfung.
    pub synonyms: Vec<String>,
}

/// Eine bedingte Auto-Schlagwort-Regel (Phase 9 Schritt 2). `conditions`
/// ist derselbe `PresetCondition[]`-JSON-Vertrag wie bei Import-Presets
/// (`frontend/src/lib/presets.ts`) — Aperture X wertet ihn bewusst nur im
/// Frontend aus (eine Implementierung von `evaluateConditions`, nicht
/// zwei), diese Struktur reicht `conditions_json` deshalb unausgewertet
/// durch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagRule {
    pub id: TagRuleId,
    pub name: String,
    pub keyword_id: KeywordId,
    pub conditions_json: String,
    pub enabled: bool,
    pub created_at: OffsetDateTime,
}

/// Eine manuell zusammengestellte Sammlung, siehe `DECISIONS.md` ADR-0023
/// (keine intelligenten/verschachtelten Sammlungen in Phase 3).
#[derive(Debug, Clone, PartialEq)]
pub struct Collection {
    pub id: CollectionId,
    pub name: String,
    pub created_at: OffsetDateTime,
    /// `None` = Sammlung liegt an der Wurzel, nicht in einem Sammlungssatz.
    pub folder_id: Option<CollectionFolderId>,
    /// Intelligente Sammlung: Mitgliedschaft wird live aus
    /// `smart_criteria_json` berechnet statt über `collection_photos`
    /// gepflegt (Phase 9 Schritt 1).
    pub is_smart: bool,
    /// Serialisiertes `FilterCriteria` — nur gesetzt, wenn `is_smart`.
    pub smart_criteria_json: Option<String>,
}

// ---- Bibliotheks-Backlog (Phase 9 Schritt 1, siehe DECISIONS.md
// ADR-0032/ADR-0035, migrations/0007_library_backlog.sql) ------------------

/// Ein Ordner in der Sammlungs-Baumhierarchie ("Sammlungssatz") —
/// strukturell identisch zu `PresetFolder`.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectionFolder {
    pub id: CollectionFolderId,
    pub name: String,
    pub parent_id: Option<CollectionFolderId>,
    pub position: i64,
}

/// Ein Stapel mehrerer Fotos (z. B. eine Serienbild-Sequenz), mit einem
/// optionalen Titelbild.
#[derive(Debug, Clone, PartialEq)]
pub struct Stack {
    pub id: StackId,
    pub name: Option<String>,
    pub cover_photo_id: Option<PhotoId>,
    pub created_at: OffsetDateTime,
    /// Foto-IDs in Reihenfolge — wird beim Laden mitgeliefert statt
    /// separat abgefragt werden zu müssen.
    pub photo_ids: Vec<PhotoId>,
}

/// Eine erweiterbare Farbmarkierungs-Definition (ersetzt die frühere
/// feste `ALLOWED_COLOR_LABELS`-Palette).
#[derive(Debug, Clone, PartialEq)]
pub struct ColorLabelDefinition {
    pub name: String,
    pub display_name: String,
    pub hex: String,
    pub position: i64,
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

/// Aggregierte Katalog-Statistik (Phase 9 Schritt 3, siehe
/// `repository::stats`s Moduldoku) — schließt virtuelle Kopien aus.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogStatistics {
    pub total_photos: u64,
    pub total_file_size: u64,
    pub earliest_captured_at: Option<OffsetDateTime>,
    pub latest_captured_at: Option<OffsetDateTime>,
    /// `(Sterne, Anzahl)`, aufsteigend nach Sternen sortiert.
    pub rating_distribution: Vec<(u8, u64)>,
    /// `(Kameramodell, Anzahl)`, absteigend nach Anzahl, höchstens 8 Einträge.
    pub top_camera_models: Vec<(String, u64)>,
    pub top_lenses: Vec<(String, u64)>,
}

// ---- Echte Personen-Wiedererkennung (Phase 13 Schritt 8) -------------------

/// Eine vom Nutzer benannte Person (siehe `migrations/0011_people.sql`s
/// Moduldoku). `name: None` = automatisch erkannt (mindestens ein
/// [`FaceDetection`] zeigt hierher), aber noch nicht benannt.
#[derive(Debug, Clone, PartialEq)]
pub struct Person {
    pub id: PersonId,
    pub name: Option<String>,
    pub cover_face_id: Option<FaceDetectionId>,
    pub created_at: OffsetDateTime,
}

/// Ein einzelnes erkanntes Gesicht — Bounding-Box in Vorschaubild-
/// Pixelkoordinaten plus 128-dimensionalem Embedding (`apx_ai::people`,
/// hinter dem `people`-Cargo-Feature). `person_id: None` = noch keiner
/// Person zugeordnet.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceDetection {
    pub id: FaceDetectionId,
    pub photo_id: PhotoId,
    pub person_id: Option<PersonId>,
    pub rect_left: i64,
    pub rect_top: i64,
    pub rect_right: i64,
    pub rect_bottom: i64,
    pub embedding: Vec<f64>,
    pub created_at: OffsetDateTime,
}

/// Bounding-Box eines erkannten Gesichts (`left, top, right, bottom`),
/// in Vorschaubild-Pixelkoordinaten — eigener Typalias statt eines
/// anonymen Vier-Tupels an jeder Aufrufstelle (`clippy::type_complexity`).
pub type FaceRect = (i64, i64, i64, i64);

/// Von `dlib`s eigener Dokumentation empfohlener Schwellenwert für
/// „dieselbe Person" (euklidischer Abstand zweier 128-dimensionaler
/// Embeddings) — hier statt in `apx-ai::people` definiert (das hinter
/// dem optionalen `people`-Feature steht), damit `repository::people`s
/// Auto-Zuordnungslogik unabhängig vom `people`-Feature kompiliert (sie
/// vergleicht nur bereits gespeicherte Embeddings, berechnet selbst
/// keine neuen — das bleibt `apx-ai::people::PersonEmbedder` vorbehalten).
pub const SAME_PERSON_EMBEDDING_THRESHOLD: f64 = 0.6;

/// Euklidischer Abstand zweier Embeddings.
pub fn embedding_distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}

/// Kombinierbare Filterkriterien für [`crate::Catalog::filter_photos`] —
/// jedes `None`-Feld wird ignoriert, alle gesetzten Felder werden per UND
/// verknüpft (siehe `PLAN.md` Phase 3, Schritt 2). Bleibt als schlanker,
/// SQL-generierter Weg für die Filterleiste/Stapelverarbeitungs-Konsole
/// bestehen (immer flach UND-verknüpft ist dort ausreichend); für
/// intelligente Sammlungen ersetzt [`FilterNode`] diese Struktur seit
/// Phase 13 Schritt 7 (siehe `DECISIONS.md` ADR-0040-Nachtrag V).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FilterCriteria {
    /// Nur Fotos mit Bewertung >= diesem Wert.
    pub rating_at_least: Option<u8>,
    pub flag: Option<i8>,
    pub color_label: Option<String>,
    pub camera_model: Option<String>,
}

// ---- Verschachtelter UND/ODER-Regelbaum (Phase 13 Schritt 7) --------------
//
// Ersetzt für intelligente Sammlungen die feste, ausschließlich UND-
// verknüpfte `FilterCriteria` durch einen echten, beliebig tief
// verschachtelbaren Regelbaum. Wird **in-memory ausgewertet** statt per
// dynamischer SQL-Generierung ([`FilterNode::matches`] gegen bereits
// geladene [`Photo`]s) — Kataloge in diesem Projekt sind
// Einzelnutzer-Bibliotheken, keine Web-Anwendung mit Millionen Zeilen, eine
// zweite dynamische WHERE-Klausel-Bauart neben [`build_filter_clause`]
// (siehe `repository::search`) wäre unnötiger Aufwand für denselben
// Nutzen. Dasselbe JSON-Schema (`{"type":"condition","condition":{...}}` /
// `{"type":"group","operator":"and"|"or","children":[...]}`) wird auch vom
// Frontend-`RuleTreeEditor.tsx` erzeugt/angezeigt — die Serde-Tags sind
// bewusst so gewählt, dass sie ohne Übersetzungsschicht zum TypeScript-
// Gegenstück (`frontend/src/lib/ruleTree.ts`) passen.

/// Ein Blatt- oder Gruppenknoten im Regelbaum.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FilterNode {
    Condition {
        condition: FilterCondition,
    },
    Group {
        operator: BoolOp,
        children: Vec<FilterNode>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoolOp {
    And,
    Or,
}

/// Dasselbe Vier-Felder-Vokabular wie bisher `FilterCriteria` (Bewertung,
/// Pick/Reject-Flagge, Farbmarkierung, Kameramodell) — nur als einzelne
/// Bedingung statt fester Struktur-Felder, damit beliebig viele Bedingungen
/// desselben Felds in unterschiedlichen Zweigen des Baums vorkommen können
/// (z. B. „Kameramodell = A ODER Kameramodell = B").
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FilterCondition {
    pub field: FilterField,
    pub op: FilterOperator,
    /// Immer als String übertragen (auch für die numerischen Felder
    /// Bewertung/Flagge) — vereinfacht das Frontend-Formular, das ohnehin
    /// ein Texteingabefeld verwendet; [`FilterCondition::matches`] parst
    /// bei Bedarf.
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterField {
    Rating,
    Flag,
    ColorLabel,
    CameraModel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterOperator {
    AtLeast,
    Equals,
    NotEquals,
    Contains,
}

impl FilterCondition {
    /// Prüft eine einzelne Bedingung gegen ein Foto. Ein zum Feld nicht
    /// passender Operator (z. B. `contains` auf `rating`) oder ein nicht
    /// parsbarer Wert gilt konservativ als nicht erfüllt — dieselbe
    /// „unauswertbar = nicht erfüllt"-Konvention wie das Frontend-
    /// Gegenstück in `presets.ts`s `evaluateCondition`.
    pub fn matches(&self, photo: &Photo) -> bool {
        match self.field {
            FilterField::Rating => {
                let Ok(expected) = self.value.parse::<u8>() else {
                    return false;
                };
                match self.op {
                    FilterOperator::AtLeast => photo.rating >= expected,
                    FilterOperator::Equals => photo.rating == expected,
                    FilterOperator::NotEquals => photo.rating != expected,
                    FilterOperator::Contains => false,
                }
            }
            FilterField::Flag => {
                let Ok(expected) = self.value.parse::<i8>() else {
                    return false;
                };
                match self.op {
                    FilterOperator::Equals => photo.flag == expected,
                    FilterOperator::NotEquals => photo.flag != expected,
                    FilterOperator::AtLeast | FilterOperator::Contains => false,
                }
            }
            FilterField::ColorLabel => match &photo.color_label {
                Some(actual) => Self::matches_text(self.op, actual, &self.value),
                None => false,
            },
            FilterField::CameraModel => match &photo.camera_model {
                Some(actual) => Self::matches_text(self.op, actual, &self.value),
                None => false,
            },
        }
    }

    fn matches_text(op: FilterOperator, actual: &str, expected: &str) -> bool {
        match op {
            FilterOperator::Equals => actual.eq_ignore_ascii_case(expected),
            FilterOperator::NotEquals => !actual.eq_ignore_ascii_case(expected),
            FilterOperator::Contains => actual.to_lowercase().contains(&expected.to_lowercase()),
            FilterOperator::AtLeast => false,
        }
    }
}

impl FilterNode {
    /// Wertet den Baum rekursiv gegen ein Foto aus. Eine Gruppe ohne Kinder
    /// (z. B. ein gerade erst im Editor angelegter, noch leerer Zweig) ist
    /// für UND vakuos wahr, für ODER vakuos falsch — dieselbe Konvention
    /// wie in jeder klassischen Booleschen Algebra (leere Konjunktion/
    /// Disjunktion).
    pub fn matches(&self, photo: &Photo) -> bool {
        match self {
            FilterNode::Condition { condition } => condition.matches(photo),
            FilterNode::Group { operator, children } => match operator {
                BoolOp::And => children.iter().all(|child| child.matches(photo)),
                BoolOp::Or => children.iter().any(|child| child.matches(photo)),
            },
        }
    }
}

impl From<FilterCriteria> for FilterNode {
    /// Migriert die alte, flache UND-Verknüpfung in die neue Baumform —
    /// jedes gesetzte Feld wird eine Bedingung in einer UND-Gruppe. Ein
    /// komplett leeres `FilterCriteria` (z. B. `Default`) wird zu einer
    /// leeren UND-Gruppe, die laut [`FilterNode::matches`] auf jedes Foto
    /// zutrifft — identisch zum bisherigen Verhalten „kein Kriterium
    /// gesetzt = alle Fotos".
    fn from(criteria: FilterCriteria) -> Self {
        let mut children = Vec::new();
        if let Some(min) = criteria.rating_at_least {
            children.push(FilterNode::Condition {
                condition: FilterCondition {
                    field: FilterField::Rating,
                    op: FilterOperator::AtLeast,
                    value: min.to_string(),
                },
            });
        }
        if let Some(flag) = criteria.flag {
            children.push(FilterNode::Condition {
                condition: FilterCondition {
                    field: FilterField::Flag,
                    op: FilterOperator::Equals,
                    value: flag.to_string(),
                },
            });
        }
        if let Some(color) = criteria.color_label {
            children.push(FilterNode::Condition {
                condition: FilterCondition {
                    field: FilterField::ColorLabel,
                    op: FilterOperator::Equals,
                    value: color,
                },
            });
        }
        if let Some(model) = criteria.camera_model {
            children.push(FilterNode::Condition {
                condition: FilterCondition {
                    field: FilterField::CameraModel,
                    op: FilterOperator::Equals,
                    value: model,
                },
            });
        }
        FilterNode::Group {
            operator: BoolOp::And,
            children,
        }
    }
}

/// Liest gespeichertes `smart_criteria_json`: akzeptiert die neue Baumform
/// direkt, fällt sonst auf die alte flache `FilterCriteria` zurück (vor
/// Phase 13 Schritt 7 angelegte intelligente Sammlungen) und migriert sie
/// über [`FilterNode::from`] — siehe `DECISIONS.md` ADR-0040-Nachtrag V.
pub fn parse_filter_node(json: &str) -> Result<FilterNode> {
    if let Ok(node) = serde_json::from_str::<FilterNode>(json) {
        return Ok(node);
    }
    let legacy: FilterCriteria = serde_json::from_str(json)
        .map_err(|err| AppError::validation(format!("Gespeicherte Kriterien kaputt: {err}")))?;
    Ok(FilterNode::from(legacy))
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
