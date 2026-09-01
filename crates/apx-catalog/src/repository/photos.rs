//! SQL für die `photos`-Tabelle.

use apx_core::{AppError, FolderId, PhotoId, Result};
use rusqlite::{params, Connection, OptionalExtension};
use time::OffsetDateTime;

use crate::error::map_sqlite_err;
use crate::models::{from_unix, from_unix_opt, to_unix, to_unix_opt, NewPhoto, Photo};

/// Qualifiziert mit `photos.`, damit dieselbe Spaltenliste auch in
/// `repository::search`s Joins mit `photos_fts` verwendbar ist, ohne dass
/// gleichnamige Spalten (z. B. `filename`) mehrdeutig werden.
pub(crate) const SELECT_COLUMNS: &str =
    "photos.id, photos.folder_id, photos.filename, photos.file_size, photos.file_mtime, \
     photos.content_hash, photos.width, photos.height, photos.orientation, photos.camera_make, \
     photos.camera_model, photos.lens, photos.iso, photos.shutter, photos.aperture, \
     photos.focal_length, photos.captured_at, photos.gps_lat, photos.gps_lon, \
     photos.imported_at, photos.missing, photos.rating, photos.flag, photos.color_label, \
     photos.source_photo_id, photos.title, photos.caption, photos.copyright, photos.creator";

#[allow(clippy::type_complexity)]
pub(crate) struct PhotoRow {
    id: String,
    folder_id: String,
    filename: String,
    file_size: i64,
    file_mtime: i64,
    content_hash: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
    orientation: i64,
    camera_make: Option<String>,
    camera_model: Option<String>,
    lens: Option<String>,
    iso: Option<i64>,
    shutter: Option<f64>,
    aperture: Option<f64>,
    focal_length: Option<f64>,
    captured_at: Option<i64>,
    gps_lat: Option<f64>,
    gps_lon: Option<f64>,
    imported_at: i64,
    missing: i64,
    rating: i64,
    flag: i64,
    color_label: Option<String>,
    source_photo_id: Option<String>,
    title: Option<String>,
    caption: Option<String>,
    copyright: Option<String>,
    creator: Option<String>,
}

pub(crate) fn row_to_raw(row: &rusqlite::Row) -> rusqlite::Result<PhotoRow> {
    Ok(PhotoRow {
        id: row.get(0)?,
        folder_id: row.get(1)?,
        filename: row.get(2)?,
        file_size: row.get(3)?,
        file_mtime: row.get(4)?,
        content_hash: row.get(5)?,
        width: row.get(6)?,
        height: row.get(7)?,
        orientation: row.get(8)?,
        camera_make: row.get(9)?,
        camera_model: row.get(10)?,
        lens: row.get(11)?,
        iso: row.get(12)?,
        shutter: row.get(13)?,
        aperture: row.get(14)?,
        focal_length: row.get(15)?,
        captured_at: row.get(16)?,
        gps_lat: row.get(17)?,
        gps_lon: row.get(18)?,
        imported_at: row.get(19)?,
        missing: row.get(20)?,
        rating: row.get(21)?,
        flag: row.get(22)?,
        color_label: row.get(23)?,
        source_photo_id: row.get(24)?,
        title: row.get(25)?,
        caption: row.get(26)?,
        copyright: row.get(27)?,
        creator: row.get(28)?,
    })
}

pub(crate) fn raw_to_photo(raw: PhotoRow) -> Result<Photo> {
    Ok(Photo {
        id: raw.id.parse()?,
        folder_id: raw.folder_id.parse()?,
        filename: raw.filename,
        file_size: raw.file_size as u64,
        file_mtime: from_unix(raw.file_mtime)?,
        content_hash: raw.content_hash,
        width: raw.width.map(|v| v as u32),
        height: raw.height.map(|v| v as u32),
        orientation: raw.orientation as u16,
        camera_make: raw.camera_make,
        camera_model: raw.camera_model,
        lens: raw.lens,
        iso: raw.iso.map(|v| v as u32),
        shutter: raw.shutter.map(|v| v as f32),
        aperture: raw.aperture.map(|v| v as f32),
        focal_length: raw.focal_length.map(|v| v as f32),
        captured_at: from_unix_opt(raw.captured_at)?,
        gps_lat: raw.gps_lat,
        gps_lon: raw.gps_lon,
        imported_at: from_unix(raw.imported_at)?,
        missing: raw.missing != 0,
        rating: raw.rating as u8,
        flag: raw.flag as i8,
        color_label: raw.color_label,
        source_photo_id: raw.source_photo_id.map(|s| s.parse()).transpose()?,
        title: raw.title,
        caption: raw.caption,
        copyright: raw.copyright,
        creator: raw.creator,
    })
}

/// Aktualisiert die vier IPTC-artigen Metadaten-Überschreibungen (Phase 9
/// Schritt 2) — `None` löscht das jeweilige Feld, wie bei
/// `set_color_label`. Deckt auch Stapel-Metadatenbearbeitung ab: der
/// Aufrufer ruft dies einfach für mehrere `photo_id`s hintereinander auf.
pub(crate) fn set_metadata(
    conn: &Connection,
    photo_id: PhotoId,
    title: Option<&str>,
    caption: Option<&str>,
    copyright: Option<&str>,
    creator: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE photos SET title = ?2, caption = ?3, copyright = ?4, creator = ?5 WHERE id = ?1",
        params![photo_id.to_string(), title, caption, copyright, creator],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
}

/// Nur "echte" Foto-Zeilen (`source_photo_id IS NULL`) — virtuelle
/// Kopien dürfen nie vom Import-Upsert getroffen werden, sonst würde ein
/// erneuter Import derselben Datei die virtuelle Kopie statt des
/// Quellfotos aktualisieren.
pub(crate) fn find_by_folder_and_filename(
    conn: &Connection,
    folder_id: FolderId,
    filename: &str,
) -> Result<Option<Photo>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM photos \
         WHERE folder_id = ?1 AND filename = ?2 AND source_photo_id IS NULL"
    );
    let raw: Option<PhotoRow> = conn
        .query_row(&sql, params![folder_id.to_string(), filename], row_to_raw)
        .optional()
        .map_err(map_sqlite_err)?;
    raw.map(raw_to_photo).transpose()
}

pub(crate) fn get(conn: &Connection, id: PhotoId) -> Result<Photo> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM photos WHERE id = ?1");
    let raw = conn
        .query_row(&sql, params![id.to_string()], row_to_raw)
        .map_err(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => AppError::not_found("Foto", id.to_string()),
            other => map_sqlite_err(other),
        })?;
    raw_to_photo(raw)
}

pub(crate) fn list_by_folder(conn: &Connection, folder_id: FolderId) -> Result<Vec<Photo>> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM photos WHERE folder_id = ?1 ORDER BY filename");
    let mut stmt = conn.prepare(&sql).map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map(params![folder_id.to_string()], row_to_raw)
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_photo(row.map_err(map_sqlite_err)?)?);
    }
    Ok(result)
}

pub(crate) fn count_by_folder(conn: &Connection, folder_id: FolderId) -> Result<u64> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM photos WHERE folder_id = ?1",
            params![folder_id.to_string()],
            |row| row.get(0),
        )
        .map_err(map_sqlite_err)?;
    Ok(count as u64)
}

/// Gruppen von Fotos mit identischem `content_hash` (exakte
/// Duplikaterkennung, siehe `DECISIONS.md` ADR-0027) — Fotos ohne Hash
/// (z. B. vor Schritt 8.2 importiert) werden ignoriert statt als eine
/// gemeinsame Gruppe zusammengefasst. Jede Gruppe ist nach Dateiname
/// sortiert; die Gruppen selbst haben keine definierte Reihenfolge.
pub(crate) fn list_duplicate_groups(conn: &Connection) -> Result<Vec<Vec<Photo>>> {
    let mut stmt = conn
        .prepare(
            "SELECT content_hash FROM photos \
             WHERE content_hash IS NOT NULL \
             GROUP BY content_hash HAVING COUNT(*) > 1",
        )
        .map_err(map_sqlite_err)?;
    let hashes: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .map_err(map_sqlite_err)?
        .collect::<rusqlite::Result<Vec<String>>>()
        .map_err(map_sqlite_err)?;
    drop(stmt);

    let sql =
        format!("SELECT {SELECT_COLUMNS} FROM photos WHERE content_hash = ?1 ORDER BY filename");
    let mut groups = Vec::with_capacity(hashes.len());
    for hash in hashes {
        let mut stmt = conn.prepare(&sql).map_err(map_sqlite_err)?;
        let rows = stmt
            .query_map(params![hash], row_to_raw)
            .map_err(map_sqlite_err)?;
        let mut group = Vec::new();
        for row in rows {
            group.push(raw_to_photo(row.map_err(map_sqlite_err)?)?);
        }
        groups.push(group);
    }
    Ok(groups)
}

/// Erstes Foto mit passendem `content_hash`, oder `None`. Das
/// Matching-Verfahren des Kollaborationsmodus (Phase 9 Schritt 10, siehe
/// `DECISIONS.md` ADR-0035 Punkt 4) — eine importierte `.apxs`-
/// Freigabedatei enthält keine Pixel-Bytes, nur den Hash, über den ein
/// gleiches Foto im lokalen Katalog wiedergefunden wird. Existieren mehrere
/// lokale Duplikate desselben Inhalts (siehe `list_duplicate_groups`
/// oben), gewinnt bewusst das erste nach Dateiname — derselbe
/// „keine feste Anspruchshaltung auf Eindeutigkeit"-Kompromiss wie dort.
pub(crate) fn find_by_content_hash(conn: &Connection, hash: &str) -> Result<Option<Photo>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM photos WHERE content_hash = ?1 ORDER BY filename LIMIT 1"
    );
    conn.query_row(&sql, params![hash], row_to_raw)
        .optional()
        .map_err(map_sqlite_err)?
        .map(raw_to_photo)
        .transpose()
}

/// Alle Fotos mit bekannten GPS-Koordinaten, ordnerübergreifend — für die
/// Kartenansicht (Phase 8 Schritt 7). Sortiert nach Aufnahmezeit (Fotos
/// ohne Zeitstempel zuletzt), damit eine Reiserouten-Ansicht direkt daraus
/// eine sinnvolle Reihenfolge ableiten kann, ohne selbst noch einmal zu
/// sortieren.
pub(crate) fn list_geotagged(conn: &Connection) -> Result<Vec<Photo>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM photos \
         WHERE gps_lat IS NOT NULL AND gps_lon IS NOT NULL \
         ORDER BY captured_at IS NULL, captured_at"
    );
    let mut stmt = conn.prepare(&sql).map_err(map_sqlite_err)?;
    let rows = stmt.query_map([], row_to_raw).map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_photo(row.map_err(map_sqlite_err)?)?);
    }
    Ok(result)
}

/// Setzt oder löscht (`None`) die GPS-Koordinaten eines Fotos von Hand —
/// z. B. wenn ein Foto auf der Kartenansicht platziert wird, weil es keine
/// EXIF-GPS-Daten trug.
pub(crate) fn set_gps(conn: &Connection, id: PhotoId, gps: Option<(f64, f64)>) -> Result<()> {
    let (lat, lon) = match gps {
        Some((lat, lon)) => (Some(lat), Some(lon)),
        None => (None, None),
    };
    let changed = conn
        .execute(
            "UPDATE photos SET gps_lat = ?2, gps_lon = ?3 WHERE id = ?1",
            params![id.to_string(), lat, lon],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Foto", id.to_string()));
    }
    Ok(())
}

/// Legt eine virtuelle Kopie von `source_id` an (Phase 9 Schritt 1, siehe
/// `migrations/0007_library_backlog.sql`s Moduldoku) — eine neue
/// `photos`-Zeile mit denselben Datei-/Metadaten-Feldern, aber eigener
/// ID, `rating`/`flag`/`color_label` vom Quellfoto übernommen (nicht
/// zurückgesetzt, damit sie als sinnvoller Ausgangspunkt zum Abweichen
/// dient) und `source_photo_id = Some(source_id)`. `edit_history` wird
/// hier bewusst NICHT kopiert — das übernimmt der Aufrufer (`apx-app`,
/// der bereits Zugriff auf `commit_edit` hat), damit dieses Modul nicht
/// von `repository::edits` abhängen muss.
pub(crate) fn create_virtual_copy(
    conn: &Connection,
    source_id: PhotoId,
    created_at: OffsetDateTime,
) -> Result<PhotoId> {
    let source = get(conn, source_id)?;
    if source.source_photo_id.is_some() {
        return Err(AppError::validation(
            "Von einer virtuellen Kopie kann keine weitere virtuelle Kopie angelegt werden"
                .to_string(),
        ));
    }
    let id = PhotoId::new();
    conn.execute(
        "INSERT INTO photos (
            id, folder_id, filename, file_size, file_mtime, content_hash, width, height,
            orientation, camera_make, camera_model, lens, iso, shutter, aperture, focal_length,
            captured_at, gps_lat, gps_lon, imported_at, missing, rating, flag, color_label,
            source_photo_id
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,0,?21,?22,?23,?24)",
        params![
            id.to_string(),
            source.folder_id.to_string(),
            source.filename,
            source.file_size as i64,
            to_unix(source.file_mtime),
            source.content_hash,
            source.width.map(|v| v as i64),
            source.height.map(|v| v as i64),
            source.orientation as i64,
            source.camera_make,
            source.camera_model,
            source.lens,
            source.iso.map(|v| v as i64),
            source.shutter.map(|v| v as f64),
            source.aperture.map(|v| v as f64),
            source.focal_length.map(|v| v as f64),
            to_unix_opt(source.captured_at),
            source.gps_lat,
            source.gps_lon,
            to_unix(created_at),
            source.rating as i64,
            source.flag as i64,
            source.color_label,
            source_id.to_string(),
        ],
    )
    .map_err(map_sqlite_err)?;
    Ok(id)
}

/// Alle virtuellen Kopien eines Quellfotos, nach Anlagezeit sortiert.
pub(crate) fn list_virtual_copies(conn: &Connection, source_id: PhotoId) -> Result<Vec<Photo>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM photos WHERE source_photo_id = ?1 ORDER BY imported_at"
    );
    let mut stmt = conn.prepare(&sql).map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map(params![source_id.to_string()], row_to_raw)
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_photo(row.map_err(map_sqlite_err)?)?);
    }
    Ok(result)
}

pub(crate) fn set_missing(conn: &Connection, id: PhotoId, missing: bool) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE photos SET missing = ?2 WHERE id = ?1",
            params![id.to_string(), missing as i64],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Foto", id.to_string()));
    }
    Ok(())
}

pub(crate) fn set_rating(conn: &Connection, id: PhotoId, rating: u8) -> Result<()> {
    if rating > 5 {
        return Err(AppError::validation(format!(
            "Bewertung muss zwischen 0 und 5 liegen, war {rating}"
        )));
    }
    let changed = conn
        .execute(
            "UPDATE photos SET rating = ?2 WHERE id = ?1",
            params![id.to_string(), rating as i64],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Foto", id.to_string()));
    }
    Ok(())
}

pub(crate) fn set_flag(conn: &Connection, id: PhotoId, flag: i8) -> Result<()> {
    if !(-1..=1).contains(&flag) {
        return Err(AppError::validation(format!(
            "Flagge muss -1 (Reject), 0 (keine) oder 1 (Pick) sein, war {flag}"
        )));
    }
    let changed = conn
        .execute(
            "UPDATE photos SET flag = ?2 WHERE id = ?1",
            params![id.to_string(), flag as i64],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Foto", id.to_string()));
    }
    Ok(())
}

/// Erweiterbare Farbmarkierungen (Phase 9 Schritt 1, siehe
/// `repository::color_labels`) — validiert gegen die dynamische
/// `color_label_definitions`-Tabelle statt einer fest verdrahteten
/// Palette (ersetzt die frühere `ALLOWED_COLOR_LABELS`-Konstante).
pub(crate) fn set_color_label(
    conn: &Connection,
    id: PhotoId,
    color_label: Option<&str>,
) -> Result<()> {
    if let Some(color) = color_label {
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM color_label_definitions WHERE name = ?1)",
                params![color],
                |row| row.get(0),
            )
            .map_err(map_sqlite_err)?;
        if !exists {
            return Err(AppError::validation(format!(
                "Unbekannte Farbmarkierung '{color}' — keine passende Zeile in color_label_definitions"
            )));
        }
    }
    let changed = conn
        .execute(
            "UPDATE photos SET color_label = ?2 WHERE id = ?1",
            params![id.to_string(), color_label],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Foto", id.to_string()));
    }
    Ok(())
}

/// Legt ein Foto an oder aktualisiert es, falls unter demselben
/// `(folder_id, filename)` bereits eines existiert (siehe `UNIQUE`-Constraint
/// in Migration 1). Gibt `(id, war_neu_oder_geändert)` zurück — Aufrufer
/// (der Import-Job) nutzt das für seine Fortschrittsstatistik.
///
/// - Existiert noch kein Eintrag: neue Zeile, `changed = true`.
/// - Existiert einer mit identischer Größe/Änderungszeit: unverändert,
///   `changed = false` — das ist der Fall "Datei schon importiert".
/// - Existiert einer, aber Größe/Änderungszeit weichen ab (Datei wurde
///   außerhalb von Aperture X ersetzt): Zeile wird aktualisiert, ID bleibt
///   erhalten, `changed = true`.
pub(crate) fn upsert(
    conn: &Connection,
    new_photo: &NewPhoto,
    imported_at: OffsetDateTime,
) -> Result<(PhotoId, bool)> {
    if let Some(existing) =
        find_by_folder_and_filename(conn, new_photo.folder_id, &new_photo.filename)?
    {
        let unchanged = existing.file_size == new_photo.file_size
            && existing.file_mtime == new_photo.file_mtime;
        if unchanged {
            return Ok((existing.id, false));
        }
        update_row(conn, existing.id, new_photo)?;
        return Ok((existing.id, true));
    }

    let id = PhotoId::new();
    insert_row(conn, id, new_photo, imported_at)?;
    Ok((id, true))
}

fn insert_row(
    conn: &Connection,
    id: PhotoId,
    p: &NewPhoto,
    imported_at: OffsetDateTime,
) -> Result<()> {
    conn.execute(
        "INSERT INTO photos (
            id, folder_id, filename, file_size, file_mtime, content_hash, width, height,
            orientation, camera_make, camera_model, lens, iso, shutter, aperture, focal_length,
            captured_at, gps_lat, gps_lon, imported_at, missing
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,0)",
        params![
            id.to_string(),
            p.folder_id.to_string(),
            p.filename,
            p.file_size as i64,
            to_unix(p.file_mtime),
            p.content_hash,
            p.width.map(|v| v as i64),
            p.height.map(|v| v as i64),
            p.orientation as i64,
            p.camera_make,
            p.camera_model,
            p.lens,
            p.iso.map(|v| v as i64),
            p.shutter.map(|v| v as f64),
            p.aperture.map(|v| v as f64),
            p.focal_length.map(|v| v as f64),
            to_unix_opt(p.captured_at),
            p.gps_lat,
            p.gps_lon,
            to_unix(imported_at),
        ],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
}

fn update_row(conn: &Connection, id: PhotoId, p: &NewPhoto) -> Result<()> {
    conn.execute(
        "UPDATE photos SET
            file_size = ?2, file_mtime = ?3, content_hash = ?4, width = ?5, height = ?6,
            orientation = ?7, camera_make = ?8, camera_model = ?9, lens = ?10, iso = ?11,
            shutter = ?12, aperture = ?13, focal_length = ?14, captured_at = ?15,
            gps_lat = ?16, gps_lon = ?17, missing = 0
         WHERE id = ?1",
        params![
            id.to_string(),
            p.file_size as i64,
            to_unix(p.file_mtime),
            p.content_hash,
            p.width.map(|v| v as i64),
            p.height.map(|v| v as i64),
            p.orientation as i64,
            p.camera_make,
            p.camera_model,
            p.lens,
            p.iso.map(|v| v as i64),
            p.shutter.map(|v| v as f64),
            p.aperture.map(|v| v as f64),
            p.focal_length.map(|v| v as f64),
            to_unix_opt(p.captured_at),
            p.gps_lat,
            p.gps_lon,
        ],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;
    use crate::repository::folders;
    use std::path::Path;

    fn setup() -> (Connection, FolderId) {
        let conn = Connection::open_in_memory().expect("In-Memory-DB");
        migrations::apply(&conn).expect("Migration");
        let folder_id =
            folders::insert(&conn, Path::new("/fotos"), None, OffsetDateTime::now_utc())
                .expect("Ordner");
        (conn, folder_id)
    }

    fn sample_photo(folder_id: FolderId, size: u64, mtime: OffsetDateTime) -> NewPhoto {
        NewPhoto {
            folder_id,
            filename: "IMG_0001.CR2".to_string(),
            file_size: size,
            file_mtime: mtime,
            content_hash: Some("abc123".to_string()),
            width: Some(6000),
            height: Some(4000),
            orientation: 1,
            camera_make: Some("Canon".to_string()),
            camera_model: Some("EOS R5".to_string()),
            lens: Some("RF 24-70mm".to_string()),
            iso: Some(400),
            shutter: Some(0.008),
            aperture: Some(2.8),
            focal_length: Some(50.0),
            captured_at: Some(
                OffsetDateTime::now_utc()
                    .replace_nanosecond(0)
                    .expect("gültig"),
            ),
            gps_lat: Some(52.5),
            gps_lon: Some(13.4),
        }
    }

    #[test]
    fn upsert_inserts_new_photo() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        let (id, changed) = upsert(
            &conn,
            &sample_photo(folder_id, 1000, mtime),
            OffsetDateTime::now_utc(),
        )
        .expect("ok");
        assert!(changed);

        let fetched = get(&conn, id).expect("sollte gefunden werden");
        assert_eq!(fetched.filename, "IMG_0001.CR2");
        assert_eq!(fetched.file_size, 1000);
        assert!(!fetched.missing);
    }

    #[test]
    fn second_import_of_unchanged_photo_is_not_a_duplicate() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        let photo = sample_photo(folder_id, 1000, mtime);

        let (first_id, first_changed) =
            upsert(&conn, &photo, OffsetDateTime::now_utc()).expect("ok");
        let (second_id, second_changed) =
            upsert(&conn, &photo, OffsetDateTime::now_utc()).expect("ok");

        assert!(first_changed);
        assert!(
            !second_changed,
            "unveränderte Datei darf beim zweiten Import nicht als Änderung zählen"
        );
        assert_eq!(first_id, second_id);
        assert_eq!(count_by_folder(&conn, folder_id).expect("ok"), 1);
    }

    #[test]
    fn changed_file_updates_existing_row_instead_of_duplicating() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        let (first_id, _) = upsert(
            &conn,
            &sample_photo(folder_id, 1000, mtime),
            OffsetDateTime::now_utc(),
        )
        .expect("ok");

        let later_mtime = mtime + time::Duration::seconds(60);
        let (second_id, changed) = upsert(
            &conn,
            &sample_photo(folder_id, 2000, later_mtime),
            OffsetDateTime::now_utc(),
        )
        .expect("ok");

        assert!(changed);
        assert_eq!(
            first_id, second_id,
            "ID muss stabil bleiben, keine neue Zeile"
        );
        assert_eq!(count_by_folder(&conn, folder_id).expect("ok"), 1);
        assert_eq!(get(&conn, first_id).expect("ok").file_size, 2000);
    }

    #[test]
    fn set_missing_marks_photo_and_unknown_id_fails() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        let (id, _) = upsert(
            &conn,
            &sample_photo(folder_id, 1000, mtime),
            OffsetDateTime::now_utc(),
        )
        .expect("ok");

        set_missing(&conn, id, true).expect("ok");
        assert!(get(&conn, id).expect("ok").missing);

        assert!(set_missing(&conn, PhotoId::new(), true).is_err());
    }

    #[test]
    fn set_rating_updates_and_rejects_out_of_range() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        let (id, _) = upsert(
            &conn,
            &sample_photo(folder_id, 1000, mtime),
            OffsetDateTime::now_utc(),
        )
        .expect("ok");

        assert_eq!(get(&conn, id).expect("ok").rating, 0, "Default ist 0");
        set_rating(&conn, id, 5).expect("ok");
        assert_eq!(get(&conn, id).expect("ok").rating, 5);

        assert!(
            set_rating(&conn, id, 6).is_err(),
            "Bewertung über 5 muss abgelehnt werden"
        );
        assert!(set_rating(&conn, PhotoId::new(), 3).is_err());
    }

    #[test]
    fn set_flag_updates_and_rejects_out_of_range() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        let (id, _) = upsert(
            &conn,
            &sample_photo(folder_id, 1000, mtime),
            OffsetDateTime::now_utc(),
        )
        .expect("ok");

        set_flag(&conn, id, 1).expect("Pick sollte akzeptiert werden");
        assert_eq!(get(&conn, id).expect("ok").flag, 1);
        set_flag(&conn, id, -1).expect("Reject sollte akzeptiert werden");
        assert_eq!(get(&conn, id).expect("ok").flag, -1);

        assert!(
            set_flag(&conn, id, 2).is_err(),
            "Flagge außerhalb von -1..=1 muss abgelehnt werden"
        );
    }

    #[test]
    fn set_color_label_updates_clears_and_rejects_unknown_color() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        let (id, _) = upsert(
            &conn,
            &sample_photo(folder_id, 1000, mtime),
            OffsetDateTime::now_utc(),
        )
        .expect("ok");

        set_color_label(&conn, id, Some("red")).expect("ok");
        assert_eq!(
            get(&conn, id).expect("ok").color_label.as_deref(),
            Some("red")
        );

        set_color_label(&conn, id, None).expect("Zurücksetzen sollte funktionieren");
        assert_eq!(get(&conn, id).expect("ok").color_label, None);

        assert!(
            set_color_label(&conn, id, Some("orange")).is_err(),
            "unbekannte Farbe muss abgelehnt werden"
        );
    }

    #[test]
    fn list_duplicate_groups_finds_matching_hashes_and_ignores_the_rest() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");

        let mut original = sample_photo(folder_id, 1000, mtime);
        original.content_hash = Some("gleicherhash".to_string());
        original.filename = "original.cr2".to_string();
        upsert(&conn, &original, OffsetDateTime::now_utc()).expect("ok");

        let mut kopie = sample_photo(folder_id, 1000, mtime);
        kopie.content_hash = Some("gleicherhash".to_string());
        kopie.filename = "kopie.cr2".to_string();
        upsert(&conn, &kopie, OffsetDateTime::now_utc()).expect("ok");

        let mut einzelstueck = sample_photo(folder_id, 1000, mtime);
        einzelstueck.content_hash = Some("anderer-hash".to_string());
        einzelstueck.filename = "einzelstueck.cr2".to_string();
        upsert(&conn, &einzelstueck, OffsetDateTime::now_utc()).expect("ok");

        let mut ohne_hash = sample_photo(folder_id, 1000, mtime);
        ohne_hash.content_hash = None;
        ohne_hash.filename = "ohne_hash.cr2".to_string();
        upsert(&conn, &ohne_hash, OffsetDateTime::now_utc()).expect("ok");

        let groups = list_duplicate_groups(&conn).expect("ok");
        assert_eq!(groups.len(), 1, "genau eine Duplikatgruppe");
        assert_eq!(groups[0].len(), 2);
        let filenames: Vec<&str> = groups[0].iter().map(|p| p.filename.as_str()).collect();
        assert_eq!(
            filenames,
            vec!["kopie.cr2", "original.cr2"],
            "Gruppe ist nach Dateiname sortiert"
        );
    }

    #[test]
    fn find_by_content_hash_returns_the_first_match_by_filename_or_none() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");

        let mut zebra = sample_photo(folder_id, 1000, mtime);
        zebra.content_hash = Some("gleicherhash".to_string());
        zebra.filename = "zebra.cr2".to_string();
        upsert(&conn, &zebra, OffsetDateTime::now_utc()).expect("ok");

        let mut apfel = sample_photo(folder_id, 1000, mtime);
        apfel.content_hash = Some("gleicherhash".to_string());
        apfel.filename = "apfel.cr2".to_string();
        upsert(&conn, &apfel, OffsetDateTime::now_utc()).expect("ok");

        let found = find_by_content_hash(&conn, "gleicherhash")
            .expect("ok")
            .expect("sollte ein Foto finden");
        assert_eq!(found.filename, "apfel.cr2", "erstes nach Dateiname gewinnt");

        assert!(find_by_content_hash(&conn, "unbekannt")
            .expect("ok")
            .is_none());
    }

    #[test]
    fn deleting_folder_cascades_to_photos() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        upsert(
            &conn,
            &sample_photo(folder_id, 1000, mtime),
            OffsetDateTime::now_utc(),
        )
        .expect("ok");

        conn.execute(
            "DELETE FROM folders WHERE id = ?1",
            params![folder_id.to_string()],
        )
        .expect("Delete darf nicht scheitern");

        assert_eq!(count_by_folder(&conn, folder_id).expect("ok"), 0);
    }

    #[test]
    fn list_geotagged_only_returns_photos_with_gps() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");

        let mut with_gps = sample_photo(folder_id, 1000, mtime);
        with_gps.filename = "IMG_0001.CR2".to_string();
        upsert(&conn, &with_gps, OffsetDateTime::now_utc()).expect("ok");

        let mut without_gps = sample_photo(folder_id, 2000, mtime);
        without_gps.filename = "IMG_0002.CR2".to_string();
        without_gps.gps_lat = None;
        without_gps.gps_lon = None;
        upsert(&conn, &without_gps, OffsetDateTime::now_utc()).expect("ok");

        let geotagged = list_geotagged(&conn).expect("ok");
        assert_eq!(geotagged.len(), 1);
        assert_eq!(geotagged[0].filename, "IMG_0001.CR2");
    }

    #[test]
    fn set_gps_updates_and_clears_coordinates() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        let mut photo = sample_photo(folder_id, 1000, mtime);
        photo.gps_lat = None;
        photo.gps_lon = None;
        let (id, _) = upsert(&conn, &photo, OffsetDateTime::now_utc()).expect("ok");

        assert!(get(&conn, id).expect("ok").gps_lat.is_none());
        set_gps(&conn, id, Some((48.1, 11.6))).expect("ok");
        let fetched = get(&conn, id).expect("ok");
        assert_eq!(fetched.gps_lat, Some(48.1));
        assert_eq!(fetched.gps_lon, Some(11.6));

        set_gps(&conn, id, None).expect("ok");
        assert!(get(&conn, id).expect("ok").gps_lat.is_none());

        assert!(set_gps(&conn, PhotoId::new(), Some((0.0, 0.0))).is_err());
    }

    #[test]
    fn create_virtual_copy_shares_file_but_has_independent_metadata() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        let (source_id, _) = upsert(
            &conn,
            &sample_photo(folder_id, 1000, mtime),
            OffsetDateTime::now_utc(),
        )
        .expect("ok");
        set_rating(&conn, source_id, 3).expect("ok");

        let copy_id =
            create_virtual_copy(&conn, source_id, OffsetDateTime::now_utc()).expect("anlegen");
        assert_ne!(copy_id, source_id);

        let copy = get(&conn, copy_id).expect("ok");
        assert_eq!(copy.source_photo_id, Some(source_id));
        assert_eq!(copy.filename, "IMG_0001.CR2");
        assert_eq!(
            copy.rating, 3,
            "startet mit dem Bewertungsstand des Quellfotos"
        );

        // Unabhängig danach: die virtuelle Kopie ändern beeinflusst das
        // Original nicht.
        set_rating(&conn, copy_id, 5).expect("ok");
        assert_eq!(get(&conn, source_id).expect("ok").rating, 3);
        assert_eq!(get(&conn, copy_id).expect("ok").rating, 5);

        assert_eq!(
            list_virtual_copies(&conn, source_id).expect("liste").len(),
            1
        );
    }

    #[test]
    fn virtual_copy_shares_folder_and_filename_without_violating_uniqueness() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        let (source_id, _) = upsert(
            &conn,
            &sample_photo(folder_id, 1000, mtime),
            OffsetDateTime::now_utc(),
        )
        .expect("ok");
        // Sollte keinen UNIQUE-Konflikt auslösen — die partielle
        // Unique-Index gilt nur für source_photo_id IS NULL.
        create_virtual_copy(&conn, source_id, OffsetDateTime::now_utc()).expect("anlegen");
        create_virtual_copy(&conn, source_id, OffsetDateTime::now_utc())
            .expect("zweite Kopie anlegen");
        assert_eq!(
            list_virtual_copies(&conn, source_id).expect("liste").len(),
            2
        );
    }

    #[test]
    fn create_virtual_copy_of_a_virtual_copy_is_rejected() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        let (source_id, _) = upsert(
            &conn,
            &sample_photo(folder_id, 1000, mtime),
            OffsetDateTime::now_utc(),
        )
        .expect("ok");
        let copy_id =
            create_virtual_copy(&conn, source_id, OffsetDateTime::now_utc()).expect("anlegen");
        assert!(create_virtual_copy(&conn, copy_id, OffsetDateTime::now_utc()).is_err());
    }

    #[test]
    fn re_importing_the_same_file_never_matches_a_virtual_copy() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        let photo = sample_photo(folder_id, 1000, mtime);
        let (source_id, _) = upsert(&conn, &photo, OffsetDateTime::now_utc()).expect("ok");
        create_virtual_copy(&conn, source_id, OffsetDateTime::now_utc()).expect("anlegen");

        // Ein zweiter Import derselben Datei muss weiterhin das
        // Quellfoto treffen, nicht die virtuelle Kopie.
        let (matched_id, changed) = upsert(&conn, &photo, OffsetDateTime::now_utc()).expect("ok");
        assert_eq!(matched_id, source_id);
        assert!(!changed);
    }
}
