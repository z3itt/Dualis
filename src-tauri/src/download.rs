use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;
use crate::tools::{self, http_client};
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub const SETTING_COOKIES_BROWSER: &str = "cookies_browser";
pub const SETTING_COOKIES_FILE: &str = "cookies_file";
pub const SETTING_DOWNLOAD_FORMAT: &str = "download_format";

pub const DEFAULT_DOWNLOAD_FORMAT: &str = "auto";

/// Matches Discord bot `/home/zeit/music.py` `ytdl_format_options`.
const BOT_YT_FORMAT: &str = "bestaudio[ext=m4a]/bestaudio/best";
const BOT_YT_CLIENT: &str = "youtube:player_client=mweb,ios,android";

#[derive(Debug, Clone)]
pub struct ResolvedItem {
    pub title: String,
    pub artist: String,
    pub source_url: String,
    pub source_kind: String,
    pub ytdlp_query: String,
}

#[derive(Debug, Clone)]
pub struct PlaylistMeta {
    pub title: String,
    pub artist: String,
    pub source_url: String,
    pub source_kind: String,
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedBatch {
    pub playlist: Option<PlaylistMeta>,
    pub items: Vec<ResolvedItem>,
}

/// Browser name or cookies file path for YouTube auth.
pub struct YtAuth<'a> {
    pub browser: Option<&'a str>,
    pub cookies_file: Option<&'a str>,
}

/// Attach YouTube auth: cookies.txt takes priority over browser cookies.
pub fn apply_ytdlp_cookies(cmd: &mut Command, auth: &YtAuth<'_>) {
    if let Some(file) = auth
        .cookies_file
        .map(str::trim)
        .filter(|f| !f.is_empty() && Path::new(f).is_file())
    {
        cmd.arg("--cookies").arg(file);
        return;
    }
    if let Some(b) = auth
        .browser
        .map(str::trim)
        .filter(|b| !b.is_empty() && !eq_ignore_case(b, "none"))
    {
        cmd.arg("--cookies-from-browser").arg(b);
    }
}

/// Cookies plus the bot player-client set. Used for playlist expansion.
pub fn apply_ytdlp_auth(cmd: &mut Command, auth: &YtAuth<'_>) {
    apply_ytdlp_cookies(cmd, auth);
    apply_ytdlp_client(cmd, Some(BOT_YT_CLIENT));
}

fn apply_ytdlp_client(cmd: &mut Command, client_args: Option<&str>) {
    if let Some(args) = client_args.filter(|s| !s.is_empty()) {
        cmd.arg("--extractor-args").arg(args);
    }
}

fn resolve_ffmpeg() -> Option<PathBuf> {
    resolve_tool("ffmpeg", &["/usr/bin/ffmpeg", "/usr/local/bin/ffmpeg"])
}

fn resolve_node() -> Option<PathBuf> {
    resolve_tool("node", &["/usr/bin/node", "/usr/local/bin/node"]).or_else(|| {
        std::env::var("HOME").ok().and_then(|home| {
            let candidates = [
                format!("{home}/.local/share/fnm/current/bin/node"),
                format!("{home}/.fnm/current/bin/node"),
                format!("{home}/.nvm/current/bin/node"),
            ];
            candidates
                .into_iter()
                .map(PathBuf::from)
                .find(|p| p.is_file())
        })
    })
}

fn resolve_tool(name: &str, fallbacks: &[&str]) -> Option<PathBuf> {
    which::which(name).ok().or_else(|| {
        fallbacks
            .iter()
            .map(PathBuf::from)
            .find(|p| p.is_file())
    })
}

fn extended_path() -> String {
    let mut parts: Vec<String> = std::env::var("PATH")
        .ok()
        .map(|p| p.split(':').map(String::from).collect())
        .unwrap_or_default();
    let mut extras = vec![
        "/usr/bin".to_string(),
        "/usr/local/bin".to_string(),
        "/bin".to_string(),
    ];
    if let Ok(home) = std::env::var("HOME") {
        extras.push(format!("{home}/.local/share/fnm/current/bin"));
        extras.push(format!("{home}/.cargo/bin"));
    }
    for extra in extras {
        if !parts.iter().any(|p| p == &extra) {
            parts.push(extra);
        }
    }
    parts.join(":")
}

/// Flags shared by every yt-dlp invocation (JS runtime, ffmpeg, PATH).
pub fn apply_ytdlp_runtime(cmd: &mut Command) {
    cmd.arg("--remote-components").arg("ejs:github");
    if let Some(node) = resolve_node() {
        cmd.arg("--js-runtimes").arg(format!("node:{}", node.display()));
    }
    if let Some(ffmpeg) = resolve_ffmpeg() {
        cmd.arg("--ffmpeg-location").arg(ffmpeg);
    }
    cmd.arg("--no-check-certificate");
    cmd.arg("--source-address").arg("0.0.0.0");
    cmd.env("PATH", extended_path());
}

fn ffmpeg_available() -> bool {
    resolve_ffmpeg().is_some()
}

#[derive(Clone)]
pub(crate) struct DownloadStrategy {
    label: String,
    format: Option<&'static str>,
    client_args: Option<&'static str>,
    extract_audio: bool,
}

#[allow(dead_code)]
fn base_formats(preset: &str) -> Vec<&'static str> {
    match preset.trim().to_lowercase().as_str() {
        "best" => vec!["ba/b", "bestaudio/best", "b"],
        "any" => vec!["b", "ba/b", "worst"],
        "fast" => vec!["worstaudio/worst", "worst", "ba/b"],
        _ => vec!["ba/b", "bestaudio/best", "b", "worst"],
    }
}

fn preset_formats(preset: &str) -> Vec<(&'static str, bool)> {
    match preset.trim().to_lowercase().as_str() {
        "fast" => vec![("worstaudio/worst", false), ("worst", true)],
        "any" => vec![("best", true), ("bestaudio/best", false), ("ba/b", false)],
        "best" => vec![("bestaudio/best", true), ("bestaudio/best", false), ("ba/b", false)],
        _ => vec![
            ("bestaudio/best", true),
            ("bestaudio/best", false),
            ("ba/b", false),
            ("b", true),
        ],
    }
}

/// Discord-bot style first (`music.py`), then limited client fallbacks.
pub fn download_strategies(preset: &str) -> Vec<DownloadStrategy> {
    let mut out = Vec::new();
    let preset = preset.trim().to_lowercase();

    out.push(DownloadStrategy {
        label: "bot · m4a".into(),
        format: Some(BOT_YT_FORMAT),
        client_args: Some(BOT_YT_CLIENT),
        extract_audio: false,
    });
    if ffmpeg_available() {
        out.push(DownloadStrategy {
            label: "bot · m4a · ffmpeg".into(),
            format: Some(BOT_YT_FORMAT),
            client_args: Some(BOT_YT_CLIENT),
            extract_audio: true,
        });
    }

    for (format, extract) in preset_formats(&preset) {
        out.push(DownloadStrategy {
            label: format!(
                "bot · {format}{}",
                if extract { " · ffmpeg" } else { "" }
            ),
            format: Some(format),
            client_args: Some(BOT_YT_CLIENT),
            extract_audio: extract,
        });
    }

    for (format, extract) in preset_formats(&preset) {
        out.push(DownloadStrategy {
            label: format!(
                "standard · {format}{}",
                if extract { " · ffmpeg" } else { "" }
            ),
            format: Some(format),
            client_args: None,
            extract_audio: extract,
        });
    }
    out.push(DownloadStrategy {
        label: "standard · default".into(),
        format: None,
        client_args: None,
        extract_audio: false,
    });

    for (label, client) in [
        ("web", "youtube:player_client=web"),
        ("android", "youtube:player_client=android"),
        ("ios", "youtube:player_client=ios"),
    ] {
        out.push(DownloadStrategy {
            label: format!("{label} · bestaudio/best · ffmpeg"),
            format: Some("bestaudio/best"),
            client_args: Some(client),
            extract_audio: true,
        });
    }

    out
}

pub fn friendly_download_error(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("sign in to confirm") || lower.contains("not a bot") {
        return "YouTube blocked this download. In ⋮ Settings: pick the browser where you are logged into YouTube, or import a cookies.txt file (same as most Discord bots use).".into();
    }
    if lower.contains("cookies") && lower.contains("browser") {
        return "Could not read browser cookies. Try another browser in Settings, or sign into YouTube in that browser first.".into();
    }
    if lower.contains("requested format is not available") || lower.contains("format is not available") {
        return "Could not find a downloadable audio stream for this video. Make sure YouTube login is set in Settings, then retry. If it still fails, the video may be region-locked or age-restricted.".into();
    }
    format!("Download failed: {raw}")
}

fn eq_ignore_case(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

#[allow(dead_code)]
pub fn format_attempts(preset: &str) -> Vec<&'static str> {
    base_formats(preset)
}

fn is_format_error(raw: &str) -> bool {
    let lower = raw.to_lowercase();
    lower.contains("format is not available")
        || lower.contains("requested format")
        || lower.contains("no video formats")
        || lower.contains("no formats found")
        || lower.contains("only images are available")
        || lower.contains("nesting violation")
}

fn is_retryable_download_error(raw: &str) -> bool {
    let lower = raw.to_lowercase();
    is_format_error(raw)
        || lower.contains("unable to extract")
        || lower.contains("failed to extract")
        || lower.contains("javascript runtime")
        || lower.contains("sign in to confirm")
        || lower.contains("not a bot")
}

pub async fn expand_inputs(
    paths: &AppPaths,
    inputs: &[String],
    auth: &YtAuth<'_>,
) -> AppResult<Vec<ResolvedBatch>> {
    let mut batches = Vec::new();
    for raw in inputs {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let batch = expand_one(paths, trimmed, auth).await?;
        if !batch.items.is_empty() {
            batches.push(batch);
        }
    }
    if batches.is_empty() {
        return Err(AppError::msg("No valid links or queries were provided"));
    }
    Ok(batches)
}

async fn expand_one(paths: &AppPaths, input: &str, auth: &YtAuth<'_>) -> AppResult<ResolvedBatch> {
    if is_spotify_url(input) {
        return expand_spotify(input).await;
    }
    if looks_like_url(input) {
        if is_youtube_playlist_url(input) {
            return expand_youtube_playlist(paths, input, auth).await;
        }
        return Ok(ResolvedBatch {
            playlist: None,
            items: vec![ResolvedItem {
                title: String::from("Resolving…"),
                artist: String::from("YouTube"),
                source_url: input.to_string(),
                source_kind: kind_from_url(input),
                ytdlp_query: input.to_string(),
            }],
        });
    }
    Ok(ResolvedBatch {
        playlist: None,
        items: vec![ResolvedItem {
            title: input.to_string(),
            artist: String::from("Search"),
            source_url: input.to_string(),
            source_kind: String::from("search"),
            ytdlp_query: format!("ytsearch1:{input}"),
        }],
    })
}

pub fn is_spotify_url(input: &str) -> bool {
    input.contains("open.spotify.com") || input.starts_with("spotify:")
}

fn looks_like_url(input: &str) -> bool {
    input.starts_with("http://") || input.starts_with("https://")
}

fn kind_from_url(input: &str) -> String {
    if input.contains("music.youtube.com") {
        "youtube-music".into()
    } else {
        "youtube".into()
    }
}

pub fn is_youtube_playlist_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    let has_video = lower.contains("/watch") && (lower.contains("v=") || lower.contains("/watch/"));
    if has_video {
        return false;
    }
    lower.contains("/playlist")
        || lower.contains("/browse/vl")
        || lower.contains("list=")
        || lower.contains("/album/")
}

async fn expand_youtube_playlist(
    paths: &AppPaths,
    url: &str,
    auth: &YtAuth<'_>,
) -> AppResult<ResolvedBatch> {
    let ytdlp = tools::ensure_ytdlp(paths).await?;
    let mut cmd = Command::new(&ytdlp);
    cmd.arg("--flat-playlist")
        .arg("-J")
        .arg("--no-download")
        .arg("--ignore-errors")
        .arg("--no-warnings");
    apply_ytdlp_runtime(&mut cmd);
    apply_ytdlp_auth(&mut cmd, auth);
    cmd.arg(url);
    let output = cmd
        .output()
        .await
        .map_err(|e| AppError::msg(format!("Failed to inspect playlist: {e}")))?;
    if !output.status.success() && output.stdout.is_empty() {
        let err = String::from_utf8_lossy(&output.stderr);
        let line = err.lines().last().unwrap_or("yt-dlp failed");
        return Err(AppError::msg(friendly_download_error(line)));
    }
    let json: Value = serde_json::from_slice(&output.stdout).map_err(|_| {
        AppError::msg("Playlist metadata was not valid JSON. Try a single track URL.")
    })?;
    let entries = json
        .get("entries")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut items = Vec::new();
    for entry in entries {
        if items.len() >= 80 {
            break;
        }
        if entry.is_null() {
            continue;
        }
        let id = entry.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let webpage = entry
            .get("webpage_url")
            .or_else(|| entry.get("url"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let source = if webpage.starts_with("http") {
            webpage.to_string()
        } else if !id.is_empty() {
            format!("https://www.youtube.com/watch?v={id}")
        } else {
            continue;
        };
        let title = entry
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Playlist item")
            .to_string();
        let artist = entry
            .get("uploader")
            .or_else(|| entry.get("channel"))
            .and_then(|v| v.as_str())
            .unwrap_or("YouTube")
            .to_string();
        items.push(ResolvedItem {
            title,
            artist,
            source_url: source.clone(),
            source_kind: kind_from_url(url),
            ytdlp_query: source,
        });
    }
    if items.is_empty() {
        return Err(AppError::msg(
            "That playlist had no playable entries. Paste a single video URL instead.",
        ));
    }
    let title = json
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("YouTube playlist")
        .to_string();
    let artist = json
        .get("uploader")
        .or_else(|| json.get("channel"))
        .and_then(|v| v.as_str())
        .unwrap_or("YouTube")
        .to_string();
    let cover_url = json
        .get("thumbnails")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.last())
        .and_then(|thumb| thumb.get("url"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            json.get("thumbnail")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        });
    Ok(ResolvedBatch {
        playlist: Some(PlaylistMeta {
            title,
            artist,
            source_url: url.to_string(),
            source_kind: kind_from_url(url),
            cover_url,
        }),
        items,
    })
}

async fn expand_spotify(input: &str) -> AppResult<ResolvedBatch> {
    let url = normalize_spotify_url(input);
    if url.contains("/playlist/") || url.contains("/album/") {
        let tracks = scrape_spotify_tracks(&url).await?;
        let meta = spotify_oembed(&url).await.ok();
        if tracks.is_empty() {
            let title = meta
                .as_ref()
                .map(|m| m.title.clone())
                .unwrap_or_else(|| "this collection".into());
            return Err(AppError::msg(format!(
                "Could not read Spotify tracks from “{title}”. Open the playlist, copy individual track links, or paste the YouTube Music playlist URL."
            )));
        }
        let kind = if url.contains("/album/") { "spotify-album" } else { "spotify-playlist" };
        return Ok(ResolvedBatch {
            playlist: Some(PlaylistMeta {
                title: meta
                    .as_ref()
                    .map(|m| m.title.clone())
                    .unwrap_or_else(|| "Spotify playlist".into()),
                artist: meta
                    .as_ref()
                    .map(|m| m.artist.clone())
                    .unwrap_or_else(|| "Spotify".into()),
                source_url: url,
                source_kind: kind.into(),
                cover_url: meta.and_then(|m| m.thumbnail),
            }),
            items: tracks,
        });
    }
    let meta = spotify_track_meta(&url).await?;
    Ok(ResolvedBatch {
        playlist: None,
        items: vec![spotify_item(meta.title, meta.artist, url)],
    })
}

fn normalize_spotify_url(input: &str) -> String {
    if let Some(rest) = input.strip_prefix("spotify:track:") {
        return format!("https://open.spotify.com/track/{rest}");
    }
    if let Some(rest) = input.strip_prefix("spotify:playlist:") {
        return format!("https://open.spotify.com/playlist/{rest}");
    }
    if let Some(rest) = input.strip_prefix("spotify:album:") {
        return format!("https://open.spotify.com/album/{rest}");
    }
    input.split('?').next().unwrap_or(input).to_string()
}

struct OembedMeta {
    title: String,
    artist: String,
    thumbnail: Option<String>,
}

async fn spotify_oembed(url: &str) -> AppResult<OembedMeta> {
    let client = http_client()?;
    let encoded = urlencoding_lite(url);
    let endpoint = format!("https://open.spotify.com/oembed?url={encoded}");
    let json: Value = client.get(endpoint).send().await?.error_for_status()?.json().await?;
    let title = json
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown title")
        .to_string();
    let artist = json
        .get("author_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown artist")
        .to_string();
    let thumbnail = json
        .get("thumbnail_url")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Ok(OembedMeta {
        title,
        artist,
        thumbnail,
    })
}

fn is_placeholder_artist(artist: &str) -> bool {
    let artist = artist.trim();
    artist.is_empty()
        || eq_ignore_case(artist, "unknown artist")
        || eq_ignore_case(artist, "spotify")
        || eq_ignore_case(artist, "various artists")
}

fn split_title_artist(raw: &str) -> (String, Option<String>) {
    let raw = raw
        .trim()
        .trim_end_matches(" | Spotify")
        .trim()
        .to_string();
    let lyrics = raw.split(" - song and lyrics by ").collect::<Vec<_>>();
    if lyrics.len() == 2 {
        return (lyrics[0].trim().to_string(), Some(lyrics[1].trim().to_string()));
    }
    for sep in [" · ", " – ", " — ", " - "] {
        if let Some((title, rest)) = raw.split_once(sep) {
            let title = title.trim();
            let artist = rest
                .split(sep)
                .next()
                .unwrap_or(rest)
                .trim()
                .trim_end_matches(" Spotify")
                .trim();
            if !title.is_empty() && !artist.is_empty() && !eq_ignore_case(artist, "spotify") {
                return (title.to_string(), Some(artist.to_string()));
            }
        }
    }
    (raw, None)
}

pub fn youtube_search_query(title: &str, artist: &str) -> String {
    let (clean_title, from_title) = split_title_artist(title);
    let artist = if is_placeholder_artist(artist) {
        from_title.unwrap_or_default()
    } else {
        artist.trim().to_string()
    };
    if artist.is_empty() {
        format!("ytsearch1:{clean_title}")
    } else {
        format!("ytsearch1:{clean_title} {artist}")
    }
}

/// Query to hand to yt-dlp. Prefer the stored search string so Spotify retries
/// do not hit open.spotify.com instead of YouTube.
pub fn resolve_download_query(
    source_kind: &str,
    title: &str,
    artist: &str,
    source_url: Option<&str>,
    stored: Option<&str>,
) -> String {
    if let Some(query) = stored.map(str::trim).filter(|q| !q.is_empty()) {
        return query.to_string();
    }
    if source_kind.eq_ignore_ascii_case("spotify") {
        return youtube_search_query(title, artist);
    }
    source_url.unwrap_or("").to_string()
}

fn spotify_item(title: String, artist: String, source_url: String) -> ResolvedItem {
    let (clean_title, from_title) = split_title_artist(&title);
    let artist = if is_placeholder_artist(&artist) {
        from_title.unwrap_or_else(|| artist.clone())
    } else {
        artist
    };
    let ytdlp_query = youtube_search_query(&clean_title, &artist);
    ResolvedItem {
        title: clean_title,
        artist,
        source_url,
        source_kind: "spotify".into(),
        ytdlp_query,
    }
}

async fn spotify_track_meta(url: &str) -> AppResult<OembedMeta> {
    let mut meta = match spotify_oembed(url).await {
        Ok(meta) => meta,
        Err(_) => scrape_spotify_track_page(url).await?,
    };
    if is_placeholder_artist(&meta.artist) {
        if let Ok(scraped) = scrape_spotify_track_page(url).await {
            if !is_placeholder_artist(&scraped.artist) {
                meta.artist = scraped.artist;
            }
            if !scraped.title.is_empty() && (meta.title.is_empty() || meta.title == "Unknown title") {
                meta.title = scraped.title;
            }
        }
    }
    let (title, from_title) = split_title_artist(&meta.title);
    meta.title = title;
    if is_placeholder_artist(&meta.artist) {
        if let Some(artist) = from_title {
            meta.artist = artist;
        }
    }
    Ok(meta)
}

async fn scrape_spotify_track_page(url: &str) -> AppResult<OembedMeta> {
    let client = http_client()?;
    let html = client.get(url).send().await?.error_for_status()?.text().await?;
    let og_title = capture_first(&html, r#"property="og:title"\s+content="([^"]+)""#)
        .or_else(|| capture_first(&html, r#"content="([^"]+)"\s+property="og:title""#));
    let og_desc = capture_first(&html, r#"property="og:description"\s+content="([^"]+)""#);
    let json_artist = capture_first(&html, r#""artists"\s*:\s*\[\s*\{\s*"name"\s*:\s*"([^"]+)""#)
        .or_else(|| capture_first(&html, r#""author_name"\s*:\s*"([^"]+)""#));
    let raw_title = og_title.unwrap_or_else(|| "Unknown title".into());
    let (title, from_title) = split_title_artist(&raw_title);
    let artist = json_artist
        .or(from_title)
        .or_else(|| {
            og_desc.and_then(|desc| {
                desc.split(" · ").nth(1).map(|s| s.trim().to_string())
            })
        })
        .unwrap_or_else(|| "Unknown artist".into());
    let thumbnail = capture_first(&html, r#"property="og:image"\s+content="([^"]+)""#);
    Ok(OembedMeta {
        title,
        artist,
        thumbnail,
    })
}

fn capture_first(haystack: &str, pattern: &str) -> Option<String> {
    Regex::new(pattern)
        .ok()
        .and_then(|re| re.captures(haystack))
        .and_then(|cap| cap.get(1).map(|m| html_unescape(m.as_str())))
}

fn html_unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

async fn scrape_spotify_tracks(url: &str) -> AppResult<Vec<ResolvedItem>> {
    let client = http_client()?;
    let mut html = client.get(url).send().await?.error_for_status()?.text().await?;
    let mut ids = extract_spotify_track_ids(&html);
    if ids.is_empty() {
        if let Some(embed) = embed_url(url) {
            if let Ok(body) = client.get(&embed).send().await {
                if let Ok(text) = body.text().await {
                    html = text;
                    ids = extract_spotify_track_ids(&html);
                }
            }
        }
    }
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id.clone()) {
            continue;
        }
        let track_url = format!("https://open.spotify.com/track/{id}");
        if let Ok(meta) = spotify_track_meta(&track_url).await {
            items.push(spotify_item(meta.title, meta.artist, track_url));
        }
        if items.len() >= 50 {
            break;
        }
    }
    Ok(items)
}

fn embed_url(url: &str) -> Option<String> {
    if let Some(id) = url.split("/playlist/").nth(1) {
        return Some(format!(
            "https://open.spotify.com/embed/playlist/{}",
            id.trim_end_matches('/')
        ));
    }
    if let Some(id) = url.split("/album/").nth(1) {
        return Some(format!(
            "https://open.spotify.com/embed/album/{}",
            id.trim_end_matches('/')
        ));
    }
    None
}

pub fn extract_spotify_track_ids(html: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut seen = HashSet::new();
    let patterns = [
        Regex::new(r"spotify:track:([A-Za-z0-9]{22})").ok(),
        Regex::new(r"open\.spotify\.com/track/([A-Za-z0-9]{22})").ok(),
        Regex::new(r#""uri"\s*:\s*"spotify:track:([A-Za-z0-9]{22})""#).ok(),
    ];
    for re in patterns.into_iter().flatten() {
        for cap in re.captures_iter(html) {
            let id = cap[1].to_string();
            if seen.insert(id.clone()) {
                ids.push(id);
            }
        }
    }
    ids
}

fn urlencoding_lite(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[derive(Debug, Deserialize)]
struct YtMeta {
    title: Option<String>,
    uploader: Option<String>,
    artist: Option<String>,
    track: Option<String>,
    thumbnail: Option<String>,
    duration: Option<f64>,
}

pub struct DownloadResult {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub cover_path: Option<PathBuf>,
    pub duration_ms: Option<i64>,
}

pub async fn download_audio(
    paths: &AppPaths,
    dest_dir: &Path,
    query: &str,
    auth: &YtAuth<'_>,
    download_format: &str,
    mut on_progress: impl FnMut(f32, String),
) -> AppResult<DownloadResult> {
    std::fs::create_dir_all(dest_dir)?;
    let ytdlp = tools::ensure_ytdlp(paths).await?;
    on_progress(0.05, "Resolving stream metadata".into());

    let strategies = download_strategies(download_format);
    let mut last_err = String::from("yt-dlp failed");

    for (index, strategy) in strategies.iter().enumerate() {
        clean_source_files(dest_dir);
        if index > 0 {
            on_progress(
                0.08,
                format!("Retrying download ({})", strategy.label),
            );
        }
        match run_ytdlp_download(
            &ytdlp,
            dest_dir,
            query,
            auth,
            strategy,
            &mut on_progress,
        )
        .await
        {
            Ok(result) => return Ok(result),
            Err(err) => {
                last_err = err.to_string();
                if index + 1 < strategies.len() && is_retryable_download_error(&last_err) {
                    continue;
                }
                return Err(AppError::msg(friendly_download_error(&last_err)));
            }
        }
    }

    Err(AppError::msg(friendly_download_error(&last_err)))
}

fn clean_source_files(dest_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dest_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with("source.") {
            let _ = std::fs::remove_file(path);
        }
    }
}

async fn run_ytdlp_download(
    ytdlp: &Path,
    dest_dir: &Path,
    query: &str,
    auth: &YtAuth<'_>,
    strategy: &DownloadStrategy,
    on_progress: &mut impl FnMut(f32, String),
) -> AppResult<DownloadResult> {
    let output_tpl = dest_dir.join("source.%(ext)s");
    let mut cmd = Command::new(ytdlp);
    apply_ytdlp_runtime(&mut cmd);
    if let Some(format) = strategy.format {
        cmd.arg("-f").arg(format);
    }
    if strategy.extract_audio {
        cmd.arg("-x")
            .arg("--audio-format")
            .arg("best")
            .arg("--prefer-ffmpeg");
    }
    cmd.arg("--no-playlist")
        .arg("--no-mtime")
        .arg("--newline")
        .arg("-o")
        .arg(output_tpl.to_string_lossy().as_ref())
        .arg("--print-json")
        .arg("--no-progress")
        .arg("--restrict-filenames")
        .arg("--retries")
        .arg("3")
        .arg("--fragment-retries")
        .arg("3");
    apply_ytdlp_cookies(&mut cmd, auth);
    apply_ytdlp_client(&mut cmd, strategy.client_args);
    cmd.arg(query)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::msg(format!("Failed to start yt-dlp: {e}")))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::msg("yt-dlp stdout missing"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::msg("yt-dlp stderr missing"))?;

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();
    let mut json_line = None;
    let mut err_buf = String::new();

    loop {
        tokio::select! {
            line = stdout_reader.next_line() => {
                match line {
                    Ok(Some(text)) => {
                        if text.starts_with('{') {
                            json_line = Some(text);
                        }
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
            line = stderr_reader.next_line() => {
                if let Ok(Some(text)) = line {
                    if let Some(pct) = parse_percent(&text) {
                        on_progress((pct / 100.0).clamp(0.05, 0.9), format!("Downloading {pct:.0}%"));
                    }
                    err_buf.push_str(&text);
                    err_buf.push('\n');
                }
            }
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| AppError::msg(format!("yt-dlp wait failed: {e}")))?;
    if !status.success() {
        let message = err_buf
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("yt-dlp failed");
        return Err(AppError::msg(message));
    }

    let meta: YtMeta = json_line
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(YtMeta {
            title: None,
            uploader: None,
            artist: None,
            track: None,
            thumbnail: None,
            duration: None,
        });

    let path = find_source_file(dest_dir)?;
    let title = meta
        .track
        .or(meta.title)
        .unwrap_or_else(|| "Unknown title".into());
    let artist = meta
        .artist
        .or(meta.uploader)
        .unwrap_or_else(|| "Unknown artist".into());
    let duration_ms = meta.duration.map(|d| (d * 1000.0) as i64);
    let cover_path = if let Some(thumb) = meta.thumbnail {
        let cover = dest_dir.join("cover.jpg");
        if download_cover(&thumb, &cover).await.is_ok() {
            Some(cover)
        } else {
            None
        }
    } else {
        None
    };

    on_progress(1.0, "Download complete".into());
    Ok(DownloadResult {
        path,
        title,
        artist,
        cover_path,
        duration_ms,
    })
}

pub async fn download_cover(url: &str, dest: &Path) -> AppResult<()> {
    let bytes = http_client()?.get(url).send().await?.error_for_status()?.bytes().await?;
    tokio::fs::write(dest, bytes).await?;
    Ok(())
}

fn parse_percent(line: &str) -> Option<f32> {
    let re = Regex::new(r"(\d{1,3}(?:\.\d+)?)%").ok()?;
    re.captures(line)
        .and_then(|c| c[1].parse::<f32>().ok())
        .filter(|v| *v <= 100.0)
}

fn find_source_file(dir: &Path) -> AppResult<PathBuf> {
    let mut found = None;
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with("source.") && !name.ends_with(".partial") {
            found = Some(path);
            break;
        }
    }
    found.ok_or_else(|| AppError::msg("yt-dlp did not write an audio file"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_watch_url_is_not_playlist() {
        assert!(!is_youtube_playlist_url(
            "https://music.youtube.com/watch?v=abc123&list=PLxxxx"
        ));
    }

    #[test]
    fn bot_error_is_friendly() {
        let msg = friendly_download_error("ERROR: Sign in to confirm you're not a bot");
        assert!(msg.contains("cookies"));
    }

    #[test]
    fn format_error_is_friendly() {
        let msg = friendly_download_error("ERROR: Requested format is not available");
        assert!(msg.contains("Settings") || msg.contains("stream"));
    }

    #[test]
    fn auto_format_has_fallbacks() {
        assert!(download_strategies("auto").len() >= 5);
    }

    #[test]
    fn auto_has_multiple_strategies() {
        assert!(download_strategies("auto").len() >= 8);
    }

    #[test]
    fn bot_strategy_is_first() {
        let strategies = download_strategies("auto");
        assert_eq!(strategies[0].label, "bot · m4a");
        assert_eq!(strategies[0].format, Some(BOT_YT_FORMAT));
        assert_eq!(strategies[0].client_args, Some(BOT_YT_CLIENT));
    }

    #[test]
    fn youtube_search_uses_title_and_artist() {
        assert_eq!(
            youtube_search_query("Introvert", "ReoNa"),
            "ytsearch1:Introvert ReoNa"
        );
    }

    #[test]
    fn youtube_search_parses_combined_title() {
        assert_eq!(
            youtube_search_query("SAVANA · Artist Name · Spotify", "Unknown artist"),
            "ytsearch1:SAVANA Artist Name"
        );
    }

    #[test]
    fn youtube_search_parses_lyrics_by() {
        assert_eq!(
            youtube_search_query("Nightcall - song and lyrics by Kavinsky | Spotify", "Spotify"),
            "ytsearch1:Nightcall Kavinsky"
        );
    }

    #[test]
    fn retry_uses_stored_youtube_search() {
        assert_eq!(
            resolve_download_query(
                "spotify",
                "Introvert",
                "ReoNa",
                Some("https://open.spotify.com/track/abc"),
                Some("ytsearch1:Introvert ReoNa"),
            ),
            "ytsearch1:Introvert ReoNa"
        );
    }

    #[test]
    fn retry_rebuilds_spotify_search_without_stored_query() {
        assert_eq!(
            resolve_download_query(
                "spotify",
                "Introvert",
                "ReoNa",
                Some("https://open.spotify.com/track/abc"),
                None,
            ),
            "ytsearch1:Introvert ReoNa"
        );
    }
}
