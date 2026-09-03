//! Echte Personen-Wiedererkennung (Phase 13 Schritt 8, siehe `PLAN.md`
//! und `DECISIONS.md` ADR-0040-Nachtrag VI) — echte Gesichts-Embeddings
//! statt der groben Hautton-Heuristik in [`crate::faces`] (die bleibt
//! als Fallback bestehen, wenn dieses Feature nicht kompiliert oder
//! keine Modelle hinterlegt sind).
//!
//! **Lizenz zuerst geprüft, wie von `PLAN.md` verlangt:** die naheliegenden
//! Kandidaten (InsightFace `buffalo_l`/`antelopev2`, OpenCV Zoo `SFace`)
//! scheiden aus — InsightFaces Modelle sind laut deren eigener
//! Modell-Zoo-Dokumentation ausdrücklich „für nicht-kommerzielle
//! Forschungszwecke" lizenziert (kommerzielle Nutzung erfordert eine
//! separate Lizenz von InsightFace selbst), SFaces Trainingsdatensatz
//! ist auf Nachfrage nicht eindeutig geklärt (`opencv/opencv_zoo#124`,
//! `opencv/opencv#21192`: das Apache-2.0-`LICENSE`-Datei im Repo-Verzeichnis
//! klärt nicht, ob `MS1MV2` — abgeleitet vom wegen Herkunfts-/
//! Einwilligungsproblemen 2019 zurückgezogenen `MS-Celeb-1M` — im
//! tatsächlich verteilten `.onnx` steckt).
//!
//! **Tatsächlich verwendet:** `dlib`s eigenes Gesichts-Embedding-Netz
//! (`dlib_face_recognition_resnet_model_v1.dat`) — von dessen Autor
//! (davisking, `dlib-models`-Repo-README) ausdrücklich als gemeinfrei
//! erklärt („anyone can do whatever they want with these model files as
//! I've released them into the public domain"), trotz teils
//! nicht-kommerziell lizenzierter Trainingsquellen (Face Scrub), weil der
//! Autor als Rechteinhaber des *trainierten Modells* (nicht der
//! Trainingsdaten selbst) diese Freigabe explizit ausgesprochen hat.
//! Für die zur Ausrichtung nötigen Gesichts-Landmarken **nicht** das im
//! selben Repo mitgelieferte 68-Punkte-Modell
//! (`shape_predictor_68_face_landmarks.dat`) — dessen README nennt
//! ausdrücklich einen Autoren-Hinweis des Datensatz-Erstellers, der
//! kommerzielle Nutzung explizit ausschließt — sondern das 5-Punkte-Modell
//! (`shape_predictor_5_face_landmarks.dat`, CC0-1.0/gemeinfrei, eigener
//! `dlib`-Datensatz). `dlib::get_face_chip_details` (von der
//! `dlib-face-recognition`-Crate intern aufgerufen) unterstützt beide
//! Landmark-Zahlen gleichermaßen — dieselbe 5-Punkte-Ausrichtung, die z. B.
//! auch `ageitgey/face_recognition` standardmäßig anbietet.
//!
//! Die Gesichts-*Erkennung* selbst (Bounding-Boxes, bevor überhaupt ein
//! Embedding berechnet wird) läuft über `dlib::get_frontal_face_detector`
//! — vollständig in `libdlib` selbst einkompiliert (Boost Software
//! License 1.0), keine externe Modelldatei, keine separate Lizenzfrage.
//!
//! **Echt spike-verifiziert** (nicht nur `cargo add --dry-run`): gegen
//! drei echte, gemeinfreie Fotos (US-Regierungsfotos, Pete Souza/
//! Weißes Haus) lief die volle Kette Erkennen→Ausrichten→Einbetten→
//! Vergleichen; zwei Fotos derselben Person lagen bei Abstand 0.35 (< der
//! von `dlib` selbst dokumentierten Schwelle [`SAME_PERSON_THRESHOLD`]
//! von 0.6), ein Foto einer anderen Person bei 0.85 (darüber) — siehe
//! `DECISIONS.md` ADR-0040-Nachtrag VI für die vollständige Herleitung.
//!
//! **Bekannter, real gefundener Fehler in der Abhängigkeitskette
//! behoben:** `dlib-face-recognition-sys`s `build.rs` versucht *immer*
//! zuerst, `dlib`s Quellcode von `dlib.net` herunterzuladen, bevor es den
//! eigenen (aber dadurch toten) pkg-config-Pfad gegen eine bereits
//! installierte System-`libdlib` überhaupt versucht — siehe
//! `vendor/dlib-face-recognition-sys/VENDORED.md` für den lokalen Fix
//! (nur Umsortierung zweier vorhandener Codeblöcke, keine neue Logik).

use std::path::Path;

use dlib_face_recognition::{
    FaceDetector, FaceDetectorTrait, FaceEncoderNetwork, FaceEncoderTrait, ImageMatrix,
    LandmarkPredictor, LandmarkPredictorTrait, Rectangle,
};

use crate::error::{AiError, Result};

pub use apx_catalog::embedding_distance;
/// Re-Export: `dlib`s eigener empfohlener Schwellenwert für „dieselbe
/// Person" (euklidischer Abstand zweier 128-dimensionaler Embeddings,
/// siehe `dlib_face_recognition::FaceEncoding::distance`s Doku-Kommentar,
/// dort wörtlich dieselbe Zahl) — in `apx_catalog::models` definiert
/// (nicht hier), damit `apx-catalog::repository::people`s Auto-
/// Zuordnungslogik unabhängig vom `people`-Feature kompiliert.
pub use apx_catalog::SAME_PERSON_EMBEDDING_THRESHOLD as SAME_PERSON_THRESHOLD;

/// Ein erkanntes Gesicht mit Bounding-Box (Bildpixel-Koordinaten,
/// Ursprung oben links) und 128-dimensionalem Embedding.
#[derive(Debug, Clone)]
pub struct DetectedFace {
    pub left: i64,
    pub top: i64,
    pub right: i64,
    pub bottom: i64,
    pub embedding: Vec<f64>,
}

/// Lädt die beiden benötigten Modelldateien einmal und hält sie für
/// mehrfache Aufrufe von [`PersonEmbedder::detect_and_embed`] bereit
/// (Deserialisieren ist nicht kostenlos, siehe `dlib`s eigene Doku zur
/// `shape_predictor`/`face_encoding_nn`-Deserialisierung).
pub struct PersonEmbedder {
    detector: FaceDetector,
    predictor: LandmarkPredictor,
    encoder: FaceEncoderNetwork,
}

impl PersonEmbedder {
    /// `landmark_model_path` muss auf `shape_predictor_5_face_landmarks.dat`
    /// zeigen (**nicht** die 68-Punkte-Variante, siehe Moduldoku),
    /// `encoder_model_path` auf `dlib_face_recognition_resnet_model_v1.dat`
    /// — beide vom Nutzer selbst heruntergeladen (kein Bundling im
    /// Installer, derselbe Opt-in wie beim LaMa-Inpainting-Modell aus
    /// Schritt 1).
    pub fn new(landmark_model_path: &Path, encoder_model_path: &Path) -> Result<Self> {
        let predictor =
            LandmarkPredictor::open(landmark_model_path).map_err(|message| AiError::Model {
                message: format!("Landmarken-Modell konnte nicht geladen werden: {message}"),
            })?;
        let encoder =
            FaceEncoderNetwork::open(encoder_model_path).map_err(|message| AiError::Model {
                message: format!("Embedding-Modell konnte nicht geladen werden: {message}"),
            })?;
        Ok(Self {
            detector: FaceDetector::new(),
            predictor,
            encoder,
        })
    }

    /// Erkennt alle Gesichter in `rgb8` (interleaved sRGB `u8`, wie von
    /// `image::RgbImage::as_raw()`/`.into_raw()` geliefert) und berechnet
    /// für jedes ein 128-dimensionales Embedding. Läuft auf einer bereits
    /// gerenderten Vorschau (`apx_catalog::PreviewLevel::Standard`), nicht
    /// auf dem vollen Originalbild — dieselbe Auflösungsbegrenzung wie
    /// jede andere KI-Analyse in diesem Projekt.
    pub fn detect_and_embed(
        &self,
        rgb8: &[u8],
        width: u32,
        height: u32,
    ) -> Result<Vec<DetectedFace>> {
        if width == 0 || height == 0 || rgb8.len() < (width as usize * height as usize * 3) {
            return Err(AiError::Analysis {
                message: "Bild zu klein oder Pixel-Puffer zu kurz für die Gesichtserkennung"
                    .to_string(),
            });
        }
        // Sicher: `rgb8` hat mindestens `width * height * 3` Bytes (oben
        // geprüft), `ImageMatrix::new`s einzige Voraussetzung.
        let matrix = unsafe { ImageMatrix::new(width as usize, height as usize, rgb8.as_ptr()) };

        let locations = self.detector.face_locations(&matrix);
        if locations.is_empty() {
            return Ok(Vec::new());
        }

        let landmarks: Vec<_> = locations
            .iter()
            .map(|rect| self.predictor.face_landmarks(&matrix, rect))
            .collect();
        let encodings = self.encoder.get_face_encodings(&matrix, &landmarks, 0);

        Ok(locations
            .iter()
            .zip(encodings.iter())
            .map(|(rect, encoding)| {
                let Rectangle {
                    left,
                    top,
                    right,
                    bottom,
                } = *rect;
                DetectedFace {
                    left,
                    top,
                    right,
                    bottom,
                    embedding: encoding.as_ref().to_vec(),
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Läuft nur, wenn beide Modelldateien lokal vorliegen (Umgebungs-
    /// variablen `APX_TEST_LANDMARK_MODEL`/`APX_TEST_ENCODER_MODEL`) —
    /// dieselbe „übersprungen, nicht fehlgeschlagen"-Konvention wie
    /// `apx-pipeline`s GPU-Adapter-Tests: kein Netzwerk-Download in CI
    /// (siehe `PLAN.md` Phase 13s Verifikations-Abschnitt), die Modelle
    /// sind ~22 MB/~9 MB groß und werden vom Nutzer selbst heruntergeladen.
    #[test]
    fn detects_and_embeds_real_faces_when_models_are_locally_available() {
        let (Ok(landmark_path), Ok(encoder_path)) = (
            std::env::var("APX_TEST_LANDMARK_MODEL"),
            std::env::var("APX_TEST_ENCODER_MODEL"),
        ) else {
            eprintln!("übersprungen: APX_TEST_LANDMARK_MODEL/APX_TEST_ENCODER_MODEL nicht gesetzt");
            return;
        };
        let (Ok(photo_a), Ok(photo_b), Ok(photo_c)) = (
            std::env::var("APX_TEST_FACE_PHOTO_A"),
            std::env::var("APX_TEST_FACE_PHOTO_B"),
            std::env::var("APX_TEST_FACE_PHOTO_C"),
        ) else {
            eprintln!(
                "übersprungen: APX_TEST_FACE_PHOTO_A/B/C (zwei Fotos derselben, eines einer anderen Person) nicht gesetzt"
            );
            return;
        };

        let embedder = PersonEmbedder::new(Path::new(&landmark_path), Path::new(&encoder_path))
            .expect("Modelle sollten sich laden lassen");

        let embed_first = |path: &str| -> Vec<f64> {
            let img = image::open(path)
                .expect("Testfoto sollte lesbar sein")
                .to_rgb8();
            let faces = embedder
                .detect_and_embed(img.as_raw(), img.width(), img.height())
                .expect("sollte ohne Fehler laufen");
            assert!(
                !faces.is_empty(),
                "erwartet mindestens ein Gesicht in {path}"
            );
            faces[0].embedding.clone()
        };

        let a = embed_first(&photo_a);
        let b = embed_first(&photo_b);
        let c = embed_first(&photo_c);

        let same = embedding_distance(&a, &b);
        let different = embedding_distance(&a, &c);
        assert!(
            same < SAME_PERSON_THRESHOLD,
            "gleiche Person sollte unter der Schwelle liegen, war {same}"
        );
        assert!(
            different >= SAME_PERSON_THRESHOLD,
            "andere Person sollte über der Schwelle liegen, war {different}"
        );
    }
}
