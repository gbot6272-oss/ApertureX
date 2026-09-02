//! Personenansicht (Phase 11 Schritt 5, siehe `DECISIONS.md` ADR-0038):
//! grobe Gesichtsregion-Erkennung über dieselbe Hautton-Heuristik wie
//! [`crate::segmentation::person_alpha`] (YCbCr-Chrominanzfenster) plus
//! einfacher Konturanalyse (Connected-Component-Labeling auf dem
//! binarisierten Alpha-Kanal) — **keine echte Landmark-/Embedding-
//! Erkennung** (dasselbe ONNX-Beschaffungsproblem wie ADR-0033), deshalb
//! grobe Bounding-Boxes statt präziser Gesichtszüge.
//!
//! **Ehrlich begrenzt:** liefert nur zusammenhängende Hautton-Blobs mit
//! plausiblem Gesichts-Seitenverhältnis, keine zuverlässige Personen-
//! *Identifizierung* über mehrere Fotos hinweg — `apx-app`s
//! `group_photos_by_people`-Command (siehe dessen Moduldoku) nutzt die
//! Blob-Anzahl/-Größe nur als grobe Vorsortierung, kein echtes
//! Gesichts-Embedding.

use crate::error::Result;
use crate::segmentation::person_alpha;

/// Eine grobe Gesichtsregion in normierten Bildkoordinaten (`0.0..=1.0`,
/// Ursprung oben links) — auflösungsunabhängig, wie die Klick-Koordinaten
/// von [`crate::segmentation::AiMaskKind::ClickRegion`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceRegion {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Ab diesem Alpha-Wert (von 255) gilt ein Pixel als „Hautton genug" für
/// die Konturanalyse — bewusst über der reinen Erkennungsschwelle von
/// `person_alpha` (die selbst schon weich gewichtet, siehe dessen
/// Moduldoku), damit nur klar erkannte Kernregionen zählen.
const BLOB_THRESHOLD: u8 = 140;
/// Mindestfläche eines Blobs relativ zur Bildfläche, um Bildrauschen
/// (vereinzelte hautfarbene Pixel) nicht als Gesicht zu zählen.
const MIN_AREA_FRACTION: f32 = 0.002;
/// Plausibles Breite/Höhe-Seitenverhältnis für ein Gesicht — großzügig,
/// da keine echte Ausrichtungs-/Landmark-Prüfung stattfindet.
const MIN_ASPECT: f32 = 0.5;
const MAX_ASPECT: f32 = 1.8;
/// Mehr als das wird verworfen (nur die größten behalten) — bei so
/// vielen „Hautton"-Blobs ist die Heuristik vermutlich auf großflächige
/// hautfarbene Bildinhalte (Wand, Sandstrand, Herbstlaub) statt echte
/// Gesichter angesprungen, nicht auf eine Gruppenaufnahme.
const MAX_REGIONS: usize = 12;

/// Erkennt grobe Gesichtsregionen in `pixels` (interleaved RGB `f32`,
/// wie [`person_alpha`] es erwartet).
pub fn detect_face_regions(pixels: &[f32], width: u32, height: u32) -> Result<Vec<FaceRegion>> {
    let alpha = person_alpha(pixels, width, height)?;
    let w = width as usize;
    let h = height as usize;
    if w == 0 || h == 0 {
        return Ok(Vec::new());
    }

    let mut visited = vec![false; w * h];
    let mut regions = Vec::new();

    for start in 0..w * h {
        if visited[start] || alpha[start] < BLOB_THRESHOLD {
            continue;
        }

        // Flood-Fill (4-Nachbarschaft) mit einem iterativen Stapel statt
        // Rekursion — vermeidet einen Stapelüberlauf bei großen
        // zusammenhängenden Flächen (z. B. einer hautfarbenen Wand).
        let mut stack = vec![start];
        visited[start] = true;
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (w, 0usize, h, 0usize);
        let mut area = 0usize;

        while let Some(idx) = stack.pop() {
            let (x, y) = (idx % w, idx / w);
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
            area += 1;

            for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                    continue;
                }
                let nidx = ny as usize * w + nx as usize;
                if !visited[nidx] && alpha[nidx] >= BLOB_THRESHOLD {
                    visited[nidx] = true;
                    stack.push(nidx);
                }
            }
        }

        if (area as f32 / (w * h) as f32) < MIN_AREA_FRACTION {
            continue;
        }
        let box_w = (max_x - min_x + 1) as f32;
        let box_h = (max_y - min_y + 1) as f32;
        if !(MIN_ASPECT..=MAX_ASPECT).contains(&(box_w / box_h)) {
            continue;
        }

        regions.push(FaceRegion {
            x: min_x as f32 / w as f32,
            y: min_y as f32 / h as f32,
            width: box_w / w as f32,
            height: box_h / h as f32,
        });
    }

    // Größte zuerst — bei einer Obergrenze sind die auffälligsten
    // Regionen die relevanteren.
    regions.sort_by(|a, b| {
        (b.width * b.height)
            .partial_cmp(&(a.width * a.height))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    regions.truncate(MAX_REGIONS);
    Ok(regions)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Zwei getrennte, quadratische Hautton-Flecken auf neutralgrauem
    /// Hintergrund — die Kern-Behauptung dieses Moduls: `person_alpha`
    /// als Grundlage plus Konturanalyse findet **zwei** getrennte
    /// plausible Regionen, nicht eine verschmolzene oder keine.
    #[test]
    fn detects_two_separate_skin_toned_blobs_as_distinct_regions() {
        let width = 64u32;
        let height = 64u32;
        let w = width as usize;
        let h = height as usize;
        // Neutralgrauer Hintergrund (kein Hautton: Cb/Cr ≈ 0).
        let mut pixels = vec![0.5f32; w * h * 3];

        // Typischer heller Hautton (liegt im `person_alpha`-
        // Chrominanzfenster) als 12x12-Quadrat an zwei weit
        // auseinanderliegenden Stellen.
        let skin = (0.85f32, 0.62f32, 0.52f32);
        let mut paint_square = |cx: usize, cy: usize| {
            for y in cy.saturating_sub(6)..(cy + 6).min(h) {
                for x in cx.saturating_sub(6)..(cx + 6).min(w) {
                    let i = (y * w + x) * 3;
                    pixels[i] = skin.0;
                    pixels[i + 1] = skin.1;
                    pixels[i + 2] = skin.2;
                }
            }
        };
        paint_square(12, 12);
        paint_square(48, 48);

        let regions =
            detect_face_regions(&pixels, width, height).expect("sollte ohne Fehler laufen");

        assert_eq!(
            regions.len(),
            2,
            "erwartet genau zwei getrennte Regionen, gefunden: {regions:?}"
        );
        for region in &regions {
            assert!(region.width > 0.0 && region.height > 0.0);
            let aspect = region.width / region.height;
            assert!(
                (MIN_ASPECT..=MAX_ASPECT).contains(&aspect),
                "Seitenverhältnis {aspect} außerhalb des plausiblen Bereichs"
            );
        }
    }
}
