//! Duplikat-Assistent: welches Foto einer Gruppe bleibt (Phase 34 F9,
//! siehe `DECISIONS.md` ADR-0070).
//!
//! Duplikatgruppen findet die App seit Phase 9 (exakt über den
//! Inhalts-Hash, ähnlich über den Wahrnehmungs-Hash). Was danach kam,
//! war eine Liste — und ein `suggestBestPhoto` im Frontend, das Auflösung
//! vor Dateigröße vor Bewertung stellte und das Ergebnis mit einem
//! Sternchen markierte. Damit war die Arbeit nicht getan: man musste
//! jede Gruppe einzeln öffnen und von Hand aussortieren, und *warum*
//! ausgerechnet dieses Foto vorgeschlagen war, stand nirgends.
//!
//! Dieses Modul ersetzt die Heuristik durch eine geordnete Liste von
//! Kriterien, die jeweils sagen kann, warum sie entschieden hat. Es
//! rechnet nur: keine Datei, kein Katalog, kein Papierkorb.
//!
//! **Die Reihenfolge ist die eigentliche Aussage.** Zuerst kommt, was
//! ein Mensch ausdrücklich entschieden hat (Flagge, Bewertung) — eine
//! Messung darf eine Entscheidung nicht überstimmen. Dann, was Arbeit
//! wäre, sie zu verlieren (Bearbeitungen). Dann das Original vor dem
//! Ableger (RAW vor JPEG), weil aus einem RAW ein JPEG wird, aber nicht
//! umgekehrt. Erst danach die reinen Messwerte.

use apx_core::PhotoId;

/// Ein Foto einer Duplikatgruppe, soweit es für die Auswahl zählt.
pub(crate) struct KeeperCandidate {
    pub photo_id: PhotoId,
    pub filename: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub file_size: u64,
    pub rating: u8,
    /// 1 = Pick, -1 = Reject, 0 = keine.
    pub flag: i8,
    /// Ob an diesem Foto schon gearbeitet wurde.
    pub has_edits: bool,
}

/// Welches Kriterium den Ausschlag gab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeeperReason {
    Picked,
    HigherRating,
    HasEdits,
    RawOverDerivative,
    HigherResolution,
    LargerFile,
    /// Alle Kriterien gleich — die Gruppe ist wirklich austauschbar,
    /// und es bleibt das erste Foto.
    Indistinguishable,
}

impl KeeperReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Picked => "picked",
            Self::HigherRating => "higher_rating",
            Self::HasEdits => "has_edits",
            Self::RawOverDerivative => "raw",
            Self::HigherResolution => "higher_resolution",
            Self::LargerFile => "larger_file",
            Self::Indistinguishable => "indistinguishable",
        }
    }
}

pub(crate) struct KeeperChoice {
    pub keeper: PhotoId,
    pub reason: KeeperReason,
}

/// Endungen, die für ein Kameraoriginal stehen.
///
/// Bewusst nicht `apx_raw::is_supported_extension`: das beantwortet
/// „kann die App das lesen", und dazu gehören JPEG und PNG. Hier geht
/// es um „ist das das Original oder ein Ableger davon".
const RAW_EXTENSIONS: &[&str] = &[
    "cr2", "cr3", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "pef", "dng", "3fr",
    "iiq", "erf", "mos", "mrw", "x3f",
];

fn is_raw(filename: &str) -> bool {
    filename
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .is_some_and(|ext| RAW_EXTENSIONS.contains(&ext.as_str()))
}

fn pixels(candidate: &KeeperCandidate) -> u64 {
    u64::from(candidate.width.unwrap_or(0)) * u64::from(candidate.height.unwrap_or(0))
}

/// Wählt aus `group` das Foto, das bleiben soll — samt Begründung.
///
/// `None` nur für eine leere Gruppe. Ein Reject-markiertes Foto wird nie
/// gewählt, solange es eine Alternative gibt: die Flagge ist die eine
/// ausdrückliche Aussage „das nicht", und sie zu übergehen wäre die
/// schlimmste Art, klüger sein zu wollen.
pub(crate) fn choose_keeper(group: &[KeeperCandidate]) -> Option<KeeperChoice> {
    if group.is_empty() {
        return None;
    }
    let not_rejected: Vec<&KeeperCandidate> = group.iter().filter(|c| c.flag != -1).collect();
    let pool: Vec<&KeeperCandidate> = if not_rejected.is_empty() {
        group.iter().collect()
    } else {
        not_rejected
    };

    // Jedes Kriterium: Kennzahl je Kandidat, und der Grund, falls es
    // allein entscheidet. Erst wenn ein Kriterium einen eindeutigen
    // Spitzenreiter hat, ist Schluss — sonst geht es zum nächsten.
    type Score = fn(&KeeperCandidate) -> u64;
    let criteria: [(Score, KeeperReason); 6] = [
        (|c| u64::from(c.flag == 1), KeeperReason::Picked),
        (|c| u64::from(c.rating), KeeperReason::HigherRating),
        (|c| u64::from(c.has_edits), KeeperReason::HasEdits),
        (
            |c| u64::from(is_raw(&c.filename)),
            KeeperReason::RawOverDerivative,
        ),
        (pixels, KeeperReason::HigherResolution),
        (|c| c.file_size, KeeperReason::LargerFile),
    ];

    let mut remaining = pool;
    for (score, reason) in criteria {
        let best = remaining.iter().map(|c| score(c)).max().unwrap_or(0);
        let leaders: Vec<&KeeperCandidate> = remaining
            .iter()
            .copied()
            .filter(|c| score(c) == best)
            .collect();
        if leaders.len() == 1 {
            return Some(KeeperChoice {
                keeper: leaders[0].photo_id,
                reason,
            });
        }
        // Gleichstand: dieses Kriterium hat nichts entschieden, aber es
        // hat das Feld verkleinert. Wer hier schon hinten liegt, ist
        // auch dann raus, wenn ein späteres Kriterium ihn bevorzugte —
        // sonst schlüge Dateigröße die Bewertung.
        remaining = leaders;
    }

    Some(KeeperChoice {
        keeper: remaining[0].photo_id,
        reason: KeeperReason::Indistinguishable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(filename: &str) -> KeeperCandidate {
        KeeperCandidate {
            photo_id: PhotoId::new(),
            filename: filename.to_string(),
            width: Some(6000),
            height: Some(4000),
            file_size: 1_000_000,
            rating: 0,
            flag: 0,
            has_edits: false,
        }
    }

    #[test]
    fn eine_leere_gruppe_hat_keinen_gewinner() {
        assert!(choose_keeper(&[]).is_none());
    }

    #[test]
    fn die_pick_flagge_schlaegt_jede_messung() {
        let mut winzig = candidate("winzig.JPG");
        winzig.flag = 1;
        winzig.width = Some(640);
        winzig.height = Some(480);
        winzig.file_size = 1_000;
        let gross = candidate("gross.CR3");

        let choice = choose_keeper(&[gross, winzig]).expect("Gruppe nicht leer");
        assert_eq!(choice.reason, KeeperReason::Picked);
    }

    #[test]
    fn ein_reject_markiertes_foto_bleibt_nie_solange_es_eine_alternative_gibt() {
        let mut abgelehnt = candidate("abgelehnt.CR3");
        abgelehnt.flag = -1;
        abgelehnt.rating = 5;
        let schlicht = candidate("schlicht.JPG");
        let schlicht_id = schlicht.photo_id;

        let choice = choose_keeper(&[abgelehnt, schlicht]).expect("Gruppe nicht leer");
        assert_eq!(choice.keeper, schlicht_id);
    }

    /// Wenn alles abgelehnt ist, ist „nichts behalten" keine Antwort —
    /// der Aufrufer würde sonst die ganze Gruppe wegwerfen.
    #[test]
    fn waehlt_auch_dann_eines_wenn_alle_abgelehnt_sind() {
        let mut a = candidate("a.CR3");
        a.flag = -1;
        let mut b = candidate("b.CR3");
        b.flag = -1;
        b.rating = 3;
        let b_id = b.photo_id;

        let choice = choose_keeper(&[a, b]).expect("Gruppe nicht leer");
        assert_eq!(choice.keeper, b_id);
        assert_eq!(choice.reason, KeeperReason::HigherRating);
    }

    #[test]
    fn bearbeitungen_zaehlen_mehr_als_aufloesung() {
        let mut bearbeitet = candidate("bearbeitet.JPG");
        bearbeitet.has_edits = true;
        bearbeitet.width = Some(3000);
        bearbeitet.height = Some(2000);
        let bearbeitet_id = bearbeitet.photo_id;
        let unberuehrt = candidate("unberuehrt.JPG");

        let choice = choose_keeper(&[unberuehrt, bearbeitet]).expect("Gruppe nicht leer");
        assert_eq!(choice.keeper, bearbeitet_id);
        assert_eq!(choice.reason, KeeperReason::HasEdits);
    }

    #[test]
    fn raw_schlaegt_das_gleich_grosse_jpeg() {
        let raw = candidate("IMG_0001.CR3");
        let raw_id = raw.photo_id;
        let mut jpeg = candidate("IMG_0001.JPG");
        jpeg.file_size = 9_000_000; // größer, aber ein Ableger

        let choice = choose_keeper(&[jpeg, raw]).expect("Gruppe nicht leer");
        assert_eq!(choice.keeper, raw_id);
        assert_eq!(choice.reason, KeeperReason::RawOverDerivative);
    }

    #[test]
    fn bei_gleichem_rang_entscheidet_die_aufloesung() {
        let klein = candidate("a.JPG");
        let mut gross = candidate("b.JPG");
        gross.width = Some(8000);
        let gross_id = gross.photo_id;

        let choice = choose_keeper(&[klein, gross]).expect("Gruppe nicht leer");
        assert_eq!(choice.keeper, gross_id);
        assert_eq!(choice.reason, KeeperReason::HigherResolution);
    }

    /// Der Punkt der Feldverkleinerung: das höher bewertete Foto bleibt
    /// im Rennen, auch wenn ein anderes eine größere Datei hat.
    #[test]
    fn ein_spaeteres_kriterium_kippt_kein_frueheres() {
        let mut bewertet = candidate("bewertet.JPG");
        bewertet.rating = 4;
        bewertet.file_size = 1_000;
        let bewertet_id = bewertet.photo_id;
        let mut dick = candidate("dick.JPG");
        dick.rating = 2;
        dick.file_size = 50_000_000;

        let choice = choose_keeper(&[dick, bewertet]).expect("Gruppe nicht leer");
        assert_eq!(choice.keeper, bewertet_id);
        assert_eq!(choice.reason, KeeperReason::HigherRating);
    }

    #[test]
    fn meldet_eine_wirklich_austauschbare_gruppe_als_solche() {
        let a = candidate("a.CR3");
        let a_id = a.photo_id;
        let b = candidate("b.CR3");

        let choice = choose_keeper(&[a, b]).expect("Gruppe nicht leer");
        assert_eq!(choice.reason, KeeperReason::Indistinguishable);
        assert_eq!(choice.keeper, a_id, "bei Gleichstand bleibt das erste");
    }

    #[test]
    fn erkennt_raw_endungen_unabhaengig_von_der_schreibweise() {
        assert!(is_raw("IMG_0001.cr3"));
        assert!(is_raw("IMG_0001.NEF"));
        assert!(is_raw("scan.DNG"));
        assert!(!is_raw("IMG_0001.jpg"));
        assert!(!is_raw("ohne-endung"));
    }
}
