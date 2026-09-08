use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub source_url: Option<String>,
    pub source_kind: String,
    pub source_path: Option<String>,
    pub vocals_path: Option<String>,
    pub instrumental_path: Option<String>,
    pub cover_path: Option<String>,
    pub duration_ms: Option<i64>,
    pub sample_rate: Option<i64>,
    pub status: String,
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub peaks: Option<Vec<f32>>,
    #[serde(default)]
    pub playlist_id: Option<String>,
    #[serde(default)]
    pub playlist_index: Option<i64>,
    #[serde(default)]
    pub ytdlp_query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub source_url: Option<String>,
    pub source_kind: String,
    pub cover_path: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub track_count: i64,
    pub ready_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySnapshot {
    pub tracks: Vec<Track>,
    pub playlists: Vec<Playlist>,
}

pub fn open(path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;
        CREATE TABLE IF NOT EXISTS tracks (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            artist TEXT NOT NULL,
            source_url TEXT,
            source_kind TEXT NOT NULL,
            source_path TEXT,
            vocals_path TEXT,
            instrumental_path TEXT,
            cover_path TEXT,
            duration_ms INTEGER,
            sample_rate INTEGER,
            status TEXT NOT NULL,
            error TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS playlists (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            artist TEXT NOT NULL,
            source_url TEXT,
            source_kind TEXT NOT NULL,
            cover_path TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        ",
    )?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> AppResult<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(tracks)")?;
    let names: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|n| n.ok())
        .collect();
    if !names.iter().any(|n| n == "playlist_id") {
        conn.execute("ALTER TABLE tracks ADD COLUMN playlist_id TEXT", [])?;
    }
    if !names.iter().any(|n| n == "playlist_index") {
        conn.execute("ALTER TABLE tracks ADD COLUMN playlist_index INTEGER", [])?;
    }
    if !names.iter().any(|n| n == "ytdlp_query") {
        conn.execute("ALTER TABLE tracks ADD COLUMN ytdlp_query TEXT", [])?;
    }
    Ok(())
}

pub fn get_setting(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
    let value = stmt
        .query_row(params![key], |row| row.get::<_, String>(0))
        .optional()?;
    Ok(value)
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

fn attach_peaks(track: &mut Track) {
    if let Some(vocals) = track.vocals_path.as_ref() {
        if let Some(dir) = std::path::Path::new(vocals).parent() {
            track.peaks = crate::dsp::wav::read_peaks(&dir.join("peaks.json"));
        }
    }
}

pub fn upsert_track(conn: &Connection, track: &Track) -> AppResult<()> {
    conn.execute(
        "
        INSERT INTO tracks (
            id, title, artist, source_url, source_kind, source_path,
            vocals_path, instrumental_path, cover_path, duration_ms,
            sample_rate, status, error, created_at, updated_at,
            playlist_id, playlist_index, ytdlp_query
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
        ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            artist = excluded.artist,
            source_url = excluded.source_url,
            source_kind = excluded.source_kind,
            source_path = excluded.source_path,
            vocals_path = excluded.vocals_path,
            instrumental_path = excluded.instrumental_path,
            cover_path = excluded.cover_path,
            duration_ms = excluded.duration_ms,
            sample_rate = excluded.sample_rate,
            status = excluded.status,
            error = excluded.error,
            updated_at = excluded.updated_at,
            playlist_id = excluded.playlist_id,
            playlist_index = excluded.playlist_index,
            ytdlp_query = excluded.ytdlp_query
        ",
        params![
            track.id,
            track.title,
            track.artist,
            track.source_url,
            track.source_kind,
            track.source_path,
            track.vocals_path,
            track.instrumental_path,
            track.cover_path,
            track.duration_ms,
            track.sample_rate,
            track.status,
            track.error,
            track.created_at,
            track.updated_at,
            track.playlist_id,
            track.playlist_index,
            track.ytdlp_query,
        ],
    )?;
    let _ = track.peaks;
    Ok(())
}

pub fn upsert_playlist(conn: &Connection, playlist: &Playlist) -> AppResult<()> {
    conn.execute(
        "
        INSERT INTO playlists (
            id, title, artist, source_url, source_kind, cover_path, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            artist = excluded.artist,
            source_url = excluded.source_url,
            source_kind = excluded.source_kind,
            cover_path = excluded.cover_path,
            updated_at = excluded.updated_at
        ",
        params![
            playlist.id,
            playlist.title,
            playlist.artist,
            playlist.source_url,
            playlist.source_kind,
            playlist.cover_path,
            playlist.created_at,
            playlist.updated_at,
        ],
    )?;
    Ok(())
}

pub fn get_track(conn: &Connection, id: &str) -> AppResult<Option<Track>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, title, artist, source_url, source_kind, source_path,
               vocals_path, instrumental_path, cover_path, duration_ms,
               sample_rate, status, error, created_at, updated_at,
               playlist_id, playlist_index, ytdlp_query
        FROM tracks WHERE id = ?1
        ",
    )?;
    let track = stmt
        .query_row(params![id], |row| row_to_track(row))
        .optional()?;
    if let Some(mut track) = track {
        attach_peaks(&mut track);
        return Ok(Some(track));
    }
    Ok(None)
}

pub fn list_tracks(conn: &Connection) -> AppResult<Vec<Track>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, title, artist, source_url, source_kind, source_path,
               vocals_path, instrumental_path, cover_path, duration_ms,
               sample_rate, status, error, created_at, updated_at,
               playlist_id, playlist_index, ytdlp_query
        FROM tracks
        ORDER BY created_at DESC, playlist_index ASC
        ",
    )?;
    let rows = stmt.query_map([], |row| row_to_track(row))?;
    let mut tracks = Vec::new();
    for row in rows {
        let mut track = row?;
        attach_peaks(&mut track);
        tracks.push(track);
    }
    Ok(tracks)
}

pub fn list_playlists(conn: &Connection) -> AppResult<Vec<Playlist>> {
    let mut stmt = conn.prepare(
        "
        SELECT p.id, p.title, p.artist, p.source_url, p.source_kind, p.cover_path,
               p.created_at, p.updated_at,
               (SELECT COUNT(*) FROM tracks t WHERE t.playlist_id = p.id),
               (SELECT COUNT(*) FROM tracks t WHERE t.playlist_id = p.id AND t.status = 'ready')
        FROM playlists p
        ORDER BY p.created_at DESC
        ",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Playlist {
            id: row.get(0)?,
            title: row.get(1)?,
            artist: row.get(2)?,
            source_url: row.get(3)?,
            source_kind: row.get(4)?,
            cover_path: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
            track_count: row.get(8)?,
            ready_count: row.get(9)?,
        })
    })?;
    let mut playlists = Vec::new();
    for row in rows {
        playlists.push(row?);
    }
    Ok(playlists)
}

pub fn snapshot(conn: &Connection) -> AppResult<LibrarySnapshot> {
    Ok(LibrarySnapshot {
        tracks: list_tracks(conn)?,
        playlists: list_playlists(conn)?,
    })
}

pub fn playlist_track_ids(conn: &Connection, playlist_id: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id FROM tracks WHERE playlist_id = ?1")?;
    let rows = stmt.query_map(params![playlist_id], |row| row.get::<_, String>(0))?;
    let mut ids = Vec::new();
    for row in rows {
        ids.push(row?);
    }
    Ok(ids)
}

pub fn delete_track(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM tracks WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn delete_playlist(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM tracks WHERE playlist_id = ?1", params![id])?;
    conn.execute("DELETE FROM playlists WHERE id = ?1", params![id])?;
    Ok(())
}

fn row_to_track(row: &rusqlite::Row) -> rusqlite::Result<Track> {
    Ok(Track {
        id: row.get(0)?,
        title: row.get(1)?,
        artist: row.get(2)?,
        source_url: row.get(3)?,
        source_kind: row.get(4)?,
        source_path: row.get(5)?,
        vocals_path: row.get(6)?,
        instrumental_path: row.get(7)?,
        cover_path: row.get(8)?,
        duration_ms: row.get(9)?,
        sample_rate: row.get(10)?,
        status: row.get(11)?,
        error: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
        peaks: None,
        playlist_id: row.get(15)?,
        playlist_index: row.get(16)?,
        ytdlp_query: row.get(17)?,
    })
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
