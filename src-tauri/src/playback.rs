use std::path::{Path, PathBuf};

use crate::error::AppResult;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub fn start(tracks_root: PathBuf) -> AppResult<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    tauri::async_runtime::spawn(async move {
        let listener = match TcpListener::from_std(listener) {
            Ok(l) => l,
            Err(_) => return,
        };
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                continue;
            };
            let root = tracks_root.clone();
            tauri::async_runtime::spawn(async move {
                let _ = serve(stream, &root).await;
            });
        }
    });
    Ok(port)
}

async fn serve(mut stream: TcpStream, tracks_root: &Path) -> std::io::Result<()> {
    let mut buf = vec![0u8; 16_384];
    let n = stream.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }
    let request = String::from_utf8_lossy(&buf[..n]);
    let mut lines = request.lines();
    let first = lines.next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    if method != "GET" && method != "HEAD" {
        write_status(&mut stream, 405, "Method Not Allowed", b"").await?;
        return Ok(());
    }

    let mut range = None;
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Range: bytes=") {
            range = parse_range(value);
        }
    }

    let Some(hex) = target.strip_prefix("/stem/") else {
        write_status(&mut stream, 404, "Not Found", b"").await?;
        return Ok(());
    };
    let hex = hex.split('?').next().unwrap_or(hex);
    let Some(path) = decode_hex_path(hex) else {
        write_status(&mut stream, 400, "Bad Request", b"").await?;
        return Ok(());
    };
    let Ok(file) = PathBuf::from(&path).canonicalize() else {
        write_status(&mut stream, 404, "Not Found", b"").await?;
        return Ok(());
    };
    let Ok(root) = tracks_root.canonicalize() else {
        write_status(&mut stream, 500, "Server Error", b"").await?;
        return Ok(());
    };
    if !file.starts_with(&root) || !file.is_file() {
        write_status(&mut stream, 403, "Forbidden", b"").await?;
        return Ok(());
    }

    let meta = tokio::fs::metadata(&file).await?;
    let total = meta.len();
    let (start, end) = match range {
        Some((s, e)) => {
            let start = s.min(total.saturating_sub(1));
            let end = e.unwrap_or(total.saturating_sub(1)).min(total.saturating_sub(1));
            if start > end {
                write_status(&mut stream, 416, "Range Not Satisfiable", b"").await?;
                return Ok(());
            }
            (start, end)
        }
        None => (0, total.saturating_sub(1)),
    };
    let length = end.saturating_sub(start) + 1;
    let status = if range.is_some() { "206 Partial Content" } else { "200 OK" };
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: audio/wav\r\nAccept-Ranges: bytes\r\nContent-Length: {length}\r\nContent-Range: bytes {start}-{end}/{total}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(header.as_bytes()).await?;
    if method == "HEAD" {
        stream.flush().await?;
        return Ok(());
    }

    let mut file = tokio::fs::File::open(&file).await?;
    if start > 0 {
        use tokio::io::AsyncSeekExt;
        file.seek(std::io::SeekFrom::Start(start)).await?;
    }
    let mut remaining = length;
    let mut chunk = vec![0u8; 64 * 1024];
    while remaining > 0 {
        let want = remaining.min(chunk.len() as u64) as usize;
        let read = file.read(&mut chunk[..want]).await?;
        if read == 0 {
            break;
        }
        stream.write_all(&chunk[..read]).await?;
        remaining -= read as u64;
    }
    stream.flush().await?;
    Ok(())
}

async fn write_status(stream: &mut TcpStream, code: u16, reason: &str, body: &[u8]) -> std::io::Result<()> {
    let header = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await
}

fn parse_range(value: &str) -> Option<(u64, Option<u64>)> {
    let value = value.trim();
    let (start, end) = value.split_once('-')?;
    let start = if start.is_empty() { 0 } else { start.parse().ok()? };
    let end = if end.is_empty() { None } else { Some(end.parse().ok()?) };
    Some((start, end))
}

fn decode_hex_path(hex: &str) -> Option<String> {
    if hex.len() % 2 != 0 || hex.is_empty() {
        return None;
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    let chars: Vec<char> = hex.chars().collect();
    for pair in chars.chunks(2) {
        let s: String = pair.iter().collect();
        bytes.push(u8::from_str_radix(&s, 16).ok()?);
    }
    String::from_utf8(bytes).ok()
}

pub fn encode_hex_path(path: &str) -> String {
    path.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

#[allow(dead_code)]
pub fn stem_url(port: u16, path: &str) -> String {
    format!("http://127.0.0.1:{port}/stem/{}", encode_hex_path(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        let original = "/home/zeit/.cache/com.z3itt.dualis/tracks/abc/vocals.wav";
        assert_eq!(decode_hex_path(&encode_hex_path(original)).as_deref(), Some(original));
    }
}
