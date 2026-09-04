//! Inhaltssensitives Skalieren (Content-Aware Scale / Seam Carving,
//! Phase 15 Schritt 4, siehe `DECISIONS.md` ADR-0042 — Photoshop-
//! exklusiv seit CS4, Lightroom kann nur gleichmäßig skalieren/
//! zuschneiden). Klassischer Algorithmus (Avidan & Shamir, SIGGRAPH
//! 2007), selbst implementiert — das einzige Crate auf crates.io
//! (`seamcarving`) ist LGPL-3.0-or-later lizenziert, nicht dieselbe
//! durchgehend permissive Linie wie jede andere Abhängigkeit dieses
//! Projekts (siehe ADR-0042). Keine ONNX-Inferenz, reine
//! deterministische Bildverarbeitung, deshalb (wie `sky_replace::
//! composite`) infallibel statt `Result`.
//!
//! **Verfahren:** Energie = Gradientenbetrag der Luminanz (zentrale
//! Differenzen), optional zusätzlich gewichtet mit einer Schutzmaske
//! (`protect`, z. B. [`crate::segmentation::person_alpha`] — schützt
//! erkannte Personen/Gesichter automatisch vor Verzerrung). Dynamische
//! Programmierung sucht je Iteration die kostengünstigste Naht (ein
//! Pixel je Zeile), die dann entweder entfernt (Verkleinern) oder
//! mit ihrem Nachbarn gemittelt dupliziert wird (Vergrößern).
//! Höhenänderung läuft über **dieselbe** Nahtsuche wie die Breite — das
//! Bild wird dafür einmal transponiert, nach der Anpassung zurück,
//! damit der Algorithmus nicht doppelt (einmal je Richtung) gepflegt
//! werden muss.

/// Wie stark `protect` die Nahtsuche von geschützten Pixeln fernhält —
/// von Hand austariert: groß genug, dass eine Naht durch einen
/// vollständig geschützten Bereich (`protect == 255`) fast immer
/// teurer ist als jede Alternative am Bildrand.
const PROTECTION_WEIGHT: f32 = 6.0;

fn luminance_map(pixels: &[u8], width: usize, height: usize) -> Vec<f32> {
    let n = width * height;
    let mut out = vec![0f32; n];
    for i in 0..n {
        let r = pixels[i * 3] as f32 / 255.0;
        let g = pixels[i * 3 + 1] as f32 / 255.0;
        let b = pixels[i * 3 + 2] as f32 / 255.0;
        out[i] = 0.3 * r + 0.59 * g + 0.11 * b;
    }
    out
}

fn energy_map(luminance: &[f32], width: usize, height: usize, protect: Option<&[u8]>) -> Vec<f32> {
    let at = |x: i32, y: i32| -> f32 {
        let cx = x.clamp(0, width as i32 - 1) as usize;
        let cy = y.clamp(0, height as i32 - 1) as usize;
        luminance[cy * width + cx]
    };
    let mut energy = vec![0f32; width * height];
    for y in 0..height {
        for x in 0..width {
            let dx = at(x as i32 + 1, y as i32) - at(x as i32 - 1, y as i32);
            let dy = at(x as i32, y as i32 + 1) - at(x as i32, y as i32 - 1);
            let mut e = (dx * dx + dy * dy).sqrt();
            if let Some(p) = protect {
                e += (p[y * width + x] as f32 / 255.0) * PROTECTION_WEIGHT;
            }
            energy[y * width + x] = e;
        }
    }
    energy
}

/// Günstigste vertikale Naht per dynamischer Programmierung — ein
/// x-Wert je Zeile, `seam[0]` oben. Verfolgt Rückverweise explizit
/// während des Vorwärtsdurchlaufs (statt sie hinterher aus den
/// aufsummierten Kosten zurückzurechnen) — robuster als ein
/// Float-Vergleich, der bei Rundungsfehlern danebenliegen könnte.
fn find_vertical_seam(energy: &[f32], width: usize, height: usize) -> Vec<usize> {
    let mut cost = energy.to_vec();
    let mut back = vec![0usize; width * height];
    for y in 1..height {
        for x in 0..width {
            let mut best_x = x;
            let mut best_cost = f32::MAX;
            let candidates = [
                x.checked_sub(1),
                Some(x),
                if x + 1 < width { Some(x + 1) } else { None },
            ];
            for cand in candidates.into_iter().flatten() {
                let c = cost[(y - 1) * width + cand];
                if c < best_cost {
                    best_cost = c;
                    best_x = cand;
                }
            }
            back[y * width + x] = best_x;
            cost[y * width + x] += best_cost;
        }
    }
    let last = height - 1;
    let mut min_x = 0;
    let mut min_val = f32::MAX;
    for x in 0..width {
        let c = cost[last * width + x];
        if c < min_val {
            min_val = c;
            min_x = x;
        }
    }
    let mut seam = vec![0usize; height];
    seam[last] = min_x;
    let mut x = min_x;
    for y in (0..last).rev() {
        x = back[(y + 1) * width + x];
        seam[y] = x;
    }
    seam
}

fn remove_seam_rgb8(pixels: &[u8], width: usize, height: usize, seam: &[usize]) -> Vec<u8> {
    let new_width = width - 1;
    let mut out = vec![0u8; new_width * height * 3];
    for (y, &skip_x) in seam.iter().enumerate() {
        let mut dst_x = 0;
        for x in 0..width {
            if x == skip_x {
                continue;
            }
            let src = (y * width + x) * 3;
            let dst = (y * new_width + dst_x) * 3;
            out[dst..dst + 3].copy_from_slice(&pixels[src..src + 3]);
            dst_x += 1;
        }
    }
    out
}

fn remove_seam_u8(map: &[u8], width: usize, height: usize, seam: &[usize]) -> Vec<u8> {
    let new_width = width - 1;
    let mut out = vec![0u8; new_width * height];
    for (y, &skip_x) in seam.iter().enumerate() {
        let mut dst_x = 0;
        for x in 0..width {
            if x == skip_x {
                continue;
            }
            out[y * new_width + dst_x] = map[y * width + x];
            dst_x += 1;
        }
    }
    out
}

/// Fügt eine Naht ein (Vergrößern) — der neue Pixel ist der Mittelwert
/// aus dem Nahtpixel und seinem rechten Nachbarn, damit die neue Spalte
/// nicht wie eine harte Doppelkante wirkt.
fn insert_seam_rgb8(pixels: &[u8], width: usize, height: usize, seam: &[usize]) -> Vec<u8> {
    let new_width = width + 1;
    let mut out = vec![0u8; new_width * height * 3];
    for (y, &seam_x) in seam.iter().enumerate() {
        let mut dst_x = 0;
        for x in 0..width {
            let src = (y * width + x) * 3;
            let dst = (y * new_width + dst_x) * 3;
            out[dst..dst + 3].copy_from_slice(&pixels[src..src + 3]);
            dst_x += 1;
            if x == seam_x {
                let next_x = (x + 1).min(width - 1);
                let next_src = (y * width + next_x) * 3;
                let dst2 = (y * new_width + dst_x) * 3;
                for c in 0..3 {
                    out[dst2 + c] =
                        ((pixels[src + c] as u16 + pixels[next_src + c] as u16) / 2) as u8;
                }
                dst_x += 1;
            }
        }
    }
    out
}

fn insert_seam_u8(map: &[u8], width: usize, height: usize, seam: &[usize]) -> Vec<u8> {
    let new_width = width + 1;
    let mut out = vec![0u8; new_width * height];
    for (y, &seam_x) in seam.iter().enumerate() {
        let mut dst_x = 0;
        for x in 0..width {
            out[y * new_width + dst_x] = map[y * width + x];
            dst_x += 1;
            if x == seam_x {
                let next_x = (x + 1).min(width - 1);
                out[y * new_width + dst_x] =
                    ((map[y * width + x] as u16 + map[y * width + next_x] as u16) / 2) as u8;
                dst_x += 1;
            }
        }
    }
    out
}

fn transpose_rgb8(pixels: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 3];
    for y in 0..height {
        for x in 0..width {
            let src = (y * width + x) * 3;
            let dst = (x * height + y) * 3;
            out[dst..dst + 3].copy_from_slice(&pixels[src..src + 3]);
        }
    }
    out
}

fn transpose_u8(map: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height];
    for y in 0..height {
        for x in 0..width {
            out[x * height + y] = map[y * width + x];
        }
    }
    out
}

/// Passt die Breite einer `width`×`height`-Bitmap (samt optionaler
/// gleich großer Schutzmaske) auf `target_width` an — wiederholtes
/// Entfernen (Verkleinern) oder Einfügen (Vergrößern) einzelner
/// vertikaler Nähte, je eine pro Iteration.
fn resize_width(
    mut pixels: Vec<u8>,
    mut width: usize,
    height: usize,
    target_width: usize,
    mut protect: Option<Vec<u8>>,
) -> (Vec<u8>, Option<Vec<u8>>) {
    while width > target_width {
        let luminance = luminance_map(&pixels, width, height);
        let energy = energy_map(&luminance, width, height, protect.as_deref());
        let seam = find_vertical_seam(&energy, width, height);
        pixels = remove_seam_rgb8(&pixels, width, height, &seam);
        protect = protect.map(|p| remove_seam_u8(&p, width, height, &seam));
        width -= 1;
    }
    while width < target_width {
        let luminance = luminance_map(&pixels, width, height);
        let energy = energy_map(&luminance, width, height, protect.as_deref());
        let seam = find_vertical_seam(&energy, width, height);
        pixels = insert_seam_rgb8(&pixels, width, height, &seam);
        protect = protect.map(|p| insert_seam_u8(&p, width, height, &seam));
        width += 1;
    }
    (pixels, protect)
}

/// Passt `pixels` (interleaved RGB `u8`, `width * height * 3` Bytes) auf
/// `target_width`×`target_height` an. `protect` (optional, `u8`,
/// `0..=255`, dieselbe Bildgröße wie `pixels`) gewichtet die Nahtsuche
/// zusätzlich — höhere Werte werden von Nähten gemieden, z. B. mit
/// [`crate::segmentation::person_alpha`]s Ausgabe, damit erkannte
/// Personen automatisch geschützt bleiben.
pub fn resize_rgb8(
    pixels: &[u8],
    width: u32,
    height: u32,
    target_width: u32,
    target_height: u32,
    protect: Option<&[u8]>,
) -> (u32, u32, Vec<u8>) {
    let (w, h) = (width as usize, height as usize);
    if w == 0 || h == 0 {
        return (width, height, pixels.to_vec());
    }
    let target_w = (target_width.max(1) as usize).min(w * 4);
    let target_h = (target_height.max(1) as usize).min(h * 4);

    let (pixels, protect) =
        resize_width(pixels.to_vec(), w, h, target_w, protect.map(|p| p.to_vec()));

    // Höhe: Bild (+ Maske) transponieren, dieselbe Breiten-Logik
    // anwenden, zurücktransponieren.
    let transposed_pixels = transpose_rgb8(&pixels, target_w, h);
    let transposed_protect = protect.map(|p| transpose_u8(&p, target_w, h));
    let (resized, _protect) =
        resize_width(transposed_pixels, h, target_w, target_h, transposed_protect);
    let result = transpose_rgb8(&resized, target_h, target_w);

    (target_w as u32, target_h as u32, result)
}
