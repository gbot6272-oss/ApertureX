//! Merkmalsbasiertes Homographie-Stitching (Phase 13 Schritt 5, siehe
//! `DECISIONS.md` ADR-0040-Nachtrag III) — echtes Freihand-Panorama-
//! Stitching mit Rotation/Perspektive/Parallaxe, im Unterschied zu
//! [`super::panorama`]s reiner Verschiebungs-Registrierung
//! (Phasenkorrelation, nur für Aufnahmen ohne Kamerarotation geeignet).
//!
//! **Pipeline:** FAST-9-Eckenerkennung (`imageproc::corners`) +
//! eigene BRIEF-artige binäre Deskriptoren (siehe [`describe_keypoint`])
//! auf beiden Bildern → brute-force Hamming-Abstands-Matching mit Lowes
//! Verhältnistest → eigener RANSAC-Loop über `homography`s DLT-Schätzung
//! → Rückwärts-Verzerrung (jedes Ausgabepixel sucht seine Quellposition
//! per inverser Homografie, bilineare Abtastung) auf eine gemeinsame
//! Leinwand.
//!
//! **Warum nicht das rust-cv-Ökosystem (`akaze`/`cv-core`/`space`/
//! `arrsac`/`sample-consensus`)**, obwohl ADR-0040 es per `cargo add
//! --dry-run` als real auf crates.io verfügbar bestätigt hatte: ein
//! echter Einbindungsversuch (dieser Schritt, vor dieser Version des
//! Moduls) zeigte, dass `cargo add --dry-run` nur die Metadaten-Auflösung
//! prüft, nicht den tatsächlichen Kompilierlauf. `akaze` (letzte
//! Veröffentlichung 2021) hängt transitiv an `bitarray 0.2`, dessen
//! `src/lib.rs` unbedingt `#![feature(min_const_generics)]` setzt —
//! dieses Attribut selbst (nicht das längst stabile Feature dahinter)
//! verlangt einen Nightly-Compiler und schlägt auf jedem aktuellen
//! stabilen Rust mit `error[E0554]` fehl, real gegen `rustc 1.94.1`
//! getestet. Das rust-cv-Ökosystem ist damit auf stabilem Rust praktisch
//! tot, unabhängig von den zusätzlich noch bestehenden Versions-
//! Inkompatibilitäten (`cv-core 0.15`/`nalgebra 0.21`/`space 0.10` vs.
//! `homography`s aktiv gepflegtem `nalgebra 0.33`; `homography` hängt
//! zudem gar nicht von `sample-consensus` ab, kein `Estimator`-Trait-Impl
//! — per `grep` in dessen `Cargo.toml` bestätigt). Eine ehrliche Korrektur
//! an ADR-0040s zu optimistischer Einschätzung, nicht rückwirkend
//! umgeschrieben (siehe `DECISIONS.md`). Statt einer Schein-Integration
//! nutzt dieses Modul `imageproc` (bereits in `apx-ai` real erprobt, siehe
//! Phase 13 Schritt 4) für die Eckenerkennung, einen selbst geschriebenen
//! BRIEF-artigen Deskriptor (klassische, publizierte Technik — Calonder
//! et al. 2010 — keine Fabrikation) und einen eigenen, kurzen RANSAC-Loop
//! über `homography`s direkte DLT-Schätzfunktion.

use imageproc::corners::{corners_fast9, Corner};
use imageproc::definitions::Score;
use imageproc::suppress::local_maxima;
use nalgebra::Matrix3;
use rand::rngs::StdRng;
use rand::seq::IndexedRandom;
use rand::{Rng, SeedableRng};
use std::sync::OnceLock;

use crate::error::{Result, StackingError};
use crate::luma::rgba8_to_luma_f32;

/// Deskriptorlänge in Bytes (`256` Bit) — dieselbe Größe wie ORBs
/// Standarddeskriptor, ein guter Kompromiss aus Unterscheidungskraft und
/// Geschwindigkeit.
const DESCRIPTOR_BYTES: usize = 32;
/// Patch-Halbradius (Pixel) um jedes Merkmal, aus dem die BRIEF-
/// Abtastpunkte gezogen werden.
const PATCH_RADIUS: i32 = 15;

/// Eine Punkt-Korrespondenz `(Quellposition, Zielposition)`, beide in
/// Pixelkoordinaten — die gemeinsame Eingabeform für [`solve_homography`]
/// und [`estimate_homography_ransac`].
type PointCorrespondence = ((f32, f32), (f32, f32));

/// Ein erkanntes Merkmal: Bildposition (Pixelkoordinaten) plus binärer
/// Deskriptor.
struct Feature {
    point: (f32, f32),
    descriptor: [u8; DESCRIPTOR_BYTES],
}

/// Feste, einmalig deterministisch erzeugte BRIEF-Abtastpunktpaare
/// (`(dx1, dy1, dx2, dy2)`, relativ zum Merkmalszentrum) — **muss** für
/// alle Bilder identisch sein, sonst wären zwei Deskriptoren nicht
/// vergleichbar (dieselbe Grundvoraussetzung wie bei jeder BRIEF-
/// Implementierung). Gleichverteilt statt gaußverteilt (ORBs Wahl) im
/// Patch gezogen — eine kleinere, aber real dokumentierte Variante
/// derselben Grundidee, ohne eine zusätzliche Normalverteilungs-
/// Abhängigkeit (`rand_distr`) zu brauchen.
fn brief_pattern() -> &'static [(i32, i32, i32, i32); DESCRIPTOR_BYTES * 8] {
    static PATTERN: OnceLock<[(i32, i32, i32, i32); DESCRIPTOR_BYTES * 8]> = OnceLock::new();
    PATTERN.get_or_init(|| {
        let mut rng = StdRng::seed_from_u64(0xB81E_F000_u64);
        std::array::from_fn(|_| {
            (
                rng.random_range(-PATCH_RADIUS..=PATCH_RADIUS),
                rng.random_range(-PATCH_RADIUS..=PATCH_RADIUS),
                rng.random_range(-PATCH_RADIUS..=PATCH_RADIUS),
                rng.random_range(-PATCH_RADIUS..=PATCH_RADIUS),
            )
        })
    })
}

/// Grauwert an `(x, y)` — an den Bildrand geklemmt statt eine
/// Bereichsprüfung je Abtastpunkt zu verlangen (ein Patch kann über den
/// Rand hinausragen, wenn das Merkmal nah am Rand liegt).
fn clamped_gray(gray: &imageproc::image::GrayImage, x: i32, y: i32) -> u8 {
    let cx = x.clamp(0, gray.width() as i32 - 1) as u32;
    let cy = y.clamp(0, gray.height() as i32 - 1) as u32;
    gray.get_pixel(cx, cy)[0]
}

/// Berechnet den BRIEF-artigen Deskriptor für ein Merkmal an `(x, y)`:
/// für jedes der 256 festen Punktpaare aus [`brief_pattern`] wird Bit `i`
/// gesetzt, wenn der erste Punkt heller ist als der zweite.
fn describe_keypoint(gray: &imageproc::image::GrayImage, x: u32, y: u32) -> [u8; DESCRIPTOR_BYTES] {
    let pattern = brief_pattern();
    let mut descriptor = [0u8; DESCRIPTOR_BYTES];
    for (bit_index, &(dx1, dy1, dx2, dy2)) in pattern.iter().enumerate() {
        let v1 = clamped_gray(gray, x as i32 + dx1, y as i32 + dy1);
        let v2 = clamped_gray(gray, x as i32 + dx2, y as i32 + dy2);
        if v1 < v2 {
            descriptor[bit_index / 8] |= 1 << (bit_index % 8);
        }
    }
    descriptor
}

/// Hamming-Abstand zweier Deskriptoren — Anzahl unterschiedlicher Bits
/// (XOR + Populationszählung je Byte).
fn hamming_distance(a: &[u8; DESCRIPTOR_BYTES], b: &[u8; DESCRIPTOR_BYTES]) -> u32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x ^ y).count_ones())
        .sum()
}

/// FAST-9-Schwelle — moderat: real fotografierte Kanten haben deutlich
/// mehr Kontrast als dieser Wert, aber er ist niedrig genug, um auch bei
/// gedämpfter Beleuchtung noch genug Merkmale zu finden.
const FAST_THRESHOLD: u8 = 20;
/// Nicht-Maximum-Unterdrückungsradius für die Eckenauswahl (Pixel) —
/// verhindert dicht gedrängte, redundante Merkmale an derselben Kante.
const NMS_RADIUS: u32 = 8;
/// Obergrenze der behaltenen Merkmale je Bild (nach Score sortiert) —
/// begrenzt das brute-force-Matching (`O(n·m)`) bei sehr texturreichen
/// Fotos auf eine praktikable Laufzeit.
const MAX_FEATURES: usize = 800;

/// Anteil des zweitbesten Hamming-Abstands, den der beste Treffer
/// höchstens erreichen darf (Lowes Verhältnistest aus der SIFT-Arbeit,
/// hier auf binäre Deskriptoren angewendet — dieselbe Praxis wie beim
/// verbreiteten ORB-Matching).
const RATIO_TEST_MAX: f32 = 0.8;

/// RANSAC-Iterationen für die Homografie-Schätzung — deutlich mehr als
/// für z. B. eine Fundamentalmatrix nötig wären (dort reichen oft <100),
/// da eine Homografie schon 4 statt nur 7-8 Punkte pro Stichprobe
/// braucht und Ausreißer bei Merkmalspaaren zwischen Freihandaufnahmen
/// real häufig sind.
const RANSAC_ITERATIONS: usize = 2000;
/// Maximale Rückprojektionsabweichung (Pixel), ab der ein Merkmalspaar
/// noch als Inlier zählt.
const INLIER_THRESHOLD_PX: f64 = 3.0;
/// Mindestzahl an Inliern, unter der eine Homografie als unzuverlässig
/// verworfen wird (statt eine auf zu wenig Beweisen beruhende Schätzung
/// zurückzugeben) — willkürlich, aber deutlich über dem theoretischen
/// Minimum von 4, um Zufallstreffer bei wenigen Merkmalen abzufangen.
const MIN_INLIERS: usize = 12;

/// Wandelt einen RGBA8-Puffer in Graustufen um und extrahiert FAST-9-
/// Ecken + BRIEF-Deskriptoren daraus (siehe Moduldoku).
fn extract_features(pixels: &[u8], width: u32, height: u32) -> Vec<Feature> {
    let gray_bytes: Vec<u8> = rgba8_to_luma_f32(pixels)
        .into_iter()
        .map(|v| v.round().clamp(0.0, 255.0) as u8)
        .collect();
    let gray = imageproc::image::GrayImage::from_raw(width, height, gray_bytes)
        .expect("Puffergröße passt exakt zu width*height (aus rgba8_to_luma_f32)");

    let mut corners: Vec<Corner> = corners_fast9(&gray, FAST_THRESHOLD);
    corners = local_maxima(&corners, NMS_RADIUS);
    corners.sort_by(|a, b| {
        b.score()
            .partial_cmp(&a.score())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    corners.truncate(MAX_FEATURES);

    corners
        .into_iter()
        .map(|c| Feature {
            point: (c.x as f32, c.y as f32),
            descriptor: describe_keypoint(&gray, c.x, c.y),
        })
        .collect()
}

/// Findet für jedes Merkmal in `a` seinen besten Treffer in `b` per
/// brute-force Hamming-Abstand (kein KD-Baum/HNSW — bei den hier
/// üblichen einigen hundert Merkmalen je Foto ist eine lineare Suche
/// schnell genug). Behält nur Paare, die Lowes Verhältnistest bestehen —
/// liefert `(Index in a, Index in b)`-Paare.
fn match_features(a: &[Feature], b: &[Feature]) -> Vec<(usize, usize)> {
    if b.is_empty() {
        return Vec::new();
    }
    a.iter()
        .enumerate()
        .filter_map(|(i, fa)| {
            let mut best_index = 0usize;
            let mut best_distance = u32::MAX;
            let mut second_best_distance = u32::MAX;
            for (j, fb) in b.iter().enumerate() {
                let distance = hamming_distance(&fa.descriptor, &fb.descriptor);
                if distance < best_distance {
                    second_best_distance = best_distance;
                    best_distance = distance;
                    best_index = j;
                } else if distance < second_best_distance {
                    second_best_distance = distance;
                }
            }
            if second_best_distance == u32::MAX {
                return None; // nur ein einziges Merkmal in `b` — kein Verhältnistest möglich.
            }
            if (best_distance as f32) < RATIO_TEST_MAX * (second_best_distance as f32) {
                Some((i, best_index))
            } else {
                None
            }
        })
        .collect()
}

/// Löst die Homografie per DLT (`homography`-Crate, direkter SVD-Aufruf,
/// kein RANSAC) aus mindestens vier Punkt-Korrespondenzen `(Quelle,
/// Ziel)` und normiert sie so, dass `matrix[(2,2)] == 1.0` (die
/// klassische Homografie-Konvention — die rohe SVD-Lösung ist nur bis auf
/// einen Skalierungsfaktor bestimmt). `None`, wenn zu wenige Punkte
/// übergeben wurden oder die Lösung entartet ist (`matrix[(2,2)] ≈ 0`,
/// z. B. bei (nahezu) kollinearen Stichprobenpunkten).
fn solve_homography(pairs: &[PointCorrespondence]) -> Option<Matrix3<f64>> {
    if pairs.len() < 4 {
        return None;
    }
    let mut computation = homography::HomographyComputation::<f64>::new();
    for &((sx, sy), (dx, dy)) in pairs {
        computation.add_point_correspondence(
            homography::geo::Point::new(sx as f64, sy as f64),
            homography::geo::Point::new(dx as f64, dy as f64),
        );
    }
    let solution = computation.get_restrictions().compute();
    let scale = solution.matrix[(2, 2)];
    if !scale.is_finite() || scale.abs() < 1e-9 {
        return None;
    }
    let mut normalized = solution.matrix;
    for row in 0..3 {
        for col in 0..3 {
            normalized[(row, col)] /= scale;
        }
    }
    Some(normalized)
}

/// Bildet einen Punkt per Homografie ab (homogene Koordinaten,
/// perspektivische Division) — `None`, wenn der Punkt auf die
/// Fluchtebene fällt (`w ≈ 0`).
fn apply_homography(h: &Matrix3<f64>, point: (f64, f64)) -> Option<(f64, f64)> {
    let w = h[(2, 0)] * point.0 + h[(2, 1)] * point.1 + h[(2, 2)];
    if !w.is_finite() || w.abs() < 1e-9 {
        return None;
    }
    let x = (h[(0, 0)] * point.0 + h[(0, 1)] * point.1 + h[(0, 2)]) / w;
    let y = (h[(1, 0)] * point.0 + h[(1, 1)] * point.1 + h[(1, 2)]) / w;
    Some((x, y))
}

/// Schätzt die Homografie, die `matches`s Quellpunkte am besten auf ihre
/// Zielpunkte abbildet — eigener RANSAC-Loop (siehe Moduldoku): zieht
/// wiederholt vier zufällige Korrespondenzen, löst die minimale
/// Homografie darüber, zählt Inlier über alle Korrespondenzen, behält
/// die beste Stichprobe. Verfeinert abschließend über alle Inlier der
/// besten Stichprobe (mehr Punkte → stabilere DLT-Lösung als die
/// minimalen vier). `None`, wenn zu wenige Korrespondenzen übergeben
/// wurden oder keine Stichprobe genug Inlier fand (siehe
/// [`MIN_INLIERS`]) — eine ehrliche "nicht genug Beweise"-Antwort statt
/// einer auf Zufallstreffern beruhenden Schätzung.
fn estimate_homography_ransac(matches: &[PointCorrespondence]) -> Option<Matrix3<f64>> {
    if matches.len() < 4 {
        return None;
    }
    let mut rng = StdRng::seed_from_u64(0xA1A2_E53E_u64); // deterministisch: identische Eingaben liefern identische Ergebnisse (u. a. für Tests).
    let indices: Vec<usize> = (0..matches.len()).collect();

    let mut best_inliers: Vec<usize> = Vec::new();
    for _ in 0..RANSAC_ITERATIONS {
        let sample_indices: Vec<usize> = indices.choose_multiple(&mut rng, 4).copied().collect();
        let sample: Vec<PointCorrespondence> = sample_indices.iter().map(|&i| matches[i]).collect();
        let Some(h) = solve_homography(&sample) else {
            continue;
        };
        let inliers: Vec<usize> = matches
            .iter()
            .enumerate()
            .filter(|(_, &(src, dst))| {
                apply_homography(&h, (src.0 as f64, src.1 as f64)).is_some_and(|(px, py)| {
                    let dx = px - dst.0 as f64;
                    let dy = py - dst.1 as f64;
                    (dx * dx + dy * dy).sqrt() < INLIER_THRESHOLD_PX
                })
            })
            .map(|(i, _)| i)
            .collect();
        if inliers.len() > best_inliers.len() {
            best_inliers = inliers;
        }
    }

    if best_inliers.len() < MIN_INLIERS {
        return None;
    }
    let refined_pairs: Vec<PointCorrespondence> =
        best_inliers.iter().map(|&i| matches[i]).collect();
    solve_homography(&refined_pairs).or_else(|| {
        // Sollte praktisch nie eintreten (die minimale Stichprobe hat
        // bereits erfolgreich gelöst) — bewusst kein Absturz, sondern
        // Rückfall auf die letzte erfolgreiche (Minimal-Stichproben-)Lösung.
        solve_homography(&[
            refined_pairs[0],
            refined_pairs[1],
            refined_pairs[2],
            refined_pairs[3],
        ])
    })
}

/// Schätzt für jedes Bild in `images_pixels` die Homografie, die es auf
/// `reference_pixels`s Koordinatensystem abbildet — `None` je Bild, wenn
/// keine verlässliche Homografie gefunden wurde (zu wenige/zu
/// unzuverlässige Merkmalsübereinstimmungen). Der Aufrufer entscheidet,
/// wie mit einem `None`-Eintrag umzugehen ist (siehe `apx-app`s
/// `stack_panorama`-Command: Rückfall auf die reine
/// Verschiebungs-Registrierung für das gesamte Panorama).
pub fn estimate_pairwise_homographies_rgba8(
    reference_pixels: &[u8],
    images_pixels: &[&[u8]],
    width: u32,
    height: u32,
) -> Vec<Option<Matrix3<f64>>> {
    let reference_features = extract_features(reference_pixels, width, height);
    images_pixels
        .iter()
        .map(|pixels| {
            let features = extract_features(pixels, width, height);
            let matches = match_features(&features, &reference_features);
            let correspondences: Vec<PointCorrespondence> = matches
                .iter()
                .map(|&(i, j)| (features[i].point, reference_features[j].point))
                .collect();
            estimate_homography_ransac(&correspondences)
        })
        .collect()
}

/// Ein Quellbild mit seiner Homografie zum Referenzbild (das
/// Referenzbild selbst bekommt `Matrix3::identity()`).
pub struct HomographyPositionedImage<'a> {
    pub pixels: &'a [u8],
    /// Bildet eine Position in DIESEM Bild auf die entsprechende Position
    /// im Referenzbild-Koordinatensystem ab.
    pub homography: Matrix3<f64>,
}

/// Bilineare RGB-Abtastung an einer gebrochenzahligen Position — anders
/// als `panorama.rs`s reine Verschiebung (Ganzzahlversatz, keine
/// Interpolation nötig) braucht die Homografie-Rückwärtsabtastung
/// gebrochenzahlige Quellpositionen. Randpixel werden geklemmt (dieselbe
/// Konvention wie `apx-pipeline`s `lens_corrections::bilinear_sample`).
fn bilinear_sample_rgba8(pixels: &[u8], width: u32, height: u32, x: f64, y: f64) -> [f32; 3] {
    let w = width as i64;
    let h = height as i64;
    let x0 = x.floor();
    let y0 = y.floor();
    let fx = (x - x0) as f32;
    let fy = (y - y0) as f32;
    let sample = |sx: i64, sy: i64, channel: usize| -> f32 {
        let cx = sx.clamp(0, w - 1) as usize;
        let cy = sy.clamp(0, h - 1) as usize;
        pixels[(cy * width as usize + cx) * 4 + channel] as f32
    };
    let x0i = x0 as i64;
    let y0i = y0 as i64;
    let mut out = [0f32; 3];
    for (channel, slot) in out.iter_mut().enumerate() {
        let v00 = sample(x0i, y0i, channel);
        let v10 = sample(x0i + 1, y0i, channel);
        let v01 = sample(x0i, y0i + 1, channel);
        let v11 = sample(x0i + 1, y0i + 1, channel);
        let top = v00 + (v10 - v00) * fx;
        let bottom = v01 + (v11 - v01) * fx;
        *slot = top + (bottom - top) * fy;
    }
    out
}

/// Setzt `images` per Rückwärtsverzerrung auf einer gemeinsamen Leinwand
/// zusammen: die Leinwand-Bounding-Box ergibt sich aus den vier Ecken
/// jedes Bildes, vorwärts durch seine Homografie projiziert; für jedes
/// Leinwandpixel wird per inverser Homografie die Quellposition in jedem
/// Bild gesucht und bilinear abgetastet — Überlappungsbereiche werden
/// gemittelt (dieselbe Vereinfachung wie `panorama::stitch_shift_rgba8`:
/// kein Feathering/keine Nahtoptimierung).
pub fn stitch_homography_rgba8(
    images: &[HomographyPositionedImage],
    width: u32,
    height: u32,
) -> Result<(u32, u32, Vec<u8>)> {
    if images.len() < 2 {
        return Err(StackingError::TooFewImages {
            message: format!(
                "Panorama-Zusammenführung braucht mindestens 2 Bilder, {} übergeben",
                images.len()
            ),
        });
    }
    let expected_len = (width as usize) * (height as usize) * 4;
    for (index, image) in images.iter().enumerate() {
        if image.pixels.len() != expected_len {
            return Err(StackingError::DimensionMismatch {
                message: format!(
                    "Bild {index} hat {} Bytes, erwartet wurden {expected_len} ({width}x{height} RGBA8)",
                    image.pixels.len()
                ),
            });
        }
    }

    let corners = [
        (0.0, 0.0),
        (width as f64, 0.0),
        (0.0, height as f64),
        (width as f64, height as f64),
    ];
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for image in images {
        for &corner in &corners {
            let Some((x, y)) = apply_homography(&image.homography, corner) else {
                return Err(StackingError::DimensionMismatch {
                    message: "entartete Homografie (Bildecke fällt auf die Fluchtebene)".into(),
                });
            };
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
    }

    let canvas_width = (max_x - min_x).ceil().max(1.0) as u32;
    let canvas_height = (max_y - min_y).ceil().max(1.0) as u32;
    // Schutz vor einer pathologischen (fast entarteten) Homografie, die
    // eine riesige Leinwand ergäbe, statt unkontrolliert Speicher zu
    // allozieren — 64 Megapixel ist großzügig über jedem realistischen
    // Panorama aus ein paar Aufnahmen.
    const MAX_CANVAS_PIXELS: u64 = 64_000_000;
    if (canvas_width as u64) * (canvas_height as u64) > MAX_CANVAS_PIXELS {
        return Err(StackingError::DimensionMismatch {
            message: format!(
                "berechnete Leinwand {canvas_width}x{canvas_height} ist unplausibel groß — \
                 vermutlich eine fehlerhafte Homografie"
            ),
        });
    }

    let mut inverses = Vec::with_capacity(images.len());
    for image in images {
        let inverse =
            image
                .homography
                .try_inverse()
                .ok_or_else(|| StackingError::DimensionMismatch {
                    message: "Homografie ist nicht invertierbar".into(),
                })?;
        inverses.push(inverse);
    }

    let canvas_pixels = (canvas_width as usize) * (canvas_height as usize);
    let mut sum = vec![[0.0f32; 3]; canvas_pixels];
    let mut count = vec![0u32; canvas_pixels];

    for (image, inverse) in images.iter().zip(&inverses) {
        for cy in 0..canvas_height {
            for cx in 0..canvas_width {
                let reference_point = (cx as f64 + min_x, cy as f64 + min_y);
                let Some((sx, sy)) = apply_homography(inverse, reference_point) else {
                    continue;
                };
                if sx < 0.0 || sy < 0.0 || sx > (width - 1) as f64 || sy > (height - 1) as f64 {
                    continue;
                }
                let rgb = bilinear_sample_rgba8(image.pixels, width, height, sx, sy);
                let dst_index = (cy as usize) * (canvas_width as usize) + cx as usize;
                sum[dst_index][0] += rgb[0];
                sum[dst_index][1] += rgb[1];
                sum[dst_index][2] += rgb[2];
                count[dst_index] += 1;
            }
        }
    }

    let mut canvas = vec![0u8; canvas_pixels * 4];
    for pixel in 0..canvas_pixels {
        let n = count[pixel].max(1) as f32;
        canvas[pixel * 4] = (sum[pixel][0] / n).round().clamp(0.0, 255.0) as u8;
        canvas[pixel * 4 + 1] = (sum[pixel][1] / n).round().clamp(0.0, 255.0) as u8;
        canvas[pixel * 4 + 2] = (sum[pixel][2] / n).round().clamp(0.0, 255.0) as u8;
        canvas[pixel * 4 + 3] = if count[pixel] > 0 { 255 } else { 0 };
    }
    Ok((canvas_width, canvas_height, canvas))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for chunk in pixels.chunks_exact_mut(4) {
            chunk[0] = r;
            chunk[1] = g;
            chunk[2] = b;
            chunk[3] = 255;
        }
        pixels
    }

    /// Ein Testbild mit verstreuten hellen Quadraten auf dunklem
    /// Hintergrund — jedes Quadrat hat vier echte L-förmige Ecken, die
    /// FAST tatsächlich erkennt. **Bewusst kein Schachbrett**: an dessen
    /// Kreuzungen treffen sich vier Quadranten in einem X-förmigen
    /// Sattelpunkt (zwei helle, zwei dunkle Quadranten alternierend um
    /// den Kreis) — FAST verlangt einen einzigen zusammenhängenden Bogen
    /// von mindestens neun der 16 Kreispunkte, den ein Sattelpunkt nie
    /// liefert (bestätigt: `corners_fast9` findet auf einem echten
    /// Schachbrettmuster null Ecken). Reale Fotos haben dagegen reichlich
    /// echte L-Ecken (Gebäudekanten, Objekträndern), das hier ist nur ein
    /// synthetischer Ersatz dafür — nur für reine Kompositions-Tests
    /// geeignet ([`stitch_homography_with_identity_reproduces_the_reference_image`]),
    /// NICHT fürs Matching (siehe [`scattered_shapes`]).
    fn dotted_grid(width: u32, height: u32, cell: u32, square: u32) -> Vec<u8> {
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let on = (x % cell) < square && (y % cell) < square;
                let value = if on { 230u8 } else { 20u8 };
                let index = ((y * width + x) * 4) as usize;
                pixels[index] = value;
                pixels[index + 1] = value;
                pixels[index + 2] = value;
                pixels[index + 3] = 255;
            }
        }
        pixels
    }

    /// Wie [`dotted_grid`], aber mit **unterschiedlich großen, leicht
    /// versetzten** Quadraten statt eines perfekt periodischen Rasters —
    /// für Matching-Tests unverzichtbar: bei perfekt gleich großen,
    /// exakt periodisch angeordneten Quadraten sieht die lokale BRIEF-
    /// Umgebung (Patch-Radius [`PATCH_RADIUS`]) jeder "oberen linken
    /// Ecke" identisch aus wie die jeder anderen — Lowes Verhältnistest
    /// verwirft solche mehrdeutigen Treffer dann zu Recht (real
    /// beobachtet: mit [`dotted_grid`] blieb `estimate_pairwise_
    /// homographies_rgba8` bei einer echten Verschiebung ergebnislos).
    /// Reale Fotos sind so gut wie nie perfekt periodisch — dieses
    /// Testbild ist die realistischere Näherung.
    fn scattered_shapes(width: u32, height: u32) -> Vec<u8> {
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let mut rng = StdRng::seed_from_u64(0xF00D_5CA7_u64);
        let cell = 32u32;
        for base_y in (0..height).step_by(cell as usize) {
            for base_x in (0..width).step_by(cell as usize) {
                let size = 8 + rng.random_range(0..14u32); // 8..22
                let max_offset = cell.saturating_sub(size).max(1);
                let offset_x = rng.random_range(0..max_offset);
                let offset_y = rng.random_range(0..max_offset);
                let x0 = base_x + offset_x;
                let y0 = base_y + offset_y;
                for y in y0..(y0 + size).min(height) {
                    for x in x0..(x0 + size).min(width) {
                        let index = ((y * width + x) * 4) as usize;
                        pixels[index] = 230;
                        pixels[index + 1] = 230;
                        pixels[index + 2] = 230;
                        pixels[index + 3] = 255;
                    }
                }
            }
        }
        pixels
    }

    #[test]
    fn stitch_homography_rejects_a_single_image() {
        let a = solid(4, 4, 1, 2, 3);
        let result = stitch_homography_rgba8(
            &[HomographyPositionedImage {
                pixels: &a,
                homography: Matrix3::identity(),
            }],
            4,
            4,
        );
        assert!(matches!(result, Err(StackingError::TooFewImages { .. })));
    }

    #[test]
    fn stitch_homography_with_identity_reproduces_the_reference_image() {
        let width = 8u32;
        let height = 8u32;
        let a = dotted_grid(width, height, 2, 1);
        let (canvas_w, canvas_h, canvas) = stitch_homography_rgba8(
            &[
                HomographyPositionedImage {
                    pixels: &a,
                    homography: Matrix3::identity(),
                },
                HomographyPositionedImage {
                    pixels: &a,
                    homography: Matrix3::identity(),
                },
            ],
            width,
            height,
        )
        .expect("sollte zusammensetzen");
        assert_eq!((canvas_w, canvas_h), (width, height));
        // Beide Bilder sind identisch und deckungsgleich (Identität) —
        // das Ergebnis muss exakt dem Original entsprechen (Mittelwert
        // zweier gleicher Werte).
        assert_eq!(canvas, a);
    }

    #[test]
    fn estimate_homography_ransac_recovers_a_known_pure_translation() {
        // Eine reine Verschiebung um (15, -7) — der einfachste nicht
        // entartete Fall, ohne die Merkmalserkennung selbst zu testen
        // (das würde ein echtes Foto brauchen). Mindestens `MIN_INLIERS`
        // (12) Punkte, sonst verwirft `estimate_homography_ransac` das
        // Ergebnis unabhängig von dessen Qualität als "zu wenig Beweise".
        let translation = (15.0f32, -7.0f32);
        let sources: Vec<(f32, f32)> = (0..16)
            .map(|i| {
                let x = 10.0 + (i as f32 % 4.0) * 60.0;
                let y = 10.0 + (i as f32 / 4.0).floor() * 45.0;
                (x, y)
            })
            .collect();
        let matches: Vec<PointCorrespondence> = sources
            .iter()
            .map(|&(x, y)| ((x, y), (x + translation.0, y + translation.1)))
            .collect();
        let h = estimate_homography_ransac(&matches).expect("sollte eine Homografie finden");
        let (px, py) = apply_homography(&h, (50.0, 50.0)).expect("sollte abbilden");
        assert!((px - (50.0 + translation.0 as f64)).abs() < 0.5);
        assert!((py - (50.0 + translation.1 as f64)).abs() < 0.5);
    }

    #[test]
    fn estimate_homography_ransac_ignores_a_minority_of_outlier_matches() {
        // Echtes 2D-Raster statt einer Punktreihe entlang einer einzigen
        // Geraden — kollineare Punkte legen eine Homografie nicht
        // eindeutig fest (klassische DLT-Voraussetzung: Punkte in
        // allgemeiner Lage).
        let translation = (20.0f32, 5.0f32);
        let mut matches: Vec<PointCorrespondence> = (0..20)
            .map(|i| {
                let x = 10.0 + (i as f32 % 5.0) * 40.0;
                let y = 20.0 + (i as f32 / 5.0).floor() * 40.0;
                ((x, y), (x + translation.0, y + translation.1))
            })
            .collect();
        // Fünf klare Ausreißer (ganz andere Zielposition) dazu.
        for i in 0..5 {
            matches.push(((300.0 + i as f32, 300.0), (5.0, 5.0)));
        }
        let h = estimate_homography_ransac(&matches).expect("sollte trotz Ausreißern schätzen");
        let (px, py) = apply_homography(&h, (100.0, 100.0)).expect("sollte abbilden");
        assert!((px - (100.0 + translation.0 as f64)).abs() < 1.0);
        assert!((py - (100.0 + translation.1 as f64)).abs() < 1.0);
    }

    #[test]
    fn estimate_homography_ransac_returns_none_for_too_few_correspondences() {
        let matches = vec![((0.0, 0.0), (1.0, 1.0)), ((10.0, 0.0), (11.0, 1.0))];
        assert!(estimate_homography_ransac(&matches).is_none());
    }

    #[test]
    fn extract_features_finds_real_keypoints_in_scattered_shapes() {
        let image = scattered_shapes(200, 200);
        let features = extract_features(&image, 200, 200);
        assert!(
            !features.is_empty(),
            "verstreute Quadrate mit echten L-Ecken sollten echte Merkmale liefern"
        );
    }

    #[test]
    fn match_features_matches_identical_descriptors_to_themselves() {
        let image = scattered_shapes(200, 200);
        let features = extract_features(&image, 200, 200);
        assert!(features.len() >= 2, "Testvoraussetzung: genug Merkmale");
        let matches = match_features(&features, &features);
        // Jedes Merkmal muss sich selbst als besten Treffer finden
        // (Abstand 0 zu sich selbst, klar unter dem Verhältnistest).
        for &(i, j) in &matches {
            assert_eq!(i, j);
        }
        assert!(!matches.is_empty());
    }

    #[test]
    fn estimate_pairwise_homographies_finds_a_real_homography_for_shifted_scattered_shapes() {
        let width = 200u32;
        let height = 200u32;
        let reference = scattered_shapes(width, height);
        // Zirkuläre Verschiebung — dieselbe Struktur, andere Position, an
        // den unterschiedlich großen Quadraten sollten sich genug
        // übereinstimmende (und dank ihrer unterschiedlichen Größe
        // eindeutig zuordenbare) Merkmale finden.
        let mut shifted = vec![0u8; reference.len()];
        let dx = 11i32;
        let dy = 17i32;
        for y in 0..height as i32 {
            for x in 0..width as i32 {
                let src_x = (x - dx).rem_euclid(width as i32) as u32;
                let src_y = (y - dy).rem_euclid(height as i32) as u32;
                let dst_index = ((y as u32 * width + x as u32) * 4) as usize;
                let src_index = ((src_y * width + src_x) * 4) as usize;
                shifted[dst_index..dst_index + 4]
                    .copy_from_slice(&reference[src_index..src_index + 4]);
            }
        }
        let results =
            estimate_pairwise_homographies_rgba8(&reference, &[shifted.as_slice()], width, height);
        assert_eq!(results.len(), 1);
        let h =
            results[0].expect("sollte für ein strukturiertes Punktraster eine Homografie finden");
        // `h` bildet `shifted`s eigene Koordinaten auf `reference`s
        // Koordinatensystem ab (siehe `estimate_pairwise_homographies_
        // rgba8`s Doku). `shifted` zeigt `reference`s Inhalt um `(dx,dy)`
        // verschoben — ein Punkt bei `(100,100)` in `shifted` zeigt also
        // denselben Bildinhalt wie `reference` bei `(100-dx, 100-dy)`,
        // NICHT `(100+dx, 100+dy)`.
        let (px, py) = apply_homography(&h, (100.0, 100.0)).expect("sollte abbilden");
        assert!((px - (100.0 - dx as f64)).abs() < 3.0);
        assert!((py - (100.0 - dy as f64)).abs() < 3.0);
    }
}
