//! Video-Export (Phase 8 Schritt 4, `PLAN.md`: „Übergänge/Ken-Burns-Effekt/
//! Intro-Outro-Screens: reine Frontend-Canvas-Wiedergabe … Video-Export
//! (MP4): `std::process::Command` prüft `ffmpeg -version` beim Start;
//! vorhanden → echter Export über den System-Encoder, sonst klare
//! Fehlermeldung").
//!
//! Die *Live-Wiedergabe* der Diashow (Übergänge, Ken-Burns-Effekt, Intro-/
//! Outro-Screens, Musik-Synchronisation) läuft komplett im Frontend
//! (`SlideshowPlayer.tsx`, `<canvas>` + `<audio>`) — dieses Modul bildet
//! dieselbe Zeitachse zusätzlich in Rust nach, ausschließlich für den
//! *Video-Export*, damit die erzeugte MP4-Datei dieselbe Abfolge zeigt wie
//! die Vorschau. Es rendert kein Foto selbst (bekommt die bereits über
//! `engine::render_to_pixels` gerenderten Pixel als [`TimelineSlide`]) und
//! kennt keine Katalog-/Dateisystempfade — reine Bild-/Zeitachsen-Mathematik
//! plus das Anstoßen des System-`ffmpeg`-Prozesses.
//!
//! **Bewusste Vereinfachung (wie `print.rs`s feste Bilderpaket-Vorlagen):**
//! nur zwei Übergangsarten — [`TransitionKind::Cut`] (harter Schnitt) und
//! [`TransitionKind::CrossFade`] (Überblendung), kein Wipe/Slide. Eine
//! Überblendung mischt die beiden *eingefrorenen* Ken-Burns-Endzustände der
//! benachbarten Folien (Foto A bei Fortschritt 1.0, Foto B bei Fortschritt
//! 0.0) statt während der Überblendung selbst weiterzuzoomen/-zuschwenken —
//! deutlich einfacher zu berechnen und zu testen, sichtbar kaum anders.
//!
//! **Musik-Synchronisation:** die Gesamtlänge des exportierten Videos ist
//! immer durch die Foto-/Übergangsanzahl bestimmt (Pipe-Frameanzahl); ist
//! eine Musikdatei kürzer, endet das Video mit ihr (`-shortest`) statt sie
//! zu wiederholen oder in Stille zu enden — keine Audio-Loop-Logik.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::error::{ExportError, Result};
use crate::resize;
use crate::watermark::{self, WatermarkPosition};

/// Interpoliert Zoom (`1.0` = ganzes Bild sichtbar, größer = näher
/// herangezoomt) und Schwenk-Mittelpunkt (normiert `0.0..=1.0`) linear
/// zwischen einem Start- und Endzustand — der eigentliche Ken-Burns-Effekt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KenBurnsSpec {
    pub zoom_start: f32,
    pub zoom_end: f32,
    pub pan_start: (f32, f32),
    pub pan_end: (f32, f32),
}

impl KenBurnsSpec {
    /// Kein Zoom/Schwenk — das ganze Bild bleibt die gesamte Anzeigedauer
    /// über sichtbar (Ken-Burns-Effekt ausgeschaltet, oder für Titelkarten,
    /// die ohnehin keinen Effekt bekommen).
    pub const STATIC: Self = Self {
        zoom_start: 1.0,
        zoom_end: 1.0,
        pan_start: (0.5, 0.5),
        pan_end: (0.5, 0.5),
    };

    /// Der normierte Bildausschnitt (`x, y, w, h`, alle `0.0..=1.0`) bei
    /// `progress` (`0.0` = Start, `1.0` = Ende, dazwischen linear
    /// interpoliert; außerhalb wird geklemmt).
    fn crop_rect_at(&self, progress: f32) -> (f32, f32, f32, f32) {
        let t = progress.clamp(0.0, 1.0);
        let zoom = (self.zoom_start + (self.zoom_end - self.zoom_start) * t).max(1.0);
        let cx = self.pan_start.0 + (self.pan_end.0 - self.pan_start.0) * t;
        let cy = self.pan_start.1 + (self.pan_end.1 - self.pan_start.1) * t;
        let w = 1.0 / zoom;
        let h = 1.0 / zoom;
        let x = (cx - w / 2.0).clamp(0.0, (1.0 - w).max(0.0));
        let y = (cy - h / 2.0).clamp(0.0, (1.0 - h).max(0.0));
        (x, y, w, h)
    }
}

/// Ein deterministisches, aber je Folienindex unterschiedliches Ken-Burns-
/// Muster (abwechselnd Heranzoomen/Wegzoomen, fünf verschiedene
/// Schwenkziele) — keine echte Zufälligkeit, damit derselbe Foto-Satz bei
/// jedem Export identisch aussieht. `enabled = false` liefert
/// [`KenBurnsSpec::STATIC`] (Effekt im Dialog abgeschaltet).
pub fn default_ken_burns(index: usize, enabled: bool) -> KenBurnsSpec {
    if !enabled {
        return KenBurnsSpec::STATIC;
    }
    const TARGETS: [(f32, f32); 5] = [(0.3, 0.3), (0.7, 0.3), (0.3, 0.7), (0.7, 0.7), (0.5, 0.4)];
    const MAX_ZOOM: f32 = 1.25;
    let target = TARGETS[index % TARGETS.len()];
    if index.is_multiple_of(2) {
        KenBurnsSpec {
            zoom_start: 1.0,
            zoom_end: MAX_ZOOM,
            pan_start: (0.5, 0.5),
            pan_end: target,
        }
    } else {
        KenBurnsSpec {
            zoom_start: MAX_ZOOM,
            zoom_end: 1.0,
            pan_start: target,
            pan_end: (0.5, 0.5),
        }
    }
}

/// Eine einzelne Folie der Zeitachse — entweder ein bereits gerendertes
/// Foto (mit Ken-Burns-Effekt) oder eine vorgerenderte Titelkarte
/// (Intro/Outro, siehe [`render_title_card`] — kein Effekt, `progress` wird
/// beim Rendern ignoriert).
#[derive(Debug, Clone)]
pub enum TimelineSlide {
    Photo {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
        ken_burns: KenBurnsSpec,
        hold_seconds: f32,
    },
    Title {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
        hold_seconds: f32,
    },
}

impl TimelineSlide {
    fn hold_seconds(&self) -> f32 {
        match self {
            Self::Photo { hold_seconds, .. } | Self::Title { hold_seconds, .. } => *hold_seconds,
        }
    }

    /// Rendert diese Folie bei `progress` (`0.0..=1.0` innerhalb ihrer
    /// eigenen Anzeigedauer) auf `output_w`×`output_h`.
    fn render_at(&self, progress: f32, output_w: u32, output_h: u32) -> Result<Vec<u8>> {
        match self {
            Self::Title {
                width,
                height,
                rgba,
                ..
            } => resize::resize_rgba8(*width, *height, rgba, output_w, output_h),
            Self::Photo {
                width,
                height,
                rgba,
                ken_burns,
                ..
            } => {
                let crop_rect = ken_burns.crop_rect_at(progress);
                let (nx, ny, nw, nh) = cover_adjust(crop_rect, *width, *height, output_w, output_h);
                let (crop_w, crop_h, cropped) = crop_rgba8(*width, *height, rgba, nx, ny, nw, nh)?;
                resize::resize_rgba8(crop_w, crop_h, &cropped, output_w, output_h)
            }
        }
    }
}

/// Passt den Ken-Burns-Ausschnitt (`x, y, w, h`, normiert) zusätzlich an
/// das Seitenverhältnis der Ausgabefläche an ("cover" — wie CSS
/// `object-fit: cover`, dieselbe Idee wie `print::FitMode::Cover"). Ohne
/// das würde ein z. B. hochformatiges Foto auf eine querformatige
/// Videoauflösung verzerrt gestreckt statt beschnitten. Dieselbe Formel
/// (nur in normierten statt Pixel-Koordinaten) wie
/// `frontend/src/lib/slideshow.ts`s `coverAdjustedSourceRect` — siehe
/// Moduldoku zur bewussten Doppelung.
fn cover_adjust(
    rect: (f32, f32, f32, f32),
    img_w: u32,
    img_h: u32,
    target_w: u32,
    target_h: u32,
) -> (f32, f32, f32, f32) {
    let (x, y, w, h) = rect;
    if img_w == 0 || img_h == 0 || target_w == 0 || target_h == 0 || w <= 0.0 || h <= 0.0 {
        return (x, y, w, h);
    }
    let crop_w_px = w * img_w as f32;
    let crop_h_px = h * img_h as f32;
    let crop_aspect = crop_w_px / crop_h_px;
    let target_aspect = target_w as f32 / target_h as f32;

    if crop_aspect > target_aspect {
        let new_w_px = crop_h_px * target_aspect;
        let new_w = new_w_px / img_w as f32;
        (x + (w - new_w) / 2.0, y, new_w, h)
    } else {
        let new_h_px = crop_w_px / target_aspect;
        let new_h = new_h_px / img_h as f32;
        (x, y + (h - new_h) / 2.0, w, new_h)
    }
}

/// Schneidet den normierten Ausschnitt (`nx, ny, nw, nh`, alle
/// `0.0..=1.0`) aus einem interleaved-RGBA8-Puffer aus. Gibt die
/// tatsächliche Pixelgröße des Ausschnitts mit zurück (Rundung kann sie um
/// ein Pixel von `nw * width` abweichen lassen).
fn crop_rgba8(
    width: u32,
    height: u32,
    rgba: &[u8],
    nx: f32,
    ny: f32,
    nw: f32,
    nh: f32,
) -> Result<(u32, u32, Vec<u8>)> {
    let expected_len = width as usize * height as usize * 4;
    if rgba.len() != expected_len {
        return Err(ExportError::Unsupported(format!(
            "Pufferlänge {} passt nicht zu {width}x{height} RGBA8",
            rgba.len()
        )));
    }
    let x0 = ((nx * width as f32).round() as i64).clamp(0, width as i64 - 1) as u32;
    let y0 = ((ny * height as f32).round() as i64).clamp(0, height as i64 - 1) as u32;
    let crop_w = ((nw * width as f32).round().max(1.0) as u32).min(width - x0);
    let crop_h = ((nh * height as f32).round().max(1.0) as u32).min(height - y0);

    let mut out = vec![0u8; crop_w as usize * crop_h as usize * 4];
    for row in 0..crop_h {
        let src_start = ((y0 + row) as usize * width as usize + x0 as usize) * 4;
        let src_end = src_start + crop_w as usize * 4;
        let dst_start = row as usize * crop_w as usize * 4;
        out[dst_start..dst_start + crop_w as usize * 4].copy_from_slice(&rgba[src_start..src_end]);
    }
    Ok((crop_w, crop_h, out))
}

/// Baut eine Titelkarte (Intro/Outro) — einfarbiger Hintergrund plus
/// mittig zentrierter Text, per [`watermark::apply_text_watermark`]
/// gerastert (kein zweiter Textrasterisierungs-Pfad). `text` leer lässt
/// den reinen Hintergrund stehen — eine Titelkarte ohne Text ist dann ein
/// einfacher Übergangsbildschirm (z. B. Schwarzblende).
#[allow(clippy::too_many_arguments)]
pub fn render_title_card(
    width: u32,
    height: u32,
    background_rgb: [u8; 3],
    text: &str,
    font_bytes: Option<&[u8]>,
    font_size_px: f32,
    text_color: [u8; 3],
) -> Result<Vec<u8>> {
    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    for px in pixels.chunks_exact_mut(4) {
        px[0] = background_rgb[0];
        px[1] = background_rgb[1];
        px[2] = background_rgb[2];
        px[3] = 255;
    }
    if !text.is_empty() {
        let font_bytes = font_bytes.ok_or_else(|| {
            ExportError::Unsupported(
                "Titeltext gesetzt, aber keine Schriftdatei ausgewählt".to_string(),
            )
        })?;
        watermark::apply_text_watermark(
            width,
            height,
            &mut pixels,
            font_bytes,
            text,
            font_size_px,
            text_color,
            WatermarkPosition::Center,
            1.0,
            0,
        )?;
    }
    Ok(pixels)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionKind {
    Cut,
    CrossFade,
}

/// Eine einzelne Zeile des Frame-Plans — welche Folie(n) `render_frame`
/// für diesen Ausgabeframe braucht. Reine Daten, unabhängig von den
/// tatsächlichen Pixeln (siehe Moduldoku: Überblendung mischt zwei
/// eingefrorene Endzustände).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FramePlanEntry {
    Hold {
        slide_index: usize,
        progress: f32,
    },
    Transition {
        from_index: usize,
        to_index: usize,
        alpha: f32,
    },
}

/// Baut die vollständige Frame-für-Frame-Liste für `slides` bei `fps` —
/// jede Folie trägt ihre eigene `hold_seconds` (mindestens ein Frame,
/// selbst bei `hold_seconds` nahe null), zwischen zwei Folien optional
/// `transition_seconds` Überblendungsframes. Leer bei leeren `slides` oder
/// `fps == 0`.
pub fn build_frame_plan(
    slides: &[TimelineSlide],
    fps: u32,
    transition: TransitionKind,
    transition_seconds: f32,
) -> Vec<FramePlanEntry> {
    let mut plan = Vec::new();
    if slides.is_empty() || fps == 0 {
        return plan;
    }
    let transition_frames = match transition {
        TransitionKind::Cut => 0,
        TransitionKind::CrossFade => (transition_seconds.max(0.0) * fps as f32).round() as u32,
    };

    for (i, slide) in slides.iter().enumerate() {
        let hold_frames = ((slide.hold_seconds() * fps as f32).round() as u32).max(1);
        for f in 0..hold_frames {
            let progress = if hold_frames <= 1 {
                1.0
            } else {
                f as f32 / (hold_frames - 1) as f32
            };
            plan.push(FramePlanEntry::Hold {
                slide_index: i,
                progress,
            });
        }
        if transition_frames > 0 && i + 1 < slides.len() {
            for f in 0..transition_frames {
                let alpha = (f + 1) as f32 / transition_frames as f32;
                plan.push(FramePlanEntry::Transition {
                    from_index: i,
                    to_index: i + 1,
                    alpha,
                });
            }
        }
    }
    plan
}

/// Rendert einen einzelnen Ausgabeframe gemäß `entry` — bei einer
/// Überblendung werden die beiden eingefrorenen Endzustände (Fortschritt
/// `1.0` bzw. `0.0`, siehe Moduldoku) alpha-gemischt.
pub fn render_frame(
    slides: &[TimelineSlide],
    entry: FramePlanEntry,
    output_w: u32,
    output_h: u32,
) -> Result<Vec<u8>> {
    match entry {
        FramePlanEntry::Hold {
            slide_index,
            progress,
        } => slides
            .get(slide_index)
            .ok_or_else(|| {
                ExportError::Unsupported(format!("Folienindex {slide_index} außerhalb"))
            })?
            .render_at(progress, output_w, output_h),
        FramePlanEntry::Transition {
            from_index,
            to_index,
            alpha,
        } => {
            let from_frame = slides
                .get(from_index)
                .ok_or_else(|| {
                    ExportError::Unsupported(format!("Folienindex {from_index} außerhalb"))
                })?
                .render_at(1.0, output_w, output_h)?;
            let to_frame = slides
                .get(to_index)
                .ok_or_else(|| {
                    ExportError::Unsupported(format!("Folienindex {to_index} außerhalb"))
                })?
                .render_at(0.0, output_w, output_h)?;
            Ok(blend_crossfade(&from_frame, &to_frame, alpha))
        }
    }
}

fn blend_crossfade(from: &[u8], to: &[u8], alpha: f32) -> Vec<u8> {
    let alpha = alpha.clamp(0.0, 1.0);
    from.iter()
        .zip(to.iter())
        .map(|(&a, &b)| {
            (a as f32 * (1.0 - alpha) + b as f32 * alpha)
                .round()
                .clamp(0.0, 255.0) as u8
        })
        .collect()
}

/// Prüft, ob ein System-`ffmpeg` im `PATH` aufrufbar ist (`ffmpeg
/// -version`) — genau die in `PLAN.md` Schritt 4 vorgesehene Prüfung.
/// Kein Bundling eines eigenen `ffmpeg`-Binaries (Lizenz-/Bündelungsaufwand
/// je Plattform, siehe `DECISIONS.md` ADR-0034).
pub fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[derive(Debug, Clone)]
pub struct VideoExportOptions {
    pub output_width: u32,
    pub output_height: u32,
    pub fps: u32,
    /// Pfad zu einer Audiodatei (beliebiges von `ffmpeg` unterstütztes
    /// Format) — `None` exportiert stumm.
    pub audio_path: Option<PathBuf>,
}

/// Ergebnis eines Video-Exports — die tatsächliche Länge (aus der
/// gerenderten Frameanzahl, nicht aus der Audiodatei — siehe Moduldoku).
#[derive(Debug, Clone, Copy)]
pub struct VideoExportOutcome {
    pub frame_count: usize,
    pub duration_seconds: f32,
}

/// Rendert `slides` gemäß `transition`/`transition_seconds` Frame für
/// Frame und pipet sie roh (RGBA8) in einen `ffmpeg`-Prozess, der sie zu
/// `dest_path` als H.264-MP4 kodiert — ein Frame nach dem anderen im
/// Speicher, kein Zwischenspeichern einer Bildsequenz auf der Platte.
/// Schlägt mit [`ExportError::Video`] fehl, wenn `ffmpeg` fehlt oder sich
/// nicht sauber beendet (siehe [`ffmpeg_available`]).
pub fn export_slideshow_video(
    slides: &[TimelineSlide],
    transition: TransitionKind,
    transition_seconds: f32,
    options: &VideoExportOptions,
    dest_path: &Path,
) -> Result<VideoExportOutcome> {
    if !ffmpeg_available() {
        return Err(ExportError::Video {
            message: "ffmpeg wurde nicht gefunden (im PATH nicht aufrufbar) — bitte installieren, \
                      Video-Export ist ohne System-ffmpeg nicht möglich (siehe DECISIONS.md ADR-0034)"
                .to_string(),
        });
    }
    if slides.is_empty() {
        return Err(ExportError::Unsupported(
            "Diashow enthält keine Folien".to_string(),
        ));
    }
    if options.output_width == 0 || options.output_height == 0 || options.fps == 0 {
        return Err(ExportError::Unsupported(
            "Video-Auflösung/Bildrate muss größer null sein".to_string(),
        ));
    }

    let plan = build_frame_plan(slides, options.fps, transition, transition_seconds);

    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-y")
        .arg("-f")
        .arg("rawvideo")
        .arg("-pix_fmt")
        .arg("rgba")
        .arg("-video_size")
        .arg(format!(
            "{}x{}",
            options.output_width, options.output_height
        ))
        .arg("-framerate")
        .arg(options.fps.to_string())
        .arg("-i")
        .arg("pipe:0");

    if let Some(audio_path) = &options.audio_path {
        cmd.arg("-i").arg(audio_path);
    }

    cmd.arg("-c:v")
        .arg("libx264")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-r")
        .arg(options.fps.to_string())
        .arg("-movflags")
        .arg("+faststart");

    if options.audio_path.is_some() {
        // Video-Länge bleibt maßgeblich für die Bildfolge, aber das
        // Gesamtergebnis endet mit dem kürzeren der beiden Streams (siehe
        // Moduldoku) statt die Musik zu wiederholen.
        cmd.arg("-c:a").arg("aac").arg("-shortest");
    }
    cmd.arg(dest_path);

    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|err| ExportError::Video {
        message: format!("ffmpeg konnte nicht gestartet werden: {err}"),
    })?;
    let mut stdin = child.stdin.take().ok_or_else(|| ExportError::Video {
        message: "ffmpeg-Standardeingabe nicht verfügbar".to_string(),
    })?;

    for entry in &plan {
        let frame = render_frame(slides, *entry, options.output_width, options.output_height)?;
        if let Err(err) = stdin.write_all(&frame) {
            // ffmpeg kann seine Standardeingabe schon vor dem letzten Frame
            // schließen (z. B. wenn es selbst früh fehlschlägt) — der echte
            // Fehler steht dann in stderr, nicht im Schreibfehler selbst.
            drop(stdin);
            let output = child.wait_with_output();
            let detail = output
                .map(|o| String::from_utf8_lossy(&o.stderr).to_string())
                .unwrap_or_else(|_| err.to_string());
            return Err(ExportError::Video {
                message: format!("Frame konnte nicht an ffmpeg gesendet werden: {detail}"),
            });
        }
    }
    drop(stdin);

    let output = child.wait_with_output().map_err(|err| ExportError::Video {
        message: format!("ffmpeg-Prozess fehlgeschlagen: {err}"),
    })?;
    if !output.status.success() {
        return Err(ExportError::Video {
            message: format!(
                "ffmpeg beendete sich mit Fehler: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    Ok(VideoExportOutcome {
        frame_count: plan.len(),
        duration_seconds: plan.len() as f32 / options.fps as f32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_photo(width: u32, height: u32, rgb: [u8; 3], hold_seconds: f32) -> TimelineSlide {
        let rgba: Vec<u8> = (0..width * height)
            .flat_map(|_| [rgb[0], rgb[1], rgb[2], 255])
            .collect();
        TimelineSlide::Photo {
            width,
            height,
            rgba,
            ken_burns: KenBurnsSpec::STATIC,
            hold_seconds,
        }
    }

    // ---- KenBurnsSpec ---------------------------------------------------

    #[test]
    fn static_ken_burns_covers_the_whole_image_at_any_progress() {
        for progress in [0.0, 0.3, 1.0] {
            let (x, y, w, h) = KenBurnsSpec::STATIC.crop_rect_at(progress);
            assert_eq!((x, y, w, h), (0.0, 0.0, 1.0, 1.0));
        }
    }

    #[test]
    fn zooming_in_shrinks_the_crop_rectangle_over_time() {
        let spec = KenBurnsSpec {
            zoom_start: 1.0,
            zoom_end: 2.0,
            pan_start: (0.5, 0.5),
            pan_end: (0.5, 0.5),
        };
        let (_, _, w0, h0) = spec.crop_rect_at(0.0);
        let (_, _, w1, h1) = spec.crop_rect_at(1.0);
        assert_eq!((w0, h0), (1.0, 1.0));
        assert!((w1 - 0.5).abs() < 1e-5 && (h1 - 0.5).abs() < 1e-5);
    }

    #[test]
    fn crop_rectangle_never_leaves_the_source_bounds() {
        let spec = KenBurnsSpec {
            zoom_start: 3.0,
            zoom_end: 3.0,
            pan_start: (0.0, 0.0),
            pan_end: (1.0, 1.0),
        };
        for progress in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let (x, y, w, h) = spec.crop_rect_at(progress);
            assert!(x >= 0.0 && y >= 0.0);
            assert!(x + w <= 1.0 + 1e-5);
            assert!(y + h <= 1.0 + 1e-5);
        }
    }

    #[test]
    fn default_ken_burns_disabled_is_static() {
        assert_eq!(default_ken_burns(0, false), KenBurnsSpec::STATIC);
        assert_eq!(default_ken_burns(7, false), KenBurnsSpec::STATIC);
    }

    #[test]
    fn default_ken_burns_alternates_zoom_direction_by_index() {
        let a = default_ken_burns(0, true);
        let b = default_ken_burns(1, true);
        assert!(a.zoom_start < a.zoom_end, "gerader Index: Heranzoomen");
        assert!(b.zoom_start > b.zoom_end, "ungerader Index: Wegzoomen");
    }

    // ---- cover_adjust -------------------------------------------------------

    #[test]
    fn cover_adjust_is_a_no_op_when_aspect_ratios_already_match() {
        let adjusted = cover_adjust((0.1, 0.2, 0.5, 0.5), 100, 100, 200, 200);
        assert_eq!(adjusted, (0.1, 0.2, 0.5, 0.5));
    }

    #[test]
    fn cover_adjust_narrows_a_too_wide_crop_for_a_taller_target() {
        // Ganzes 200x100-Bild (2:1) auf eine 1:1-Zielfläche — die Breite
        // muss auf die Höhe reduziert werden, zentriert.
        let (x, y, w, h) = cover_adjust((0.0, 0.0, 1.0, 1.0), 200, 100, 100, 100);
        assert_eq!((y, h), (0.0, 1.0));
        assert!((w - 0.5).abs() < 1e-5); // 100px von 200px Breite bleiben übrig
        assert!((x - 0.25).abs() < 1e-5); // zentriert
    }

    #[test]
    fn cover_adjust_narrows_a_too_tall_crop_for_a_wider_target() {
        let (x, y, w, h) = cover_adjust((0.0, 0.0, 1.0, 1.0), 100, 200, 100, 100);
        assert_eq!((x, w), (0.0, 1.0));
        assert!((h - 0.5).abs() < 1e-5);
        assert!((y - 0.25).abs() < 1e-5);
    }

    // ---- crop_rgba8 -------------------------------------------------------

    #[test]
    fn crop_rgba8_extracts_the_requested_region() {
        // 4x4-Bild, obere linke Hälfte rot, Rest schwarz.
        let mut rgba = vec![0u8; 4 * 4 * 4];
        for y in 0..2u32 {
            for x in 0..2u32 {
                let idx = (y as usize * 4 + x as usize) * 4;
                rgba[idx..idx + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
        let (w, h, cropped) = crop_rgba8(4, 4, &rgba, 0.0, 0.0, 0.5, 0.5).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(&cropped[0..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn crop_rgba8_rejects_mismatched_buffer_length() {
        let err = crop_rgba8(4, 4, &[0u8; 3], 0.0, 0.0, 1.0, 1.0).unwrap_err();
        assert!(matches!(err, ExportError::Unsupported(_)));
    }

    // ---- render_title_card -------------------------------------------------

    #[test]
    fn title_card_without_text_is_a_plain_background() {
        let pixels =
            render_title_card(4, 4, [10, 20, 30], "", None, 12.0, [255, 255, 255]).unwrap();
        assert_eq!(&pixels[0..4], &[10, 20, 30, 255]);
    }

    #[test]
    fn title_card_with_text_but_no_font_is_a_clean_error() {
        let err =
            render_title_card(4, 4, [0, 0, 0], "Hallo", None, 12.0, [255, 255, 255]).unwrap_err();
        assert!(matches!(err, ExportError::Unsupported(_)));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn title_card_with_text_renders_visible_glyph_coverage() {
        let font_bytes =
            std::fs::read("/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf")
                .expect("Liberation Sans sollte auf dieser Maschine vorhanden sein");
        let pixels = render_title_card(
            64,
            32,
            [0, 0, 0],
            "Hi",
            Some(&font_bytes),
            20.0,
            [255, 255, 255],
        )
        .unwrap();
        assert!(pixels
            .chunks_exact(4)
            .any(|p| p[0] > 0 || p[1] > 0 || p[2] > 0));
    }

    // ---- build_frame_plan ---------------------------------------------------

    #[test]
    fn frame_plan_is_empty_for_no_slides_or_zero_fps() {
        assert!(build_frame_plan(&[], 25, TransitionKind::Cut, 0.0).is_empty());
        let slides = [solid_photo(2, 2, [255, 0, 0], 1.0)];
        assert!(build_frame_plan(&slides, 0, TransitionKind::Cut, 0.0).is_empty());
    }

    #[test]
    fn cut_transition_produces_only_hold_frames() {
        let slides = [
            solid_photo(2, 2, [255, 0, 0], 1.0),
            solid_photo(2, 2, [0, 255, 0], 1.0),
        ];
        let plan = build_frame_plan(&slides, 10, TransitionKind::Cut, 0.5);
        assert_eq!(plan.len(), 20); // 10 Frames je Folie, keine Überblendung
        assert!(plan
            .iter()
            .all(|e| matches!(e, FramePlanEntry::Hold { .. })));
    }

    #[test]
    fn crossfade_transition_inserts_transition_frames_between_slides() {
        let slides = [
            solid_photo(2, 2, [255, 0, 0], 1.0),
            solid_photo(2, 2, [0, 255, 0], 1.0),
        ];
        let plan = build_frame_plan(&slides, 10, TransitionKind::CrossFade, 0.5);
        // 10 Halte-Frames Folie 0 + 5 Überblendungsframes + 10 Halte-Frames Folie 1.
        assert_eq!(plan.len(), 25);
        let transition_count = plan
            .iter()
            .filter(|e| matches!(e, FramePlanEntry::Transition { .. }))
            .count();
        assert_eq!(transition_count, 5);
    }

    #[test]
    fn last_slide_has_no_trailing_transition() {
        let slides = [solid_photo(2, 2, [255, 0, 0], 1.0)];
        let plan = build_frame_plan(&slides, 10, TransitionKind::CrossFade, 0.5);
        assert_eq!(plan.len(), 10);
        assert!(plan
            .iter()
            .all(|e| matches!(e, FramePlanEntry::Hold { .. })));
    }

    #[test]
    fn very_short_hold_still_produces_at_least_one_frame() {
        let slides = [solid_photo(2, 2, [255, 0, 0], 0.001)];
        let plan = build_frame_plan(&slides, 25, TransitionKind::Cut, 0.0);
        assert_eq!(plan.len(), 1);
    }

    // ---- render_frame ---------------------------------------------------

    #[test]
    fn render_frame_hold_returns_the_slides_own_pixels_when_static() {
        let slides = [solid_photo(2, 2, [10, 20, 30], 1.0)];
        let frame = render_frame(
            &slides,
            FramePlanEntry::Hold {
                slide_index: 0,
                progress: 0.5,
            },
            2,
            2,
        )
        .unwrap();
        assert_eq!(&frame[0..4], &[10, 20, 30, 255]);
    }

    #[test]
    fn render_frame_transition_blends_the_two_slides_evenly_at_half_alpha() {
        let slides = [
            solid_photo(2, 2, [200, 0, 0], 1.0),
            solid_photo(2, 2, [0, 200, 0], 1.0),
        ];
        let frame = render_frame(
            &slides,
            FramePlanEntry::Transition {
                from_index: 0,
                to_index: 1,
                alpha: 0.5,
            },
            2,
            2,
        )
        .unwrap();
        assert_eq!(&frame[0..4], &[100, 100, 0, 255]);
    }

    #[test]
    fn render_frame_transition_at_alpha_zero_and_one_matches_the_endpoints() {
        let slides = [
            solid_photo(2, 2, [200, 0, 0], 1.0),
            solid_photo(2, 2, [0, 200, 0], 1.0),
        ];
        let start = render_frame(
            &slides,
            FramePlanEntry::Transition {
                from_index: 0,
                to_index: 1,
                alpha: 0.0,
            },
            2,
            2,
        )
        .unwrap();
        let end = render_frame(
            &slides,
            FramePlanEntry::Transition {
                from_index: 0,
                to_index: 1,
                alpha: 1.0,
            },
            2,
            2,
        )
        .unwrap();
        assert_eq!(&start[0..4], &[200, 0, 0, 255]);
        assert_eq!(&end[0..4], &[0, 200, 0, 255]);
    }

    #[test]
    fn render_frame_rejects_an_out_of_range_slide_index() {
        let slides = [solid_photo(2, 2, [1, 2, 3], 1.0)];
        let err = render_frame(
            &slides,
            FramePlanEntry::Hold {
                slide_index: 5,
                progress: 0.0,
            },
            2,
            2,
        )
        .unwrap_err();
        assert!(matches!(err, ExportError::Unsupported(_)));
    }

    // ---- ffmpeg_available / export_slideshow_video --------------------------

    #[test]
    fn ffmpeg_available_does_not_panic_either_way() {
        // Kein Anspruch auf einen bestimmten Wert (hängt von der
        // Testmaschine ab) — nur, dass die Prüfung selbst sauber
        // durchläuft, siehe Moduldoku/`PLAN.md` Schritt 4.
        let _ = ffmpeg_available();
    }

    #[test]
    fn export_slideshow_video_reports_a_clean_error_when_ffmpeg_is_missing() {
        if ffmpeg_available() {
            // Echtes ffmpeg auf dieser Maschine vorhanden — der
            // erfolgreiche Pfad ist nicht Gegenstand dieses Tests (keine
            // Kodier-Laufzeit in der Unit-Test-Suite), siehe Moduldoku.
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let slides = [solid_photo(4, 4, [255, 0, 0], 0.5)];
        let err = export_slideshow_video(
            &slides,
            TransitionKind::Cut,
            0.0,
            &VideoExportOptions {
                output_width: 4,
                output_height: 4,
                fps: 10,
                audio_path: None,
            },
            &dir.path().join("out.mp4"),
        )
        .unwrap_err();
        assert!(matches!(err, ExportError::Video { .. }));
    }

    #[test]
    fn export_slideshow_video_rejects_empty_slides() {
        if !ffmpeg_available() {
            return; // die ffmpeg-fehlt-Fehlermeldung würde sonst zuerst greifen
        }
        let dir = tempfile::tempdir().unwrap();
        let err = export_slideshow_video(
            &[],
            TransitionKind::Cut,
            0.0,
            &VideoExportOptions {
                output_width: 4,
                output_height: 4,
                fps: 10,
                audio_path: None,
            },
            &dir.path().join("out.mp4"),
        )
        .unwrap_err();
        assert!(matches!(err, ExportError::Unsupported(_)));
    }
}
