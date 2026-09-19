//! Metadaten-Vorgaben (Phase 33 F4) — wiederverwendbare IPTC-Sätze, die
//! sich auf beliebig viele Fotos anwenden lassen.
//!
//! **Warum keine neue Tabelle.** Der Katalog hat mit `templates` bereits
//! einen benannten Ablageort für „ein JSON unter einem Namen, gruppiert
//! nach Art" (Migration 6) — Export, Druck, Buch, Diaschau, Workflow,
//! Filter und Umbenennungsmuster liegen alle dort. Eine Metadaten-Vorgabe
//! ist genau dasselbe Muster; eine eigene Tabelle wäre eine achte Kopie
//! derselben drei Spalten.
//!
//! **Die eine Entscheidung, an der alles hängt: `Option<String>`.**
//! `None` heißt „dieses Feld ist nicht Teil der Vorgabe" und lässt den
//! bestehenden Wert unangetastet. `Some("")` heißt „leeren". Das ist der
//! Unterschied zwischen einer Vorgabe, die Urheber und Copyright
//! nachträgt, ohne die einzeln geschriebenen Bildunterschriften zu
//! zerstören — und einer, die genau das tut. Ein Texteingabefeld allein
//! könnte beides nicht unterscheiden; deshalb hat in der Oberfläche
//! jedes Feld einen eigenen Haken „übernehmen".
//!
//! **Platzhalter.** `© {year} Name` ist der Grund, warum es sie gibt:
//! eine Copyright-Zeile ist über Jahre hinweg dieselbe bis auf die
//! Jahreszahl. Unbekannte Platzhalter bleiben wörtlich stehen, statt
//! stillschweigend zu verschwinden — wer sich vertippt, soll das im
//! Ergebnis sehen und nicht ein halb leeres Feld bekommen.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Was mit den Stichwörtern der Vorgabe passiert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum KeywordMode {
    /// Die Stichwörter der Vorgabe kommen zu den vorhandenen dazu.
    #[default]
    Add,
    /// Die vorhandenen Stichwörter werden durch die der Vorgabe ersetzt.
    Replace,
}

/// Eine gespeicherte Metadaten-Vorgabe. Wird als JSON im
/// `templates`-Eintrag abgelegt; `#[serde(default)]` überall, damit eine
/// mit einer früheren Version geschriebene Vorgabe weiterhin liest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MetadataPreset {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub caption: Option<String>,
    #[serde(default)]
    pub copyright: Option<String>,
    #[serde(default)]
    pub creator: Option<String>,
    /// Frei benannte IPTC-Zusatzfelder. Anders als oben gibt es hier kein
    /// „nicht Teil der Vorgabe": ein Feld, das in der Vorgabe steht, wird
    /// gesetzt; eines, das nicht darin steht, bleibt unberührt.
    #[serde(default)]
    pub custom: BTreeMap<String, String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub keyword_mode: KeywordMode,
}

/// Die Werte eines konkreten Fotos, aus denen Platzhalter gefüllt werden.
#[derive(Debug, Clone, Default)]
pub struct PhotoContext {
    pub filename: String,
    /// Aufnahmejahr, `None` bei Fotos ohne Datum.
    pub year: Option<i32>,
    pub camera_model: Option<String>,
    pub lens: Option<String>,
}

/// Das Ergebnis der Anwendung auf ein Foto: fertige Werte, bereit zum
/// Schreiben. `None` heißt weiterhin „nicht anfassen".
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ResolvedMetadata {
    pub title: Option<String>,
    pub caption: Option<String>,
    pub copyright: Option<String>,
    pub creator: Option<String>,
    pub custom: BTreeMap<String, String>,
    pub keywords_to_add: Vec<String>,
    /// Bei [`KeywordMode::Replace`]: alles, was nicht in der Vorgabe
    /// steht, fliegt raus.
    pub replace_keywords: bool,
}

/// Ersetzt die bekannten Platzhalter in `text`.
///
/// Ein Platzhalter, für den dieses Foto keinen Wert hat (kein
/// Aufnahmedatum, kein Objektiv in den EXIF-Daten), wird zu einem leeren
/// String — nicht zum Wort „unbekannt" und nicht zum stehen gelassenen
/// Platzhalter: in einer Copyright-Zeile wäre beides schlechter als eine
/// Lücke.
pub fn expand_placeholders(text: &str, ctx: &PhotoContext) -> String {
    let mut out = text.to_string();
    out = out.replace(
        "{year}",
        &ctx.year.map(|y| y.to_string()).unwrap_or_default(),
    );
    out = out.replace("{filename}", &ctx.filename);
    out = out.replace("{camera}", ctx.camera_model.as_deref().unwrap_or_default());
    out = out.replace("{lens}", ctx.lens.as_deref().unwrap_or_default());
    // Dateiname ohne Endung — beim Titel häufiger gewollt als der volle
    // Name mit `.CR3` daran.
    let stem = ctx
        .filename
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(&ctx.filename);
    out = out.replace("{stem}", stem);
    out
}

/// Wendet `preset` auf ein Foto an und liefert die zu schreibenden Werte.
pub fn resolve(preset: &MetadataPreset, ctx: &PhotoContext) -> ResolvedMetadata {
    let expand = |value: &Option<String>| -> Option<String> {
        value.as_ref().map(|text| expand_placeholders(text, ctx))
    };
    ResolvedMetadata {
        title: expand(&preset.title),
        caption: expand(&preset.caption),
        copyright: expand(&preset.copyright),
        creator: expand(&preset.creator),
        custom: preset
            .custom
            .iter()
            .map(|(key, value)| (key.clone(), expand_placeholders(value, ctx)))
            .collect(),
        keywords_to_add: preset
            .keywords
            .iter()
            .map(|keyword| expand_placeholders(keyword, ctx))
            .map(|keyword| keyword.trim().to_string())
            .filter(|keyword| !keyword.is_empty())
            .collect(),
        replace_keywords: preset.keyword_mode == KeywordMode::Replace,
    }
}

/// Führt die aufgelösten Zusatzfelder mit den bereits am Foto
/// vorhandenen zusammen.
///
/// Absicht: eine Vorgabe soll ein Feld setzen können, ohne alle anderen
/// mitzunehmen — `Catalog::set_photo_custom_metadata` ersetzt die ganze
/// Sammlung, also muss das Zusammenführen vorher passieren, und zwar an
/// einer Stelle, die man testen kann.
pub fn merge_custom(
    existing: &BTreeMap<String, String>,
    from_preset: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut merged = existing.clone();
    for (key, value) in from_preset {
        if value.is_empty() {
            // Ein leerer Wert in einem Zusatzfeld ist ein Löschauftrag:
            // anders als oben gibt es hier kein `Option`, und ein
            // Zusatzfeld mit leerem Wert wäre ohnehin sinnlos.
            merged.remove(key);
        } else {
            merged.insert(key.clone(), value.clone());
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> PhotoContext {
        PhotoContext {
            filename: "IMG_0042.CR3".to_string(),
            year: Some(2024),
            camera_model: Some("Canon EOS R5".to_string()),
            lens: Some("RF 50mm F1.2".to_string()),
        }
    }

    #[test]
    fn ein_nicht_gesetztes_feld_bleibt_unangetastet() {
        let preset = MetadataPreset {
            copyright: Some("© 2024".to_string()),
            ..Default::default()
        };
        let resolved = resolve(&preset, &ctx());
        assert_eq!(resolved.copyright.as_deref(), Some("© 2024"));
        // Genau der Punkt: die Bildunterschrift bleibt, wie sie war.
        assert_eq!(resolved.caption, None);
        assert_eq!(resolved.title, None);
    }

    #[test]
    fn ein_leerer_string_ist_ein_loeschauftrag_kein_nichtstun() {
        let preset = MetadataPreset {
            title: Some(String::new()),
            ..Default::default()
        };
        assert_eq!(resolve(&preset, &ctx()).title.as_deref(), Some(""));
    }

    #[test]
    fn das_jahr_kommt_aus_dem_aufnahmedatum() {
        let preset = MetadataPreset {
            copyright: Some("© {year} Anna Beispiel".to_string()),
            ..Default::default()
        };
        assert_eq!(
            resolve(&preset, &ctx()).copyright.as_deref(),
            Some("© 2024 Anna Beispiel")
        );
    }

    #[test]
    fn ein_foto_ohne_datum_bekommt_eine_luecke_statt_des_platzhalters() {
        let preset = MetadataPreset {
            copyright: Some("© {year} Anna".to_string()),
            ..Default::default()
        };
        let mut context = ctx();
        context.year = None;
        assert_eq!(
            resolve(&preset, &context).copyright.as_deref(),
            Some("©  Anna")
        );
    }

    #[test]
    fn kamera_objektiv_dateiname_und_stamm_werden_gefuellt() {
        let context = ctx();
        assert_eq!(
            expand_placeholders("{camera} · {lens} · {filename} · {stem}", &context),
            "Canon EOS R5 · RF 50mm F1.2 · IMG_0042.CR3 · IMG_0042"
        );
    }

    #[test]
    fn ein_dateiname_ohne_endung_bleibt_sein_eigener_stamm() {
        let context = PhotoContext {
            filename: "ohne_endung".to_string(),
            ..Default::default()
        };
        assert_eq!(expand_placeholders("{stem}", &context), "ohne_endung");
    }

    #[test]
    fn ein_unbekannter_platzhalter_bleibt_woertlich_stehen() {
        // Wer sich vertippt, soll das sehen. Ein stillschweigend
        // entferntes `{autor}` wäre der schlechtere Ausgang.
        assert_eq!(expand_placeholders("© {autor}", &ctx()), "© {autor}");
    }

    #[test]
    fn stichwoerter_werden_getrimmt_und_leere_verworfen() {
        let preset = MetadataPreset {
            keywords: vec!["  Reise ".to_string(), String::new(), "  ".to_string()],
            ..Default::default()
        };
        assert_eq!(resolve(&preset, &ctx()).keywords_to_add, vec!["Reise"]);
    }

    #[test]
    fn auch_stichwoerter_kennen_platzhalter() {
        let preset = MetadataPreset {
            keywords: vec!["{year}".to_string(), "{camera}".to_string()],
            ..Default::default()
        };
        assert_eq!(
            resolve(&preset, &ctx()).keywords_to_add,
            vec!["2024", "Canon EOS R5"]
        );
    }

    #[test]
    fn der_ersetzen_modus_wird_durchgereicht() {
        let preset = MetadataPreset {
            keyword_mode: KeywordMode::Replace,
            ..Default::default()
        };
        assert!(resolve(&preset, &ctx()).replace_keywords);
        assert!(!resolve(&MetadataPreset::default(), &ctx()).replace_keywords);
    }

    #[test]
    fn zusatzfelder_ergaenzen_statt_zu_ersetzen() {
        let existing = BTreeMap::from([
            ("Ort".to_string(), "Wien".to_string()),
            ("Auftrag".to_string(), "alt".to_string()),
        ]);
        let from_preset = BTreeMap::from([("Auftrag".to_string(), "neu".to_string())]);
        let merged = merge_custom(&existing, &from_preset);
        assert_eq!(merged.get("Ort").map(String::as_str), Some("Wien"));
        assert_eq!(merged.get("Auftrag").map(String::as_str), Some("neu"));
    }

    #[test]
    fn ein_leeres_zusatzfeld_loescht_es() {
        let existing = BTreeMap::from([("Ort".to_string(), "Wien".to_string())]);
        let from_preset = BTreeMap::from([("Ort".to_string(), String::new())]);
        assert!(merge_custom(&existing, &from_preset).is_empty());
    }

    #[test]
    fn eine_vorgabe_aus_einer_frueheren_version_liest_weiter() {
        // Nur die beiden Felder, die es in der ersten Fassung gab.
        let json = r#"{"copyright":"© 2024","creator":"Anna"}"#;
        let preset: MetadataPreset = serde_json::from_str(json).expect("lesbar");
        assert_eq!(preset.copyright.as_deref(), Some("© 2024"));
        assert_eq!(preset.keyword_mode, KeywordMode::Add);
        assert!(preset.keywords.is_empty());
        assert!(preset.custom.is_empty());
    }

    #[test]
    fn eine_vorgabe_uebersteht_den_json_rundlauf() {
        let preset = MetadataPreset {
            title: Some("{stem}".to_string()),
            caption: None,
            copyright: Some("© {year}".to_string()),
            creator: Some("Anna Beispiel".to_string()),
            custom: BTreeMap::from([("Ort".to_string(), "Wien".to_string())]),
            keywords: vec!["Reise".to_string()],
            keyword_mode: KeywordMode::Replace,
        };
        let json = serde_json::to_string(&preset).expect("schreibbar");
        assert_eq!(
            serde_json::from_str::<MetadataPreset>(&json).expect("lesbar"),
            preset
        );
    }
}
