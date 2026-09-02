//! Wohlbekannte IPTC-Kernfeld-Schlüssel für `Photo::custom_metadata`
//! (Phase 12 Schritt 4, voller EXIF/IPTC-Editor — siehe `DECISIONS.md`
//! ADR-0039).
//!
//! Reine Namenskonvention: das generische `custom_metadata_json`-Feld
//! selbst (`migrations/0010_custom_metadata.sql`) kennt keine Struktur
//! und akzeptiert jeden Schlüssel — diese Liste ist nur die Grundlage
//! für die im Frontend fest angebotenen, häufig genutzten Felder
//! (dieselben Schlüsselnamen wie im IPTC-Core-/Adobe-XMP-Vokabular, für
//! bessere Interoperabilität mit Sidecar-Dateien anderer Werkzeuge).
//! Zusätzliche, frei benannte Schlüssel sind jederzeit erlaubt.

/// `(Schlüssel, Anzeigename)`, in derselben Reihenfolge wie im
/// Metadaten-Dialog dargestellt.
pub const WELL_KNOWN_FIELDS: &[(&str, &str)] = &[
    ("Headline", "Überschrift"),
    ("Instructions", "Anweisungen"),
    ("Source", "Quelle"),
    ("TransmissionReference", "Auftragskennung"),
    ("City", "Stadt"),
    ("State", "Bundesland/Provinz"),
    ("Country", "Land"),
    ("Sublocation", "Ort (genauer)"),
    ("Event", "Ereignis"),
    ("Genre", "Genre"),
];
