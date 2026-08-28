//! Repository-Module: ein Modul pro Tabelle, reine Funktionen über
//! `&rusqlite::Connection` (funktioniert dank `Deref` auch mit
//! `&rusqlite::Transaction`). Nur [`crate::Catalog`] ruft diese Module auf.

pub(crate) mod folders;
pub(crate) mod photos;
pub(crate) mod previews;
