//! Mehrspur-Zeitachse (Phase 17 Schritt 1, siehe `DECISIONS.md`
//! ADR-0045): kombiniert mehrere Video-Clips/Fotos/Titelkarten zu einer
//! Sequenz mit Übergängen, gerendert in einem Rutsch (kein editierbares
//! Projekt-Objekt — siehe ADR-0045s Architektur-Begründung: derselbe
//! "Dialog rendert einmalig"-Ansatz wie die Diashow, `video.rs`).
//!
//! Zweistufig: (1) jeder Zeitachsen-Eintrag wird zu einem eigenen
//! Video-Segment gerendert — ein Video-Clip per Trim+Skalierung
//! (dieselbe `ffmpeg`-Subprozess-Technik wie `apx_app::commands::
//! trim_video`), ein Foto/eine Titelkarte über die bereits vorhandene
//! Ein-Folie-Diashow-Renderung (`export_slideshow_video`) — ein
//! Video-Clip hat selbst hunderte/tausende Frames und passt nicht in
//! `TimelineSlide`s "einmal rendern, im Speicher halten"-Modell wie
//! ein Foto. (2) alle Segmente werden per `ffmpeg`-`xfade`-Filterkette
//! zu einer Sequenz verkettet — auch reine Schnitte laufen über
//! `xfade` mit einer für das Auge nicht wahrnehmbaren Dauer
//! (`CUT_TRANSITION_SECONDS`), das hält den Verkettungscode
//! einheitlich (eine Filterkette statt zweier verschiedener Wege für
//! Schnitt vs. Überblendung).
//!
//! **Bewusste Vereinfachung (siehe ADR-0045):** jeder Video-Clip
//! verliert beim Segment-Rendern seine eigene Tonspur (`-an`) — Audio
//! kommt ausschließlich über eine optionale Hintergrundmusik-Datei
//! (`TimelineExportOptions.audio_path`), genau wie bei der Diashow.
//! Die eigene Tonspur eines Clips (z. B. gesprochener Kommentar)
//! bleibt für einen späteren Schritt offen.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{ExportError, Result};
use crate::video::{
    default_ken_burns, export_slideshow_video, ffmpeg_available, TimelineSlide, TransitionKind,
    VideoExportOptions,
};
use crate::watermark::{self, WatermarkPosition};

/// Ein Text-/Titel-Overlay über einer Zeitspanne der fertigen Sequenz
/// (Phase 17 Schritt 4, siehe `DECISIONS.md` ADR-0045) — Zeiten
/// beziehen sich auf die **verkettete** Sequenz, nicht auf einen
/// einzelnen Eintrag (ein Overlay kann z. B. über einen Übergang
/// hinweg sichtbar bleiben). Rendering läuft bewusst NICHT über
/// `ffmpeg`s `drawtext`-Filter (bräuchte Systemschriften/`fontconfig`
/// oder eine gebündelte Schriftart) — stattdessen wird der Text wie
/// bei Titelkarten/Wasserzeichen rein in Rust über
/// [`watermark::apply_text_watermark`] auf einen transparenten
/// Vollbild-Kanal gerastert, als PNG zwischengespeichert und per
/// `ffmpeg`s `overlay`-Filter mit `enable='between(t,start,end)'`
/// zeitlich eingeblendet (siehe [`apply_text_overlays`]) — dieselbe
/// Text-Rasterisierung wie überall sonst im Projekt, kein zweiter
/// Textpfad nur für Video.
#[derive(Debug, Clone)]
pub struct TimelineTextOverlay {
    pub text: String,
    pub position: WatermarkPosition,
    pub start_seconds: f32,
    pub end_seconds: f32,
    pub font_bytes: Vec<u8>,
    pub font_size_px: f32,
    pub text_color: [u8; 3],
}

/// Ein Bild-in-Bild-/Split-Screen-Overlay über einer Zeitspanne der
/// fertigen Sequenz (Phase 17 Schritt 7, siehe `DECISIONS.md`
/// ADR-0045) — `source` ist ein ganz normaler [`TimelineItem`] (Video-
/// Clip, Foto oder Titelkarte), genau wie ein Haupt-Zeitachsen-
/// Eintrag, nur zusätzlich verkleinert und über die bereits verkettete
/// Sequenz gelegt statt selbst Teil der Kette zu sein. **Split-Screen
/// ist bewusst kein eigener Mechanismus** — dieselbe Struktur mit
/// `scale` nahe `1.0` und zwei Overlays an gegenüberliegenden
/// Positionen (z. B. `TopLeft`+`TopRight`, je `scale: 0.5`) ergibt ein
/// Nebeneinander zweier Quellen, ohne eine zweite Kompositions-
/// Infrastruktur zu brauchen.
#[derive(Debug, Clone)]
pub struct TimelinePipOverlay {
    pub source: TimelineItem,
    pub start_seconds: f32,
    pub end_seconds: f32,
    pub position: WatermarkPosition,
    /// Anteil an der Ziel-Auflösung (`0.05..=1.0`) — die Einblendung
    /// behält dasselbe Seitenverhältnis wie die Ziel-Auflösung selbst
    /// (siehe [`apply_pip_overlays`]s Moduldoku).
    pub scale: f32,
}

/// Übergangsart zwischen zwei Zeitachsen-Einträgen (Phase 17 Schritt 3,
/// siehe `DECISIONS.md` ADR-0045) — bewusst ein eigener, reicherer Typ
/// statt Wiederverwendung von `video::TransitionKind` (Cut/CrossFade):
/// letzterer trägt zusätzlich die Live-Canvas-Wiedergabe der Diashow
/// (`SlideshowPlayer.tsx`), die keine `ffmpeg`-`xfade`-Filternamen
/// kennt und mit nur zwei Varianten auskommt. Jede Variante außer
/// `Cut` ist ein Name, den `ffmpeg`s `xfade`-Filter direkt versteht
/// (siehe `ffmpeg_name()`) — kein eigener Bildmisch-Code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineTransitionKind {
    Cut,
    Fade,
    Dissolve,
    WipeLeft,
    WipeRight,
    SlideUp,
    SlideDown,
    CircleOpen,
}

impl TimelineTransitionKind {
    /// Der `xfade`-Filter-Name — bei `Cut` irrelevant (die Dauer ist
    /// ohnehin `CUT_TRANSITION_SECONDS`, ein für das Auge nicht
    /// wahrnehmbarer Bruchteil einer Sekunde), aber ein gültiger Name
    /// wird trotzdem gebraucht, damit die Filterkette einheitlich
    /// bleibt (siehe Moduldoku).
    fn ffmpeg_name(self) -> &'static str {
        match self {
            Self::Cut | Self::Fade => "fade",
            Self::Dissolve => "dissolve",
            Self::WipeLeft => "wipeleft",
            Self::WipeRight => "wiperight",
            Self::SlideUp => "slideup",
            Self::SlideDown => "slidedown",
            Self::CircleOpen => "circleopen",
        }
    }
}

/// Ein einzelner Zeitachsen-Eintrag — anders als `TimelineSlide`
/// referenziert `VideoClip` eine Quelldatei plus Trim-Bereich statt
/// eines vorab gerenderten Puffers (siehe Moduldoku).
#[derive(Debug, Clone)]
pub enum TimelineItem {
    VideoClip {
        source_path: PathBuf,
        in_ms: i64,
        out_ms: i64,
        /// Tempo-Faktor (Phase 17 Schritt 2) — `1.0` = unverändert,
        /// `> 1.0` = Zeitraffer (schneller/kürzer), `< 1.0` = Zeitlupe
        /// (langsamer/länger). Wirkt nur auf das Bild (`setpts`, siehe
        /// `render_video_clip_segment`) — Ton kommt ohnehin nur über die
        /// optionale Hintergrundmusik (siehe Moduldoku), kein `atempo`
        /// nötig.
        speed: f32,
    },
    Photo {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
        hold_seconds: f32,
    },
    Title {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
        hold_seconds: f32,
    },
}

impl TimelineItem {
    fn duration_seconds(&self) -> f32 {
        match self {
            Self::VideoClip {
                in_ms,
                out_ms,
                speed,
                ..
            } => {
                let trimmed = (*out_ms - *in_ms).max(0) as f32 / 1000.0;
                // Schneller abgespielt = kürzeres Segment, langsamer =
                // längeres — dieselbe Beziehung wie `setpts=PTS/speed`.
                trimmed / speed.max(0.01)
            }
            Self::Photo { hold_seconds, .. } | Self::Title { hold_seconds, .. } => *hold_seconds,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimelineExportOptions {
    pub output_width: u32,
    pub output_height: u32,
    pub fps: u32,
    /// Optionale Hintergrundmusik — siehe Moduldoku zur bewussten
    /// Vereinfachung ggü. Audio je Clip.
    pub audio_path: Option<PathBuf>,
    /// Text-/Titel-Overlays (Phase 17 Schritt 4) — angewendet auf die
    /// bereits verkettete Sequenz, vor dem Einmischen der Musik.
    pub text_overlays: Vec<TimelineTextOverlay>,
    /// Bild-in-Bild-/Split-Screen-Overlays (Phase 17 Schritt 7) —
    /// angewendet nach den Text-Overlays, ebenfalls vor dem Einmischen
    /// der Musik (siehe [`render_video_timeline`]s Reihenfolge).
    pub pip_overlays: Vec<TimelinePipOverlay>,
}

#[derive(Debug, Clone, Copy)]
pub struct TimelineExportOutcome {
    pub duration_seconds: f32,
}

/// Ein Schnitt läuft über dieselbe `xfade`-Filterkette wie eine
/// Überblendung, nur mit einer nicht wahrnehmbaren Dauer (etwa ein
/// Frame bei üblichen Bildraten) — vermeidet einen zweiten,
/// gesonderten Verkettungsweg nur für reine Schnitte.
const CUT_TRANSITION_SECONDS: f32 = 0.04;

/// Rendert `items` zu einer einzigen Video-Datei — siehe Moduldoku für
/// den zweistufigen Ansatz. `transitions.len()` muss genau
/// `items.len() - 1` sein (ein Übergang zwischen je zwei
/// aufeinanderfolgenden Einträgen).
pub fn render_video_timeline(
    items: &[TimelineItem],
    transitions: &[TimelineTransitionKind],
    transition_seconds: f32,
    options: &TimelineExportOptions,
    dest_path: &Path,
) -> Result<TimelineExportOutcome> {
    if !ffmpeg_available() {
        return Err(ExportError::Video {
            message: "ffmpeg wurde nicht gefunden (im PATH nicht aufrufbar) — bitte installieren \
                      (siehe DECISIONS.md ADR-0034)"
                .to_string(),
        });
    }
    if items.is_empty() {
        return Err(ExportError::Unsupported(
            "Zeitachse enthält keine Einträge".to_string(),
        ));
    }
    if transitions.len() != items.len() - 1 {
        return Err(ExportError::Unsupported(format!(
            "Erwartete {} Übergänge für {} Einträge, {} übergeben",
            items.len() - 1,
            items.len(),
            transitions.len()
        )));
    }
    if options.output_width == 0 || options.output_height == 0 || options.fps == 0 {
        return Err(ExportError::Unsupported(
            "Video-Auflösung/Bildrate muss größer null sein".to_string(),
        ));
    }

    let tmp_dir = tempfile::tempdir().map_err(|err| ExportError::Video {
        message: format!("Temp-Verzeichnis konnte nicht angelegt werden: {err}"),
    })?;

    let mut segment_paths = Vec::with_capacity(items.len());
    let mut durations = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let segment_path = tmp_dir.path().join(format!("segment_{index}.mp4"));
        render_segment(item, options, &segment_path)?;
        segment_paths.push(segment_path);
        durations.push(item.duration_seconds());
    }

    let gaps: Vec<f32> = transitions
        .iter()
        .map(|t| match t {
            TimelineTransitionKind::Cut => CUT_TRANSITION_SECONDS,
            _ => transition_seconds.max(CUT_TRANSITION_SECONDS),
        })
        .collect();

    let mut video_only_path = if segment_paths.len() == 1 {
        segment_paths[0].clone()
    } else {
        let concatenated = tmp_dir.path().join("concatenated.mp4");
        concat_with_xfade(
            &segment_paths,
            &durations,
            &gaps,
            transitions,
            options.fps,
            &concatenated,
        )?;
        concatenated
    };

    if !options.text_overlays.is_empty() {
        let overlaid = tmp_dir.path().join("overlaid.mp4");
        apply_text_overlays(
            &video_only_path,
            &options.text_overlays,
            options.output_width,
            options.output_height,
            options.fps,
            tmp_dir.path(),
            &overlaid,
        )?;
        video_only_path = overlaid;
    }

    if !options.pip_overlays.is_empty() {
        let composited = tmp_dir.path().join("pip.mp4");
        apply_pip_overlays(
            &video_only_path,
            &options.pip_overlays,
            options.output_width,
            options.output_height,
            options.fps,
            tmp_dir.path(),
            &composited,
        )?;
        video_only_path = composited;
    }

    if let Some(audio_path) = &options.audio_path {
        mux_audio(&video_only_path, audio_path, dest_path)?;
    } else {
        std::fs::copy(&video_only_path, dest_path).map_err(|err| ExportError::Video {
            message: format!("Ergebnisdatei konnte nicht geschrieben werden: {err}"),
        })?;
    }

    Ok(TimelineExportOutcome {
        duration_seconds: total_duration_after_xfade(&durations, &gaps),
    })
}

fn render_segment(
    item: &TimelineItem,
    options: &TimelineExportOptions,
    dest_path: &Path,
) -> Result<()> {
    match item {
        TimelineItem::VideoClip {
            source_path,
            in_ms,
            out_ms,
            speed,
        } => render_video_clip_segment(source_path, *in_ms, *out_ms, *speed, options, dest_path),
        TimelineItem::Photo {
            width,
            height,
            rgba,
            hold_seconds,
        } => render_single_slide_segment(
            TimelineSlide::Photo {
                width: *width,
                height: *height,
                rgba: rgba.clone(),
                ken_burns: default_ken_burns(0, false),
                hold_seconds: *hold_seconds,
            },
            options,
            dest_path,
        ),
        TimelineItem::Title {
            width,
            height,
            rgba,
            hold_seconds,
        } => render_single_slide_segment(
            TimelineSlide::Title {
                width: *width,
                height: *height,
                rgba: rgba.clone(),
                hold_seconds: *hold_seconds,
            },
            options,
            dest_path,
        ),
    }
}

/// Ein einzelnes Foto/eine Titelkarte läuft über die bereits
/// vorhandene Diashow-Ein-Folie-Renderung — kein zweiter
/// Bildsequenz-Kodierpfad nur für diesen Fall.
fn render_single_slide_segment(
    slide: TimelineSlide,
    options: &TimelineExportOptions,
    dest_path: &Path,
) -> Result<()> {
    export_slideshow_video(
        &[slide],
        TransitionKind::Cut,
        0.0,
        &VideoExportOptions {
            output_width: options.output_width,
            output_height: options.output_height,
            fps: options.fps,
            audio_path: None,
        },
        dest_path,
    )
    .map(|_| ())
}

/// Trimmt `source_path` auf `[in_ms, out_ms)` und skaliert/beschneidet
/// auf die Zielauflösung (`scale`+`crop`, "cover"-Verhalten wie
/// `TimelineSlide::Photo`s Ken-Burns-Anpassung) — bewusst immer
/// neu kodiert (kein Stream-Copy-Versuch wie `trim_video`), weil jedes
/// Segment exakt dieselbe Auflösung/Bildrate für die anschließende
/// `xfade`-Verkettung haben muss.
fn render_video_clip_segment(
    source_path: &Path,
    in_ms: i64,
    out_ms: i64,
    speed: f32,
    options: &TimelineExportOptions,
    dest_path: &Path,
) -> Result<()> {
    let start_secs = in_ms as f64 / 1000.0;
    let duration_secs = (out_ms - in_ms).max(0) as f64 / 1000.0;
    let (w, h) = (options.output_width, options.output_height);
    let speed = speed.max(0.01);

    // `setpts=PTS/speed`: höheres Tempo staucht die Zeitstempel
    // (kürzeres Segment), niedrigeres Tempo streckt sie (Zeitlupe) —
    // bei `speed == 1.0` bewusst weggelassen (No-op, spart einen
    // Filter-Schritt).
    let vf = if (speed - 1.0).abs() < f32::EPSILON {
        format!("scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h}")
    } else {
        format!(
            "scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},setpts=PTS/{speed}"
        )
    };

    let output = Command::new("ffmpeg")
        .args(["-y", "-ss", &format!("{start_secs}"), "-i"])
        .arg(source_path)
        .args(["-t", &format!("{duration_secs}")])
        .args(["-vf", &vf])
        .args(["-an", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-r"])
        .arg(options.fps.to_string())
        .arg(dest_path)
        .output()
        .map_err(|err| ExportError::Video {
            message: format!("ffmpeg nicht startbar (ist ffmpeg installiert?): {err}"),
        })?;
    check_ffmpeg_output(output, "Video-Segment rendern")
}

/// Verkettet `segment_paths` per `xfade`-Filterkette (siehe Moduldoku)
/// zu `dest_path`.
fn concat_with_xfade(
    segment_paths: &[PathBuf],
    durations: &[f32],
    gaps: &[f32],
    transitions: &[TimelineTransitionKind],
    fps: u32,
    dest_path: &Path,
) -> Result<()> {
    let offsets = xfade_offsets(durations, gaps);

    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-y");
    for path in segment_paths {
        cmd.arg("-i").arg(path);
    }

    let mut filter = String::new();
    let mut prev_label = "0:v".to_string();
    for (i, gap) in gaps.iter().enumerate() {
        let next_input = format!("{}:v", i + 1);
        let out_label = if i == gaps.len() - 1 {
            "vout".to_string()
        } else {
            format!("v{i}")
        };
        // `transitions` fällt bei fehlendem Eintrag (sollte durch die
        // Längenprüfung in `render_video_timeline` nicht vorkommen) auf
        // `Fade` zurück, statt zu paniken.
        let transition_name = transitions
            .get(i)
            .copied()
            .unwrap_or(TimelineTransitionKind::Fade)
            .ffmpeg_name();
        filter.push_str(&format!(
            "[{prev_label}][{next_input}]xfade=transition={transition_name}:duration={:.3}:offset={:.3}[{out_label}];",
            gap, offsets[i]
        ));
        prev_label = out_label;
    }
    filter.pop(); // trailing ';'

    let output = cmd
        .arg("-filter_complex")
        .arg(&filter)
        .args(["-map", "[vout]", "-r"])
        .arg(fps.to_string())
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
        .arg(dest_path)
        .output()
        .map_err(|err| ExportError::Video {
            message: format!("ffmpeg nicht startbar (ist ffmpeg installiert?): {err}"),
        })?;
    check_ffmpeg_output(output, "Zeitachse verketten")
}

/// Brennt `overlays` in `video_path` ein (siehe [`TimelineTextOverlay`]s
/// Dokumentation für den Ansatz) — jedes Overlay wird zu einem
/// transparenten Vollbild-PNG gerastert (Text bereits an der richtigen
/// Stelle, siehe [`watermark::apply_text_watermark`]s Positionierung),
/// dann per `ffmpeg`-`overlay`-Filterkette mit `enable`-Zeitfenster
/// nacheinander über das Video gelegt.
fn apply_text_overlays(
    video_path: &Path,
    overlays: &[TimelineTextOverlay],
    width: u32,
    height: u32,
    fps: u32,
    tmp_dir: &Path,
    dest_path: &Path,
) -> Result<()> {
    let mut overlay_paths = Vec::with_capacity(overlays.len());
    for (index, overlay) in overlays.iter().enumerate() {
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        watermark::apply_text_watermark(
            width,
            height,
            &mut pixels,
            &overlay.font_bytes,
            &overlay.text,
            overlay.font_size_px,
            overlay.text_color,
            overlay.position,
            1.0,
            24,
        )?;
        let png_path = tmp_dir.join(format!("overlay_{index}.png"));
        let buf = image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(width, height, pixels)
            .ok_or_else(|| ExportError::Video {
            message: "Overlay-Puffer hat die falsche Größe".to_string(),
        })?;
        buf.save(&png_path).map_err(|err| ExportError::Video {
            message: format!("Overlay-Bild konnte nicht geschrieben werden: {err}"),
        })?;
        overlay_paths.push(png_path);
    }

    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-y").arg("-i").arg(video_path);
    for path in &overlay_paths {
        cmd.arg("-i").arg(path);
    }

    let mut filter = String::new();
    let mut prev_label = "0:v".to_string();
    for (index, overlay) in overlays.iter().enumerate() {
        let out_label = if index == overlays.len() - 1 {
            "vout".to_string()
        } else {
            format!("t{index}")
        };
        filter.push_str(&format!(
            "[{prev_label}][{}:v]overlay=0:0:enable='between(t,{:.3},{:.3})'[{out_label}];",
            index + 1,
            overlay.start_seconds,
            overlay.end_seconds
        ));
        prev_label = out_label;
    }
    filter.pop(); // trailing ';'

    let output = cmd
        .arg("-filter_complex")
        .arg(&filter)
        .args(["-map", "[vout]", "-r"])
        .arg(fps.to_string())
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
        .arg(dest_path)
        .output()
        .map_err(|err| ExportError::Video {
            message: format!("ffmpeg nicht startbar (ist ffmpeg installiert?): {err}"),
        })?;
    check_ffmpeg_output(output, "Text-Overlays einblenden")
}

/// Blendet `overlays` als Bild-in-Bild-Einblendungen in `video_path`
/// ein (siehe [`TimelinePipOverlay`]s Dokumentation für den Ansatz).
/// Jede Overlay-Quelle wird zunächst wie ein ganz normaler Zeitachsen-
/// Eintrag zu einem eigenen Segment gerendert (`render_segment`), aber
/// auf `width * scale`×`height * scale` verkleinert (behält damit
/// dasselbe Seitenverhältnis wie die Ziel-Auflösung, statt eine feste
/// Box-Form zu erzwingen — ein 9:16-Bild-in-Bild auf einer 9:16-
/// Zeitachse bleibt also proportional). Anschließend legt eine
/// `ffmpeg`-`overlay`-Filterkette (mit `enable`-Zeitfenster, wie bei
/// [`apply_text_overlays`]) jedes verkleinerte Segment an die per
/// [`watermark::origin_for`] berechnete Position — dieselbe
/// Positionierungsformel wie beim Text-/Bild-Wasserzeichen.
fn apply_pip_overlays(
    video_path: &Path,
    overlays: &[TimelinePipOverlay],
    width: u32,
    height: u32,
    fps: u32,
    tmp_dir: &Path,
    dest_path: &Path,
) -> Result<()> {
    const MARGIN: u32 = 16;

    let mut segments = Vec::with_capacity(overlays.len());
    for (index, overlay) in overlays.iter().enumerate() {
        let pip_w = ((width as f32 * overlay.scale).round() as u32).max(2);
        let pip_h = ((height as f32 * overlay.scale).round() as u32).max(2);
        let pip_options = TimelineExportOptions {
            output_width: pip_w,
            output_height: pip_h,
            fps,
            audio_path: None,
            text_overlays: Vec::new(),
            pip_overlays: Vec::new(),
        };
        let segment_path = tmp_dir.join(format!("pip_{index}.mp4"));
        render_segment(&overlay.source, &pip_options, &segment_path)?;
        segments.push((segment_path, pip_w, pip_h));
    }

    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-y").arg("-i").arg(video_path);
    for (path, _, _) in &segments {
        cmd.arg("-i").arg(path);
    }

    let mut filter = String::new();
    let mut prev_label = "0:v".to_string();
    for (index, (_, pip_w, pip_h)) in segments.iter().enumerate() {
        let overlay = &overlays[index];
        let (x, y) = watermark::origin_for(overlay.position, width, height, *pip_w, *pip_h, MARGIN);
        let out_label = if index == segments.len() - 1 {
            "vout".to_string()
        } else {
            format!("p{index}")
        };
        filter.push_str(&format!(
            "[{prev_label}][{}:v]overlay=x={x}:y={y}:enable='between(t,{:.3},{:.3})'[{out_label}];",
            index + 1,
            overlay.start_seconds,
            overlay.end_seconds
        ));
        prev_label = out_label;
    }
    filter.pop(); // trailing ';'

    let output = cmd
        .arg("-filter_complex")
        .arg(&filter)
        .args(["-map", "[vout]", "-r"])
        .arg(fps.to_string())
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
        .arg(dest_path)
        .output()
        .map_err(|err| ExportError::Video {
            message: format!("ffmpeg nicht startbar (ist ffmpeg installiert?): {err}"),
        })?;
    check_ffmpeg_output(output, "Bild-in-Bild einblenden")
}

fn mux_audio(video_path: &Path, audio_path: &Path, dest_path: &Path) -> Result<()> {
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(video_path)
        .arg("-i")
        .arg(audio_path)
        .args(["-c:v", "copy", "-c:a", "aac", "-shortest"])
        .arg(dest_path)
        .output()
        .map_err(|err| ExportError::Video {
            message: format!("ffmpeg nicht startbar (ist ffmpeg installiert?): {err}"),
        })?;
    check_ffmpeg_output(output, "Hintergrundmusik einmischen")
}

fn check_ffmpeg_output(output: std::process::Output, step: &str) -> Result<()> {
    if !output.status.success() {
        return Err(ExportError::Video {
            message: format!(
                "ffmpeg-Schritt '{step}' fehlgeschlagen: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }
    Ok(())
}

/// Berechnet für jede `xfade`-Verkettungsstufe den `offset`-Parameter
/// (Position im bisher verketteten Strom, an der die Überblendung
/// beginnt) — reine Zeitrechnung, kein `ffmpeg`-Aufruf, deshalb separat
/// unit-testbar. `durations[i]` ist die Länge von Segment `i`,
/// `gaps[i]` die Überblendungsdauer zwischen Segment `i` und `i+1`
/// (`gaps.len() == durations.len() - 1`, sonst wird das Ende der
/// kürzeren Liste als Grenze genommen).
pub fn xfade_offsets(durations: &[f32], gaps: &[f32]) -> Vec<f32> {
    let mut offsets = Vec::with_capacity(gaps.len());
    let mut cumulative = durations.first().copied().unwrap_or(0.0);
    for (i, gap) in gaps.iter().enumerate() {
        let offset = (cumulative - gap).max(0.0);
        offsets.push(offset);
        let next_duration = durations.get(i + 1).copied().unwrap_or(0.0);
        cumulative = cumulative + next_duration - gap;
    }
    offsets
}

/// Gesamtlänge der verketteten Sequenz — Summe der Segmentlängen minus
/// der durch Überblendungen überlappten Zeit (jede Überblendung "spart"
/// ihre eigene Dauer, weil sich zwei Segmente in dieser Zeit
/// überlagern statt hintereinander zu laufen).
pub fn total_duration_after_xfade(durations: &[f32], gaps: &[f32]) -> f32 {
    let sum_durations: f32 = durations.iter().sum();
    let sum_gaps: f32 = gaps.iter().sum();
    (sum_durations - sum_gaps).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_for_two_equal_segments_with_no_overlap_match_the_first_duration() {
        let offsets = xfade_offsets(&[5.0, 5.0], &[0.0]);
        assert_eq!(offsets, vec![5.0]);
    }

    #[test]
    fn offsets_account_for_overlap_duration() {
        // Zwei 5s-Segmente, 1s Überblendung: die Überblendung beginnt bei
        // 5s - 1s = 4s im ersten Segment.
        let offsets = xfade_offsets(&[5.0, 5.0], &[1.0]);
        assert_eq!(offsets, vec![4.0]);
    }

    #[test]
    fn offsets_chain_correctly_across_three_segments() {
        // 5s, 5s, 5s mit je 1s Überblendung: erste Überblendung bei 4s
        // (5-1), zweite bei (5 + 5 - 1) - 1 = 8s.
        let offsets = xfade_offsets(&[5.0, 5.0, 5.0], &[1.0, 1.0]);
        assert_eq!(offsets, vec![4.0, 8.0]);
    }

    #[test]
    fn total_duration_subtracts_overlap_time() {
        let total = total_duration_after_xfade(&[5.0, 5.0, 5.0], &[1.0, 1.0]);
        assert_eq!(total, 13.0); // 15 - 2*1
    }

    #[test]
    fn total_duration_of_a_single_segment_is_its_own_duration() {
        let total = total_duration_after_xfade(&[7.5], &[]);
        assert_eq!(total, 7.5);
    }

    #[test]
    fn total_duration_never_goes_negative_for_pathological_overlap() {
        let total = total_duration_after_xfade(&[1.0, 1.0], &[5.0]);
        assert_eq!(total, 0.0);
    }

    #[test]
    fn double_speed_halves_the_segment_duration() {
        let item = TimelineItem::VideoClip {
            source_path: PathBuf::from("clip.mp4"),
            in_ms: 0,
            out_ms: 4000,
            speed: 2.0,
        };
        assert_eq!(item.duration_seconds(), 2.0);
    }

    #[test]
    fn half_speed_doubles_the_segment_duration() {
        let item = TimelineItem::VideoClip {
            source_path: PathBuf::from("clip.mp4"),
            in_ms: 0,
            out_ms: 4000,
            speed: 0.5,
        };
        assert_eq!(item.duration_seconds(), 8.0);
    }

    #[test]
    fn normal_speed_keeps_the_trimmed_duration() {
        let item = TimelineItem::VideoClip {
            source_path: PathBuf::from("clip.mp4"),
            in_ms: 1000,
            out_ms: 4000,
            speed: 1.0,
        };
        assert_eq!(item.duration_seconds(), 3.0);
    }
}
