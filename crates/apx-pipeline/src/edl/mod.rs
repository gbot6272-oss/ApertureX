//! Das EDL-Datenmodell (Edit Decision List) — wie eine Bearbeitung
//! non-destruktiv beschrieben wird (siehe `SPEC.md` §2.1, `ARCHITECTURE.md`
//! §5, `DECISIONS.md` ADR-0013/ADR-0014).

mod migrate;
pub(crate) mod v1;
pub(crate) mod v2;
pub(crate) mod v3;

pub use migrate::{from_envelope, to_envelope};
pub use v1::{EdlV1, WhiteBalanceAdjustment};
pub use v2::{
    BasicAdjustments, CalibrationAdjustment, ColorGradingAdjustment, ColorGradingWheel,
    ColorMixerAdjustment, ColorMixerRegion, CropRect, CurveChannel, CurvePoint, CurvesAdjustment,
    DetailsAdjustment, EdlV2, EffectsAdjustment, GeometryAdjustment, GridOverlay, GuidedLine,
    HslAdjustment, HslBand, LensCorrectionAdjustment, ManualTransform, PrimaryColorAdjustment,
    ProcessVersion, RepairMode, RepairPoint, RepairStroke, UprightMode,
};
pub use v3::{
    AiMaskKind, BlendMode, BrushStroke, EdlV3, Mask, MaskAdjustments, MaskCombine, MaskComponent,
    MaskGeometry, MaskGroup, MaskPoint, OverlayColor,
};

/// Die aktuelle EDL-Schema-Version — `EdlV3` (siehe `v3.rs`). Version 1
/// (`EdlV1`) und Version 2 (`EdlV2`) bleiben für alte, gespeicherte
/// `edit_history`-Einträge lesbar (siehe `migrate.rs`s Aufwärtspfad),
/// werden aber nicht mehr neu geschrieben.
pub const EDL_SCHEMA_VERSION: u32 = 3;
