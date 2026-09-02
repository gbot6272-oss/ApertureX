//! Aperture X — Tethered Shooting (Phase 9 Schritt 11, `SPEC.md` §5,
//! siehe `PLAN.md`, `DECISIONS.md` ADR-0035 Punkt 5).
//!
//! [`TetherBackend`] beschreibt den vollen Ablauf einer Aufnahme über
//! eine per USB/PTP angeschlossene Kamera: Kamera erkennen, auslösen,
//! das aufgenommene Bild herunterladen. Zwei Implementierungen:
//!
//! - [`FakeBackend`] — läuft in jedem normalen `cargo test`, unabhängig
//!   vom `tethering`-Feature. "Nimmt" ein echtes, gültiges JPEG auf (über
//!   die `image`-Crate erzeugt, keine Null-Bytes) und "lädt" es in ein
//!   Zielverzeichnis herunter — genug, damit `apx-app`s Aufrufer (der
//!   danach den bestehenden Import-Pfad aus Phase 3/5 anstößt, siehe
//!   `PLAN.md`) den gesamten Ablauf ohne echte Hardware durchtesten kann.
//! - [`gphoto2_backend::Gphoto2Backend`] — echte `libgphoto2`-FFI-Aufrufe
//!   über die `gphoto2`-Crate, nur kompiliert mit dem Cargo-Feature
//!   `tethering` (standardmäßig aus).
//!
//! **Ehrlich begrenzt — über `DECISIONS.md` ADR-0034s ffmpeg-Präzedenzfall
//! hinaus bewusst eingeschränkt:** `Gphoto2Backend`s Aufrufe sind real
//! geschrieben (siehe deren Moduldoku für die verifizierten
//! API-Signaturen), aber in dieser Sandbox **nie gegen eine echte Kamera
//! oder auch nur eine installierte `libgphoto2`-Bibliothek ausgeführt**
//! — Letztere fehlt hier und im Standard-CI vollständig. Anders als bei
//! FTP/SFTP (ADR-0034 Punkt 5) ist nicht einmal ein
//! "unerreichbarer Server"-Fehlerpfad testbar, weil schon das
//! *Kompilieren* mit aktiviertem Feature die Systembibliothek braucht.

use std::path::{Path, PathBuf};

use thiserror::Error;

pub type Result<T> = std::result::Result<T, TetherError>;

#[derive(Debug, Error)]
pub enum TetherError {
    #[error("Keine Kamera gefunden — angeschlossen und eingeschaltet?")]
    NoCameraFound,

    #[error("Kamera-Fehler: {message}")]
    Camera { message: String },

    #[error("Download vom Kamerapuffer fehlgeschlagen: {message}")]
    Download { message: String },
}

impl From<TetherError> for apx_core::AppError {
    fn from(err: TetherError) -> Self {
        apx_core::AppError::tether(err.to_string())
    }
}

/// Minimale Kamera-Kennung, wie von `libgphoto2` gemeldet — Modellname
/// und Verbindungsport (z. B. `"usb:001,004"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraInfo {
    pub model: String,
    pub port: String,
}

/// Der volle Tethering-Ablauf: Kamera erkennen, auslösen, herunterladen.
/// Absichtlich **kein** separates Konfigurations-/Live-View-API — der
/// Umfang bleibt auf den in `PLAN.md` beschriebenen Kernablauf
/// beschränkt (Kamera erkennen → auslösen → herunterladen → automatisches
/// Import-Preset), nicht die volle `libgphoto2`-Fähigkeitsfläche.
///
/// `Send` ist Teil des Vertrags: `apx-app` hält das aktive Backend über
/// mehrere Tauri-Command-Aufrufe hinweg in einem `Mutex` (damit die
/// Kamera-Verbindung und der Aufnahmezähler zwischen "erkennen" und
/// mehreren "auslösen"-Aufrufen erhalten bleiben) — das verlangt `State`
/// selbst nach `Send + Sync`.
pub trait TetherBackend: Send {
    /// Erkennt die angeschlossene Kamera, oder `None`, wenn keine
    /// gefunden wurde (kein Fehler — das ist der Normalfall, solange
    /// niemand eine Kamera angeschlossen hat).
    fn detect_camera(&mut self) -> Result<Option<CameraInfo>>;

    /// Löst aus und lädt die aufgenommene Datei nach `dest_dir` herunter.
    /// Gibt den lokalen Pfad der heruntergeladenen Datei zurück — der
    /// Aufrufer (`apx-app`) übergibt diesen Pfad danach unverändert an
    /// den bestehenden Import-Pfad (`import::run_with_mode`, Phase 3/5).
    /// Ein Fehler, wenn [`detect_camera`](Self::detect_camera) zuvor
    /// keine Kamera gefunden hat.
    fn capture_and_download(&mut self, dest_dir: &Path) -> Result<PathBuf>;
}

/// Test-/Entwicklungs-Backend ohne echte Hardware. Simuliert eine
/// Kamera, die bei jedem Auslösen ein neues, echtes (über die
/// `image`-Crate kodiertes) JPEG liefert — kein Null-Byte-Platzhalter,
/// damit der nachgelagerte Import-Pfad (liest EXIF/Abmessungen, erzeugt
/// eine Vorschau) dieselbe Datei wie bei einer echten Aufnahme
/// verarbeiten kann.
pub struct FakeBackend {
    camera: Option<CameraInfo>,
    next_sequence: u32,
}

impl FakeBackend {
    /// Eine simulierte Kamera ist angeschlossen.
    pub fn connected(model: impl Into<String>) -> Self {
        Self {
            camera: Some(CameraInfo {
                model: model.into(),
                port: "usb:mock,000".to_string(),
            }),
            next_sequence: 1,
        }
    }

    /// Keine Kamera angeschlossen — `detect_camera` liefert `None`,
    /// `capture_and_download` schlägt fehl (deckt den "keine Kamera"-Pfad
    /// ab, ohne echte Hardware zu brauchen).
    pub fn disconnected() -> Self {
        Self {
            camera: None,
            next_sequence: 1,
        }
    }
}

impl TetherBackend for FakeBackend {
    fn detect_camera(&mut self) -> Result<Option<CameraInfo>> {
        Ok(self.camera.clone())
    }

    fn capture_and_download(&mut self, dest_dir: &Path) -> Result<PathBuf> {
        if self.camera.is_none() {
            return Err(TetherError::NoCameraFound);
        }
        std::fs::create_dir_all(dest_dir).map_err(|err| TetherError::Download {
            message: format!("Zielverzeichnis nicht anlegbar: {err}"),
        })?;

        let sequence = self.next_sequence;
        self.next_sequence += 1;
        let filename = format!("TETHER_{sequence:04}.jpg");
        let path = dest_dir.join(filename);

        // Ein winziges, aber echtes 4x3-JPEG — derselbe Ansatz wie
        // `import::mod.rs`s eigene `write_valid_jpeg`-Testhilfe, damit
        // der Aufrufer (`apx-app`s Tether-Command) exakt denselben
        // Import-Pfad wie bei einer echten Aufnahme durchläuft.
        let image = image::RgbImage::from_pixel(4, 3, image::Rgb([120, 140, 160]));
        image::DynamicImage::ImageRgb8(image)
            .save_with_format(&path, image::ImageFormat::Jpeg)
            .map_err(|err| TetherError::Download {
                message: format!("Test-JPEG nicht schreibbar: {err}"),
            })?;

        Ok(path)
    }
}

/// Echtes `libgphoto2`-Backend — nur mit dem Cargo-Feature `tethering`
/// kompiliert (siehe Moduldoku oben für die Sandbox-Einschränkung).
#[cfg(feature = "tethering")]
pub mod gphoto2_backend {
    //! Echte `libgphoto2`-FFI-Aufrufe über die `gphoto2`-Crate (MIT,
    //! bindet dynamisch an die Systembibliothek `libgphoto2`, LGPL-2.1 —
    //! siehe `THIRD_PARTY.md`). API-Signaturen gegen die tatsächliche
    //! Crate-Dokumentation verifiziert (`Context::new`,
    //! `Context::autodetect_camera`, `Camera::capture_image`,
    //! `CameraFS::download_to`) — `gphoto2::task::Task::wait` blockiert
    //! synchron, passend zu [`super::TetherBackend`]s synchroner
    //! Signatur, ohne dass `apx-tether` selbst eine Async-Runtime
    //! braucht.

    use std::path::{Path, PathBuf};

    use super::{CameraInfo, Result, TetherBackend, TetherError};

    /// Hält den `libgphoto2`-Kontext und — nach einer erfolgreichen
    /// [`TetherBackend::detect_camera`] — die geöffnete Kamera-Handle.
    pub struct Gphoto2Backend {
        context: gphoto2::Context,
        camera: Option<gphoto2::Camera>,
        info: Option<CameraInfo>,
    }

    impl Gphoto2Backend {
        pub fn new() -> Result<Self> {
            let context = gphoto2::Context::new().map_err(|err| TetherError::Camera {
                message: err.to_string(),
            })?;
            Ok(Self {
                context,
                camera: None,
                info: None,
            })
        }
    }

    impl TetherBackend for Gphoto2Backend {
        fn detect_camera(&mut self) -> Result<Option<CameraInfo>> {
            match self.context.autodetect_camera().wait() {
                Ok(camera) => {
                    // `abilities()` liefert das Modell synchron, ohne
                    // eine weitere Task — `port_info()` den Verbindungs-
                    // port. Beide sind reine Metadaten-Abfragen, keine
                    // Kamerakommunikation.
                    let model = camera.abilities().model().to_string();
                    let port = camera
                        .port_info()
                        .map(|info| info.path().to_string())
                        .unwrap_or_else(|_| "unbekannt".to_string());
                    let info = CameraInfo { model, port };
                    self.info = Some(info.clone());
                    self.camera = Some(camera);
                    Ok(Some(info))
                }
                Err(_) => {
                    self.camera = None;
                    self.info = None;
                    Ok(None)
                }
            }
        }

        fn capture_and_download(&mut self, dest_dir: &Path) -> Result<PathBuf> {
            let camera = self.camera.as_ref().ok_or(TetherError::NoCameraFound)?;
            std::fs::create_dir_all(dest_dir).map_err(|err| TetherError::Download {
                message: format!("Zielverzeichnis nicht anlegbar: {err}"),
            })?;

            let captured = camera
                .capture_image()
                .wait()
                .map_err(|err| TetherError::Camera {
                    message: err.to_string(),
                })?;
            // `folder()`/`name()` liefern `Cow<str>` (nicht `&str`, wie in
            // der Crate-Dokumentation zunächst angenommen — beim ersten
            // echten Kompilieren mit installiertem `libgphoto2-dev` in
            // Phase 11 Schritt 10 aufgefallen und korrigiert) — `&str`
            // per Deref-Koerzion daraus geliehen, `dest_path` braucht den
            // Namen als eigenständiges `Path`-Segment.
            let name: &str = &captured.name();
            let folder: &str = &captured.folder();
            let dest_path = dest_dir.join(name);
            camera
                .fs()
                .download_to(folder, name, &dest_path)
                .wait()
                .map_err(|err| TetherError::Download {
                    message: err.to_string(),
                })?;
            Ok(dest_path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disconnected_backend_reports_no_camera() {
        let mut backend = FakeBackend::disconnected();
        assert_eq!(backend.detect_camera().expect("ok"), None);
    }

    #[test]
    fn disconnected_backend_refuses_to_capture() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let mut backend = FakeBackend::disconnected();
        let result = backend.capture_and_download(tmp.path());
        assert!(matches!(result, Err(TetherError::NoCameraFound)));
    }

    #[test]
    fn connected_backend_reports_the_configured_camera() {
        let mut backend = FakeBackend::connected("EOS 90D");
        let info = backend
            .detect_camera()
            .expect("ok")
            .expect("Kamera erkannt");
        assert_eq!(info.model, "EOS 90D");
    }

    #[test]
    fn capture_writes_a_real_decodable_jpeg_into_dest_dir() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let mut backend = FakeBackend::connected("EOS 90D");
        backend.detect_camera().expect("ok");

        let path = backend
            .capture_and_download(tmp.path())
            .expect("Aufnahme sollte gelingen");
        assert!(path.starts_with(tmp.path()));
        assert_eq!(path.extension().and_then(|ext| ext.to_str()), Some("jpg"));

        let decoded = image::open(&path).expect("sollte ein gültiges Bild sein");
        assert_eq!((decoded.width(), decoded.height()), (4, 3));
    }

    #[test]
    fn successive_captures_get_distinct_incrementing_filenames() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let mut backend = FakeBackend::connected("EOS 90D");
        backend.detect_camera().expect("ok");

        let first = backend.capture_and_download(tmp.path()).expect("ok");
        let second = backend.capture_and_download(tmp.path()).expect("ok");
        assert_ne!(first, second);
    }
}

/// Phase 11 Schritt 10: mit installiertem `libgphoto2-dev` läuft dieser
/// Test jetzt echt gegen die reale `libgphoto2`-C-API statt nur
/// strukturell zu kompilieren (siehe DECISIONS.md ADR-0038) — genau die
/// von ADR-0035 Punkt 5 als nicht testbar bezeichnete Klasse. Ohne
/// angeschlossene Kamera muss `detect_camera()` einen sauberen `Ok(None)`
/// statt eines Panics liefern (max. 1 Test für diesen Schritt, siehe die
/// vom Nutzer ab Schritt 4 gelockerte Testdisziplin).
#[cfg(all(test, feature = "tethering"))]
mod gphoto2_tests {
    use super::gphoto2_backend::Gphoto2Backend;
    use super::TetherBackend;

    #[test]
    fn detect_camera_without_hardware_returns_a_clean_none_not_a_panic() {
        let mut backend = Gphoto2Backend::new().expect("libgphoto2-Kontext sollte aufbaubar sein");
        let result = backend.detect_camera();
        assert!(
            matches!(result, Ok(None)),
            "ohne angeschlossene Kamera erwartet: Ok(None), erhalten: {result:?}"
        );
    }
}
