//! Video als Katalog-Asset (Phase 16 Schritt 4, siehe `DECISIONS.md`
//! ADR-0043) — Erkennung per Dateiendung plus Metadaten-Extraktion per
//! `ffprobe`. Dasselbe Subprozess-Muster wie `apx_export::video`s
//! `ffmpeg`-Aufrufe (ADR-0034): kein Bündeln, System-Installation
//! vorausgesetzt, `ffprobe` ist Teil derselben ffmpeg-Installation.
//!
//! **Bewusst NICHT über `apx_raw::read_metadata`**: das ist ein reiner
//! Bild-Decoder (RAW/JPEG/PNG/TIFF) — ein Video-Container hat ein
//! komplett anderes Metadaten-Modell (Dauer, Codec, Bildrate, Audiospur)
//! und würde dort schlicht mit einem Dekodierfehler scheitern.

use std::path::Path;
use std::process::Command;

const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "m4v", "avi", "mkv", "webm"];

/// Ob eine Datei anhand ihrer Endung als Video statt als Foto behandelt
/// wird — dieselbe Endungs-basierte Erkennung wie `apx_raw::format`s
/// `is_supported_extension` für Fotos.
pub(crate) fn is_video_extension(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    let ext = ext.to_lowercase();
    VIDEO_EXTENSIONS.contains(&ext.as_str())
}

#[derive(Debug, Clone, Default)]
pub(crate) struct VideoMeta {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: Option<i64>,
    pub codec: Option<String>,
    pub has_audio: Option<bool>,
    pub frame_rate: Option<f32>,
}

/// Liest Video-Metadaten per `ffprobe -show_format -show_streams`
/// (JSON-Ausgabe, geparst über das ohnehin vorhandene `serde_json` — kein
/// neues Crate). Ein klarer Fehler statt eines Absturzes, wenn `ffprobe`
/// nicht installiert ist (derselbe Fehlerpfad wie
/// `apx_export::video::ffmpeg_available`).
pub(crate) fn extract_video_metadata(path: &Path) -> Result<VideoMeta, String> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .output()
        .map_err(|err| {
            format!("ffprobe nicht startbar (ist ffmpeg installiert und im PATH?): {err}")
        })?;

    if !output.status.success() {
        return Err(format!(
            "ffprobe meldete einen Fehler für '{}': {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("ffprobe-Ausgabe nicht als JSON lesbar: {err}"))?;

    let duration_ms = json["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .map(|secs| (secs * 1000.0).round() as i64);

    let streams = json["streams"].as_array().cloned().unwrap_or_default();
    let video_stream = streams
        .iter()
        .find(|s| s["codec_type"].as_str() == Some("video"));
    let has_audio = Some(
        streams
            .iter()
            .any(|s| s["codec_type"].as_str() == Some("audio")),
    );

    let width = video_stream
        .and_then(|s| s["width"].as_u64())
        .map(|v| v as u32);
    let height = video_stream
        .and_then(|s| s["height"].as_u64())
        .map(|v| v as u32);
    let codec = video_stream
        .and_then(|s| s["codec_name"].as_str())
        .map(|s| s.to_string());
    let frame_rate = video_stream
        .and_then(|s| s["avg_frame_rate"].as_str())
        .and_then(parse_frame_rate_fraction);

    Ok(VideoMeta {
        width,
        height,
        duration_ms,
        codec,
        has_audio,
        frame_rate,
    })
}

/// `ffprobe` liefert `avg_frame_rate` als Bruch-String (z. B.
/// `"30000/1001"` für 29.97 fps, `"25/1"` für 25 fps) statt einer
/// Dezimalzahl.
fn parse_frame_rate_fraction(raw: &str) -> Option<f32> {
    let (num, den) = raw.split_once('/')?;
    let num: f64 = num.parse().ok()?;
    let den: f64 = den.parse().ok()?;
    if den == 0.0 {
        return None;
    }
    Some((num / den) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_common_video_extensions() {
        for ext in VIDEO_EXTENSIONS {
            assert!(is_video_extension(Path::new(&format!("clip.{ext}"))));
            // Groß-/Kleinschreibung darf keine Rolle spielen.
            assert!(is_video_extension(Path::new(&format!(
                "CLIP.{}",
                ext.to_uppercase()
            ))));
        }
        assert!(!is_video_extension(Path::new("foto.jpg")));
        assert!(!is_video_extension(Path::new("ohne_endung")));
    }

    #[test]
    fn parses_frame_rate_fraction() {
        assert!((parse_frame_rate_fraction("30000/1001").unwrap() - 29.970_03).abs() < 0.001);
        assert_eq!(parse_frame_rate_fraction("25/1"), Some(25.0));
        assert_eq!(parse_frame_rate_fraction("0/0"), None);
        assert_eq!(parse_frame_rate_fraction("garbage"), None);
    }
}
