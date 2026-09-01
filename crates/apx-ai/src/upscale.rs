//! Hochskalierung (Phase 9 Schritt 6, siehe `PLAN.md`/`DECISIONS.md`
//! ADR-0035) — eine echte kantengerichtete Interpolation für die
//! diagonalen Zwischenpixel eines 2×-Upscales, statt reinem bikubischem
//! Resampling (`apx_export::resize`, das für Größenbegrenzung beim
//! Export gedacht ist, nicht für Detailverbesserung).
//!
//! **Dieselbe Ehrlichkeitslinie wie ADR-0033/`denoise.rs`**: kein
//! ONNX-Modell/„Super Resolution"-Netz, ein klassischer, deterministischer
//! Algorithmus (vereinfachtes Kantenrichtungs-Prinzip, verwandt mit New-
//! Edge-Directed-Interpolation/NEDI) — die Aufrufer beschriften dies
//! bewusst nicht als „KI"/„AI".
//!
//! **Algorithmus**: jedes 2×2-Feld benachbarter Originalpixel wird zu
//! einem 4×4-Ausgabefeld — der Originalpixel selbst an geraden
//! Koordinaten, horizontale/vertikale Zwischenpixel per einfacher
//! linearer Interpolation, und das diagonale Zwischenpixel per
//! Kantenrichtungs-Entscheidung: die beiden Diagonalen des 2×2-Feldes
//! (`\` und `/`) werden auf ihre Luminanzdifferenz geprüft — die
//! Diagonale mit der *geringeren* Differenz verläuft vermutlich entlang
//! einer Kante (arm an Kontrast in dieser Richtung bedeutet, eine echte
//! Kante liegt eher quer dazu) und wird stärker gewichtet, um nicht über
//! die Kante hinweg zu mitteln.

fn luminance(r: f32, g: f32, b: f32) -> f32 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn get(pixels: &[u8], width: u32, height: u32, x: i32, y: i32, channel: usize) -> f32 {
    let cx = x.clamp(0, width as i32 - 1);
    let cy = y.clamp(0, height as i32 - 1);
    pixels[((cy as u32 * width + cx as u32) * 4) as usize + channel] as f32
}

fn set(out: &mut [u8], out_width: u32, x: u32, y: u32, rgb: [f32; 3], alpha: u8) {
    let index = ((y * out_width + x) * 4) as usize;
    out[index] = rgb[0].round().clamp(0.0, 255.0) as u8;
    out[index + 1] = rgb[1].round().clamp(0.0, 255.0) as u8;
    out[index + 2] = rgb[2].round().clamp(0.0, 255.0) as u8;
    out[index + 3] = alpha;
}

/// Skaliert `pixels` (RGBA8, `width * height * 4` Bytes) auf das
/// Doppelte in jeder Richtung. Gibt `(neue Breite, neue Höhe, Pixel)`
/// zurück.
pub fn edge_directed_upscale_2x_rgba8(
    pixels: &[u8],
    width: u32,
    height: u32,
) -> (u32, u32, Vec<u8>) {
    if width == 0 || height == 0 {
        return (0, 0, Vec::new());
    }
    let out_width = width * 2;
    let out_height = height * 2;
    let mut out = vec![0u8; (out_width * out_height * 4) as usize];

    let pixel_at = |x: i32, y: i32| -> [f32; 4] {
        [
            get(pixels, width, height, x, y, 0),
            get(pixels, width, height, x, y, 1),
            get(pixels, width, height, x, y, 2),
            get(pixels, width, height, x, y, 3),
        ]
    };

    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let p00 = pixel_at(x, y);
            let p10 = pixel_at(x + 1, y);
            let p01 = pixel_at(x, y + 1);
            let p11 = pixel_at(x + 1, y + 1);

            let ox = x as u32 * 2;
            let oy = y as u32 * 2;

            // Original-Pixel unverändert.
            set(
                &mut out,
                out_width,
                ox,
                oy,
                [p00[0], p00[1], p00[2]],
                p00[3] as u8,
            );
            // Horizontales Zwischenpixel — lineare Interpolation.
            set(
                &mut out,
                out_width,
                ox + 1,
                oy,
                [
                    (p00[0] + p10[0]) / 2.0,
                    (p00[1] + p10[1]) / 2.0,
                    (p00[2] + p10[2]) / 2.0,
                ],
                ((p00[3] + p10[3]) / 2.0) as u8,
            );
            // Vertikales Zwischenpixel — lineare Interpolation.
            set(
                &mut out,
                out_width,
                ox,
                oy + 1,
                [
                    (p00[0] + p01[0]) / 2.0,
                    (p00[1] + p01[1]) / 2.0,
                    (p00[2] + p01[2]) / 2.0,
                ],
                ((p00[3] + p01[3]) / 2.0) as u8,
            );

            // Diagonales Zwischenpixel — kantengerichtet.
            let lum00 = luminance(p00[0], p00[1], p00[2]);
            let lum11 = luminance(p11[0], p11[1], p11[2]);
            let lum10 = luminance(p10[0], p10[1], p10[2]);
            let lum01 = luminance(p01[0], p01[1], p01[2]);
            let backslash_diff = (lum00 - lum11).abs();
            let slash_diff = (lum10 - lum01).abs();

            // Gewichtung invers zur jeweiligen Differenz — die glattere
            // Diagonale bekommt mehr Gewicht (siehe Moduldoku).
            let backslash_weight = slash_diff + 1.0;
            let slash_weight = backslash_diff + 1.0;
            let total = backslash_weight + slash_weight;

            let diag = [
                (backslash_weight * (p00[0] + p11[0]) / 2.0
                    + slash_weight * (p10[0] + p01[0]) / 2.0)
                    / total,
                (backslash_weight * (p00[1] + p11[1]) / 2.0
                    + slash_weight * (p10[1] + p01[1]) / 2.0)
                    / total,
                (backslash_weight * (p00[2] + p11[2]) / 2.0
                    + slash_weight * (p10[2] + p01[2]) / 2.0)
                    / total,
            ];
            let diag_alpha = ((p00[3] + p11[3] + p10[3] + p01[3]) / 4.0) as u8;
            set(&mut out, out_width, ox + 1, oy + 1, diag, diag_alpha);
        }
    }

    (out_width, out_height, out)
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

    #[test]
    fn output_dimensions_are_doubled() {
        let pixels = solid(3, 2, 10, 20, 30);
        let (w, h, out) = edge_directed_upscale_2x_rgba8(&pixels, 3, 2);
        assert_eq!((w, h), (6, 4));
        assert_eq!(out.len(), (6 * 4 * 4) as usize);
    }

    #[test]
    fn uniform_image_stays_uniform() {
        let pixels = solid(4, 4, 100, 150, 200);
        let (w, h, out) = edge_directed_upscale_2x_rgba8(&pixels, 4, 4);
        for chunk in out.chunks_exact(4) {
            assert_eq!(chunk, &[100, 150, 200, 255]);
        }
        assert_eq!((w, h), (8, 8));
    }

    #[test]
    fn original_pixels_land_exactly_on_even_output_coordinates() {
        let pixels = vec![
            10, 10, 10, 255, 200, 200, 200, 255, //
            50, 50, 50, 255, 90, 90, 90, 255,
        ];
        let (w, _h, out) = edge_directed_upscale_2x_rgba8(&pixels, 2, 2);
        let at = |x: u32, y: u32| -> [u8; 4] {
            let i = ((y * w + x) * 4) as usize;
            [out[i], out[i + 1], out[i + 2], out[i + 3]]
        };
        assert_eq!(at(0, 0), [10, 10, 10, 255]);
        assert_eq!(at(2, 0), [200, 200, 200, 255]);
        assert_eq!(at(0, 2), [50, 50, 50, 255]);
        assert_eq!(at(2, 2), [90, 90, 90, 255]);
    }

    #[test]
    fn favors_the_smoother_diagonal_over_a_sharp_edge() {
        // "\"-Diagonale (p00, p11) ist glatt (beide dunkel, Differenz 0) —
        // eine Kante läuft vermutlich entlang dieser Richtung. Die
        // "/"-Diagonale (p10, p01) hat dagegen die volle Differenz (eine
        // davon hell, eine dunkel) — sie kreuzt die vermutete Kante. Das
        // diagonale Zwischenpixel soll deshalb näher an den dunklen
        // "\"-Werten liegen als der naive 4-Pixel-Mittelwert.
        let mut pixels = vec![0u8; 4 * 4];
        let set_px = |pixels: &mut Vec<u8>, i: usize, v: u8| {
            pixels[i * 4] = v;
            pixels[i * 4 + 1] = v;
            pixels[i * 4 + 2] = v;
            pixels[i * 4 + 3] = 255;
        };
        set_px(&mut pixels, 0, 20); // (0,0) = p00
        set_px(&mut pixels, 1, 250); // (1,0) = p10
        set_px(&mut pixels, 2, 10); // (0,1) = p01
        set_px(&mut pixels, 3, 20); // (1,1) = p11

        let (w, _h, out) = edge_directed_upscale_2x_rgba8(&pixels, 2, 2);
        let diag_index = ((w + 1) * 4) as usize;
        let naive_average = (20.0 + 250.0 + 10.0 + 20.0) / 4.0; // 75
        assert!(
            (out[diag_index] as f32 - naive_average).abs() > 30.0,
            "muss deutlich von der naiven Mittelung abweichen"
        );
        assert!(
            out[diag_index] < 60,
            "muss näher an der dunklen \\-Diagonale liegen"
        );
    }
}
