//! Web (Phase 8 Schritt 6, `PLAN.md`: „HTML-/responsiver Galerie-
//! Generator ..., Themes, Upload via FTP/SFTP").
//!
//! Der Galerie-Generator ist reines HTML/CSS-Templating (kein
//! JavaScript, keine Build-Pipeline) — [`generate_gallery_html`] ist eine
//! reine String-Funktion, [`export_gallery`] erledigt die Miniaturbild-
//! Erzeugung über denselben `resize`/`format`-Renderpfad wie der normale
//! Export und schreibt alles auf die Platte.
//!
//! Zwei Upload-Wege: [`upload_via_ftp`] (`suppaftp`, synchron, reines
//! FTP ohne TLS) und [`upload_via_sftp`] (`russh`/`russh-sftp`, asynchron
//! — SSH braucht einen laufenden Tokio-Runtime-Kontext, `apx-app`s
//! Tauri-Commands haben den ohnehin). **Bewusste Vereinfachung:** kein
//! Host-Key-Pinning (`check_server_key` akzeptiert jeden Schlüssel, wie
//! ein `ssh -o StrictHostKeyChecking=no`) — echtes Pinning bräuchte eine
//! dauerhafte Speicherung bekannter Schlüssel samt Nutzer-Bestätigung bei
//! der ersten Verbindung, das ist ein eigenständiges UI-Thema für eine
//! spätere Phase. Beide Uploads laufen nur mit Nutzername/Passwort (kein
//! Schlüsseldatei-Login).
//!
//! **Bewusste Vereinfachung (Galerie):** drei feste Themes (Hell/Dunkel/
//! Minimal) statt frei editierbarer Design-Tokens, eine einzige
//! `index.html`-Seite ohne Paginierung.

use std::path::{Path, PathBuf};

use crate::error::{ExportError, Result};
use crate::format::{encode_rgba8, EncodeOptions, ExportFormat};
use crate::resize::{self, SizeConstraint};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GalleryTheme {
    Light,
    Dark,
    Minimal,
}

impl GalleryTheme {
    fn css(self) -> &'static str {
        match self {
            GalleryTheme::Light => LIGHT_CSS,
            GalleryTheme::Dark => DARK_CSS,
            GalleryTheme::Minimal => MINIMAL_CSS,
        }
    }
}

const LIGHT_CSS: &str =
    "body{background:#fff;color:#111;font-family:sans-serif;margin:0;padding:2rem}\
h1{font-weight:300}\
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:1rem}\
.grid img{width:100%;height:auto;display:block;border-radius:4px}\
figcaption{font-size:.8rem;color:#555;margin-top:.25rem}";

const DARK_CSS: &str =
    "body{background:#111;color:#eee;font-family:sans-serif;margin:0;padding:2rem}\
h1{font-weight:300}\
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:1rem}\
.grid img{width:100%;height:auto;display:block;border-radius:4px}\
figcaption{font-size:.8rem;color:#aaa;margin-top:.25rem}";

const MINIMAL_CSS: &str =
    "body{background:#fafafa;color:#000;font-family:Georgia,serif;margin:0;padding:1rem}\
h1{font-weight:400;font-size:1.2rem}\
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(180px,1fr));gap:.25rem}\
.grid img{width:100%;height:auto;display:block}\
figcaption{display:none}";

/// Ein Foto in der fertigen Galerie — `file_name` ist bereits ein
/// relativer Pfad (z. B. `photos/0001.jpg`), wie ihn [`export_gallery`]
/// erzeugt.
pub struct GalleryPhoto {
    pub file_name: String,
    pub caption: String,
}

/// Baut die vollständige `index.html` — reine String-Funktion, keine
/// Dateisystemzugriffe.
pub fn generate_gallery_html(title: &str, theme: GalleryTheme, photos: &[GalleryPhoto]) -> String {
    let mut figures = String::new();
    for photo in photos {
        figures.push_str(&format!(
            "<figure><img src=\"{}\" loading=\"lazy\" alt=\"{}\"><figcaption>{}</figcaption></figure>\n",
            html_escape(&photo.file_name),
            html_escape(&photo.caption),
            html_escape(&photo.caption),
        ));
    }
    format!(
        "<!doctype html>\n<html lang=\"de\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{title}</title>\n<style>{css}</style>\n</head>\n<body>\n<h1>{title}</h1>\n<div class=\"grid\">\n{figures}</div>\n</body>\n</html>\n",
        title = html_escape(title),
        css = theme.css(),
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[derive(Debug)]
pub struct GalleryOutcome {
    pub dest_dir: PathBuf,
    pub photo_count: usize,
}

/// Erzeugt die vollständige Galerie auf der Platte: `dest_dir/index.html`
/// sowie `dest_dir/photos/NNNN.jpg` (Miniaturbilder, längere Kante
/// höchstens `max_edge` Pixel). `photos` sind bereits gerenderte
/// RGBA8-Puffer (`(width, height, rgba, caption)`, siehe
/// `engine::render_to_pixels`).
pub fn export_gallery(
    photos: &[(u32, u32, Vec<u8>, String)],
    title: &str,
    theme: GalleryTheme,
    max_edge: u32,
    dest_dir: &Path,
) -> Result<GalleryOutcome> {
    if photos.is_empty() {
        return Err(ExportError::Unsupported(
            "Galerie enthält keine Fotos".to_string(),
        ));
    }
    let photos_dir = dest_dir.join("photos");
    std::fs::create_dir_all(&photos_dir).map_err(|err| ExportError::Io {
        path: photos_dir.display().to_string(),
        message: err.to_string(),
    })?;

    let mut gallery_photos = Vec::with_capacity(photos.len());
    for (index, (width, height, rgba, caption)) in photos.iter().enumerate() {
        let (target_w, target_h) =
            resize::target_dimensions(*width, *height, SizeConstraint::MaxEdge(max_edge));
        let resized = resize::resize_rgba8(*width, *height, rgba, target_w, target_h)?;
        let bytes = encode_rgba8(
            target_w,
            target_h,
            &resized,
            ExportFormat::Jpeg,
            &EncodeOptions::default(),
        )?;
        let file_name = format!("photos/{:04}.jpg", index + 1);
        let dest_path = dest_dir.join(&file_name);
        std::fs::write(&dest_path, &bytes).map_err(|err| ExportError::Io {
            path: dest_path.display().to_string(),
            message: err.to_string(),
        })?;
        gallery_photos.push(GalleryPhoto {
            file_name,
            caption: caption.clone(),
        });
    }

    let html = generate_gallery_html(title, theme, &gallery_photos);
    let index_path = dest_dir.join("index.html");
    std::fs::write(&index_path, html).map_err(|err| ExportError::Io {
        path: index_path.display().to_string(),
        message: err.to_string(),
    })?;

    Ok(GalleryOutcome {
        dest_dir: dest_dir.to_path_buf(),
        photo_count: photos.len(),
    })
}

/// Verbindungsdaten für einen FTP-Upload — reines FTP, kein FTPS/TLS in
/// dieser Stufe (siehe Moduldoku).
pub struct FtpTarget {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    /// Leer = Server-Wurzelverzeichnis nach dem Login.
    pub remote_dir: String,
}

/// Lädt die von [`export_gallery`] erzeugte Ordnerstruktur (`index.html`
/// sowie `photos/*.jpg`) auf einen FTP-Server hoch — gibt die Anzahl
/// hochgeladener Dateien zurück.
pub fn upload_via_ftp(local_dir: &Path, target: &FtpTarget) -> Result<usize> {
    let mut ftp =
        suppaftp::FtpStream::connect((target.host.as_str(), target.port)).map_err(|err| {
            ExportError::Upload {
                message: format!(
                    "FTP-Verbindung zu '{}:{}' fehlgeschlagen: {err}",
                    target.host, target.port
                ),
            }
        })?;
    ftp.login(&target.username, &target.password)
        .map_err(|err| ExportError::Upload {
            message: format!("FTP-Anmeldung fehlgeschlagen: {err}"),
        })?;

    if !target.remote_dir.is_empty() {
        let _ = ftp.mkdir(&target.remote_dir); // existiert ggf. schon — kein Fehler
        ftp.cwd(&target.remote_dir)
            .map_err(|err| ExportError::Upload {
                message: format!("Zielordner '{}' nicht erreichbar: {err}", target.remote_dir),
            })?;
    }

    let mut uploaded = 0usize;
    upload_dir_recursive(&mut ftp, local_dir, local_dir, &mut uploaded)?;
    let _ = ftp.quit();
    Ok(uploaded)
}

fn upload_dir_recursive(
    ftp: &mut suppaftp::FtpStream,
    root: &Path,
    dir: &Path,
    uploaded: &mut usize,
) -> Result<()> {
    let entries = std::fs::read_dir(dir).map_err(|err| ExportError::Io {
        path: dir.display().to_string(),
        message: err.to_string(),
    })?;
    for entry in entries {
        let entry = entry.map_err(|err| ExportError::Io {
            path: dir.display().to_string(),
            message: err.to_string(),
        })?;
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if path.is_dir() {
            let _ = ftp.mkdir(&rel);
            upload_dir_recursive(ftp, root, &path, uploaded)?;
        } else {
            let mut file = std::fs::File::open(&path).map_err(|err| ExportError::Io {
                path: path.display().to_string(),
                message: err.to_string(),
            })?;
            ftp.put_file(&rel, &mut file)
                .map_err(|err| ExportError::Upload {
                    message: format!("Hochladen von '{rel}' fehlgeschlagen: {err}"),
                })?;
            *uploaded += 1;
        }
    }
    Ok(())
}

/// Verbindungsdaten für einen SFTP-Upload (Nutzername/Passwort, kein
/// Schlüsseldatei-Login — siehe Moduldoku).
pub struct SftpTarget {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    /// Leer = Server-Wurzelverzeichnis nach dem Login.
    pub remote_dir: String,
}

/// SSH-`Handler`, der jeden Server-Schlüssel akzeptiert (kein Pinning,
/// siehe Moduldoku).
struct AcceptAnyHostKey;

impl russh::client::Handler for AcceptAnyHostKey {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> std::result::Result<bool, Self::Error> {
        Ok(true)
    }
}

/// Wie [`upload_via_ftp`], aber über SFTP (SSH-Dateiübertragung) — läuft
/// asynchron, weil `russh` einen Tokio-Kontext braucht (in `apx-app`s
/// Tauri-Commands bereits vorhanden).
pub async fn upload_via_sftp(local_dir: &Path, target: &SftpTarget) -> Result<usize> {
    let config = std::sync::Arc::new(russh::client::Config::default());
    let mut session = russh::client::connect(
        config,
        (target.host.as_str(), target.port),
        AcceptAnyHostKey,
    )
    .await
    .map_err(|err| ExportError::Upload {
        message: format!(
            "SFTP-Verbindung zu '{}:{}' fehlgeschlagen: {err}",
            target.host, target.port
        ),
    })?;

    let auth = session
        .authenticate_password(&target.username, &target.password)
        .await
        .map_err(|err| ExportError::Upload {
            message: format!("SFTP-Anmeldung fehlgeschlagen: {err}"),
        })?;
    if !auth.success() {
        return Err(ExportError::Upload {
            message: "SFTP-Anmeldung abgelehnt (Nutzername/Passwort prüfen)".to_string(),
        });
    }

    let channel = session
        .channel_open_session()
        .await
        .map_err(|err| ExportError::Upload {
            message: format!("SFTP-Kanal fehlgeschlagen: {err}"),
        })?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|err| ExportError::Upload {
            message: format!("SFTP-Subsystem fehlgeschlagen: {err}"),
        })?;
    let sftp = russh_sftp::client::SftpSession::new(channel.into_stream())
        .await
        .map_err(|err| ExportError::Upload {
            message: format!("SFTP-Sitzung fehlgeschlagen: {err}"),
        })?;

    if !target.remote_dir.is_empty() {
        let _ = sftp.create_dir(&target.remote_dir).await; // existiert ggf. schon
    }

    let mut uploaded = 0usize;
    upload_dir_shallow_sftp(&sftp, local_dir, target.remote_dir.clone(), &mut uploaded).await?;
    Ok(uploaded)
}

/// Läuft `local_dir` rekursiv ab und lädt jede Datei per SFTP hoch —
/// `Box::pin` umgeht Rusts Einschränkung, dass async-Funktionen sich
/// nicht direkt selbst aufrufen dürfen.
fn upload_dir_shallow_sftp<'a>(
    sftp: &'a russh_sftp::client::SftpSession,
    local_dir: &'a Path,
    remote_prefix: String,
    uploaded: &'a mut usize,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        use tokio::io::AsyncWriteExt;

        let entries = std::fs::read_dir(local_dir).map_err(|err| ExportError::Io {
            path: local_dir.display().to_string(),
            message: err.to_string(),
        })?;
        for entry in entries {
            let entry = entry.map_err(|err| ExportError::Io {
                path: local_dir.display().to_string(),
                message: err.to_string(),
            })?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let remote_path = if remote_prefix.is_empty() {
                name
            } else {
                format!("{remote_prefix}/{name}")
            };

            if path.is_dir() {
                let _ = sftp.create_dir(&remote_path).await;
                upload_dir_shallow_sftp(sftp, &path, remote_path, uploaded).await?;
            } else {
                let data = std::fs::read(&path).map_err(|err| ExportError::Io {
                    path: path.display().to_string(),
                    message: err.to_string(),
                })?;
                let mut file =
                    sftp.create(&remote_path)
                        .await
                        .map_err(|err| ExportError::Upload {
                            message: format!("Hochladen von '{remote_path}' fehlgeschlagen: {err}"),
                        })?;
                file.write_all(&data)
                    .await
                    .map_err(|err| ExportError::Upload {
                        message: format!("Hochladen von '{remote_path}' fehlgeschlagen: {err}"),
                    })?;
                let _ = file.shutdown().await;
                *uploaded += 1;
            }
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_photo(width: u32, height: u32) -> Vec<u8> {
        (0..width * height)
            .flat_map(|_| [180u8, 120, 60, 255])
            .collect()
    }

    #[test]
    fn generate_gallery_html_embeds_title_and_all_figures() {
        let photos = [
            GalleryPhoto {
                file_name: "photos/0001.jpg".to_string(),
                caption: "Berge".to_string(),
            },
            GalleryPhoto {
                file_name: "photos/0002.jpg".to_string(),
                caption: "Wald".to_string(),
            },
        ];
        let html = generate_gallery_html("Urlaub 2026", GalleryTheme::Dark, &photos);
        assert!(html.contains("Urlaub 2026"));
        assert!(html.contains("photos/0001.jpg"));
        assert!(html.contains("photos/0002.jpg"));
        assert!(html.contains("<!doctype html>"));
    }

    #[test]
    fn generate_gallery_html_escapes_special_characters() {
        let photos = [GalleryPhoto {
            file_name: "photos/0001.jpg".to_string(),
            caption: "<script>".to_string(),
        }];
        let html = generate_gallery_html("A & B", GalleryTheme::Light, &photos);
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("A &amp; B"));
    }

    #[test]
    fn export_gallery_writes_index_and_thumbnail_files() {
        let dir = tempfile::tempdir().unwrap();
        let photos = vec![(8u32, 8u32, solid_photo(8, 8), "Test".to_string())];
        let outcome = export_gallery(
            &photos,
            "Test-Galerie",
            GalleryTheme::Minimal,
            4,
            dir.path(),
        )
        .unwrap();
        assert_eq!(outcome.photo_count, 1);
        assert!(dir.path().join("index.html").exists());
        assert!(dir.path().join("photos/0001.jpg").exists());
    }

    #[test]
    fn export_gallery_rejects_empty_photo_list() {
        let dir = tempfile::tempdir().unwrap();
        let err = export_gallery(&[], "Leer", GalleryTheme::Light, 800, dir.path()).unwrap_err();
        assert!(matches!(err, ExportError::Unsupported(_)));
    }

    #[test]
    fn upload_via_ftp_reports_a_clean_error_when_unreachable() {
        // Kein echter FTP-Server in dieser Sandbox — die Verbindung muss
        // sauber fehlschlagen statt zu blockieren/abzustürzen.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "x").unwrap();
        let target = FtpTarget {
            host: "127.0.0.1".to_string(),
            port: 1, // reserviert, garantiert kein FTP-Dienst dahinter
            username: "anonymous".to_string(),
            password: "".to_string(),
            remote_dir: String::new(),
        };
        let err = upload_via_ftp(dir.path(), &target).unwrap_err();
        assert!(matches!(err, ExportError::Upload { .. }));
    }

    #[tokio::test]
    async fn upload_via_sftp_reports_a_clean_error_when_unreachable() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "x").unwrap();
        let target = SftpTarget {
            host: "127.0.0.1".to_string(),
            port: 1,
            username: "anonymous".to_string(),
            password: "".to_string(),
            remote_dir: String::new(),
        };
        let err = upload_via_sftp(dir.path(), &target).await.unwrap_err();
        assert!(matches!(err, ExportError::Upload { .. }));
    }
}
