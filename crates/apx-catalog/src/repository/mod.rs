//! Repository-Module: ein Modul pro Tabelle, reine Funktionen über
//! `&rusqlite::Connection` (funktioniert dank `Deref` auch mit
//! `&rusqlite::Transaction`). Nur [`crate::Catalog`] ruft diese Module auf.

pub(crate) mod collections;
pub(crate) mod edits;
pub(crate) mod folders;
pub(crate) mod keywords;
pub(crate) mod photos;
pub(crate) mod presets;
pub(crate) mod previews;
pub(crate) mod search;
